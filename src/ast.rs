//! Abstract syntax tree for galvanic.
//!
// All fields are consumed by the parser's test suite and will be used by later
// compiler phases (name resolution, type checking, codegen). Suppress the
// dead-code lint for this module rather than sprinkling allows throughout.
#![allow(dead_code)]
//!
//! Each node type corresponds to a section of the Ferrocene Language
//! Specification (FLS). Citations are embedded in the type documentation.
//!
//! # Cache-line design note
//!
//! AST nodes currently use `Box<T>` for recursive types. `Box` means every
//! recursive field dereference is a potential cache miss — the child node may
//! live anywhere on the heap. An arena design (`u32` indices into a flat
//! `Vec<ExprData>`) would keep sequential traversal in cache. That redesign is
//! flagged here as future work; the research value of the first implementation
//! is in getting the FLS mapping right, not in premature optimization.
//!
//! The one place where layout *is* controlled today is [`Span`]: 8 bytes,
//! two per cache line slot alongside a `Token`. All other structs accept
//! Rust's default layout for now.

// ── Span ─────────────────────────────────────────────────────────────────────

/// A byte-range span into the source text.
///
/// Spans connect AST nodes back to source locations for diagnostics.
///
/// # Layout (8 bytes)
///
/// ```text
/// offset 0 │ start: u32  — first byte of the span  (4 bytes)
/// offset 4 │ len:   u32  — byte count of the span  (4 bytes)
/// ```
///
/// FLS §1: source text is a sequence of Unicode scalar values encoded in UTF-8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Byte offset of the first character.
    pub start: u32,
    /// Byte length of the span.
    pub len: u32,
}

impl Span {
    pub fn new(start: u32, len: u32) -> Self {
        Span { start, len }
    }

    /// Extend this span to cover everything up to and including `other`.
    ///
    /// `other` must lie at or after `self` in the source text.
    pub fn to(self, other: Span) -> Span {
        let end = other.start + other.len;
        Span {
            start: self.start,
            len: end.saturating_sub(self.start),
        }
    }

    /// Return the source text covered by this span.
    pub fn text<'src>(&self, src: &'src str) -> &'src str {
        let start = self.start as usize;
        &src[start..start + self.len as usize]
    }
}

// ── Source file ───────────────────────────────────────────────────────────────

/// The top-level source file — a sequence of items.
///
/// FLS §18.1: A Rust source file is a sequence of Unicode scalar values
/// in UTF-8 encoding. At the syntactic level it consists of a (possibly
/// empty) list of items.
///
/// FLS §3: Items are the top-level constituents of a crate.
#[derive(Debug)]
pub struct SourceFile {
    pub items: Vec<Item>,
    pub span: Span,
}

// ── Items ─────────────────────────────────────────────────────────────────────

/// A top-level item.
///
/// FLS §3: An item is a component of a crate. Items can be nested inside
/// modules. The parser currently handles only function items.
#[derive(Debug)]
pub struct Item {
    pub kind: ItemKind,
    pub span: Span,
}

/// The kind of a top-level item.
///
/// FLS §3: item kinds include functions, structs, enums, unions, traits,
/// implementations, type aliases, constants, statics, use declarations,
/// and extern blocks. `Fn`, `Struct`, `Enum`, `Impl`, `Trait`, and `Const`
/// are implemented here.
#[derive(Debug)]
pub enum ItemKind {
    /// A function definition. FLS §9.
    Fn(Box<FnDef>),
    /// A struct definition. FLS §14.
    Struct(Box<StructDef>),
    /// An enum definition. FLS §15.
    Enum(Box<EnumDef>),
    /// An impl block (inherent or trait). FLS §11.
    ///
    /// `impl TypeName { methods… }` — inherent impl.
    /// `impl TraitName for TypeName { methods… }` — trait impl.
    Impl(Box<ImplDef>),
    /// A trait definition. FLS §13.
    ///
    /// `trait TraitName { fn method_sig(&self) -> Type; }` — defines a trait.
    /// Trait definitions are parsed but not yet used for type checking.
    /// They drive static dispatch via `impl Trait for Type`.
    Trait(Box<TraitDef>),
    /// A constant item. FLS §7.1.
    ///
    /// `const NAME: Type = expr;` — names a compile-time constant value.
    /// Every use of a constant is replaced with its value (FLS §7.1:10).
    Const(Box<ConstDef>),
    /// A static item. FLS §7.2.
    ///
    /// `static NAME: Type = expr;` — names a value with a fixed memory address.
    /// All references to a static refer to the same memory location (FLS §7.2:15).
    /// Unlike constants, statics are not substituted inline; they reside in the
    /// data section and are loaded via an address reference at runtime.
    ///
    /// Cache-line note: a static occupies one 8-byte slot in the `.data` section
    /// (one slot per half cache line). Reading it costs an ADRP + ADD + LDR
    /// sequence (12 bytes in the instruction stream), whereas a `const` costs
    /// only a single MOV (4 bytes). This is the primary cache-line tradeoff
    /// documented in galvanic's design.
    Static(Box<StaticDef>),
}

/// A static item declaration.
///
/// FLS §7.2: Static items.
///
/// Grammar (abridged):
/// ```text
/// StaticDeclaration ::= "static" "mut"? Identifier ":" Type "=" Expression ";"
/// ```
///
/// FLS §7.2:15: "All references to a static refer to the same memory address."
/// FLS §7.2 AMBIGUOUS: The spec does not specify the data-section alignment for
/// statics. Galvanic aligns each static to 8 bytes (`.align 3`) matching the
/// 64-bit register width.
///
/// Cache-line note: each `StaticDef` is only allocated during parsing.
#[derive(Debug)]
pub struct StaticDef {
    /// The name of the static.
    pub name: Span,
    /// The declared type. Currently only `i32` is supported.
    pub ty: Ty,
    /// The initializer expression. Must be a constant expression (FLS §6.1.2).
    pub value: Expr,
    /// Whether this static is mutable (`static mut`).
    ///
    /// FLS §7.2: Mutable statics can only be accessed inside `unsafe` blocks
    /// (FLS §19). Galvanic parses `mut` but does not yet enforce the unsafe
    /// requirement — this is future work.
    pub mutable: bool,
    /// Span of the entire declaration including the trailing semicolon.
    pub span: Span,
}

/// A constant item declaration.
///
/// FLS §7.1: Constant items.
///
/// Grammar (abridged):
/// ```text
/// ConstantDeclaration ::= "const" Identifier ":" Type "=" Expression ";"
/// ```
///
/// Cache-line note: `ConstDef` is only allocated during parsing;
/// it is not on any hot lowering path.
#[derive(Debug)]
pub struct ConstDef {
    /// The name of the constant.
    pub name: Span,
    /// The declared type. Currently only `i32` is supported.
    pub ty: Ty,
    /// The initializer expression. Must be a constant expression (FLS §6.1.2).
    pub value: Expr,
    /// Span of the entire declaration including the trailing semicolon.
    pub span: Span,
}

// ── Functions ─────────────────────────────────────────────────────────────────

/// A function definition.
///
/// FLS §9: Functions.
///
/// Grammar (abridged — qualifiers and where clauses omitted):
/// ```text
/// FunctionDeclaration ::=
///     "fn" Identifier "(" FunctionParameters? ")"
///     FunctionReturnType?
///     BlockExpression
/// ```
///
/// FLS §9 AMBIGUOUS: the spec lists `FunctionQualifiers` (`const`, `async`,
/// `unsafe`, `extern`) but does not fully enumerate which qualifier
/// combinations are legal. For example, `const async fn` is currently
/// rejected by rustc but the FLS does not state this constraint explicitly.
/// This implementation accepts no qualifiers; they are left for a future cycle.
#[derive(Debug)]
pub struct FnDef {
    /// The item's visibility.
    ///
    /// FLS §10.2: Visibility determines where the function can be named.
    pub vis: Visibility,
    /// The function's name (span of the identifier token).
    pub name: Span,
    /// The optional `self` parameter (present in methods only).
    ///
    /// FLS §10.1: Methods are functions that have a `self` parameter.
    /// The `self` parameter is always first; regular parameters follow.
    pub self_param: Option<SelfKind>,
    /// The function's parameters (excluding `self`).
    pub params: Vec<Param>,
    /// The declared return type.
    ///
    /// FLS §9: "If no return type is specified, the return type is `()`."
    pub ret_ty: Option<Ty>,
    /// The function body.
    ///
    /// FLS §9: The body is required for non-trait, non-extern functions.
    /// `None` is reserved for future use in trait/extern contexts.
    pub body: Option<Block>,
}

/// The form of a `self` parameter in a method definition.
///
/// FLS §10.1: Associated functions. Methods take a special `self` parameter
/// as their first argument, which refers to the value on which the method is
/// invoked.
///
/// FLS §10.1 AMBIGUOUS: The FLS lists `self`, `&self`, `&mut self`, and
/// `mut self` as valid forms, but does not specify which are valid in all
/// impl contexts. This implementation supports `self`, `&self`, and `&mut
/// self`; `mut self` is treated as `self` (mutability of the binding).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfKind {
    /// `self` — takes ownership of the receiver.
    Val,
    /// `&self` — shared borrow of the receiver.
    Ref,
    /// `&mut self` — mutable borrow of the receiver.
    RefMut,
}

/// An impl block (inherent or trait).
///
/// FLS §11: Implementations. `impl Type { methods }` defines inherent methods
/// on a named type. `impl Trait for Type { methods }` implements a trait.
///
/// FLS §11 AMBIGUOUS: The spec allows `impl<T>` with generic parameters,
/// but the interaction between generics and impl resolution is complex.
/// Generic impls are future work.
#[derive(Debug)]
pub struct ImplDef {
    /// The type being implemented (always the struct/enum name).
    pub ty: Span,
    /// The trait being implemented, if any.
    ///
    /// `None` for inherent impls (`impl Foo { ... }`).
    /// `Some(span)` for trait impls (`impl Bar for Foo { ... }`),
    /// where the span refers to the trait name `Bar`.
    ///
    /// FLS §11.1: Trait implementations provide a concrete implementation
    /// of a trait's associated items for a named type.
    pub trait_name: Option<Span>,
    /// The methods defined in this impl block.
    pub methods: Vec<Box<FnDef>>,
    /// Span of the entire impl block.
    pub span: Span,
}

/// A trait definition.
///
/// FLS §13: Traits. `trait Name { fn method_sig(&self) -> Type; }` declares
/// a set of associated items that implementors must provide.
///
/// At this milestone, trait definitions are parsed and stored in the AST
/// but are not used for type checking. They enable `impl Trait for Type`
/// blocks to be parsed, and trait method calls resolve via static dispatch
/// using the same `TypeName__method_name` mangling as inherent methods.
///
/// FLS §13 AMBIGUOUS: The FLS does not specify a required order between
/// trait definition and its implementations within a crate; we assume the
/// standard Rust rule (trait must be defined before use in type checking,
/// but galvanic does not type-check at this milestone).
#[derive(Debug)]
pub struct TraitDef {
    /// The trait name (span of the identifier token).
    pub name: Span,
    /// Method signatures declared in the trait body.
    ///
    /// Each `FnDef` has `body: None` (the body is provided by implementors).
    /// FLS §13: A trait item may declare a method without a body.
    pub methods: Vec<Box<FnDef>>,
    /// Span of the entire trait definition.
    pub span: Span,
}

/// A function parameter.
///
/// FLS §9.2: A function parameter yields a set of bindings that bind matched
/// input values to names at the call site.
///
/// FLS §9.2 AMBIGUOUS: the spec allows arbitrary irrefutable patterns in
/// parameter position (e.g., `(a, b): (i32, i32)`, `_: i32`). The extent
/// of patterns valid in parameter position is not independently listed in §9
/// — the reader must cross-reference §5 (Patterns) and infer which patterns
/// are irrefutable. This implementation supports only `name: Type` and the
/// `self` family; full pattern parameters are future work.
#[derive(Debug)]
pub struct Param {
    /// The parameter name (simple identifier).
    pub name: Span,
    /// The declared type.
    pub ty: Ty,
    pub span: Span,
}

// ── Visibility ────────────────────────────────────────────────────────────────

/// Visibility of an item or field.
///
/// FLS §10.2: Visibility and Privacy.
///
/// FLS §10.2 NOTE: The FLS defines a fine-grained visibility system including
/// `pub(crate)`, `pub(super)`, and `pub(in path)`. This implementation handles
/// only the two common cases: private (default) and `pub`. Restricted
/// visibility forms are future work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    /// Default (private) visibility.
    Private,
    /// `pub` — visible everywhere in the crate tree.
    Pub,
}

// ── Structs ───────────────────────────────────────────────────────────────────

/// A struct definition.
///
/// FLS §14: Structs. A struct is a product type with named or positional
/// fields. Three forms are defined:
///
/// - Named-field struct: `struct Foo { x: i32, y: f64 }`
/// - Tuple struct: `struct Foo(i32, f64);`
/// - Unit struct: `struct Foo;`
///
/// FLS §14 AMBIGUOUS: The spec does not specify whether visibility on the
/// struct (`pub struct`) and visibility on individual fields interact with
/// name resolution in a well-defined way for all contexts. This
/// implementation records visibility but defers enforcement to a future
/// name-resolution phase.
#[derive(Debug)]
pub struct StructDef {
    /// The struct's visibility.
    pub vis: Visibility,
    /// The struct's name.
    pub name: Span,
    /// The struct's shape.
    pub kind: StructKind,
    /// The span of the entire struct definition.
    pub span: Span,
}

/// The three forms a struct body can take.
///
/// FLS §14: Struct body forms.
#[derive(Debug)]
pub enum StructKind {
    /// Named-field struct body: `{ field: Type, … }`.
    ///
    /// FLS §14.1: A struct with named fields.
    Named(Vec<NamedField>),
    /// Tuple-struct body: `(Type, …);`.
    ///
    /// FLS §14.2: A struct with positional fields.
    Tuple(Vec<TupleField>),
    /// Unit struct: no body, terminated by `;`.
    ///
    /// FLS §14.3: A struct with no fields.
    Unit,
}

/// A named struct field.
///
/// FLS §14.1: `VisibilityModifier? Identifier ":" Type`.
#[derive(Debug)]
pub struct NamedField {
    /// The field's visibility.
    pub vis: Visibility,
    /// The field name.
    pub name: Span,
    /// The field type.
    pub ty: Ty,
    pub span: Span,
}

/// A tuple struct field.
///
/// FLS §14.2: `VisibilityModifier? Type`.
#[derive(Debug)]
pub struct TupleField {
    /// The field's visibility.
    pub vis: Visibility,
    /// The field type.
    pub ty: Ty,
    pub span: Span,
}

// ── Enums ─────────────────────────────────────────────────────────────────────

/// An enum definition.
///
/// FLS §15: Enumerations. An enum is a sum type: a value is exactly one of
/// its variants at any given time.
///
/// Three variant forms are defined:
///
/// - Unit variant:        `Foo`
/// - Tuple variant:       `Foo(i32, f64)`
/// - Named-field variant: `Foo { x: i32, y: f64 }`
///
/// FLS §15 AMBIGUOUS: The spec does not specify whether visibility on the
/// enum itself (`pub enum`) interacts with visibility on individual variant
/// fields in all contexts. This implementation records visibility on the
/// enum but defers enforcement to a future name-resolution phase.
#[derive(Debug)]
pub struct EnumDef {
    /// The enum's visibility.
    pub vis: Visibility,
    /// The enum's name.
    pub name: Span,
    /// The enum's variants.
    pub variants: Vec<EnumVariant>,
    /// The span of the entire enum definition.
    pub span: Span,
}

/// A single variant of an enum.
///
/// FLS §15: EnumVariant.
#[derive(Debug)]
pub struct EnumVariant {
    /// The variant name.
    pub name: Span,
    /// The variant's shape.
    pub kind: EnumVariantKind,
    /// The span of the entire variant.
    pub span: Span,
}

/// The three forms an enum variant body can take.
///
/// FLS §15: Variant body forms.
#[derive(Debug)]
pub enum EnumVariantKind {
    /// Unit variant: no fields. FLS §15.1: `Identifier`.
    Unit,
    /// Tuple variant: positional fields. FLS §15.2: `Identifier "(" TupleField* ")"`.
    Tuple(Vec<TupleField>),
    /// Named-field variant. FLS §15.3: `Identifier "{" NamedField* "}"`.
    Named(Vec<NamedField>),
}

// ── Types ─────────────────────────────────────────────────────────────────────

/// A type expression.
///
/// FLS §4: Types. This initial implementation handles only the most common
/// forms: named types (paths), the unit type, and reference types.
#[derive(Debug)]
pub struct Ty {
    pub kind: TyKind,
    pub span: Span,
}

/// The kind of a type expression.
///
/// FLS §4: Type kinds.
///
/// Many type forms are not yet represented: tuple types (§4.4), array types
/// (§4.5), slice types (§4.6), function pointer types (§4.9), trait objects
/// (§4.10), impl-Trait types (§4.11), and generic type arguments (`Vec<i32>`).
#[derive(Debug)]
pub enum TyKind {
    /// A named type (a path). FLS §4.1, §14.
    ///
    /// Each element of the `Vec` is the span of one path segment identifier.
    /// Examples: `i32` → `[Span("i32")]`, `std::vec::Vec` → three spans.
    Path(Vec<Span>),

    /// The unit type `()`. FLS §4.4.
    ///
    /// FLS §4.4: The unit type has exactly one value, also written `()`.
    Unit,

    /// A reference type `&T` or `&mut T`. FLS §4.8.
    ///
    /// FLS §4.8 NOTE: References may carry a lifetime (`&'a T`). Lifetime
    /// parameters in type position are not yet parsed; they are future work.
    Ref {
        mutable: bool,
        inner: Box<Ty>,
    },
}

// ── Blocks ────────────────────────────────────────────────────────────────────

/// A block expression `{ stmts* expr? }`.
///
/// FLS §6.10: A block expression sequences statements and evaluates to the
/// value of its tail expression (if present) or to `()` otherwise.
///
/// FLS §6.10 NOTE: The spec says the tail expression is the *last element*
/// of the block when it is an expression without a trailing semicolon. This
/// requires the parser to distinguish `expr;` (statement) from `expr` (tail)
/// at the syntactic level — the distinction is purely syntactic.
#[derive(Debug)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    /// The tail expression — the block's value. Absent means the block is `()`.
    pub tail: Option<Box<Expr>>,
    pub span: Span,
}

// ── Statements ────────────────────────────────────────────────────────────────

/// A statement.
///
/// FLS §8: A statement is a component of a block expression.
#[derive(Debug)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

/// The kind of a statement.
///
/// FLS §8: Statement kinds include empty statements, item statements,
/// let statements, and expression statements.
#[derive(Debug)]
pub enum StmtKind {
    /// A let binding. FLS §8.1.
    ///
    /// Grammar: `"let" Pattern (":" Type)? ("=" Expression)? ";"`
    ///
    /// FLS §8.1: The pattern may be any irrefutable pattern. Common forms:
    /// - `let x = expr;` — identifier pattern, binds `x`.
    /// - `let _ = expr;` — wildcard pattern, discards.
    /// - `let (a, b) = tuple;` — tuple pattern (FLS §5.10.3), destructures.
    Let {
        pat: Pat,
        ty: Option<Ty>,
        init: Option<Box<Expr>>,
    },

    /// An expression followed by `;`. FLS §8.3.
    ///
    /// FLS §8.3: An expression statement evaluates an expression and discards
    /// the result. The result type is not constrained to `()`.
    Expr(Box<Expr>),

    /// An empty statement (lone `;`). FLS §8.2.
    Empty,
}

// ── Expressions ───────────────────────────────────────────────────────────────

/// An expression.
///
/// FLS §6: Expressions.
///
/// # Cache-line note
///
/// `Expr` uses `Box<Expr>` in recursive variants (`Binary`, `Unary`, etc.).
/// This means each recursive dereference is a potential cache miss. An
/// arena-based design — all `ExprKind` data in a flat `Vec`, addressed by
/// `u32` indices — would be more cache-friendly for tree traversal. The
/// trade-off is API complexity. This is flagged as a future redesign.
#[derive(Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

/// The kind of an expression.
///
/// FLS §6: Expression kinds.
#[derive(Debug)]
pub enum ExprKind {
    /// An integer literal. FLS §6.1.1.
    ///
    /// The value is stored as `u128` (the widest integer type). Type-checking
    /// will narrow it to the inferred or suffixed type.
    ///
    /// FLS §6.1.1 NOTE: the spec says integer literals must not exceed the
    /// maximum value of their type, but this constraint is not enforced at
    /// the lexical or parse level — it is a type-checking concern.
    LitInt(u128),

    /// A float literal. FLS §6.1.2.
    ///
    /// The raw text is preserved via the span; converting to `f64` here would
    /// be lossy and premature. The type checker will resolve the suffix.
    LitFloat,

    /// A boolean literal. FLS §6.1.3.
    LitBool(bool),

    /// A string literal (regular, raw, byte, or C). FLS §6.1.4.
    ///
    /// Escape processing is deferred; the raw source text is in the span.
    LitStr,

    /// A character literal. FLS §6.1.5.
    ///
    /// Escape processing is deferred.
    LitChar,

    /// The unit value `()`. FLS §6.3.3.
    ///
    /// FLS §6.3.3: `()` is a tuple expression with zero elements. Its type
    /// and value are both the unit type `()`.
    Unit,

    /// A path expression resolving to a variable, function, or constant.
    ///
    /// FLS §6.2: A path expression is a path that resolves to a place or
    /// value. Each `Span` in the `Vec` is one path segment.
    Path(Vec<Span>),

    /// A block expression. FLS §6.10.
    Block(Box<Block>),

    /// A unary operator expression. FLS §6.4.
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },

    /// A binary operator expression. FLS §6.5–§6.9.
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },

    /// A type cast expression. FLS §6.5.9.
    ///
    /// `expr as Ty` converts the value of `expr` to the target type `ty`.
    ///
    /// FLS §6.5.9: "A type cast expression is used to convert a value of one
    /// type to a value of another type."
    ///
    /// FLS §6.5.9: The `as` operator has higher precedence than `*`, `/`, `%`
    /// and lower precedence than unary operators. It is left-associative.
    ///
    /// At this milestone only numeric casts to `i32` are supported.
    /// Casts between pointer types, `bool` → integer, and truncating/widening
    /// integer casts will be added as the type system grows.
    Cast {
        /// The expression whose value is being cast.
        expr: Box<Expr>,
        /// The target type.
        ty: Box<crate::ast::Ty>,
    },

    /// A compound assignment expression. FLS §6.5.11.
    ///
    /// `target op= value` reads the current value of `target`, applies `op`,
    /// and stores the result back to `target`. The expression has type `()`.
    ///
    /// Supported operators: `+=`, `-=`, `*=`, `/=`, `%=`, `&=`, `|=`, `^=`,
    /// `<<=`, `>>=`.
    ///
    /// FLS §6.5.11: "The type of a compound assignment expression is the unit type ()."
    CompoundAssign {
        /// The arithmetic/bitwise/shift operation to apply.
        op: BinOp,
        /// The place expression (left-hand side; must be a local variable path).
        target: Box<Expr>,
        /// The value expression (right-hand side; evaluated at runtime).
        value: Box<Expr>,
    },

    /// A function call expression. FLS §6.3.1.
    ///
    /// FLS §6.3.1 NOTE: the spec distinguishes call expressions (any callee)
    /// from method call expressions (`receiver.method(args)`). Method calls
    /// are not yet implemented.
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },

    /// A struct literal expression. FLS §6.11.
    ///
    /// Example: `Point { x: 1, y: 2 }` or `Point { x: 5, ..other }`
    ///
    /// FLS §6.11: A struct expression constructs an instance of a struct type.
    /// Each field must be initialised exactly once. The order of field initialisers
    /// in the source need not match the declaration order; galvanic normalises to
    /// declaration order during lowering.
    ///
    /// FLS §6.11: The struct update syntax `Struct { field: val, ..base }` copies
    /// all fields not explicitly listed from the `base` expression.
    StructLit {
        /// The struct type name (single identifier).
        name: Span,
        /// The field initialisers in source order: (field_name, value).
        fields: Vec<(Span, Box<Expr>)>,
        /// Optional base expression for struct update syntax `..base`.
        ///
        /// FLS §6.11: When present, fields not listed in `fields` are copied
        /// from the base struct. The base must have the same struct type.
        base: Option<Box<Expr>>,
    },

    /// A named-field enum variant construction expression. FLS §6.11 + §15.
    ///
    /// Example: `Color::Rgb { r: 255, g: 0, b: 0 }`
    ///
    /// FLS §6.11: Struct expressions also apply to enum variants with named
    /// fields. The two-segment path identifies the variant; fields are given
    /// by name and may appear in any order.
    ///
    /// FLS §15.3: Named-field enum variants (`Variant { field: Type }`).
    ///
    /// FLS §6.11 AMBIGUOUS: The spec does not state whether the shorthand
    /// form (`Color::Rgb { r, g, b }` with implicit `r: r`) is a struct
    /// expression or separate syntax. Galvanic only supports the explicit
    /// `field: expr` form at this milestone.
    ///
    /// Cache-line note: shares layout with `StructLit`; the extra `Vec<Span>`
    /// for the two-segment path adds one pointer (8 bytes) per construction site.
    EnumVariantLit {
        /// Two-segment path: `[enum_name, variant_name]`.
        path: Vec<Span>,
        /// Field initialisers in source order: (field_name, value).
        fields: Vec<(Span, Box<Expr>)>,
    },

    /// A field access expression. FLS §6.13.
    ///
    /// Example: `point.x`
    ///
    /// FLS §6.13: A field access expression evaluates the receiver operand
    /// and then accesses one of its fields.
    FieldAccess {
        /// The receiver expression.
        receiver: Box<Expr>,
        /// The field name span.
        field: Span,
    },

    /// A method call expression. FLS §6.3.2.
    ///
    /// Example: `vec.push(1)`, `self.len()`
    ///
    /// FLS §6.3.2: A method call expression invokes a method on a receiver.
    /// The receiver is auto-dereferenced to find an applicable implementation.
    ///
    /// FLS §6.3.2 AMBIGUOUS: The spec does not fully specify how many
    /// auto-deref steps are legal or how they interact with `Deref` trait
    /// implementations. This implementation parses the syntax only; method
    /// resolution is deferred to a future type-checking phase.
    MethodCall {
        /// The receiver expression.
        receiver: Box<Expr>,
        /// The method name span.
        method: Span,
        /// The argument expressions.
        args: Vec<Expr>,
    },

    /// A range expression. FLS §6.16.
    ///
    /// `start..end` (exclusive) or `start..=end` (inclusive).
    ///
    /// FLS §6.16: A range expression produces a value of the standard library
    /// range type. Galvanic supports integer ranges only, used as the iterator
    /// in `for` loop expressions.
    ///
    /// FLS §6.16 AMBIGUOUS: The spec defines range expressions as producing
    /// `Range`, `RangeFrom`, `RangeTo`, etc. values. Galvanic restricts support
    /// to `start..end` with integer operands, desugaring directly to a while loop
    /// in the lowering pass rather than creating a library range value.
    Range {
        /// Lower bound (inclusive). `None` for `..end` (RangeTo).
        start: Option<Box<Expr>>,
        /// Upper bound. `None` for `start..` (RangeFrom).
        end: Option<Box<Expr>>,
        /// `true` for `..=` (inclusive), `false` for `..` (exclusive).
        inclusive: bool,
    },

    /// A loop expression. FLS §6.8.1.
    ///
    /// `loop { body }`
    ///
    /// FLS §6.8.1: A loop expression executes its body repeatedly until a
    /// `break` expression is reached. Its value is the operand of `break`,
    /// or `()` if `break` carries no value.
    Loop(Box<Block>),

    /// A while loop expression. FLS §6.8.2.
    ///
    /// `while cond { body }`
    ///
    /// FLS §6.8.2: A while loop expression evaluates the condition before each
    /// iteration; if the condition is `false` the loop terminates and evaluates
    /// to `()`.
    While {
        cond: Box<Expr>,
        body: Box<Block>,
    },

    /// A while-let loop expression. FLS §6.15.4.
    ///
    /// `while let Pattern = Expr { body }`
    ///
    /// FLS §6.15.4: "A while let loop expression is syntactic sugar for a loop
    /// expression containing a match expression that breaks on mismatch."
    /// The loop evaluates to `()`.
    ///
    /// Cache-line note: lowered to a loop header + pattern-match check + body,
    /// same instruction count as a `while` loop plus a pattern comparison.
    WhileLet {
        /// The pattern to test each iteration.
        pat: Pat,
        /// The value being matched on each iteration.
        scrutinee: Box<Expr>,
        /// The loop body, executed when the pattern matches.
        body: Box<Block>,
    },

    /// A for loop expression. FLS §6.8.3.
    ///
    /// `for pat in iter { body }`
    ///
    /// FLS §6.8.3: A for loop expression iterates over the values produced by
    /// an [`IntoIterator`]. The loop evaluates to `()`.
    ///
    /// FLS §6.8.3 NOTE: The pattern may be any irrefutable pattern. This
    /// implementation restricts the loop variable to a simple identifier;
    /// destructuring patterns in `for` position are future work.
    For {
        /// The loop variable (simple identifier pattern).
        pat: Span,
        /// The iterator expression.
        iter: Box<Expr>,
        /// The loop body.
        body: Box<Block>,
    },

    /// A break expression. FLS §6.8.4.
    ///
    /// `break` or `break value`
    ///
    /// FLS §6.8.4: A break expression exits the innermost enclosing loop.
    /// The optional value becomes the result of the enclosing `loop` expression;
    /// `while` and `for` loops do not accept a break value.
    ///
    /// FLS §6.8.4 AMBIGUOUS: The spec does not clearly distinguish whether the
    /// break-with-value restriction (only in `loop`, not `while`/`for`) is a
    /// syntactic or semantic constraint. This implementation parses `break expr`
    /// freely and defers the restriction to a future type-checking phase.
    Break(Option<Box<Expr>>),

    /// A continue expression. FLS §6.8.5.
    ///
    /// `continue`
    ///
    /// FLS §6.8.5: A continue expression skips the remainder of the current
    /// loop body and begins the next iteration.
    Continue,

    /// A return expression. FLS §6.12.
    ///
    /// FLS §6.12: `return` without a value returns `()`.
    Return(Option<Box<Expr>>),

    /// An if (or if-else) expression. FLS §6.11.
    ///
    /// FLS §6.11 AMBIGUOUS: the spec does not explicitly state the type of an
    /// `if` expression without an `else` branch. The Rust reference says it
    /// must be `()`, but the FLS leaves this implicit. This implementation
    /// allows such expressions; the type checker will enforce the constraint.
    If {
        cond: Box<Expr>,
        then_block: Box<Block>,
        /// `Some(expr)` for `else`/`else if`. The expr is either a `Block` or
        /// another `If` expression.
        else_expr: Option<Box<Expr>>,
    },

    /// An if-let expression. FLS §6.17.
    ///
    /// `if let Pattern = Scrutinee { ThenBlock } [else { ElseExpr }]`
    ///
    /// FLS §6.17: An if-let expression evaluates the scrutinee and tests it
    /// against the pattern. If the pattern matches, the then block executes
    /// with any pattern bindings in scope. If it does not match, the else
    /// branch executes (if present).
    ///
    /// Lowering strategy: emit a pattern-match check (like a single match arm)
    /// followed by a conditional branch to the else path. Pattern bindings are
    /// installed in `locals` before the then block and removed after.
    ///
    /// FLS §6.17: "An if let expression is syntactic sugar for a match
    /// expression with a single arm." Galvanic lowers it directly without
    /// constructing a match node.
    ///
    /// Cache-line note: lowered to the same comparison chain as a 2-arm match.
    IfLet {
        /// The pattern to test the scrutinee against.
        pat: Pat,
        /// The value being matched.
        scrutinee: Box<Expr>,
        /// Executed when the pattern matches.
        then_block: Box<Block>,
        /// Executed when the pattern does not match (optional).
        ///
        /// `Some(expr)` for `else`/`else if let`. The expr is a `Block`,
        /// an `If`, or another `IfLet` expression.
        else_expr: Option<Box<Expr>>,
    },

    /// A match expression. FLS §6.18.
    ///
    /// `match scrutinee { arm0, arm1, ... }`
    ///
    /// FLS §6.18: A match expression branches over all possible values of the
    /// scrutinee. Arms are tested in source order; the first arm whose pattern
    /// matches executes the arm's body. The wildcard pattern `_` matches any
    /// value.
    ///
    /// Cache-line note: lowered to a comparison chain — no new IR instructions.
    Match {
        /// The value being matched.
        scrutinee: Box<Expr>,
        /// The match arms, in source order.
        arms: Vec<MatchArm>,
    },

    /// An array expression. FLS §6.8.
    ///
    /// `[elem0, elem1, elem2]`
    ///
    /// FLS §6.8: An array expression constructs a value of an array type. All
    /// elements must have the same type. The length is determined by the number
    /// of element expressions.
    ///
    /// At this milestone only `[i32; N]` arrays are supported. The elements
    /// are evaluated left-to-right (FLS §6.4:14) and stored to consecutive
    /// stack slots.
    ///
    /// Cache-line note: N elements occupying N consecutive 8-byte stack slots.
    /// A 8-element array fills exactly one 64-byte cache line on the stack.
    Array(Vec<Expr>),

    /// A tuple expression. FLS §6.10.
    ///
    /// `(expr0, expr1, ...)` — always two or more elements (one-element tuples
    /// require a trailing comma; zero elements is the unit expression `()`).
    ///
    /// FLS §6.10: A tuple expression evaluates each operand left-to-right
    /// and produces a tuple value. Field access is via `.0`, `.1`, etc.
    ///
    /// Cache-line note: N fields occupy N consecutive 8-byte stack slots,
    /// identical layout to N-field struct or N-element array.
    Tuple(Vec<Expr>),

    /// An indexing expression. FLS §6.9.
    ///
    /// `base[index]`
    ///
    /// FLS §6.9: An indexing expression accesses an element of an array or
    /// slice by position. The index must be a `usize` value.
    ///
    /// At this milestone the index is treated as an `i32` (runtime value).
    /// Bounds checking is not yet emitted — this is
    /// FLS §6.9 AMBIGUOUS: the spec does not specify the panic mechanism for
    /// out-of-bounds access without the standard library.
    ///
    /// Cache-line note: lowered to `add sp, #base; ldr [base, idx, lsl #3]` —
    /// two 4-byte instructions per index.
    Index {
        /// The array or slice being indexed.
        base: Box<Expr>,
        /// The index expression.
        index: Box<Expr>,
    },
}

// ── Match arms and patterns ───────────────────────────────────────────────────

/// A single arm in a match expression.
///
/// FLS §6.18: Each `MatchArm` consists of a pattern, an optional guard
/// (`if expr`), and a body expression.
///
/// FLS §6.18: "A match arm guard is an additional condition attached to
/// a match arm. The match arm guard is only evaluated if the pattern
/// matches. If the guard evaluates to `false`, the arm is not selected."
///
/// Cache-line note: `pat` is a small enum (fits in 2 words), `guard` and
/// `body` are `Option<Box<Expr>>` / `Box<Expr>` pointers. The struct fits
/// comfortably in a 64-byte cache line.
#[derive(Debug)]
pub struct MatchArm {
    /// The pattern to test.
    pub pat: Pat,
    /// Optional guard expression: `if <guard>`.
    ///
    /// FLS §6.18: Evaluated only when the pattern matches. If it evaluates
    /// to `false`, the arm is skipped and the next arm is tried.
    pub guard: Option<Box<Expr>>,
    /// The body expression executed when the pattern matches and guard passes.
    pub body: Box<Expr>,
    /// Source span covering the full arm (`pat [if guard] => body`).
    pub span: Span,
}

/// A pattern in a match arm.
///
/// FLS §5: Patterns. This is an intentionally minimal subset covering the
/// most common match patterns for integer and boolean scrutinees. Struct,
/// tuple, enum, and binding patterns are future work.
///
/// FLS §5.1: Wildcard pattern `_` — matches any value without binding.
/// FLS §5.1.4: Identifier patterns — bind the matched value to a name.
/// FLS §5.2: Literal patterns — integer and boolean literals.
/// FLS §5.1.9: Range patterns — `lo..=hi` (inclusive) and `lo..hi` (exclusive).
/// FLS §5.1.11: Or patterns — `p0 | p1 | ...`.
#[derive(Debug, Clone)]
pub enum Pat {
    /// Wildcard pattern `_`. Matches any value. FLS §5.1.
    Wildcard,
    /// Identifier pattern: matches any value and binds it to a name.
    ///
    /// FLS §5.1.4: "An identifier pattern matches any value and optionally
    /// binds it to the identifier." The `Span` points to the identifier token
    /// in the source text; call `span.text(source)` to recover the name.
    ///
    /// Example: `match x { 0 => 0, n => n * 2 }` — `n` is an identifier
    /// pattern in the second arm. It always matches and binds `x` to `n`,
    /// making `n` available in the arm body.
    ///
    /// Cache-line note: lowering emits 2 instructions (ldr scrut + str to
    /// binding slot = 8 bytes) to install the binding before the arm body.
    Ident(Span),
    /// Non-negative integer literal pattern. FLS §5.2.
    LitInt(u128),
    /// Negative integer literal pattern `-n`. FLS §5.2.
    ///
    /// Stored as the absolute value; the pattern matches `-(n as i32)`.
    /// Parsed from `-` followed by an integer literal token.
    ///
    /// FLS §5.2: "A literal pattern matches a value by comparing it against
    /// a constant literal value." Negative literals are valid literal patterns
    /// per the Rust reference (e.g., `match x { -1 => ... }`).
    NegLitInt(u128),
    /// Boolean literal pattern `true` / `false`. FLS §5.2.
    LitBool(bool),
    /// Inclusive range pattern `lo..=hi`. FLS §5.1.9.
    ///
    /// Matches any value `v` such that `lo <= v && v <= hi`.
    /// Both bounds are stored as `i128` to accommodate negative bounds
    /// (e.g., `-5..=-1`).
    ///
    /// FLS §5.1.9: "A range pattern matches any value that falls within
    /// the range's bounds." For `..=`, both bounds are inclusive.
    ///
    /// Cache-line note: lowering emits ~7 instructions per arm (ldr + 2×mov
    /// + 2×cmp + and + cbz = 28 bytes) — two range arms per 64-byte cache line.
    RangeInclusive {
        /// Lower bound (inclusive).
        lo: i128,
        /// Upper bound (inclusive).
        hi: i128,
    },
    /// Exclusive range pattern `lo..hi`. FLS §5.1.9.
    ///
    /// Matches any value `v` such that `lo <= v && v < hi`.
    ///
    /// FLS §5.1.9: Range patterns with `..` have an exclusive upper bound.
    RangeExclusive {
        /// Lower bound (inclusive).
        lo: i128,
        /// Upper bound (exclusive).
        hi: i128,
    },
    /// OR pattern `p0 | p1 | ...`. Matches if any alternative matches.
    ///
    /// FLS §5.1.11: Or patterns. The alternatives are tested left-to-right;
    /// the first matching alternative causes the arm to match.
    ///
    /// Example: `match x { 0 | 1 => "small", _ => "large" }`.
    ///
    /// Cache-line note: each alternative adds ~3 instructions (mov + cmp + orr),
    /// so 5 alternatives fit in a 64-byte instruction cache line.
    Or(Vec<Pat>),
    /// Path pattern — an enum unit variant path like `Color::Red`.
    ///
    /// FLS §5.5: Path patterns. A path that resolves to a unit enum variant
    /// matches only that variant. The path is stored as a sequence of `Span`s;
    /// `span.text(source)` recovers each segment.
    ///
    /// Example: `match c { Color::Red => 0, Color::Blue => 1, _ => 2 }`.
    ///
    /// Galvanic represents unit enum variant values as their integer
    /// discriminant (0, 1, 2, ...), so this pattern lowers to an integer
    /// equality comparison against the discriminant.
    ///
    /// Cache-line note: lowers identically to a LitInt pattern —
    /// ~3 instructions (mov + cmp + cbz = 12 bytes) per arm.
    Path(Vec<Span>),
    /// Tuple struct/variant pattern: `Enum::Variant(p1, p2, ...)`.
    ///
    /// FLS §5.4: Struct patterns. A tuple variant pattern matches by
    /// discriminant, then optionally binds positional fields to names.
    ///
    /// Example: `match x { Opt::Some(v) => v, Opt::None => 0 }` —
    /// `Opt::Some(v)` is a tuple struct pattern; `v` is bound to the first
    /// field of the matched variant.
    ///
    /// Field patterns: `Pat::Ident` (binding) and `Pat::Wildcard` (ignore)
    /// are supported. Nested patterns are future work.
    ///
    /// Cache-line note: lowers to ~5 instructions (ldr discriminant + mov +
    /// cmp + cbz + 1×ldr per field binding = 20+ bytes per arm).
    TupleStruct {
        /// The variant path (e.g., `["Opt", "Some"]`).
        path: Vec<Span>,
        /// Positional field patterns.
        fields: Vec<Pat>,
    },

    /// Named-field struct/variant pattern: `Enum::Variant { field, ... }`.
    ///
    /// FLS §5.3: Struct patterns. A named-field enum variant pattern matches
    /// by discriminant and optionally binds named fields.
    ///
    /// Example: `match c { Color::Rgb { r, g, b } => r + g + b, _ => 0 }`
    ///
    /// The shorthand form `{ field }` is sugar for `{ field: field }` (an
    /// identifier pattern binding `field` from the variant's field of the
    /// same name). The `_` wildcard (`{ field: _ }` or `{ .. }`) is future work.
    ///
    /// Cache-line note: each field binding lowers to ~2 instructions (ldr +
    /// str); N field bindings cost ~2N instructions (8N bytes) per arm.
    StructVariant {
        /// Two-segment path: `[enum_name, variant_name]`.
        path: Vec<Span>,
        /// Field patterns: `(field_name_span, pattern)` in source order.
        /// The shorthand `{ x }` is represented as `(x_span, Pat::Ident(x_span))`.
        fields: Vec<(Span, Pat)>,
    },

    /// Tuple pattern `(p0, p1, ...)`. Matches a tuple value of the given arity.
    ///
    /// FLS §5.10.3: "A tuple pattern is a pattern that matches a tuple which
    /// satisfies all criteria defined by its subpatterns."
    ///
    /// Used in `let (a, b) = t;` — each sub-pattern is matched against the
    /// corresponding tuple element. Only `Pat::Ident` and `Pat::Wildcard`
    /// sub-patterns are supported at this milestone.
    ///
    /// The empty form `()` matches the unit value (0-element tuple).
    ///
    /// Cache-line note: rebinding an existing tuple variable emits zero
    /// instructions (alias); a tuple literal init emits N stores (4N bytes).
    Tuple(Vec<Pat>),
}

// ── Operators ─────────────────────────────────────────────────────────────────

/// Unary operators.
///
/// FLS §6.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// Arithmetic negation `-`. FLS §6.4.1.
    Neg,
    /// Logical/bitwise not `!`. FLS §6.4.2.
    Not,
    /// Dereference `*`. FLS §6.4.3.
    Deref,
    /// Shared borrow `&`. FLS §6.4.4.
    Ref,
    /// Mutable borrow `&mut`. FLS §6.4.4.
    RefMut,
}

/// Binary operators, ordered by precedence group (lowest to highest).
///
/// FLS §6.5–§6.9.
///
/// FLS NOTE: The FLS does not assign numeric precedence levels; precedence
/// is encoded structurally in the grammar. The ordering here is documentation
/// only — actual precedence is enforced by the recursive descent call chain
/// in the parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    // Assignment — FLS §6.9 (lowest precedence among binops)
    /// `=`
    Assign,

    // Logical — FLS §6.8
    /// `||`
    Or,
    /// `&&`
    And,

    // Comparison — FLS §6.7
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `>`
    Gt,
    /// `<=`
    Le,
    /// `>=`
    Ge,

    // Bitwise — FLS §6.6
    /// `|`
    BitOr,
    /// `^`
    BitXor,
    /// `&`
    BitAnd,
    /// `<<`
    Shl,
    /// `>>`
    Shr,

    // Arithmetic — FLS §6.5
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `%`
    Rem,
}
