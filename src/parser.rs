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
    NamedField, Param, SourceFile, Span, Stmt, StmtKind, StructDef, StructKind, Ty, TyKind,
    TupleField, UnaryOp, Visibility,
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
}

impl<'src> Parser<'src> {
    fn new(tokens: &'src [Token], src: &'src str) -> Self {
        // Guard: we require at least one token (the Eof sentinel).
        assert!(!tokens.is_empty(), "token slice must contain at least Eof");
        Parser { tokens, src, cursor: 0 }
    }

    // ── Low-level token access ────────────────────────────────────────────────

    /// The current token (never out of bounds; stays at Eof at end).
    fn current(&self) -> &Token {
        &self.tokens[self.cursor]
    }

    fn peek_kind(&self) -> TokenKind {
        self.current().kind
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
                let fn_def = self.parse_fn_def(vis)?;
                let end = fn_def
                    .body
                    .as_ref()
                    .map(|b| b.span)
                    .unwrap_or(start);
                let span = start.to(end);
                Ok(Item { kind: ItemKind::Fn(Box::new(fn_def)), span })
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
            kind => Err(self.error(format!(
                "expected item (fn, struct, enum, …), found {kind:?}"
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
    fn parse_fn_def(&mut self, vis: Visibility) -> Result<FnDef, ParseError> {
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

        // Parameter list enclosed in `( )`.
        self.expect(TokenKind::OpenParen)?;
        let params = self.parse_params()?;
        self.expect(TokenKind::CloseParen)?;

        // Optional return type `-> Type`.
        // FLS §9: absent return type means the function returns `()`.
        let ret_ty = if self.eat(TokenKind::RArrow) {
            Some(self.parse_ty()?)
        } else {
            None
        };

        // Function body (required for non-extern/non-trait functions).
        // FLS §9: the body must be a block expression.
        let body = Some(self.parse_block()?);

        Ok(FnDef { vis, name, params, ret_ty, body })
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
        Ok(StructDef { vis, name, kind, span })
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

        // Enum body is always `{ … }`.
        self.expect(TokenKind::OpenBrace)?;
        let variants = self.parse_enum_variants()?;
        let end = self.current_span();
        self.expect(TokenKind::CloseBrace)?;

        let span = start.to(end);
        Ok(EnumDef { vis, name, variants, span })
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

    /// Parse the parameter list between the `(` and `)`.
    ///
    /// FLS §9.2: Function parameters.
    ///
    /// Grammar: `(Identifier ":" Type ("," Identifier ":" Type)* ","?)?`
    ///
    /// FLS §9.2 NOTE: Full patterns (struct, tuple, `_`) in parameter
    /// position are not yet handled. `self`, `mut self`, `&self`, and
    /// `&mut self` are also not yet supported.
    fn parse_params(&mut self) -> Result<Vec<Param>, ParseError> {
        let mut params = Vec::new();

        while self.peek_kind() != TokenKind::CloseParen
            && self.peek_kind() != TokenKind::Eof
        {
            let start = self.current_span();

            if self.peek_kind() != TokenKind::Ident {
                return Err(self.error(format!(
                    "expected parameter name (identifier), found {:?}",
                    self.peek_kind()
                )));
            }
            let name = self.current_span();
            self.advance();

            self.expect(TokenKind::Colon)?;
            let ty = self.parse_ty()?;
            let end = ty.span;

            params.push(Param { name, ty, span: start.to(end) });

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
            // Unit type `()` — FLS §4.4
            TokenKind::OpenParen => {
                self.advance();
                let end = self.current_span();
                self.expect(TokenKind::CloseParen)?;
                Ok(Ty { kind: TyKind::Unit, span: start.to(end) })
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

                let end = *segments.last().unwrap();
                Ok(Ty { kind: TyKind::Path(segments), span: start.to(end) })
            }

            kind => Err(self.error(format!("expected type, found {kind:?}"))),
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
                | ExprKind::Loop(_)
                | ExprKind::While { .. }
                | ExprKind::For { .. }
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

        // Pattern: identifier or `_`.
        if self.peek_kind() != TokenKind::Ident
            && self.peek_kind() != TokenKind::Underscore
        {
            return Err(self.error(format!(
                "expected identifier in let pattern, found {:?}",
                self.peek_kind()
            )));
        }
        let name = self.current_span();
        self.advance();

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

        let end = self.current_span();
        self.expect(TokenKind::Semi)?;

        Ok(Stmt {
            kind: StmtKind::Let { name, ty, init },
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

    /// Assignment and compound assignment — FLS §6.5.10, §6.5.11. Right-associative.
    ///
    /// Plain `=` lowers to `ExprKind::Binary { op: BinOp::Assign, .. }`.
    /// Compound `op=` lowers to `ExprKind::CompoundAssign { op, .. }`.
    ///
    /// FLS §6.5.11: Compound assignment operators are distinct expression forms
    /// from plain assignment. The left-hand side must be a place expression.
    fn parse_assign(&mut self) -> Result<Expr, ParseError> {
        let lhs = self.parse_or()?;

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

    /// Logical or `||` — FLS §6.8.2. Left-associative.
    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_and()?;

        while self.peek_kind() == TokenKind::OrOr {
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

        while self.peek_kind() == TokenKind::AndAnd {
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

        while self.peek_kind() == TokenKind::Or {
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

        while self.peek_kind() == TokenKind::Caret {
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

        while self.peek_kind() == TokenKind::And {
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
                TokenKind::OpenParen => {
                    expr = self.parse_call(expr)?;
                }

                // Field access or method call: `expr.ident` or `expr.ident(args)`
                // FLS §6.3.2, §6.3.3
                TokenKind::Dot => {
                    self.advance(); // eat `.`

                    if self.peek_kind() != TokenKind::Ident {
                        return Err(self.error("expected field or method name after `.`"));
                    }
                    let member_span = self.current_span();
                    self.advance(); // eat identifier

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

            // Path expression or function call — FLS §6.2, §6.3.1
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

                let path_end = *segments.last().unwrap();
                Ok(Expr {
                    kind: ExprKind::Path(segments),
                    span: start.to(path_end),
                })
            }

            // Grouped expression or unit `()` — FLS §6.3.2, §6.3.3
            TokenKind::OpenParen => {
                self.advance(); // eat `(`

                // Unit `()` — FLS §6.3.3
                if self.eat(TokenKind::CloseParen) {
                    return Ok(Expr { kind: ExprKind::Unit, span: start });
                }

                // Grouped (parenthesised) expression — FLS §6.3.2
                let inner = self.parse_expr()?;
                self.expect(TokenKind::CloseParen)?;
                Ok(inner)
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

            // Loop expression — FLS §6.8.1
            TokenKind::KwLoop => {
                self.advance();
                let body = Box::new(self.parse_block()?);
                let span = start.to(body.span);
                Ok(Expr { kind: ExprKind::Loop(body), span })
            }

            // While loop expression — FLS §6.8.2
            TokenKind::KwWhile => self.parse_while_expr(),

            // For loop expression — FLS §6.8.3
            TokenKind::KwFor => self.parse_for_expr(),

            // Break expression — FLS §6.8.4
            TokenKind::KwBreak => {
                self.advance();
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
                Ok(Expr { kind: ExprKind::Break(value), span: start.to(end) })
            }

            // Continue expression — FLS §6.8.5
            TokenKind::KwContinue => {
                self.advance();
                Ok(Expr { kind: ExprKind::Continue, span: start })
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

        let cond = Box::new(self.parse_expr()?);
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
    fn parse_while_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.current_span();
        self.expect(TokenKind::KwWhile)?;
        let cond = Box::new(self.parse_expr()?);
        let body = Box::new(self.parse_block()?);
        let span = start.to(body.span);
        Ok(Expr { kind: ExprKind::While { cond, body }, span })
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
    fn parse_for_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.current_span();
        self.expect(TokenKind::KwFor)?;

        // Pattern: identifier only for now (future: full irrefutable patterns).
        if self.peek_kind() != TokenKind::Ident {
            return Err(self.error(format!(
                "expected loop variable (identifier) after `for`, found {:?}",
                self.peek_kind()
            )));
        }
        let pat = self.current_span();
        self.advance();

        self.expect(TokenKind::KwIn)?;
        let iter = Box::new(self.parse_expr()?);
        let body = Box::new(self.parse_block()?);
        let span = start.to(body.span);
        Ok(Expr { kind: ExprKind::For { pat, iter, body }, span })
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
    use crate::ast::{EnumVariantKind, ExprKind, ItemKind, StmtKind, StructKind, TyKind, Visibility};
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
        assert_eq!(f.params[0].name.text(src), "a");
        assert_eq!(f.params[1].name.text(src), "b");
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
        let ExprKind::Loop(ref loop_body) = tail.kind else {
            panic!("expected Loop, got {:?}", tail.kind);
        };
        assert!(loop_body.stmts.is_empty());
        assert!(loop_body.tail.is_none());
    }

    #[test]
    fn loop_with_body() {
        // FLS §6.8.1: `loop` with statements in its body.
        let src = "fn f() { loop { let x = 1; } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected loop as tail");
        let ExprKind::Loop(ref loop_body) = tail.kind else {
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
        assert!(matches!(loop_expr.kind, ExprKind::Loop(_)));
    }

    #[test]
    fn break_without_value() {
        // FLS §6.8.4: bare `break;` exits the loop.
        let src = "fn f() { loop { break; } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected loop tail");
        let ExprKind::Loop(ref loop_body) = tail.kind else {
            panic!("expected Loop");
        };
        let StmtKind::Expr(ref brk) = loop_body.stmts[0].kind else {
            panic!("expected expr stmt");
        };
        assert!(matches!(brk.kind, ExprKind::Break(None)));
    }

    #[test]
    fn break_with_value() {
        // FLS §6.8.4: `break expr` — loop produces a value.
        let src = "fn f() -> i32 { loop { break 42; } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected loop tail");
        let ExprKind::Loop(ref loop_body) = tail.kind else {
            panic!("expected Loop");
        };
        let StmtKind::Expr(ref brk) = loop_body.stmts[0].kind else {
            panic!("expected expr stmt");
        };
        let ExprKind::Break(Some(ref val)) = brk.kind else {
            panic!("expected Break(Some(_)), got {:?}", brk.kind);
        };
        assert!(matches!(val.kind, ExprKind::LitInt(42)));
    }

    #[test]
    fn continue_expression() {
        // FLS §6.8.5: `continue` skips the rest of the loop body.
        let src = "fn f() { loop { continue; } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected loop tail");
        let ExprKind::Loop(ref loop_body) = tail.kind else {
            panic!("expected Loop");
        };
        let StmtKind::Expr(ref cont) = loop_body.stmts[0].kind else {
            panic!("expected expr stmt");
        };
        assert!(matches!(cont.kind, ExprKind::Continue));
    }

    #[test]
    fn while_loop_simple() {
        // FLS §6.8.2: `while cond {}` — loop terminates when condition is false.
        let src = "fn f() { while running {} }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected while as tail");
        let ExprKind::While { ref cond, ref body } = tail.kind else {
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
        let ExprKind::While { ref cond, ref body } = tail.kind else {
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
    fn for_loop_simple() {
        // FLS §6.8.3: `for x in iter {}` — for loop over an iterator.
        let src = "fn f() { for x in items {} }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind else { panic!("expected Fn item") };
        let body = f.body.as_ref().unwrap();
        let tail = body.tail.as_ref().expect("expected for as tail");
        let ExprKind::For { ref pat, ref iter, ref body } = tail.kind else {
            panic!("expected For, got {:?}", tail.kind);
        };
        assert_eq!(pat.text(src), "x");
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
        assert_eq!(pat.text(src), "item");
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
}
