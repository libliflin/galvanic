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
//! 11. Unary `-` `!` `*` `&` `&mut`
//! 12. Primary: literals, paths, calls, `(expr)`, blocks, `if`, `return`
//!
//! FLS §6 NOTE: The FLS does not assign numeric precedence levels. Precedence
//! is encoded in the grammar structure. This ordering follows the Rust
//! reference and is consistent with rustc's behaviour.

use crate::ast::{
    BinOp, Block, Expr, ExprKind, FnDef, Item, ItemKind, Param, SourceFile, Span, Stmt,
    StmtKind, Ty, TyKind, UnaryOp,
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
    /// FLS §3: Item kinds. Only function items (`fn`) are implemented.
    fn parse_item(&mut self) -> Result<Item, ParseError> {
        let start = self.current_span();

        match self.peek_kind() {
            TokenKind::KwFn => {
                let fn_def = self.parse_fn_def()?;
                let end = fn_def
                    .body
                    .as_ref()
                    .map(|b| b.span)
                    .unwrap_or(start);
                let span = start.to(end);
                Ok(Item { kind: ItemKind::Fn(Box::new(fn_def)), span })
            }
            kind => Err(self.error(format!(
                "expected item (fn, …), found {kind:?}"
            ))),
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
    fn parse_fn_def(&mut self) -> Result<FnDef, ParseError> {
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

        Ok(FnDef { name, params, ret_ty, body })
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
            ExprKind::Block(_) | ExprKind::If { .. }
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

    /// Assignment — FLS §6.9. Right-associative.
    ///
    /// FLS §6.9 NOTE: Compound assignment operators (`+=`, `-=`, …) are
    /// not yet handled. The spec treats them as distinct expression forms
    /// from plain `=` assignment.
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

    /// Multiplicative operators `*` `/` `%` — FLS §6.5. Left-associative.
    fn parse_multiplicative(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_unary()?;

        loop {
            let op = match self.peek_kind() {
                TokenKind::Star => BinOp::Mul,
                TokenKind::Slash => BinOp::Div,
                TokenKind::Percent => BinOp::Rem,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_unary()?;
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

            _ => self.parse_primary(),
        }
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
                let path_expr = Expr {
                    kind: ExprKind::Path(segments),
                    span: start.to(path_end),
                };

                // Call expression: path immediately followed by `(`.
                if self.peek_kind() == TokenKind::OpenParen {
                    self.parse_call(path_expr)
                } else {
                    Ok(path_expr)
                }
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
    use crate::ast::{ExprKind, ItemKind, StmtKind, TyKind};
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
        let ItemKind::Fn(ref f) = sf.items[0].kind;
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
        let ItemKind::Fn(ref f) = sf.items[0].kind;
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
        let ItemKind::Fn(ref f) = sf.items[0].kind;
        assert_eq!(f.params.len(), 2);
        assert_eq!(f.params[0].name.text(src), "a");
        assert_eq!(f.params[1].name.text(src), "b");
    }

    #[test]
    fn fn_trailing_comma_in_params() {
        // FLS §9.2: trailing comma in parameter list is allowed.
        let src = "fn f(x: i32,) {}";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind;
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
        let ItemKind::Fn(ref f) = sf.items[0].kind;
        assert!(matches!(f.ret_ty.as_ref().unwrap().kind, TyKind::Unit));
    }

    #[test]
    fn type_ref() {
        // FLS §4.8: reference type `&i32`.
        let src = "fn f(x: &i32) {}";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind;
        assert!(matches!(f.params[0].ty.kind, TyKind::Ref { mutable: false, .. }));
    }

    #[test]
    fn type_mut_ref() {
        // FLS §4.8: mutable reference type `&mut i32`.
        let src = "fn f(x: &mut i32) {}";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind;
        assert!(matches!(f.params[0].ty.kind, TyKind::Ref { mutable: true, .. }));
    }

    // ── Let statements ────────────────────────────────────────────────────────

    #[test]
    fn let_with_init() {
        // FLS §8.1: let with initializer.
        let src = "fn f() { let x = 42; }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind;
        let body = f.body.as_ref().unwrap();
        assert_eq!(body.stmts.len(), 1);
        assert!(matches!(body.stmts[0].kind, StmtKind::Let { .. }));
    }

    #[test]
    fn let_with_type_and_init() {
        // FLS §8.1: let with type annotation and initializer.
        let src = "fn f() { let x: i32 = 42; }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind;
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
        let ItemKind::Fn(ref f) = sf.items[0].kind;
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
        let ItemKind::Fn(ref f) = sf.items[0].kind;
        let body = f.body.as_ref().unwrap();
        assert!(body.stmts.is_empty());
        assert!(matches!(body.tail.as_ref().unwrap().kind, ExprKind::LitInt(42)));
    }

    #[test]
    fn binary_add() {
        // FLS §6.5: arithmetic addition.
        let src = "fn f() -> i32 { a + b }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind;
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::Binary { op: BinOp::Add, .. }));
    }

    #[test]
    fn operator_precedence_mul_over_add() {
        // FLS §6.5: `*` binds tighter than `+`.
        // `1 + 2 * 3` should parse as `1 + (2 * 3)`.
        let src = "fn f() -> i32 { 1 + 2 * 3 }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind;
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
        let ItemKind::Fn(ref f) = sf.items[0].kind;
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
        let ItemKind::Fn(ref f) = sf.items[0].kind;
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
        let ItemKind::Fn(ref f) = sf.items[0].kind;
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
        let ItemKind::Fn(ref f) = sf.items[0].kind;
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
        let ItemKind::Fn(ref f) = sf.items[0].kind;
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
        let ItemKind::Fn(ref f) = sf.items[0].kind;
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::If { else_expr: Some(_), .. }));
    }

    #[test]
    fn if_else_if_chain() {
        // FLS §6.11: else-if chain.
        let src = "fn f() -> i32 { if a { 1 } else if b { 2 } else { 3 } }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind;
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
        let ItemKind::Fn(ref f) = sf.items[0].kind;
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::Unit));
    }

    #[test]
    fn boolean_literals() {
        // FLS §6.1.3: boolean literals.
        let src = "fn f() -> bool { true }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind;
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::LitBool(true)));
    }

    #[test]
    fn unary_negate() {
        // FLS §6.4.1: unary negation.
        let src = "fn f() -> i32 { -1 }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind;
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::Unary { op: UnaryOp::Neg, .. }));
    }

    #[test]
    fn borrow_expression() {
        // FLS §6.4.4: shared borrow `&x`.
        let src = "fn f(x: i32) -> &i32 { &x }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind;
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::Unary { op: UnaryOp::Ref, .. }));
    }

    #[test]
    fn integer_literal_hex() {
        // FLS §2.4: hex integer literal.
        let src = "fn f() -> i32 { 0xFF }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind;
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::LitInt(255)));
    }

    #[test]
    fn integer_literal_with_suffix() {
        // FLS §2.4: integer suffix is stripped before value parsing.
        let src = "fn f() -> u32 { 42u32 }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind;
        let tail = f.body.as_ref().unwrap().tail.as_ref().unwrap();
        assert!(matches!(tail.kind, ExprKind::LitInt(42)));
    }

    #[test]
    fn full_function() {
        // Integration: function with params, local binding, and tail expression.
        let src = "fn add(a: i32, b: i32) -> i32 { let sum = a + b; sum }";
        let sf = parse_ok(src);
        let ItemKind::Fn(ref f) = sf.items[0].kind;
        assert_eq!(f.params.len(), 2);
        let body = f.body.as_ref().unwrap();
        assert_eq!(body.stmts.len(), 1); // the let
        assert!(body.tail.is_some()); // `sum`
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
}
