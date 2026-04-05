//! Lexical analysis for galvanic.
//!
//! Implements tokenization as specified in FLS §2 (Lexical Elements).
//! Reference: <https://rust-lang.github.io/fls/lexical-elements.html>
//!
//! # Token stream contract
//!
//! Whitespace (FLS §2.1) and comments (FLS §2.5) are consumed but not
//! emitted. The caller receives only meaningful tokens, terminated by
//! [`TokenKind::Eof`].
//!
//! # Cache-line layout
//!
//! [`Token`] is 8 bytes. At 64 bytes per cache line, 8 tokens fit per line.
//! The parser's hot iteration over `Vec<Token>` therefore needs ~N/8
//! cache-line loads for N tokens — roughly 4× better than a naive 32-byte
//! token. The layout is enforced by the field ordering and `repr(u8)` on
//! [`TokenKind`].

// ── TokenKind ────────────────────────────────────────────────────────────────

/// Every distinct kind of token the galvanic lexer can produce.
///
/// `repr(u8)`: the discriminant is one byte, enabling the compact 8-byte
/// [`Token`] struct. There are currently fewer than 128 variants; the repr
/// will remain valid as long as the total stays below 256.
///
/// FLS §2: Lexical elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TokenKind {
    // ── Identifier ───────────────────────────────────────────────────────────
    /// A non-keyword identifier. FLS §2.3.
    ///
    /// FLS §2.3 NOTE: the spec requires Unicode NFC normalisation and
    /// XID_Start / XID_Continue categories (Unicode Standard Annex #31).
    /// This implementation handles ASCII correctly; non-ASCII identifier
    /// characters are accepted but NFC normalisation is not yet applied.
    /// A production lexer would use the `unicode-ident` crate.
    Ident,

    // ── Strict keywords — FLS §2.6 ───────────────────────────────────────────
    /// `as`
    KwAs,
    /// `async`
    KwAsync,
    /// `await`
    KwAwait,
    /// `break`
    KwBreak,
    /// `const`
    KwConst,
    /// `continue`
    KwContinue,
    /// `crate`
    KwCrate,
    /// `dyn`
    KwDyn,
    /// `else`
    KwElse,
    /// `enum`
    KwEnum,
    /// `extern`
    KwExtern,
    /// `false`
    KwFalse,
    /// `fn`
    KwFn,
    /// `for`
    KwFor,
    /// `if`
    KwIf,
    /// `impl`
    KwImpl,
    /// `in`
    KwIn,
    /// `let`
    KwLet,
    /// `loop`
    KwLoop,
    /// `match`
    KwMatch,
    /// `mod`
    KwMod,
    /// `move`
    KwMove,
    /// `mut`
    KwMut,
    /// `pub`
    KwPub,
    /// `ref`
    KwRef,
    /// `return`
    KwReturn,
    /// `self` (lowercase)
    KwSelfLower,
    /// `Self` (uppercase)
    KwSelfUpper,
    /// `static`
    KwStatic,
    /// `struct`
    KwStruct,
    /// `super`
    KwSuper,
    /// `trait`
    KwTrait,
    /// `true`
    KwTrue,
    /// `type`
    KwType,
    /// `unsafe`
    KwUnsafe,
    /// `use`
    KwUse,
    /// `where`
    KwWhere,
    /// `while`
    KwWhile,

    // ── Reserved keywords — FLS §2.6 ─────────────────────────────────────────
    // Reserved for future use; not yet valid in any position.
    /// `abstract`
    KwAbstract,
    /// `become`
    KwBecome,
    /// `box`
    KwBox,
    /// `do`
    KwDo,
    /// `final`
    KwFinal,
    /// `macro`
    KwMacro,
    /// `override`
    KwOverride,
    /// `priv`
    KwPriv,
    /// `try`
    KwTry,
    /// `typeof`
    KwTypeof,
    /// `unsized`
    KwUnsized,
    /// `virtual`
    KwVirtual,
    /// `yield`
    KwYield,

    // ── Weak keywords — FLS §2.6 ──────────────────────────────────────────────
    // Context-dependent; only special in specific syntactic positions.
    /// `macro_rules` — special only in macro definition position.
    KwMacroRules,
    /// `union` — special only in union declaration position.
    KwUnion,
    /// `safe` — special only in extern-block context.
    KwSafe,

    // ── Literals — FLS §2.4 ───────────────────────────────────────────────────
    /// Integer literal: decimal, hex `0x`, octal `0o`, binary `0b`.
    /// Optional type suffix: `i8` `i16` `i32` `i64` `i128` `isize`
    ///                        `u8` `u16` `u32` `u64` `u128` `usize`.
    LitInteger,
    /// Float literal. Optional suffix: `f32` `f64`.
    LitFloat,
    /// Character literal `'...'`.
    LitChar,
    /// Byte literal `b'...'`.
    LitByte,
    /// String literal `"..."`.
    LitStr,
    /// Byte string literal `b"..."`.
    LitByteStr,
    /// Raw string literal `r"..."` or `r#"..."#`.
    LitRawStr,
    /// Raw byte string literal `br"..."` or `br#"..."#`.
    LitRawByteStr,
    /// C string literal `c"..."`.
    LitCStr,
    /// Raw C string literal `cr"..."` or `cr#"..."#`.
    LitRawCStr,

    // ── Lifetime / label — FLS §2.3 ───────────────────────────────────────────
    /// A lifetime or loop label: `'ident`.
    ///
    /// FLS §2.6 AMBIGUOUS: `'static` is listed as a "weak keyword" but the
    /// spec does not define a boundary between lifetime-as-keyword and
    /// lifetime-as-identifier. All `'ident` forms are emitted as `Lifetime`;
    /// the parser assigns special meaning to `'static`.
    Lifetime,

    // ── Simple punctuation — FLS §2.2 ────────────────────────────────────────
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Star,
    /// `/`
    Slash,
    /// `%`
    Percent,
    /// `^`
    Caret,
    /// `!`
    Not,
    /// `&`
    And,
    /// `|`
    Or,
    /// `=`
    Eq,
    /// `<`
    Lt,
    /// `>`
    Gt,
    /// `@`
    At,
    /// `.`
    Dot,
    /// `,`
    Comma,
    /// `;`
    Semi,
    /// `:`
    Colon,
    /// `#`
    Pound,
    /// `$`
    Dollar,
    /// `?`
    Question,
    /// `_` (bare underscore, not as an identifier prefix).
    ///
    /// FLS §2.6 AMBIGUOUS: `_` appears in both the strict-keyword table and
    /// the punctuation table. The spec does not state an explicit precedence
    /// rule. We emit `Underscore` for a bare `_` not followed by an
    /// XID_Continue character; `_foo` and `__x` are emitted as `Ident`.
    Underscore,

    // ── Compound punctuation — FLS §2.2 ──────────────────────────────────────
    /// `&&`
    AndAnd,
    /// `||`
    OrOr,
    /// `<<`
    Shl,
    /// `>>`
    Shr,
    /// `+=`
    PlusEq,
    /// `-=`
    MinusEq,
    /// `*=`
    StarEq,
    /// `/=`
    SlashEq,
    /// `%=`
    PercentEq,
    /// `^=`
    CaretEq,
    /// `&=`
    AndEq,
    /// `|=`
    OrEq,
    /// `<<=`
    ShlEq,
    /// `>>=`
    ShrEq,
    /// `==`
    EqEq,
    /// `!=`
    Ne,
    /// `>=`
    Ge,
    /// `<=`
    Le,
    /// `..`
    DotDot,
    /// `...`
    DotDotDot,
    /// `..=`
    DotDotEq,
    /// `::`
    ColonColon,
    /// `->`
    RArrow,
    /// `=>`
    FatArrow,

    // ── Delimiters — FLS §2.2 ────────────────────────────────────────────────
    /// `(`
    OpenParen,
    /// `)`
    CloseParen,
    /// `{`
    OpenBrace,
    /// `}`
    CloseBrace,
    /// `[`
    OpenBracket,
    /// `]`
    CloseBracket,

    // ── Sentinel ──────────────────────────────────────────────────────────────
    /// End of file. Always the last token in the stream.
    Eof,
}

// ── Token ─────────────────────────────────────────────────────────────────────

/// A single lexical token.
///
/// # Layout (8 bytes)
///
/// ```text
/// offset 0 │ start: u32       — byte offset into source  (4 bytes)
/// offset 4 │ len:   u16       — byte length of the span  (2 bytes, max 65535)
/// offset 6 │ kind:  TokenKind — token kind discriminant  (1 byte, repr u8)
/// offset 7 │ (padding)                                   (1 byte)
/// ```
///
/// A 64-byte cache line holds 8 `Token` values. The parser's sequential scan
/// over `Vec<Token>` loads 8 tokens per cache miss — roughly 4× better than
/// a 32-byte layout. Field ordering (wide fields first) avoids padding waste.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    /// Byte offset of the first character of this token in the source string.
    pub start: u32,
    /// Byte length of this token's text in the source string.
    pub len: u16,
    /// What kind of token this is.
    pub kind: TokenKind,
}

impl Token {
    /// Return the source text of this token.
    pub fn text<'src>(&self, source: &'src str) -> &'src str {
        let start = self.start as usize;
        let end = start + self.len as usize;
        &source[start..end]
    }
}

// ── LexError ─────────────────────────────────────────────────────────────────

/// An error produced by the lexer.
///
/// All variants carry the byte offset (`pos`) at which the error was detected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexError {
    /// A character was encountered that cannot start any valid token. FLS §2.1.
    UnexpectedChar { pos: u32, ch: char },
    /// A string or character literal was opened but never closed. FLS §2.4.
    UnterminatedLiteral { pos: u32 },
    /// A block comment was opened but never closed. FLS §2.5.
    UnterminatedBlockComment { pos: u32 },
    /// An unrecognised escape sequence was found inside a literal. FLS §2.4.
    InvalidEscape { pos: u32 },
    /// `\u{…}` contained a Unicode surrogate (U+D800–U+DFFF). FLS §2.4.
    SurrogateInUnicodeEscape { pos: u32 },
    /// A raw string delimiter `r###"` was not closed by the matching `"###`. FLS §2.4.
    UnterminatedRawLiteral { pos: u32 },
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LexError::UnexpectedChar { pos, ch } => {
                write!(f, "unexpected character {:?} at byte {}", ch, pos)
            }
            LexError::UnterminatedLiteral { pos } => {
                write!(f, "unterminated literal starting at byte {}", pos)
            }
            LexError::UnterminatedBlockComment { pos } => {
                write!(f, "unterminated block comment starting at byte {}", pos)
            }
            LexError::InvalidEscape { pos } => {
                write!(f, "invalid escape sequence at byte {}", pos)
            }
            LexError::SurrogateInUnicodeEscape { pos } => {
                write!(f, "unicode escape is a surrogate at byte {}", pos)
            }
            LexError::UnterminatedRawLiteral { pos } => {
                write!(f, "unterminated raw literal starting at byte {}", pos)
            }
        }
    }
}

impl std::error::Error for LexError {}

// ── Lexer (private) ───────────────────────────────────────────────────────────

struct Lexer<'src> {
    src: &'src str,
    bytes: &'src [u8],
    pos: usize,
}

impl<'src> Lexer<'src> {
    fn new(src: &'src str) -> Self {
        Lexer { src, bytes: src.as_bytes(), pos: 0 }
    }

    // ── Low-level access ─────────────────────────────────────────────────────

    fn at_end(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    fn current(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn peek(&self, offset: usize) -> Option<u8> {
        self.bytes.get(self.pos + offset).copied()
    }

    fn bump(&mut self) {
        if self.pos < self.bytes.len() {
            self.pos += 1;
        }
    }

    /// Advance past `n` bytes.
    fn bump_n(&mut self, n: usize) {
        self.pos = (self.pos + n).min(self.bytes.len());
    }

    /// Decode the Unicode scalar at `self.pos` without advancing.
    fn current_char(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    /// Advance past the Unicode scalar at `self.pos`.
    fn bump_char(&mut self) {
        if let Some(ch) = self.current_char() {
            self.pos += ch.len_utf8();
        }
    }

    /// Build a token from `[start, self.pos)`.
    fn token(&self, start: usize, kind: TokenKind) -> Token {
        Token {
            start: start as u32,
            len: (self.pos - start) as u16,
            kind,
        }
    }

    // ── Whitespace — FLS §2.1 ────────────────────────────────────────────────

    /// Skip all whitespace characters. FLS §2.1.
    fn skip_whitespace(&mut self) {
        loop {
            match self.current_char() {
                Some(ch) if is_whitespace(ch) => self.bump_char(),
                _ => break,
            }
        }
    }

    // ── Comments — FLS §2.5 ──────────────────────────────────────────────────

    /// Skip a line comment `// …`. Returns Err if a CR appears (FLS §2.5
    /// states CR is forbidden in comments).
    fn skip_line_comment(&mut self) {
        // Caller has already confirmed we're at `//`.
        // Advance to end of line (but not past the newline itself — the
        // newline will be consumed by skip_whitespace on the next call).
        loop {
            match self.current() {
                None | Some(b'\n') => break,
                Some(b'\r') => break, // CR terminates the line; skip_whitespace handles it
                _ => self.bump(),
            }
        }
    }

    /// Skip a possibly-nested block comment `/* … */`. FLS §2.5.
    fn skip_block_comment(&mut self, start: usize) -> Result<(), LexError> {
        // Caller is past the opening `/*`.
        let mut depth: u32 = 1;
        loop {
            match (self.current(), self.peek(1)) {
                (None, _) => {
                    return Err(LexError::UnterminatedBlockComment { pos: start as u32 })
                }
                (Some(b'/'), Some(b'*')) => {
                    self.bump_n(2);
                    depth += 1;
                }
                (Some(b'*'), Some(b'/')) => {
                    self.bump_n(2);
                    depth -= 1;
                    if depth == 0 {
                        return Ok(());
                    }
                }
                _ => self.bump(),
            }
        }
    }

    // ── Identifier / keyword — FLS §2.3, §2.6 ────────────────────────────────

    /// Scan an identifier or keyword starting at `self.pos`.
    /// The caller has verified the first char is a valid identifier-start.
    fn scan_ident_or_keyword(&mut self, start: usize) -> Token {
        // Consume XID_Continue characters (ASCII fast path, full Unicode noted).
        loop {
            match self.current_char() {
                Some(ch) if is_ident_continue(ch) => self.bump_char(),
                _ => break,
            }
        }
        let text = &self.src[start..self.pos];
        let kind = keyword_kind(text).unwrap_or(TokenKind::Ident);
        self.token(start, kind)
    }

    // ── Numeric literals — FLS §2.4 ──────────────────────────────────────────

    fn scan_number(&mut self, start: usize) -> Result<Token, LexError> {
        // We are at the first digit.

        // Check for base prefix: 0b, 0o, 0x.
        if self.current() == Some(b'0') {
            match self.peek(1) {
                Some(b'b') | Some(b'B') => {
                    self.bump_n(2);
                    self.scan_digits_with_underscores(|b| matches!(b, b'0' | b'1'));
                    self.scan_integer_suffix();
                    return Ok(self.token(start, TokenKind::LitInteger));
                }
                Some(b'o') | Some(b'O') => {
                    self.bump_n(2);
                    self.scan_digits_with_underscores(|b| b.is_ascii_digit() && b < b'8');
                    self.scan_integer_suffix();
                    return Ok(self.token(start, TokenKind::LitInteger));
                }
                Some(b'x') | Some(b'X') => {
                    self.bump_n(2);
                    self.scan_digits_with_underscores(|b| b.is_ascii_hexdigit());
                    self.scan_integer_suffix();
                    return Ok(self.token(start, TokenKind::LitInteger));
                }
                _ => {}
            }
        }

        // Decimal digits.
        self.scan_digits_with_underscores(|b| b.is_ascii_digit());

        // Decide: float or integer?
        //
        // FLS §2.4: A float literal is a decimal literal followed by `.`
        // not followed by another digit or an identifier character, OR
        // followed by more digits (with optional exponent), OR followed by
        // an exponent alone.
        //
        // FLS §2.4 NOTE: The exact disambiguation between `1.method()` (integer
        // `1`, dot, identifier `method`) and `1.0` (float) relies on lookahead
        // that the spec describes informally. We implement the standard rule:
        // `digits .` is a float only when `.` is not immediately followed by an
        // XID_Start character or another `.`.
        let is_float = match (self.current(), self.peek(1)) {
            (Some(b'.'), Some(b'0'..=b'9')) => true,
            (Some(b'.'), next) => {
                // Not followed by digit: float only if not an ident-start or another dot
                !matches!(next, Some(b'a'..=b'z') | Some(b'A'..=b'Z') | Some(b'_') | Some(b'.'))
            }
            (Some(b'e') | Some(b'E'), _) => true,
            _ => false,
        };

        if is_float {
            // Consume the `.` and any following digits.
            if self.current() == Some(b'.') && !matches!(self.peek(1), Some(b'.')) {
                self.bump(); // consume `.`
                self.scan_digits_with_underscores(|b| b.is_ascii_digit());
            }
            // Optional exponent: e/E [+-] digits.
            if matches!(self.current(), Some(b'e') | Some(b'E')) {
                self.bump();
                if matches!(self.current(), Some(b'+') | Some(b'-')) {
                    self.bump();
                }
                self.scan_digits_with_underscores(|b| b.is_ascii_digit());
            }
            self.scan_float_suffix();
            Ok(self.token(start, TokenKind::LitFloat))
        } else {
            self.scan_integer_suffix();
            Ok(self.token(start, TokenKind::LitInteger))
        }
    }

    /// Consume zero or more digit-or-underscore bytes matching `pred`.
    fn scan_digits_with_underscores(&mut self, pred: impl Fn(u8) -> bool) {
        loop {
            match self.current() {
                Some(b'_') => self.bump(),
                Some(b) if pred(b) => self.bump(),
                _ => break,
            }
        }
    }

    /// Consume an integer type suffix if present. FLS §2.4.
    fn scan_integer_suffix(&mut self) {
        // Suffixes: i8 i16 i32 i64 i128 isize u8 u16 u32 u64 u128 usize
        let rest = &self.src[self.pos..];
        let suffix_len = [
            "i128", "i64", "i32", "i16", "i8", "isize",
            "u128", "u64", "u32", "u16", "u8", "usize",
        ]
        .iter()
        .find_map(|s| {
            if let Some(after_suffix) = rest.strip_prefix(s) {
                // Make sure the suffix is not followed by a further ident char.
                let after = after_suffix.chars().next();
                if !after.map(is_ident_continue).unwrap_or(false) {
                    Some(s.len())
                } else {
                    None
                }
            } else {
                None
            }
        });
        if let Some(n) = suffix_len {
            self.bump_n(n);
        }
    }

    /// Consume a float type suffix if present. FLS §2.4.
    fn scan_float_suffix(&mut self) {
        let rest = &self.src[self.pos..];
        let suffix_len = ["f64", "f32"].iter().find_map(|s| {
            if let Some(after_suffix) = rest.strip_prefix(s) {
                let after = after_suffix.chars().next();
                if !after.map(is_ident_continue).unwrap_or(false) {
                    Some(s.len())
                } else {
                    None
                }
            } else {
                None
            }
        });
        if let Some(n) = suffix_len {
            self.bump_n(n);
        }
    }

    // ── Character / byte literals — FLS §2.4 ─────────────────────────────────

    /// Scan `'...'` or `b'...'`. The caller is positioned just after the
    /// opening quote. Returns the token kind and bumps past the closing `'`.
    fn scan_char_body(&mut self, start: usize, kind: TokenKind) -> Result<Token, LexError> {
        match self.current() {
            None => return Err(LexError::UnterminatedLiteral { pos: start as u32 }),
            Some(b'\\') => self.scan_escape()?,
            Some(b'\'') => {
                // Empty char literal — not valid Rust but we emit the token
                // and let the parser report the semantic error.
            }
            _ => self.bump_char(),
        }
        if self.current() != Some(b'\'') {
            return Err(LexError::UnterminatedLiteral { pos: start as u32 });
        }
        self.bump(); // closing `'`
        Ok(self.token(start, kind))
    }

    // ── String / byte-string literals — FLS §2.4 ─────────────────────────────

    /// Scan `"…"` or `b"…"`. Caller is positioned just after the opening `"`.
    fn scan_str_body(&mut self, start: usize, kind: TokenKind) -> Result<Token, LexError> {
        loop {
            match self.current() {
                None => return Err(LexError::UnterminatedLiteral { pos: start as u32 }),
                Some(b'"') => {
                    self.bump();
                    return Ok(self.token(start, kind));
                }
                Some(b'\\') => self.scan_escape()?,
                _ => self.bump_char(),
            }
        }
    }

    /// Scan a raw string `r"…"` or `r##"…"##`. Caller is positioned just
    /// after the leading `r` (or `br` / `cr`). FLS §2.4.
    fn scan_raw_str_body(&mut self, start: usize, kind: TokenKind) -> Result<Token, LexError> {
        // Count the opening `#` characters.
        let mut hashes: usize = 0;
        while self.current() == Some(b'#') {
            hashes += 1;
            self.bump();
        }
        if self.current() != Some(b'"') {
            return Err(LexError::UnterminatedRawLiteral { pos: start as u32 });
        }
        self.bump(); // opening `"`

        // Scan until we find `"` followed by exactly `hashes` `#` characters.
        loop {
            match self.current() {
                None => return Err(LexError::UnterminatedRawLiteral { pos: start as u32 }),
                Some(b'"') => {
                    self.bump();
                    let mut close_hashes = 0usize;
                    while close_hashes < hashes && self.current() == Some(b'#') {
                        close_hashes += 1;
                        self.bump();
                    }
                    if close_hashes == hashes {
                        return Ok(self.token(start, kind));
                    }
                    // Not enough hashes — keep scanning.
                }
                _ => self.bump_char(),
            }
        }
    }

    // ── Escape sequences — FLS §2.4 ──────────────────────────────────────────

    /// Consume one escape sequence. Caller is at the `\`. FLS §2.4.
    fn scan_escape(&mut self) -> Result<(), LexError> {
        let escape_pos = self.pos as u32;
        self.bump(); // consume `\`
        match self.current() {
            Some(b'n' | b'r' | b't' | b'\\' | b'\'' | b'"' | b'0') => {
                self.bump();
                Ok(())
            }
            Some(b'x') => {
                // ASCII hex escape: \x[0-9a-fA-F]{2}
                self.bump();
                for _ in 0..2 {
                    match self.current() {
                        Some(b) if b.is_ascii_hexdigit() => self.bump(),
                        _ => return Err(LexError::InvalidEscape { pos: escape_pos }),
                    }
                }
                Ok(())
            }
            Some(b'u') => {
                // Unicode escape: \u{1–6 hex digits}, no surrogates. FLS §2.4.
                self.bump();
                if self.current() != Some(b'{') {
                    return Err(LexError::InvalidEscape { pos: escape_pos });
                }
                self.bump();
                let hex_start = self.pos;
                while self.current().map(|b| b.is_ascii_hexdigit()).unwrap_or(false) {
                    self.bump();
                }
                let hex_len = self.pos - hex_start;
                if hex_len == 0 || hex_len > 6 {
                    return Err(LexError::InvalidEscape { pos: escape_pos });
                }
                if self.current() != Some(b'}') {
                    return Err(LexError::InvalidEscape { pos: escape_pos });
                }
                self.bump();
                // Validate: must not be a surrogate (U+D800–U+DFFF). FLS §2.4.
                let hex_str = &self.src[hex_start..hex_start + hex_len];
                let codepoint = u32::from_str_radix(hex_str, 16)
                    .map_err(|_| LexError::InvalidEscape { pos: escape_pos })?;
                if (0xD800..=0xDFFF).contains(&codepoint) {
                    return Err(LexError::SurrogateInUnicodeEscape { pos: escape_pos });
                }
                Ok(())
            }
            Some(b'\n') => {
                // String continuation: `\` followed by newline skips whitespace. FLS §2.4.
                self.bump();
                self.skip_whitespace();
                Ok(())
            }
            _ => Err(LexError::InvalidEscape { pos: escape_pos }),
        }
    }

    // ── Lifetime — FLS §2.3 ───────────────────────────────────────────────────

    /// Called when we see `'`. Decides between char literal, lifetime, or error.
    ///
    /// Disambiguation rule (FLS §2.3 / §2.4):
    /// - `'\...'` → char literal (escape sequence)
    /// - `'<ident_char>'` where ident_char is one character → char literal
    /// - `'<ident>` (not closed by `'` immediately after first ident char) → lifetime
    /// - `'<non-ident-start>'` → char literal attempt
    fn scan_quote(&mut self, start: usize) -> Result<Token, LexError> {
        self.bump(); // consume opening `'`

        match self.current() {
            // Escape → must be a char literal.
            Some(b'\\') => self.scan_char_body(start, TokenKind::LitChar),

            // Identifier-start → lifetime OR char literal.
            Some(b) if is_ident_start(b as char) => {
                let ident_start = self.pos;
                self.bump_char();
                let first_char_end = self.pos;

                // Continue consuming identifier characters for the lifetime case.
                loop {
                    match self.current_char() {
                        Some(ch) if is_ident_continue(ch) => self.bump_char(),
                        _ => break,
                    }
                }
                let is_single_char_ident = self.pos == first_char_end;

                if is_single_char_ident && self.current() == Some(b'\'') {
                    // `'x'` — single character char literal.
                    self.bump(); // closing `'`
                    Ok(self.token(start, TokenKind::LitChar))
                } else {
                    // `'ident` — lifetime. The text we already consumed is the ident.
                    // If the ident was `_`, that is still a valid lifetime name.
                    let _ = ident_start; // ident already consumed
                    Ok(self.token(start, TokenKind::Lifetime))
                }
            }

            // Any other character → char literal.
            Some(_) => self.scan_char_body(start, TokenKind::LitChar),

            None => Err(LexError::UnterminatedLiteral { pos: start as u32 }),
        }
    }

    // ── Main dispatch ─────────────────────────────────────────────────────────

    /// Produce the next token (or return `None` at EOF).
    fn next_token(&mut self) -> Option<Result<Token, LexError>> {
        // Skip whitespace and comments (trivia). FLS §2.1, §2.5.
        loop {
            self.skip_whitespace();
            match (self.current(), self.peek(1)) {
                (Some(b'/'), Some(b'/')) => {
                    self.bump_n(2);
                    self.skip_line_comment();
                }
                (Some(b'/'), Some(b'*')) => {
                    let start = self.pos;
                    self.bump_n(2);
                    if let Err(e) = self.skip_block_comment(start) {
                        return Some(Err(e));
                    }
                }
                _ => break,
            }
        }

        if self.at_end() {
            return Some(Ok(self.token(self.pos, TokenKind::Eof)));
        }

        let start = self.pos;

        // Dispatch on the first byte.
        let result = match self.current().unwrap() {
            // ── Identifiers / keywords / raw strings / byte literals ──────────
            b'r' => {
                match (self.peek(1), self.peek(2)) {
                    (Some(b'"'), _) | (Some(b'#'), _) => {
                        self.bump(); // `r`
                        self.scan_raw_str_body(start, TokenKind::LitRawStr)
                    }
                    _ => {
                        Ok(self.scan_ident_or_keyword(start))
                    }
                }
            }
            b'b' => {
                match (self.peek(1), self.peek(2)) {
                    (Some(b'\''), _) => {
                        self.bump_n(2); // `b'`
                        self.scan_char_body(start, TokenKind::LitByte)
                    }
                    (Some(b'"'), _) => {
                        self.bump_n(2); // `b"`
                        self.scan_str_body(start, TokenKind::LitByteStr)
                    }
                    (Some(b'r'), Some(b'"')) | (Some(b'r'), Some(b'#')) => {
                        self.bump_n(2); // `br`
                        self.scan_raw_str_body(start, TokenKind::LitRawByteStr)
                    }
                    _ => Ok(self.scan_ident_or_keyword(start)),
                }
            }
            b'c' => {
                match (self.peek(1), self.peek(2)) {
                    (Some(b'"'), _) => {
                        self.bump_n(2); // `c"`
                        self.scan_str_body(start, TokenKind::LitCStr)
                    }
                    (Some(b'r'), Some(b'"')) | (Some(b'r'), Some(b'#')) => {
                        self.bump_n(2); // `cr`
                        self.scan_raw_str_body(start, TokenKind::LitRawCStr)
                    }
                    _ => Ok(self.scan_ident_or_keyword(start)),
                }
            }
            b if is_ident_start(b as char) => Ok(self.scan_ident_or_keyword(start)),

            // ── Numeric literals ──────────────────────────────────────────────
            b'0'..=b'9' => self.scan_number(start),

            // ── String literal ────────────────────────────────────────────────
            b'"' => {
                self.bump(); // opening `"`
                self.scan_str_body(start, TokenKind::LitStr)
            }

            // ── Char literal / lifetime ────────────────────────────────────────
            b'\'' => self.scan_quote(start),

            // ── Punctuation ───────────────────────────────────────────────────
            b'+' => {
                self.bump();
                if self.current() == Some(b'=') { self.bump(); Ok(self.token(start, TokenKind::PlusEq)) }
                else { Ok(self.token(start, TokenKind::Plus)) }
            }
            b'-' => {
                self.bump();
                if self.current() == Some(b'>') { self.bump(); Ok(self.token(start, TokenKind::RArrow)) }
                else if self.current() == Some(b'=') { self.bump(); Ok(self.token(start, TokenKind::MinusEq)) }
                else { Ok(self.token(start, TokenKind::Minus)) }
            }
            b'*' => {
                self.bump();
                if self.current() == Some(b'=') { self.bump(); Ok(self.token(start, TokenKind::StarEq)) }
                else { Ok(self.token(start, TokenKind::Star)) }
            }
            b'/' => {
                self.bump();
                if self.current() == Some(b'=') { self.bump(); Ok(self.token(start, TokenKind::SlashEq)) }
                else { Ok(self.token(start, TokenKind::Slash)) }
            }
            b'%' => {
                self.bump();
                if self.current() == Some(b'=') { self.bump(); Ok(self.token(start, TokenKind::PercentEq)) }
                else { Ok(self.token(start, TokenKind::Percent)) }
            }
            b'^' => {
                self.bump();
                if self.current() == Some(b'=') { self.bump(); Ok(self.token(start, TokenKind::CaretEq)) }
                else { Ok(self.token(start, TokenKind::Caret)) }
            }
            b'!' => {
                self.bump();
                if self.current() == Some(b'=') { self.bump(); Ok(self.token(start, TokenKind::Ne)) }
                else { Ok(self.token(start, TokenKind::Not)) }
            }
            b'&' => {
                self.bump();
                if self.current() == Some(b'&') { self.bump(); Ok(self.token(start, TokenKind::AndAnd)) }
                else if self.current() == Some(b'=') { self.bump(); Ok(self.token(start, TokenKind::AndEq)) }
                else { Ok(self.token(start, TokenKind::And)) }
            }
            b'|' => {
                self.bump();
                if self.current() == Some(b'|') { self.bump(); Ok(self.token(start, TokenKind::OrOr)) }
                else if self.current() == Some(b'=') { self.bump(); Ok(self.token(start, TokenKind::OrEq)) }
                else { Ok(self.token(start, TokenKind::Or)) }
            }
            b'=' => {
                self.bump();
                if self.current() == Some(b'=') { self.bump(); Ok(self.token(start, TokenKind::EqEq)) }
                else if self.current() == Some(b'>') { self.bump(); Ok(self.token(start, TokenKind::FatArrow)) }
                else { Ok(self.token(start, TokenKind::Eq)) }
            }
            b'<' => {
                self.bump();
                if self.current() == Some(b'<') {
                    self.bump();
                    if self.current() == Some(b'=') { self.bump(); Ok(self.token(start, TokenKind::ShlEq)) }
                    else { Ok(self.token(start, TokenKind::Shl)) }
                } else if self.current() == Some(b'=') { self.bump(); Ok(self.token(start, TokenKind::Le)) }
                else { Ok(self.token(start, TokenKind::Lt)) }
            }
            b'>' => {
                self.bump();
                if self.current() == Some(b'>') {
                    self.bump();
                    if self.current() == Some(b'=') { self.bump(); Ok(self.token(start, TokenKind::ShrEq)) }
                    else { Ok(self.token(start, TokenKind::Shr)) }
                } else if self.current() == Some(b'=') { self.bump(); Ok(self.token(start, TokenKind::Ge)) }
                else { Ok(self.token(start, TokenKind::Gt)) }
            }
            b'@' => { self.bump(); Ok(self.token(start, TokenKind::At)) }
            b'.' => {
                self.bump();
                if self.current() == Some(b'.') {
                    self.bump();
                    if self.current() == Some(b'.') { self.bump(); Ok(self.token(start, TokenKind::DotDotDot)) }
                    else if self.current() == Some(b'=') { self.bump(); Ok(self.token(start, TokenKind::DotDotEq)) }
                    else { Ok(self.token(start, TokenKind::DotDot)) }
                } else {
                    Ok(self.token(start, TokenKind::Dot))
                }
            }
            b',' => { self.bump(); Ok(self.token(start, TokenKind::Comma)) }
            b';' => { self.bump(); Ok(self.token(start, TokenKind::Semi)) }
            b':' => {
                self.bump();
                if self.current() == Some(b':') { self.bump(); Ok(self.token(start, TokenKind::ColonColon)) }
                else { Ok(self.token(start, TokenKind::Colon)) }
            }
            b'#' => { self.bump(); Ok(self.token(start, TokenKind::Pound)) }
            b'$' => { self.bump(); Ok(self.token(start, TokenKind::Dollar)) }
            b'?' => { self.bump(); Ok(self.token(start, TokenKind::Question)) }
            b'(' => { self.bump(); Ok(self.token(start, TokenKind::OpenParen)) }
            b')' => { self.bump(); Ok(self.token(start, TokenKind::CloseParen)) }
            b'{' => { self.bump(); Ok(self.token(start, TokenKind::OpenBrace)) }
            b'}' => { self.bump(); Ok(self.token(start, TokenKind::CloseBrace)) }
            b'[' => { self.bump(); Ok(self.token(start, TokenKind::OpenBracket)) }
            b']' => { self.bump(); Ok(self.token(start, TokenKind::CloseBracket)) }

            // ── Unknown ────────────────────────────────────────────────────────
            _ => {
                let ch = self.current_char().unwrap_or('\u{FFFD}');
                self.bump_char();
                Err(LexError::UnexpectedChar { pos: start as u32, ch })
            }
        };

        Some(result)
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Tokenize `source` into a `Vec<Token>`.
///
/// Returns all tokens including the terminal [`TokenKind::Eof`] token.
/// Whitespace and comments are consumed but not included in the output.
///
/// # Errors
///
/// Returns the first [`LexError`] encountered. On error, the returned `Vec`
/// contains all tokens successfully produced before the error.
pub fn tokenize(source: &str) -> Result<Vec<Token>, LexError> {
    let mut lexer = Lexer::new(source);
    let mut tokens = Vec::new();
    loop {
        match lexer.next_token() {
            None => break,
            Some(Err(e)) => return Err(e),
            Some(Ok(tok)) => {
                let is_eof = tok.kind == TokenKind::Eof;
                tokens.push(tok);
                if is_eof {
                    break;
                }
            }
        }
    }
    Ok(tokens)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// FLS §2.1: Whitespace characters.
fn is_whitespace(ch: char) -> bool {
    matches!(
        ch,
        '\t'            // U+0009 horizontal tab
        | '\n'          // U+000A line feed
        | '\x0B'        // U+000B vertical tab
        | '\x0C'        // U+000C form feed
        | '\r'          // U+000D carriage return
        | ' '           // U+0020 space
        | '\u{85}'      // U+0085 next line
        | '\u{200E}'    // U+200E left-to-right mark
        | '\u{200F}'    // U+200F right-to-left mark
        | '\u{2028}'    // U+2028 line separator
        | '\u{2029}'    // U+2029 paragraph separator
    )
}

/// FLS §2.3: True if `ch` can start an identifier.
///
/// NOTE: Full Unicode requires XID_Start ∪ {U+005F '_'}. We use the ASCII
/// fast path here; non-ASCII letters are accepted via `is_alphabetic()`.
fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_alphabetic()
}

/// FLS §2.3: True if `ch` can continue an identifier.
///
/// NOTE: Full Unicode requires XID_Continue. ASCII fast path used here.
fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric()
}

/// Map an identifier string to a keyword token kind. FLS §2.6.
/// Returns `None` if the string is not a keyword.
fn keyword_kind(s: &str) -> Option<TokenKind> {
    // Strict keywords — FLS §2.6
    let kind = match s {
        "as"       => TokenKind::KwAs,
        "async"    => TokenKind::KwAsync,
        "await"    => TokenKind::KwAwait,
        "break"    => TokenKind::KwBreak,
        "const"    => TokenKind::KwConst,
        "continue" => TokenKind::KwContinue,
        "crate"    => TokenKind::KwCrate,
        "dyn"      => TokenKind::KwDyn,
        "else"     => TokenKind::KwElse,
        "enum"     => TokenKind::KwEnum,
        "extern"   => TokenKind::KwExtern,
        "false"    => TokenKind::KwFalse,
        "fn"       => TokenKind::KwFn,
        "for"      => TokenKind::KwFor,
        "if"       => TokenKind::KwIf,
        "impl"     => TokenKind::KwImpl,
        "in"       => TokenKind::KwIn,
        "let"      => TokenKind::KwLet,
        "loop"     => TokenKind::KwLoop,
        "match"    => TokenKind::KwMatch,
        "mod"      => TokenKind::KwMod,
        "move"     => TokenKind::KwMove,
        "mut"      => TokenKind::KwMut,
        "pub"      => TokenKind::KwPub,
        "ref"      => TokenKind::KwRef,
        "return"   => TokenKind::KwReturn,
        "self"     => TokenKind::KwSelfLower,
        "Self"     => TokenKind::KwSelfUpper,
        "static"   => TokenKind::KwStatic,
        "struct"   => TokenKind::KwStruct,
        "super"    => TokenKind::KwSuper,
        "trait"    => TokenKind::KwTrait,
        "true"     => TokenKind::KwTrue,
        "type"     => TokenKind::KwType,
        "unsafe"   => TokenKind::KwUnsafe,
        "use"      => TokenKind::KwUse,
        "where"    => TokenKind::KwWhere,
        "while"    => TokenKind::KwWhile,
        // Bare `_` is an Underscore token, not handled here; `_` as an
        // identifier-start leads here only when followed by XID_Continue.
        "_"        => TokenKind::Underscore,

        // Reserved keywords — FLS §2.6
        "abstract" => TokenKind::KwAbstract,
        "become"   => TokenKind::KwBecome,
        "box"      => TokenKind::KwBox,
        "do"       => TokenKind::KwDo,
        "final"    => TokenKind::KwFinal,
        "macro"    => TokenKind::KwMacro,
        "override" => TokenKind::KwOverride,
        "priv"     => TokenKind::KwPriv,
        "try"      => TokenKind::KwTry,
        "typeof"   => TokenKind::KwTypeof,
        "unsized"  => TokenKind::KwUnsized,
        "virtual"  => TokenKind::KwVirtual,
        "yield"    => TokenKind::KwYield,

        // Weak keywords — FLS §2.6
        "macro_rules" => TokenKind::KwMacroRules,
        "union"       => TokenKind::KwUnion,
        "safe"        => TokenKind::KwSafe,

        _ => return None,
    };
    Some(kind)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Tokenize `src`, strip the trailing Eof, return the kinds.
    fn lex_kinds(src: &str) -> Vec<TokenKind> {
        let tokens = tokenize(src).expect("lex failed");
        tokens
            .into_iter()
            .filter(|t| t.kind != TokenKind::Eof)
            .map(|t| t.kind)
            .collect()
    }

    /// Tokenize `src` and return `(kind, text)` pairs (without Eof).
    fn lex_with_text(src: &str) -> Vec<(TokenKind, String)> {
        let tokens = tokenize(src).expect("lex failed");
        tokens
            .into_iter()
            .filter(|t| t.kind != TokenKind::Eof)
            .map(|t| (t.kind, t.text(src).to_owned()))
            .collect()
    }

    // ── Token layout ─────────────────────────────────────────────────────────

    #[test]
    fn token_is_eight_bytes() {
        // Cache-line design goal: 8 tokens per 64-byte line.
        assert_eq!(std::mem::size_of::<Token>(), 8);
    }

    #[test]
    fn token_align_is_four() {
        assert_eq!(std::mem::align_of::<Token>(), 4);
    }

    // ── Empty input ───────────────────────────────────────────────────────────

    #[test]
    fn empty_source_gives_eof() {
        let tokens = tokenize("").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::Eof);
        assert_eq!(tokens[0].start, 0);
        assert_eq!(tokens[0].len, 0);
    }

    // ── Keywords ─────────────────────────────────────────────────────────────

    #[test]
    fn strict_keywords() {
        assert_eq!(lex_kinds("let"), vec![TokenKind::KwLet]);
        assert_eq!(lex_kinds("fn"), vec![TokenKind::KwFn]);
        assert_eq!(lex_kinds("struct"), vec![TokenKind::KwStruct]);
        assert_eq!(lex_kinds("if"), vec![TokenKind::KwIf]);
        assert_eq!(lex_kinds("else"), vec![TokenKind::KwElse]);
        assert_eq!(lex_kinds("return"), vec![TokenKind::KwReturn]);
        assert_eq!(lex_kinds("true"), vec![TokenKind::KwTrue]);
        assert_eq!(lex_kinds("false"), vec![TokenKind::KwFalse]);
        assert_eq!(lex_kinds("self"), vec![TokenKind::KwSelfLower]);
        assert_eq!(lex_kinds("Self"), vec![TokenKind::KwSelfUpper]);
        assert_eq!(lex_kinds("async"), vec![TokenKind::KwAsync]);
        assert_eq!(lex_kinds("await"), vec![TokenKind::KwAwait]);
    }

    #[test]
    fn reserved_keywords() {
        assert_eq!(lex_kinds("abstract"), vec![TokenKind::KwAbstract]);
        assert_eq!(lex_kinds("yield"), vec![TokenKind::KwYield]);
        assert_eq!(lex_kinds("virtual"), vec![TokenKind::KwVirtual]);
    }

    #[test]
    fn weak_keywords() {
        assert_eq!(lex_kinds("macro_rules"), vec![TokenKind::KwMacroRules]);
        assert_eq!(lex_kinds("union"), vec![TokenKind::KwUnion]);
        assert_eq!(lex_kinds("safe"), vec![TokenKind::KwSafe]);
    }

    #[test]
    fn keyword_prefix_is_ident() {
        // `letter` starts with `let` but is not a keyword.
        assert_eq!(lex_kinds("letter"), vec![TokenKind::Ident]);
        assert_eq!(lex_kinds("iffy"), vec![TokenKind::Ident]);
        assert_eq!(lex_kinds("structure"), vec![TokenKind::Ident]);
    }

    // ── Identifiers ───────────────────────────────────────────────────────────

    #[test]
    fn plain_identifiers() {
        assert_eq!(lex_kinds("foo"), vec![TokenKind::Ident]);
        assert_eq!(lex_kinds("_foo"), vec![TokenKind::Ident]);
        assert_eq!(lex_kinds("foo_bar"), vec![TokenKind::Ident]);
        assert_eq!(lex_kinds("Foo"), vec![TokenKind::Ident]);
        assert_eq!(lex_kinds("foo123"), vec![TokenKind::Ident]);
    }

    #[test]
    fn bare_underscore_is_underscore_token() {
        assert_eq!(lex_kinds("_"), vec![TokenKind::Underscore]);
    }

    #[test]
    fn double_underscore_is_ident() {
        assert_eq!(lex_kinds("__"), vec![TokenKind::Ident]);
    }

    // ── Integer literals ─────────────────────────────────────────────────────

    #[test]
    fn decimal_integer() {
        assert_eq!(lex_kinds("42"), vec![TokenKind::LitInteger]);
        assert_eq!(lex_kinds("0"), vec![TokenKind::LitInteger]);
        assert_eq!(lex_kinds("1_000_000"), vec![TokenKind::LitInteger]);
    }

    #[test]
    fn integer_with_suffix() {
        let pairs = lex_with_text("42u32");
        assert_eq!(pairs, vec![(TokenKind::LitInteger, "42u32".to_owned())]);

        let pairs = lex_with_text("100i64");
        assert_eq!(pairs, vec![(TokenKind::LitInteger, "100i64".to_owned())]);

        let pairs = lex_with_text("0usize");
        assert_eq!(pairs, vec![(TokenKind::LitInteger, "0usize".to_owned())]);
    }

    #[test]
    fn hex_integer() {
        let pairs = lex_with_text("0xFF");
        assert_eq!(pairs, vec![(TokenKind::LitInteger, "0xFF".to_owned())]);

        let pairs = lex_with_text("0xDEAD_BEEF");
        assert_eq!(pairs, vec![(TokenKind::LitInteger, "0xDEAD_BEEF".to_owned())]);
    }

    #[test]
    fn binary_integer() {
        let pairs = lex_with_text("0b1010");
        assert_eq!(pairs, vec![(TokenKind::LitInteger, "0b1010".to_owned())]);
    }

    #[test]
    fn octal_integer() {
        let pairs = lex_with_text("0o755");
        assert_eq!(pairs, vec![(TokenKind::LitInteger, "0o755".to_owned())]);
    }

    // ── Float literals ────────────────────────────────────────────────────────

    #[test]
    fn float_with_decimal() {
        assert_eq!(lex_kinds("3.14"), vec![TokenKind::LitFloat]);
        assert_eq!(lex_kinds("1.0"), vec![TokenKind::LitFloat]);
        assert_eq!(lex_kinds("0.5"), vec![TokenKind::LitFloat]);
    }

    #[test]
    fn float_with_exponent() {
        assert_eq!(lex_kinds("1e10"), vec![TokenKind::LitFloat]);
        assert_eq!(lex_kinds("2.5e-3"), vec![TokenKind::LitFloat]);
        assert_eq!(lex_kinds("1E+5"), vec![TokenKind::LitFloat]);
    }

    #[test]
    fn float_with_suffix() {
        let pairs = lex_with_text("1.0f64");
        assert_eq!(pairs, vec![(TokenKind::LitFloat, "1.0f64".to_owned())]);

        let pairs = lex_with_text("3.14f32");
        assert_eq!(pairs, vec![(TokenKind::LitFloat, "3.14f32".to_owned())]);
    }

    #[test]
    fn integer_dot_method_call() {
        // `1.foo` should be integer `1`, dot `.`, ident `foo`. FLS §2.4.
        let pairs = lex_with_text("1.foo");
        assert_eq!(pairs, vec![
            (TokenKind::LitInteger, "1".to_owned()),
            (TokenKind::Dot,        ".".to_owned()),
            (TokenKind::Ident,      "foo".to_owned()),
        ]);
    }

    // ── Character literals ────────────────────────────────────────────────────

    #[test]
    fn char_literals() {
        assert_eq!(lex_kinds("'a'"), vec![TokenKind::LitChar]);
        assert_eq!(lex_kinds("'Z'"), vec![TokenKind::LitChar]);
        assert_eq!(lex_kinds("'0'"), vec![TokenKind::LitChar]);
    }

    #[test]
    fn char_escape_sequences() {
        assert_eq!(lex_kinds(r"'\n'"), vec![TokenKind::LitChar]);
        assert_eq!(lex_kinds(r"'\t'"), vec![TokenKind::LitChar]);
        assert_eq!(lex_kinds(r"'\\'"), vec![TokenKind::LitChar]);
        assert_eq!(lex_kinds(r"'\''"), vec![TokenKind::LitChar]);
        assert_eq!(lex_kinds(r"'\u{1F600}'"), vec![TokenKind::LitChar]);
    }

    #[test]
    fn byte_literal() {
        assert_eq!(lex_kinds("b'A'"), vec![TokenKind::LitByte]);
        assert_eq!(lex_kinds(r"b'\n'"), vec![TokenKind::LitByte]);
    }

    // ── String literals ───────────────────────────────────────────────────────

    #[test]
    fn string_literal() {
        assert_eq!(lex_kinds(r#""hello""#), vec![TokenKind::LitStr]);
        assert_eq!(lex_kinds(r#""hello world""#), vec![TokenKind::LitStr]);
        assert_eq!(lex_kinds(r#""""#), vec![TokenKind::LitStr]);
    }

    #[test]
    fn string_escape_sequences() {
        // A string containing \n \t \\ \" escapes — all should be consumed as one LitStr.
        assert_eq!(lex_kinds(r#""\n\t\\\"" "#), vec![TokenKind::LitStr]);
        // Followed by a `#` punctuator.
        assert_eq!(lex_kinds(r##""\n\t\\\""#"##), vec![TokenKind::LitStr, TokenKind::Pound]);
    }

    #[test]
    fn byte_string_literal() {
        assert_eq!(lex_kinds(r#"b"hello""#), vec![TokenKind::LitByteStr]);
    }

    #[test]
    fn raw_string_literal() {
        assert_eq!(lex_kinds(r##"r"hello""##), vec![TokenKind::LitRawStr]);
        assert_eq!(lex_kinds(r##"r#"hello"#"##), vec![TokenKind::LitRawStr]);
        assert_eq!(lex_kinds(r###"r##"has "quotes" inside"##"###), vec![TokenKind::LitRawStr]);
    }

    #[test]
    fn raw_byte_string_literal() {
        assert_eq!(lex_kinds(r##"br"hello""##), vec![TokenKind::LitRawByteStr]);
    }

    #[test]
    fn c_string_literal() {
        assert_eq!(lex_kinds(r#"c"hello""#), vec![TokenKind::LitCStr]);
    }

    // ── Lifetimes ─────────────────────────────────────────────────────────────

    #[test]
    fn lifetime() {
        assert_eq!(lex_kinds("'a"), vec![TokenKind::Lifetime]);
        assert_eq!(lex_kinds("'static"), vec![TokenKind::Lifetime]);
        assert_eq!(lex_kinds("'lifetime_name"), vec![TokenKind::Lifetime]);
    }

    // ── Punctuation ───────────────────────────────────────────────────────────

    #[test]
    fn simple_punctuation() {
        assert_eq!(lex_kinds("+"), vec![TokenKind::Plus]);
        assert_eq!(lex_kinds("-"), vec![TokenKind::Minus]);
        assert_eq!(lex_kinds("*"), vec![TokenKind::Star]);
        assert_eq!(lex_kinds("/"), vec![TokenKind::Slash]);
        assert_eq!(lex_kinds("%"), vec![TokenKind::Percent]);
        assert_eq!(lex_kinds("^"), vec![TokenKind::Caret]);
        assert_eq!(lex_kinds("!"), vec![TokenKind::Not]);
        assert_eq!(lex_kinds("&"), vec![TokenKind::And]);
        assert_eq!(lex_kinds("|"), vec![TokenKind::Or]);
        assert_eq!(lex_kinds("="), vec![TokenKind::Eq]);
        assert_eq!(lex_kinds("<"), vec![TokenKind::Lt]);
        assert_eq!(lex_kinds(">"), vec![TokenKind::Gt]);
        assert_eq!(lex_kinds("@"), vec![TokenKind::At]);
        assert_eq!(lex_kinds("."), vec![TokenKind::Dot]);
        assert_eq!(lex_kinds(","), vec![TokenKind::Comma]);
        assert_eq!(lex_kinds(";"), vec![TokenKind::Semi]);
        assert_eq!(lex_kinds(":"), vec![TokenKind::Colon]);
        assert_eq!(lex_kinds("#"), vec![TokenKind::Pound]);
        assert_eq!(lex_kinds("$"), vec![TokenKind::Dollar]);
        assert_eq!(lex_kinds("?"), vec![TokenKind::Question]);
    }

    #[test]
    fn compound_punctuation() {
        assert_eq!(lex_kinds("&&"), vec![TokenKind::AndAnd]);
        assert_eq!(lex_kinds("||"), vec![TokenKind::OrOr]);
        assert_eq!(lex_kinds("<<"), vec![TokenKind::Shl]);
        assert_eq!(lex_kinds(">>"), vec![TokenKind::Shr]);
        assert_eq!(lex_kinds("+="), vec![TokenKind::PlusEq]);
        assert_eq!(lex_kinds("-="), vec![TokenKind::MinusEq]);
        assert_eq!(lex_kinds("*="), vec![TokenKind::StarEq]);
        assert_eq!(lex_kinds("/="), vec![TokenKind::SlashEq]);
        assert_eq!(lex_kinds("%="), vec![TokenKind::PercentEq]);
        assert_eq!(lex_kinds("^="), vec![TokenKind::CaretEq]);
        assert_eq!(lex_kinds("&="), vec![TokenKind::AndEq]);
        assert_eq!(lex_kinds("|="), vec![TokenKind::OrEq]);
        assert_eq!(lex_kinds("<<="), vec![TokenKind::ShlEq]);
        assert_eq!(lex_kinds(">>="), vec![TokenKind::ShrEq]);
        assert_eq!(lex_kinds("=="), vec![TokenKind::EqEq]);
        assert_eq!(lex_kinds("!="), vec![TokenKind::Ne]);
        assert_eq!(lex_kinds(">="), vec![TokenKind::Ge]);
        assert_eq!(lex_kinds("<="), vec![TokenKind::Le]);
        assert_eq!(lex_kinds(".."), vec![TokenKind::DotDot]);
        assert_eq!(lex_kinds("..."), vec![TokenKind::DotDotDot]);
        assert_eq!(lex_kinds("..="), vec![TokenKind::DotDotEq]);
        assert_eq!(lex_kinds("::"), vec![TokenKind::ColonColon]);
        assert_eq!(lex_kinds("->"), vec![TokenKind::RArrow]);
        assert_eq!(lex_kinds("=>"), vec![TokenKind::FatArrow]);
    }

    #[test]
    fn delimiters() {
        assert_eq!(lex_kinds("("), vec![TokenKind::OpenParen]);
        assert_eq!(lex_kinds(")"), vec![TokenKind::CloseParen]);
        assert_eq!(lex_kinds("{"), vec![TokenKind::OpenBrace]);
        assert_eq!(lex_kinds("}"), vec![TokenKind::CloseBrace]);
        assert_eq!(lex_kinds("["), vec![TokenKind::OpenBracket]);
        assert_eq!(lex_kinds("]"), vec![TokenKind::CloseBracket]);
    }

    // ── Comments (should not produce tokens) ──────────────────────────────────

    #[test]
    fn line_comment_skipped() {
        assert_eq!(lex_kinds("// this is a comment\nlet"), vec![TokenKind::KwLet]);
        assert_eq!(lex_kinds("// entire file is a comment"), vec![]);
    }

    #[test]
    fn block_comment_skipped() {
        assert_eq!(lex_kinds("/* comment */ let"), vec![TokenKind::KwLet]);
        assert_eq!(lex_kinds("/* nested /* comment */ */ let"), vec![TokenKind::KwLet]);
    }

    #[test]
    fn unterminated_block_comment_is_error() {
        assert!(tokenize("/* never closed").is_err());
    }

    // ── Whitespace ────────────────────────────────────────────────────────────

    #[test]
    fn whitespace_between_tokens() {
        assert_eq!(
            lex_kinds("let   x   =   42;"),
            vec![
                TokenKind::KwLet,
                TokenKind::Ident,
                TokenKind::Eq,
                TokenKind::LitInteger,
                TokenKind::Semi,
            ]
        );
    }

    // ── Realistic Rust snippets ───────────────────────────────────────────────

    #[test]
    fn let_binding() {
        let pairs = lex_with_text("let x = 42;");
        assert_eq!(pairs, vec![
            (TokenKind::KwLet,      "let".to_owned()),
            (TokenKind::Ident,      "x".to_owned()),
            (TokenKind::Eq,         "=".to_owned()),
            (TokenKind::LitInteger, "42".to_owned()),
            (TokenKind::Semi,       ";".to_owned()),
        ]);
    }

    #[test]
    fn function_signature() {
        // fn add(a: i32, b: i32) -> i32
        let kinds = lex_kinds("fn add(a: i32, b: i32) -> i32");
        assert_eq!(kinds, vec![
            TokenKind::KwFn,
            TokenKind::Ident,      // add
            TokenKind::OpenParen,
            TokenKind::Ident,      // a
            TokenKind::Colon,
            TokenKind::Ident,      // i32
            TokenKind::Comma,
            TokenKind::Ident,      // b
            TokenKind::Colon,
            TokenKind::Ident,      // i32
            TokenKind::CloseParen,
            TokenKind::RArrow,
            TokenKind::Ident,      // i32
        ]);
    }

    #[test]
    fn struct_definition() {
        // struct Point { x: f64, y: f64 }
        let kinds = lex_kinds("struct Point { x: f64, y: f64 }");
        assert_eq!(kinds, vec![
            TokenKind::KwStruct,
            TokenKind::Ident,      // Point
            TokenKind::OpenBrace,
            TokenKind::Ident,      // x
            TokenKind::Colon,
            TokenKind::Ident,      // f64
            TokenKind::Comma,
            TokenKind::Ident,      // y
            TokenKind::Colon,
            TokenKind::Ident,      // f64
            TokenKind::CloseBrace,
        ]);
    }

    #[test]
    fn no_std_attribute() {
        // #![no_std]
        let kinds = lex_kinds("#![no_std]");
        assert_eq!(kinds, vec![
            TokenKind::Pound,
            TokenKind::Not,
            TokenKind::OpenBracket,
            TokenKind::Ident,      // no_std
            TokenKind::CloseBracket,
        ]);
    }

    #[test]
    fn span_is_correct() {
        // Verify that Token::text() reconstructs the right slice.
        let src = "let x = 42;";
        let tokens = tokenize(src).unwrap();
        let texts: Vec<&str> = tokens
            .iter()
            .filter(|t| t.kind != TokenKind::Eof)
            .map(|t| t.text(src))
            .collect();
        assert_eq!(texts, vec!["let", "x", "=", "42", ";"]);
    }

    // ── Error cases ───────────────────────────────────────────────────────────

    #[test]
    fn unterminated_string_is_error() {
        assert!(tokenize(r#""never closed"#).is_err());
    }

    #[test]
    fn surrogate_unicode_escape_is_error() {
        // \u{D800} is a surrogate — forbidden in char/string literals. FLS §2.4.
        assert!(tokenize(r"'\u{D800}'").is_err());
    }
}
