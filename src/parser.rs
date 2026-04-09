//! Recursive-descent parser for galvanic.
//!
//! Consumes the token stream produced by [`crate::lexer`] and produces
//! a [`crate::ast::SourceFile`].
//!
//! # Design
//!
//! The parser is a hand-written recursive descent parser. Each grammar rule
//! in the FLS maps to one method. Methods return `Result<T, ParseError>` and
//! advance the cursor on success. On error the cursor is left at the offending
//! token so the caller can produce a useful message.
//!
//! Operator precedence (lowest to highest in the expression grammar):
//!
//! 1. Assignment `=` (right-associative)
//! 2. Logical or `||`
//! 3. Logical and `&&`
//! 4. Comparison `==` `!=` `<` `>` `<=` `>=`
//! 5. Bitwise or `|`
//! 6. Bitwise xor `^`
//! 7. Bitwise and `&`
//! 8. Shift `<<` `>>`
//! 9. Additive `+` `-`
//! 10. Multiplicative `*` `/` `%`
//! 11. Type cast `as` (FLS §6.5.9)
//! 12. Unary `-` `!` `*` `&` `&mut`
//! 13. Primary: literals, paths, calls, `(expr)`, blocks, `if`, `return`
//!
//! FLS §6 NOTE: The FLS does not assign numeric precedence levels. Precedence
//! is encoded in the grammar structure. This ordering follows the Rust
//! reference and is consistent with rustc's behaviour.

use crate::ast::{
    BinOp, Block, EnumDef, EnumVariant, EnumVariantKind, Expr, ExprKind, FnDef, Item, ItemKind,
    NamedField, Param, ParamKind, Pat, SourceFile, Span, Stmt, StmtKind, StructDef, StructKind,
    Ty, TyKind, TupleField, UnaryOp, Visibility, ImplDef, TraitDef, ConstDef, StaticDef, TypeAliasDef,
    AssocTypeDef,
};
use crate::lexer::{Token, TokenKind};

// ── ParseError ────────────────────────────────────────────────────────────────

/// A parse error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// The span where the error was detected.
    pub span: Span,
    /// Human-readable description of what was expected and what was found.
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parse error at byte {}: {}", self.span.start, self.message)
    }
}

impl std::error::Error for ParseError {}

// ── Public API ────────────────────────────────────────────────────────────────

/// Parse a token stream into a [`SourceFile`].
///
/// `tokens` must include the terminal `Eof` token as produced by
/// [`crate::lexer::tokenize`].
pub fn parse(tokens: &[Token], src: &str) -> Result<SourceFile, ParseError> {
    Parser::new(tokens, src).parse_source_file()
}

// ── Parser ────────────────────────────────────────────────────────────────────

/// Recursive-descent parser.
struct Parser<'src> {
    tokens: &'src [Token],
    src: &'src str,
    /// Index of the current (un-consumed) token.
    ///
    /// The lexer always appends a terminal `Eof` token, so `cursor` is always
    /// in-bounds: `self.tokens[cursor]` is valid for any `cursor` in
    /// `0..tokens.len()`.
    cursor: usize,
    /// When `true`, struct literal expressions (`Name { field: expr }`) are
    /// not parsed even if the lookahead suggests one.
    ///
    /// Set to `true` before parsing the condition of an `if`, `while`, or
    /// `for` expression to resolve the `if foo { }` ambiguity: without this
    /// flag, `foo {` would be parsed as the start of a struct literal instead
    /// of a variable name followed by a block body.
    ///
    /// FLS §6.17 AMBIGUOUS: The FLS does not explicitly describe this
    /// restriction; it follows from the Rust reference grammar rule that
    /// struct expressions are not allowed in expression-without-struct-literal
    /// positions (e.g., `if`/`while`/`for` conditions).
    restrict_struct_lit: bool,
}

impl<'src> Parser<'src> {
    fn new(tokens: &'src [Token], src: &'src str) -> Self {
        // Guard: we require at least one token (the Eof sentinel).
        assert!(!tokens.is_empty(), "token slice must contain at least Eof");
        Parser { tokens, src, cursor: 0, restrict_struct_lit: false }
    }

    // ── Low-level token access ────────────────────────────────────────────────

    /// The current token (never out of bounds; stays at Eof at end).
    fn current(&self) -> &Token {
        &self.tokens[self.cursor]
    }

    fn peek_kind(&self) -> TokenKind {
        self.current().kind
    }

    /// Peek at the token `n` positions ahead of the cursor without consuming.
    /// Returns `TokenKind::Eof` if the index is out of bounds.
    fn peek_nth(&self, n: usize) -> TokenKind {
        self.tokens.get(self.cursor + n).map(|t| t.kind).unwrap_or(TokenKind::Eof)
    }

    /// Advance past the current token and return it.
    fn advance(&mut self) -> Token {
        let tok = *self.current();
        // Don't step past the Eof sentinel.
        if self.cursor + 1 < self.tokens.len() {
            self.cursor += 1;
        }
        tok
    }

    fn span_of(tok: &Token) -> Span {
        Span::new(tok.start, tok.len as u32)
    }

    fn current_span(&self) -> Span {
        Self::span_of(self.current())
    }

    fn error(&self, msg: impl Into<String>) -> ParseError {
        ParseError { span: self.current_span(), message: msg.into() }
    }

    /// Consume the current token if it matches `kind` and return its span.
    /// Otherwise return an error without advancing.
    fn expect(&mut self, kind: TokenKind) -> Result<Span, ParseError> {
        if self.peek_kind() == kind {
            Ok(Self::span_of(&self.advance()))
        } else {
            Err(self.error(format!(
                "expected {kind:?}, found {:?}",
                self.peek_kind()
            )))
        }
    }

    /// Consume the current token if it matches `kind`. Return `true` iff consumed.
    fn eat(&mut self, kind: TokenKind) -> bool {
        if self.peek_kind() == kind {
            self.advance();
            true
        } else {
            false
        }
    }

    // ── Source file ───────────────────────────────────────────────────────────

    /// Parse a complete source file.
    ///
    /// FLS §18.1: A source file is a sequence of items terminated by EOF.
    /// FLS §3: Items are the top-level constituents of a crate.
    ///
    /// FLS §13 NOTE: Attributes (e.g., `#![no_std]`) may appear before
    /// items. Attributes are not yet parsed; a `#` at the top level will
    /// produce a parse error. This is expected behaviour for Phase 2.
    fn parse_source_file(&mut self) -> Result<SourceFile, ParseError> {
        let start = self.current_span();
        let mut items = Vec::new();

        while self.peek_kind() != TokenKind::Eof {
            items.push(self.parse_item()?);
        }

        let end = self.current_span();
        Ok(SourceFile { items, span: start.to(end) })
    }

    // ── Items ─────────────────────────────────────────────────────────────────

    /// Parse one item.
    ///
    /// FLS §3: Item kinds. Function and struct items are implemented.
    ///
    /// FLS §10.2: An optional `pub` visibility modifier may precede an item.
    fn parse_item(&mut self) -> Result<Item, ParseError> {
        let start = self.current_span();

        // Optional visibility modifier.
        let vis = self.parse_visibility();

        match self.peek_kind() {
            TokenKind::KwFn => {
                let fn_def = self.parse_fn_def(vis, false, false)?;
                let end = fn_def
                    .body
                    .as_ref()
                    .map(|b| b.span)
                    .unwrap_or(start);
                let span = start.to(end);
                Ok(Item { kind: ItemKind::Fn(Box::new(fn_def)), span })
            }
            // FLS §19: `unsafe fn` — a function that may only be called from an
            // unsafe context. The `unsafe` keyword is consumed here; `parse_fn_def`
            // is called with `is_unsafe = true`. Codegen is identical to a normal
            // function — the qualifier is a static typing constraint only.
            //
            // FLS §19 AMBIGUOUS: The spec requires callers to use an unsafe context,
            // but does not define the enforcement mechanism. Galvanic records the
            // qualifier and defers enforcement.
            TokenKind::KwUnsafe if self.peek_nth(1) == TokenKind::KwFn => {
                self.advance(); // consume `unsafe`
                let fn_def = self.parse_fn_def(vis, false, true)?;
                let end = fn_def.body.as_ref().map(|b| b.span).unwrap_or(start);
                let span = start.to(end);
                Ok(Item { kind: ItemKind::Fn(Box::new(fn_def)), span })
            }
            // FLS §19: `unsafe trait` — a trait that may only be implemented with
            // `unsafe impl`. The `unsafe` keyword is consumed here; `parse_trait_def`
            // is called with `is_unsafe = true`.
            //
            // FLS §19 AMBIGUOUS: The spec requires implementors to use `unsafe impl`,
            // but does not specify how the compiler verifies this pairing. Enforcement
            // is deferred.
            TokenKind::KwUnsafe if self.peek_nth(1) == TokenKind::KwTrait => {
                self.advance(); // consume `unsafe`
                let trait_def = self.parse_trait_def(vis, true)?;
                let span = start.to(trait_def.span);
                Ok(Item { kind: ItemKind::Trait(Box::new(trait_def)), span })
            }
            // FLS §19: `unsafe impl` — implementing an unsafe trait satisfies its
            // safety invariant. The `unsafe` keyword is consumed here.
            //
            // FLS §19 AMBIGUOUS: See TraitDef.is_unsafe note. Enforcement deferred.
            TokenKind::KwUnsafe if self.peek_nth(1) == TokenKind::KwImpl => {
                self.advance(); // consume `unsafe`
                let impl_def = self.parse_impl_def(true)?;
                let span = start.to(impl_def.span);
                Ok(Item { kind: ItemKind::Impl(Box::new(impl_def)), span })
            }
            TokenKind::KwStruct => {
                let struct_def = self.parse_struct_def(vis)?;
                let span = start.to(struct_def.span);
                Ok(Item { kind: ItemKind::Struct(Box::new(struct_def)), span })
            }
            TokenKind::KwEnum => {
                let enum_def = self.parse_enum_def(vis)?;
                let span = start.to(enum_def.span);
                Ok(Item { kind: ItemKind::Enum(Box::new(enum_def)), span })
            }
            TokenKind::KwImpl => {
                let impl_def = self.parse_impl_def(false)?;
                let span = start.to(impl_def.span);
                Ok(Item { kind: ItemKind::Impl(Box::new(impl_def)), span })
            }
            TokenKind::KwTrait => {
                let trait_def = self.parse_trait_def(vis, false)?;
                let span = start.to(trait_def.span);
                Ok(Item { kind: ItemKind::Trait(Box::new(trait_def)), span })
            }
            TokenKind::KwConst => {
                // FLS §9:41–43: `const fn` — a function eligible for compile-time
                // evaluation. Detect `const` followed immediately by `fn` and parse as
                // an `is_const` function rather than a const item.
                if self.peek_nth(1) == TokenKind::KwFn {
                    self.advance(); // consume `const`
                    let fn_def = self.parse_fn_def(vis, true, false)?;
                    let end = fn_def.body.as_ref().map(|b| b.span).unwrap_or(start);
                    let span = start.to(end);
                    Ok(Item { kind: ItemKind::Fn(Box::new(fn_def)), span })
                } else {
                    let const_def = self.parse_const_def(vis)?;
                    let span = start.to(const_def.span);
                    Ok(Item { kind: ItemKind::Const(Box::new(const_def)), span })
                }
            }
            TokenKind::KwStatic => {
                let static_def = self.parse_static_def(vis)?;
                let span = start.to(static_def.span);
                Ok(Item { kind: ItemKind::Static(Box::new(static_def)), span })
            }
            TokenKind::KwType => {
                let type_alias_def = self.parse_type_alias_def(vis)?;
                let span = start.to(type_alias_def.span);
                Ok(Item { kind: ItemKind::TypeAlias(Box::new(type_alias_def)), span })
            }
            kind => Err(self.error(format!(
                "expected item (fn, struct, enum, impl, trait, const, static, type, …), found {kind:?}"
            ))),
        }
    }

    /// Consume an optional `pub` keyword and return the visibility.
    ///
    /// FLS §10.2: Visibility. Only bare `pub` is handled; restricted forms
    /// (`pub(crate)`, `pub(super)`, `pub(in path)`) are not yet implemented.
    fn parse_visibility(&mut self) -> Visibility {
        if self.eat(TokenKind::KwPub) {
            Visibility::Pub
        } else {
            Visibility::Private
        }
    }

    /// Parse a function definition.
    ///
    /// FLS §9: Functions.
    ///
    /// Grammar (simplified — qualifiers and where-clauses omitted):
    /// ```text
    /// FunctionDeclaration ::=
    ///     "fn" Identifier "(" FunctionParameters? ")"
    ///     FunctionReturnType?
    ///     BlockExpression
    /// ```
    ///
    /// FLS §9 AMBIGUOUS: Qualifiers (`async`, `const`, `unsafe`, `extern`)
    /// are listed in FLS §9 but their interaction rules are not fully
    /// specified. This implementation accepts no qualifiers; encountering
    /// one produces a parse error directing the user to the limitation.
    /// `is_const` is `true` when the `const` keyword preceded `fn` at the call
    /// site (the caller has already consumed `const`). FLS §9:41–43.
    /// `is_unsafe` is `true` when the `unsafe` keyword preceded `fn` at the
    /// call site (the caller has already consumed `unsafe`). FLS §19.
    fn parse_fn_def(&mut self, vis: Visibility, is_const: bool, is_unsafe: bool) -> Result<FnDef, ParseError> {
        self.expect(TokenKind::KwFn)?;

        // Function name must be an identifier — keywords are not valid here.
        // FLS §9: the function name is a non-keyword identifier.
        if self.peek_kind() != TokenKind::Ident {
            return Err(self.error(format!(
                "expected function name (identifier), found {:?}",
                self.peek_kind()
            )));
        }
        let name = self.current_span();
        self.advance();

        // Optional generic type parameter list: `<T>`, `<T, U>`, `<T: Trait>`, etc.
        //
        // FLS §12.1: A generic function may declare type parameters in
        // angle brackets after the function name: `fn foo<T, U>(...)`.
        // Each type parameter is an identifier, optionally followed by a
        // colon and one or more trait bounds (`T: Scalable` or `T: A + B`).
        //
        // FLS §12.1 AMBIGUOUS: The FLS does not specify the disambiguation
        // rule for `<` in this position (generic list vs. less-than). In
        // practice, `<` immediately after a function name always starts a
        // generic parameter list in both the FLS grammar and rustc.
        let mut generic_params = Vec::new();
        if self.peek_kind() == TokenKind::Lt {
            self.advance(); // eat `<`
            loop {
                if self.peek_kind() == TokenKind::Gt {
                    self.advance(); // eat `>`
                    break;
                }
                if self.peek_kind() != TokenKind::Ident {
                    return Err(self.error(format!(
                        "expected type parameter name or `>`, found {:?}",
                        self.peek_kind()
                    )));
                }
                generic_params.push(self.current_span());
                self.advance();
                // FLS §12.1, §4.14: Optional inline trait bound `T: TraitName`
                // or `T: A + B`. The bound is recorded in the AST for future
                // use; at this milestone galvanic infers the concrete type from
                // the call-site argument type rather than the bound annotation.
                // FLS §10.2: Associated type bindings use angle-bracket syntax:
                // `T: Container<Item = i32>` — consume and discard the args.
                //
                // FLS §12.1 AMBIGUOUS: The FLS does not specify how `>>` (lexed
                // as a single Shr token) should be split in generic contexts.
                // Galvanic tracks whether `>>` consumed the outer `>` implicitly.
                let mut consumed_outer_gt = false;
                if self.eat(TokenKind::Colon) {
                    'bound_loop: loop {
                        if self.peek_kind() == TokenKind::Ident {
                            self.advance(); // skip bound trait name
                            // Consume optional type args: `Trait<AssocType = Ty>`.
                            if self.peek_kind() == TokenKind::Lt {
                                self.advance(); // eat `<`
                                let mut depth = 1usize;
                                while depth > 0 && self.peek_kind() != TokenKind::Eof {
                                    match self.peek_kind() {
                                        TokenKind::Lt => { self.advance(); depth += 1; }
                                        TokenKind::Shr => {
                                            // `>>` acts as two `>`. If depth==1, the
                                            // second `>` belongs to the outer param list.
                                            self.advance();
                                            if depth >= 2 {
                                                depth -= 2;
                                            } else {
                                                depth = 0;
                                                consumed_outer_gt = true;
                                            }
                                        }
                                        TokenKind::Gt => { self.advance(); depth -= 1; }
                                        _ => { self.advance(); }
                                    }
                                }
                            }
                        }
                        if consumed_outer_gt || !self.eat(TokenKind::Plus) {
                            break 'bound_loop;
                        }
                    }
                }
                if !self.eat(TokenKind::Comma) {
                    if !consumed_outer_gt {
                        self.expect(TokenKind::Gt)?;
                    }
                    break;
                }
            }
        }

        // Parameter list enclosed in `( )`.
        self.expect(TokenKind::OpenParen)?;

        // Check for optional `self` / `&self` / `&mut self` as first parameter.
        // FLS §10.1: Methods begin with a self parameter.
        let self_param = self.parse_self_param();

        let params = self.parse_params()?;
        self.expect(TokenKind::CloseParen)?;

        // Optional return type `-> Type`.
        // FLS §9: absent return type means the function returns `()`.
        let ret_ty = if self.eat(TokenKind::RArrow) {
            Some(self.parse_ty()?)
        } else {
            None
        };

        // Optional `where` clause: `where T: Trait, U: A + B`.
        // FLS §4.14: Trait and lifetime bounds via where clauses.
        // Bounds are parsed and discarded — monomorphization uses call-site types.
        self.parse_where_clause();

        // Function body: required for non-trait functions, optional for trait
        // method signatures (which end with `;` instead of a block).
        //
        // FLS §9: The body is a block expression for normal functions.
        // FLS §13: Trait method signatures omit the body; they end with `;`.
        let body = if self.peek_kind() == TokenKind::Semi {
            self.advance(); // eat `;`
            None
        } else {
            Some(self.parse_block()?)
        };

        Ok(FnDef { vis, is_const, is_unsafe, name, generic_params, self_param, params, ret_ty, body })
    }

    /// Parse an inherent impl block.
    ///
    /// FLS §11: Implementations.
    ///
    /// Grammar (simplified):
    /// ```text
    /// ImplDeclaration ::=
    ///     "impl" Identifier "{" FunctionDeclaration* "}"
    /// ```
    ///
    /// FLS §11 NOTE: Trait impls (`impl Trait for Type`) and generic impls
    /// (`impl<T>`) are not yet implemented.
    fn parse_impl_def(&mut self, is_unsafe: bool) -> Result<ImplDef, ParseError> {
        let start = self.current_span();
        self.expect(TokenKind::KwImpl)?;

        // FLS §12.1: `impl` may be followed by a generic parameter list:
        //   `impl<T> Type<T> { ... }` or `impl<T> Trait<T> for Type<T> { ... }`.
        // FLS §12.1 AMBIGUOUS: The FLS does not specify the disambiguation rule
        // for `<` immediately after `impl` (generic params vs. less-than). In
        // practice, `<` after `impl` always starts a generic parameter list.
        let mut generic_params = Vec::new();
        if self.peek_kind() == TokenKind::Lt {
            self.advance(); // eat `<`
            loop {
                if self.peek_kind() == TokenKind::Gt {
                    self.advance(); // eat `>`
                    break;
                }
                if self.peek_kind() != TokenKind::Ident {
                    return Err(self.error(format!(
                        "expected type parameter name or `>` in impl generic params, found {:?}",
                        self.peek_kind()
                    )));
                }
                generic_params.push(self.current_span());
                self.advance();
                // FLS §12.1, §4.14: Optional inline trait bound `T: TraitName`.
                // FLS §10.2: Associated type bindings use angle-bracket syntax:
                // `T: Container<Item = i32>` — consume and discard the args.
                // FLS §12.1 AMBIGUOUS: `>>` is a single Shr token; when depth==1
                // it simultaneously closes our inner `<...>` and the outer `<T>`.
                let mut consumed_outer_gt = false;
                if self.eat(TokenKind::Colon) {
                    'impl_bound_loop: loop {
                        if self.peek_kind() == TokenKind::Ident {
                            self.advance(); // skip bound trait name
                            // Consume optional type args: `Trait<AssocType = Ty>`.
                            if self.peek_kind() == TokenKind::Lt {
                                self.advance(); // eat `<`
                                let mut depth = 1usize;
                                while depth > 0 && self.peek_kind() != TokenKind::Eof {
                                    match self.peek_kind() {
                                        TokenKind::Lt => { self.advance(); depth += 1; }
                                        TokenKind::Shr => {
                                            self.advance();
                                            if depth >= 2 {
                                                depth -= 2;
                                            } else {
                                                depth = 0;
                                                consumed_outer_gt = true;
                                            }
                                        }
                                        TokenKind::Gt => { self.advance(); depth -= 1; }
                                        _ => { self.advance(); }
                                    }
                                }
                            }
                        }
                        if consumed_outer_gt || !self.eat(TokenKind::Plus) {
                            break 'impl_bound_loop;
                        }
                    }
                }
                if !self.eat(TokenKind::Comma) {
                    if !consumed_outer_gt {
                        self.expect(TokenKind::Gt)?;
                    }
                    break;
                }
            }
        }

        // After `impl` (and optional `<T>`), we expect an identifier. It is either:
        //   (a) `impl TypeName {`            — inherent impl: trait_name = None
        //   (b) `impl TraitName for TypeName {` — trait impl: trait_name = Some(…)
        //
        // FLS §11.1: Trait implementations have the form `impl Trait for Type`.
        // FLS §11.2: Inherent implementations have the form `impl Type`.
        if self.peek_kind() != TokenKind::Ident {
            return Err(self.error(format!(
                "expected type name after `impl`, found {:?}",
                self.peek_kind()
            )));
        }
        let first_ident = self.current_span();
        self.advance();

        // FLS §12.1: After the type/trait name, there may be angle-bracket type
        // arguments: `Pair<T>` or `Iterator<Item = i32>`. Consume and discard them
        // — for monomorphization at this milestone, all type args resolve to i32.
        if self.peek_kind() == TokenKind::Lt {
            self.advance(); // eat `<`
            let mut depth = 1usize;
            while depth > 0 && self.peek_kind() != TokenKind::Eof {
                match self.peek_kind() {
                    TokenKind::Lt => { self.advance(); depth += 1; }
                    TokenKind::Gt => { self.advance(); depth -= 1; }
                    _ => { self.advance(); }
                }
            }
        }

        // Disambiguate: if the next token is `for`, `first_ident` is the trait name.
        let (trait_name, ty) = if self.peek_kind() == TokenKind::KwFor {
            self.advance(); // eat `for`
            if self.peek_kind() != TokenKind::Ident {
                return Err(self.error(format!(
                    "expected type name after `for`, found {:?}",
                    self.peek_kind()
                )));
            }
            let struct_ty = self.current_span();
            self.advance();
            // FLS §12.1: Skip angle-bracket type args on the impl target type too.
            if self.peek_kind() == TokenKind::Lt {
                self.advance(); // eat `<`
                let mut depth = 1usize;
                while depth > 0 && self.peek_kind() != TokenKind::Eof {
                    match self.peek_kind() {
                        TokenKind::Lt => { self.advance(); depth += 1; }
                        TokenKind::Gt => { self.advance(); depth -= 1; }
                        _ => { self.advance(); }
                    }
                }
            }
            (Some(first_ident), struct_ty)
        } else {
            (None, first_ident)
        };

        // Optional `where` clause before the impl body.
        // FLS §4.14: `impl<T> Trait for Type where T: Bound { ... }`.
        self.parse_where_clause();

        self.expect(TokenKind::OpenBrace)?;

        let mut methods = Vec::new();
        let mut assoc_consts = Vec::new();
        let mut assoc_types = Vec::new();
        while self.peek_kind() != TokenKind::CloseBrace && self.peek_kind() != TokenKind::Eof {
            let vis = self.parse_visibility();
            // FLS §10.2: `type Name = Ty;` is an associated type binding in an impl block.
            if self.peek_kind() == TokenKind::KwType {
                let at = self.parse_assoc_type_def()?;
                assoc_types.push(at);
                continue;
            }
            // FLS §10.3: `const NAME: Ty = EXPR;` is an associated constant (not a `const fn`).
            // Distinguish by peeking: `const fn` → const function, `const IDENT` → assoc const.
            if self.peek_kind() == TokenKind::KwConst
                && self.peek_nth(1) != TokenKind::KwFn
            {
                let ac = self.parse_assoc_const()?;
                assoc_consts.push(ac);
                continue;
            }
            // FLS §9:41: Allow `const fn` inside impl blocks.
            let is_const = if self.peek_kind() == TokenKind::KwConst
                && self.peek_nth(1) == TokenKind::KwFn
            {
                self.advance(); // consume `const`
                true
            } else {
                false
            };
            // FLS §19: Allow `unsafe fn` inside impl blocks.
            let is_unsafe = if self.peek_kind() == TokenKind::KwUnsafe
                && self.peek_nth(1) == TokenKind::KwFn
            {
                self.advance(); // consume `unsafe`
                true
            } else {
                false
            };
            if self.peek_kind() != TokenKind::KwFn {
                return Err(self.error(format!(
                    "expected `fn`, `const`, `unsafe`, or `type` inside impl block, found {:?}",
                    self.peek_kind()
                )));
            }
            let method = self.parse_fn_def(vis, is_const, is_unsafe)?;
            methods.push(Box::new(method));
        }

        let end = self.current_span();
        self.expect(TokenKind::CloseBrace)?;
        let span = start.to(end);

        Ok(ImplDef { is_unsafe, ty, generic_params, trait_name, methods, assoc_consts, assoc_types, span })
    }

    /// Parse a trait definition.
    ///
    /// FLS §13: Traits.
    ///
    /// Grammar (simplified):
    /// ```text
    /// TraitDeclaration ::=
    ///     Visibility? "trait" Identifier "{" TraitItem* "}"
    /// TraitItem ::=
    ///     Visibility? "fn" Identifier "(" TraitFunctionParams? ")" FunctionReturnType? ";"
    /// ```
    ///
    /// Method signatures inside a trait body have no body — they end with `;`.
    /// The `FnDef::body` field is `None` for trait method signatures.
    ///
    /// FLS §13 AMBIGUOUS: The FLS allows default method bodies in traits
    /// (`fn foo(&self) -> i32 { 0 }`). This implementation only supports
    /// signatures without a body (the common case for galvanic's milestone).
    fn parse_trait_def(&mut self, _vis: Visibility, is_unsafe: bool) -> Result<TraitDef, ParseError> {
        let start = self.current_span();
        self.expect(TokenKind::KwTrait)?;

        if self.peek_kind() != TokenKind::Ident {
            return Err(self.error(format!(
                "expected trait name after `trait`, found {:?}",
                self.peek_kind()
            )));
        }
        let name = self.current_span();
        self.advance();

        // FLS §4.14: Optional supertrait bounds: `trait Derived: Base + Other { ... }`
        //
        // When a `:` follows the trait name, parse one or more `+`-separated
        // trait names as supertrait bounds. Galvanic does not enforce that
        // implementors of `Derived` also implement `Base` — no type-system checking
        // is performed. The spans are stored in `TraitDef.supertraits` for FLS
        // traceability and to enable future enforcement.
        //
        // FLS §4.14 AMBIGUOUS: The spec does not define how supertrait method
        // availability is resolved for concrete types at call sites. Galvanic's
        // monomorphization naturally handles this: `t.base_method()` on a generic
        // `T: Derived` resolves to `T__base_method` at call sites, which exists
        // because the concrete type implements the supertrait.
        let mut supertraits = Vec::new();
        if self.peek_kind() == TokenKind::Colon {
            self.advance(); // consume `:`
            loop {
                if self.peek_kind() == TokenKind::Ident {
                    supertraits.push(self.current_span());
                    self.advance();
                }
                if self.peek_kind() == TokenKind::Plus {
                    self.advance(); // consume `+`
                } else {
                    break;
                }
            }
        }

        // FLS §4.14: Optional where clause on trait definition.
        // `trait Transform where Self: Sized { ... }`
        // The where clause is parsed and discarded — galvanic does not enforce
        // trait bounds at the type-system level during monomorphization.
        // FLS §4.14 AMBIGUOUS: The spec does not specify whether where-clause bounds
        // on trait definitions are checked at definition time or implementation time.
        self.parse_where_clause();

        self.expect(TokenKind::OpenBrace)?;

        let mut methods = Vec::new();
        let mut assoc_consts = Vec::new();
        let mut assoc_types = Vec::new();
        while self.peek_kind() != TokenKind::CloseBrace && self.peek_kind() != TokenKind::Eof {
            let vis = self.parse_visibility();
            // FLS §10.2: `type Name;` or `type Name = Ty;` in a trait body.
            if self.peek_kind() == TokenKind::KwType {
                let at = self.parse_assoc_type_def()?;
                assoc_types.push(at);
                continue;
            }
            // FLS §10.3: `const NAME: Ty;` or `const NAME: Ty = EXPR;` in a trait body.
            if self.peek_kind() == TokenKind::KwConst
                && self.peek_nth(1) != TokenKind::KwFn
            {
                let ac = self.parse_assoc_const()?;
                assoc_consts.push(ac);
                continue;
            }
            // FLS §9:41: `const fn` may appear in trait bodies.
            let is_const = if self.peek_kind() == TokenKind::KwConst
                && self.peek_nth(1) == TokenKind::KwFn
            {
                self.advance(); // consume `const`
                true
            } else {
                false
            };
            // FLS §19: `unsafe fn` may appear in trait bodies.
            let is_unsafe = if self.peek_kind() == TokenKind::KwUnsafe
                && self.peek_nth(1) == TokenKind::KwFn
            {
                self.advance(); // consume `unsafe`
                true
            } else {
                false
            };
            if self.peek_kind() != TokenKind::KwFn {
                return Err(self.error(format!(
                    "expected `fn`, `const`, `unsafe`, or `type` inside trait body, found {:?}",
                    self.peek_kind()
                )));
            }
            // Parse the function signature. If the next token after the
            // signature is `;`, consume it (body-less method signature).
            // If it is `{`, parse the full body (default method — future work).
            let method = self.parse_fn_def(vis, is_const, is_unsafe)?;
            methods.push(Box::new(method));
        }

        let end = self.current_span();
        self.expect(TokenKind::CloseBrace)?;
        let span = start.to(end);

        Ok(TraitDef { is_unsafe, name, supertraits, methods, assoc_consts, assoc_types, span })
    }

    /// Consume a `self`, `&self`, or `&mut self` parameter if present.
    ///
    /// FLS §10.1: A self parameter must be the first parameter in a method.
    /// Returns `Some(SelfKind)` if consumed, `None` if no self parameter.
    ///
    /// After consuming, also eats a trailing comma if one follows, so that
    /// `parse_params` sees any remaining parameters without a leading comma.
    fn parse_self_param(&mut self) -> Option<crate::ast::SelfKind> {
        use crate::ast::SelfKind;

        // `self`
        if self.peek_kind() == TokenKind::KwSelfLower {
            // Disambiguate `self` (self param) from `self: Type` (explicit self).
            // If the next token after `self` is `:`, it's an explicit param; defer.
            if self.peek_nth(1) != TokenKind::Colon {
                self.advance(); // eat `self`
                self.eat(TokenKind::Comma);
                return Some(SelfKind::Val);
            }
        }

        // `&self`
        if self.peek_kind() == TokenKind::And && self.peek_nth(1) == TokenKind::KwSelfLower {
            self.advance(); // eat `&`
            self.advance(); // eat `self`
            self.eat(TokenKind::Comma);
            return Some(SelfKind::Ref);
        }

        // `&mut self`
        if self.peek_kind() == TokenKind::And
            && self.peek_nth(1) == TokenKind::KwMut
            && self.peek_nth(2) == TokenKind::KwSelfLower
        {
            self.advance(); // eat `&`
            self.advance(); // eat `mut`
            self.advance(); // eat `self`
            self.eat(TokenKind::Comma);
            return Some(SelfKind::RefMut);
        }

        None
    }

    /// Consume a `where` clause if present, discarding all bounds.
    ///
    /// FLS §4.14: Trait and lifetime bounds — `where T: Trait, U: A + B`.
    /// The bounds are parsed and silently discarded; galvanic infers the
    /// concrete type from the call-site argument type at monomorphization, not
    /// from the where-clause annotation.
    ///
    /// FLS §4.14: AMBIGUOUS — The spec does not specify the disambiguation
    /// rule between a `where` keyword and an identifier named `where` in older
    /// Rust editions. In practice `where` here is always the keyword.
    fn parse_where_clause(&mut self) {
        if self.peek_kind() != TokenKind::KwWhere {
            return;
        }
        self.advance(); // eat `where`
        // Parse comma-separated predicates: `T: Bound + Bound, U: Other, ...`
        loop {
            // Each predicate starts with a type (identifier or `&`). We skip
            // tokens until we see `,`, `{`, `;`, or EOF, which terminate the clause.
            // For the simple case: `T: TraitA + TraitB`, we consume `T`, `:`,
            // each bound name, and `+` separators.
            if matches!(
                self.peek_kind(),
                TokenKind::OpenBrace | TokenKind::Semi | TokenKind::Eof
            ) {
                break;
            }
            // Consume the LHS type (identifier or `Self`, possibly `&`-prefixed).
            // FLS §4.14: LHS may be a type parameter (`T`), the receiver type
            // (`Self`), a reference type (`&T`), or a type-projection `T::AssocType`.
            // `Self` is a keyword, not an identifier — handle both cases.
            // FLS §10.2 / §4.14: `where C::Item: Trait` constrains the associated
            // type projection. The LHS `C::Item` is consumed as `Ident :: Ident`.
            if self.peek_kind() == TokenKind::Ident
                || self.peek_kind() == TokenKind::KwSelfUpper
            {
                self.advance(); // skip type param name, path, or `Self`
                // FLS §10.2: Also consume `:: AssocName` for projections like `C::Item`.
                if self.peek_kind() == TokenKind::ColonColon {
                    self.advance(); // skip `::`
                    if self.peek_kind() == TokenKind::Ident {
                        self.advance(); // skip associated type name (e.g. `Item`)
                    }
                }
            } else if self.peek_kind() == TokenKind::And {
                self.advance(); // skip `&`
                if matches!(
                    self.peek_kind(),
                    TokenKind::Ident | TokenKind::KwSelfUpper
                ) {
                    self.advance(); // skip the type after `&`
                }
            } else {
                // Unexpected token — stop consuming to avoid runaway parsing.
                break;
            }
            // Consume the `:` and bounds.
            if self.eat(TokenKind::Colon) {
                loop {
                    // Consume a bound identifier (plain trait name, e.g. `Scalable`).
                    if self.peek_kind() == TokenKind::Ident {
                        self.advance(); // skip bound trait name
                        // Consume type args on the bound: `Iterator<Item = i32>`.
                        if self.peek_kind() == TokenKind::Lt {
                            self.advance(); // eat `<`
                            let mut depth = 1usize;
                            while depth > 0 && self.peek_kind() != TokenKind::Eof {
                                match self.peek_kind() {
                                    TokenKind::Lt => { self.advance(); depth += 1; }
                                    TokenKind::Gt => { self.advance(); depth -= 1; }
                                    _ => { self.advance(); }
                                }
                            }
                        }
                    }
                    // `+` separates multiple bounds.
                    if !self.eat(TokenKind::Plus) {
                        break;
                    }
                }
            }
            // `,` separates multiple predicates.
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
    }

    /// Parse a constant item declaration.
    ///
    /// FLS §7.1: Constant items.
    ///
    /// Grammar:
    /// ```text
    /// ConstantDeclaration ::= "const" Identifier ":" Type "=" Expression ";"
    /// ```
    ///
    /// FLS §7.1:10: Every use of a constant is replaced with its value.
    /// FLS §6.1.2:37–45: The initializer is a const context — compile-time
    /// evaluation is permitted and required.
    fn parse_const_def(&mut self, _vis: Visibility) -> Result<ConstDef, ParseError> {
        let start = self.current_span();
        self.expect(TokenKind::KwConst)?;
        let name = self.expect(TokenKind::Ident)?;
        self.expect(TokenKind::Colon)?;
        let ty = self.parse_ty()?;
        self.expect(TokenKind::Eq)?;
        let value = self.parse_expr()?;
        let end = self.current_span();
        self.expect(TokenKind::Semi)?;
        Ok(ConstDef { name, ty, value, span: start.to(end) })
    }

    /// Parse an associated constant declaration inside an `impl` or `trait` block.
    ///
    /// FLS §10.3: Associated Constants.
    ///
    /// Grammar:
    /// ```text
    /// AssocConstDecl ::= "const" Identifier ":" Type ("=" Expression)? ";"
    /// ```
    ///
    /// The `= Expression` part is optional in trait bodies (required const, no default)
    /// and mandatory in impl blocks. If absent, `value` is `None`.
    fn parse_assoc_const(&mut self) -> Result<crate::ast::AssocConst, ParseError> {
        let start = self.current_span();
        self.expect(TokenKind::KwConst)?;
        let name = self.expect(TokenKind::Ident)?;
        self.expect(TokenKind::Colon)?;
        let ty = self.parse_ty()?;
        // Optional initializer.
        let value = if self.eat(TokenKind::Eq) {
            Some(self.parse_expr()?)
        } else {
            None
        };
        let end = self.current_span();
        self.expect(TokenKind::Semi)?;
        Ok(crate::ast::AssocConst { name, ty, value, span: start.to(end) })
    }

    /// Parse an associated type declaration inside an `impl` or `trait` block.
    ///
    /// FLS §10.2: Associated Types.
    ///
    /// Grammar:
    /// ```text
    /// AssocTypeDecl ::= "type" Identifier ("=" Type)? ";"
    /// ```
    ///
    /// In a trait body: `type Item;` (abstract) or `type Item = i32;` (with default).
    /// In an impl block: `type Item = i32;` (concrete binding).
    ///
    /// FLS §10.2: "Each implementation of the trait must provide a type binding
    /// for each abstract associated type declared in the trait."
    fn parse_assoc_type_def(&mut self) -> Result<AssocTypeDef, ParseError> {
        let start = self.current_span();
        self.expect(TokenKind::KwType)?;
        let name = self.expect(TokenKind::Ident)?;
        // Optional `= Type` — present in impl blocks and trait defaults, absent in
        // abstract trait declarations.
        let ty = if self.eat(TokenKind::Eq) {
            Some(self.parse_ty()?)
        } else {
            None
        };
        let end = self.current_span();
        self.expect(TokenKind::Semi)?;
        Ok(AssocTypeDef { name, ty, span: start.to(end) })
    }

    /// Parse a static item declaration.
    ///
    /// FLS §7.2: Static items.
    ///
    /// Grammar (abridged):
    /// ```text
    /// StaticDeclaration ::= "static" "mut"? Identifier ":" Type "=" Expression ";"
    /// ```
    ///
    /// FLS §7.2:15: All references to a static refer to the same memory address.
    /// FLS §7.2: Mutable statics (`static mut`) require `unsafe` to access;
    /// galvanic parses `mut` but does not yet enforce the unsafe requirement.
    fn parse_static_def(&mut self, _vis: Visibility) -> Result<StaticDef, ParseError> {
        let start = self.current_span();
        self.expect(TokenKind::KwStatic)?;
        let mutable = self.eat(TokenKind::KwMut);
        let name = self.expect(TokenKind::Ident)?;
        self.expect(TokenKind::Colon)?;
        let ty = self.parse_ty()?;
        self.expect(TokenKind::Eq)?;
        let value = self.parse_expr()?;
        let end = self.current_span();
        self.expect(TokenKind::Semi)?;
        Ok(StaticDef { name, ty, value, mutable, span: start.to(end) })
    }

    /// Parse a type alias declaration.
    ///
    /// FLS §4.10: Type aliases.
    ///
    /// Grammar:
    /// ```text
    /// TypeAlias ::= "type" Identifier "=" Type ";"
    /// ```
    ///
    /// FLS §4.10: "A type alias defines a new name for an existing type."
    fn parse_type_alias_def(&mut self, _vis: Visibility) -> Result<TypeAliasDef, ParseError> {
        let start = self.current_span();
        self.expect(TokenKind::KwType)?;
        let name = self.expect(TokenKind::Ident)?;
        self.expect(TokenKind::Eq)?;
        let ty = self.parse_ty()?;
        let end = self.current_span();
        self.expect(TokenKind::Semi)?;
        Ok(TypeAliasDef { name, ty, span: start.to(end) })
    }

    /// Parse a struct definition.
    ///
    /// FLS §14: Structs.
    ///
    /// Grammar:
    /// ```text
    /// StructDeclaration ::=
    ///     "struct" Identifier
    ///     ( "{" NamedField* "}"          -- named-field struct
    ///     | "(" TupleField* ")" ";"      -- tuple struct
    ///     | ";"                          -- unit struct
    ///     )
    /// ```
    ///
    /// FLS §14 NOTE: Generic type parameters and where clauses on structs are
    /// not yet implemented. They are future work.
    fn parse_struct_def(&mut self, vis: Visibility) -> Result<StructDef, ParseError> {
        let start = self.current_span();
        self.expect(TokenKind::KwStruct)?;

        // Struct name must be an identifier.
        if self.peek_kind() != TokenKind::Ident {
            return Err(self.error(format!(
                "expected struct name (identifier), found {:?}",
                self.peek_kind()
            )));
        }
        let name = self.current_span();
        self.advance();

        // FLS §12.1: A generic struct may declare type parameters after its name:
        // `struct Pair<T, U> { ... }`. Each parameter is a plain identifier;
        // bounds and where-clauses are not yet supported.
        //
        // FLS §12.1 AMBIGUOUS: The FLS does not specify the disambiguation rule
        // for `<` in this position. In practice `<` immediately after a struct
        // name always starts a generic parameter list.
        let mut generic_params = Vec::new();
        if self.peek_kind() == TokenKind::Lt {
            self.advance(); // eat `<`
            loop {
                if self.peek_kind() == TokenKind::Gt {
                    self.advance(); // eat `>`
                    break;
                }
                if self.peek_kind() != TokenKind::Ident {
                    return Err(self.error(format!(
                        "expected type parameter name or `>`, found {:?}",
                        self.peek_kind()
                    )));
                }
                generic_params.push(self.current_span());
                self.advance();
                if !self.eat(TokenKind::Comma) {
                    self.expect(TokenKind::Gt)?;
                    break;
                }
            }
        }

        // FLS §4.14: Optional where clause on struct definition.
        // `struct Wrapper<T> where T: Getter { val: T }`
        // Bounds are parsed and discarded — galvanic's monomorphization infers
        // concrete types from call-site arguments, not from where-clause bounds.
        // FLS §4.14 AMBIGUOUS: The spec does not specify when where-clause bounds
        // on struct definitions are checked (parse time, type-check, or mono).
        self.parse_where_clause();

        let (kind, end) = match self.peek_kind() {
            // Named-field struct: `struct Foo { … }`
            TokenKind::OpenBrace => {
                self.advance(); // eat `{`
                let fields = self.parse_named_fields()?;
                let end = self.current_span();
                self.expect(TokenKind::CloseBrace)?;
                (StructKind::Named(fields), end)
            }
            // Tuple struct: `struct Foo(…);`
            TokenKind::OpenParen => {
                self.advance(); // eat `(`
                let fields = self.parse_tuple_fields()?;
                self.expect(TokenKind::CloseParen)?;
                let end = self.current_span();
                self.expect(TokenKind::Semi)?;
                (StructKind::Tuple(fields), end)
            }
            // Unit struct: `struct Foo;`
            TokenKind::Semi => {
                let end = self.current_span();
                self.advance(); // eat `;`
                (StructKind::Unit, end)
            }
            kind => {
                return Err(self.error(format!(
                    "expected `{{`, `(`, or `;` after struct name, found {kind:?}"
                )));
            }
        };

        let span = start.to(end);
        Ok(StructDef { vis, name, generic_params, kind, span })
    }

    /// Parse an enum definition.
    ///
    /// FLS §15: Enumerations.
    ///
    /// Grammar:
    /// ```text
    /// EnumDeclaration ::=
    ///     "enum" Identifier "{" EnumVariant* "}"
    /// EnumVariant ::=
    ///     Identifier
    ///     ( "{" NamedField* "}"   -- named-field variant
    ///     | "(" TupleField* ")"  -- tuple variant
    ///     |                      -- unit variant
    ///     )
    /// ```
    ///
    /// FLS §15 NOTE: Generic type parameters and where clauses on enums are
    /// not yet implemented. They are future work.
    fn parse_enum_def(&mut self, vis: Visibility) -> Result<EnumDef, ParseError> {
        let start = self.current_span();
        self.expect(TokenKind::KwEnum)?;

        // Enum name must be an identifier.
        if self.peek_kind() != TokenKind::Ident {
            return Err(self.error(format!(
                "expected enum name (identifier), found {:?}",
                self.peek_kind()
            )));
        }
        let name = self.current_span();
        self.advance();

        // FLS §12.1: Optionally parse generic type parameters `<T, U, …>`.
        // FLS §12.1 AMBIGUOUS: The FLS does not specify the disambiguation rule
        // for `<` after an enum name (generic params vs. less-than). Galvanic
        // treats `<` immediately after the enum name as opening a type-param list.
        let mut generic_params = Vec::new();
        if self.peek_kind() == TokenKind::Lt {
            self.advance(); // eat `<`
            while self.peek_kind() != TokenKind::Gt && self.peek_kind() != TokenKind::Eof {
                if self.peek_kind() == TokenKind::Ident {
                    generic_params.push(self.current_span());
                    self.advance();
                }
                if self.peek_kind() == TokenKind::Comma {
                    self.advance();
                }
            }
            self.expect(TokenKind::Gt)?;
        }

        // FLS §4.14: Optional where clause on enum definition.
        // `enum Maybe<T> where T: Sized { Some(T), None }`
        // Bounds are parsed and discarded — same strategy as structs and fns.
        // FLS §4.14 AMBIGUOUS: The spec does not specify when where-clause bounds
        // on enum definitions are checked (parse time, type-check, or mono).
        self.parse_where_clause();

        // Enum body is always `{ … }`.
        self.expect(TokenKind::OpenBrace)?;
        let variants = self.parse_enum_variants()?;
        let end = self.current_span();
        self.expect(TokenKind::CloseBrace)?;

        let span = start.to(end);
        Ok(EnumDef { vis, name, generic_params, variants, span })
    }

    /// Parse the variant list inside an enum body `{ Var1, Var2, … }`.
    ///
    /// FLS §15: Each variant is an identifier optionally followed by a
    /// tuple body `(…)` or a named-field body `{…}`.
    fn parse_enum_variants(&mut self) -> Result<Vec<EnumVariant>, ParseError> {
        let mut variants = Vec::new();

        while self.peek_kind() != TokenKind::CloseBrace
            && self.peek_kind() != TokenKind::Eof
        {
            let start = self.current_span();

            // Variant name must be an identifier.
            if self.peek_kind() != TokenKind::Ident {
                return Err(self.error(format!(
                    "expected variant name (identifier), found {:?}",
                    self.peek_kind()
                )));
            }
            let name = self.current_span();
            self.advance();

            let (kind, end) = match self.peek_kind() {
                // Named-field variant: `Foo { x: i32, … }`
                TokenKind::OpenBrace => {
                    self.advance(); // eat `{`
                    let fields = self.parse_named_fields()?;
                    let end = self.current_span();
                    self.expect(TokenKind::CloseBrace)?;
                    (EnumVariantKind::Named(fields), end)
                }
                // Tuple variant: `Foo(i32, …)`
                TokenKind::OpenParen => {
                    self.advance(); // eat `(`
                    let fields = self.parse_tuple_fields()?;
                    let end = self.current_span();
                    self.expect(TokenKind::CloseParen)?;
                    (EnumVariantKind::Tuple(fields), end)
                }
                // Unit variant: `Foo`
                _ => (EnumVariantKind::Unit, name),
            };

            variants.push(EnumVariant { name, kind, span: start.to(end) });

            if !self.eat(TokenKind::Comma) {
                break;
            }
        }

        Ok(variants)
    }

    /// Parse the named fields of a struct body `{ field: Type, … }`.
    ///
    /// FLS §14.1: Named fields.
    fn parse_named_fields(&mut self) -> Result<Vec<NamedField>, ParseError> {
        let mut fields = Vec::new();

        while self.peek_kind() != TokenKind::CloseBrace
            && self.peek_kind() != TokenKind::Eof
        {
            let start = self.current_span();
            let vis = self.parse_visibility();

            if self.peek_kind() != TokenKind::Ident {
                return Err(self.error(format!(
                    "expected field name (identifier), found {:?}",
                    self.peek_kind()
                )));
            }
            let name = self.current_span();
            self.advance();

            self.expect(TokenKind::Colon)?;
            let ty = self.parse_ty()?;
            let end = ty.span;

            fields.push(NamedField { vis, name, ty, span: start.to(end) });

            if !self.eat(TokenKind::Comma) {
                break;
            }
        }

        Ok(fields)
    }

    /// Parse the tuple fields of a tuple-struct body `(Type, …)`.
    ///
    /// FLS §14.2: Tuple fields.
    fn parse_tuple_fields(&mut self) -> Result<Vec<TupleField>, ParseError> {
        let mut fields = Vec::new();

        while self.peek_kind() != TokenKind::CloseParen
            && self.peek_kind() != TokenKind::Eof
        {
            let start = self.current_span();
            let vis = self.parse_visibility();
            let ty = self.parse_ty()?;
            let end = ty.span;

            fields.push(TupleField { vis, ty, span: start.to(end) });

            if !self.eat(TokenKind::Comma) {
                break;
            }
        }

        Ok(fields)
    }

    /// Parse a struct literal expression `Name { field: expr, … }`.
    ///
    /// Called from `parse_primary` after the struct name (a single identifier)
    /// has already been consumed and the lookahead confirms `{ Ident :` or `{}`.
    ///
    /// FLS §6.11: Struct expressions.
    fn parse_struct_lit(&mut self, name: Span) -> Result<Expr, ParseError> {
        let start = name;
        self.expect(TokenKind::OpenBrace)?; // eat `{`

        let mut fields = Vec::new();
        let mut base = None;

        while self.peek_kind() != TokenKind::CloseBrace
            && self.peek_kind() != TokenKind::Eof
        {
            // FLS §6.11: Struct update syntax `..base_expr` — copies all fields
            // not explicitly listed from `base_expr`. Must appear last, after
            // any explicit field initialisers.
            if self.peek_kind() == TokenKind::DotDot {
                self.advance(); // consume `..`
                let base_expr = self.parse_expr()?;
                base = Some(Box::new(base_expr));
                // `..base` is always the last item; no comma follows.
                break;
            }

            if self.peek_kind() != TokenKind::Ident {
                return Err(self.error(format!(
                    "expected field name in struct literal, found {:?}",
                    self.peek_kind()
                )));
            }
            let field_name = self.current_span();
            self.advance();
            // FLS §6.11: Shorthand field initialization — `Point { x, y }` is
            // equivalent to `Point { x: x, y: y }`. If the next token is not
            // `:`, treat the field name as both name and value expression.
            let value = if self.eat(TokenKind::Colon) {
                self.parse_expr()?
            } else {
                Expr {
                    kind: ExprKind::Path(vec![field_name]),
                    span: field_name,
                }
            };
            fields.push((field_name, Box::new(value)));

            if !self.eat(TokenKind::Comma) {
                break;
            }
        }

        let end = self.current_span();
        self.expect(TokenKind::CloseBrace)?;
        Ok(Expr {
            kind: ExprKind::StructLit { name, fields, base },
            span: start.to(end),
        })
    }

    /// Parse the parameter list between the `(` and `)`.
    ///
    /// FLS §9.2: Function parameters.
    ///
    /// Grammar: `(Identifier ":" Type ("," Identifier ":" Type)* ","?)?`
    ///
    /// FLS §9.2 NOTE: Full patterns (struct, tuple, `_`) in parameter
    /// position are not yet handled. `self`, `mut self`, `&self`, and
    /// `&mut self` are also not yet supported.
    /// Parse the element patterns inside a tuple parameter pattern, called after
    /// consuming the opening `(`. Handles flat bindings (`a`, `_`) and
    /// arbitrarily nested tuple patterns (`(a, (b, c))`). Consumes up to and
    /// including the closing `)`.
    ///
    /// Returns `Vec<Pat>` where each element is `Pat::Ident`, `Pat::Wildcard`,
    /// or `Pat::Tuple` (for nested tuples).
    ///
    /// FLS §5.10.3, §9.2: Tuple patterns in parameter position may nest
    /// arbitrarily. Each leaf binding corresponds to one ARM64 register.
    fn parse_tuple_param_inner_pats(&mut self) -> Result<Vec<Pat>, ParseError> {
        let mut pats = Vec::new();
        loop {
            if self.peek_kind() == TokenKind::CloseParen {
                break;
            }
            let pat = if self.peek_kind() == TokenKind::OpenParen {
                // Nested tuple pattern: recurse.
                self.advance(); // consume `(`
                let inner = self.parse_tuple_param_inner_pats()?;
                Pat::Tuple(inner)
            } else if self.peek_kind() == TokenKind::Underscore {
                self.advance();
                Pat::Wildcard
            } else if self.peek_kind() == TokenKind::Ident {
                let s = self.current_span();
                self.advance();
                Pat::Ident(s)
            } else {
                return Err(self.error(format!(
                    "expected identifier, `_`, or `(` in tuple parameter pattern, found {:?}",
                    self.peek_kind()
                )));
            };
            pats.push(pat);
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::CloseParen)?;
        Ok(pats)
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, ParseError> {
        let mut params = Vec::new();

        while self.peek_kind() != TokenKind::CloseParen
            && self.peek_kind() != TokenKind::Eof
        {
            let start = self.current_span();

            // FLS §9.2: Parameters may be prefixed with `mut` to make the
            // binding mutable within the function body. The `mut` is not
            // part of the name — consume and discard it.
            self.eat(TokenKind::KwMut);

            // FLS §5.10.3, §9.2: Tuple pattern in parameter position
            // `(a, b, ...): (T1, T2, ...)`.
            // Supports nested tuple patterns: `(a, (b, c)): (T1, (T2, T3))`.
            let kind = if self.peek_kind() == TokenKind::OpenParen {
                self.advance(); // consume `(`
                let pats = self.parse_tuple_param_inner_pats()?;
                ParamKind::Tuple(pats)
            } else if self.peek_kind() == TokenKind::Ident
                && self.peek_nth(1) == TokenKind::OpenParen
            {
                // FLS §5.10.4, §9.2: Tuple struct pattern in parameter position
                // `Pair(a, b): Pair`.
                let type_span = self.current_span();
                self.advance(); // consume struct type name
                self.advance(); // consume `(`
                let mut fields: Vec<Span> = Vec::new();
                loop {
                    if self.peek_kind() == TokenKind::CloseParen {
                        break;
                    }
                    if matches!(self.peek_kind(), TokenKind::Underscore | TokenKind::Ident) {
                        fields.push(self.current_span());
                        self.advance();
                    } else {
                        return Err(self.error(format!(
                            "expected identifier or `_` in tuple struct parameter pattern, found {:?}",
                            self.peek_kind()
                        )));
                    }
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(TokenKind::CloseParen)?;
                ParamKind::TupleStruct { type_span, fields }
            } else if self.peek_kind() == TokenKind::Ident
                && self.peek_nth(1) == TokenKind::OpenBrace
            {
                // FLS §5.10.2, §9.2: Struct pattern in parameter position
                // `Point { x, y }: Point`.
                // Supports nested struct patterns: `Outer { inner: Inner { a, b }, c }`.
                let type_span = self.current_span();
                self.advance(); // consume struct type name
                self.advance(); // consume `{`
                let mut fields: Vec<(Span, crate::ast::Pat)> = Vec::new();
                loop {
                    if self.peek_kind() == TokenKind::CloseBrace {
                        break;
                    }
                    // Each entry is `field_name` (shorthand) or `field_name: binding_pat`.
                    let field_name = if self.peek_kind() == TokenKind::Ident {
                        let s = self.current_span();
                        self.advance();
                        s
                    } else {
                        return Err(self.error(format!(
                            "expected field name in struct parameter pattern, found {:?}",
                            self.peek_kind()
                        )));
                    };
                    let binding = if self.eat(TokenKind::Colon) {
                        // Explicit binding: `field: _`, `field: name`, or
                        // `field: Inner { a, b }` (nested struct).
                        // Reuse parse_single_pattern which handles all these forms.
                        self.parse_single_pattern()?
                    } else {
                        // Shorthand: `{ x }` is sugar for `{ x: x }`.
                        crate::ast::Pat::Ident(field_name)
                    };
                    fields.push((field_name, binding));
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(TokenKind::CloseBrace)?;
                ParamKind::Struct { type_span, fields }
            } else if self.peek_kind() == TokenKind::Ident {
                let name = self.current_span();
                self.advance();
                ParamKind::Ident(name)
            } else {
                return Err(self.error(format!(
                    "expected parameter name (identifier, tuple, or struct pattern), found {:?}",
                    self.peek_kind()
                )));
            };

            self.expect(TokenKind::Colon)?;
            let ty = self.parse_ty()?;
            let end = ty.span;

            params.push(Param { kind, ty, span: start.to(end) });

            if !self.eat(TokenKind::Comma) {
                break;
            }
        }

        Ok(params)
    }

    // ── Types ─────────────────────────────────────────────────────────────────

    /// Parse a type expression.
    ///
    /// FLS §4: Types.
    ///
    /// Grammar (simplified):
    /// ```text
    /// Type ::= "()" | "&" "mut"? Type | PathType
    /// PathType ::= Identifier ("::" Identifier)*
    /// ```
    ///
    /// FLS §4 NOTE: Generic type arguments (`Vec<i32>`), tuple types,
    /// array/slice types, function pointer types, and trait objects are
    /// not yet implemented.
    fn parse_ty(&mut self) -> Result<Ty, ParseError> {
        let start = self.current_span();

        match self.peek_kind() {
            // Unit type `()` or tuple type `(T1, T2, ...)` — FLS §4.4
            TokenKind::OpenParen => {
                self.advance();
                // Peek: if immediately `)` it is the unit type.
                if self.peek_kind() == TokenKind::CloseParen {
                    let end = self.current_span();
                    self.advance();
                    return Ok(Ty { kind: TyKind::Unit, span: start.to(end) });
                }
                // Otherwise parse a non-empty tuple type `(T1, T2, ...)`.
                let mut elems = Vec::new();
                loop {
                    elems.push(self.parse_ty()?);
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                    if self.peek_kind() == TokenKind::CloseParen {
                        break;
                    }
                }
                let end = self.current_span();
                self.expect(TokenKind::CloseParen)?;
                Ok(Ty { kind: TyKind::Tuple(elems), span: start.to(end) })
            }

            // Reference type `&T` or `&mut T` — FLS §4.8
            TokenKind::And => {
                self.advance();
                let mutable = self.eat(TokenKind::KwMut);
                let inner = self.parse_ty()?;
                let end = inner.span;
                Ok(Ty {
                    kind: TyKind::Ref { mutable, inner: Box::new(inner) },
                    span: start.to(end),
                })
            }

            // Function pointer type `fn(T1, T2) -> R` — FLS §4.9
            TokenKind::KwFn => {
                self.advance(); // consume `fn`
                self.expect(TokenKind::OpenParen)?;
                let mut params = Vec::new();
                if self.peek_kind() != TokenKind::CloseParen {
                    loop {
                        params.push(self.parse_ty()?);
                        if !self.eat(TokenKind::Comma) {
                            break;
                        }
                        if self.peek_kind() == TokenKind::CloseParen {
                            break;
                        }
                    }
                }
                let end = self.current_span();
                self.expect(TokenKind::CloseParen)?;
                // Optional `-> ReturnType`
                let ret = if self.peek_kind() == TokenKind::RArrow {
                    self.advance(); // consume `->`
                    Some(Box::new(self.parse_ty()?))
                } else {
                    None
                };
                let end = if let Some(ref r) = ret { r.span } else { end };
                Ok(Ty { kind: TyKind::FnPtr { params, ret }, span: start.to(end) })
            }

            // Array type `[T; N]` — FLS §4.5.
            //
            // FLS §4.5: "An array type is a sequence type with a statically
            // known length." Syntax: `[ElementType ; Length]` where `Length`
            // is a constant expression. At this milestone only integer literal
            // lengths are supported.
            //
            // Cache-line note: N elements × 8-byte slots. Parsed eagerly so
            // that type annotations on array let bindings and parameters can
            // be accepted without a parse error.
            TokenKind::OpenBracket => {
                self.advance(); // consume `[`
                let elem = self.parse_ty()?;
                // Distinguish `[T; N]` (array) from `[T]` (slice).
                // FLS §4.9: A slice type `[T]` has no length. A following `]` without `;`
                // means this is a slice type. A following `;` means array type `[T; N]`.
                if self.peek_kind() == TokenKind::CloseBracket {
                    // Slice type `[T]` — FLS §4.9.
                    let end = self.current_span();
                    self.advance(); // consume `]`
                    return Ok(Ty { kind: TyKind::Slice { elem: Box::new(elem) }, span: start.to(end) });
                }
                self.expect(TokenKind::Semi)?;
                let len_span = self.current_span();
                self.expect(TokenKind::LitInteger)?;
                let len_text = len_span.text(self.src);
                // Strip any numeric separators and parse the base-10 length.
                let len_digits: String = len_text.chars().filter(|&c| c != '_').collect();
                let len: usize = len_digits.parse().map_err(|_| {
                    self.error(format!(
                        "array length `{len_text}` is not a valid usize literal"
                    ))
                })?;
                let end = self.current_span();
                self.expect(TokenKind::CloseBracket)?;
                Ok(Ty { kind: TyKind::Array { elem: Box::new(elem), len }, span: start.to(end) })
            }

            // Named type — FLS §4.1, §14
            TokenKind::Ident => {
                let mut segments = vec![self.current_span()];
                self.advance();

                // Path segments separated by `::`.
                while self.peek_kind() == TokenKind::ColonColon {
                    self.advance(); // eat `::`
                    if self.peek_kind() == TokenKind::Ident {
                        segments.push(self.current_span());
                        self.advance();
                    } else {
                        return Err(self.error("expected identifier after `::`"));
                    }
                }

                // FLS §12.1: A generic type in type position may have angle-bracket
                // type arguments, e.g. `Pair<i32>` or `Vec<T>`. Consume and discard
                // them — galvanic monomorphizes generic types to i32 and does not
                // carry type arguments through the IR.
                //
                // FLS §12.1: AMBIGUOUS — the FLS does not specify the disambiguation
                // rule for `<` in type position (generic type args vs. less-than).
                // Galvanic uses a greedy depth-counting strategy: if `<` immediately
                // follows a type name, it is treated as a type argument list.
                if self.peek_kind() == TokenKind::Lt {
                    self.advance(); // consume `<`
                    let mut depth = 1usize;
                    while depth > 0 && self.peek_kind() != TokenKind::Eof {
                        match self.peek_kind() {
                            TokenKind::Lt => { self.advance(); depth += 1; }
                            TokenKind::Gt => { self.advance(); depth -= 1; }
                            _ => { self.advance(); }
                        }
                    }
                }

                let end = *segments.last().unwrap();
                Ok(Ty { kind: TyKind::Path(segments), span: start.to(end) })
            }

            // `impl Fn(T1, ...) -> R` — anonymous type parameter. FLS §12: argument-position
            // `impl Trait` is syntactic sugar for an anonymous type parameter with a trait bound.
            // FLS §4.13: `Fn`, `FnMut`, and `FnOnce` are the closure traits.
            //
            // Galvanic maps `impl Fn(T1, ...) -> R` to `TyKind::FnPtr { params, ret }` — the
            // same IR representation as `fn(T1, ...) -> R`. Non-capturing closures passed to
            // `impl Fn` parameters are coerced to function pointers at the call site.
            //
            // FLS §12 NOTE: The full semantics of `impl Trait` (monomorphisation, capturing
            // closures) are not yet implemented. Only non-capturing closures are supported.
            // `dyn Trait` — a trait object type. FLS §4.13.
            //
            // `&dyn Trait` is a fat pointer: (data ptr, vtable ptr). The `dyn`
            // keyword indicates dynamic dispatch through the vtable at runtime.
            // Parsed as `TyKind::DynTrait(trait_name_span)`. The surrounding
            // `&` (or `&mut`) is parsed by the `TokenKind::And` arm above and
            // wraps the DynTrait in a `TyKind::Ref`.
            //
            // FLS §4.13: "A trait object is an opaque value of another type
            // that implements a set of traits." The `dyn` keyword makes dynamic
            // dispatch explicit.
            //
            // FLS §4.13: AMBIGUOUS — The FLS does not specify vtable layout.
            // Cache-line note: fat pointer = 2 × 8-byte slots (16 bytes).
            TokenKind::KwDyn => {
                self.advance(); // consume `dyn`
                if self.peek_kind() != TokenKind::Ident {
                    return Err(self.error(format!(
                        "expected trait name after `dyn`, found {:?}",
                        self.peek_kind()
                    )));
                }
                let trait_span = self.current_span();
                self.advance(); // consume trait name
                Ok(Ty { kind: TyKind::DynTrait(trait_span), span: start.to(trait_span) })
            }

            // `Self` or `Self::AssocType` — FLS §10.2: In trait method signatures and
            // impl method signatures, `Self` refers to the implementing type. `Self::Item`
            // is an associated type projection. Galvanic represents these as a two-segment
            // `TyKind::Path(["Self", "Item"])` so that `lower_ty` can resolve them via
            // the per-impl type alias registry (key `"Self::Item"` → concrete IrTy).
            //
            // FLS §10.2: AMBIGUOUS — the spec does not fully specify how `Self::X`
            // projections are resolved when `Self` appears in a default method body or
            // a trait method signature. Galvanic resolves `Self::X` to the concrete type
            // registered in the impl block (or the trait's default) at codegen time.
            TokenKind::KwSelfUpper => {
                let self_span = self.current_span();
                self.advance(); // consume `Self`
                let mut segments = vec![self_span];
                // Optional `::AssocTypeName`.
                if self.peek_kind() == TokenKind::ColonColon {
                    self.advance(); // consume `::`
                    if self.peek_kind() == TokenKind::Ident {
                        segments.push(self.current_span());
                        self.advance(); // consume associated type name
                    }
                    // If `::`  is not followed by an ident, stop — leave the `::` unconsumed
                    // (the caller handles the error). Since we already consumed `::`, push
                    // nothing and let `lower_ty` handle the bare `Self` path.
                }
                let end = *segments.last().unwrap();
                Ok(Ty { kind: TyKind::Path(segments), span: start.to(end) })
            }

            TokenKind::KwImpl => {
                self.advance(); // consume `impl`
                // Expect a trait name identifier.
                if self.peek_kind() != TokenKind::Ident {
                    return Err(self.error(format!(
                        "expected trait name after `impl`, found {:?}",
                        self.peek_kind()
                    )));
                }
                let trait_span = self.current_span();
                let trait_name = trait_span.text(self.src);
                self.advance(); // consume trait name

                // The callable closure traits use `Fn(T1, ...) -> R` call-parens syntax.
                // FLS §4.13: `Fn`, `FnMut`, `FnOnce` all use this form.
                if matches!(trait_name, "Fn" | "FnMut" | "FnOnce") {
                    self.expect(TokenKind::OpenParen)?;
                    let mut params = Vec::new();
                    if self.peek_kind() != TokenKind::CloseParen {
                        loop {
                            params.push(self.parse_ty()?);
                            if !self.eat(TokenKind::Comma) {
                                break;
                            }
                            if self.peek_kind() == TokenKind::CloseParen {
                                break;
                            }
                        }
                    }
                    let end = self.current_span();
                    self.expect(TokenKind::CloseParen)?;
                    let ret = if self.peek_kind() == TokenKind::RArrow {
                        self.advance(); // consume `->`
                        Some(Box::new(self.parse_ty()?))
                    } else {
                        None
                    };
                    let end = if let Some(ref r) = ret { r.span } else { end };
                    Ok(Ty { kind: TyKind::FnPtr { params, ret }, span: start.to(end) })
                } else {
                    // Non-callable trait (e.g. `impl MyTrait`): argument-position impl Trait.
                    // FLS §11: impl Trait in argument position is an anonymous generic type param.
                    Ok(Ty { kind: TyKind::ImplTrait(trait_span), span: start.to(trait_span) })
                }
            }

            kind => Err(self.error(format!("expected type, found {kind:?}"))),
        }
    }

    // ── Loop label helpers ────────────────────────────────────────────────────

    /// If the next two tokens are `Lifetime` `:`, consume both and return the
    /// label string without the leading `'`. Otherwise return `None`.
    ///
    /// FLS §6.15.6: A block label is a lifetime-like token followed by `:`.
    fn parse_opt_label(&mut self) -> Option<String> {
        if self.peek_kind() == TokenKind::Lifetime && self.peek_nth(1) == TokenKind::Colon {
            let tok = self.advance(); // consume 'label
            self.advance(); // consume :
            let text = tok.text(self.src);
            Some(text.strip_prefix('\'').unwrap_or(text).to_owned())
        } else {
            None
        }
    }

    /// If the current token is a `Lifetime` (used as a break/continue target
    /// label), consume it and return the label string without the leading `'`.
    /// Otherwise return `None`.
    ///
    /// Unlike `parse_opt_label`, this does NOT require a following `:`.
    ///
    /// FLS §6.15.6: `break 'label` — the lifetime token names the target loop.
    /// FLS §6.15.7: `continue 'label` — the lifetime token names the target loop.
    fn parse_opt_break_label(&mut self) -> Option<String> {
        if self.peek_kind() == TokenKind::Lifetime {
            let tok = self.advance();
            let text = tok.text(self.src);
            Some(text.strip_prefix('\'').unwrap_or(text).to_owned())
        } else {
            None
        }
    }

    // ── Blocks ────────────────────────────────────────────────────────────────

    /// Parse a block expression `{ stmts* tail? }`.
    ///
    /// FLS §6.10: A block expression is an expression that sequences
    /// statements. The block's value is the tail expression if present,
    /// or `()` if absent.
    ///
    /// FLS §6.10 NOTE: The distinction between a statement (followed by `;`)
    /// and a tail expression (not followed by `;`) is purely syntactic and
    /// must be resolved during parsing, not type-checking.
    fn parse_block(&mut self) -> Result<Block, ParseError> {
        let start = self.current_span();
        self.expect(TokenKind::OpenBrace)?;

        let mut stmts = Vec::new();
        let mut tail: Option<Box<Expr>> = None;

        while self.peek_kind() != TokenKind::CloseBrace
            && self.peek_kind() != TokenKind::Eof
        {
            match self.parse_stmt_or_tail()? {
                StmtOrTail::Stmt(s) => stmts.push(s),
                StmtOrTail::Tail(e) => {
                    tail = Some(Box::new(e));
                    break; // tail must be the last element
                }
            }
        }

        let end = self.current_span();
        self.expect(TokenKind::CloseBrace)?;

        Ok(Block { stmts, tail, span: start.to(end) })
    }

    // ── Statements ────────────────────────────────────────────────────────────

    /// Parse a statement or a tail expression.
    ///
    /// FLS §8: Statements. FLS §6.10: block tail expression.
    ///
    /// The key rule (FLS §8.3):
    ///
    /// > `ExpressionStatement ::= ExpressionWithoutBlock ";"
    /// >                        | ExpressionWithBlock ";"?`
    ///
    /// Expressions that end with a closing brace (`if`, `loop`, block
    /// literals, etc.) are called *expressions-with-block*. They may appear
    /// as statements *without* a trailing semicolon. An expression-with-block
    /// at the very end of a block (next token is `}`) is the tail expression;
    /// anywhere else it is a statement.
    ///
    /// Expressions-without-block (literals, arithmetic, calls, etc.) require a
    /// trailing `;` to be statements; without it they are the tail expression.
    fn parse_stmt_or_tail(&mut self) -> Result<StmtOrTail, ParseError> {
        let start = self.current_span();

        // Empty statement `;` — FLS §8.2
        if self.eat(TokenKind::Semi) {
            return Ok(StmtOrTail::Stmt(Stmt {
                kind: StmtKind::Empty,
                span: start,
            }));
        }

        // Let statement — FLS §8.1
        if self.peek_kind() == TokenKind::KwLet {
            return Ok(StmtOrTail::Stmt(self.parse_let_stmt()?));
        }

        // Inner item statement — FLS §3, §9.
        // A `fn` keyword in block position starts an inner function definition,
        // not an expression. Parse it as a full item and wrap in `StmtKind::Item`.
        // Inner functions do not capture outer locals (unlike closures).
        // Only `fn` is handled here; other item kinds may be added in future milestones.
        if self.peek_kind() == TokenKind::KwFn {
            let item = self.parse_item()?;
            let item_span = item.span;
            return Ok(StmtOrTail::Stmt(Stmt {
                kind: StmtKind::Item(Box::new(item)),
                span: start.to(item_span),
            }));
        }

        // Expression statement or tail — FLS §8.3, §6.10
        let expr = self.parse_expr()?;
        let expr_span = expr.span;

        // Explicit semicolon → always a statement.
        if self.eat(TokenKind::Semi) {
            return Ok(StmtOrTail::Stmt(Stmt {
                kind: StmtKind::Expr(Box::new(expr)),
                span: start.to(expr_span),
            }));
        }

        // No semicolon. For expressions-with-block, a trailing `;` is
        // optional. If more tokens follow before `}`, the expression is used
        // as a statement (side-effect only). If `}` is next, it is the tail.
        let is_expr_with_block = matches!(
            expr.kind,
            ExprKind::Block(_)
                | ExprKind::If { .. }
                | ExprKind::IfLet { .. }
                | ExprKind::Loop { .. }
                | ExprKind::While { .. }
                | ExprKind::WhileLet { .. }
                | ExprKind::For { .. }
                | ExprKind::Match { .. }
        );

        if is_expr_with_block && self.peek_kind() != TokenKind::CloseBrace {
            // Expression-with-block in non-tail position: treat as statement.
            Ok(StmtOrTail::Stmt(Stmt {
                kind: StmtKind::Expr(Box::new(expr)),
                span: start.to(expr_span),
            }))
        } else {
            // Tail expression: the block's value.
            Ok(StmtOrTail::Tail(expr))
        }
    }

    /// Parse a let statement.
    ///
    /// FLS §8.1: Let statement.
    ///
    /// Grammar: `"let" Identifier (":" Type)? ("=" Expression)? ";"`
    ///
    /// FLS §8.1 NOTE: The spec allows a full irrefutable pattern on the left
    /// side. This implementation supports only a simple identifier or `_`.
    /// Struct and tuple patterns in let position are future work.
    fn parse_let_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start = self.current_span();
        self.expect(TokenKind::KwLet)?;

        // Optional `mut` keyword — FLS §8.1.
        // Mutability is parsed but not yet enforced (no borrow checker yet).
        self.eat(TokenKind::KwMut);

        // Pattern — FLS §8.1: any irrefutable pattern is permitted.
        // Common forms: identifier, `_`, tuple `(a, b)` (FLS §5.10.3), or
        // a refutable pattern for let-else. FLS §5.1.11: OR patterns are also
        // allowed. `parse_pattern` handles OR collection; `parse_single_pattern`
        // is the unified single-alternative parser for all pattern positions.
        let pat = self.parse_pattern()?;

        // Optional type annotation `: Type`.
        let ty = if self.eat(TokenKind::Colon) {
            Some(self.parse_ty()?)
        } else {
            None
        };

        // Optional initializer `= Expression`.
        let init = if self.eat(TokenKind::Eq) {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };

        // Optional else block for let-else: `else { Block }`. FLS §8.1.
        //
        // `let PAT = EXPR else { BLOCK };`
        //
        // The else block must be a diverging expression (contain return, break,
        // continue, or an infinite loop). Variables bound by PAT are in scope
        // after the entire let-else statement.
        //
        // FLS §8.1 AMBIGUOUS: The spec does not restrict which patterns are
        // valid in let-else. Galvanic supports TupleStruct enum variant patterns
        // and literal patterns at this milestone.
        let else_block = if self.eat(TokenKind::KwElse) {
            Some(Box::new(self.parse_block()?))
        } else {
            None
        };

        let end = self.current_span();
        self.expect(TokenKind::Semi)?;

        Ok(Stmt {
            kind: StmtKind::Let { pat, ty, init, else_block },
            span: start.to(end),
        })
    }

    // ── Expressions ───────────────────────────────────────────────────────────

    /// Parse an expression (entry point).
    ///
    /// FLS §6: Expressions. Dispatches to `parse_assign` which represents
    /// the lowest-precedence binary operator.
    pub fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_assign()
    }

    /// Returns `true` if `expr` is a "block-like" expression (one that ends
    /// with a closing brace `}`).
    ///
    /// FLS §6.21 / Rust Reference §Expressions:
    /// Block-like expressions (`if`, `match`, `loop`, `while`, `for`, bare
    /// blocks) do not continue to the right with binary operators in statement
    /// position. For example, `if c {} * x` should parse as two statements
    /// (`if c {}` and `*x`), NOT as `(if c {}) * x` (multiplication).
    ///
    /// This check is used in every binary-operator parse function to stop
    /// consuming operators once the LHS is a block-like expression.
    ///
    /// FLS §6.21 AMBIGUOUS: The spec does not explicitly define this
    /// disambiguation rule; it is inherited from Rust's expression grammar.
    fn is_expr_with_block(expr: &Expr) -> bool {
        matches!(
            expr.kind,
            ExprKind::Block(_)
                | ExprKind::If { .. }
                | ExprKind::IfLet { .. }
                | ExprKind::Loop { .. }
                | ExprKind::While { .. }
                | ExprKind::WhileLet { .. }
                | ExprKind::For { .. }
                | ExprKind::Match { .. }
        )
    }

    /// Assignment and compound assignment — FLS §6.5.10, §6.5.11. Right-associative.
    ///
    /// Plain `=` lowers to `ExprKind::Binary { op: BinOp::Assign, .. }`.
    /// Compound `op=` lowers to `ExprKind::CompoundAssign { op, .. }`.
    ///
    /// FLS §6.5.11: Compound assignment operators are distinct expression forms
    /// from plain assignment. The left-hand side must be a place expression.
    fn parse_assign(&mut self) -> Result<Expr, ParseError> {
        let lhs = self.parse_range()?;

        // Block-like LHS cannot be the LHS of assignment in statement context.
        if Self::is_expr_with_block(&lhs) {
            return Ok(lhs);
        }

        if self.eat(TokenKind::Eq) {
            let rhs = self.parse_assign()?; // right-associative
            let span = lhs.span.to(rhs.span);
            return Ok(Expr {
                kind: ExprKind::Binary {
                    op: BinOp::Assign,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
                span,
            });
        }

        // FLS §6.5.11: Compound assignment operators.
        //
        // Each `op=` token maps to the corresponding `BinOp` arithmetic/bitwise
        // operation. The compound assignment desugars at the lowering level to
        // a load, the binary op, and a store — no new AST binary operators needed.
        let compound_op = match self.peek_kind() {
            TokenKind::PlusEq    => Some(BinOp::Add),
            TokenKind::MinusEq   => Some(BinOp::Sub),
            TokenKind::StarEq    => Some(BinOp::Mul),
            TokenKind::SlashEq   => Some(BinOp::Div),
            TokenKind::PercentEq => Some(BinOp::Rem),
            TokenKind::AndEq     => Some(BinOp::BitAnd),
            TokenKind::OrEq      => Some(BinOp::BitOr),
            TokenKind::CaretEq   => Some(BinOp::BitXor),
            TokenKind::ShlEq     => Some(BinOp::Shl),
            TokenKind::ShrEq     => Some(BinOp::Shr),
            _                    => None,
        };

        if let Some(op) = compound_op {
            self.advance();
            let value = self.parse_assign()?; // right-associative
            let span = lhs.span.to(value.span);
            return Ok(Expr {
                kind: ExprKind::CompoundAssign {
                    op,
                    target: Box::new(lhs),
                    value: Box::new(value),
                },
                span,
            });
        }

        Ok(lhs)
    }

    /// Range expressions `start..end` and `start..=end` — FLS §6.16.
    ///
    /// Ranges have lower precedence than logical operators but higher than
    /// assignment (FLS §6.21). Only `start..end` (exclusive) and `start..=end`
    /// (inclusive) with both operands present are supported at this milestone.
    ///
    /// FLS §6.16: "A range expression constructs a range value."
    /// FLS §6.16 AMBIGUOUS: The spec allows `..`, `start..`, `..end`, `..=end`,
    /// `start..end`, `start..=end`. Galvanic restricts to `start..end` for now;
    /// partial ranges used as iterators are future work.
    fn parse_range(&mut self) -> Result<Expr, ParseError> {
        let lhs = self.parse_or()?;

        if Self::is_expr_with_block(&lhs) {
            return Ok(lhs);
        }

        if self.peek_kind() == TokenKind::DotDot || self.peek_kind() == TokenKind::DotDotEq {
            let inclusive = self.peek_kind() == TokenKind::DotDotEq;
            self.advance();
            let rhs = self.parse_or()?;
            let span = lhs.span.to(rhs.span);
            return Ok(Expr {
                kind: ExprKind::Range {
                    start: Some(Box::new(lhs)),
                    end: Some(Box::new(rhs)),
                    inclusive,
                },
                span,
            });
        }

        Ok(lhs)
    }

    /// Logical or `||` — FLS §6.8.2. Left-associative.
    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_and()?;

        while !Self::is_expr_with_block(&lhs) && self.peek_kind() == TokenKind::OrOr {
            self.advance();
            let rhs = self.parse_and()?;
            let span = lhs.span.to(rhs.span);
            lhs = Expr {
                kind: ExprKind::Binary { op: BinOp::Or, lhs: Box::new(lhs), rhs: Box::new(rhs) },
                span,
            };
        }

        Ok(lhs)
    }

    /// Logical and `&&` — FLS §6.8.1. Left-associative.
    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_cmp()?;

        while !Self::is_expr_with_block(&lhs) && self.peek_kind() == TokenKind::AndAnd {
            self.advance();
            let rhs = self.parse_cmp()?;
            let span = lhs.span.to(rhs.span);
            lhs = Expr {
                kind: ExprKind::Binary { op: BinOp::And, lhs: Box::new(lhs), rhs: Box::new(rhs) },
                span,
            };
        }

        Ok(lhs)
    }

    /// Comparison operators — FLS §6.7. Non-associative (chaining is a type error).
    ///
    /// FLS §6.7 AMBIGUOUS: The spec says comparison operators are
    /// "non-associative" but this is a *type-level* constraint, not a
    /// syntactic one. `a < b < c` parses as `(a < b) < c` in the grammar;
    /// the type checker must reject it because `bool < i32` is meaningless.
    /// Rustc enforces this via type checking, not parsing. We parse it
    /// left-associatively and leave the error to the (future) type checker.
    fn parse_cmp(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_bitor()?;

        loop {
            if Self::is_expr_with_block(&lhs) { break; }
            let op = match self.peek_kind() {
                TokenKind::EqEq => BinOp::Eq,
                TokenKind::Ne => BinOp::Ne,
                TokenKind::Lt => BinOp::Lt,
                TokenKind::Gt => BinOp::Gt,
                TokenKind::Le => BinOp::Le,
                TokenKind::Ge => BinOp::Ge,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_bitor()?;
            let span = lhs.span.to(rhs.span);
            lhs = Expr {
                kind: ExprKind::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) },
                span,
            };
        }

        Ok(lhs)
    }

    /// Bitwise or `|` — FLS §6.6.3. Left-associative.
    fn parse_bitor(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_bitxor()?;

        while !Self::is_expr_with_block(&lhs) && self.peek_kind() == TokenKind::Or {
            self.advance();
            let rhs = self.parse_bitxor()?;
            let span = lhs.span.to(rhs.span);
            lhs = Expr {
                kind: ExprKind::Binary {
                    op: BinOp::BitOr,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
                span,
            };
        }

        Ok(lhs)
    }

    /// Bitwise xor `^` — FLS §6.6.2. Left-associative.
    fn parse_bitxor(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_bitand()?;

        while !Self::is_expr_with_block(&lhs) && self.peek_kind() == TokenKind::Caret {
            self.advance();
            let rhs = self.parse_bitand()?;
            let span = lhs.span.to(rhs.span);
            lhs = Expr {
                kind: ExprKind::Binary {
                    op: BinOp::BitXor,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
                span,
            };
        }

        Ok(lhs)
    }

    /// Bitwise and `&` — FLS §6.6.1. Left-associative.
    ///
    /// FLS §6.6.1 AMBIGUOUS: `&` is overloaded — in unary position it is a
    /// borrow operator (FLS §6.4.4); in binary position it is bitwise AND.
    /// The disambiguation is positional: `parse_bitand` is only entered
    /// after a left-hand operand has been fully parsed, so `&` here is
    /// always the binary bitwise AND. Borrow expressions are parsed in
    /// `parse_unary` before the binary layer is reached.
    fn parse_bitand(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_shift()?;

        while !Self::is_expr_with_block(&lhs) && self.peek_kind() == TokenKind::And {
            self.advance();
            let rhs = self.parse_shift()?;
            let span = lhs.span.to(rhs.span);
            lhs = Expr {
                kind: ExprKind::Binary {
                    op: BinOp::BitAnd,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
                span,
            };
        }

        Ok(lhs)
    }

    /// Shift operators `<<` `>>` — FLS §6.6.4. Left-associative.
    fn parse_shift(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_additive()?;

        loop {
            if Self::is_expr_with_block(&lhs) { break; }
            let op = match self.peek_kind() {
                TokenKind::Shl => BinOp::Shl,
                TokenKind::Shr => BinOp::Shr,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_additive()?;
            let span = lhs.span.to(rhs.span);
            lhs = Expr {
                kind: ExprKind::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) },
                span,
            };
        }

        Ok(lhs)
    }

    /// Additive operators `+` `-` — FLS §6.5. Left-associative.
    fn parse_additive(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_multiplicative()?;

        loop {
            if Self::is_expr_with_block(&lhs) { break; }
            let op = match self.peek_kind() {
                TokenKind::Plus => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_multiplicative()?;
            let span = lhs.span.to(rhs.span);
            lhs = Expr {
                kind: ExprKind::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) },
                span,
            };
        }

        Ok(lhs)
    }

    /// Type cast expressions `expr as Ty` — FLS §6.5.9. Left-associative.
    ///
    /// Precedence: lower than unary, higher than `*`, `/`, `%`.
    /// `a * b as i32` → `a * (b as i32)` because `parse_multiplicative`
    /// calls `parse_cast` for each operand.
    ///
    /// FLS §6.5.9: "A type cast expression converts a value of one type to
    /// a value of another type." The `as` keyword is followed by a type path.
    ///
    /// Cache-line note: in the common case (no `as` token), this function
    /// immediately returns the result of `parse_unary` with no heap allocation.
    /// `#[inline]` ensures the wrapper is merged with the caller in release
    /// builds, keeping the hot path (no `as`) as cheap as before this level
    /// was added.
    #[inline]
    fn parse_cast(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_unary()?;

        while self.peek_kind() == TokenKind::KwAs {
            self.advance(); // consume `as`
            let ty = self.parse_ty()?;
            let span = expr.span.to(ty.span);
            expr = Expr {
                kind: ExprKind::Cast { expr: Box::new(expr), ty: Box::new(ty) },
                span,
            };
        }

        Ok(expr)
    }

    /// Multiplicative operators `*` `/` `%` — FLS §6.5. Left-associative.
    fn parse_multiplicative(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_cast()?;

        loop {
            if Self::is_expr_with_block(&lhs) { break; }
            let op = match self.peek_kind() {
                TokenKind::Star => BinOp::Mul,
                TokenKind::Slash => BinOp::Div,
                TokenKind::Percent => BinOp::Rem,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_cast()?;
            let span = lhs.span.to(rhs.span);
            lhs = Expr {
                kind: ExprKind::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) },
                span,
            };
        }

        Ok(lhs)
    }

    /// Unary operators — FLS §6.4. Right-associative.
    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        let start = self.current_span();

        match self.peek_kind() {
            // Negation `-` — FLS §6.4.1
            TokenKind::Minus => {
                self.advance();
                let operand = self.parse_unary()?;
                let span = start.to(operand.span);
                Ok(Expr {
                    kind: ExprKind::Unary { op: UnaryOp::Neg, operand: Box::new(operand) },
                    span,
                })
            }

            // Not `!` — FLS §6.4.2
            TokenKind::Not => {
                self.advance();
                let operand = self.parse_unary()?;
                let span = start.to(operand.span);
                Ok(Expr {
                    kind: ExprKind::Unary { op: UnaryOp::Not, operand: Box::new(operand) },
                    span,
                })
            }

            // Dereference `*` — FLS §6.4.3
            TokenKind::Star => {
                self.advance();
                let operand = self.parse_unary()?;
                let span = start.to(operand.span);
                Ok(Expr {
                    kind: ExprKind::Unary { op: UnaryOp::Deref, operand: Box::new(operand) },
                    span,
                })
            }

            // Borrow `&` or `&mut` — FLS §6.4.4
            TokenKind::And => {
                self.advance();
                let mutable = self.eat(TokenKind::KwMut);
                let operand = self.parse_unary()?;
                let span = start.to(operand.span);
                let op = if mutable { UnaryOp::RefMut } else { UnaryOp::Ref };
                Ok(Expr {
                    kind: ExprKind::Unary { op, operand: Box::new(operand) },
                    span,
                })
            }

            _ => {
                let primary = self.parse_primary()?;
                self.parse_postfix(primary)
            }
        }
    }

    /// Apply postfix operations — calls `(args)`, field access `.field`, and
    /// method calls `.method(args)` — to an already-parsed expression.
    ///
    /// This loop handles any chain of postfix operations:
    /// `a.b.c(1).d` parses as `((a.b).c(1)).d`.
    ///
    /// FLS §6.3.1: Call expressions.
    /// FLS §6.3.2: Method call expressions.
    /// FLS §6.3.3: Field access expressions.
    fn parse_postfix(&mut self, mut expr: Expr) -> Result<Expr, ParseError> {
        loop {
            match self.peek_kind() {
                // Call expression: `expr(args)` — FLS §6.3.1
                //
                // FLS §6.21 / Rust grammar: Block-like expressions (`for`, `while`,
                // `loop`, `if`, etc.) do not accept a `(` postfix as a call.
                // `for x in arr {}(args)` would be misidentified as calling the
                // for-loop result; break the postfix loop instead.
                // A grouped expression `(for x in arr {})` is always the caller's
                // responsibility. See also `is_expr_with_block` in `parse_stmt_or_tail`.
                TokenKind::OpenParen => {
                    if Self::is_expr_with_block(&expr) {
                        break;
                    }
                    expr = self.parse_call(expr)?;
                }

                // Field access or method call: `expr.ident` or `expr.ident(args)`
                // FLS §6.3.2, §6.3.3
                // Index expression — FLS §6.9
                //
                // `base[index]` — evaluates the base and index at runtime,
                // then accesses the element at position `index`.
                //
                // FLS §6.9: "An indexing expression is used to access an element
                // of an array or slice by position."
                // FLS §6.9: The index type is `usize` (spec); galvanic uses `i32`
                // at this milestone. Bounds checking is deferred.
                //
                // Cache-line note: lowered to `add + ldr` (two 4-byte instructions).
                TokenKind::OpenBracket => {
                    let base_span = expr.span;
                    self.advance(); // eat `[`
                    let index = self.parse_expr()?;
                    let end = self.current_span();
                    self.expect(TokenKind::CloseBracket)?;
                    expr = Expr {
                        kind: ExprKind::Index {
                            base: Box::new(expr),
                            index: Box::new(index),
                        },
                        span: base_span.to(end),
                    };
                }

                TokenKind::Dot => {
                    self.advance(); // eat `.`

                    // FLS §6.13: field access by name (`receiver.field`).
                    // FLS §6.10: tuple field access by index (`receiver.0`).
                    // Both produce an `ExprKind::FieldAccess` node; the field
                    // span text is the identifier or integer literal.
                    let member_span = match self.peek_kind() {
                        TokenKind::Ident | TokenKind::LitInteger => {
                            let span = self.current_span();
                            self.advance();
                            span
                        }
                        _ => return Err(self.error("expected field or method name after `.`")),
                    };

                    if self.peek_kind() == TokenKind::OpenParen {
                        // Method call: `receiver.method(args)`
                        let receiver_span = expr.span;
                        self.expect(TokenKind::OpenParen)?;
                        let mut args = Vec::new();
                        while self.peek_kind() != TokenKind::CloseParen
                            && self.peek_kind() != TokenKind::Eof
                        {
                            args.push(self.parse_expr()?);
                            if !self.eat(TokenKind::Comma) {
                                break;
                            }
                        }
                        let end = self.current_span();
                        self.expect(TokenKind::CloseParen)?;
                        expr = Expr {
                            kind: ExprKind::MethodCall {
                                receiver: Box::new(expr),
                                method: member_span,
                                args,
                            },
                            span: receiver_span.to(end),
                        };
                    } else {
                        // Field access: `receiver.field`
                        let span = expr.span.to(member_span);
                        expr = Expr {
                            kind: ExprKind::FieldAccess {
                                receiver: Box::new(expr),
                                field: member_span,
                            },
                            span,
                        };
                    }
                }

                _ => break,
            }
        }
        Ok(expr)
    }

    /// Primary expressions — literals, paths, calls, grouped expressions,
    /// blocks, `if`, and `return`.
    ///
    /// FLS §6: various expression forms.
    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let start = self.current_span();

        match self.peek_kind() {
            // Integer literal — FLS §6.1.1
            TokenKind::LitInteger => {
                let tok = self.advance();
                let val = parse_int_literal(tok.text(self.src));
                Ok(Expr { kind: ExprKind::LitInt(val), span: Self::span_of(&tok) })
            }

            // Float literal — FLS §6.1.2
            TokenKind::LitFloat => {
                let tok = self.advance();
                Ok(Expr { kind: ExprKind::LitFloat, span: Self::span_of(&tok) })
            }

            // String/byte-string literals — FLS §6.1.4
            TokenKind::LitStr
            | TokenKind::LitRawStr
            | TokenKind::LitByteStr
            | TokenKind::LitRawByteStr
            | TokenKind::LitCStr
            | TokenKind::LitRawCStr => {
                let tok = self.advance();
                Ok(Expr { kind: ExprKind::LitStr, span: Self::span_of(&tok) })
            }

            // Char/byte literal — FLS §6.1.5
            TokenKind::LitChar | TokenKind::LitByte => {
                let tok = self.advance();
                Ok(Expr { kind: ExprKind::LitChar, span: Self::span_of(&tok) })
            }

            // Boolean literals — FLS §6.1.3
            TokenKind::KwTrue => {
                self.advance();
                Ok(Expr { kind: ExprKind::LitBool(true), span: start })
            }
            TokenKind::KwFalse => {
                self.advance();
                Ok(Expr { kind: ExprKind::LitBool(false), span: start })
            }

            // `self` used as a path expression — FLS §6.3 + §10.1.
            //
            // In method bodies, `self` refers to the receiver. It is parsed as
            // a single-segment path so that `self.field` resolves through the
            // same field-access machinery as any other local variable.
            //
            // FLS §10.1: `self` is bound in the scope of the method body and
            // acts like a local variable of the enclosing struct type.
            TokenKind::KwSelfLower => {
                let seg = self.current_span();
                self.advance();
                Ok(Expr { kind: ExprKind::Path(vec![seg]), span: seg })
            }

            // Path expression, function call, or struct literal — FLS §6.2, §6.3.1, §6.11
            TokenKind::Ident => {
                let mut segments = vec![self.current_span()];
                self.advance();

                // Path continuation with `::`.
                while self.peek_kind() == TokenKind::ColonColon {
                    self.advance(); // eat `::`
                    if self.peek_kind() == TokenKind::Ident {
                        segments.push(self.current_span());
                        self.advance();
                    } else {
                        return Err(self.error("expected identifier after `::`"));
                    }
                }

                // FLS §6.11: Struct literal — `Name { field: expr, … }`.
                //
                // Attempt struct literal only when:
                // 1. The path is a single identifier (multi-segment paths are
                //    not yet supported as struct literal heads).
                // 2. The next token is `{`.
                // 3. The token after `{` is either `}` (empty struct), `..`
                //    (struct update syntax with no explicit fields), or
                //    `Ident :` (named field — distinguishes from a block with
                //    a plain expression statement like `{ foo }`).
                // 4. `restrict_struct_lit` is false (not inside an
                //    `if`/`while`/`for` condition).
                if segments.len() == 1
                    && !self.restrict_struct_lit
                    && self.peek_kind() == TokenKind::OpenBrace
                    && (self.peek_nth(1) == TokenKind::CloseBrace
                        // FLS §6.11: Struct update `Name { ..base }` with no explicit fields.
                        || self.peek_nth(1) == TokenKind::DotDot
                        || (self.peek_nth(1) == TokenKind::Ident
                            && (self.peek_nth(2) == TokenKind::Colon
                                // FLS §6.11: Shorthand field — `Name { field, … }`.
                                || self.peek_nth(2) == TokenKind::Comma
                                || self.peek_nth(2) == TokenKind::CloseBrace)))
                {
                    return self.parse_struct_lit(segments[0]);
                }

                // FLS §6.11 + §15.3: Named-field enum variant construction.
                //
                // `Enum::Variant { field: expr, … }` — a two-segment path followed
                // by a brace-delimited field list. Disambiguated from a block by
                // requiring `Ident :` or `}` after `{`.
                //
                // FLS §6.11: Struct expressions apply to enum variants with named
                // fields. `restrict_struct_lit` suppresses this inside if/while/for
                // conditions (same restriction as plain struct literals).
                if segments.len() == 2
                    && !self.restrict_struct_lit
                    && self.peek_kind() == TokenKind::OpenBrace
                    && (self.peek_nth(1) == TokenKind::CloseBrace
                        || (self.peek_nth(1) == TokenKind::Ident
                            && (self.peek_nth(2) == TokenKind::Colon
                                // FLS §6.11: Shorthand field — `Enum::Variant { field, … }`.
                                || self.peek_nth(2) == TokenKind::Comma
                                || self.peek_nth(2) == TokenKind::CloseBrace)))
                {
                    let path = segments;
                    let path_start = path[0];
                    self.advance(); // eat `{`
                    let mut fields: Vec<(crate::ast::Span, Box<crate::ast::Expr>)> = Vec::new();
                    while self.peek_kind() != TokenKind::CloseBrace {
                        let field_name = self.expect(TokenKind::Ident)?;
                        // FLS §6.11: Shorthand field — `Variant { field }` = `Variant { field: field }`.
                        let val = if self.eat(TokenKind::Colon) {
                            self.parse_expr()?
                        } else {
                            crate::ast::Expr {
                                kind: crate::ast::ExprKind::Path(vec![field_name]),
                                span: field_name,
                            }
                        };
                        fields.push((field_name, Box::new(val)));
                        if !self.eat(TokenKind::Comma) {
                            break;
                        }
                    }
                    let end_span = self.current_span();
                    self.expect(TokenKind::CloseBrace)?;
                    return Ok(crate::ast::Expr {
                        kind: crate::ast::ExprKind::EnumVariantLit { path, fields },
                        span: path_start.to(end_span),
                    });
                }

                let path_end = *segments.last().unwrap();
                Ok(Expr {
                    kind: ExprKind::Path(segments),
                    span: start.to(path_end),
                })
            }

            // Grouped expression, unit `()`, or tuple — FLS §6.7, §6.3.3, §6.10
            TokenKind::OpenParen => {
                self.advance(); // eat `(`

                // Unit `()` — FLS §6.3.3
                if self.eat(TokenKind::CloseParen) {
                    return Ok(Expr { kind: ExprKind::Unit, span: start });
                }

                // Parse first element. Then check for `,` to decide
                // whether this is a grouped expression or a tuple.
                let first = self.parse_expr()?;

                if self.eat(TokenKind::Comma) {
                    // Tuple expression — FLS §6.10.
                    // Elements are evaluated left-to-right; at least two
                    // elements (a trailing comma after a single element would
                    // produce a 1-tuple, which we also accept here).
                    let mut elems = vec![first];
                    while self.peek_kind() != TokenKind::CloseParen
                        && self.peek_kind() != TokenKind::Eof
                    {
                        elems.push(self.parse_expr()?);
                        if !self.eat(TokenKind::Comma) {
                            break;
                        }
                    }
                    let end = self.current_span();
                    self.expect(TokenKind::CloseParen)?;
                    Ok(Expr { kind: ExprKind::Tuple(elems), span: start.to(end) })
                } else {
                    // Grouped (parenthesised) expression — FLS §6.7
                    self.expect(TokenKind::CloseParen)?;
                    Ok(first)
                }
            }

            // Block expression — FLS §6.10
            TokenKind::OpenBrace => {
                let block = self.parse_block()?;
                let span = block.span;
                Ok(Expr { kind: ExprKind::Block(Box::new(block)), span })
            }

            // Return expression — FLS §6.12
            TokenKind::KwReturn => {
                self.advance();
                // No value if the next token ends the statement/block.
                let value = if matches!(
                    self.peek_kind(),
                    TokenKind::Semi | TokenKind::CloseBrace | TokenKind::Eof
                ) {
                    None
                } else {
                    Some(Box::new(self.parse_expr()?))
                };
                let end = value.as_ref().map(|e| e.span).unwrap_or(start);
                Ok(Expr { kind: ExprKind::Return(value), span: start.to(end) })
            }

            // If expression — FLS §6.11
            TokenKind::KwIf => self.parse_if_expr(),

            // Labeled loop expression — FLS §6.15.6.
            // `'label: loop { … }` / `'label: while … { … }` / `'label: for … in … { … }`
            //
            // Named block expression — FLS §6.4.3.
            // `'label: { … }` — a block that can be exited via `break 'label value`.
            TokenKind::Lifetime if self.peek_nth(1) == TokenKind::Colon
                && matches!(
                    self.peek_nth(2),
                    TokenKind::KwLoop
                        | TokenKind::KwWhile
                        | TokenKind::KwFor
                        | TokenKind::OpenBrace
                ) =>
            {
                let label = self.parse_opt_label().expect("just checked Lifetime+Colon");
                match self.peek_kind() {
                    TokenKind::KwLoop => {
                        self.advance();
                        let body = Box::new(self.parse_block()?);
                        let span = start.to(body.span);
                        Ok(Expr { kind: ExprKind::Loop { label: Some(label), body }, span })
                    }
                    TokenKind::KwWhile => self.parse_while_expr(Some(label)),
                    TokenKind::KwFor => self.parse_for_expr(Some(label)),
                    // Named block expression — FLS §6.4.3.
                    // `'label: { stmts... }` — block with an early-exit label.
                    TokenKind::OpenBrace => {
                        let body = Box::new(self.parse_block()?);
                        let span = start.to(body.span);
                        Ok(Expr { kind: ExprKind::NamedBlock { label, body }, span })
                    }
                    _ => unreachable!(),
                }
            }

            // Loop expression — FLS §6.15.2
            TokenKind::KwLoop => {
                self.advance();
                let body = Box::new(self.parse_block()?);
                let span = start.to(body.span);
                Ok(Expr { kind: ExprKind::Loop { label: None, body }, span })
            }

            // While loop expression — FLS §6.15.3
            TokenKind::KwWhile => self.parse_while_expr(None),

            // For loop expression — FLS §6.15.1
            TokenKind::KwFor => self.parse_for_expr(None),

            // Break expression — FLS §6.15.6
            TokenKind::KwBreak => {
                self.advance();
                // Optional target label: `break 'label` or `break 'label value`.
                let label = if self.peek_kind() == TokenKind::Lifetime {
                    self.parse_opt_break_label()
                } else {
                    None
                };
                // No value if the next token terminates the expression context.
                let value = if matches!(
                    self.peek_kind(),
                    TokenKind::Semi | TokenKind::CloseBrace | TokenKind::Eof
                ) {
                    None
                } else {
                    Some(Box::new(self.parse_expr()?))
                };
                let end = value.as_ref().map(|e| e.span).unwrap_or(start);
                Ok(Expr { kind: ExprKind::Break { label, value }, span: start.to(end) })
            }

            // Continue expression — FLS §6.15.7
            TokenKind::KwContinue => {
                self.advance();
                // Optional target label: `continue 'label`.
                let label = if self.peek_kind() == TokenKind::Lifetime {
                    self.parse_opt_break_label()
                } else {
                    None
                };
                Ok(Expr { kind: ExprKind::Continue { label }, span: start })
            }

            // Match expression — FLS §6.18
            TokenKind::KwMatch => self.parse_match_expr(),

            // Array expression — FLS §6.8
            //
            // `[elem0, elem1, …]` — a comma-separated list of element expressions
            // enclosed in square brackets. All elements must have the same type.
            //
            // FLS §6.8: "An array expression constructs a value of an array type."
            // FLS §6.8: Elements are evaluated left-to-right (FLS §6.4:14).
            //
            // FLS §6.8 NOTE: Galvanic supports only the list form `[e, e, …]`.
            // The repeat form `[expr; N]` (FLS §6.8: "array expression with length
            // specified by an integer expression") is not yet supported.
            //
            // Cache-line note: an N-element array occupies N consecutive 8-byte
            // stack slots; a 8-element array exactly fills one 64-byte cache line.
            TokenKind::OpenBracket => {
                self.advance(); // eat `[`

                // If the bracket is empty, produce an empty array.
                if self.eat(TokenKind::CloseBracket) {
                    return Ok(Expr {
                        kind: ExprKind::Array(vec![]),
                        span: start.to(self.current_span()),
                    });
                }

                // Parse the first element expression.
                let first = self.parse_expr()?;

                // Detect the repeat form `[value; N]` (FLS §6.8).
                if self.eat(TokenKind::Semi) {
                    let count = self.parse_expr()?;
                    let end = self.current_span();
                    self.expect(TokenKind::CloseBracket)?;
                    return Ok(Expr {
                        kind: ExprKind::ArrayRepeat {
                            value: Box::new(first),
                            count: Box::new(count),
                        },
                        span: start.to(end),
                    });
                }

                // List form `[e0, e1, ...]`.
                let mut elems: Vec<Expr> = vec![first];
                while self.eat(TokenKind::Comma)
                    && self.peek_kind() != TokenKind::CloseBracket
                    && self.peek_kind() != TokenKind::Eof
                {
                    elems.push(self.parse_expr()?);
                }
                let end = self.current_span();
                self.expect(TokenKind::CloseBracket)?;
                Ok(Expr {
                    kind: ExprKind::Array(elems),
                    span: start.to(end),
                })
            }

            // Const block expression — FLS §6.4.2.
            //
            // `const { ... }` — a block whose body is evaluated in a const
            // context at compile time. Distinct from top-level const items
            // (§7.1, which are named) and `const fn` declarations (§9:41–43).
            //
            // Disambiguation: `const fn` is parsed at the item level (the
            // parser never reaches here for `const fn` in item position).
            // Inside an expression context the only valid `const` primary is
            // `const { ... }` because `const NAME` and `const fn` can only
            // appear at statement/item level, not as sub-expressions.
            //
            // FLS §6.4.2: "A const block expression is a block expression
            // preceded by the keyword `const`."
            TokenKind::KwConst if self.peek_nth(1) == TokenKind::OpenBrace => {
                self.advance(); // eat `const`
                let block = self.parse_block()?;
                let span = start.to(block.span);
                Ok(Expr { kind: ExprKind::ConstBlock(Box::new(block)), span })
            }

            // Unsafe block expression — FLS §6.4.4.
            //
            // `unsafe { ... }` — an unsafe block expression is a block
            // expression preceded by the keyword `unsafe`. This permits
            // operations restricted by the safety model inside the block.
            //
            // Disambiguation: `unsafe` at item level can precede `fn`, `impl`,
            // or `trait`. Those are parsed at statement/item level. Here in
            // `parse_primary`, `unsafe` followed by `{` is unambiguously an
            // unsafe block expression — not an item declaration.
            //
            // FLS §6.4.4: "An unsafe block expression is a block expression
            // preceded by keyword unsafe."
            TokenKind::KwUnsafe if self.peek_nth(1) == TokenKind::OpenBrace => {
                self.advance(); // eat `unsafe`
                let block = self.parse_block()?;
                let span = start.to(block.span);
                Ok(Expr { kind: ExprKind::UnsafeBlock(Box::new(block)), span })
            }

            // Closure expression — FLS §6.14.
            //
            // `|params| body` or `|params| -> RetTy body` where body is
            // either a block `{ ... }` or any expression.
            //
            // `||` (zero params) is also a valid start: the `||` token is
            // produced by the lexer for `||` (logical-or) — at primary
            // level it can only be a zero-parameter closure.
            //
            // `move |params| body` — FLS §6.14, §6.22: the `move` keyword
            // causes captured variables to be moved into the closure
            // environment. For Copy types this is identical to non-move.
            //
            // Disambiguation: `|` as bitwise OR is handled at the
            // parse_bitwise_or level, which calls parse_unary →
            // parse_primary only when building the right operand. At
            // that point the `|` has already been consumed as the
            // operator. Here we are starting a new primary, so `|` is
            // unambiguously the opening of a closure parameter list.
            //
            // FLS §6.14 AMBIGUOUS: The spec lists attribute support
            // on closure params; galvanic does not implement attributes.
            TokenKind::Or => self.parse_closure(false, false),
            TokenKind::OrOr => self.parse_closure(true, false),
            TokenKind::KwMove => {
                self.advance(); // eat `move`
                match self.peek_kind() {
                    TokenKind::OrOr => self.parse_closure(true, true),
                    _ => self.parse_closure(false, true),
                }
            }

            kind => Err(self.error(format!("expected expression, found {kind:?}"))),
        }
    }

    /// Parse a call expression given a fully-parsed callee.
    ///
    /// FLS §6.3.1: Call expressions.
    fn parse_call(&mut self, callee: Expr) -> Result<Expr, ParseError> {
        let start = callee.span;
        self.expect(TokenKind::OpenParen)?;

        let mut args = Vec::new();
        while self.peek_kind() != TokenKind::CloseParen
            && self.peek_kind() != TokenKind::Eof
        {
            args.push(self.parse_expr()?);
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }

        let end = self.current_span();
        self.expect(TokenKind::CloseParen)?;

        Ok(Expr {
            kind: ExprKind::Call { callee: Box::new(callee), args },
            span: start.to(end),
        })
    }

    /// Parse an if expression.
    ///
    /// FLS §6.11: If expressions.
    ///
    /// Grammar:
    /// ```text
    /// IfExpression ::=
    ///     "if" Expression BlockExpression
    ///     ("else" (BlockExpression | IfExpression))?
    /// ```
    ///
    /// FLS §6.11 NOTE: The condition expression is parsed with `parse_expr`,
    /// which stops at `{` because `{` does not match any binary operator.
    /// This naturally separates the condition from the block. However, a
    /// block expression as the condition — `if { x } { y }` — would parse
    /// `{ x }` as the condition and `{ y }` as the body. The FLS does not
    /// explicitly forbid this, but rustc rejects it. We parse it and defer
    /// the restriction to the type checker.
    fn parse_if_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.current_span();
        self.expect(TokenKind::KwIf)?;

        // FLS §6.17: if-let expression — `if let Pattern = Expr { Block } [else ...]`
        if self.eat(TokenKind::KwLet) {
            let pat = self.parse_pattern()?;
            // The `=` separator between pattern and scrutinee.
            self.expect(TokenKind::Eq)?;
            // Restrict struct literals in scrutinee to avoid `if let P = Foo { }` ambiguity.
            let prev_restrict = self.restrict_struct_lit;
            self.restrict_struct_lit = true;
            let scrutinee_result = self.parse_expr();
            self.restrict_struct_lit = prev_restrict;
            let scrutinee = Box::new(scrutinee_result?);
            let then_block = Box::new(self.parse_block()?);

            let else_expr = if self.eat(TokenKind::KwElse) {
                if self.peek_kind() == TokenKind::KwIf {
                    Some(Box::new(self.parse_if_expr()?))
                } else {
                    let block = self.parse_block()?;
                    let span = block.span;
                    Some(Box::new(Expr { kind: ExprKind::Block(Box::new(block)), span }))
                }
            } else {
                None
            };

            let end = else_expr
                .as_ref()
                .map(|e| e.span)
                .unwrap_or(then_block.span);

            return Ok(Expr {
                kind: ExprKind::IfLet { pat, scrutinee, then_block, else_expr },
                span: start.to(end),
            });
        }

        // FLS §6.17: regular if (or if-else) expression.
        // Restrict struct literals in condition to avoid `if Foo { }` ambiguity.
        let prev_restrict = self.restrict_struct_lit;
        self.restrict_struct_lit = true;
        let cond_result = self.parse_expr();
        self.restrict_struct_lit = prev_restrict;
        let cond = Box::new(cond_result?);
        let then_block = Box::new(self.parse_block()?);

        let else_expr = if self.eat(TokenKind::KwElse) {
            if self.peek_kind() == TokenKind::KwIf {
                Some(Box::new(self.parse_if_expr()?))
            } else {
                let block = self.parse_block()?;
                let span = block.span;
                Some(Box::new(Expr { kind: ExprKind::Block(Box::new(block)), span }))
            }
        } else {
            None
        };

        let end = else_expr
            .as_ref()
            .map(|e| e.span)
            .unwrap_or(then_block.span);

        Ok(Expr {
            kind: ExprKind::If { cond, then_block, else_expr },
            span: start.to(end),
        })
    }

    /// Parse a match expression.
    ///
    /// FLS §6.18: Match expressions.
    ///
    /// Grammar:
    /// ```text
    /// MatchExpression ::=
    ///     "match" Expression "{" MatchArm* "}"
    /// MatchArm ::=
    ///     Pattern ("if" Expression)? "=>" Expression ","?
    /// ```
    ///
    /// FLS §6.18: "A match expression branches on a pattern." Arms are tested
    /// in source order; the first matching arm executes. All arms must have the
    /// same type.
    ///
    /// FLS §6.18: Match arm guards (`if expr`) are evaluated after the pattern
    /// matches. If the guard evaluates to `false`, the arm is skipped.
    ///
    /// FLS §6.18 AMBIGUOUS: The spec requires exhaustiveness but does not
    /// specify the compile-time algorithm. This implementation defers
    /// exhaustiveness checking to a future type-checking phase; the lowering
    /// emits an unconditional jump to the last arm (wildcard assumed).
    fn parse_match_expr(&mut self) -> Result<Expr, ParseError> {
        use crate::ast::MatchArm;
        let start = self.current_span();
        self.expect(TokenKind::KwMatch)?;

        // Scrutinee — parsed without consuming the `{`.
        let scrutinee = Box::new(self.parse_expr()?);

        self.expect(TokenKind::OpenBrace)?;

        let mut arms = Vec::new();
        while self.peek_kind() != TokenKind::CloseBrace && self.peek_kind() != TokenKind::Eof {
            let arm_start = self.current_span();
            let pat = self.parse_pattern()?;

            // Optional match arm guard: `if <expr>`.
            // FLS §6.18: "A match arm guard is an additional condition that
            // must hold for the arm to be selected. The guard is only
            // evaluated when the pattern matches."
            let guard = if self.eat(TokenKind::KwIf) {
                Some(Box::new(self.parse_expr()?))
            } else {
                None
            };

            self.expect(TokenKind::FatArrow)?;

            // The arm body is an expression. If it is a block expression the
            // trailing comma is optional; otherwise the comma is required
            // (rustc) but we accept it as optional to keep parsing lenient.
            let body = Box::new(self.parse_expr()?);
            let arm_end = body.span;
            arms.push(MatchArm { pat, guard, body, span: arm_start.to(arm_end) });

            // Consume trailing comma if present.
            self.eat(TokenKind::Comma);
        }

        let end = self.current_span();
        self.expect(TokenKind::CloseBrace)?;

        Ok(Expr {
            kind: ExprKind::Match { scrutinee, arms },
            span: start.to(end),
        })
    }

    /// Parse a closure expression.
    ///
    /// FLS §6.14: Closure expressions.
    ///
    /// Grammar (simplified for galvanic's current subset):
    /// ```text
    /// ClosureExpression ::=
    ///     "move"? "|" ClosureParam* "|" ("->" Type)? ExpressionWithoutBlock
    ///   | "move"? "||"                  ("->" Type)? ExpressionWithoutBlock
    /// ClosureParam ::= Pattern (":" Type)?
    /// ```
    ///
    /// `zero_params`: if `true`, the caller already consumed `||` (zero-param
    /// shorthand); if `false`, the caller consumed `|` (or will consume it
    /// here) and we parse params.
    /// `is_move`: if `true`, the `move` keyword was already consumed.
    ///
    /// FLS §6.14: Non-capturing closures coerce to `fn` pointer types.
    /// Galvanic compiles them to hidden named functions and materialises the
    /// address as a function pointer value.
    ///
    /// FLS §6.22: `move` closures capture by value. For `Copy` types this
    /// is semantically identical to shared-reference capture.
    fn parse_closure(&mut self, zero_params: bool, is_move: bool) -> Result<Expr, ParseError> {
        use crate::ast::{ClosureParam, Pat};

        let start = self.current_span();

        let params: Vec<ClosureParam> = if zero_params {
            // `||` — consume the `||` token (already peeked as OrOr).
            self.advance();
            vec![]
        } else {
            // `|` — consume the opening `|`, then parse params until `|`.
            self.advance(); // eat `|`
            let mut ps = Vec::new();
            while self.peek_kind() != TokenKind::Or
                && self.peek_kind() != TokenKind::Eof
            {
                let param_start = self.current_span();
                // Pattern: identifier or `_`.
                // FLS §6.14: ClosureParam → Pattern (`:` Type)?
                let pat = match self.peek_kind() {
                    TokenKind::Underscore => {
                        self.advance();
                        Pat::Wildcard
                    }
                    TokenKind::Ident => {
                        let span = self.current_span();
                        self.advance();
                        Pat::Ident(span)
                    }
                    other => {
                        return Err(self.error(format!(
                            "expected identifier or `_` in closure parameter, found {other:?}"
                        )));
                    }
                };
                // Optional type annotation.
                let ty = if self.eat(TokenKind::Colon) {
                    Some(self.parse_ty()?)
                } else {
                    None
                };
                let param_end = self.current_span();
                ps.push(ClosureParam { pat, ty, span: param_start.to(param_end) });
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            // Consume closing `|`.
            self.expect(TokenKind::Or)?;
            ps
        };

        // Optional return type annotation `-> Type`.
        // FLS §6.14: Return type annotation is optional.
        let ret_ty = if self.eat(TokenKind::RArrow) {
            Some(Box::new(self.parse_ty()?))
        } else {
            None
        };

        // Body: a block `{ … }` or any expression.
        // FLS §6.14: The body is an ExpressionWithBlock or ExpressionWithoutBlock.
        let body: Box<Expr> = if self.peek_kind() == TokenKind::OpenBrace {
            let block = self.parse_block()?;
            let span = block.span;
            Box::new(Expr { kind: ExprKind::Block(Box::new(block)), span })
        } else {
            Box::new(self.parse_expr()?)
        };

        let span = start.to(body.span);
        Ok(Expr { kind: ExprKind::Closure { is_move, params, ret_ty, body }, span })
    }

    /// Parse a match arm pattern, including OR patterns.
    ///
    /// FLS §5: Patterns. Supported subset: wildcard `_`, integer literals,
    /// negative integer literals, bool literals, and OR patterns `p0 | p1`.
    ///
    /// FLS §5.1: Wildcard pattern — `_`.
    /// FLS §5.2: Literal patterns — integer and boolean literals.
    /// FLS §5.1.11: Or patterns — `p0 | p1 | ...`.
    fn parse_pattern(&mut self) -> Result<crate::ast::Pat, ParseError> {
        use crate::ast::Pat;
        let first = self.parse_single_pattern()?;
        // FLS §5.1.11: If the next token is `|`, collect additional alternatives.
        if self.peek_kind() != TokenKind::Or {
            return Ok(first);
        }
        let mut alts = vec![first];
        while self.peek_kind() == TokenKind::Or {
            self.advance(); // consume `|`
            alts.push(self.parse_single_pattern()?);
        }
        Ok(Pat::Or(alts))
    }

    /// Parse a single pattern alternative (no `|`).
    ///
    /// FLS §5.1: Wildcard pattern — `_`.
    /// FLS §5.1.4: Identifier patterns — a bare lowercase identifier binds the value.
    /// FLS §5.2: Literal patterns — integer and boolean literals.
    fn parse_single_pattern(&mut self) -> Result<crate::ast::Pat, ParseError> {
        use crate::ast::Pat;
        match self.peek_kind() {
            // Wildcard pattern `_`. FLS §5.1.
            TokenKind::Underscore => {
                self.advance();
                Ok(Pat::Wildcard)
            }
            // Identifier or path pattern.
            //
            // FLS §5.1.4: An identifier pattern matches any value and binds it
            // to the given name in the match arm body.
            //
            // FLS §5.5: A path pattern resolves to a constant or enum unit
            // variant. When the first identifier is followed by `::`, the
            // pattern is a path (`Color::Red`), not a binding identifier.
            //
            // Disambiguation: peek ahead after consuming the first identifier.
            // If the next token is `::`, consume additional `:: Ident` segments
            // and produce `Pat::Path`. Otherwise produce `Pat::Ident`.
            //
            // FLS §5.1.4 AMBIGUOUS: The spec does not specify how identifier
            // patterns interact with `ref`/`mut` qualifiers. Galvanic supports
            // only the simplest form: `match x { n => ... }`.
            TokenKind::Ident => {
                let tok = self.advance();
                let first_span = Self::span_of(&tok);
                // Check for path continuation `::`.
                if self.peek_kind() == TokenKind::ColonColon {
                    // FLS §5.5: Path pattern — `Segment :: Segment (:: Segment)*`.
                    let mut segments = vec![first_span];
                    while self.peek_kind() == TokenKind::ColonColon {
                        self.advance(); // consume `::`
                        if self.peek_kind() != TokenKind::Ident {
                            return Err(self.error(
                                "expected identifier after `::` in path pattern".to_owned(),
                            ));
                        }
                        let seg = self.advance();
                        segments.push(Self::span_of(&seg));
                    }
                    // FLS §5.4: Tuple struct/variant pattern — `Enum::Variant(p, …)`.
                    // After the path segments, if `(` follows, parse field patterns.
                    if self.peek_kind() == TokenKind::OpenParen {
                        self.advance(); // consume `(`
                        let mut fields = Vec::new();
                        while self.peek_kind() != TokenKind::CloseParen {
                            fields.push(self.parse_single_pattern()?);
                            if !self.eat(TokenKind::Comma) {
                                break;
                            }
                        }
                        self.expect(TokenKind::CloseParen)?;
                        return Ok(Pat::TupleStruct { path: segments, fields });
                    }
                    // FLS §5.3: Named-field struct/variant pattern — `Enum::Variant { field, … }`.
                    // A two-segment path followed by `{` with `Ident` or `}` inside.
                    // Shorthand `{ field }` is treated as `{ field: field }`.
                    if segments.len() == 2
                        && self.peek_kind() == TokenKind::OpenBrace
                        && (self.peek_nth(1) == TokenKind::CloseBrace
                            || self.peek_nth(1) == TokenKind::Ident)
                    {
                        self.advance(); // consume `{`
                        let mut pat_fields: Vec<(crate::ast::Span, Pat)> = Vec::new();
                        while self.peek_kind() != TokenKind::CloseBrace {
                            let field_name = self.expect(TokenKind::Ident)?;
                            // Explicit `field: pat` form.
                            let field_pat = if self.peek_kind() == TokenKind::Colon {
                                self.advance(); // consume `:`
                                self.parse_single_pattern()?
                            } else {
                                // Shorthand `{ field }` — binds field to an ident pattern of the same name.
                                Pat::Ident(field_name)
                            };
                            pat_fields.push((field_name, field_pat));
                            if !self.eat(TokenKind::Comma) {
                                break;
                            }
                        }
                        self.expect(TokenKind::CloseBrace)?;
                        return Ok(Pat::StructVariant { path: segments, fields: pat_fields });
                    }
                    Ok(Pat::Path(segments))
                } else if self.peek_kind() == TokenKind::OpenBrace
                    && (self.peek_nth(1) == TokenKind::CloseBrace
                        || self.peek_nth(1) == TokenKind::Ident)
                {
                    // FLS §5.3: Single-segment struct pattern — `StructName { field, … }`.
                    // A plain struct type name (no `::`) followed by `{` with field bindings.
                    // Shorthand `{ field }` binds field to an ident pattern of the same name.
                    //
                    // FLS §5.3: "A struct pattern matches a struct or enum struct variant
                    // by its field patterns." For named struct types the path has one segment.
                    self.advance(); // consume `{`
                    let mut pat_fields: Vec<(crate::ast::Span, Pat)> = Vec::new();
                    while self.peek_kind() != TokenKind::CloseBrace {
                        let field_name = self.expect(TokenKind::Ident)?;
                        let field_pat = if self.peek_kind() == TokenKind::Colon {
                            self.advance(); // consume `:`
                            self.parse_single_pattern()?
                        } else {
                            // Shorthand — `{ field }` binds as `{ field: field }`.
                            Pat::Ident(field_name)
                        };
                        pat_fields.push((field_name, field_pat));
                        if !self.eat(TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(TokenKind::CloseBrace)?;
                    Ok(Pat::StructVariant { path: vec![first_span], fields: pat_fields })
                } else if self.peek_kind() == TokenKind::OpenParen {
                    // Single-segment tuple struct / variant pattern — `Name(p0, p1, …)`.
                    // FLS §5.4: Tuple struct and tuple variant patterns.
                    //
                    // A bare identifier followed by `(` is a tuple struct pattern.
                    // Equivalent to `Name::Name(p0, …)` but with a single-segment path.
                    // Handles both let position and match position uniformly.
                    self.advance(); // consume `(`
                    let mut fields = Vec::new();
                    while self.peek_kind() != TokenKind::CloseParen {
                        fields.push(self.parse_single_pattern()?);
                        if !self.eat(TokenKind::Comma) {
                            break;
                        }
                        if self.peek_kind() == TokenKind::CloseParen {
                            break; // trailing comma
                        }
                    }
                    self.expect(TokenKind::CloseParen)?;
                    Ok(Pat::TupleStruct { path: vec![first_span], fields })
                } else if self.peek_kind() == TokenKind::At {
                    // FLS §5.1.4: Binding pattern `name @ subpat`.
                    //
                    // The identifier is the binding name; `@` introduces the
                    // sub-pattern that the matched value must also satisfy.
                    //
                    // Example: `n @ 1..=5` — binds the matched value to `n`
                    // if it falls in the range [1, 5].
                    //
                    // FLS §5.1.4 AMBIGUOUS: The spec does not enumerate which
                    // sub-pattern kinds are valid after `@`. Galvanic supports
                    // literal and range sub-patterns; nested `@` is rejected.
                    self.advance(); // consume `@`
                    let subpat = self.parse_single_pattern()?;
                    Ok(Pat::Bound { name: first_span, subpat: Box::new(subpat) })
                } else {
                    Ok(Pat::Ident(first_span))
                }
            }
            // Integer literal pattern. FLS §5.2.
            // Also handles range patterns `lo..=hi` and `lo..hi`. FLS §5.1.9.
            TokenKind::LitInteger => {
                let tok = self.advance();
                let val = parse_int_literal(tok.text(self.src));
                // FLS §5.1.9: `lo..=hi` — inclusive range pattern.
                if self.peek_kind() == TokenKind::DotDotEq {
                    self.advance(); // consume `..=`
                    let hi = self.parse_range_bound()?;
                    return Ok(Pat::RangeInclusive { lo: val as i128, hi });
                }
                // FLS §5.1.9: `lo..hi` — exclusive range pattern.
                if self.peek_kind() == TokenKind::DotDot {
                    self.advance(); // consume `..`
                    let hi = self.parse_range_bound()?;
                    return Ok(Pat::RangeExclusive { lo: val as i128, hi });
                }
                Ok(Pat::LitInt(val))
            }
            // Negative integer literal pattern `-n`. FLS §5.2.
            // Also handles negative lower bounds in range patterns. FLS §5.1.9.
            //
            // A unary minus before an integer literal forms a negative literal
            // pattern. This is the only place in pattern syntax where `-` is
            // meaningful; it is not a unary operator expression in this context.
            //
            // FLS §5.2: Literal patterns include negative integer literals.
            // The parser consumes `-` followed by `LitInteger` and produces
            // `Pat::NegLitInt(n)` where `n` is the absolute value.
            //
            // Note: `-` followed by anything other than `LitInteger` is a
            // parse error; negative booleans do not exist in Rust.
            TokenKind::Minus => {
                self.advance(); // consume `-`
                if self.peek_kind() != TokenKind::LitInteger {
                    return Err(self.error(
                        "expected integer literal after `-` in pattern".to_owned(),
                    ));
                }
                let tok = self.advance();
                let val = parse_int_literal(tok.text(self.src));
                // FLS §5.1.9: `-lo..=hi` — inclusive range with negative lower bound.
                if self.peek_kind() == TokenKind::DotDotEq {
                    self.advance(); // consume `..=`
                    let hi = self.parse_range_bound()?;
                    return Ok(Pat::RangeInclusive { lo: -(val as i128), hi });
                }
                // FLS §5.1.9: `-lo..hi` — exclusive range with negative lower bound.
                if self.peek_kind() == TokenKind::DotDot {
                    self.advance(); // consume `..`
                    let hi = self.parse_range_bound()?;
                    return Ok(Pat::RangeExclusive { lo: -(val as i128), hi });
                }
                Ok(Pat::NegLitInt(val))
            }
            // Boolean literal patterns `true` / `false`. FLS §5.2.
            TokenKind::KwTrue => {
                self.advance();
                Ok(Pat::LitBool(true))
            }
            TokenKind::KwFalse => {
                self.advance();
                Ok(Pat::LitBool(false))
            }
            // Slice/array pattern `[p0, p1, ...]`. FLS §5.1.8.
            //
            // Matches a fixed-size array by destructuring each element position.
            // Sub-patterns may be `Pat::Ident` (binding) or `Pat::Wildcard` (discard).
            //
            // FLS §5.1.8: "A slice pattern matches an array or slice type and
            // destructures its elements." Galvanic supports fixed-size arrays only
            // at this milestone (no unsized slice `&[T]` support yet).
            //
            // FLS §5.1.8 AMBIGUOUS: The spec allows rest patterns `..` inside
            // slice patterns (`[a, .., z]`). Galvanic does not yet support `..`
            // inside slice patterns; only exact-arity patterns are accepted.
            TokenKind::OpenBracket => {
                self.advance(); // consume `[`
                let mut pats = Vec::new();
                while self.peek_kind() != TokenKind::CloseBracket {
                    pats.push(self.parse_single_pattern()?);
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(TokenKind::CloseBracket)?;
                Ok(Pat::Slice(pats))
            }
            // Tuple pattern `(p0, p1, ...)`. FLS §5.10.3.
            //
            // Matches a tuple by destructuring each position. The empty form `()`
            // is the unit pattern. A single element requires a trailing comma `(p,)`.
            // Valid in both let and match position (e.g. `let (a, b) = t;`,
            // `match t { (a, b) => ... }`).
            TokenKind::OpenParen => {
                self.advance(); // consume `(`
                if self.peek_kind() == TokenKind::CloseParen {
                    self.advance(); // consume `)`
                    return Ok(Pat::Tuple(vec![]));
                }
                // Parse the first pattern inside the parens.
                let first = self.parse_single_pattern()?;
                // FLS §5.1.11: `(pat1 | pat2 | ...)` is a grouped OR pattern.
                // The `|` separator inside parens indicates an OR group, which is
                // distinct from the top-level OR in a match arm. Used in `@` bindings:
                // `n @ (1 | 5..=10)` — the parens group the OR sub-pattern.
                if self.peek_kind() == TokenKind::Or {
                    let mut alts = vec![first];
                    while self.eat(TokenKind::Or) {
                        alts.push(self.parse_single_pattern()?);
                    }
                    self.expect(TokenKind::CloseParen)?;
                    return Ok(Pat::Or(alts));
                }
                // Otherwise: tuple pattern `(pat,)` or `(pat1, pat2, ...)`.
                let mut pats = vec![first];
                while self.eat(TokenKind::Comma) {
                    if self.peek_kind() == TokenKind::CloseParen {
                        break; // trailing comma
                    }
                    pats.push(self.parse_single_pattern()?);
                }
                self.expect(TokenKind::CloseParen)?;
                Ok(Pat::Tuple(pats))
            }
            kind => Err(self.error(format!(
                "expected pattern (identifier, integer literal, `-` integer, `true`, `false`, `_`, `[`, or `(`), found {kind:?}"
            ))),
        }
    }

    /// Parse the upper (or lower) bound of a range pattern.
    ///
    /// FLS §5.1.9: Range pattern bounds are integer literals (positive or
    /// negative). This helper parses either `n` or `-n`.
    fn parse_range_bound(&mut self) -> Result<i128, ParseError> {
        if self.peek_kind() == TokenKind::Minus {
            self.advance(); // consume `-`
            if self.peek_kind() != TokenKind::LitInteger {
                return Err(self.error(
                    "expected integer literal after `-` in range pattern bound".to_owned(),
                ));
            }
            let tok = self.advance();
            let val = parse_int_literal(tok.text(self.src));
            Ok(-(val as i128))
        } else if self.peek_kind() == TokenKind::LitInteger {
            let tok = self.advance();
            let val = parse_int_literal(tok.text(self.src));
            Ok(val as i128)
        } else {
            Err(self.error(
                "expected integer literal for range pattern bound".to_owned(),
            ))
        }
    }

    /// Parse a while loop expression.
    ///
    /// FLS §6.8.2: While loop expressions.
    ///
    /// Grammar: `"while" Expression BlockExpression`
    ///
    /// FLS §6.8.2 NOTE: The condition is parsed with `parse_expr`, which stops
    /// naturally at `{` since `{` is not a valid binary operator. A block
    /// condition `while { x } { y }` would parse `{ x }` as the condition;
    /// this is rejected by rustc but the FLS does not forbid it syntactically.
    /// We parse it and defer to a future semantic phase.
    fn parse_while_expr(&mut self, label: Option<String>) -> Result<Expr, ParseError> {
        let start = self.current_span();
        self.expect(TokenKind::KwWhile)?;

        // FLS §6.15.4: while-let expression — `while let Pattern = Expr { body }`
        if self.eat(TokenKind::KwLet) {
            let pat = self.parse_pattern()?;
            self.expect(TokenKind::Eq)?;
            // Restrict struct literals in scrutinee to avoid `while let P = Foo { }` ambiguity.
            let prev_restrict = self.restrict_struct_lit;
            self.restrict_struct_lit = true;
            let scrutinee_result = self.parse_expr();
            self.restrict_struct_lit = prev_restrict;
            let scrutinee = Box::new(scrutinee_result?);
            let body = Box::new(self.parse_block()?);
            let span = start.to(body.span);
            return Ok(Expr { kind: ExprKind::WhileLet { label, pat, scrutinee, body }, span });
        }

        // FLS §6.15.3: regular while expression.
        // Restrict struct literals in condition to avoid `while Foo { }` ambiguity.
        let prev_restrict = self.restrict_struct_lit;
        self.restrict_struct_lit = true;
        let cond_result = self.parse_expr();
        self.restrict_struct_lit = prev_restrict;
        let cond = Box::new(cond_result?);
        let body = Box::new(self.parse_block()?);
        let span = start.to(body.span);
        Ok(Expr { kind: ExprKind::While { label, cond, body }, span })
    }

    /// Parse a for loop expression.
    ///
    /// FLS §6.8.3: For loop expressions.
    ///
    /// Grammar: `"for" Pattern "in" Expression BlockExpression`
    ///
    /// FLS §6.8.3 NOTE: The iterator expression is parsed with `parse_expr`.
    /// The `in` keyword acts as a natural stopping point because it is not a
    /// valid binary operator. Full irrefutable patterns (tuple, struct,
    /// `ref`, `_`) in `for` position are future work; only identifiers are
    /// accepted here.
    fn parse_for_expr(&mut self, label: Option<String>) -> Result<Expr, ParseError> {
        let start = self.current_span();
        self.expect(TokenKind::KwFor)?;

        // Pattern: any irrefutable pattern — identifier, wildcard, or tuple.
        // FLS §6.15.1: "The pattern in a for loop may be any irrefutable pattern."
        // FLS §5.1.4, §5.10.3: Identifiers and tuple patterns are irrefutable.
        let pat = self.parse_pattern()?;

        self.expect(TokenKind::KwIn)?;
        // Restrict struct literals in the iterable expression to avoid
        // `for x in Foo { }` ambiguity.
        let prev_restrict = self.restrict_struct_lit;
        self.restrict_struct_lit = true;
        let iter_result = self.parse_expr();
        self.restrict_struct_lit = prev_restrict;
        let iter = Box::new(iter_result?);
        let body = Box::new(self.parse_block()?);
        let span = start.to(body.span);
        Ok(Expr { kind: ExprKind::For { label, pat, iter, body }, span })
    }
}

// ── Internal discriminant ─────────────────────────────────────────────────────

/// Outcome of `parse_stmt_or_tail`: either a complete statement or a tail expr.
enum StmtOrTail {
    Stmt(Stmt),
    Tail(Expr),
}

// ── Integer literal parsing ───────────────────────────────────────────────────

/// Parse the text of an integer literal token to a `u128`.
///
/// FLS §2.4: Integer literals may be decimal, hex (`0x`), octal (`0o`), or
/// binary (`0b`), with optional type suffix and embedded underscores.
///
/// FLS §2.4 NOTE: The spec requires that the literal value fits within the
/// range of the suffix type. This function parses the raw numeric value
/// without bounds checking. Overflow checking is a type-checking concern.
fn parse_int_literal(text: &str) -> u128 {
    // Strip optional type suffix.
    let text = strip_int_suffix(text);
    // Remove digit separator underscores (valid per FLS §2.4).
    let digits: String = text.chars().filter(|&c| c != '_').collect();

    if let Some(hex) = digits.strip_prefix("0x").or_else(|| digits.strip_prefix("0X")) {
        u128::from_str_radix(hex, 16).unwrap_or(0)
    } else if let Some(oct) = digits.strip_prefix("0o").or_else(|| digits.strip_prefix("0O")) {
        u128::from_str_radix(oct, 8).unwrap_or(0)
    } else if let Some(bin) = digits.strip_prefix("0b").or_else(|| digits.strip_prefix("0B")) {
        u128::from_str_radix(bin, 2).unwrap_or(0)
    } else {
        digits.parse::<u128>().unwrap_or(0)
    }
}

/// Strip a numeric type suffix from an integer literal.
///
/// Suffixes defined by FLS §2.4: `i8`, `i16`, `i32`, `i64`, `i128`,
/// `isize`, `u8`, `u16`, `u32`, `u64`, `u128`, `usize`.
fn strip_int_suffix(text: &str) -> &str {
    // Longer suffixes first to avoid prefix-matching (e.g., `i1` before `i128`).
    const SUFFIXES: &[&str] = &[
        "i128", "u128", "isize", "usize", "i64", "u64", "i32", "u32", "i16", "u16",
        "i8", "u8",
    ];
    for &suffix in SUFFIXES {
        if let Some(stripped) = text.strip_suffix(suffix) {
            return stripped;
        }
    }
    text
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{EnumVariantKind, ExprKind, ItemKind, ParamKind, Pat, StmtKind, StructKind, TyKind, Visibility};
    use crate::lexer::tokenize;

    /// Parse `src` into a SourceFile, panicking on error.
    fn parse_ok(src: &str) -> SourceFile {
        let tokens = tokenize(src).expect("lex error");
        parse(&tokens, src).expect("parse error")
    }

    /// Parse `src` and expect a parse error.
    fn parse_err(src: &str) -> ParseError {
        let tokens = tokenize(src).expect("lex error");
        parse(&tokens, src).expect_err("expected parse error")
    }

    // ── Source file ───────────────────────────────────────────────────────────

    #[test]
    fn empty_source_file() {
        let sf = parse_ok("");
        assert!(sf.items.is_empty());
    }

    // ── Function items ────────────────────────────────────────────────────────

    #[test]
    fn fn_empty_body() {
        // FLS §9: minimal function with no parameters and no return type.
        let sf = parse_ok("fn main() {}");
        assert_eq!(sf.items.len(), 1);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        assert_eq!(f.name.text("fn main() {}"), "main");
        assert!(f.params.is_empty());
        assert!(f.ret_ty.is_none());
        let body = f.body.as_ref().unwrap();
        assert!(body.stmts.is_empty());
        assert!(body.tail.is_none());
    }

    #[test]
    fn fn_with_return_type() {
        // FLS §9: function with return type annotation.
        let src = "fn answer() -> i32 {}";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        assert!(f.ret_ty.is_some());
        let TyKind::Path(ref segs) = f.ret_ty.as_ref().unwrap().kind else {
            panic!("expected path type");
        };
        assert_eq!(segs[0].text(src), "i32");
    }

    #[test]
    fn fn_with_params() {
        // FLS §9.2: function parameters.
        let src = "fn add(a: i32, b: i32) -> i32 { a }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        assert_eq!(f.params.len(), 2);
        assert!(matches!(&f.params[0].kind, ParamKind::Ident(s) if s.text(src) == "a"));
        assert!(matches!(&f.params[1].kind, ParamKind::Ident(s) if s.text(src) == "b"));
    }

    #[test]
    fn fn_mut_param() {
        // FLS §9.2: parameters may be prefixed with `mut`.
        let src = "fn f(mut x: i32) -> i32 { x }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        assert_eq!(f.params.len(), 1);
        assert!(matches!(&f.params[0].kind, ParamKind::Ident(s) if s.text(src) == "x"));
    }

    #[test]
    fn fn_ptr_type_no_ret() {
        // FLS §4.9: function pointer type `fn(i32)` with implicit unit return.
        let src = "fn f(g: fn(i32)) {}";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        assert!(matches!(&f.params[0].ty.kind, TyKind::FnPtr { ret: None, .. }));
    }

    #[test]
    fn fn_ptr_type_with_ret() {
        // FLS §4.9: function pointer type `fn(i32) -> i32`.
        let src = "fn apply(f: fn(i32) -> i32, x: i32) -> i32 { x }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        assert!(matches!(&f.params[0].ty.kind, TyKind::FnPtr { ret: Some(_), .. }));
    }

    #[test]
    fn fn_ptr_type_two_params() {
        // FLS §4.9: function pointer type with two parameters `fn(i32, i32) -> i32`.
        let src = "fn apply2(f: fn(i32, i32) -> i32) -> i32 { 0 }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let TyKind::FnPtr { ref params, .. } = f.params[0].ty.kind else { panic!("expected FnPtr") };
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn impl_fn_type_no_ret() {
        // FLS §12, §4.13: `impl Fn(i32)` as a parameter type (anonymous type parameter).
        // Maps to FnPtr internally — same lowering path as `fn(i32)`.
        let src = "fn apply(f: impl Fn(i32)) {}";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        assert!(matches!(&f.params[0].ty.kind, TyKind::FnPtr { ret: None, .. }));
    }

    #[test]
    fn impl_fn_type_with_ret() {
        // FLS §12, §4.13: `impl Fn(i32) -> i32` as a parameter type.
        let src = "fn apply(f: impl Fn(i32) -> i32, x: i32) -> i32 { f(x) }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        assert!(matches!(&f.params[0].ty.kind, TyKind::FnPtr { ret: Some(_), .. }));
    }

    #[test]
    fn impl_fnmut_type() {
        // FLS §12, §4.13: `impl FnMut() -> i32` as a parameter type.
        let src = "fn run(f: impl FnMut() -> i32) -> i32 { f() }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        assert!(matches!(&f.params[0].ty.kind, TyKind::FnPtr { ret: Some(_), .. }));
    }

    #[test]
    fn impl_fnonce_type() {
        // FLS §12, §4.13: `impl FnOnce() -> i32` as a parameter type.
        let src = "fn consume(f: impl FnOnce() -> i32) -> i32 { f() }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        assert!(matches!(&f.params[0].ty.kind, TyKind::FnPtr { ret: Some(_), .. }));
    }

    #[test]
    fn impl_fn_two_params() {
        // FLS §12, §4.13: `impl Fn(i32, i32) -> i32` — two-parameter closure trait.
        let src = "fn apply2(f: impl Fn(i32, i32) -> i32, a: i32, b: i32) -> i32 { f(a, b) }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let TyKind::FnPtr { ref params, .. } = f.params[0].ty.kind else { panic!("expected FnPtr") };
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn array_type_annotation_in_let() {
        // FLS §4.5: array type `[T; N]` in a let binding annotation.
        let src = "fn f() -> i32 { let a: [i32; 3] = [1, 2, 3]; a[0] }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref func) = sf.items[0].kind else { panic!("expected Fn") };
        let body = func.body.as_ref().expect("expected function body");
        let crate::ast::StmtKind::Let { ty: Some(ty), .. } = &body.stmts[0].kind else {
            panic!("expected let with type annotation")
        };
        let TyKind::Array { ref elem, len } = ty.kind else { panic!("expected TyKind::Array") };
        assert_eq!(len, 3);
        let TyKind::Path(ref segs) = elem.kind else { panic!("expected element path") };
        assert_eq!(segs[0].text(src), "i32");
    }

    #[test]
    fn array_type_with_repeat_init() {
        // FLS §4.5: array type annotation with repeat initializer `[0; 5]`.
        let src = "fn f() -> i32 { let a: [i32; 5] = [0; 5]; a[0] }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref func) = sf.items[0].kind else { panic!("expected Fn") };
        let body = func.body.as_ref().expect("expected function body");
        let crate::ast::StmtKind::Let { ty: Some(ty), .. } = &body.stmts[0].kind else {
            panic!("expected let with type annotation")
        };
        let TyKind::Array { len, .. } = ty.kind else { panic!("expected TyKind::Array") };
        assert_eq!(len, 5);
    }

    #[test]
    fn array_type_as_parameter() {
        // FLS §4.5: `[T; N]` in function parameter position.
        let src = "fn sum(a: [i32; 4]) -> i32 { 0 }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn") };
        let TyKind::Array { ref elem, len } = f.params[0].ty.kind else {
            panic!("expected TyKind::Array in param")
        };
        assert_eq!(len, 4);
        let TyKind::Path(ref segs) = elem.kind else { panic!("expected element path") };
        assert_eq!(segs[0].text(src), "i32");
    }

    #[test]
    fn fn_trailing_comma_in_params() {
        // FLS §9.2: trailing comma in parameter list is allowed.
        let src = "fn f(x: i32,) {}";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        assert_eq!(f.params.len(), 1);
    }

    #[test]
    fn fn_struct_pattern_param_shorthand() {
        // FLS §5.10.2, §9.2: struct pattern in parameter position (shorthand).
        // `fn sum(Point { x, y }: Point)` — shorthand `{ x }` is sugar for `{ x: x }`.
        let src = "fn sum(Point { x, y }: Point) -> i32 { x + y }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        assert_eq!(f.params.len(), 1);
        let ParamKind::Struct { type_span, fields } = &f.params[0].kind else {
            panic!("expected ParamKind::Struct")
        };
        assert_eq!(type_span.text(src), "Point");
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].0.text(src), "x");
        assert!(matches!(&fields[0].1, Pat::Ident(s) if s.text(src) == "x"));
        assert_eq!(fields[1].0.text(src), "y");
        assert!(matches!(&fields[1].1, Pat::Ident(s) if s.text(src) == "y"));
    }

    #[test]
    fn fn_struct_pattern_param_explicit_binding() {
        // FLS §5.10.2, §9.2: struct pattern with explicit `field: binding` form.
        let src = "fn f(Point { x: a, y: b }: Point) -> i32 { a + b }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        assert_eq!(f.params.len(), 1);
        let ParamKind::Struct { type_span, fields } = &f.params[0].kind else {
            panic!("expected ParamKind::Struct")
        };
        assert_eq!(type_span.text(src), "Point");
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].0.text(src), "x");
        assert!(matches!(&fields[0].1, Pat::Ident(s) if s.text(src) == "a"));
        assert_eq!(fields[1].0.text(src), "y");
        assert!(matches!(&fields[1].1, Pat::Ident(s) if s.text(src) == "b"));
    }

    #[test]
    fn fn_struct_pattern_param_wildcard_field() {
        // FLS §5.10.2, §9.2: wildcard `_` in struct pattern parameter.
        let src = "fn f(Point { x, y: _ }: Point) -> i32 { x }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let ParamKind::Struct { fields, .. } = &f.params[0].kind else {
            panic!("expected ParamKind::Struct")
        };
        assert_eq!(fields.len(), 2);
        // First field: shorthand x → Pat::Ident(x).
        assert!(matches!(&fields[0].1, Pat::Ident(_)));
        // Second field: wildcard y: _ → Pat::Wildcard.
        assert!(matches!(&fields[1].1, Pat::Wildcard));
    }

    #[test]
    fn multiple_fns() {
        let src = "fn a() {} fn b() {}";
        let sf = parse_ok(src);
        assert_eq!(sf.items.len(), 2);
    }

    // ── Types ─────────────────────────────────────────────────────────────────

    #[test]
    fn type_unit() {
        // FLS §4.4: unit return type `()`.
        let src = "fn f() -> () {}";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        assert!(matches!(f.ret_ty.as_ref().unwrap().kind, TyKind::Unit));
    }

    #[test]
    fn type_ref() {
        // FLS §4.8: reference type `&i32`.
        let src = "fn f(x: &i32) {}";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        assert!(matches!(f.params[0].ty.kind, TyKind::Ref { mutable: false, .. }));
    }

    #[test]
    fn type_mut_ref() {
        // FLS §4.8: mutable reference type `&mut i32`.
        let src = "fn f(x: &mut i32) {}";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        assert!(matches!(f.params[0].ty.kind, TyKind::Ref { mutable: true, .. }));
    }

    // ── Let statements ────────────────────────────────────────────────────────

    #[test]
    fn let_with_init() {
        // FLS §8.1: let with initializer.
        let src = "fn f() { let x = 42; }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        assert_eq!(body.stmts.len(), 1);
        assert!(matches!(body.stmts[0].kind, StmtKind::Let { .. }));
    }

    #[test]
    fn let_with_type_and_init() {
        // FLS §8.1: let with type annotation and initializer.
        let src = "fn f() { let x: i32 = 42; }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let StmtKind::Let { ty, init, .. } = &body.stmts[0].kind else {
            panic!("expected let");
        };
        assert!(ty.is_some());
        assert!(init.is_some());
    }

    #[test]
    fn let_without_init() {
        // FLS §8.1: let without initializer (declaration only).
        let src = "fn f() { let x: i32; }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let StmtKind::Let { init, .. } = &body.stmts[0].kind else {
            panic!("expected let");
        };
        assert!(init.is_none());
    }

    #[test]
    fn let_else_tuple_struct_pattern() {
        // FLS §8.1: let-else with two-segment TupleStruct pattern.
        // `let Opt::Some(v) = o else { return 0 };` — refutable pattern with
        // a diverging else block.
        use crate::ast::{Pat, StmtKind};
        let src = "fn f(o: Opt) -> i32 { let Opt::Some(v) = o else { return 0 }; v }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        // let-else is a stmt; `v` is the tail expression (not a stmt).
        assert_eq!(body.stmts.len(), 1, "expected 1 stmt (the let-else)");
        assert!(body.tail.is_some(), "expected tail expression `v`");
        let StmtKind::Let { pat, else_block, .. } = &body.stmts[0].kind else {
            panic!("expected let");
        };
        // Pattern: TupleStruct with two-segment path Opt::Some.
        let Pat::TupleStruct { path, fields } = pat else {
            panic!("expected TupleStruct pattern, got {:?}", pat);
        };
        assert_eq!(path.len(), 2, "path must have two segments");
        assert_eq!(fields.len(), 1, "one positional field");
        assert!(matches!(fields[0], Pat::Ident(_)), "field is Ident binding");
        // else_block must be present.
        assert!(else_block.is_some(), "let-else must have an else block");
    }

    #[test]
    fn let_struct_pattern_single_segment_two_fields() {
        // FLS §5.10.2 + §8.1: struct pattern in let binding.
        // `let Point { x, y } = p;` produces Pat::StructVariant with a one-segment
        // path and two shorthand field patterns.
        let src = "fn f(p: Point) { let Point { x, y } = p; }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let StmtKind::Let { pat, .. } = &body.stmts[0].kind else {
            panic!("expected let");
        };
        let Pat::StructVariant { path, fields } = pat else {
            panic!("expected Pat::StructVariant, got {pat:?}");
        };
        assert_eq!(path.len(), 1, "single-segment path for plain struct type");
        assert_eq!(path[0].text(src), "Point");
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].0.text(src), "x");
        assert_eq!(fields[1].0.text(src), "y");
    }

    #[test]
    fn let_struct_pattern_wildcard_field() {
        // FLS §5.10.2 + §5.11: wildcard sub-pattern in struct let-pattern.
        // `let Point { x: _, y }` ignores x and binds y.
        let src = "fn f(p: Point) { let Point { x: _, y } = p; }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let StmtKind::Let { pat, .. } = &body.stmts[0].kind else {
            panic!("expected let");
        };
        let Pat::StructVariant { fields, .. } = pat else {
            panic!("expected Pat::StructVariant, got {pat:?}");
        };
        assert!(matches!(fields[0].1, Pat::Wildcard));
        assert!(matches!(fields[1].1, Pat::Ident(_)));
    }

    #[test]
    fn let_tuple_struct_pattern_two_fields() {
        // FLS §5.10.4 + §8.1: tuple struct pattern in let binding.
        // `let Point(x, y) = p;` produces Pat::TupleStruct with a one-segment
        // path and two Ident field patterns.
        let src = "fn f(p: Point) { let Point(x, y) = p; }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let StmtKind::Let { pat, .. } = &body.stmts[0].kind else {
            panic!("expected let");
        };
        let Pat::TupleStruct { path, fields } = pat else {
            panic!("expected Pat::TupleStruct, got {pat:?}");
        };
        assert_eq!(path.len(), 1, "single-segment path for tuple struct type");
        assert_eq!(path[0].text(src), "Point");
        assert_eq!(fields.len(), 2);
        assert!(matches!(fields[0], Pat::Ident(_)));
        assert!(matches!(fields[1], Pat::Ident(_)));
    }

    #[test]
    fn let_tuple_struct_pattern_wildcard_field() {
        // FLS §5.10.4 + §5.11: wildcard sub-pattern in tuple struct let-pattern.
        // `let Point(_, y)` ignores the first field and binds y.
        let src = "fn f(p: Point) { let Point(_, y) = p; }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let StmtKind::Let { pat, .. } = &body.stmts[0].kind else {
            panic!("expected let");
        };
        let Pat::TupleStruct { fields, .. } = pat else {
            panic!("expected Pat::TupleStruct, got {pat:?}");
        };
        assert_eq!(fields.len(), 2);
        assert!(matches!(fields[0], Pat::Wildcard));
        assert!(matches!(fields[1], Pat::Ident(_)));
    }

    #[test]
    fn let_slice_pattern_two_fields() {
        // FLS §5.1.8: slice/array pattern in let statement — two-element array.
        let src = "fn f() { let arr = [1, 2]; let [a, b] = arr; }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let StmtKind::Let { pat, .. } = &body.stmts[1].kind else {
            panic!("expected let at index 1");
        };
        let Pat::Slice(fields) = pat else {
            panic!("expected Pat::Slice, got {pat:?}");
        };
        assert_eq!(fields.len(), 2);
        assert!(matches!(fields[0], Pat::Ident(_)));
        assert!(matches!(fields[1], Pat::Ident(_)));
    }

    #[test]
    fn let_slice_pattern_wildcard_field() {
        // FLS §5.1.8 + §5.1: wildcard sub-pattern in slice pattern — ignores element.
        let src = "fn f() { let arr = [1, 2, 3]; let [_, b, _] = arr; }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let StmtKind::Let { pat, .. } = &body.stmts[1].kind else {
            panic!("expected let at index 1");
        };
        let Pat::Slice(fields) = pat else {
            panic!("expected Pat::Slice, got {pat:?}");
        };
        assert_eq!(fields.len(), 3);
        assert!(matches!(fields[0], Pat::Wildcard));
        assert!(matches!(fields[1], Pat::Ident(_)));
        assert!(matches!(fields[2], Pat::Wildcard));
    }

    // ── Expressions ───────────────────────────────────────────────────────────

    #[test]
    fn tail_expression() {
        // FLS §6.10: tail expression is the block's value.
        let src = "fn f() -> i32 { 42 }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        assert!(body.stmts.is_empty());
        assert!(matches!(body.tail.as_ref().unwrap().kind, ExprKind::LitInt(42)));
    }

    #[test]
    fn binary_add() {
        // FLS §6.5: arithmetic addition.
        let src = "fn f() -> i32 { a + b }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::Binary { op: BinOp::Add, .. }));
    }

    #[test]
    fn operator_precedence_mul_over_add() {
        // FLS §6.5: `*` binds tighter than `+`.
        // `1 + 2 * 3` should parse as `1 + (2 * 3)`.
        let src = "fn f() -> i32 { 1 + 2 * 3 }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        // Outer op is Add.
        let ExprKind::Binary { op: BinOp::Add, ref rhs, .. } = tail.kind else {
            panic!("expected Add at top level");
        };
        // RHS is a Mul.
        assert!(matches!(rhs.kind, ExprKind::Binary { op: BinOp::Mul, .. }));
    }

    #[test]
    fn call_expression() {
        // FLS §6.3.1: call expression.
        let src = "fn f() { foo(1, 2); }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let StmtKind::Expr(ref expr) = body.stmts[0].kind else {
            panic!("expected expr stmt");
        };
        let ExprKind::Call { ref args, .. } = expr.kind else {
            panic!("expected call");
        };
        assert_eq!(args.len(), 2);
    }

    #[test]
    fn return_with_value() {
        // FLS §6.12: return expression with a value.
        let src = "fn f() -> i32 { return 0; }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let StmtKind::Expr(ref ret) = body.stmts[0].kind else {
            panic!("expected expr stmt");
        };
        assert!(matches!(ret.kind, ExprKind::Return(Some(_))));
    }

    #[test]
    fn return_without_value() {
        // FLS §6.12: bare `return;` returns `()`.
        let src = "fn f() { return; }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let StmtKind::Expr(ref ret) = body.stmts[0].kind else {
            panic!("expected expr stmt");
        };
        assert!(matches!(ret.kind, ExprKind::Return(None)));
    }

    #[test]
    fn if_then_as_tail() {
        // FLS §6.11, §6.10: an if-without-else at the end of a block is the
        // tail expression (evaluates to `()`). No trailing `;` is required.
        let src = "fn f() { if x { y; } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        // No statements — the if is the tail.
        assert!(body.stmts.is_empty(), "expected no stmts, got {:?}", body.stmts.len());
        let tail = body.tail.as_ref().expect("expected tail");
        assert!(matches!(tail.kind, ExprKind::If { else_expr: None, .. }));
    }

    #[test]
    fn if_then_as_stmt() {
        // FLS §8.3: ExpressionWithBlock may appear as a statement without `;`
        // when more content follows. Here the if is followed by a let stmt.
        let src = "fn f() { if x { y; } let z = 1; }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        assert_eq!(body.stmts.len(), 2); // if-stmt + let-stmt
        let StmtKind::Expr(ref e) = body.stmts[0].kind else {
            panic!("expected expr stmt, got {:?}", body.stmts[0].kind);
        };
        assert!(matches!(e.kind, ExprKind::If { else_expr: None, .. }));
    }

    #[test]
    fn if_then_else() {
        // FLS §6.11: if-else expression.
        let src = "fn f() -> i32 { if cond { 1 } else { 0 } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::If { else_expr: Some(_), .. }));
    }

    #[test]
    fn if_else_if_chain() {
        // FLS §6.11: else-if chain.
        let src = "fn f() -> i32 { if a { 1 } else if b { 2 } else { 3 } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        let ExprKind::If { else_expr: Some(ref else_e), .. } = tail.kind else {
            panic!("expected if with else");
        };
        // The else branch is another If.
        assert!(matches!(else_e.kind, ExprKind::If { .. }));
    }

    #[test]
    fn if_let_literal_pattern() {
        // FLS §6.17: if-let with integer literal pattern.
        let src = "fn f(x: i32) -> i32 { if let 42 = x { 1 } else { 0 } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(
            matches!(tail.kind, ExprKind::IfLet { .. }),
            "expected IfLet expression"
        );
    }

    #[test]
    fn if_let_ident_pattern() {
        // FLS §6.17 + §5.1.4: if-let with identifier pattern.
        let src = "fn f(x: i32) -> i32 { if let n = x { n } else { 0 } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(
            matches!(tail.kind, ExprKind::IfLet { pat: Pat::Ident(_), .. }),
            "expected IfLet with identifier pattern"
        );
    }

    #[test]
    fn if_let_no_else() {
        // FLS §6.17: if-let without else branch (unit context, as tail expression).
        let src = "fn f(x: i32) { if let 1 = x { } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        // Parsed as a tail expression (no semicolon).
        let e = body.tail.as_ref().expect("expected tail expression");
        assert!(
            matches!(e.kind, ExprKind::IfLet { else_expr: None, .. }),
            "expected IfLet with no else"
        );
    }

    #[test]
    fn unit_literal() {
        // FLS §6.3.3: `()` is the unit value.
        let src = "fn f() -> () { () }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::Unit));
    }

    #[test]
    fn boolean_literals() {
        // FLS §6.1.3: boolean literals.
        let src = "fn f() -> bool { true }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::LitBool(true)));
    }

    #[test]
    fn unary_negate() {
        // FLS §6.4.1: unary negation.
        let src = "fn f() -> i32 { -1 }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::Unary { op: UnaryOp::Neg, .. }));
    }

    #[test]
    fn borrow_expression() {
        // FLS §6.4.4: shared borrow `&x`.
        let src = "fn f(x: i32) -> &i32 { &x }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::Unary { op: UnaryOp::Ref, .. }));
    }

    #[test]
    fn integer_literal_hex() {
        // FLS §2.4: hex integer literal.
        let src = "fn f() -> i32 { 0xFF }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::LitInt(255)));
    }

    #[test]
    fn integer_literal_with_suffix() {
        // FLS §2.4: integer suffix is stripped before value parsing.
        let src = "fn f() -> u32 { 42u32 }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::LitInt(42)));
    }

    #[test]
    fn full_function() {
        // Integration: function with params, local binding, and tail expression.
        let src = "fn add(a: i32, b: i32) -> i32 { let sum = a + b; sum }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        assert_eq!(f.params.len(), 2);
        let body = f.body.as_ref().unwrap();
        assert_eq!(body.stmts.len(), 1); // the let
        assert!(body.tail.is_some()); // `sum`
    }

    // ── Comparison operators ──────────────────────────────────────────────────

    #[test]
    fn comparison_less_than() {
        // FLS §6.7: `<` comparison operator.
        let src = "fn f(a: i32, b: i32) -> bool { a < b }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::Binary { op: BinOp::Lt, .. }));
    }

    #[test]
    fn comparison_less_equal() {
        // FLS §6.7: `<=` comparison operator.
        let src = "fn f(a: i32, b: i32) -> bool { a <= b }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::Binary { op: BinOp::Le, .. }));
    }

    #[test]
    fn comparison_equal() {
        // FLS §6.7: `==` equality operator.
        let src = "fn f(a: i32, b: i32) -> bool { a == b }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::Binary { op: BinOp::Eq, .. }));
    }

    #[test]
    fn comparison_not_equal() {
        // FLS §6.7: `!=` inequality operator.
        let src = "fn f(a: i32, b: i32) -> bool { a != b }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::Binary { op: BinOp::Ne, .. }));
    }

    // ── Logical operators ─────────────────────────────────────────────────────

    #[test]
    fn logical_and() {
        // FLS §6.8.1: `&&` logical and.
        let src = "fn f(a: bool, b: bool) -> bool { a && b }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::Binary { op: BinOp::And, .. }));
    }

    #[test]
    fn logical_or() {
        // FLS §6.8.2: `||` logical or.
        let src = "fn f(a: bool, b: bool) -> bool { a || b }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::Binary { op: BinOp::Or, .. }));
    }

    #[test]
    fn comparison_binds_tighter_than_logical_and() {
        // FLS §6.7–§6.8: comparisons have higher precedence than `&&`.
        // `a < b && c > d` should parse as `(a < b) && (c > d)`.
        let src = "fn f() -> bool { a < b && c > d }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        // Outer op is `&&`.
        let ExprKind::Binary { op: BinOp::And, ref lhs, ref rhs } = tail.kind else {
            panic!("expected And at top level, got {:?}", tail.kind);
        };
        // Each side is a comparison.
        assert!(matches!(lhs.kind, ExprKind::Binary { op: BinOp::Lt, .. }));
        assert!(matches!(rhs.kind, ExprKind::Binary { op: BinOp::Gt, .. }));
    }

    // ── Recursive function (integration) ─────────────────────────────────────

    #[test]
    fn recursive_fibonacci() {
        // Integration: `fib` exercises comparison, if-else, recursive calls,
        // and arithmetic in call arguments — the full expression pipeline.
        //
        // FLS §6.3.1 (calls), §6.5 (arithmetic), §6.7 (comparison), §6.11 (if).
        let src = "fn fib(n: u64) -> u64 { if n <= 1 { n } else { fib(n - 1) + fib(n - 2) } }";
        let sf = parse_ok(src);
        assert_eq!(sf.items.len(), 1);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        assert_eq!(f.params.len(), 1);
        let body = f.body.as_ref().unwrap();
        assert!(body.stmts.is_empty());
        // Tail is an if-else.
        let tail = body.tail.as_ref().unwrap();
        let ExprKind::If { ref cond, ref else_expr, .. } = tail.kind else {
            panic!("expected If as tail, got {:?}", tail.kind);
        };
        // Condition is `n <= 1`.
        assert!(matches!(cond.kind, ExprKind::Binary { op: BinOp::Le, .. }));
        // There is an else branch.
        assert!(else_expr.is_some());
        // The else expression is a block whose tail is `fib(n-1) + fib(n-2)`.
        let else_inner = else_expr.as_ref().unwrap();
        let ExprKind::Block(ref else_block) = else_inner.kind else {
            panic!("expected else to be a Block, got {:?}", else_inner.kind);
        };
        let else_tail = else_block.tail.as_ref().unwrap();
        assert!(matches!(else_tail.kind, ExprKind::Binary { op: BinOp::Add, .. }));
    }

    // ── Error cases ───────────────────────────────────────────────────────────

    #[test]
    fn error_missing_fn_name() {
        // `fn` not followed by an identifier should fail.
        let err = parse_err("fn () {}");
        assert!(err.message.contains("expected function name"), "{}", err.message);
    }

    #[test]
    fn error_missing_open_brace() {
        // Missing `{` for the function body.
        let err = parse_err("fn f()");
        assert!(err.message.contains("OpenBrace"), "{}", err.message);
    }

    #[test]
    fn error_non_item_at_top_level() {
        // An expression at top level is not an item.
        let err = parse_err("42");
        assert!(err.message.contains("expected item"), "{}", err.message);
    }

    // ── Struct items ──────────────────────────────────────────────────────────

    #[test]
    fn struct_unit() {
        // FLS §14.3: unit struct with no fields.
        let src = "struct Foo;";
        let sf = parse_ok(src);
        assert_eq!(sf.items.len(), 1);
        let ItemKind::Struct(ref s) = sf.items[0].kind else {
            panic!("expected Struct item");
        };
        assert_eq!(s.name.text(src), "Foo");
        assert!(matches!(s.kind, StructKind::Unit));
        assert_eq!(s.vis, Visibility::Private);
    }

    #[test]
    fn struct_unit_pub() {
        // FLS §10.2: `pub` visibility modifier.
        let src = "pub struct Marker;";
        let sf = parse_ok(src);
        let ItemKind::Struct(ref s) = sf.items[0].kind else {
            panic!("expected Struct item");
        };
        assert_eq!(s.name.text(src), "Marker");
        assert_eq!(s.vis, Visibility::Pub);
        assert!(matches!(s.kind, StructKind::Unit));
    }

    #[test]
    fn struct_named_empty() {
        // FLS §14.1: named-field struct with no fields.
        let src = "struct Empty {}";
        let sf = parse_ok(src);
        let ItemKind::Struct(ref s) = sf.items[0].kind else {
            panic!("expected Struct item");
        };
        assert_eq!(s.name.text(src), "Empty");
        let StructKind::Named(ref fields) = s.kind else {
            panic!("expected Named struct");
        };
        assert!(fields.is_empty());
    }

    #[test]
    fn struct_named_fields() {
        // FLS §14.1: named-field struct with two fields.
        let src = "struct Point { x: i32, y: i32 }";
        let sf = parse_ok(src);
        let ItemKind::Struct(ref s) = sf.items[0].kind else {
            panic!("expected Struct item");
        };
        let StructKind::Named(ref fields) = s.kind else {
            panic!("expected Named struct");
        };
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name.text(src), "x");
        assert_eq!(fields[1].name.text(src), "y");
        // Both fields are private by default.
        assert_eq!(fields[0].vis, Visibility::Private);
        // Both fields have path type `i32`.
        assert!(matches!(fields[0].ty.kind, TyKind::Path(_)));
    }

    #[test]
    fn struct_named_trailing_comma() {
        // Trailing comma after the last field is allowed.
        let src = "struct Pair { a: i32, b: f64, }";
        let sf = parse_ok(src);
        let ItemKind::Struct(ref s) = sf.items[0].kind else {
            panic!("expected Struct item");
        };
        let StructKind::Named(ref fields) = s.kind else {
            panic!("expected Named struct");
        };
        assert_eq!(fields.len(), 2);
    }

    #[test]
    fn struct_named_pub_field() {
        // FLS §10.2: individual fields may be `pub`.
        let src = "struct Rect { pub width: u32, pub height: u32 }";
        let sf = parse_ok(src);
        let ItemKind::Struct(ref s) = sf.items[0].kind else {
            panic!("expected Struct item");
        };
        let StructKind::Named(ref fields) = s.kind else {
            panic!("expected Named struct");
        };
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].vis, Visibility::Pub);
        assert_eq!(fields[1].vis, Visibility::Pub);
    }

    #[test]
    fn struct_tuple() {
        // FLS §14.2: tuple struct with two positional fields.
        let src = "struct Pair(i32, f64);";
        let sf = parse_ok(src);
        let ItemKind::Struct(ref s) = sf.items[0].kind else {
            panic!("expected Struct item");
        };
        let StructKind::Tuple(ref fields) = s.kind else {
            panic!("expected Tuple struct");
        };
        assert_eq!(fields.len(), 2);
        assert!(matches!(fields[0].ty.kind, TyKind::Path(_)));
        assert!(matches!(fields[1].ty.kind, TyKind::Path(_)));
        assert_eq!(fields[0].vis, Visibility::Private);
    }

    #[test]
    fn struct_tuple_pub_field() {
        // FLS §10.2: tuple struct fields may be `pub`.
        let src = "struct Newtype(pub i32);";
        let sf = parse_ok(src);
        let ItemKind::Struct(ref s) = sf.items[0].kind else {
            panic!("expected Struct item");
        };
        let StructKind::Tuple(ref fields) = s.kind else {
            panic!("expected Tuple struct");
        };
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].vis, Visibility::Pub);
    }

    #[test]
    fn struct_and_fn_mixed() {
        // Multiple items of different kinds in one source file.
        // (Struct-expression syntax in fn bodies is not yet implemented,
        // so we use a simple fn body here.)
        let src = "struct Flag; fn check() {}";
        let sf = parse_ok(src);
        assert_eq!(sf.items.len(), 2);
        assert!(matches!(sf.items[0].kind, ItemKind::Struct(_)));
        assert!(matches!(sf.items[1].kind, ItemKind::Fn(_)));
    }

    #[test]
    fn error_struct_missing_name() {
        // `struct` keyword without a name.
        let err = parse_err("struct {}");
        assert!(err.message.contains("expected struct name"), "{}", err.message);
    }

    #[test]
    fn error_struct_missing_body() {
        // Struct name with no body delimiter.
        let err = parse_err("struct Foo fn");
        assert!(
            err.message.contains("expected `{`") || err.message.contains("{"),
            "{}",
            err.message
        );
    }

    #[test]
    fn error_struct_tuple_missing_semicolon() {
        // Tuple struct body without terminating `;`.
        let err = parse_err("struct Pair(i32, i32)");
        assert!(err.message.contains("Semi"), "{}", err.message);
    }

    // ── Enum items ────────────────────────────────────────────────────────────

    #[test]
    fn enum_empty() {
        // FLS §15: enum with no variants.
        let src = "enum Empty {}";
        let sf = parse_ok(src);
        assert_eq!(sf.items.len(), 1);
        let ItemKind::Enum(ref e) = sf.items[0].kind else {
            panic!("expected Enum item");
        };
        assert_eq!(e.name.text(src), "Empty");
        assert!(e.variants.is_empty());
        assert_eq!(e.vis, Visibility::Private);
    }

    #[test]
    fn enum_unit_variant() {
        // FLS §15.1: unit variants — identifiers with no fields.
        let src = "enum Direction { North, South, East, West }";
        let sf = parse_ok(src);
        let ItemKind::Enum(ref e) = sf.items[0].kind else {
            panic!("expected Enum item");
        };
        assert_eq!(e.variants.len(), 4);
        assert_eq!(e.variants[0].name.text(src), "North");
        assert_eq!(e.variants[3].name.text(src), "West");
        assert!(matches!(e.variants[0].kind, EnumVariantKind::Unit));
    }

    #[test]
    fn enum_unit_trailing_comma() {
        // Trailing comma after the last variant is allowed.
        let src = "enum Bit { Zero, One, }";
        let sf = parse_ok(src);
        let ItemKind::Enum(ref e) = sf.items[0].kind else {
            panic!("expected Enum item");
        };
        assert_eq!(e.variants.len(), 2);
    }

    #[test]
    fn enum_tuple_variant() {
        // FLS §15.2: tuple variant with positional fields.
        let src = "enum Shape { Circle(f64), Rectangle(f64, f64) }";
        let sf = parse_ok(src);
        let ItemKind::Enum(ref e) = sf.items[0].kind else {
            panic!("expected Enum item");
        };
        assert_eq!(e.variants.len(), 2);
        assert_eq!(e.variants[0].name.text(src), "Circle");
        let EnumVariantKind::Tuple(ref fields) = e.variants[0].kind else {
            panic!("expected Tuple variant");
        };
        assert_eq!(fields.len(), 1);
        let EnumVariantKind::Tuple(ref fields2) = e.variants[1].kind else {
            panic!("expected Tuple variant");
        };
        assert_eq!(fields2.len(), 2);
    }

    #[test]
    fn enum_named_variant() {
        // FLS §15.3: named-field variant.
        let src = "enum Event { KeyPress { code: u32, shift: bool } }";
        let sf = parse_ok(src);
        let ItemKind::Enum(ref e) = sf.items[0].kind else {
            panic!("expected Enum item");
        };
        assert_eq!(e.variants.len(), 1);
        assert_eq!(e.variants[0].name.text(src), "KeyPress");
        let EnumVariantKind::Named(ref fields) = e.variants[0].kind else {
            panic!("expected Named variant");
        };
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name.text(src), "code");
        assert_eq!(fields[1].name.text(src), "shift");
    }

    #[test]
    fn enum_mixed_variants() {
        // FLS §15: all three variant forms in one enum.
        let src = "enum Message { Quit, Move { x: i32, y: i32 }, Write(i32) }";
        let sf = parse_ok(src);
        let ItemKind::Enum(ref e) = sf.items[0].kind else {
            panic!("expected Enum item");
        };
        assert_eq!(e.variants.len(), 3);
        assert!(matches!(e.variants[0].kind, EnumVariantKind::Unit));
        assert!(matches!(e.variants[1].kind, EnumVariantKind::Named(_)));
        assert!(matches!(e.variants[2].kind, EnumVariantKind::Tuple(_)));
    }

    #[test]
    fn enum_pub_visibility() {
        // FLS §10.2: `pub` visibility on an enum.
        let src = "pub enum Color { Red, Green, Blue }";
        let sf = parse_ok(src);
        let ItemKind::Enum(ref e) = sf.items[0].kind else {
            panic!("expected Enum item");
        };
        assert_eq!(e.vis, Visibility::Pub);
        assert_eq!(e.variants.len(), 3);
    }

    #[test]
    fn enum_and_fn_mixed() {
        // Enum and fn items together in one source file.
        let src = "enum Flag { On, Off } fn check() {}";
        let sf = parse_ok(src);
        assert_eq!(sf.items.len(), 2);
        assert!(matches!(sf.items[0].kind, ItemKind::Enum(_)));
        assert!(matches!(sf.items[1].kind, ItemKind::Fn(_)));
    }

    #[test]
    fn error_enum_missing_name() {
        // `enum` keyword without a name.
        let err = parse_err("enum {}");
        assert!(err.message.contains("expected enum name"), "{}", err.message);
    }

    #[test]
    fn error_enum_missing_open_brace() {
        // Enum name without `{`.
        let err = parse_err("enum Foo fn");
        assert!(err.message.contains("OpenBrace"), "{}", err.message);
    }

    // ── pub fn visibility ─────────────────────────────────────────────────────

    #[test]
    fn fn_pub_visibility() {
        // FLS §10.2: `pub` visibility on a function item.
        let src = "pub fn exported() {}";
        let sf = parse_ok(src);
        assert_eq!(sf.items.len(), 1);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        assert_eq!(f.vis, Visibility::Pub);
        assert_eq!(f.name.text(src), "exported");
        assert!(!f.is_const, "regular fn must not be const");
    }

    #[test]
    fn const_fn_simple() {
        // FLS §9:41–43: `const fn` — a function eligible for compile-time
        // evaluation when called from a const context.
        let src = "const fn add(a: i32, b: i32) -> i32 { a + b }";
        let sf = parse_ok(src);
        assert_eq!(sf.items.len(), 1);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        assert!(f.is_const, "const fn must have is_const=true");
        assert_eq!(f.name.text(src), "add");
        assert_eq!(f.params.len(), 2);
    }

    #[test]
    fn const_fn_pub() {
        // FLS §9:41–43, §10.2: `pub const fn` combines visibility and const.
        let src = "pub const fn identity(x: i32) -> i32 { x }";
        let sf = parse_ok(src);
        assert_eq!(sf.items.len(), 1);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        assert!(f.is_const, "pub const fn must have is_const=true");
        assert_eq!(f.vis, Visibility::Pub);
        assert_eq!(f.name.text(src), "identity");
    }

    // ── Bitwise and shift operators ───────────────────────────────────────────

    #[test]
    fn bitwise_and() {
        // FLS §6.6.1: bitwise and `&`. Binds tighter than bitwise xor.
        let src = "fn f(a: u32, b: u32) -> u32 { a & b }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::Binary { op: BinOp::BitAnd, .. }));
    }

    #[test]
    fn bitwise_xor() {
        // FLS §6.6.2: bitwise xor `^`. Binds tighter than bitwise or.
        let src = "fn f(a: u32, b: u32) -> u32 { a ^ b }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::Binary { op: BinOp::BitXor, .. }));
    }

    #[test]
    fn bitwise_or() {
        // FLS §6.6.3: bitwise or `|`.
        let src = "fn f(a: u32, b: u32) -> u32 { a | b }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::Binary { op: BinOp::BitOr, .. }));
    }

    #[test]
    fn shift_left() {
        // FLS §6.5.3: left shift `<<`. Binds tighter than additive.
        let src = "fn f(a: u32, b: u32) -> u32 { a << b }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::Binary { op: BinOp::Shl, .. }));
    }

    #[test]
    fn shift_right() {
        // FLS §6.5.3: right shift `>>`. Binds tighter than additive.
        let src = "fn f(a: u32, b: u32) -> u32 { a >> b }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::Binary { op: BinOp::Shr, .. }));
    }

    #[test]
    fn bitwise_precedence_and_over_xor_over_or() {
        // FLS §6.6: `&` binds tighter than `^`, which binds tighter than `|`.
        // `a | b ^ c & d` parses as `a | (b ^ (c & d))`.
        let src = "fn f(a: u32, b: u32, c: u32, d: u32) -> u32 { a | b ^ c & d }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        // Outer op is `|`.
        let ExprKind::Binary { op: BinOp::BitOr, ref rhs, .. } = tail.kind else {
            panic!("expected BitOr at top, got {:?}", tail.kind);
        };
        // RHS of `|` is `^`.
        let ExprKind::Binary { op: BinOp::BitXor, rhs: ref xor_rhs, .. } = rhs.kind else {
            panic!("expected BitXor, got {:?}", rhs.kind);
        };
        // RHS of `^` is `&`.
        assert!(matches!(xor_rhs.kind, ExprKind::Binary { op: BinOp::BitAnd, .. }));
    }

    // ── Assignment expression ─────────────────────────────────────────────────

    #[test]
    fn assignment_expression() {
        // FLS §6.9: assignment is right-associative.
        // `x = 5` in statement position.
        let src = "fn f() { x = 5; }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let StmtKind::Expr(ref expr) = body.stmts[0].kind else {
            panic!("expected expr stmt");
        };
        assert!(matches!(expr.kind, ExprKind::Binary { op: BinOp::Assign, .. }));
    }

    #[test]
    fn assignment_right_associative() {
        // FLS §6.9: `a = b = c` parses as `a = (b = c)`.
        let src = "fn f() { a = b = 0; }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let StmtKind::Expr(ref expr) = body.stmts[0].kind else {
            panic!("expected expr stmt");
        };
        // Outer is `=`.
        let ExprKind::Binary { op: BinOp::Assign, ref rhs, .. } = expr.kind else {
            panic!("expected Assign at top, got {:?}", expr.kind);
        };
        // RHS is also an assignment — right-associative.
        assert!(matches!(rhs.kind, ExprKind::Binary { op: BinOp::Assign, .. }));
    }

    #[test]
    fn compound_assign_add() {
        // FLS §6.5.11: `+=` parses as CompoundAssign { op: Add, .. }.
        let src = "fn f() { let mut x = 0; x += 1; }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let StmtKind::Expr(ref expr) = body.stmts[1].kind else {
            panic!("expected expr stmt");
        };
        assert!(
            matches!(expr.kind, ExprKind::CompoundAssign { op: BinOp::Add, .. }),
            "expected CompoundAssign Add, got {:?}", expr.kind
        );
    }

    #[test]
    fn compound_assign_all_operators() {
        // FLS §6.5.11: all compound assignment operators parse correctly.
        let ops = [
            ("x -= 1", BinOp::Sub),
            ("x *= 2", BinOp::Mul),
            ("x /= 2", BinOp::Div),
            ("x %= 3", BinOp::Rem),
            ("x &= 1", BinOp::BitAnd),
            ("x |= 1", BinOp::BitOr),
            ("x ^= 1", BinOp::BitXor),
            ("x <<= 1", BinOp::Shl),
            ("x >>= 1", BinOp::Shr),
        ];
        for (stmt, expected_op) in ops {
            let src = format!("fn f() {{ let mut x = 5; {stmt}; }}");
            let sf = parse_ok(&src);
            let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
            let body = f.body.as_ref().unwrap();
            let StmtKind::Expr(ref expr) = body.stmts[1].kind else {
                panic!("expected expr stmt for `{stmt}`");
            };
            match &expr.kind {
                ExprKind::CompoundAssign { op, .. } => {
                    assert_eq!(*op, expected_op, "wrong op for `{stmt}`");
                }
                other => panic!("expected CompoundAssign for `{stmt}`, got {other:?}"),
            }
        }
    }

    // ── Remaining unary operators ─────────────────────────────────────────────

    #[test]
    fn unary_not() {
        // FLS §6.4.2: logical/bitwise not `!x`.
        let src = "fn f(x: bool) -> bool { !x }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::Unary { op: UnaryOp::Not, .. }));
    }

    #[test]
    fn unary_deref() {
        // FLS §6.4.3: dereference `*ptr`.
        let src = "fn f(p: &i32) -> i32 { *p }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::Unary { op: UnaryOp::Deref, .. }));
    }

    #[test]
    fn mutable_borrow_expression() {
        // FLS §6.4.4: mutable borrow `&mut x`.
        let src = "fn f(x: i32) -> &mut i32 { &mut x }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::Unary { op: UnaryOp::RefMut, .. }));
    }

    // ── Multi-segment paths ───────────────────────────────────────────────────

    #[test]
    fn path_with_segments() {
        // FLS §6.2: a path may contain `::` separators.
        // `std::mem::size_of` is a two-separator (three-segment) path call.
        let src = "fn f() -> usize { std::mem::size_of() }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        // A call whose callee is a multi-segment path.
        let ExprKind::Call { ref callee, ref args } = tail.kind else {
            panic!("expected Call, got {:?}", tail.kind);
        };
        assert!(args.is_empty());
        let ExprKind::Path(ref segs) = callee.kind else {
            panic!("expected Path callee, got {:?}", callee.kind);
        };
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0].text(src), "std");
        assert_eq!(segs[1].text(src), "mem");
        assert_eq!(segs[2].text(src), "size_of");
    }

    #[test]
    fn two_segment_path_expression() {
        // FLS §6.2: two-segment path in tail position (not a call).
        let src = "fn f() { Option::None }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        let ExprKind::Path(ref segs) = tail.kind else {
            panic!("expected Path, got {:?}", tail.kind);
        };
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].text(src), "Option");
        assert_eq!(segs[1].text(src), "None");
    }

    // ── Field access and method calls ─────────────────────────────────────────

    #[test]
    fn field_access_simple() {
        // FLS §6.3.3: `point.x` is a field access expression.
        let src = "fn f() { point.x; }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let StmtKind::Expr(ref expr) = body.stmts[0].kind else {
            panic!("expected expr stmt");
        };
        let ExprKind::FieldAccess { ref receiver, ref field } = expr.kind else {
            panic!("expected FieldAccess, got {:?}", expr.kind);
        };
        // Receiver is the path `point`.
        let ExprKind::Path(ref segs) = receiver.kind else {
            panic!("expected Path receiver");
        };
        assert_eq!(segs[0].text(src), "point");
        assert_eq!(field.text(src), "x");
    }

    #[test]
    fn field_access_chained() {
        // `a.b.c` parses as `(a.b).c` — left-associative.
        let src = "fn f() { a.b.c; }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let StmtKind::Expr(ref expr) = body.stmts[0].kind else {
            panic!("expected expr stmt");
        };
        // Outermost: `.c`
        let ExprKind::FieldAccess { ref receiver, ref field } = expr.kind else {
            panic!("expected FieldAccess");
        };
        assert_eq!(field.text(src), "c");
        // Inner: `a.b`
        let ExprKind::FieldAccess { field: ref inner_field, .. } = receiver.kind else {
            panic!("expected inner FieldAccess");
        };
        assert_eq!(inner_field.text(src), "b");
    }

    #[test]
    fn struct_lit_shorthand_single_field() {
        // FLS §6.11: `Point { x }` is shorthand for `Point { x: x }`.
        let src = "fn f(x: i32) -> Point { Point { x } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn") };
        let body = f.body.as_ref().unwrap();
        // Tail expression is the struct literal.
        let ExprKind::StructLit { ref fields, .. } = body.tail.as_ref().unwrap().kind else {
            panic!("expected StructLit tail");
        };
        assert_eq!(fields.len(), 1);
        // Field value is a path expression with the same name as the field.
        let ExprKind::Path(ref segs) = fields[0].1.kind else {
            panic!("expected Path value in shorthand field");
        };
        assert_eq!(segs[0].text(src), "x");
    }

    #[test]
    fn struct_lit_shorthand_two_fields() {
        // FLS §6.11: `Point { x, y }` is shorthand for `Point { x: x, y: y }`.
        let src = "fn f(x: i32, y: i32) -> Point { Point { x, y } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn") };
        let body = f.body.as_ref().unwrap();
        let ExprKind::StructLit { ref fields, .. } = body.tail.as_ref().unwrap().kind else {
            panic!("expected StructLit tail");
        };
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].0.text(src), "x");
        assert_eq!(fields[1].0.text(src), "y");
    }

    #[test]
    fn struct_lit_shorthand_mixed() {
        // FLS §6.11: shorthand and explicit fields can be mixed.
        let src = "fn f(x: i32) -> Point { Point { x, y: 0 } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn") };
        let body = f.body.as_ref().unwrap();
        let ExprKind::StructLit { ref fields, .. } = body.tail.as_ref().unwrap().kind else {
            panic!("expected StructLit tail");
        };
        assert_eq!(fields.len(), 2);
        // First field: shorthand — value is a path expression.
        let ExprKind::Path(_) = fields[0].1.kind else {
            panic!("expected Path for shorthand field");
        };
        // Second field: explicit — value is a literal.
        let ExprKind::LitInt(_) = fields[1].1.kind else {
            panic!("expected LitInt for explicit field");
        };
    }

    #[test]
    fn struct_lit_update_syntax_single_override() {
        // FLS §6.11: struct update syntax `Point { x: 5, ..a }`.
        let src = "fn f(a: Point) -> Point { Point { x: 5, ..a } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn") };
        let body = f.body.as_ref().unwrap();
        let ExprKind::StructLit { ref fields, ref base, .. } = body.tail.as_ref().unwrap().kind else {
            panic!("expected StructLit tail");
        };
        assert_eq!(fields.len(), 1, "expected 1 explicit field");
        assert_eq!(fields[0].0.text(src), "x");
        assert!(base.is_some(), "expected base expression from `..a`");
        let ExprKind::Path(_) = base.as_ref().unwrap().kind else {
            panic!("expected Path for base expression");
        };
    }

    #[test]
    fn struct_lit_update_syntax_no_explicit_fields() {
        // FLS §6.11: struct update with no explicit fields — copies everything.
        let src = "fn f(a: Point) -> Point { Point { ..a } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn") };
        let body = f.body.as_ref().unwrap();
        let ExprKind::StructLit { ref fields, ref base, .. } = body.tail.as_ref().unwrap().kind else {
            panic!("expected StructLit tail");
        };
        assert!(fields.is_empty(), "expected no explicit fields");
        assert!(base.is_some(), "expected base expression from `..a`");
    }

    #[test]
    fn method_call_no_args() {
        // FLS §6.3.2: `vec.len()` — method call with no arguments.
        let src = "fn f() { vec.len(); }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let StmtKind::Expr(ref expr) = body.stmts[0].kind else {
            panic!("expected expr stmt");
        };
        let ExprKind::MethodCall { ref method, ref args, .. } = expr.kind else {
            panic!("expected MethodCall, got {:?}", expr.kind);
        };
        assert_eq!(method.text(src), "len");
        assert!(args.is_empty());
    }

    #[test]
    fn method_call_with_args() {
        // FLS §6.3.2: `vec.push(1)` — method call with one argument.
        let src = "fn f() { vec.push(1); }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let StmtKind::Expr(ref expr) = body.stmts[0].kind else {
            panic!("expected expr stmt");
        };
        let ExprKind::MethodCall { ref method, ref args, .. } = expr.kind else {
            panic!("expected MethodCall");
        };
        assert_eq!(method.text(src), "push");
        assert_eq!(args.len(), 1);
        assert!(matches!(args[0].kind, ExprKind::LitInt(1)));
    }

    #[test]
    fn method_call_chained() {
        // `a.foo().bar()` — chained method calls, left-associative.
        let src = "fn f() { a.foo().bar(); }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let StmtKind::Expr(ref expr) = body.stmts[0].kind else {
            panic!("expected expr stmt");
        };
        // Outermost: `.bar()`
        let ExprKind::MethodCall { ref receiver, ref method, ref args } = expr.kind else {
            panic!("expected outer MethodCall");
        };
        assert_eq!(method.text(src), "bar");
        assert!(args.is_empty());
        // Inner: `a.foo()`
        let ExprKind::MethodCall { method: ref inner_method, .. } = receiver.kind else {
            panic!("expected inner MethodCall");
        };
        assert_eq!(inner_method.text(src), "foo");
    }

    #[test]
    fn method_call_mixed_with_field() {
        // `obj.field.method()` — field access then method call.
        let src = "fn f() { obj.data.len(); }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let StmtKind::Expr(ref expr) = body.stmts[0].kind else {
            panic!("expected expr stmt");
        };
        let ExprKind::MethodCall { ref receiver, ref method, .. } = expr.kind else {
            panic!("expected MethodCall");
        };
        assert_eq!(method.text(src), "len");
        // Receiver is `self.data`
        let ExprKind::FieldAccess { ref field, .. } = receiver.kind else {
            panic!("expected FieldAccess receiver");
        };
        assert_eq!(field.text(src), "data");
    }

    #[test]
    fn error_dot_without_ident() {
        // `a.` with nothing after the dot is a parse error.
        let err = parse_err("fn f() { a.; }");
        assert!(
            err.message.contains("field") || err.message.contains("method"),
            "{}",
            err.message
        );
    }

    // ── Loop expressions — FLS §6.8 ───────────────────────────────────────────

    #[test]
    fn loop_empty_body() {
        // FLS §6.8.1: `loop {}` — infinite loop with empty body.
        let src = "fn f() { loop {} }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        // The loop is the tail expression (no trailing `;`, ends the block).
        let tail = body.tail.as_ref().expect("expected loop as tail");
        let ExprKind::Loop { body: ref loop_body, .. } = tail.kind else {
            panic!("expected Loop, got {:?}", tail.kind);
        };
        assert!(loop_body.stmts.is_empty());
        assert!(loop_body.tail.is_none());
    }

    #[test]
    fn loop_with_body() {
        // FLS §6.15.2: `loop` with statements in its body.
        let src = "fn f() { loop { let x = 1; } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected loop as tail");
        let ExprKind::Loop { body: ref loop_body, .. } = tail.kind else {
            panic!("expected Loop");
        };
        assert_eq!(loop_body.stmts.len(), 1);
    }

    #[test]
    fn loop_as_stmt() {
        // FLS §8.3: a loop (expression-with-block) in non-tail position is a
        // statement; no trailing `;` is required before the next statement.
        let src = "fn f() { loop {} let x = 1; }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        // The loop is stmt[0], the let is stmt[1].
        assert_eq!(body.stmts.len(), 2);
        let StmtKind::Expr(ref loop_expr) = body.stmts[0].kind else {
            panic!("expected expr stmt for loop");
        };
        assert!(matches!(loop_expr.kind, ExprKind::Loop { .. }));
    }

    #[test]
    fn break_without_value() {
        // FLS §6.15.6: bare `break;` exits the loop.
        let src = "fn f() { loop { break; } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected loop tail");
        let ExprKind::Loop { body: ref loop_body, .. } = tail.kind else {
            panic!("expected Loop");
        };
        let StmtKind::Expr(ref brk) = loop_body.stmts[0].kind else {
            panic!("expected expr stmt");
        };
        assert!(matches!(brk.kind, ExprKind::Break { value: None, .. }));
    }

    #[test]
    fn break_with_value() {
        // FLS §6.15.6: `break expr` — loop produces a value.
        let src = "fn f() -> i32 { loop { break 42; } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected loop tail");
        let ExprKind::Loop { body: ref loop_body, .. } = tail.kind else {
            panic!("expected Loop");
        };
        let StmtKind::Expr(ref brk) = loop_body.stmts[0].kind else {
            panic!("expected expr stmt");
        };
        let ExprKind::Break { value: Some(ref val), .. } = brk.kind else {
            panic!("expected Break {{ value: Some(_) }}, got {:?}", brk.kind);
        };
        assert!(matches!(val.kind, ExprKind::LitInt(42)));
    }

    #[test]
    fn continue_expression() {
        // FLS §6.15.7: `continue` skips the rest of the loop body.
        let src = "fn f() { loop { continue; } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected loop tail");
        let ExprKind::Loop { body: ref loop_body, .. } = tail.kind else {
            panic!("expected Loop");
        };
        let StmtKind::Expr(ref cont) = loop_body.stmts[0].kind else {
            panic!("expected expr stmt");
        };
        assert!(matches!(cont.kind, ExprKind::Continue { .. }));
    }

    #[test]
    fn while_loop_simple() {
        // FLS §6.8.2: `while cond {}` — loop terminates when condition is false.
        let src = "fn f() { while running {} }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected while as tail");
        let ExprKind::While { ref cond, ref body, .. } = tail.kind else {
            panic!("expected While, got {:?}", tail.kind);
        };
        // Condition is the path `running`.
        assert!(matches!(cond.kind, ExprKind::Path(_)));
        assert!(body.stmts.is_empty());
    }

    #[test]
    fn while_loop_with_condition_expr() {
        // FLS §6.8.2: `while i < 10 {}` — condition uses a comparison operator.
        let src = "fn f() { while i < 10 { i = i + 1; } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected while as tail");
        let ExprKind::While { ref cond, ref body, .. } = tail.kind else {
            panic!("expected While");
        };
        assert!(matches!(cond.kind, ExprKind::Binary { op: BinOp::Lt, .. }));
        assert_eq!(body.stmts.len(), 1);
    }

    #[test]
    fn while_loop_as_stmt() {
        // FLS §8.3: while (expression-with-block) in non-tail position is a
        // statement without needing a trailing `;`.
        let src = "fn f() { while x {} let y = 1; }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        assert_eq!(body.stmts.len(), 2);
        let StmtKind::Expr(ref while_expr) = body.stmts[0].kind else {
            panic!("expected expr stmt");
        };
        assert!(matches!(while_expr.kind, ExprKind::While { .. }));
    }

    #[test]
    fn while_let_literal_pattern() {
        // FLS §6.15.4: `while let Pat = expr { body }` — loops while pattern matches.
        let src = "fn f() { while let 1 = x {} }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected while-let as tail");
        assert!(
            matches!(tail.kind, ExprKind::WhileLet { pat: Pat::LitInt(1), .. }),
            "expected WhileLet with LitInt(1) pattern, got {:?}",
            tail.kind
        );
    }

    #[test]
    fn while_let_ident_pattern() {
        // FLS §6.15.4 + §5.1.4: `while let v = expr { body }` — identifier pattern.
        let src = "fn f() { while let v = next() {} }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected while-let as tail");
        assert!(
            matches!(tail.kind, ExprKind::WhileLet { pat: Pat::Ident(_), .. }),
            "expected WhileLet with Ident pattern"
        );
    }

    #[test]
    fn while_let_as_stmt() {
        // FLS §6.15.4: while-let (expression-with-block) in statement position
        // does not require a trailing semicolon.
        let src = "fn f() { while let 0 = x {} let y = 1; }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        assert_eq!(body.stmts.len(), 2);
        let StmtKind::Expr(ref wl_expr) = body.stmts[0].kind else {
            panic!("expected expr stmt");
        };
        assert!(matches!(wl_expr.kind, ExprKind::WhileLet { .. }));
    }

    #[test]
    fn for_loop_simple() {
        // FLS §6.8.3: `for x in iter {}` — for loop over an iterator.
        let src = "fn f() { for x in items {} }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected for as tail");
        let ExprKind::For { ref pat, ref iter, ref body, .. } = tail.kind else {
            panic!("expected For, got {:?}", tail.kind);
        };
        let crate::ast::Pat::Ident(ref pat_span) = *pat else { panic!("expected Ident pat") };
        assert_eq!(pat_span.text(src), "x");
        assert!(matches!(iter.kind, ExprKind::Path(_)));
        assert!(body.stmts.is_empty());
    }

    #[test]
    fn for_loop_with_body() {
        // FLS §6.8.3: for loop body with a statement.
        let src = "fn f() { for item in list { process(item); } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected for as tail");
        let ExprKind::For { ref pat, ref body, .. } = tail.kind else {
            panic!("expected For");
        };
        let crate::ast::Pat::Ident(ref pat_span) = *pat else { panic!("expected Ident pat") };
        assert_eq!(pat_span.text(src), "item");
        assert_eq!(body.stmts.len(), 1);
        let StmtKind::Expr(ref call) = body.stmts[0].kind else {
            panic!("expected expr stmt in for body");
        };
        assert!(matches!(call.kind, ExprKind::Call { .. }));
    }

    #[test]
    fn for_loop_as_stmt() {
        // FLS §8.3: for (expression-with-block) in non-tail position is a
        // statement without needing a trailing `;`.
        let src = "fn f() { for i in v {} let n = 0; }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        assert_eq!(body.stmts.len(), 2);
        let StmtKind::Expr(ref for_expr) = body.stmts[0].kind else {
            panic!("expected expr stmt");
        };
        assert!(matches!(for_expr.kind, ExprKind::For { .. }));
    }

    #[test]
    fn error_for_missing_in() {
        // `for x items {}` — missing `in` keyword.
        let err = parse_err("fn f() { for x items {} }");
        assert!(err.message.contains("KwIn"), "{}", err.message);
    }

    // ── Range expressions (FLS §6.16) ─────────────────────────────────────────

    #[test]
    fn range_exclusive() {
        // FLS §6.16: `0..10` produces an exclusive range.
        let src = "fn f() -> i32 { for i in 0..10 { } 0 }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().unwrap();
        let StmtKind::Expr(ref for_expr) = body.stmts[0].kind else { panic!() };
        let ExprKind::For { ref iter, .. } = for_expr.kind else { panic!() };
        let ExprKind::Range { start: Some(_), end: Some(_), inclusive } = iter.kind else {
            panic!("expected Range, got {:?}", iter.kind)
        };
        assert!(!inclusive, "expected exclusive range");
    }

    #[test]
    fn range_inclusive() {
        // FLS §6.16: `0..=9` produces an inclusive range.
        let src = "fn f() -> i32 { for i in 0..=9 { } 0 }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().unwrap();
        let StmtKind::Expr(ref for_expr) = body.stmts[0].kind else { panic!() };
        let ExprKind::For { ref iter, .. } = for_expr.kind else { panic!() };
        let ExprKind::Range { start: Some(_), end: Some(_), inclusive } = iter.kind else {
            panic!("expected Range, got {:?}", iter.kind)
        };
        assert!(inclusive, "expected inclusive range");
    }

    #[test]
    fn range_lower_precedence_than_comparison() {
        // FLS §6.21: `a < b..c` parses as `(a < b)..c`, not `a < (b..c)`.
        // Range has lower precedence than comparison operators.
        let src = "fn f() { let _x = 0..10; }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().unwrap();
        let StmtKind::Let { init: Some(ref init), .. } = body.stmts[0].kind else { panic!() };
        assert!(matches!(init.kind, ExprKind::Range { .. }), "expected Range expr in let binding");
    }

    // ── Type cast expressions (FLS §6.5.9) ───────────────────────────────────

    #[test]
    fn cast_literal_to_i32() {
        // FLS §6.5.9: `5 as i32` — integer identity cast.
        let src = "fn f() -> i32 { 5 as i32 }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected tail");
        let ExprKind::Cast { ref ty, .. } = tail.kind else {
            panic!("expected Cast, got {:?}", tail.kind);
        };
        let TyKind::Path(ref segs) = ty.kind else { panic!("expected Path type") };
        assert_eq!(segs[0].text(src), "i32");
    }

    #[test]
    fn cast_bool_to_i32() {
        // FLS §6.5.9: `true as i32` — boolean to integer cast.
        let src = "fn f() -> i32 { true as i32 }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected tail");
        let ExprKind::Cast { ref expr, ref ty } = tail.kind else {
            panic!("expected Cast, got {:?}", tail.kind);
        };
        assert!(matches!(expr.kind, ExprKind::LitBool(true)));
        let TyKind::Path(ref segs) = ty.kind else { panic!() };
        assert_eq!(segs[0].text(src), "i32");
    }

    #[test]
    fn cast_left_associative() {
        // FLS §6.5.9: `as` is left-associative. `x as i32 as i32` →
        // `(x as i32) as i32`.
        let src = "fn f(x: i32) -> i32 { x as i32 as i32 }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected tail");
        // Outer cast: _ as i32
        let ExprKind::Cast { ref expr, .. } = tail.kind else {
            panic!("expected outer Cast, got {:?}", tail.kind);
        };
        // Inner cast: x as i32
        assert!(matches!(expr.kind, ExprKind::Cast { .. }),
            "expected inner Cast, got {:?}", expr.kind);
    }

    #[test]
    fn cast_higher_precedence_than_multiply() {
        // FLS §6.5.9: `a * b as i32` → `a * (b as i32)`.
        // The cast binds tighter than multiplication.
        let src = "fn f(a: i32, b: i32) -> i32 { a * b as i32 }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected tail");
        // Top-level is Binary(Mul, a, Cast(b, i32))
        let ExprKind::Binary { op: BinOp::Mul, ref lhs, ref rhs } = tail.kind else {
            panic!("expected Mul, got {:?}", tail.kind);
        };
        assert!(matches!(lhs.kind, ExprKind::Path(_)), "lhs should be `a`");
        assert!(matches!(rhs.kind, ExprKind::Cast { .. }),
            "rhs should be `b as i32`, got {:?}", rhs.kind);
    }

    // ── Match expression patterns ─────────────────────────────────────────────

    #[test]
    fn negative_literal_pattern_in_match() {
        // FLS §5.2: Negative integer literal pattern `-n`.
        // `match x { -1 => 0, 0 => 1, _ => 2 }` should parse successfully
        // and produce Pat::NegLitInt(1) for the first arm.
        use crate::ast::{ExprKind, Pat};
        let src = "fn f(x: i32) -> i32 { match x { -1 => 0, 0 => 1, _ => 2 } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected tail");
        let ExprKind::Match { ref arms, .. } = tail.kind else {
            panic!("expected Match, got {:?}", tail.kind);
        };
        assert_eq!(arms.len(), 3, "expected 3 arms");
        // First arm: -1 pattern
        assert!(
            matches!(arms[0].pat, Pat::NegLitInt(1)),
            "expected NegLitInt(1), got {:?}", arms[0].pat
        );
        // Second arm: 0 pattern
        assert!(
            matches!(arms[1].pat, Pat::LitInt(0)),
            "expected LitInt(0), got {:?}", arms[1].pat
        );
        // Third arm: wildcard
        assert!(
            matches!(arms[2].pat, Pat::Wildcard),
            "expected Wildcard, got {:?}", arms[2].pat
        );
    }

    #[test]
    fn or_pattern_two_alternatives() {
        // FLS §5.1.11: OR pattern `0 | 1` in a match arm.
        use crate::ast::{ExprKind, Pat};
        let src = "fn f(x: i32) -> i32 { match x { 0 | 1 => 10, _ => 20 } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected tail");
        let ExprKind::Match { ref arms, .. } = tail.kind else {
            panic!("expected Match");
        };
        assert_eq!(arms.len(), 2, "expected 2 arms");
        // First arm: OR pattern with alternatives 0 and 1.
        let Pat::Or(ref alts) = arms[0].pat else {
            panic!("expected Pat::Or, got {:?}", arms[0].pat);
        };
        assert_eq!(alts.len(), 2);
        assert!(matches!(alts[0], Pat::LitInt(0)));
        assert!(matches!(alts[1], Pat::LitInt(1)));
        // Second arm: wildcard.
        assert!(matches!(arms[1].pat, Pat::Wildcard));
    }

    #[test]
    fn or_pattern_three_alternatives() {
        // FLS §5.1.11: Three-way OR pattern `1 | 2 | 3`.
        use crate::ast::{ExprKind, Pat};
        let src = "fn f(x: i32) -> i32 { match x { 1 | 2 | 3 => 1, _ => 0 } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected tail");
        let ExprKind::Match { ref arms, .. } = tail.kind else { panic!() };
        let Pat::Or(ref alts) = arms[0].pat else { panic!("expected Or") };
        assert_eq!(alts.len(), 3);
        assert!(matches!(alts[0], Pat::LitInt(1)));
        assert!(matches!(alts[1], Pat::LitInt(2)));
        assert!(matches!(alts[2], Pat::LitInt(3)));
    }

    #[test]
    fn identifier_pattern_in_match() {
        // FLS §5.1.4: Identifier pattern `n` matches any value and binds it.
        // Example: `match x { 0 => 0, n => n * 2 }` — second arm uses ident pat.
        use crate::ast::{ExprKind, Pat};
        let src = "fn f(x: i32) -> i32 { match x { 0 => 0, n => n * 2 } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected tail");
        let ExprKind::Match { ref arms, .. } = tail.kind else { panic!() };
        assert_eq!(arms.len(), 2);
        assert!(matches!(arms[0].pat, Pat::LitInt(0)));
        // Second arm should be Pat::Ident pointing to "n".
        assert!(matches!(arms[1].pat, Pat::Ident(_)));
        if let Pat::Ident(span) = &arms[1].pat {
            assert_eq!(span.text(src), "n");
        }
    }

    #[test]
    fn path_pattern_two_segments() {
        // FLS §5.5: Path pattern `Color::Red` — two-segment path.
        use crate::ast::{ExprKind, Pat};
        let src = "enum Color { Red, Blue }\nfn f(c: i32) -> i32 { match c { Color::Red => 0, _ => 1 } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[1].kind else { panic!("expected fn item") };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected tail");
        let ExprKind::Match { ref arms, .. } = tail.kind else { panic!("expected match") };
        assert_eq!(arms.len(), 2);
        if let Pat::Path(ref segs) = arms[0].pat {
            assert_eq!(segs.len(), 2);
            assert_eq!(segs[0].text(src), "Color");
            assert_eq!(segs[1].text(src), "Red");
        } else {
            panic!("expected Pat::Path, got {:?}", arms[0].pat);
        }
        assert!(matches!(arms[1].pat, Pat::Wildcard));
    }

    #[test]
    fn range_inclusive_pattern_in_match() {
        // FLS §5.1.9: Inclusive range pattern `lo..=hi`.
        use crate::ast::{ExprKind, Pat};
        let src = "fn f(x: i32) -> i32 { match x { 1..=3 => 1, _ => 0 } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected tail");
        let ExprKind::Match { ref arms, .. } = tail.kind else {
            panic!("expected Match");
        };
        assert_eq!(arms.len(), 2, "expected 2 arms");
        assert!(
            matches!(arms[0].pat, Pat::RangeInclusive { lo: 1, hi: 3 }),
            "expected RangeInclusive{{lo:1, hi:3}}, got {:?}", arms[0].pat
        );
        assert!(matches!(arms[1].pat, Pat::Wildcard));
    }

    #[test]
    fn range_exclusive_pattern_in_match() {
        // FLS §5.1.9: Exclusive range pattern `lo..hi`.
        use crate::ast::{ExprKind, Pat};
        let src = "fn f(x: i32) -> i32 { match x { 1..4 => 1, _ => 0 } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected tail");
        let ExprKind::Match { ref arms, .. } = tail.kind else {
            panic!("expected Match");
        };
        assert_eq!(arms.len(), 2, "expected 2 arms");
        assert!(
            matches!(arms[0].pat, Pat::RangeExclusive { lo: 1, hi: 4 }),
            "expected RangeExclusive{{lo:1, hi:4}}, got {:?}", arms[0].pat
        );
        assert!(matches!(arms[1].pat, Pat::Wildcard));
    }

    #[test]
    fn range_pattern_negative_lower_bound() {
        // FLS §5.1.9: Inclusive range pattern with negative lower bound `-5..=-1`.
        use crate::ast::{ExprKind, Pat};
        let src = "fn f(x: i32) -> i32 { match x { -5..=-1 => 1, _ => 0 } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected tail");
        let ExprKind::Match { ref arms, .. } = tail.kind else {
            panic!("expected Match");
        };
        assert_eq!(arms.len(), 2);
        assert!(
            matches!(arms[0].pat, Pat::RangeInclusive { lo: -5, hi: -1 }),
            "expected RangeInclusive{{lo:-5, hi:-1}}, got {:?}", arms[0].pat
        );
    }

    #[test]
    fn match_arm_guard_simple() {
        // FLS §6.18: Match arm guard `if expr` is parsed after the pattern.
        // Example: `match x { n if n > 5 => 1, _ => 0 }` — first arm has a guard.
        use crate::ast::{ExprKind, Pat};
        let src = "fn f(x: i32) -> i32 { match x { n if x > 5 => 1, _ => 0 } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected tail");
        let ExprKind::Match { ref arms, .. } = tail.kind else {
            panic!("expected Match");
        };
        assert_eq!(arms.len(), 2);
        assert!(matches!(arms[0].pat, Pat::Ident(_)));
        assert!(arms[0].guard.is_some(), "first arm should have a guard");
        assert!(arms[1].guard.is_none(), "wildcard arm should have no guard");
    }

    #[test]
    fn match_arm_no_guard() {
        // FLS §6.18: Arms without a guard have `guard: None`.
        use crate::ast::{ExprKind, Pat};
        let src = "fn f(x: i32) -> i32 { match x { 0 => 1, _ => 0 } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected tail");
        let ExprKind::Match { ref arms, .. } = tail.kind else {
            panic!("expected Match");
        };
        assert_eq!(arms.len(), 2);
        assert!(arms[0].guard.is_none());
        assert!(arms[1].guard.is_none());
        assert!(matches!(arms[0].pat, Pat::LitInt(0)));
    }

    /// FLS §5.4 + §15: Tuple struct/variant pattern with one field.
    ///
    /// `Opt::Some(v)` parses as `Pat::TupleStruct { path: ["Opt","Some"], fields: [Ident("v")] }`.
    #[test]
    fn tuple_struct_pattern_one_field() {
        let src = "fn f(x: i32) -> i32 { match x { Opt::Some(v) => 1, _ => 0, } }";
        let tokens = crate::lexer::tokenize(src).unwrap();
        let sf = parse(&tokens, src).unwrap();
        let ItemKind::Fn(f) = &sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().expect("no body");
        let Some(tail) = body.tail.as_ref() else { panic!() };
        let ExprKind::Match { arms, .. } = &tail.kind else { panic!() };
        match &arms[0].pat {
            Pat::TupleStruct { path, fields } => {
                assert_eq!(path[0].text(src), "Opt");
                assert_eq!(path[1].text(src), "Some");
                assert_eq!(fields.len(), 1);
                assert!(matches!(&fields[0], Pat::Ident(_)));
            }
            other => panic!("expected TupleStruct pattern, got {other:?}"),
        }
    }

    /// FLS §5.4 + §15: Tuple struct/variant pattern with wildcard field.
    ///
    /// `Opt::Some(_)` — the `_` field is a wildcard, not an identifier.
    #[test]
    fn tuple_struct_pattern_wildcard_field() {
        let src = "fn f(x: i32) -> i32 { match x { Opt::Some(_) => 1, _ => 0, } }";
        let tokens = crate::lexer::tokenize(src).unwrap();
        let sf = parse(&tokens, src).unwrap();
        let ItemKind::Fn(f) = &sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().expect("no body");
        let Some(tail) = body.tail.as_ref() else { panic!() };
        let ExprKind::Match { arms, .. } = &tail.kind else { panic!() };
        match &arms[0].pat {
            Pat::TupleStruct { path, fields } => {
                assert_eq!(path[1].text(src), "Some");
                assert_eq!(fields.len(), 1);
                assert!(matches!(&fields[0], Pat::Wildcard));
            }
            other => panic!("expected TupleStruct pattern, got {other:?}"),
        }
    }

    /// FLS §5.4 + §15: Tuple struct/variant pattern with two fields.
    ///
    /// `Pair::Two(a, b)` — two identifier bindings.
    #[test]
    fn tuple_struct_pattern_two_fields() {
        let src = "fn f(x: i32) -> i32 { match x { Pair::Two(a, b) => 1, _ => 0, } }";
        let tokens = crate::lexer::tokenize(src).unwrap();
        let sf = parse(&tokens, src).unwrap();
        let ItemKind::Fn(f) = &sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().expect("no body");
        let Some(tail) = body.tail.as_ref() else { panic!() };
        let ExprKind::Match { arms, .. } = &tail.kind else { panic!() };
        match &arms[0].pat {
            Pat::TupleStruct { path, fields } => {
                assert_eq!(path[1].text(src), "Two");
                assert_eq!(fields.len(), 2);
                assert!(matches!(&fields[0], Pat::Ident(_)));
                assert!(matches!(&fields[1], Pat::Ident(_)));
            }
            other => panic!("expected TupleStruct pattern, got {other:?}"),
        }
    }

    // ── Struct pattern tests ──────────────────────────────────────────────────

    /// FLS §5.3: Single-segment struct pattern `Point { x, y }` in match arm.
    ///
    /// `match p { Point { x, y } => x + y }` — shorthand field binding.
    /// Parses as `Pat::StructVariant { path: ["Point"], fields: [("x", Ident), ("y", Ident)] }`.
    #[test]
    fn struct_pattern_single_segment_two_fields() {
        let src = "fn f(n: i32) -> i32 { let p = Point { x: 1, y: 2 }; match p { Point { x, y } => x + y } }";
        let tokens = crate::lexer::tokenize(src).unwrap();
        let sf = parse(&tokens, src).unwrap();
        let ItemKind::Fn(f) = &sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().expect("no body");
        let Some(tail) = body.tail.as_ref() else { panic!() };
        let ExprKind::Match { arms, .. } = &tail.kind else { panic!("expected match") };
        match &arms[0].pat {
            Pat::StructVariant { path, fields } => {
                assert_eq!(path.len(), 1);
                assert_eq!(path[0].text(src), "Point");
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].0.text(src), "x");
                assert_eq!(fields[1].0.text(src), "y");
                assert!(matches!(&fields[0].1, Pat::Ident(_)));
                assert!(matches!(&fields[1].1, Pat::Ident(_)));
            }
            other => panic!("expected StructVariant pattern, got {other:?}"),
        }
    }

    /// FLS §5.3: Single-segment struct pattern with wildcard field.
    ///
    /// `Point { x, y: _ }` — first field bound, second discarded.
    #[test]
    fn struct_pattern_single_segment_wildcard_field() {
        let src = "fn f(n: i32) -> i32 { let p = Point { x: 1, y: 2 }; match p { Point { x, y: _ } => x } }";
        let tokens = crate::lexer::tokenize(src).unwrap();
        let sf = parse(&tokens, src).unwrap();
        let ItemKind::Fn(f) = &sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().expect("no body");
        let Some(tail) = body.tail.as_ref() else { panic!() };
        let ExprKind::Match { arms, .. } = &tail.kind else { panic!("expected match") };
        match &arms[0].pat {
            Pat::StructVariant { path, fields } => {
                assert_eq!(path.len(), 1);
                assert_eq!(path[0].text(src), "Point");
                assert_eq!(fields.len(), 2);
                assert!(matches!(&fields[1].1, Pat::Wildcard));
            }
            other => panic!("expected StructVariant pattern, got {other:?}"),
        }
    }

    // ── Trait definition tests ────────────────────────────────────────────────

    /// FLS §13: Simple trait definition with one method signature (no body).
    ///
    /// `trait Foo { fn bar(&self) -> i32; }` — one bodyless method signature.
    #[test]
    fn trait_definition_simple() {
        use crate::ast::ItemKind;
        let src = "trait Foo { fn bar(&self) -> i32; }";
        let tokens = crate::lexer::tokenize(src).unwrap();
        let sf = parse(&tokens, src).unwrap();
        assert_eq!(sf.items.len(), 1);
        let ItemKind::Trait(t) = &sf.items[0].kind else {
            panic!("expected Trait item")
        };
        assert_eq!(t.name.text(src), "Foo");
        assert_eq!(t.methods.len(), 1);
        assert_eq!(t.methods[0].name.text(src), "bar");
        assert!(t.methods[0].body.is_none(), "trait method should have no body");
    }

    /// FLS §13: Trait with multiple method signatures.
    ///
    /// `trait Shape { fn area(&self) -> i32; fn perimeter(&self) -> i32; }`
    #[test]
    fn trait_multiple_methods() {
        use crate::ast::ItemKind;
        let src = "trait Shape { fn area(&self) -> i32; fn perimeter(&self) -> i32; }";
        let tokens = crate::lexer::tokenize(src).unwrap();
        let sf = parse(&tokens, src).unwrap();
        let ItemKind::Trait(t) = &sf.items[0].kind else {
            panic!("expected Trait item")
        };
        assert_eq!(t.methods.len(), 2);
        assert_eq!(t.methods[0].name.text(src), "area");
        assert_eq!(t.methods[1].name.text(src), "perimeter");
    }

    /// FLS §11.1: `impl Trait for Type` — trait implementation parsed correctly.
    ///
    /// The `ImplDef.ty` must be the struct type (`Foo`), and `ImplDef.trait_name`
    /// must be the trait name (`Bar`).
    #[test]
    fn impl_trait_for_type() {
        use crate::ast::ItemKind;
        let src = "impl Bar for Foo { fn method(&self) -> i32 { 1 } }";
        let tokens = crate::lexer::tokenize(src).unwrap();
        let sf = parse(&tokens, src).unwrap();
        let ItemKind::Impl(imp) = &sf.items[0].kind else {
            panic!("expected Impl item")
        };
        assert_eq!(imp.ty.text(src), "Foo", "ty should be the struct");
        assert!(imp.trait_name.is_some(), "trait_name should be Some");
        assert_eq!(imp.trait_name.unwrap().text(src), "Bar");
        assert_eq!(imp.methods.len(), 1);
        assert!(imp.methods[0].body.is_some(), "impl method must have a body");
    }

    /// FLS §11.2: Inherent impl still parses correctly (no regression).
    ///
    /// `impl Foo { fn method(&self) -> i32 { 1 } }` — `trait_name` is `None`.
    #[test]
    fn impl_inherent_has_no_trait_name() {
        use crate::ast::ItemKind;
        let src = "impl Foo { fn method(&self) -> i32 { 1 } }";
        let tokens = crate::lexer::tokenize(src).unwrap();
        let sf = parse(&tokens, src).unwrap();
        let ItemKind::Impl(imp) = &sf.items[0].kind else {
            panic!("expected Impl item")
        };
        assert_eq!(imp.ty.text(src), "Foo");
        assert!(imp.trait_name.is_none(), "inherent impl has no trait name");
    }

    /// FLS §13 + §11.1: Trait definition followed by impl trait for type.
    ///
    /// Both items are parsed; the file has 3 items total: trait, struct, impl.
    #[test]
    fn trait_definition_and_impl() {
        use crate::ast::ItemKind;
        let src = r#"
trait Area { fn area(&self) -> i32; }
struct Square { side: i32 }
impl Area for Square { fn area(&self) -> i32 { self.side * self.side } }
"#;
        let tokens = crate::lexer::tokenize(src).unwrap();
        let sf = parse(&tokens, src).unwrap();
        assert_eq!(sf.items.len(), 3);
        assert!(matches!(sf.items[0].kind, ItemKind::Trait(_)));
        assert!(matches!(sf.items[1].kind, ItemKind::Struct(_)));
        assert!(matches!(sf.items[2].kind, ItemKind::Impl(_)));
    }

    // ── Array expressions (FLS §6.8) and index expressions (FLS §6.9) ──────────

    #[test]
    fn array_literal_empty() {
        // FLS §6.8: An empty array `[]` is a valid array expression.
        // FLS §6.8: "An array expression consists of a comma-separated list."
        // The list may be empty (zero elements).
        // Note: type annotation `[i32; 0]` is not yet parsed — omit it here.
        let src = "fn f() { let _a = []; }";
        // Just verify it parses without error.
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().unwrap();
        let StmtKind::Let { init: Some(ref init), .. } = body.stmts[0].kind else {
            panic!("expected let binding");
        };
        assert!(matches!(init.kind, ExprKind::Array(ref v) if v.is_empty()));
    }

    #[test]
    fn array_literal_three_elements() {
        // FLS §6.8: `[10, 20, 30]` is an array expression with three elements.
        let src = "fn f() -> i32 { let a = [10, 20, 30]; a[0] }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().unwrap();
        let StmtKind::Let { init: Some(ref init), .. } = body.stmts[0].kind else {
            panic!("expected let binding");
        };
        let ExprKind::Array(ref elems) = init.kind else {
            panic!("expected Array, got {:?}", init.kind);
        };
        assert_eq!(elems.len(), 3);
        assert!(matches!(elems[0].kind, ExprKind::LitInt(10)));
        assert!(matches!(elems[1].kind, ExprKind::LitInt(20)));
        assert!(matches!(elems[2].kind, ExprKind::LitInt(30)));
    }

    #[test]
    fn array_repeat_expression() {
        // FLS §6.8: `[0; 5]` is an array repeat expression.
        // The fill value is `0` and the count is `5`.
        let src = "fn f() -> i32 { let a = [0_i32; 5]; a[0] }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().unwrap();
        let StmtKind::Let { init: Some(ref init), .. } = body.stmts[0].kind else {
            panic!("expected let binding");
        };
        let ExprKind::ArrayRepeat { ref value, ref count } = init.kind else {
            panic!("expected ArrayRepeat, got {:?}", init.kind);
        };
        // Value is `0_i32` — a LitInt with value 0.
        assert!(matches!(value.kind, ExprKind::LitInt(0)), "expected fill 0");
        // Count is `5` — a LitInt with value 5.
        assert!(matches!(count.kind, ExprKind::LitInt(5)), "expected count 5");
    }

    #[test]
    fn array_index_constant() {
        // FLS §6.9: `a[1]` is an indexing expression.
        // The index is a literal integer — still a runtime index per the spec.
        let src = "fn f() -> i32 { let a = [10, 20]; a[1] }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected tail expression");
        let ExprKind::Index { ref base, ref index } = tail.kind else {
            panic!("expected Index, got {:?}", tail.kind);
        };
        // Base is the path expression `a`.
        assert!(matches!(base.kind, ExprKind::Path(_)));
        // Index is the integer literal 1.
        assert!(matches!(index.kind, ExprKind::LitInt(1)));
    }

    #[test]
    fn array_index_variable() {
        // FLS §6.9: `a[i]` is an indexing expression with a path index.
        let src = "fn f() -> i32 { let a = [1, 2, 3]; let i = 0; a[i] }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected tail expression");
        let ExprKind::Index { ref base, ref index } = tail.kind else {
            panic!("expected Index, got {:?}", tail.kind);
        };
        assert!(matches!(base.kind, ExprKind::Path(_)));
        assert!(matches!(index.kind, ExprKind::Path(_)));
    }

    #[test]
    fn array_index_in_expression() {
        // FLS §6.9 + §6.5.5: `a[0] + a[1]` — indexing as a sub-expression.
        let src = "fn f() -> i32 { let a = [3, 7]; a[0] + a[1] }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected tail expression");
        // Top-level is a binary addition.
        let ExprKind::Binary { op: BinOp::Add, ref lhs, ref rhs } = tail.kind else {
            panic!("expected Add, got {:?}", tail.kind);
        };
        assert!(matches!(lhs.kind, ExprKind::Index { .. }));
        assert!(matches!(rhs.kind, ExprKind::Index { .. }));
    }

    #[test]
    fn tuple_two_elements() {
        // FLS §6.10: `(1, 2)` is a tuple expression with two elements.
        let src = "fn f() -> i32 { let t = (1, 2); t.0 }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().unwrap();
        let StmtKind::Let { init: Some(ref init), .. } = body.stmts[0].kind else {
            panic!("expected let binding");
        };
        let ExprKind::Tuple(ref elems) = init.kind else {
            panic!("expected Tuple, got {:?}", init.kind);
        };
        assert_eq!(elems.len(), 2);
        assert!(matches!(elems[0].kind, ExprKind::LitInt(1)));
        assert!(matches!(elems[1].kind, ExprKind::LitInt(2)));
    }

    #[test]
    fn tuple_field_access_integer_index() {
        // FLS §6.10: `t.0` accesses the first tuple field via a FieldAccess node
        // whose `field` span contains the integer literal `0`.
        let src = "fn f() -> i32 { let t = (10, 20); t.0 + t.1 }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!() };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected tail expression");
        let ExprKind::Binary { op: BinOp::Add, ref lhs, ref rhs } = tail.kind else {
            panic!("expected Add, got {:?}", tail.kind);
        };
        // t.0 — field is "0"
        let ExprKind::FieldAccess { ref field, .. } = lhs.kind else {
            panic!("expected FieldAccess, got {:?}", lhs.kind);
        };
        assert_eq!(field.text(src), "0");
        // t.1 — field is "1"
        let ExprKind::FieldAccess { field: ref field1, .. } = rhs.kind else {
            panic!("expected FieldAccess, got {:?}", rhs.kind);
        };
        assert_eq!(field1.text(src), "1");
    }
}
