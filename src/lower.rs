//! AST-to-IR lowering for galvanic.
//!
//! Translates a parsed `SourceFile` into the minimal IR needed for ARM64
//! code generation. Each lowering function corresponds to a FLS section.
//!
//! # FLS constraint compliance (fls-constraints.md)
//!
//! This module emits **runtime instructions** for all non-const code.
//! Compile-time evaluation is only permitted in const contexts (FLS
//! §6.1.2:37–45). Since galvanic does not yet support `const` items,
//! ALL code paths emit runtime IR — no interpreter, no constant folding.
//!
//! The litmus test: if replacing a literal with a function parameter would
//! break the implementation, it's an interpreter, not a compiler.
//!
//! # FLS traceability
//!
//! - FLS §9: Functions — `lower_fn` maps each `FnDef` to an `IrFn`.
//! - FLS §8.1: Let statements — `lower_stmt` allocates a stack slot and stores.
//! - FLS §6.3: Path expressions — local variable references load from stack.
//! - FLS §6.19: Return expressions — tail expressions lower to `Instr::Ret`.
//! - FLS §2.4.4.1: Integer literal expressions — `LoadImm` materializes them.
//! - FLS §4.4: Unit type — absent tail / unit type lowers to `IrValue::Unit`.
//! - FLS §6.5.5: Arithmetic operators — `BinOp` instructions for +, -, *.
//! - FLS §6.1.2:37–45: Non-const code emits runtime instructions.
//! - FLS §18.1: Program structure — `lower` produces one `Module` per file.

use std::collections::HashMap;

use crate::ast::{BinOp, Block, Expr, ExprKind, ItemKind, Pat, SelfKind, SourceFile, Stmt, StmtKind, StructKind, TyKind};
use crate::ir::{IrBinOp, Instr, IrFn, IrTy, IrValue, Module};

/// Enum variant registry: maps enum name → (variant name → (discriminant, field_names)).
///
/// FLS §15: Enumerations. Each variant is assigned an integer discriminant
/// starting at 0.
///
/// `field_names` is the ordered list of field names:
/// - Unit variants: empty `vec![]`.
/// - Tuple variants: positional placeholders `vec![""; N]` (names unused).
/// - Named-field variants: actual field names in declaration order.
///
/// `field_names.len()` is the field count for all variant kinds.
/// Named variants use the field names to map construction / pattern fields to
/// the correct stack slot (`base + 1 + declaration_index`).
///
/// Cache-line note: `Vec<String>` adds one heap pointer per variant entry.
/// At this milestone the registry is build-time only and not on a hot path.
type EnumVariantInfo = (i32, Vec<String>);
type EnumDefs = HashMap<String, HashMap<String, EnumVariantInfo>>;

// ── FLS citations added in this module ───────────────────────────────────────
// FLS §6.12.1: Call expressions — `lower_expr` handles `ExprKind::Call`.
// FLS §9: Functions with parameters — `lower_fn` spills x0..x{n-1} to stack.
// FLS §6.15.2: Infinite loop expressions — `lower_expr` handles `ExprKind::Loop`.
// FLS §6.15.3: While loop expressions — `lower_expr` handles `ExprKind::While`.
// FLS §6.15.5: Continue expressions — `lower_expr` handles `ExprKind::Continue`.
// FLS §6.15.6: Break expressions — `lower_expr` handles `ExprKind::Break`.
// FLS §6.5.3: Comparison operator expressions — `lower_expr` handles Lt/Le/Gt/Ge/Eq/Ne.
// FLS §6.5.6: Bit operator expressions — `lower_expr` handles BitAnd/BitOr/BitXor.
// FLS §6.5.7: Shift operator expressions — `lower_expr` handles Shl/Shr.
// FLS §6.18: Match expressions — `lower_expr` handles `ExprKind::Match`.

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors that can occur during lowering.
#[derive(Debug)]
pub enum LowerError {
    /// A language feature used by the program is not yet implemented.
    Unsupported(String),
}

impl std::fmt::Display for LowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LowerError::Unsupported(msg) => write!(f, "not yet supported: {msg}"),
        }
    }
}

// ── Call-detection helpers ────────────────────────────────────────────────────

/// Return `true` if the expression tree contains at least one `Call` node.
///
/// Used during lowering to determine whether an intermediate register value
/// needs to be spilled to a stack slot before lowering the other sub-expression.
/// ARM64 calling convention: `x0`–`x17` are caller-saved — any `bl` instruction
/// in the RHS of a binary expression will clobber a live register holding the
/// LHS result. Spilling prevents this.
///
/// For example, `fib(n-1) + fib(n-2)`: the first call puts its result in some
/// register `r`. The second call (`bl fib`) re-uses that same register range
/// internally, overwriting `r`. Without a spill, the add would use the wrong
/// value for the LHS.
///
/// FLS §6.12.1: Call expressions invoke a function at runtime and follow the
/// ARM64 calling convention (caller-saved: x0–x17).
fn expr_contains_call(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Call { .. } | ExprKind::MethodCall { .. } => true,
        ExprKind::Binary { lhs, rhs, .. } => expr_contains_call(lhs) || expr_contains_call(rhs),
        ExprKind::Unary { operand, .. } => expr_contains_call(operand),
        ExprKind::Cast { expr: inner, .. } => expr_contains_call(inner),
        ExprKind::CompoundAssign { target, value, .. } => {
            expr_contains_call(target) || expr_contains_call(value)
        }
        ExprKind::Block(block) => block_contains_call(block),
        ExprKind::If { cond, then_block, else_expr } => {
            expr_contains_call(cond)
                || block_contains_call(then_block)
                || else_expr.as_ref().is_some_and(|e| expr_contains_call(e))
        }
        ExprKind::IfLet { scrutinee, then_block, else_expr, .. } => {
            expr_contains_call(scrutinee)
                || block_contains_call(then_block)
                || else_expr.as_ref().is_some_and(|e| expr_contains_call(e))
        }
        ExprKind::While { cond, body } => {
            expr_contains_call(cond) || block_contains_call(body)
        }
        ExprKind::WhileLet { scrutinee, body, .. } => {
            expr_contains_call(scrutinee) || block_contains_call(body)
        }
        ExprKind::Loop(body) => block_contains_call(body),
        ExprKind::Break(opt_val) => opt_val.as_ref().is_some_and(|e| expr_contains_call(e)),
        ExprKind::Return(opt_val) => opt_val.as_ref().is_some_and(|e| expr_contains_call(e)),
        ExprKind::Range { start, end, .. } => {
            start.as_ref().is_some_and(|e| expr_contains_call(e))
                || end.as_ref().is_some_and(|e| expr_contains_call(e))
        }
        ExprKind::For { iter, body, .. } => {
            expr_contains_call(iter) || block_contains_call(body)
        }
        ExprKind::Match { scrutinee, arms } => {
            expr_contains_call(scrutinee)
                || arms.iter().any(|a| expr_contains_call(&a.body))
        }
        ExprKind::StructLit { fields, base, .. } => {
            fields.iter().any(|(_, v)| expr_contains_call(v))
                || base.as_ref().is_some_and(|b| expr_contains_call(b))
        }
        ExprKind::EnumVariantLit { fields, .. } => {
            fields.iter().any(|(_, v)| expr_contains_call(v))
        }
        ExprKind::FieldAccess { receiver, .. } => expr_contains_call(receiver),
        ExprKind::Array(elems) => elems.iter().any(expr_contains_call),
        ExprKind::Tuple(elems) => elems.iter().any(expr_contains_call),
        ExprKind::Index { base, index } => expr_contains_call(base) || expr_contains_call(index),
        // Leaves: literals, paths, unit — none contain calls.
        _ => false,
    }
}

/// Return `true` if any statement or tail expression in the block contains a call.
///
/// FLS §6.4: Block expressions are sequences of statements followed by an
/// optional tail expression. A call anywhere in the sequence makes the block
/// "call-containing" for register-spill purposes.
fn block_contains_call(block: &Block) -> bool {
    block.stmts.iter().any(stmt_contains_call)
        || block.tail.as_ref().is_some_and(|e| expr_contains_call(e))
}

fn stmt_contains_call(stmt: &Stmt) -> bool {
    match &stmt.kind {
        StmtKind::Expr(e) => expr_contains_call(e),
        StmtKind::Let { init, .. } => init.as_ref().is_some_and(|e| expr_contains_call(e)),
        StmtKind::Empty => false,
    }
}

// ── Break-with-value detection ───────────────────────────────────────────────

/// Return `true` if the block contains a `break <value>` expression that
/// belongs to the *current* loop level (not a nested loop).
///
/// FLS §6.15.6: Only `loop` expressions support break-with-value; `while`
/// and `for` loops do not yield a value via `break`.
fn block_contains_break_with_value(block: &Block) -> bool {
    block.stmts.iter().any(stmt_contains_break_with_value)
        || block.tail.as_ref().is_some_and(|e| expr_contains_break_with_value(e))
}

fn stmt_contains_break_with_value(stmt: &Stmt) -> bool {
    match &stmt.kind {
        StmtKind::Expr(e) => expr_contains_break_with_value(e),
        StmtKind::Let { init, .. } => {
            init.as_ref().is_some_and(|e| expr_contains_break_with_value(e))
        }
        StmtKind::Empty => false,
    }
}

/// Return `true` if the expression contains a `break <value>` at the current
/// loop level. Does **not** recurse into nested loop bodies, because `break`
/// statements inside nested loops belong to those loops, not the outer one.
fn expr_contains_break_with_value(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Break(Some(_)) => true,
        // Recurse into non-loop control-flow.
        ExprKind::If { cond, then_block, else_expr } => {
            expr_contains_break_with_value(cond)
                || block_contains_break_with_value(then_block)
                || else_expr.as_ref().is_some_and(|e| expr_contains_break_with_value(e))
        }
        ExprKind::IfLet { scrutinee, then_block, else_expr, .. } => {
            expr_contains_break_with_value(scrutinee)
                || block_contains_break_with_value(then_block)
                || else_expr.as_ref().is_some_and(|e| expr_contains_break_with_value(e))
        }
        ExprKind::Block(b) => block_contains_break_with_value(b),
        ExprKind::Match { scrutinee, arms } => {
            expr_contains_break_with_value(scrutinee)
                || arms.iter().any(|a| expr_contains_break_with_value(&a.body))
        }
        ExprKind::StructLit { fields, base, .. } => {
            fields.iter().any(|(_, v)| expr_contains_break_with_value(v))
                || base.as_ref().is_some_and(|b| expr_contains_break_with_value(b))
        }
        ExprKind::EnumVariantLit { fields, .. } => {
            fields.iter().any(|(_, v)| expr_contains_break_with_value(v))
        }
        ExprKind::FieldAccess { receiver, .. } => expr_contains_break_with_value(receiver),
        ExprKind::Array(elems) => elems.iter().any(expr_contains_break_with_value),
        ExprKind::Tuple(elems) => elems.iter().any(expr_contains_break_with_value),
        ExprKind::Index { base, index } => {
            expr_contains_break_with_value(base) || expr_contains_break_with_value(index)
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            expr_contains_break_with_value(receiver)
                || args.iter().any(expr_contains_break_with_value)
        }
        // Do NOT recurse into nested loops — their `break` belongs to them.
        ExprKind::Loop(_) | ExprKind::While { .. } | ExprKind::WhileLet { .. } | ExprKind::For { .. } => false,
        _ => false,
    }
}

// ── Entry point ──────────────────────────────────────────────────────────────

/// Lower a parsed source file to the IR.
///
/// FLS §18.1: A source file is a sequence of items. Each `fn` item is
/// lowered to an `IrFn`. Struct items (FLS §14) are collected into a
/// definition table and used during function lowering for struct literal
/// and field-access expressions. Enum items with unit variants (FLS §15)
/// are collected into an enum definition table for path-expression and
/// path-pattern lowering.
pub fn lower(src: &SourceFile, source: &str) -> Result<Module, LowerError> {
    // First pass: collect struct and enum definitions.
    //
    // FLS §14: Struct definitions declare the field names and their types.
    // We store field names in declaration order; field access uses this
    // order to compute the stack-slot offset.
    //
    // FLS §15: Enum definitions declare the variant names and their kinds.
    // Unit variants are assigned discriminants 0, 1, 2, ... in declaration
    // order (FLS §15: explicit discriminants are not yet supported).
    //
    // Cache-line note: each i32 field or discriminant occupies one 8-byte
    // stack slot. A struct with N fields occupies N consecutive slots.
    let mut struct_defs: HashMap<String, Vec<String>> = HashMap::new();
    let mut enum_defs: EnumDefs = HashMap::new();
    // FLS §14.2: Tuple struct field counts. Maps struct name → field count.
    // Used to recognize constructor calls `Point(a, b)` during let-binding lowering.
    let mut tuple_struct_defs: HashMap<String, usize> = HashMap::new();
    // FLS §6.11, §6.13, §4.11: Track per-field struct type names for nested struct
    // construction and chained field access. `None` = scalar field, `Some(name)` =
    // field whose type is another named struct (requiring multiple stack slots).
    //
    // Cache-line note: struct fields of struct type occupy their nested struct's total
    // slot count instead of a single slot, allowing precise offset computation.
    let mut struct_raw_field_types: HashMap<String, Vec<Option<String>>> = HashMap::new();

    for item in &src.items {
        match &item.kind {
            ItemKind::Struct(s) => {
                let struct_name = s.name.text(source).to_owned();
                match &s.kind {
                    StructKind::Named(fields) => {
                        let field_names = fields
                            .iter()
                            .map(|f| f.name.text(source).to_owned())
                            .collect();
                        // FLS §6.13: Record the type of each field so we can compute
                        // nested slot offsets for chained field access (`s.b.x`).
                        // Scalar primitive types map to `None`; user-defined struct types
                        // map to `Some(type_name)` and occupy multiple consecutive slots.
                        let field_types: Vec<Option<String>> = fields
                            .iter()
                            .map(|f| match &f.ty.kind {
                                TyKind::Path(segs) if segs.len() == 1 => {
                                    let ty_name = segs[0].text(source);
                                    // Primitive scalar types — each occupies exactly one slot.
                                    match ty_name {
                                        "i8" | "i16" | "i32" | "i64" | "i128"
                                        | "u8" | "u16" | "u32" | "u64" | "u128"
                                        | "isize" | "usize" | "f32" | "f64"
                                        | "bool" | "char" => None,
                                        // Any other single-segment type is assumed to be a
                                        // user-defined struct — look it up in struct_defs later.
                                        other => Some(other.to_owned()),
                                    }
                                }
                                _ => None, // reference types, tuple types, etc. — treat as scalar
                            })
                            .collect();
                        struct_defs.insert(struct_name.clone(), field_names);
                        struct_raw_field_types.insert(struct_name, field_types);
                    }
                    StructKind::Unit => {
                        struct_defs.insert(struct_name.clone(), vec![]);
                        struct_raw_field_types.insert(struct_name, vec![]);
                    }
                    StructKind::Tuple(fields) => {
                        // FLS §14.2: Tuple struct. Record field count so that
                        // constructor calls `Point(a, b)` can allocate the right
                        // number of consecutive stack slots.
                        tuple_struct_defs.insert(struct_name, fields.len());
                    }
                }
            }
            ItemKind::Enum(e) => {
                // FLS §15: Collect variants with auto-discriminants and field names.
                // Unit variants: empty field list. Tuple variants: positional
                // placeholder names. Named-field variants: actual declaration-order names.
                let enum_name = e.name.text(source).to_owned();
                let mut variants: HashMap<String, EnumVariantInfo> = HashMap::new();
                for (discriminant, variant) in e.variants.iter().enumerate() {
                    use crate::ast::EnumVariantKind;
                    let variant_name = variant.name.text(source).to_owned();
                    match &variant.kind {
                        EnumVariantKind::Unit => {
                            variants.insert(variant_name, (discriminant as i32, vec![]));
                        }
                        EnumVariantKind::Tuple(fields) => {
                            // Positional placeholders — names unused; only count matters.
                            variants.insert(
                                variant_name,
                                (discriminant as i32, vec!["".to_owned(); fields.len()]),
                            );
                        }
                        // FLS §15.3: Named-field variant. Store names in declaration order
                        // so that construction and patterns can map name → slot index.
                        EnumVariantKind::Named(fields) => {
                            let names: Vec<String> = fields
                                .iter()
                                .map(|f| f.name.text(source).to_owned())
                                .collect();
                            variants.insert(variant_name, (discriminant as i32, names));
                        }
                    }
                }
                enum_defs.insert(enum_name, variants);
            }
            ItemKind::Fn(_) | ItemKind::Impl(_) | ItemKind::Trait(_) | ItemKind::Const(_) | ItemKind::Static(_) => {}
        }
    }

    // Collect constant item values: maps const name → i32 value.
    //
    // FLS §7.1: Constant items are compile-time values substituted at every
    // use site. The initializer must be a constant expression (FLS §6.1.2).
    // At this milestone only integer literal initializers are supported.
    //
    // FLS §7.1:10: "Every use of a constant is replaced with its value
    // (or a copy of it)." Galvanic implements this by emitting `LoadImm`
    // when a path expression resolves to a known const name.
    //
    // Cache-line note: this HashMap is built once and shared read-only
    // across all `lower_fn` calls — not on any hot runtime path.
    let mut const_vals: HashMap<String, i32> = HashMap::new();
    for item in &src.items {
        if let ItemKind::Const(c) = &item.kind {
            let name = c.name.text(source).to_owned();
            // Only integer literal initializers supported at this milestone.
            // FLS §6.1.2:37–45: The const initializer is evaluated at compile
            // time. Non-literal initialisers (arithmetic, other consts) are
            // future work.
            if let ExprKind::LitInt(n) = &c.value.kind
                && *n <= i32::MAX as u128
            {
                const_vals.insert(name, *n as i32);
            }
        }
    }

    // Collect static item names and their data-section entries.
    //
    // FLS §7.2: Static items are allocated in the data section with a fixed
    // address. Every use of a static emits a LoadStatic (ADRP + ADD + LDR)
    // rather than a LoadImm, because FLS §7.2:15 requires all references
    // to go through the same memory address.
    //
    // At this milestone only integer literal initializers are supported.
    //
    // Cache-line note: each StaticData entry will become a `.quad` in the
    // `.data` section — 8 bytes per static.
    let mut static_data: Vec<crate::ir::StaticData> = Vec::new();
    let mut static_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    for item in &src.items {
        if let ItemKind::Static(s) = &item.kind {
            let name = s.name.text(source).to_owned();
            if let ExprKind::LitInt(n) = &s.value.kind
                && *n <= i32::MAX as u128
            {
                static_data.push(crate::ir::StaticData { name: name.clone(), value: *n as i32 });
                static_names.insert(name);
            }
        }
    }

    // Build method self-kind registry: mangled name → SelfKind.
    //
    // FLS §10.1: `&mut self` methods must propagate mutations back to the caller.
    // At the call site we need to know whether a method is `&mut self` so we
    // can emit `CallMut` (write-back) instead of a plain `Call`.
    //
    // Cache-line note: this HashMap is populated once at compile time and
    // read during method-call lowering. It is not on any hot runtime path.
    let mut method_self_kinds: HashMap<String, SelfKind> = HashMap::new();
    for item in &src.items {
        if let ItemKind::Impl(impl_def) = &item.kind {
            let type_name = impl_def.ty.text(source);
            for method in &impl_def.methods {
                if let Some(kind) = method.self_param {
                    let method_name = method.name.text(source);
                    let mangled = format!("{type_name}__{method_name}");
                    method_self_kinds.insert(mangled, kind);
                }
            }
        }
    }

    // Build `&mut self` scalar-return registry: set of mangled names for `&mut self`
    // methods that return a scalar (non-unit, non-struct, non-enum) type.
    //
    // FLS §10.1: `&mut self` methods may return any type. When the return type is
    // a scalar (i32, bool, u32, etc.), the callee uses `RetFieldsAndValue` to pack
    // both the modified fields and the return value. The call site uses `CallMutReturn`
    // to capture the scalar from x{N} after writing back the fields.
    //
    // Cache-line note: populated once at compile time; not on any hot runtime path.
    let mut mut_self_scalar_return_fns: std::collections::HashSet<String> = std::collections::HashSet::new();
    for item in &src.items {
        if let ItemKind::Impl(impl_def) = &item.kind {
            let type_name = impl_def.ty.text(source);
            for method in &impl_def.methods {
                if method.self_param != Some(SelfKind::RefMut) {
                    continue;
                }
                let method_name = method.name.text(source);
                let mangled = format!("{type_name}__{method_name}");
                // Check if return type is a known scalar primitive (not void, not struct, not enum).
                if let Some(ret_ty_node) = &method.ret_ty
                    && let TyKind::Path(segs) = &ret_ty_node.kind
                    && segs.len() == 1
                {
                    let ret_name = segs[0].text(source);
                    // Scalar types handled by lower_ty: i32, bool, u32, etc.
                    // Exclude struct/enum types (handled by other registries).
                    if !struct_defs.contains_key(ret_name)
                        && !enum_defs.contains_key(ret_name)
                        && matches!(
                            ret_name,
                            "i32" | "i8" | "i16" | "i64" | "isize"
                                | "u8" | "u16" | "u32" | "u64" | "usize"
                                | "bool"
                        )
                    {
                        mut_self_scalar_return_fns.insert(mangled);
                    }
                }
            }
        }
    }

    // Build struct-returning associated function registry: mangled name → struct type name.
    //
    // FLS §10.1: Associated functions (no self parameter) that return a struct type
    // use a special write-back calling convention: the callee stores field values in
    // x0..x{N-1} via `RetFields`; the call site writes them to consecutive stack slots
    // via `CallMut`. This registry lets the call site identify such functions.
    //
    // Cache-line note: populated once at compile time, not on any hot path.
    let mut struct_return_fns: HashMap<String, String> = HashMap::new();
    for item in &src.items {
        if let ItemKind::Impl(impl_def) = &item.kind {
            let type_name = impl_def.ty.text(source);
            for method in &impl_def.methods {
                if method.self_param.is_some() {
                    continue; // Only associated functions (no self).
                }
                let method_name = method.name.text(source);
                let mangled = format!("{type_name}__{method_name}");
                if let Some(ret_ty) = &method.ret_ty
                    && let TyKind::Path(segs) = &ret_ty.kind
                    && segs.len() == 1
                {
                    let ret_name = segs[0].text(source);
                    if struct_defs.contains_key(ret_name) {
                        // FLS §10.1: Associated function returning a struct type.
                        struct_return_fns.insert(mangled, ret_name.to_owned());
                    }
                }
            }
        }
    }

    // Build struct-returning `&self` instance method registry: mangled name → struct type name.
    //
    // FLS §10.1: `&self` instance methods that return a named struct type use
    // the same write-back calling convention as struct-returning associated
    // functions: the callee stores field values in x0..x{N-1} via `RetFields`;
    // the call site (in a `let` binding) writes them to the destination
    // variable's consecutive stack slots via `CallMut`.
    //
    // Only `&self` (SelfKind::Ref) methods are registered here. `&mut self`
    // methods with struct returns are not yet supported (they would need
    // both write-back of modified self fields AND return fields, which requires
    // a new calling convention).
    //
    // Cache-line note: populated once at compile time, not on any hot path.
    let mut struct_return_methods: HashMap<String, String> = HashMap::new();
    for item in &src.items {
        if let ItemKind::Impl(impl_def) = &item.kind {
            let type_name = impl_def.ty.text(source);
            for method in &impl_def.methods {
                if method.self_param != Some(SelfKind::Ref) {
                    continue; // Only &self methods.
                }
                let method_name = method.name.text(source);
                let mangled = format!("{type_name}__{method_name}");
                if let Some(ret_ty) = &method.ret_ty
                    && let TyKind::Path(segs) = &ret_ty.kind
                    && segs.len() == 1
                {
                    let ret_name = segs[0].text(source);
                    if struct_defs.contains_key(ret_name) {
                        // FLS §10.1: &self method returning a struct type.
                        struct_return_methods.insert(mangled, ret_name.to_owned());
                    }
                }
            }
        }
    }

    // Build struct-returning free function registry: fn name → struct type name.
    //
    // FLS §9: Free functions that return a named struct type use the same
    // write-back calling convention as struct-returning associated functions:
    // the callee stores field values in x0..x{N-1} via `RetFields`; the call
    // site writes them to the destination variable's consecutive stack slots
    // via `CallMut`. This registry lets the call site identify such functions.
    //
    // Cache-line note: populated once at compile time, not on any hot path.
    let mut struct_return_free_fns: HashMap<String, String> = HashMap::new();
    for item in &src.items {
        if let ItemKind::Fn(fn_def) = &item.kind {
            let fn_name = fn_def.name.text(source);
            if let Some(ret_ty) = &fn_def.ret_ty
                && let TyKind::Path(segs) = &ret_ty.kind
                && segs.len() == 1
            {
                let ret_name = segs[0].text(source);
                if struct_defs.contains_key(ret_name) {
                    // FLS §9: Free function returning a named struct.
                    struct_return_free_fns.insert(fn_name.to_owned(), ret_name.to_owned());
                }
            }
        }
    }

    // Build enum-returning free function registry: fn name → enum type name.
    //
    // FLS §9, §15: Free functions that return an enum type use a write-back
    // calling convention: the callee stores discriminant + fields in
    // x0..x{1+max_fields} via `RetFields`; the call site writes them to the
    // destination enum variable's stack slots via `CallMut`.
    //
    // Cache-line note: populated once at compile time, not on any hot path.
    let mut enum_return_fns: HashMap<String, String> = HashMap::new();
    for item in &src.items {
        if let ItemKind::Fn(fn_def) = &item.kind {
            let fn_name = fn_def.name.text(source);
            if let Some(ret_ty) = &fn_def.ret_ty
                && let TyKind::Path(segs) = &ret_ty.kind
                && segs.len() == 1
            {
                let ret_name = segs[0].text(source);
                if enum_defs.contains_key(ret_name) {
                    enum_return_fns.insert(fn_name.to_owned(), ret_name.to_owned());
                }
            }
        }
    }

    // Compute struct sizes and field offsets for nested struct support.
    //
    // FLS §6.11: Struct expressions with struct-type fields require knowing the
    // total slot count for each field type to allocate contiguous stack slots.
    // FLS §6.13: Chained field access (`s.b.x`) requires computing the byte
    // offset of field `b` within `s`, which equals the sum of sizes of preceding
    // fields. For scalar fields this is 1 slot; for struct-type fields it is
    // the nested struct's total slot count.
    //
    // Algorithm: iteratively compute sizes until fixed point (handles forward
    // references between structs; cycles left at their default of field_count).
    //
    // FLS §4.11: Representation. Galvanic lays out struct fields in declaration
    // order with no padding (each slot is 8 bytes). This matches the ARM64 ABI
    // for small aggregate passing.
    //
    // Cache-line note: struct_sizes and struct_field_offsets are read-only after
    // construction and are not on any hot path.
    let mut struct_sizes: HashMap<String, usize> = HashMap::new();
    loop {
        let mut changed = false;
        for (sname, ftypes) in &struct_raw_field_types {
            if struct_sizes.contains_key(sname) {
                continue;
            }
            let mut total = 0usize;
            let mut can_compute = true;
            for ft in ftypes {
                match ft {
                    None => total += 1,
                    Some(inner) => {
                        if let Some(&sz) = struct_sizes.get(inner) {
                            total += sz;
                        } else {
                            can_compute = false;
                            break;
                        }
                    }
                }
            }
            if can_compute {
                struct_sizes.insert(sname.clone(), total);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    // Any struct not yet resolved (unknown field type or cycle) defaults to
    // its declared field count — same as the pre-nested-struct behaviour.
    for (sname, field_names) in &struct_defs {
        struct_sizes.entry(sname.clone()).or_insert(field_names.len());
    }

    // Compute field slot offsets: for each struct, the slot offset of each field
    // relative to the struct's base slot.
    //
    // Example: `Outer { a: Inner { x: i32, y: i32 }, b: i32 }` has
    //   field_offsets["Outer"] = [0, 2]  (a starts at 0, b at 2)
    //   struct_sizes["Outer"] = 3        (2 for Inner + 1 for b)
    let mut struct_field_offsets: HashMap<String, Vec<usize>> = HashMap::new();
    for (sname, ftypes) in &struct_raw_field_types {
        let mut offsets = Vec::with_capacity(ftypes.len());
        let mut off = 0usize;
        for ft in ftypes {
            offsets.push(off);
            off += match ft {
                None => 1,
                Some(inner) => struct_sizes.get(inner).copied().unwrap_or(1),
            };
        }
        struct_field_offsets.insert(sname.clone(), offsets);
    }

    // Second pass: lower function items.
    //
    // FLS §6.17: Branch target labels must be unique within the assembly file.
    // GAS local labels (`.L{n}`) are file-scoped, so if two functions both emit
    // `.L0:` and `.L1:` the assembler will error or branch to the wrong target.
    // We pass a monotonically increasing `label_base` to each `lower_fn` call
    // so that every function's labels are globally unique.
    let mut fns = Vec::new();
    let mut label_base: u32 = 0;
    for item in &src.items {
        match &item.kind {
            ItemKind::Fn(fn_def) => {
                let (ir_fn, next_label) = lower_fn(fn_def, source, &struct_defs, &tuple_struct_defs, &enum_defs, &method_self_kinds, &mut_self_scalar_return_fns, &struct_return_fns, &struct_return_free_fns, &enum_return_fns, &struct_return_methods, &const_vals, &static_names, &struct_raw_field_types, &struct_field_offsets, &struct_sizes, None, label_base)?;
                label_base = next_label;
                fns.push(ir_fn);
            }
            ItemKind::Impl(impl_def) => {
                // FLS §11: Inherent impl and trait impl. Each method becomes a
                // mangled top-level function: `TypeName__method_name`.
                // For trait impls (`impl Trait for Type`), the struct type is
                // `impl_def.ty` and mangling is identical to inherent impls.
                // FLS §13: Trait method implementations lower identically to
                // inherent methods — static dispatch uses the same mangling.
                let type_name = impl_def.ty.text(source);
                for method in &impl_def.methods {
                    let method_name = method.name.text(source);
                    let mangled = format!("{type_name}__{method_name}");
                    // Always create MethodCtx so associated functions (no self_param)
                    // get the mangled name. impl_type and self_kind are None for
                    // associated functions.
                    //
                    // FLS §10.1: Associated functions do not have a self parameter.
                    let mctx = Some(MethodCtx {
                        impl_type: method.self_param.map(|_| type_name),
                        mangled_name: &mangled,
                        self_kind: method.self_param,
                    });
                    let (ir_fn, next_label) = lower_fn(
                        method,
                        source,
                        &struct_defs,
                        &tuple_struct_defs,
                        &enum_defs,
                        &method_self_kinds,
                        &mut_self_scalar_return_fns,
                        &struct_return_fns,
                        &struct_return_free_fns,
                        &enum_return_fns,
                        &struct_return_methods,
                        &const_vals,
                        &static_names,
                        &struct_raw_field_types,
                        &struct_field_offsets,
                        &struct_sizes,
                        mctx,
                        label_base,
                    )?;
                    label_base = next_label;
                    fns.push(ir_fn);
                }
            }
            ItemKind::Struct(_) | ItemKind::Enum(_) => {} // already processed above
            // FLS §13: Trait definitions are parsed but produce no codegen.
            // Trait method implementations are emitted via `ItemKind::Impl`.
            ItemKind::Trait(_) => {}
            // FLS §7.1: Const items are collected in the first pass above.
            // They produce no runtime code of their own.
            ItemKind::Const(_) => {}
            // FLS §7.2: Static items are collected in the first pass above.
            // They produce no function code — only data section entries.
            ItemKind::Static(_) => {}
        }
    }

    Ok(Module { fns, statics: static_data })
}

// ── Function lowering ────────────────────────────────────────────────────────

/// Context passed to `lower_fn` for method lowering.
///
/// Bundles the parameters that distinguish a method from a free function.
/// FLS §10.1: Methods have a `self` parameter and a mangled name.
/// Associated functions (no `self`) also get a `MethodCtx` for the mangled
/// name; their `impl_type` and `self_kind` are both `None`.
struct MethodCtx<'a> {
    /// Struct type name this method belongs to (for self-field spilling).
    /// `None` for associated functions (no `self` parameter).
    impl_type: Option<&'a str>,
    /// Mangled function name (`TypeName__method_name`).
    mangled_name: &'a str,
    /// How `self` is received (value, shared ref, mutable ref).
    /// `None` for associated functions (no `self` parameter).
    self_kind: Option<SelfKind>,
}

/// Lower a single function (or method) definition to an `IrFn`.
///
/// FLS §9: Functions. FLS §10.1: Methods.
///
/// - `method`: if `Some`, this function is a method of the named struct type.
///   The struct's fields are spilled from leading registers before any explicit
///   parameters, and the function is emitted under the mangled name.
///
/// FLS §6.12.1: Functions with parameters receive arguments in x0–x{n-1}
/// per the ARM64 ABI. We spill each parameter to a stack slot so that
/// path expressions can reference them via `Load` — reusing the same
/// infrastructure as let-binding locals.
#[allow(clippy::too_many_arguments)]
fn lower_fn(
    fn_def: &crate::ast::FnDef,
    source: &str,
    struct_defs: &HashMap<String, Vec<String>>,
    tuple_struct_defs: &HashMap<String, usize>,
    enum_defs: &EnumDefs,
    method_self_kinds: &HashMap<String, SelfKind>,
    mut_self_scalar_return_fns: &std::collections::HashSet<String>,
    struct_return_fns: &HashMap<String, String>,
    struct_return_free_fns: &HashMap<String, String>,
    enum_return_fns: &HashMap<String, String>,
    struct_return_methods: &HashMap<String, String>,
    const_vals: &HashMap<String, i32>,
    static_names: &std::collections::HashSet<String>,
    struct_field_types: &HashMap<String, Vec<Option<String>>>,
    struct_field_offsets: &HashMap<String, Vec<usize>>,
    struct_sizes: &HashMap<String, usize>,
    method: Option<MethodCtx<'_>>,
    start_label: u32,
) -> Result<(IrFn, u32), LowerError> {
    // For associated functions, impl_type = None and self_kind = None.
    let impl_type = method.as_ref().and_then(|m| m.impl_type);
    let override_name = method.as_ref().map(|m| m.mangled_name);
    let self_kind = method.as_ref().and_then(|m| m.self_kind);
    let name = override_name
        .map(|s| s.to_owned())
        .unwrap_or_else(|| fn_def.name.text(source).to_owned());

    // FLS §9: "If no return type is specified, the return type is `()`."
    // For functions returning a struct or enum type, `lower_ty` would fail
    // (struct/enum names are not primitive IR types). Detect them separately.
    //
    // FLS §10.1: Associated functions may return the impl type or any other type.
    // FLS §9, §15: Free functions may return an enum type.
    let (ret_ty, struct_ret_name, enum_ret_name) = match &fn_def.ret_ty {
        None => (IrTy::Unit, None, None),
        Some(ty) => {
            match lower_ty(ty, source) {
                Ok(t) => (t, None, None),
                Err(_) => {
                    // Check if the return type is a known struct or enum.
                    if let TyKind::Path(segs) = &ty.kind {
                        if segs.len() == 1 {
                            let ret_name = segs[0].text(source);
                            if struct_defs.contains_key(ret_name) {
                                // Function returning a struct type.
                                // Use Unit as a placeholder IrTy; the actual return
                                // is handled via RetFields in the body lowering below.
                                (IrTy::Unit, Some(ret_name.to_owned()), None)
                            } else if enum_defs.contains_key(ret_name) {
                                // FLS §9, §15: Free function returning an enum type.
                                // Use Unit as a placeholder IrTy; the actual return
                                // is handled via RetFields after `lower_enum_expr_into`.
                                (IrTy::Unit, None, Some(ret_name.to_owned()))
                            } else {
                                return Err(LowerError::Unsupported(format!(
                                    "return type `{ret_name}` (not a known struct, enum, or primitive)"
                                )));
                            }
                        } else {
                            return Err(LowerError::Unsupported("multi-segment return type".into()));
                        }
                    } else {
                        return Err(LowerError::Unsupported("complex return type".into()));
                    }
                }
            }
        }
    };

    let body = match &fn_def.body {
        None => {
            return Err(LowerError::Unsupported(
                "extern / bodyless functions".into(),
            ));
        }
        Some(block) => block,
    };

    let mut ctx = LowerCtx::new(source, ret_ty, struct_defs, tuple_struct_defs, enum_defs, method_self_kinds, mut_self_scalar_return_fns, struct_return_fns, struct_return_free_fns, enum_return_fns, struct_return_methods, const_vals, static_names, struct_field_types, struct_field_offsets, struct_sizes, start_label);

    // FLS §9: Spill incoming parameters from ARM64 registers x0..x{n-1}
    // to stack slots. Each parameter slot is allocated in parameter order
    // so that subsequent path expressions emit `Load { slot }`.
    //
    // ARM64 ABI: the first 8 integer/pointer parameters arrive in x0–x7.
    // Spilling them to the stack normalises parameter access to the same
    // `Load` instruction used for let-binding locals, keeping codegen simple.
    //
    // Cache-line note: each spill is one `str` instruction (4 bytes); two
    // spills per 8-byte stack pair keep the spill sequence cache-aligned.
    // FLS §9: Spill parameters from ARM64 registers to stack slots.
    // `reg_idx` tracks the current register number, which may advance by more
    // than 1 per parameter when an enum type occupies multiple registers
    // (discriminant + field registers).
    let mut reg_idx: usize = 0;

    // FLS §10.1: If this is a method with a self parameter, spill the self
    // value from leading registers.
    //
    // For struct self: each field arrives in a separate register (x0..x{N-1}).
    // For enum self: the discriminant arrives in x0, then field registers follow
    // (x1..x{max_fields}), matching the enum-parameter calling convention.
    //
    // `&self` and `self` are lowered identically at this milestone: the
    // caller passes each field as an individual integer register argument
    // (a value copy). The method body can read but not mutate the original.
    //
    // FLS §10.1, §11, §15: impl blocks are legal for both struct and enum types.
    //
    // Cache-line note: N field spills = N `str` instructions (4 bytes each).
    if let Some(type_name) = impl_type {
        if let Some(field_names) = struct_defs.get(type_name) {
            // Struct self parameter: one register per field.
            let n_fields = field_names.len();
            if n_fields > 0 {
                if reg_idx + n_fields > 8 {
                    return Err(LowerError::Unsupported(
                        "self fields exceed ARM64 register window".into(),
                    ));
                }
                let base_slot = ctx.alloc_slot()?;
                for _ in 1..n_fields {
                    ctx.alloc_slot()?;
                }
                for fi in 0..n_fields {
                    ctx.instrs.push(Instr::Store {
                        src: (reg_idx + fi) as u8,
                        slot: base_slot + fi as u8,
                    });
                }
                // Register `self` as a struct variable pointing to base_slot.
                // `self` is a keyword but &'static str coerces to &'src str.
                ctx.locals.insert("self", base_slot);
                ctx.local_struct_types.insert(base_slot, type_name.to_owned());
                reg_idx += n_fields;
            } else {
                // Unit struct: no fields to pass. Register `self` with a dummy slot.
                let base_slot = ctx.alloc_slot()?;
                ctx.locals.insert("self", base_slot);
                ctx.local_struct_types.insert(base_slot, type_name.to_owned());
            }
        } else if let Some(variants) = enum_defs.get(type_name) {
            // Enum self parameter: discriminant + up to max_fields registers.
            //
            // FLS §15: Enum values carry a discriminant (tag) and variant-specific
            // fields. The calling convention mirrors enum parameter passing:
            // x{reg_idx} = discriminant, x{reg_idx+1}..x{reg_idx+max_fields} = fields.
            //
            // FLS §6.1.2:37–45: All spills are runtime store instructions.
            // Cache-line note: (1 + max_fields) × 4-byte `str` per enum self spill.
            let max_fields = variants.values().map(|(_, names)| names.len()).max().unwrap_or(0);
            let regs_needed = 1 + max_fields;
            if reg_idx + regs_needed > 8 {
                return Err(LowerError::Unsupported(
                    "enum self exceeds ARM64 register window".into(),
                ));
            }
            let base_slot = ctx.alloc_slot()?;
            for _ in 0..max_fields {
                ctx.alloc_slot()?;
            }
            // Spill discriminant.
            ctx.instrs.push(Instr::Store { src: reg_idx as u8, slot: base_slot });
            // Spill field registers.
            for fi in 0..max_fields {
                ctx.instrs.push(Instr::Store {
                    src: (reg_idx + fi + 1) as u8,
                    slot: base_slot + 1 + fi as u8,
                });
            }
            // Register `self` as an enum variable so match expressions on `self`
            // use the discriminant-based dispatch path.
            ctx.locals.insert("self", base_slot);
            ctx.local_enum_types.insert(base_slot, type_name.to_owned());
            reg_idx += regs_needed;
        } else if let Some(&n_fields) = tuple_struct_defs.get(type_name) {
            // FLS §14.2, §10.1: Tuple struct self parameter.
            //
            // A tuple struct with N fields is passed as N consecutive registers
            // (one per field), identical to anonymous tuple and named struct
            // calling conventions. Spill to N consecutive stack slots; register
            // in `local_tuple_lens` for `.0`/`.1` field access and in
            // `local_tuple_struct_types` for method call dispatch.
            //
            // FLS §6.1.2:37–45: All spills are runtime store instructions.
            // Cache-line note: N × 4-byte `str` per self spill.
            if n_fields > 0 {
                if reg_idx + n_fields > 8 {
                    return Err(LowerError::Unsupported(
                        "tuple struct self fields exceed ARM64 register window".into(),
                    ));
                }
                let base_slot = ctx.alloc_slot()?;
                for _ in 1..n_fields {
                    ctx.alloc_slot()?;
                }
                for fi in 0..n_fields {
                    ctx.instrs.push(Instr::Store {
                        src: (reg_idx + fi) as u8,
                        slot: base_slot + fi as u8,
                    });
                }
                ctx.locals.insert("self", base_slot);
                ctx.local_tuple_lens.insert(base_slot, n_fields);
                ctx.local_tuple_struct_types.insert(base_slot, type_name.to_owned());
                reg_idx += n_fields;
            } else {
                // Zero-field tuple struct: allocate a dummy slot for `self`.
                let base_slot = ctx.alloc_slot()?;
                ctx.locals.insert("self", base_slot);
                ctx.local_tuple_struct_types.insert(base_slot, type_name.to_owned());
            }
        } else {
            return Err(LowerError::Unsupported(format!(
                "impl for unknown type `{type_name}`"
            )));
        }
    }
    for param in fn_def.params.iter() {
        let param_name = param.name.text(source);

        // FLS §15: Enum type parameters — `fn f(o: Opt)`.
        // FLS §11 / §6.12.2: Struct type parameters — `fn f(s: S)`.
        //
        // A struct value with N fields occupies N consecutive registers: one
        // register per field in declaration order. This matches the method
        // self-parameter calling convention in `lower_fn` (when `impl_type`
        // is set). The N registers are spilled to N consecutive stack slots
        // so that field access uses the same Load/Store paths as let-bindings.
        //
        // An enum value with max N fields occupies N+1 consecutive registers:
        // register reg_idx holds the discriminant; registers reg_idx+1..=reg_idx+N
        // hold the fields. All registers are spilled to consecutive stack slots so
        // that TupleStruct patterns can access fields via `base_slot + 1 + fi`.
        //
        // FLS §6.1.2:37–45: All spills are runtime store instructions.
        // Cache-line note: each spill is one `str` instruction (4 bytes).
        if let TyKind::Path(segs) = &param.ty.kind
            && segs.len() == 1
        {
            let type_name = segs[0].text(source);

            // FLS §11 / §6.12.2: Struct parameter — pass each field as a
            // separate register, matching the method self-parameter convention.
            //
            // For nested structs (e.g., `fn width(r: Rect)` where `Rect` has two
            // `Point` fields), the total slot count from `struct_sizes` is used
            // rather than the number of direct fields. This handles the case where
            // `Rect` has 2 declared fields but 4 total slots (2 per `Point`).
            //
            // FLS §4.11: Struct layout — fields are stored in declaration order.
            // FLS §6.11: Struct expressions — field initializers are evaluated in
            // declaration order and each occupies one or more consecutive slots.
            //
            // Cache-line note: N total slots emit N × 4-byte `str` spill instructions.
            // For a 2-field flat struct: 8 bytes (2 instructions, same as before).
            // For a 2-field nested struct like Rect: 16 bytes (4 instructions).
            if let Some(field_names) = struct_defs.get(type_name) {
                // Use total slot count (accounts for nested struct fields).
                let n_slots = struct_sizes.get(type_name).copied().unwrap_or(field_names.len());
                let regs_needed = n_slots.max(1); // unit structs use 0 slots but need 1 slot allocated
                if n_slots > 0 && reg_idx + n_slots > 8 {
                    return Err(LowerError::Unsupported(
                        "struct parameter exceeds ARM64 register window (>8 total registers)".into(),
                    ));
                }
                let base_slot = ctx.alloc_slot()?;
                for _ in 1..n_slots {
                    ctx.alloc_slot()?;
                }
                ctx.locals.insert(param_name, base_slot);
                ctx.local_struct_types.insert(base_slot, type_name.to_owned());
                // Spill each slot register to its stack slot.
                for fi in 0..n_slots {
                    ctx.instrs.push(Instr::Store {
                        src: (reg_idx + fi) as u8,
                        slot: base_slot + fi as u8,
                    });
                }
                reg_idx += regs_needed;
                continue;
            }

            if let Some(variants) = enum_defs.get(type_name) {
                let max_fields = variants.values().map(|(_, names)| names.len()).max().unwrap_or(0);
                let regs_needed = 1 + max_fields;
                if reg_idx + regs_needed > 8 {
                    return Err(LowerError::Unsupported(
                        "enum parameter exceeds ARM64 register window (>8 total registers)".into(),
                    ));
                }
                let base_slot = ctx.alloc_slot()?;
                for _ in 0..max_fields {
                    ctx.alloc_slot()?;
                }
                ctx.locals.insert(param_name, base_slot);
                ctx.local_enum_types.insert(base_slot, type_name.to_owned());
                // Spill discriminant register.
                ctx.instrs.push(Instr::Store { src: reg_idx as u8, slot: base_slot });
                // Spill field registers.
                for fi in 0..max_fields {
                    ctx.instrs.push(Instr::Store {
                        src: (reg_idx + fi + 1) as u8,
                        slot: base_slot + 1 + fi as u8,
                    });
                }
                reg_idx += regs_needed;
                continue;
            }

            // FLS §14.2, §10.1: Tuple struct parameter — `fn f(w: Wrap)`.
            //
            // A tuple struct with N fields is passed as N consecutive registers,
            // identical to the tuple struct self-parameter calling convention.
            // Spill to N consecutive stack slots; register in `local_tuple_lens`
            // for `.0`/`.1` field access and `local_tuple_struct_types` for
            // method call dispatch (so `w.val()` resolves to `Wrap::val`).
            //
            // FLS §6.1.2:37–45: All spills are runtime store instructions.
            // Cache-line note: N × 4-byte `str` per parameter spill.
            if let Some(&n_fields) = tuple_struct_defs.get(type_name) {
                if n_fields > 0 {
                    if reg_idx + n_fields > 8 {
                        return Err(LowerError::Unsupported(
                            "tuple struct parameter exceeds ARM64 register window (>8 total registers)".into(),
                        ));
                    }
                    let base_slot = ctx.alloc_slot()?;
                    for _ in 1..n_fields {
                        ctx.alloc_slot()?;
                    }
                    ctx.locals.insert(param_name, base_slot);
                    ctx.local_tuple_lens.insert(base_slot, n_fields);
                    ctx.local_tuple_struct_types.insert(base_slot, type_name.to_owned());
                    for fi in 0..n_fields {
                        ctx.instrs.push(Instr::Store {
                            src: (reg_idx + fi) as u8,
                            slot: base_slot + fi as u8,
                        });
                    }
                    reg_idx += n_fields;
                } else {
                    // Zero-field tuple struct: allocate a dummy slot.
                    let base_slot = ctx.alloc_slot()?;
                    ctx.locals.insert(param_name, base_slot);
                    ctx.local_tuple_struct_types.insert(base_slot, type_name.to_owned());
                }
                continue;
            }
        }

        // FLS §9: i32 and bool parameters — one register each.
        if reg_idx >= 8 {
            return Err(LowerError::Unsupported(
                "functions with more than 8 parameters (exceeds ARM64 register window)".into(),
            ));
        }
        let param_ty = lower_ty(&param.ty, source)?;
        // Only i32 and bool parameters are supported (both use integer registers).
        // FLS §4.3: bool is passed as a 32-bit integer register on ARM64.
        // FLS §4.1: i32 parameters occupy one 64-bit register (x0–x7).
        // FLS §4.1: All primitive integer types and bool are supported as
        // parameters. Each uses one 64-bit ARM64 register (x0–x7).
        if !matches!(param_ty, IrTy::I32 | IrTy::Bool | IrTy::U32) {
            return Err(LowerError::Unsupported(
                "parameter type other than i32/bool/u32/i64/u64/usize/isize/i8/i16/u8/u16".into(),
            ));
        }
        let slot = ctx.alloc_slot()?;
        ctx.locals.insert(param_name, slot);
        // Spill parameter register reg_idx (arm64 x{reg_idx}) to its stack slot.
        ctx.instrs.push(Instr::Store { src: reg_idx as u8, slot });
        reg_idx += 1;
    }

    // FLS §10.1: For `&mut self` methods with unit return type, emit `RetFields`
    // instead of `Ret(Unit)`. This writes modified self fields back to the caller
    // via x0..x{N-1} on return. The caller uses `CallMut` to store them back.
    //
    // Limitation: early `return` expressions inside the body still emit `Ret(Unit)`,
    // bypassing the write-back. Only methods that terminate via their tail expression
    // are fully correct at this milestone.
    //
    // FLS §10.1 AMBIGUOUS: The spec does not define the calling convention for
    // `&mut self` in terms of register passing. Galvanic uses a value-copy convention
    // (fields passed as registers, written back on return) for simplicity.
    if self_kind == Some(SelfKind::RefMut) {
        // Determine the number of self fields. The impl_type is guaranteed Some
        // when self_kind is Some (enforced by the parser / call site).
        let type_name = impl_type.expect("impl_type must be set when self_kind is RefMut");
        if enum_defs.contains_key(type_name) {
            return Err(LowerError::Unsupported(
                "&mut self methods on enum types not yet supported".into(),
            ));
        }
        let n_fields = if let Some(f) = struct_defs.get(type_name) {
            f.len() as u8
        } else if let Some(&n) = tuple_struct_defs.get(type_name) {
            // FLS §14.2, §10.1: &mut self on tuple struct — N consecutive slots.
            n as u8
        } else {
            0
        };
        // Lower the body, capturing the tail value.
        // Fields in the method's local slots will have been updated by body code.
        let tail_val = ctx.lower_block_to_value(body, &ret_ty)?;
        if ret_ty == IrTy::Unit {
            // Unit return: emit RetFields to write back modified fields.
            // Emit RetFields: loads each field from slot 0..n_fields-1 into x0..x{N-1}
            // then performs the normal epilogue.
            ctx.instrs.push(Instr::RetFields { base_slot: 0, n_fields });
        } else {
            // Scalar return: emit RetFieldsAndValue — fields in x0..x{N-1}, value in x{N}.
            //
            // FLS §10.1: &mut self methods may return any type. Galvanic extends the
            // write-back convention: fields in x0..x{N-1}, scalar return in x{N}.
            //
            // FLS §10.1 AMBIGUOUS: The spec does not define the calling convention for
            // &mut self with non-unit return type. This extension is consistent with the
            // existing register-packing convention.
            let val_reg = ctx.val_to_reg(tail_val)?;
            ctx.instrs.push(Instr::RetFieldsAndValue { base_slot: 0, n_fields, val_reg });
        }
    } else if let Some(ref struct_name) = struct_ret_name {
        // FLS §10.1: Associated function returning a struct type.
        // The tail expression must be a struct literal. Lower all statements,
        // then store the struct literal fields to consecutive slots, then emit
        // RetFields to return them in x0..x{N-1}.
        //
        // ARM64 ABI: multiple return values packed into x0..x{N-1} (small structs).
        // The call site uses CallMut-style write-back to store them into the
        // destination variable's slots.
        //
        // FLS §10.1 AMBIGUOUS: The spec does not define the calling convention for
        // returning struct types from associated functions. Galvanic uses the same
        // register-packing convention as &mut self (fields in x0..x{N-1}).
        //
        // Cache-line note: N field stores (4 bytes each) + RetFields ldr sequence
        // (N loads) = 2N instructions before the epilogue.
        let field_names = struct_defs.get(struct_name.as_str())
            .ok_or_else(|| LowerError::Unsupported(format!("unknown struct `{struct_name}`")))?
            .clone();
        let n_fields = field_names.len();

        // Lower all statements.
        for stmt in &body.stmts {
            ctx.lower_stmt(stmt)?;
        }

        // The tail expression must be a struct literal.
        let tail = body.tail.as_deref().ok_or_else(|| {
            LowerError::Unsupported(format!(
                "associated function returning `{struct_name}` must end with a struct literal"
            ))
        })?;
        let ExprKind::StructLit { name: sn, fields: lit_fields, .. } = &tail.kind else {
            return Err(LowerError::Unsupported(format!(
                "associated function returning `{struct_name}`: tail must be a struct literal"
            )));
        };

        let actual_struct_name = sn.text(source);
        if actual_struct_name != struct_name.as_str() {
            return Err(LowerError::Unsupported(format!(
                "associated function declared to return `{struct_name}` but tail is `{actual_struct_name}`"
            )));
        }

        // Allocate consecutive slots for the return struct fields.
        let base_slot = ctx.alloc_slot()?;
        for _ in 1..n_fields {
            ctx.alloc_slot()?;
        }

        // Store each field in declaration order.
        // FLS §6.11: Field initializers evaluated in source order, stored in
        // declaration order for layout stability.
        for (field_idx, field_name) in field_names.iter().enumerate() {
            let slot = base_slot + field_idx as u8;
            let field_init = lit_fields
                .iter()
                .find(|(f, _)| f.text(source) == field_name.as_str())
                .ok_or_else(|| {
                    LowerError::Unsupported(format!(
                        "missing field `{field_name}` in `{struct_name}` literal"
                    ))
                })?;
            let val = ctx.lower_expr(&field_init.1, &IrTy::I32)?;
            let src = ctx.val_to_reg(val)?;
            ctx.instrs.push(Instr::Store { src, slot });
        }

        // Emit RetFields: loads fields from base_slot..base_slot+n_fields-1
        // into x0..x{N-1} before the epilogue.
        ctx.instrs.push(Instr::RetFields { base_slot, n_fields: n_fields as u8 });
    } else if let Some(ref enum_name) = enum_ret_name {
        // FLS §9, §15: Free function returning an enum type.
        //
        // The callee stores the enum (discriminant + fields) into consecutive
        // stack slots allocated here, then returns them in x0..x{1+max_fields-1}
        // via RetFields. The caller uses CallMut-style write-back to receive them.
        //
        // ARM64 ABI: up to 8 integer return registers (x0..x7). For the common
        // case of 1-field enums (e.g. `Option<i32>-like`), two registers suffice.
        //
        // FLS §15 AMBIGUOUS: The spec does not define a calling convention for
        // enum-returning functions. Galvanic extends the register-packing
        // convention already used for struct returns and enum parameter passing:
        // discriminant in x0, field[0] in x1, field[1] in x2, etc.
        //
        // Cache-line note: (1 + max_fields) × 4-byte ldr in RetFields sequence.
        let max_fields = enum_defs
            .get(enum_name.as_str())
            .map(|v| v.values().map(|(_, names)| names.len()).max().unwrap_or(0))
            .unwrap_or(0);
        let n_ret = 1 + max_fields as u8;

        // Lower all statements.
        for stmt in &body.stmts {
            ctx.lower_stmt(stmt)?;
        }

        // Lower the tail expression into return slots.
        let tail = body.tail.as_deref().ok_or_else(|| {
            LowerError::Unsupported(format!(
                "function returning `{enum_name}` must have a tail expression"
            ))
        })?;

        // Allocate consecutive slots for the enum return value:
        // slot ret_base = discriminant, slot ret_base+1..ret_base+max_fields = fields.
        let ret_base = ctx.alloc_slot()?;
        for _ in 0..max_fields {
            ctx.alloc_slot()?;
        }

        ctx.lower_enum_expr_into(tail, ret_base, max_fields)?;

        // RetFields: load discriminant + fields into x0..x{n_ret-1} before epilogue.
        ctx.instrs.push(Instr::RetFields { base_slot: ret_base, n_fields: n_ret });
    } else {
        ctx.lower_block(body, &ret_ty)?;
    }

    let body_instrs = ctx.instrs;
    let stack_slots = ctx.next_slot;
    let saves_lr = ctx.has_calls;
    let next_label = ctx.next_label;
    Ok((IrFn { name, ret_ty, body: body_instrs, stack_slots, saves_lr }, next_label))
}

// ── Type lowering ────────────────────────────────────────────────────────────

/// Lower a type expression to an `IrTy`.
///
/// FLS §4: Types. Supports `i32`, `bool`, and `()`.
///
/// FLS §4.3: The boolean type `bool` has two values, `true` (1) and `false`
/// (0). On ARM64, booleans are passed and returned in 32-bit integer registers
/// (the same layout as `i32`), so `bool` maps to `IrTy::I32` in the IR.
/// This is consistent with `LitBool` materialisation (FLS §6.1.3), which
/// already represents `true`/`false` as immediates 1/0.
fn lower_ty(ty: &crate::ast::Ty, source: &str) -> Result<IrTy, LowerError> {
    match &ty.kind {
        TyKind::Unit => Ok(IrTy::Unit),
        TyKind::Path(segments) if segments.len() == 1 => {
            match segments[0].text(source) {
                // FLS §4.1: Signed integer types. i8/i16/i64/isize all use
                // signed 64-bit registers on ARM64. Width truncation for
                // narrower types is deferred (FLS §4.1 AMBIGUOUS on ABI layout).
                "i32" | "i8" | "i16" | "i64" | "isize" => Ok(IrTy::I32),
                // FLS §4.3: bool is a distinct type in the IR so that `!` can
                // emit logical NOT (eor, XOR with 1) rather than bitwise NOT (mvn).
                // On ARM64, bool and i32 share the same register layout (0/1 as i64),
                // but the semantics of `!` differ.
                "bool" => Ok(IrTy::Bool),
                // FLS §4.1: Unsigned integer types. u8/u16/u32/u64/usize all
                // use 64-bit registers on ARM64. Unsigned division uses `udiv`
                // and unsigned right shift uses `lsr` (see IrBinOp::UDiv/UShr).
                // Width truncation for narrower types is deferred.
                "u8" | "u16" | "u32" | "u64" | "usize" => Ok(IrTy::U32),
                name => Err(LowerError::Unsupported(format!("type `{name}`"))),
            }
        }
        // FLS §4.8: Reference types `&T` and `&mut T`. A reference is a pointer —
        // an 8-byte address on ARM64. Galvanic represents reference-typed parameters
        // and locals using `IrTy::I32` (a 64-bit register), matching the inner type's
        // IR representation. The pointer IS the value in the register; `*x` dereferences
        // it via `LoadPtr { dst, src }`.
        //
        // FLS §4.8: "A reference type is a kind of pointer type."
        // Cache-line note: references occupy one 8-byte register slot — same as i32/i64.
        TyKind::Ref { inner, .. } => lower_ty(inner, source),
        _ => Err(LowerError::Unsupported("complex type".into())),
    }
}

// ── Loop context ─────────────────────────────────────────────────────────────

/// Context for a loop being lowered.
///
/// Pushed onto `LowerCtx::loop_stack` when entering a loop expression and
/// popped when the loop body has been fully lowered.
///
/// `break` consults the top entry for `exit_label` to branch past the loop.
/// `continue` consults `header_label` to jump back to the loop top.
///
/// FLS §6.15.2: Infinite loop expressions.
/// FLS §6.15.3: While loop expressions.
/// FLS §6.15.6: Break expressions.
/// FLS §6.15.7: Continue expressions.
struct LoopCtx {
    /// Label at the top of the loop. Target for `continue` and the back-edge.
    header_label: u32,
    /// Label immediately after the loop. Target for `break`.
    exit_label: u32,
    /// Stack slot for the loop's result value, allocated when the loop body
    /// contains a `break <value>` expression. Only `loop` expressions support
    /// break-with-value (FLS §6.15.6). `None` for `while` and `for` loops.
    break_slot: Option<u8>,
    /// The expected result type of the loop expression. Needed so that
    /// `break <expr>` can lower the value with the correct type context.
    /// Matches the `ret_ty` passed to `lower_expr` for the `loop` node.
    break_ret_ty: IrTy,
}

// ── Lowering context ─────────────────────────────────────────────────────────

/// Mutable state threaded through the lowering of a single function body.
///
/// Tracks the instruction buffer, virtual register counter, stack slot
/// counter, label counter, and the local variable map. All instructions for
/// one function are accumulated here and transferred to `IrFn::body` at the end.
///
/// FLS §8.1: Each `let` binding allocates a new stack slot and registers
/// the variable name in `locals`. Path expressions consult `locals` to
/// find the slot to load from.
struct LowerCtx<'src> {
    source: &'src str,
    instrs: Vec<Instr>,
    next_reg: u8,
    /// Next stack slot index. Slot `s` maps to byte offset `s * 8` on the
    /// stack frame. The frame size is rounded up to 16 bytes in codegen.
    next_slot: u8,
    /// Next label ID for branch targets.
    ///
    /// FLS §6.17: if expressions require unique labels for else and end
    /// targets. Labels are monotonically increasing per function.
    next_label: u32,
    /// Maps local variable names to their stack slot indices.
    ///
    /// FLS §8.1: Let statements introduce bindings into the current scope.
    /// FLS §6.3: Path expressions are resolved here before emitting Load.
    ///
    /// Limitation: this flat map does not model nested scopes. Variables
    /// introduced inside an if branch remain visible after it. Proper lexical
    /// scoping is deferred to a future milestone.
    locals: HashMap<&'src str, u8>,
    /// Whether this function emits any `Call` instructions.
    ///
    /// Set to `true` when `Instr::Call` is pushed. Used to set `IrFn::saves_lr`
    /// so codegen knows to save/restore x30 around calls.
    ///
    /// FLS §6.12.1: Call expressions make a function non-leaf; the link
    /// register must be preserved so the function can return correctly.
    has_calls: bool,
    /// Stack of enclosing loop contexts.
    ///
    /// Each entry corresponds to one loop currently being lowered. The top
    /// (last) entry is the innermost loop — the target of an unqualified
    /// `break` or `continue`.
    ///
    /// FLS §6.15.6: "A break expression without a label exits the innermost
    /// enclosing loop expression."
    /// FLS §6.15.7: "A continue expression without a label continues the
    /// innermost enclosing loop expression."
    loop_stack: Vec<LoopCtx>,

    /// The return type of the current function.
    ///
    /// Stored so that `return` expressions (FLS §6.19) can lower the returned
    /// value using the correct type, regardless of the expression context type
    /// (`ret_ty`) passed to `lower_expr` at the point of the `return`.
    ///
    /// For example, `return 42` appearing inside a unit-typed `if` body still
    /// needs to lower `42` as `IrTy::I32` if the enclosing function returns i32.
    fn_ret_ty: IrTy,

    /// Struct type definitions: maps struct name → field names in declaration order.
    ///
    /// FLS §14: Struct definitions. Used to look up field indices during
    /// struct literal construction and field access lowering.
    ///
    /// Cache-line note: field index determines the stack slot offset from the
    /// struct's base slot. Field `i` is at `base_slot + i`, each slot 8 bytes.
    struct_defs: &'src HashMap<String, Vec<String>>,

    /// Tuple struct definitions: maps struct name → field count.
    ///
    /// FLS §14.2: Tuple struct items. When `let p = Point(a, b)` is lowered
    /// and `Point` is a known tuple struct, N consecutive stack slots are
    /// allocated and the base slot is registered in `local_tuple_lens`.
    /// Subsequent `.0`, `.1` field accesses use the existing tuple field
    /// access path (FLS §6.10).
    ///
    /// Cache-line note: same layout as anonymous tuples — N consecutive
    /// 8-byte slots, with `.i` at `base_slot + i`.
    tuple_struct_defs: &'src HashMap<String, usize>,

    /// Enum type definitions: maps enum name → (variant name → discriminant).
    ///
    /// FLS §15: Enumerations. Unit variants are assigned integer discriminants
    /// (0, 1, 2, ...) in declaration order. Used to resolve path expressions
    /// (`Color::Red`) and path patterns (`Color::Red` in match arms) to their
    /// integer discriminant values.
    ///
    /// Cache-line note: enum values are represented as i32, occupying one
    /// 8-byte stack slot — identical to any other i32 local.
    enum_defs: &'src EnumDefs,

    /// Method self-kind registry: maps mangled method name → SelfKind.
    ///
    /// FLS §10.1: `&mut self` methods use a different calling convention
    /// (write-back via `CallMut`) than `&self` or `self` methods (`Call`).
    /// At the call site, this registry is consulted to select the right
    /// instruction.
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    method_self_kinds: &'src HashMap<String, SelfKind>,

    /// `&mut self` scalar-return registry: mangled names of `&mut self` methods
    /// that return a scalar (non-unit, non-struct, non-enum) type.
    ///
    /// FLS §10.1: `&mut self` methods may return any type. When the return type
    /// is scalar, the call site emits `CallMutReturn` instead of `CallMut` to
    /// capture the scalar return value from x{N} (after the field write-backs).
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    mut_self_scalar_return_fns: &'src std::collections::HashSet<String>,

    /// Struct-returning associated function registry: mangled name → struct type name.
    ///
    /// FLS §10.1: Associated functions that return a struct type use a
    /// write-back calling convention (fields returned in x0..x{N-1}).
    /// At the call site, this registry identifies such functions so the
    /// caller can emit `CallMut`-style write-back into the destination slots.
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    struct_return_fns: &'src HashMap<String, String>,

    /// Struct-returning free function registry: fn name → struct type name.
    ///
    /// FLS §9: Free functions that return a named struct type use the same
    /// write-back calling convention as struct-returning associated functions:
    /// fields returned in x0..x{N-1} via RetFields; the call site writes them
    /// to the destination variable's consecutive stack slots via CallMut.
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    struct_return_free_fns: &'src HashMap<String, String>,

    /// Struct-returning `&self` instance method registry: mangled name → struct type name.
    ///
    /// FLS §10.1: `&self` instance methods that return a named struct type use
    /// the same write-back calling convention as struct-returning associated
    /// functions. At the call site (in a `let` binding), the caller emits
    /// `CallMut` to write x0..x{N-1} into the destination variable's slots.
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    struct_return_methods: &'src HashMap<String, String>,

    /// Enum-returning free function registry: fn name → enum type name.
    ///
    /// FLS §9, §15: Free functions that return an enum type use the same
    /// write-back calling convention: discriminant + fields returned in
    /// x0..x{1+max_fields-1} via RetFields; the call site writes them to
    /// the destination enum variable's stack slots via CallMut.
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    enum_return_fns: &'src HashMap<String, String>,

    /// Maps a struct variable's base stack slot to its struct type name.
    ///
    /// FLS §6.13: Field access expressions need the struct type to compute
    /// the slot offset. When `let p = Point { x, y }` is lowered, `p`'s
    /// base slot is registered here with type `"Point"`.
    local_struct_types: HashMap<u8, String>,

    /// Maps an enum variable's base stack slot to its enum type name.
    ///
    /// FLS §15: Enum values with tuple variants occupy multiple consecutive
    /// slots: slot 0 = discriminant, slots 1..N = positional fields.
    /// When `let x = Opt::Some(v)` is lowered, `x`'s base slot is
    /// registered here with type `"Opt"`. Used by TupleStruct pattern
    /// lowering to locate field slots from the base slot.
    local_enum_types: HashMap<u8, String>,

    /// Maps an array variable's base stack slot to its element count.
    ///
    /// FLS §6.8: Array variables occupy N consecutive 8-byte stack slots,
    /// where N is the element count. When `let a = [e0, e1, e2]` is lowered,
    /// `a`'s base slot is registered here with count 3. Index expressions
    /// look up this map to emit `LoadIndexed` with the correct base slot.
    ///
    /// Cache-line note: populated once per array let binding; read once per
    /// index expression. Not on a hot path.
    local_array_lens: HashMap<u8, usize>,

    /// Maps a tuple variable's base stack slot to its field count.
    ///
    /// FLS §6.10: Tuple values occupy N consecutive 8-byte stack slots where
    /// N is the number of elements. When `let t = (a, b)` is lowered, `t`'s
    /// base slot is registered here with count 2. Field access `.0`, `.1`
    /// is lowered to `base_slot + index`.
    ///
    /// Cache-line note: same layout as struct fields or array elements —
    /// N consecutive slots per tuple.
    local_tuple_lens: HashMap<u8, usize>,

    /// Maps a tuple struct variable's base stack slot to its type name.
    ///
    /// FLS §14.2: Tuple struct types. When `let p = Point(a, b)` or `self`
    /// in an `impl Point` method is lowered, the base slot is registered here
    /// with type `"Point"`. This lets method call dispatch compute the mangled
    /// name `Point__method_name` for tuple struct receivers.
    ///
    /// Cache-line note: populated once per let binding or method self-spill;
    /// read once per method call on a tuple struct receiver. Not on a hot path.
    local_tuple_struct_types: HashMap<u8, String>,

    /// Compile-time constant values: maps const name → i32.
    ///
    /// FLS §7.1: Constant items. Every use of a constant is replaced with its
    /// value. When a path expression `FOO` resolves to a known const name,
    /// `LoadImm(value)` is emitted instead of `Load { slot }`.
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    const_vals: &'src HashMap<String, i32>,

    /// Static variable names: the set of names declared as `static` items.
    ///
    /// FLS §7.2: Static items. When a path expression `FOO` resolves to a known
    /// static name, `LoadStatic { dst, name }` is emitted instead of `Load { slot }`.
    /// This causes the codegen to emit ADRP + ADD + LDR at the use site.
    ///
    /// FLS §7.2:15: All references to a static go through its memory address —
    /// unlike const substitution, static reads are actual memory loads.
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    static_names: &'src std::collections::HashSet<String>,

    /// Per-field struct type names for nested struct support.
    ///
    /// FLS §6.11, §6.13: Struct-type fields require more than one stack slot and
    /// need special handling in both construction (recursive literal storing) and
    /// field access (chained offset computation).
    ///
    /// `None` = scalar field (i32, bool, u32, etc.), `Some(name)` = struct field.
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    struct_field_types: &'src HashMap<String, Vec<Option<String>>>,

    /// Per-struct field slot offsets.
    ///
    /// FLS §6.13: Chained field access `s.b.x` requires computing the slot
    /// offset of field `b` within `s`, which equals the sum of sizes of preceding
    /// fields. For scalar fields this is 1; for struct-type fields it is that
    /// struct's total slot count (see `struct_sizes`).
    ///
    /// `struct_field_offsets["Outer"][i]` is the slot offset of field `i`
    /// relative to the base slot of an `Outer` variable.
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    struct_field_offsets: &'src HashMap<String, Vec<usize>>,

    /// Total stack slot count for each named struct type.
    ///
    /// FLS §4.11: Galvanic lays out struct fields in declaration order, each
    /// occupying 8 bytes. A struct with N scalar fields has size N. A struct
    /// with a struct-type field of size M has size (sum of field sizes).
    ///
    /// Used to allocate the correct number of consecutive stack slots when a
    /// nested struct variable is bound via `let`.
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    struct_sizes: &'src HashMap<String, usize>,
}

impl<'src> LowerCtx<'src> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        source: &'src str,
        fn_ret_ty: IrTy,
        struct_defs: &'src HashMap<String, Vec<String>>,
        tuple_struct_defs: &'src HashMap<String, usize>,
        enum_defs: &'src EnumDefs,
        method_self_kinds: &'src HashMap<String, SelfKind>,
        mut_self_scalar_return_fns: &'src std::collections::HashSet<String>,
        struct_return_fns: &'src HashMap<String, String>,
        struct_return_free_fns: &'src HashMap<String, String>,
        enum_return_fns: &'src HashMap<String, String>,
        struct_return_methods: &'src HashMap<String, String>,
        const_vals: &'src HashMap<String, i32>,
        static_names: &'src std::collections::HashSet<String>,
        struct_field_types: &'src HashMap<String, Vec<Option<String>>>,
        struct_field_offsets: &'src HashMap<String, Vec<usize>>,
        struct_sizes: &'src HashMap<String, usize>,
        start_label: u32,
    ) -> Self {
        LowerCtx {
            source,
            instrs: Vec::new(),
            next_reg: 0,
            next_slot: 0,
            next_label: start_label,
            locals: HashMap::new(),
            has_calls: false,
            loop_stack: Vec::new(),
            fn_ret_ty,
            struct_defs,
            tuple_struct_defs,
            enum_defs,
            method_self_kinds,
            mut_self_scalar_return_fns,
            struct_return_fns,
            struct_return_free_fns,
            enum_return_fns,
            struct_return_methods,
            const_vals,
            static_names,
            struct_field_types,
            struct_field_offsets,
            struct_sizes,
            local_struct_types: HashMap::new(),
            local_enum_types: HashMap::new(),
            local_array_lens: HashMap::new(),
            local_tuple_lens: HashMap::new(),
            local_tuple_struct_types: HashMap::new(),
        }
    }

    /// Allocate the next virtual register.
    fn alloc_reg(&mut self) -> Result<u8, LowerError> {
        let r = self.next_reg;
        self.next_reg = self.next_reg.checked_add(1).ok_or_else(|| {
            LowerError::Unsupported("exceeded 256 virtual registers".into())
        })?;
        Ok(r)
    }

    /// Allocate the next stack slot for a local variable.
    ///
    /// FLS §8.1: Each let binding gets one 8-byte slot.
    fn alloc_slot(&mut self) -> Result<u8, LowerError> {
        let s = self.next_slot;
        self.next_slot = self.next_slot.checked_add(1).ok_or_else(|| {
            LowerError::Unsupported("exceeded 256 stack slots".into())
        })?;
        Ok(s)
    }

    /// Allocate the next unique label ID.
    ///
    /// FLS §6.17: Each if expression needs two labels (else and end).
    /// Labels are function-scoped and monotonically increasing.
    fn alloc_label(&mut self) -> u32 {
        let id = self.next_label;
        self.next_label += 1;
        id
    }

    /// Resolve a literal or path pattern to its integer value.
    ///
    /// Used inside OR pattern inner loops where the alternative is a simple
    /// scalar value. `Pat::LitInt`, `Pat::NegLitInt`, `Pat::LitBool`, and
    /// `Pat::Path` (enum unit variant) are all valid. Other pattern kinds
    /// (ranges, wildcards, OR, Ident) are not supported here.
    ///
    /// FLS §5.2: Literal patterns. FLS §5.5 + §15: Path patterns.
    fn pat_scalar_imm(&self, pat: &Pat) -> Result<i32, LowerError> {
        match pat {
            Pat::LitInt(n) => Ok(*n as i32),
            Pat::NegLitInt(n) => Ok(-(*n as i32)),
            Pat::LitBool(b) => Ok(*b as i32),
            Pat::Path(segs) if segs.len() == 2 => {
                let enum_name = segs[0].text(self.source);
                let variant_name = segs[1].text(self.source);
                self.enum_defs
                    .get(enum_name)
                    .and_then(|v| v.get(variant_name))
                    .map(|(disc, _)| *disc)
                    .ok_or_else(|| LowerError::Unsupported(format!(
                        "unknown enum variant `{enum_name}::{variant_name}` in pattern"
                    )))
            }
            _ => Err(LowerError::Unsupported(
                "unsupported pattern kind inside OR pattern".into(),
            )),
        }
    }

    /// Ensure `val` is in a virtual register. If it's already a register,
    /// return it. If it's a constant, emit a `LoadImm`.
    fn val_to_reg(&mut self, val: IrValue) -> Result<u8, LowerError> {
        match val {
            IrValue::Reg(r) => Ok(r),
            IrValue::I32(n) => {
                let r = self.alloc_reg()?;
                self.instrs.push(Instr::LoadImm(r, n));
                Ok(r)
            }
            IrValue::Unit => Err(LowerError::Unsupported(
                "unit value used as arithmetic operand".into(),
            )),
        }
    }

    // ── Nested struct helpers ─────────────────────────────────────────────────

    /// Store a nested struct literal into consecutive stack slots starting at `base_slot`.
    ///
    /// Called when a struct literal has a field whose type is itself a named struct.
    /// For example, in `let r = Rect { a: Point { x: 1, y: 2 }, b: ... }`, when
    /// storing the `a` field we recurse with `struct_name = "Point"`.
    ///
    /// FLS §6.11: Struct expressions. Each field initializer is evaluated and
    /// stored into the corresponding slot of the nested struct's layout.
    ///
    /// FLS §6.1.2:37–45: All stores are runtime instructions — no const folding.
    ///
    /// Cache-line note: N scalar fields in the nested struct emit N `str` instructions
    /// (4 bytes each); the slots are consecutive so consecutive stores touch the same
    /// 64-byte cache lines as non-nested structs.
    fn store_nested_struct_lit(
        &mut self,
        expr: &Expr,
        base_slot: u8,
        struct_name: &str,
    ) -> Result<(), LowerError> {
        // The initializer must be a struct literal for the same type.
        let ExprKind::StructLit { fields: lit_fields, .. } = &expr.kind else {
            return Err(LowerError::Unsupported(format!(
                "expected struct literal `{struct_name} {{ .. }}` for nested struct field"
            )));
        };

        let field_names = self
            .struct_defs
            .get(struct_name)
            .ok_or_else(|| {
                LowerError::Unsupported(format!("unknown struct type `{struct_name}`"))
            })?
            .clone();

        let field_offsets = self
            .struct_field_offsets
            .get(struct_name)
            .cloned()
            .unwrap_or_default();
        let field_types = self
            .struct_field_types
            .get(struct_name)
            .cloned()
            .unwrap_or_default();

        for (field_idx, field_name) in field_names.iter().enumerate() {
            let field_offset = field_offsets.get(field_idx).copied().unwrap_or(field_idx);
            let dst_slot = base_slot + field_offset as u8;

            let field_init = lit_fields
                .iter()
                .find(|(f, _)| f.text(self.source) == field_name.as_str())
                .ok_or_else(|| {
                    LowerError::Unsupported(format!(
                        "missing field `{field_name}` in nested `{struct_name}` literal"
                    ))
                })?;

            let nested_ty = field_types.get(field_idx).cloned().flatten();
            if let Some(nested_type_name) = nested_ty {
                // Doubly-nested struct: recurse.
                self.store_nested_struct_lit(&field_init.1, dst_slot, &nested_type_name)?;
            } else {
                let val = self.lower_expr(&field_init.1, &IrTy::I32)?;
                let src = self.val_to_reg(val)?;
                self.instrs.push(Instr::Store { src, slot: dst_slot });
            }
        }
        Ok(())
    }

    /// Resolve a place expression (variable or chained field access) to its
    /// stack slot index and optional struct type name.
    ///
    /// Returns `(slot, Some(type_name))` for a struct-typed place (e.g., `r.b`
    /// where `b` is of type `Point`) or `(slot, None)` for a scalar place.
    ///
    /// This is the core of chained field access (`r.b.x`): the outer call
    /// resolves `r.b` to `(slot_of_b, Some("Point"))`, then resolves `.x`
    /// within `Point` to yield the final scalar slot.
    ///
    /// FLS §6.13: Field access expressions. The spec allows any number of
    /// nested field accesses; we support arbitrary depth by recursion.
    ///
    /// FLS §6.1.4: Place expressions. A field access on a place expression is
    /// itself a place expression — it can be the left operand of assignment.
    ///
    /// Cache-line note: each level of recursion costs one map lookup, not a
    /// memory access. The actual load is emitted by the caller.
    fn resolve_place(
        &self,
        expr: &Expr,
    ) -> Result<(u8, Option<String>), LowerError> {
        match &expr.kind {
            ExprKind::Path(segs) if segs.len() == 1 => {
                let var_name = segs[0].text(self.source);
                let slot = *self.locals.get(var_name).ok_or_else(|| {
                    LowerError::Unsupported(format!(
                        "undefined variable `{var_name}` in field access"
                    ))
                })?;
                // Check if this variable is a struct type — if so, further field
                // access is possible. Non-struct locals return None (scalar type).
                let ty = self.local_struct_types.get(&slot).cloned();
                Ok((slot, ty))
            }
            ExprKind::FieldAccess { receiver, field } => {
                // Recursively resolve the receiver to a (slot, optional_type) pair.
                let (recv_slot, recv_ty) = self.resolve_place(receiver)?;

                let field_name = field.text(self.source);

                // FLS §6.10: Tuple field access `.0`, `.1` — integer index.
                // Check tuple lens BEFORE the struct-type check, because tuples
                // have no entry in `local_struct_types` (recv_ty is None for them).
                if self.local_tuple_lens.contains_key(&recv_slot) {
                    let idx: usize = field_name.parse().map_err(|_| {
                        LowerError::Unsupported(format!(
                            "invalid tuple field index `{field_name}`"
                        ))
                    })?;
                    return Ok((recv_slot + idx as u8, None));
                }

                // FLS §6.13: Named struct field — look up field index and offset.
                let type_name = recv_ty.ok_or_else(|| {
                    LowerError::Unsupported(format!(
                        "field access on scalar value (field `{field_name}`)"
                    ))
                })?;

                let field_names = self.struct_defs.get(&type_name).ok_or_else(|| {
                    LowerError::Unsupported(format!("unknown struct type `{type_name}`"))
                })?;
                let field_idx = field_names
                    .iter()
                    .position(|n| n == field_name)
                    .ok_or_else(|| {
                        LowerError::Unsupported(format!(
                            "no field `{field_name}` in struct `{type_name}`"
                        ))
                    })?;

                let offset = self
                    .struct_field_offsets
                    .get(&type_name)
                    .and_then(|o| o.get(field_idx))
                    .copied()
                    .unwrap_or(field_idx);

                // Return the type of this field (None if scalar, Some if nested struct).
                let field_ty = self
                    .struct_field_types
                    .get(&type_name)
                    .and_then(|t| t.get(field_idx))
                    .cloned()
                    .flatten();

                Ok((recv_slot + offset as u8, field_ty))
            }
            _ => Err(LowerError::Unsupported(
                "unsupported place expression (only variables and field accesses supported)".into(),
            )),
        }
    }

    // ── Block lowering ────────────────────────────────────────────────────────

    /// Lower a block to a value without emitting `Ret`.
    ///
    /// Processes all statements in order, then lowers the tail expression
    /// and returns its value. Used by `lower_block` (function body) and by
    /// `lower_expr` for block expressions and if/else branches.
    ///
    /// FLS §6.4: Block expressions.
    /// FLS §6.1.2:37–45: Non-const function bodies must emit runtime code.
    fn lower_block_to_value(&mut self, block: &Block, ret_ty: &IrTy) -> Result<IrValue, LowerError> {
        for stmt in &block.stmts {
            self.lower_stmt(stmt)?;
        }
        match &block.tail {
            None => Ok(IrValue::Unit),
            Some(tail) => self.lower_expr(tail, ret_ty),
        }
    }

    /// Lower a function body block, appending a final `Ret` instruction.
    ///
    /// FLS §6.4: Block expressions.
    /// FLS §6.19: Return expressions — the tail is the block's return value.
    /// FLS §6.1.2:37–45: Non-const function bodies must emit runtime code.
    fn lower_block(&mut self, block: &Block, ret_ty: &IrTy) -> Result<(), LowerError> {
        let ret_val = self.lower_block_to_value(block, ret_ty)?;
        self.instrs.push(Instr::Ret(ret_val));
        Ok(())
    }

    /// Lower an expression that returns an enum value into pre-allocated stack slots.
    ///
    /// Stores discriminant at `base_slot`, positional fields at `base_slot+1..`.
    /// Used for functions returning enum types (FLS §9, §15).
    ///
    /// Handles:
    /// - Tuple variant constructor `Enum::Variant(field, ...)` — stores discriminant + fields
    /// - Unit variant path `Enum::Variant` — stores discriminant only
    /// - Enum variable path `x` (where x is a local enum variable) — copies all slots
    /// - If-else expression — handles each branch recursively
    /// - Block expression — lowers stmts then handles tail
    ///
    /// FLS §6.1.2:37–45: All stores are runtime instructions.
    /// FLS §15 AMBIGUOUS: The spec does not define a calling convention for
    /// enum-returning functions. Galvanic uses discriminant in x0, fields in x1..xN.
    fn lower_enum_expr_into(
        &mut self,
        expr: &Expr,
        base_slot: u8,
        max_fields: usize,
    ) -> Result<(), LowerError> {
        match &expr.kind {
            // FLS §6.12.1 + §15: Tuple variant constructor `Enum::Variant(f0, f1, ...)`.
            ExprKind::Call { callee, args }
                if matches!(&callee.kind, ExprKind::Path(segs) if segs.len() == 2) =>
            {
                let segs = if let ExprKind::Path(segs) = &callee.kind {
                    segs
                } else {
                    unreachable!()
                };
                let enum_name = segs[0].text(self.source);
                let variant_name = segs[1].text(self.source);
                if let Some((discriminant, _)) = self
                    .enum_defs
                    .get(enum_name)
                    .and_then(|v| v.get(variant_name))
                    .cloned()
                {
                    let disc_reg = self.alloc_reg()?;
                    self.instrs.push(Instr::LoadImm(disc_reg, discriminant));
                    self.instrs.push(Instr::Store { src: disc_reg, slot: base_slot });
                    for (i, arg) in args.iter().enumerate() {
                        let val = self.lower_expr(arg, &IrTy::I32)?;
                        let src = self.val_to_reg(val)?;
                        self.instrs.push(Instr::Store { src, slot: base_slot + 1 + i as u8 });
                    }
                    return Ok(());
                }
                Err(LowerError::Unsupported(
                    "enum return: unknown variant constructor".into(),
                ))
            }

            // FLS §15.3 + §6.11: Named-field enum variant `Enum::Variant { field: expr, ... }`.
            ExprKind::EnumVariantLit { path, fields: lit_fields } if path.len() == 2 => {
                let enum_name = path[0].text(self.source);
                let variant_name = path[1].text(self.source);
                if let Some((discriminant, field_names)) = self
                    .enum_defs
                    .get(enum_name)
                    .and_then(|v| v.get(variant_name))
                    .cloned()
                {
                    let disc_reg = self.alloc_reg()?;
                    self.instrs.push(Instr::LoadImm(disc_reg, discriminant));
                    self.instrs.push(Instr::Store { src: disc_reg, slot: base_slot });
                    // Store fields in declaration order.
                    for (field_idx, field_name) in field_names.iter().enumerate() {
                        let slot = base_slot + 1 + field_idx as u8;
                        let field_init = lit_fields
                            .iter()
                            .find(|(f, _)| f.text(self.source) == field_name.as_str())
                            .ok_or_else(|| {
                                LowerError::Unsupported(format!(
                                    "missing field `{field_name}` in `{enum_name}::{variant_name}` literal"
                                ))
                            })?;
                        let val = self.lower_expr(&field_init.1, &IrTy::I32)?;
                        let src = self.val_to_reg(val)?;
                        self.instrs.push(Instr::Store { src, slot });
                    }
                    return Ok(());
                }
                Err(LowerError::Unsupported(
                    "enum return: unknown named-field variant".into(),
                ))
            }

            // FLS §6.3 + §15: Unit variant path `Enum::Variant`.
            ExprKind::Path(segs) if segs.len() == 2 => {
                let enum_name = segs[0].text(self.source);
                let variant_name = segs[1].text(self.source);
                if let Some((discriminant, field_names)) = self
                    .enum_defs
                    .get(enum_name)
                    .and_then(|v| v.get(variant_name))
                    .cloned()
                    .filter(|(_, names)| names.is_empty())
                {
                    let _ = field_names;
                    let disc_reg = self.alloc_reg()?;
                    self.instrs.push(Instr::LoadImm(disc_reg, discriminant));
                    self.instrs.push(Instr::Store { src: disc_reg, slot: base_slot });
                    return Ok(());
                }
                Err(LowerError::Unsupported(
                    "enum return: non-unit variant used as path (needs call syntax)".into(),
                ))
            }

            // FLS §6.3 + §15: Enum variable path `x` (copy all slots).
            ExprKind::Path(segs) if segs.len() == 1 => {
                let var_name = segs[0].text(self.source);
                let src_base = *self.locals.get(var_name).ok_or_else(|| {
                    LowerError::Unsupported(format!("enum return: undefined variable `{var_name}`"))
                })?;
                if !self.local_enum_types.contains_key(&src_base) {
                    return Err(LowerError::Unsupported(format!(
                        "enum return: variable `{var_name}` is not an enum"
                    )));
                }
                // Copy discriminant.
                let disc_reg = self.alloc_reg()?;
                self.instrs.push(Instr::Load { dst: disc_reg, slot: src_base });
                self.instrs.push(Instr::Store { src: disc_reg, slot: base_slot });
                // Copy fields.
                for fi in 0..max_fields {
                    let fr = self.alloc_reg()?;
                    self.instrs.push(Instr::Load { dst: fr, slot: src_base + 1 + fi as u8 });
                    self.instrs.push(Instr::Store { src: fr, slot: base_slot + 1 + fi as u8 });
                }
                Ok(())
            }

            // FLS §6.17: If-else expression — handle each branch recursively.
            ExprKind::If { cond, then_block, else_expr } => {
                let else_label = self.alloc_label();
                let end_label = self.alloc_label();

                let cond_val = self.lower_expr(cond, &IrTy::Bool)?;
                let cond_reg = self.val_to_reg(cond_val)?;
                self.instrs.push(Instr::CondBranch { reg: cond_reg, label: else_label });

                // Then branch.
                for stmt in &then_block.stmts {
                    self.lower_stmt(stmt)?;
                }
                if let Some(tail) = then_block.tail.as_deref() {
                    self.lower_enum_expr_into(tail, base_slot, max_fields)?;
                }
                self.instrs.push(Instr::Branch(end_label));

                // Else branch.
                self.instrs.push(Instr::Label(else_label));
                match else_expr {
                    Some(e) => self.lower_enum_expr_into(e, base_slot, max_fields)?,
                    None => {
                        return Err(LowerError::Unsupported(
                            "enum-returning if expression must have an else branch".into(),
                        ))
                    }
                }

                self.instrs.push(Instr::Label(end_label));
                Ok(())
            }

            // FLS §6.4: Block expression — lower stmts then handle tail.
            ExprKind::Block(block) => {
                for stmt in &block.stmts {
                    self.lower_stmt(stmt)?;
                }
                match block.tail.as_deref() {
                    Some(tail) => self.lower_enum_expr_into(tail, base_slot, max_fields),
                    None => Err(LowerError::Unsupported(
                        "enum-returning block must have a tail expression".into(),
                    )),
                }
            }

            _ => Err(LowerError::Unsupported(
                "enum return: unsupported expression form".into(),
            )),
        }
    }

    // ── Statement lowering ────────────────────────────────────────────────────

    /// Lower one statement to runtime IR instructions.
    ///
    /// FLS §8: Statements.
    fn lower_stmt(&mut self, stmt: &crate::ast::Stmt) -> Result<(), LowerError> {
        match &stmt.kind {
            // FLS §8.1: Let statement — allocate a stack slot and optionally
            // store the initializer value. The variable name is registered in
            // `locals` so that later path expressions can emit a Load.
            //
            // FLS §8.1: "A LetStatement may optionally have an Initializer."
            // When no initializer is present the slot is allocated but left
            // uninitialized (no Store is emitted). A later plain-assignment
            // expression statement (`x = expr;`) will store to this slot via
            // the `BinOp::Assign` path in `lower_expr`.
            //
            // FLS §8.1 NOTE: The spec requires definite initialization before
            // use (via flow analysis). Galvanic does not yet enforce this —
            // reading an uninitialized slot produces architecturally undefined
            // behavior. Programs that always initialize before use are correct.
            //
            // FLS §6.1.2:37–45: Any store instruction is a runtime instruction;
            // the initializer (when present) is evaluated at runtime.
            StmtKind::Let { pat, ty: _, init } => {
                // FLS §5.10.3: Tuple pattern in let position — `let (a, b) = t;`.
                //
                // Handled before the scalar special-case chain since it
                // doesn't produce a single `var_name`. Supports two init forms:
                //
                // 1. Tuple literal RHS: `let (a, b) = (e0, e1);` — evaluate
                //    each element, store to fresh slots, bind names.
                // 2. Variable RHS:      `let (a, b) = pair;` — `pair` must be
                //    a known tuple variable; bind names to its existing slots.
                //
                // Sub-patterns: only `Pat::Ident` (binding) and `Pat::Wildcard`
                // (discard) are supported at this milestone (FLS §5.10.3).
                //
                // FLS §6.1.2:37–45: All stores are runtime instructions.
                // Cache-line note: literal init costs N stores (4N bytes);
                // variable rebind costs 0 instructions (slot alias only).
                if let Pat::Tuple(pats) = pat {
                    // Case 1: init is a tuple literal `(e0, e1, ...)`.
                    if let Some(init_expr) = init.as_ref()
                        && let ExprKind::Tuple(elems) = &init_expr.kind
                    {
                        if elems.len() != pats.len() {
                            return Err(LowerError::Unsupported(format!(
                                "tuple pattern has {} elements but initializer has {}",
                                pats.len(), elems.len()
                            )));
                        }
                        let n = pats.len();
                        let base_slot = self.alloc_slot()?;
                        for _ in 1..n {
                            self.alloc_slot()?;
                        }
                        // Evaluate and store each element left-to-right (FLS §6.4:14).
                        for (i, elem_expr) in elems.iter().enumerate() {
                            let val = self.lower_expr(elem_expr, &IrTy::I32)?;
                            let src = self.val_to_reg(val)?;
                            self.instrs.push(Instr::Store {
                                src,
                                slot: base_slot + i as u8,
                            });
                        }
                        // Bind each sub-pattern to its slot.
                        for (i, sub_pat) in pats.iter().enumerate() {
                            match sub_pat {
                                Pat::Ident(span) => {
                                    let name = span.text(self.source);
                                    self.locals.insert(name, base_slot + i as u8);
                                }
                                Pat::Wildcard => {} // no binding
                                _ => {
                                    return Err(LowerError::Unsupported(
                                        "nested tuple pattern not yet supported".into(),
                                    ));
                                }
                            }
                        }
                        // Register as a tuple so element re-access works later.
                        if n > 0 {
                            self.local_tuple_lens.insert(base_slot, n);
                        }
                        return Ok(());
                    }

                    // Case 2: init is a simple variable path whose value is
                    // a known tuple (registered in `local_tuple_lens`).
                    //
                    // This is a zero-instruction rebind: we simply register each
                    // sub-pattern name against the source variable's slots.
                    if let Some(init_expr) = init.as_ref()
                        && let ExprKind::Path(segs) = &init_expr.kind
                        && segs.len() == 1
                    {
                        let src_name = segs[0].text(self.source);
                        let src_slot = *self.locals.get(src_name).ok_or_else(|| {
                            LowerError::Unsupported(format!(
                                "undefined variable `{src_name}` in tuple destructure"
                            ))
                        })?;
                        let src_len = *self.local_tuple_lens.get(&src_slot).ok_or_else(|| {
                            LowerError::Unsupported(format!(
                                "variable `{src_name}` is not a tuple"
                            ))
                        })?;
                        if src_len != pats.len() {
                            return Err(LowerError::Unsupported(format!(
                                "tuple pattern has {} elements but `{src_name}` has {src_len}",
                                pats.len()
                            )));
                        }
                        for (i, sub_pat) in pats.iter().enumerate() {
                            match sub_pat {
                                Pat::Ident(span) => {
                                    let name = span.text(self.source);
                                    self.locals.insert(name, src_slot + i as u8);
                                }
                                Pat::Wildcard => {}
                                _ => {
                                    return Err(LowerError::Unsupported(
                                        "nested tuple pattern not yet supported".into(),
                                    ));
                                }
                            }
                        }
                        return Ok(());
                    }

                    return Err(LowerError::Unsupported(
                        "tuple destructuring requires a tuple literal or simple variable initializer".into(),
                    ));
                }

                // FLS §5.10.2 + §8.1: Struct pattern in let binding.
                //
                // `let StructName { field1, field2, .. } = expr;` binds each named
                // field pattern to the corresponding stack slot of the struct value.
                //
                // Supported initializer forms:
                //   1. Variable path — `let Point { x, y } = p;` — slot aliasing,
                //      zero runtime instructions.
                //   2. Struct literal — `let Point { x, y } = Point { x: 3, y: 4 };` —
                //      evaluate each field and store to fresh slots, then alias names.
                //
                // FLS §6.1.2:37–45: All stores are runtime instructions; no
                // compile-time evaluation of non-const code.
                // Cache-line note: N-field struct occupies N consecutive 8-byte
                // slots. Slot aliasing costs 0 instructions; literal init costs
                // N store instructions.
                if let Pat::StructVariant { path: pat_path, fields: pat_fields } = pat
                    && pat_path.len() == 1
                {
                    let struct_name = pat_path[0].text(self.source);

                    // Case 1: Init is a simple variable path.
                    if let Some(init_expr) = init.as_ref()
                        && let ExprKind::Path(segs) = &init_expr.kind
                        && segs.len() == 1
                    {
                        let src_name = segs[0].text(self.source);
                        let src_slot =
                            *self.locals.get(src_name).ok_or_else(|| {
                                LowerError::Unsupported(format!(
                                    "undefined variable `{src_name}` in struct destructure"
                                ))
                            })?;
                        let src_type = self
                            .local_struct_types
                            .get(&src_slot)
                            .cloned()
                            .ok_or_else(|| {
                                LowerError::Unsupported(format!(
                                    "variable `{src_name}` is not a struct (cannot destructure)"
                                ))
                            })?;
                        if src_type != struct_name {
                            return Err(LowerError::Unsupported(format!(
                                "struct pattern `{struct_name}` does not match \
                                 variable type `{src_type}`"
                            )));
                        }
                        let field_names = self
                            .struct_defs
                            .get(struct_name)
                            .cloned()
                            .ok_or_else(|| {
                                LowerError::Unsupported(format!(
                                    "unknown struct type `{struct_name}`"
                                ))
                            })?;
                        let offsets = self
                            .struct_field_offsets
                            .get(struct_name)
                            .cloned()
                            .ok_or_else(|| {
                                LowerError::Unsupported(format!(
                                    "unknown field offsets for struct `{struct_name}`"
                                ))
                            })?;
                        // Alias each named field pattern to its source slot.
                        for (field_name_span, sub_pat) in pat_fields.iter() {
                            let fname = field_name_span.text(self.source);
                            let fi = field_names
                                .iter()
                                .position(|f| f == fname)
                                .ok_or_else(|| {
                                    LowerError::Unsupported(format!(
                                        "no field `{fname}` on struct `{struct_name}`"
                                    ))
                                })?;
                            let slot = src_slot + offsets[fi] as u8;
                            match sub_pat {
                                Pat::Ident(bind_span) => {
                                    let bind_name = bind_span.text(self.source);
                                    self.locals.insert(bind_name, slot);
                                }
                                Pat::Wildcard => {}
                                _ => {
                                    return Err(LowerError::Unsupported(
                                        "only ident/wildcard sub-patterns in struct \
                                         let-pattern"
                                            .into(),
                                    ));
                                }
                            }
                        }
                        return Ok(());
                    }

                    // Case 2: Init is an inline struct literal.
                    if let Some(init_expr) = init.as_ref()
                        && let ExprKind::StructLit {
                            name: lit_name_span,
                            fields: lit_fields,
                            base: lit_base,
                        } = &init_expr.kind
                    {
                        let lit_struct_name = lit_name_span.text(self.source);
                        if lit_struct_name != struct_name {
                            return Err(LowerError::Unsupported(format!(
                                "struct pattern `{struct_name}` does not match \
                                 literal type `{lit_struct_name}`"
                            )));
                        }
                        if lit_base.is_some() {
                            return Err(LowerError::Unsupported(
                                "struct update syntax not supported in struct \
                                 let-pattern"
                                    .into(),
                            ));
                        }
                        let field_names = self
                            .struct_defs
                            .get(struct_name)
                            .cloned()
                            .ok_or_else(|| {
                                LowerError::Unsupported(format!(
                                    "unknown struct type `{struct_name}`"
                                ))
                            })?;
                        let offsets = self
                            .struct_field_offsets
                            .get(struct_name)
                            .cloned()
                            .ok_or_else(|| {
                                LowerError::Unsupported(format!(
                                    "unknown field offsets for struct `{struct_name}`"
                                ))
                            })?;
                        let n_slots = self
                            .struct_sizes
                            .get(struct_name)
                            .copied()
                            .unwrap_or(field_names.len());
                        let field_types = self
                            .struct_field_types
                            .get(struct_name)
                            .cloned()
                            .unwrap_or_default();

                        // Allocate consecutive slots for the temporary struct.
                        let base_slot = self.alloc_slot()?;
                        for _ in 1..n_slots {
                            self.alloc_slot()?;
                        }
                        self.local_struct_types
                            .insert(base_slot, struct_name.to_owned());

                        // Evaluate and store each field in declaration order
                        // (FLS §6.4:14 — left-to-right evaluation order).
                        for (fi, fname) in field_names.iter().enumerate() {
                            let slot = base_slot + offsets[fi] as u8;
                            let field_init = lit_fields
                                .iter()
                                .find(|(fs, _)| fs.text(self.source) == fname.as_str())
                                .ok_or_else(|| {
                                    LowerError::Unsupported(format!(
                                        "missing field `{fname}` in struct literal"
                                    ))
                                })?;
                            let inner_type =
                                field_types.get(fi).and_then(|t| t.clone());
                            if let Some(ref itype) = inner_type
                                && self.struct_defs.contains_key(itype.as_str())
                            {
                                self.store_nested_struct_lit(
                                    &field_init.1,
                                    slot,
                                    itype,
                                )?;
                            } else {
                                let val =
                                    self.lower_expr(&field_init.1, &IrTy::I32)?;
                                let src = self.val_to_reg(val)?;
                                self.instrs.push(Instr::Store { src, slot });
                            }
                        }

                        // Bind each field pattern to its allocated slot.
                        for (field_name_span, sub_pat) in pat_fields.iter() {
                            let fname = field_name_span.text(self.source);
                            let fi = field_names
                                .iter()
                                .position(|f| f == fname)
                                .ok_or_else(|| {
                                    LowerError::Unsupported(format!(
                                        "no field `{fname}` on struct `{struct_name}`"
                                    ))
                                })?;
                            let slot = base_slot + offsets[fi] as u8;
                            match sub_pat {
                                Pat::Ident(bind_span) => {
                                    let bind_name = bind_span.text(self.source);
                                    self.locals.insert(bind_name, slot);
                                }
                                Pat::Wildcard => {}
                                _ => {
                                    return Err(LowerError::Unsupported(
                                        "only ident/wildcard sub-patterns in struct \
                                         let-pattern"
                                            .into(),
                                    ));
                                }
                            }
                        }
                        return Ok(());
                    }

                    return Err(LowerError::Unsupported(
                        "struct let-pattern requires a struct variable or struct \
                         literal initializer"
                            .into(),
                    ));
                }

                // All other patterns: extract `var_name` for the scalar path.
                //
                // FLS §5.11: Wildcard pattern `_` — evaluate init for side
                // effects but bind no name.
                let var_name = match pat {
                    Pat::Ident(span) => span.text(self.source),
                    Pat::Wildcard => {
                        if let Some(init_expr) = init.as_ref() {
                            self.lower_expr(init_expr, &IrTy::I32)?;
                        }
                        return Ok(());
                    }
                    _ => {
                        return Err(LowerError::Unsupported(
                            "complex let pattern not yet supported".into(),
                        ));
                    }
                };

                // FLS §10.1: Struct-returning associated function call as let-binding init.
                //
                // `let p = Point::new(x, y)` where `Point__new` is in `struct_return_fns`.
                // The callee returns field values in x0..x{N-1} via RetFields.
                // We allocate N consecutive slots for the variable and emit CallMut
                // to write x0..x{N-1} into those slots after the bl.
                //
                // This is the same write-back mechanism used by &mut self methods,
                // reused here for constructor-style associated functions.
                //
                // FLS §6.12.1: Call expressions. FLS §10.1: Associated functions.
                // Cache-line note: arg moves + bl + N stores per construction.
                if let Some(init_expr) = init.as_ref()
                    && let ExprKind::Call { callee, args } = &init_expr.kind
                    && let ExprKind::Path(segs) = &callee.kind
                    && segs.len() == 2
                {
                    let type_name = segs[0].text(self.source);
                    let fn_name_seg = segs[1].text(self.source);
                    let mangled = format!("{type_name}__{fn_name_seg}");
                    if let Some(struct_name) = self.struct_return_fns.get(&mangled).cloned() {
                        let field_names = self.struct_defs
                            .get(&struct_name)
                            .ok_or_else(|| {
                                LowerError::Unsupported(format!("unknown struct `{struct_name}`"))
                            })?
                            .clone();
                        let n_fields = field_names.len();

                        // Allocate N consecutive slots for the new struct variable.
                        let base_slot = self.alloc_slot()?;
                        for _ in 1..n_fields {
                            self.alloc_slot()?;
                        }
                        self.locals.insert(var_name, base_slot);
                        self.local_struct_types.insert(base_slot, struct_name.clone());

                        // Evaluate arguments and collect their virtual registers.
                        let mut arg_regs: Vec<u8> = Vec::new();
                        for arg_expr in args.iter() {
                            let val = self.lower_expr(arg_expr, &IrTy::I32)?;
                            let reg = self.val_to_reg(val)?;
                            arg_regs.push(reg);
                        }

                        self.has_calls = true;
                        // Emit CallMut: args → bl → store x0..x{N-1} to base_slot..
                        // The callee's RetFields has already put field values in x0..x{N-1}.
                        self.instrs.push(Instr::CallMut {
                            name: mangled,
                            args: arg_regs,
                            write_back_slot: base_slot,
                            n_fields: n_fields as u8,
                        });
                        return Ok(());
                    }
                }

                // FLS §9: Struct-returning free function call as let-binding init.
                //
                // `let p = make_point(a, b)` where `make_point` is in
                // `struct_return_free_fns`. The callee returns field values in
                // x0..x{N-1} via RetFields. We allocate N consecutive slots for
                // the variable and emit CallMut to write x0..x{N-1} into those
                // slots after the bl.
                //
                // This uses the same write-back mechanism as struct-returning
                // associated functions (`struct_return_fns`), extended here to
                // free functions.
                //
                // FLS §6.12.1: Call expressions. FLS §9: Functions.
                // Cache-line note: arg moves + bl + N stores per construction.
                if let Some(init_expr) = init.as_ref()
                    && let ExprKind::Call { callee, args } = &init_expr.kind
                    && let ExprKind::Path(segs) = &callee.kind
                    && segs.len() == 1
                {
                    let fn_name = segs[0].text(self.source);
                    if let Some(struct_name) = self.struct_return_free_fns.get(fn_name).cloned() {
                        let field_names = self.struct_defs
                            .get(&struct_name)
                            .ok_or_else(|| {
                                LowerError::Unsupported(format!(
                                    "unknown struct `{struct_name}` from free fn `{fn_name}`"
                                ))
                            })?
                            .clone();
                        let n_fields = field_names.len();

                        // Allocate N consecutive slots for the new struct variable.
                        let base_slot = self.alloc_slot()?;
                        for _ in 1..n_fields {
                            self.alloc_slot()?;
                        }
                        self.locals.insert(var_name, base_slot);
                        self.local_struct_types.insert(base_slot, struct_name.clone());

                        // Evaluate arguments and collect their virtual registers.
                        let mut arg_regs: Vec<u8> = Vec::new();
                        for arg_expr in args.iter() {
                            let val = self.lower_expr(arg_expr, &IrTy::I32)?;
                            let reg = self.val_to_reg(val)?;
                            arg_regs.push(reg);
                        }

                        self.has_calls = true;
                        // Emit CallMut: args → bl → store x0..x{N-1} to base_slot..
                        // The callee's RetFields has already put field values in
                        // x0..x{N-1}.
                        self.instrs.push(Instr::CallMut {
                            name: fn_name.to_owned(),
                            args: arg_regs,
                            write_back_slot: base_slot,
                            n_fields: n_fields as u8,
                        });
                        return Ok(());
                    }
                }

                // FLS §9, §15: Enum-returning free function call as let-binding init.
                //
                // `let x = wrap(n)` where `wrap` is in `enum_return_fns`.
                // The callee returns discriminant + fields in x0..x{1+max_fields-1}
                // via RetFields. We allocate 1+max_fields consecutive slots for the
                // variable and emit CallMut to write them after the bl.
                //
                // FLS §6.12.1: Call expressions. FLS §15: Enum values occupy
                // 1+max_fields consecutive slots.
                // Cache-line note: arg moves + bl + (1+max_fields) stores.
                if let Some(init_expr) = init.as_ref()
                    && let ExprKind::Call { callee, args } = &init_expr.kind
                    && let ExprKind::Path(segs) = &callee.kind
                    && segs.len() == 1
                {
                    let fn_name = segs[0].text(self.source);
                    if let Some(enum_name) = self.enum_return_fns.get(fn_name).cloned() {
                        let max_fields = self.enum_defs
                            .get(enum_name.as_str())
                            .map(|v| v.values().map(|(_, names)| names.len()).max().unwrap_or(0))
                            .unwrap_or(0);
                        let n_ret = 1 + max_fields as u8;
                        // Allocate discriminant slot + field slots.
                        let base_slot = self.alloc_slot()?;
                        for _ in 0..max_fields {
                            self.alloc_slot()?;
                        }
                        self.locals.insert(var_name, base_slot);
                        self.local_enum_types.insert(base_slot, enum_name.clone());
                        // Evaluate arguments.
                        let mut arg_regs: Vec<u8> = Vec::new();
                        for arg_expr in args.iter() {
                            let val = self.lower_expr(arg_expr, &IrTy::I32)?;
                            let reg = self.val_to_reg(val)?;
                            arg_regs.push(reg);
                        }
                        self.has_calls = true;
                        self.instrs.push(Instr::CallMut {
                            name: fn_name.to_owned(),
                            args: arg_regs,
                            write_back_slot: base_slot,
                            n_fields: n_ret,
                        });
                        return Ok(());
                    }
                }

                // FLS §10.1, §6.12.2: `&self` method call returning a struct.
                //
                // `let q = p.translate(dx, dy)` where `Point__translate` is in
                // `struct_return_methods`. The callee returns the new struct's
                // field values in x0..x{N-1} via `RetFields`. We:
                //   1. Load receiver (p) fields into leading arg registers.
                //   2. Lower explicit arguments.
                //   3. Allocate N consecutive slots for the destination variable (q).
                //   4. Emit `CallMut` to write x0..x{N-1} into q's slots after `bl`.
                //
                // The receiver struct's fields are passed by value (copied into
                // registers); the receiver itself is not modified (read-only self).
                //
                // FLS §10.1 AMBIGUOUS: The spec does not define the calling
                // convention for `&self` methods returning struct types. Galvanic
                // uses the same register-packing convention as struct-returning
                // associated functions: fields returned in x0..x{N-1}.
                //
                // FLS §6.1.2:37–45: All instructions are runtime (no const folding).
                // Cache-line note: N self-field loads + arg moves + bl + N return
                // stores = 2N+1 extra instructions compared to a scalar method call.
                if let Some(init_expr) = init.as_ref()
                    && let ExprKind::MethodCall { receiver, method, args } = &init_expr.kind
                    && let ExprKind::Path(recv_segs) = &receiver.kind
                    && recv_segs.len() == 1
                {
                    let recv_name = recv_segs[0].text(self.source);
                    let method_name = method.text(self.source);
                    if let Some(recv_base_slot) = self.locals.get(recv_name).copied()
                        && let Some(recv_type) = self.local_struct_types.get(&recv_base_slot).cloned() {
                            let mangled = format!("{recv_type}__{method_name}");
                            if let Some(ret_struct_name) = self.struct_return_methods.get(&mangled).cloned() {
                                let field_names = self.struct_defs
                                    .get(ret_struct_name.as_str())
                                    .ok_or_else(|| {
                                        LowerError::Unsupported(format!(
                                            "unknown return struct `{ret_struct_name}`"
                                        ))
                                    })?
                                    .clone();
                                let n_return_fields = field_names.len();

                                // Allocate slots for destination variable (q).
                                let base_slot = self.alloc_slot()?;
                                for _ in 1..n_return_fields {
                                    self.alloc_slot()?;
                                }
                                self.locals.insert(var_name, base_slot);
                                self.local_struct_types.insert(base_slot, ret_struct_name.clone());

                                // Load receiver fields into leading arg registers.
                                let recv_field_names = self.struct_defs
                                    .get(recv_type.as_str())
                                    .ok_or_else(|| {
                                        LowerError::Unsupported(format!(
                                            "unknown receiver struct `{recv_type}`"
                                        ))
                                    })?
                                    .clone();
                                let n_recv_fields = recv_field_names.len();
                                let mut arg_regs: Vec<u8> = Vec::new();
                                for fi in 0..n_recv_fields {
                                    let slot = recv_base_slot + fi as u8;
                                    let reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load { dst: reg, slot });
                                    arg_regs.push(reg);
                                }

                                // Lower explicit arguments.
                                for arg_expr in args.iter() {
                                    let val = self.lower_expr(arg_expr, &IrTy::I32)?;
                                    let reg = self.val_to_reg(val)?;
                                    arg_regs.push(reg);
                                }

                                self.has_calls = true;
                                // Emit CallMut: args → bl → store x0..x{N-1} to base_slot..
                                // The callee's RetFields returns new struct fields in x0..x{N-1}.
                                self.instrs.push(Instr::CallMut {
                                    name: mangled,
                                    args: arg_regs,
                                    write_back_slot: base_slot,
                                    n_fields: n_return_fields as u8,
                                });
                                return Ok(());
                            }
                        }
                }

                // FLS §6.11: Struct literal initializer — allocate one slot per
                // scalar field (or N slots for a struct-type field of total size N)
                // and store each field value.
                //
                // FLS §6.13, §4.11: Nested struct fields occupy multiple consecutive
                // slots. The total slot count for the outer struct is `struct_sizes`
                // entry for the struct type; field slot offsets come from
                // `struct_field_offsets`.
                //
                // Cache-line note: an N-slot struct occupies N consecutive 8-byte
                // slots. The base slot is recorded so that chained field access
                // can compute `base_slot + field_offset`.
                if let Some(init_expr) = init.as_ref()
                    && let ExprKind::StructLit {
                        name: struct_name_span,
                        fields: init_fields,
                        base: init_base,
                    } = &init_expr.kind
                {
                        let struct_name = struct_name_span.text(self.source);
                        // Clone to avoid borrow conflict with self inside the loop.
                        let field_names: Vec<String> = self
                            .struct_defs
                            .get(struct_name)
                            .ok_or_else(|| {
                                LowerError::Unsupported(format!(
                                    "unknown struct type `{struct_name}`"
                                ))
                            })?
                            .clone();

                        // FLS §4.11: Allocate the total number of slots for this struct,
                        // which may be greater than field_names.len() when any field is
                        // itself a struct type (each such field occupies multiple slots).
                        let total_slots = self
                            .struct_sizes
                            .get(struct_name)
                            .copied()
                            .unwrap_or(field_names.len());
                        let base_slot = self.alloc_slot()?;
                        for _ in 1..total_slots {
                            self.alloc_slot()?; // consume additional slots for nested fields
                        }

                        self.locals.insert(var_name, base_slot);
                        self.local_struct_types
                            .insert(base_slot, struct_name.to_owned());

                        // FLS §6.11: Struct update syntax `Struct { field: val, ..other }`.
                        // Resolve the base struct's first slot so we can copy unspecified
                        // fields from it. The base must be a simple variable path.
                        //
                        // FLS §6.11: "A struct expression with a base struct specifies one
                        // or more fields for the new value and copies all remaining fields
                        // from the base struct expression."
                        let base_struct_slot: Option<u8> = if let Some(base_expr) = init_base {
                            if let ExprKind::Path(segs) = &base_expr.kind
                                && segs.len() == 1
                            {
                                let base_var = segs[0].text(self.source);
                                Some(*self.locals.get(base_var).ok_or_else(|| {
                                    LowerError::Unsupported(format!(
                                        "unknown variable `{base_var}` in struct update syntax"
                                    ))
                                })?)
                            } else {
                                return Err(LowerError::Unsupported(
                                    "struct update base must be a simple variable path".into(),
                                ));
                            }
                        } else {
                            None
                        };

                        // Pre-fetch field offset and type tables to avoid repeated map
                        // lookups inside the loop. Both are keyed by struct name.
                        let field_offsets: Vec<usize> = self
                            .struct_field_offsets
                            .get(struct_name)
                            .cloned()
                            .unwrap_or_default();
                        let field_types: Vec<Option<String>> = self
                            .struct_field_types
                            .get(struct_name)
                            .cloned()
                            .unwrap_or_default();

                        // Store each field in declaration order.
                        // FLS §6.11: Field initializers are evaluated in source
                        // order but stored in declaration order for layout stability.
                        // FLS §6.11: Fields not explicitly listed are copied from base.
                        for (field_idx, field_name) in field_names.iter().enumerate() {
                            // FLS §4.11: field slot = base + offset (not base + index)
                            // because preceding struct-type fields may span multiple slots.
                            let field_offset = field_offsets
                                .get(field_idx)
                                .copied()
                                .unwrap_or(field_idx);
                            let dst_slot = base_slot + field_offset as u8;

                            if let Some(field_init) = init_fields
                                .iter()
                                .find(|(f, _)| f.text(self.source) == field_name.as_str())
                            {
                                // FLS §6.11: Explicitly provided field — evaluate and store.
                                // If the field is itself a struct type, store recursively
                                // into `total_size` consecutive slots starting at `dst_slot`.
                                let nested_ty = field_types
                                    .get(field_idx)
                                    .cloned()
                                    .flatten();
                                if let Some(nested_type_name) = nested_ty {
                                    // Nested struct field: recursively store the struct literal
                                    // into the target slots. Only struct literals are supported
                                    // as nested struct field values at this milestone.
                                    self.store_nested_struct_lit(
                                        &field_init.1,
                                        dst_slot,
                                        &nested_type_name,
                                    )?;
                                } else {
                                    let val = self.lower_expr(&field_init.1, &IrTy::I32)?;
                                    let src = self.val_to_reg(val)?;
                                    self.instrs.push(Instr::Store { src, slot: dst_slot });
                                }
                            } else if let Some(base_first_slot) = base_struct_slot {
                                // FLS §6.11: Copy unspecified field from the base struct.
                                // Load from `base_first_slot + field_offset`, store to `dst_slot`.
                                // Cache-line note: load+store = two 4-byte instructions = 8 bytes.
                                let src_slot = base_first_slot + field_offset as u8;
                                let tmp = self.alloc_reg()?;
                                self.instrs.push(Instr::Load { dst: tmp, slot: src_slot });
                                self.instrs.push(Instr::Store { src: tmp, slot: dst_slot });
                            } else {
                                return Err(LowerError::Unsupported(format!(
                                    "missing field `{field_name}` in `{struct_name}` literal"
                                )));
                            }
                        }
                        return Ok(());
                }

                // FLS §15.3 + §6.11: Named-field enum variant construction.
                //
                // `let x = Color::Rgb { r: 255, g: 0, b: 0 }` — an `EnumVariantLit`
                // expression. Layout: slot 0 = discriminant, slots 1..N = fields in
                // declaration order (matching `EnumVariantInfo.1`).
                //
                // Fields in the literal may appear in any source order; they are
                // stored in declaration order for stable slot indexing by patterns.
                //
                // FLS §6.1.2:37–45: All stores are runtime instructions.
                // Cache-line note: N+1 consecutive slots; same layout as tuple variants.
                if let Some(init_expr) = init.as_ref()
                    && let ExprKind::EnumVariantLit { path, fields: lit_fields } = &init_expr.kind
                    && path.len() == 2
                {
                    let enum_name = path[0].text(self.source);
                    let variant_name = path[1].text(self.source);
                    if let Some((discriminant, field_names)) = self.enum_defs
                        .get(enum_name)
                        .and_then(|v| v.get(variant_name))
                        .cloned()
                    {
                        let field_count = field_names.len();
                        if lit_fields.len() != field_count {
                            return Err(LowerError::Unsupported(format!(
                                "enum variant `{enum_name}::{variant_name}` expects {field_count} named fields, got {}",
                                lit_fields.len()
                            )));
                        }
                        // Allocate discriminant slot + one slot per field.
                        let base_slot = self.alloc_slot()?;
                        for _ in 0..field_count {
                            self.alloc_slot()?;
                        }
                        self.locals.insert(var_name, base_slot);
                        self.local_enum_types.insert(base_slot, enum_name.to_owned());
                        // Store discriminant.
                        let disc_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(disc_reg, discriminant));
                        self.instrs.push(Instr::Store { src: disc_reg, slot: base_slot });
                        // Store each field in declaration order.
                        // FLS §6.11: Fields evaluated in source order but stored in
                        // declaration order for layout stability.
                        for (field_idx, field_name) in field_names.iter().enumerate() {
                            let slot = base_slot + 1 + field_idx as u8;
                            let field_init = lit_fields
                                .iter()
                                .find(|(f, _)| f.text(self.source) == field_name.as_str())
                                .ok_or_else(|| {
                                    LowerError::Unsupported(format!(
                                        "missing field `{field_name}` in `{enum_name}::{variant_name}` literal"
                                    ))
                                })?;
                            let val = self.lower_expr(&field_init.1, &IrTy::I32)?;
                            let src = self.val_to_reg(val)?;
                            self.instrs.push(Instr::Store { src, slot });
                        }
                        return Ok(());
                    }
                }

                // FLS §15: Enum tuple variant construction — `let x = Enum::Variant(f0, f1, ...)`.
                //
                // A two-segment path callee in a call expression is an enum tuple variant
                // constructor. Layout: slot 0 = discriminant, slots 1..N = fields.
                // The variable's slot maps to the discriminant slot; `local_enum_types`
                // records the enum type so TupleStruct patterns can locate field slots.
                //
                // FLS §6.1.2:37–45: Construction stores at runtime; no const folding.
                //
                // Cache-line note: a variant with N fields occupies N+1 consecutive slots.
                if let Some(init_expr) = init.as_ref()
                    && let ExprKind::Call { callee, args } = &init_expr.kind
                    && let ExprKind::Path(segs) = &callee.kind
                    && segs.len() == 2
                {
                    let enum_name = segs[0].text(self.source);
                    let variant_name = segs[1].text(self.source);
                    if let Some((discriminant, field_names)) = self.enum_defs
                        .get(enum_name)
                        .and_then(|v| v.get(variant_name))
                        .cloned()
                    {
                        let field_count = field_names.len();
                        if args.len() != field_count {
                            return Err(LowerError::Unsupported(format!(
                                "enum variant `{enum_name}::{variant_name}` expects {field_count} fields, got {}",
                                args.len()
                            )));
                        }
                        // Allocate discriminant slot + one slot per field.
                        let base_slot = self.alloc_slot()?;
                        for _ in 0..field_count {
                            self.alloc_slot()?;
                        }
                        self.locals.insert(var_name, base_slot);
                        self.local_enum_types
                            .insert(base_slot, enum_name.to_owned());
                        // Store discriminant.
                        let disc_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(disc_reg, discriminant));
                        self.instrs.push(Instr::Store { src: disc_reg, slot: base_slot });
                        // Store each field in source order.
                        for (i, arg) in args.iter().enumerate() {
                            let val = self.lower_expr(arg, &IrTy::I32)?;
                            let src = self.val_to_reg(val)?;
                            self.instrs.push(Instr::Store { src, slot: base_slot + 1 + i as u8 });
                        }
                        return Ok(());
                    }
                }

                // FLS §15: Enum unit variant let binding — `let x = Opt::None;`
                //
                // Unit variants are path expressions (not call expressions), so they
                // are not caught by the call-based enum check above. Detect them here
                // and register the variable in `local_enum_types` so TupleStruct
                // patterns can locate the discriminant slot during match lowering.
                //
                // Allocate enough field slots to accommodate the enum's max field
                // count so that `base_slot + 1 + fi` is always a valid slot index
                // even for unit variant values (fields are uninitialized but never
                // accessed due to discriminant check in the pattern).
                //
                // FLS §6.1.2:37–45: Discriminant store is a runtime instruction.
                if let Some(init_expr) = init.as_ref()
                    && let ExprKind::Path(segs) = &init_expr.kind
                    && segs.len() == 2
                {
                    let enum_name = segs[0].text(self.source);
                    let variant_name = segs[1].text(self.source);
                    if let Some((discriminant, field_names)) = self.enum_defs
                        .get(enum_name)
                        .and_then(|v| v.get(variant_name))
                        .cloned()
                        .filter(|(_, names)| names.is_empty())
                    {
                        // Compute max field count across all variants of this enum.
                        let max_fields = self.enum_defs
                            .get(enum_name)
                            .map(|v| v.values().map(|(_, names)| names.len()).max().unwrap_or(0))
                            .unwrap_or(0);
                        let _ = field_names; // unit variant has no fields
                        let base_slot = self.alloc_slot()?;
                        for _ in 0..max_fields {
                            self.alloc_slot()?; // Reserve field slots (uninitialized for unit variant).
                        }
                        self.locals.insert(var_name, base_slot);
                        self.local_enum_types.insert(base_slot, enum_name.to_owned());
                        let disc_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(disc_reg, discriminant));
                        self.instrs.push(Instr::Store { src: disc_reg, slot: base_slot });
                        return Ok(());
                    }
                }

                // FLS §6.8: Array literal — `let a = [e0, e1, e2];`
                //
                // An array of N elements is laid out as N consecutive 8-byte
                // stack slots. The variable name maps to the base slot (slot of
                // element 0). Elements are evaluated and stored left-to-right
                // (FLS §6.4:14).
                //
                // `local_array_lens` records the element count so that index
                // expressions can validate the base slot maps to an array.
                //
                // FLS §6.1.2:37–45: All stores are runtime instructions.
                // Cache-line note: N elements × 4-byte stores. An 8-element
                // array initializer emits 8 store instructions = 32 bytes,
                // filling half of a 64-byte cache line.
                if let Some(init_expr) = init.as_ref()
                    && let ExprKind::Array(elems) = &init_expr.kind
                {
                    let n = elems.len();
                    // Allocate N consecutive slots.
                    let base_slot = self.alloc_slot()?;
                    for _ in 1..n {
                        self.alloc_slot()?;
                    }
                    self.locals.insert(var_name, base_slot);
                    self.local_array_lens.insert(base_slot, n);
                    // Evaluate and store each element.
                    // FLS §6.8: Elements are evaluated left-to-right.
                    for (i, elem_expr) in elems.iter().enumerate() {
                        let val = self.lower_expr(elem_expr, &IrTy::I32)?;
                        let src = self.val_to_reg(val)?;
                        self.instrs.push(Instr::Store { src, slot: base_slot + i as u8 });
                    }
                    return Ok(());
                }

                // FLS §6.10: Tuple literal `let t = (e0, e1, ...)`.
                //
                // A tuple of N elements is laid out as N consecutive 8-byte
                // stack slots, identical to a struct or array. The variable
                // name maps to the base slot (slot of element 0). Elements
                // are evaluated and stored left-to-right (FLS §6.4:14).
                //
                // `local_tuple_lens` records the field count so that `.0`,
                // `.1` field accesses can compute `base_slot + index`.
                //
                // Cache-line note: same layout as arrays — N stores per init.
                if let Some(init_expr) = init.as_ref()
                    && let ExprKind::Tuple(elems) = &init_expr.kind
                {
                    let n = elems.len();
                    let base_slot = self.alloc_slot()?;
                    for _ in 1..n {
                        self.alloc_slot()?;
                    }
                    self.locals.insert(var_name, base_slot);
                    self.local_tuple_lens.insert(base_slot, n);
                    // FLS §6.10: Elements evaluated left-to-right.
                    for (i, elem_expr) in elems.iter().enumerate() {
                        let val = self.lower_expr(elem_expr, &IrTy::I32)?;
                        let src = self.val_to_reg(val)?;
                        self.instrs.push(Instr::Store { src, slot: base_slot + i as u8 });
                    }
                    return Ok(());
                }

                // FLS §14.2: Tuple struct constructor `let p = Point(a, b)`.
                //
                // A tuple struct constructor is syntactically a call expression
                // with a single-segment callee path that names a known tuple struct.
                // Lower it like a tuple literal: allocate N consecutive stack slots
                // and store each argument value in order.
                //
                // The base slot is registered in `local_tuple_lens` so that
                // subsequent `.0`, `.1` field access expressions (FLS §6.10) resolve
                // to `base_slot + index`, reusing the existing tuple field access path.
                //
                // FLS §6.1.2:37–45: All stores are runtime instructions — no const folding.
                // Cache-line note: N stores per construction, identical to a tuple literal.
                if let Some(init_expr) = init.as_ref()
                    && let ExprKind::Call { callee, args } = &init_expr.kind
                    && let ExprKind::Path(segs) = &callee.kind
                    && segs.len() == 1
                {
                    let ctor_name = segs[0].text(self.source);
                    if let Some(&n) = self.tuple_struct_defs.get(ctor_name) {
                        let base_slot = self.alloc_slot()?;
                        for _ in 1..n {
                            self.alloc_slot()?;
                        }
                        self.locals.insert(var_name, base_slot);
                        self.local_tuple_lens.insert(base_slot, n);
                        // FLS §14.2: Record the tuple struct type so method call dispatch
                        // can compute the mangled name `TypeName__method_name`.
                        self.local_tuple_struct_types.insert(base_slot, ctor_name.to_owned());
                        // FLS §6.4:14 / §6.10: Arguments evaluated left-to-right.
                        for (i, arg_expr) in args.iter().enumerate() {
                            let val = self.lower_expr(arg_expr, &IrTy::I32)?;
                            let src = self.val_to_reg(val)?;
                            self.instrs.push(Instr::Store { src, slot: base_slot + i as u8 });
                        }
                        return Ok(());
                    }
                }

                // Normal (non-struct) let binding.
                //
                // FLS §8.1: The introduced binding comes into scope for the
                // remainder of the block *after* the let statement completes.
                // In particular, the RHS initializer is evaluated in the scope
                // that does NOT yet include the new binding. This is the
                // shadowing rule: `let x = x + 3` means "evaluate the old x,
                // add 3, bind the result to a new x". The old x remains
                // accessible during RHS evaluation.
                //
                // Ordering: alloc slot first (so the slot index is stable),
                // evaluate RHS (still sees the old `var_name` binding if any),
                // then insert the new binding into locals.
                let slot = self.alloc_slot()?;

                if let Some(init_expr) = init.as_ref() {
                    // Lower the initializer. We assume i32 for numeric
                    // expressions. Type inference is future work.
                    //
                    // FLS §8.1 AMBIGUOUS: the spec does not describe how type
                    // inference resolves the type of the initializer in the
                    // absence of a type annotation. We default to i32 for
                    // integer-producing expressions.
                    let val = self.lower_expr(init_expr, &IrTy::I32)?;
                    let src = self.val_to_reg(val)?;
                    self.instrs.push(Instr::Store { src, slot });
                }
                // Bring the new binding into scope only after the initializer
                // has been fully evaluated. FLS §8.1: uninitialized let
                // bindings (no init_expr) also become visible here.
                self.locals.insert(var_name, slot);

                Ok(())
            }

            // FLS §8.2: Expression statement — evaluate for side effects, discard value.
            //
            // Assignment and call expressions are the primary expression statements
            // at this milestone. `lower_expr` is called with `IrTy::Unit` as the
            // type hint; assignment and call handlers ignore `ret_ty`, so this is
            // safe. Unsupported expression kinds will propagate their own errors.
            //
            // FLS §6.1.2:37–45: The expression executes at runtime; its result
            // (if any) is discarded.
            StmtKind::Expr(expr) => {
                self.lower_expr(expr, &IrTy::Unit)?;
                Ok(())
            }

            // FLS §8.2: Empty statements are no-ops.
            StmtKind::Empty => Ok(()),
        }
    }

    // ── Expression lowering ──────────────────────────────────────────────────

    /// Lower an expression to runtime IR instructions.
    ///
    /// Returns the `IrValue` holding the result. Emits `LoadImm`, `BinOp`,
    /// `Load`, `Label`, `Branch`, `CondBranch`, etc. into `self.instrs`.
    ///
    /// `ret_ty` is the expected type of this expression. Used to select which
    /// variant of a literal or operator to emit.
    ///
    /// FLS §6.1.2:37–45: All code here emits runtime instructions.
    fn lower_expr(&mut self, expr: &Expr, ret_ty: &IrTy) -> Result<IrValue, LowerError> {
        match &expr.kind {
            // FLS §2.4.4.1: Integer literal — materialize as a runtime immediate.
            ExprKind::LitInt(n) => {
                match ret_ty {
                    IrTy::I32 => {
                        if *n > i32::MAX as u128 {
                            return Err(LowerError::Unsupported(format!(
                                "integer literal {n} out of range for i32"
                            )));
                        }
                        let n = *n as i32;
                        let r = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(r, n));
                        Ok(IrValue::Reg(r))
                    }
                    // FLS §4.1: Unsigned integer literal. Reuses LoadImm(i32)
                    // so the value must fit in i32 range at this milestone.
                    // Values in (i32::MAX, u32::MAX] require MOVZ+MOVK and are
                    // deferred (FLS §2.4.4.1 AMBIGUOUS: spec does not specify
                    // encoding limits for large unsigned literals).
                    IrTy::U32 => {
                        if *n > i32::MAX as u128 {
                            return Err(LowerError::Unsupported(format!(
                                "unsigned literal {n} > {}: MOVZ+MOVK not yet supported",
                                i32::MAX
                            )));
                        }
                        let n = *n as i32;
                        let r = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(r, n));
                        Ok(IrValue::Reg(r))
                    }
                    _ => Err(LowerError::Unsupported("integer literal with non-integer type".into())),
                }
            }

            // FLS §2.4.7: Boolean literals — `false` = 0, `true` = 1.
            //
            // Booleans are materialized as 0/1 integer immediates. The `CondBranch`
            // instruction tests for zero, matching `false` semantics naturally.
            //
            // FLS §6.1.2:37–45: Even statically-known booleans emit a `mov` at
            // runtime — no constant folding of `if true { ... }` to the then branch.
            ExprKind::LitBool(b) => {
                let r = self.alloc_reg()?;
                self.instrs.push(Instr::LoadImm(r, if *b { 1 } else { 0 }));
                Ok(IrValue::Reg(r))
            }

            // FLS §4.4: Unit literal `()`.
            ExprKind::Unit => Ok(IrValue::Unit),

            // FLS §6.4: Block expression — lower statements then the tail value.
            //
            // A block expression `{ stmt; ...; tail }` evaluates each statement
            // in order and produces the tail expression's value.
            ExprKind::Block(block) => {
                self.lower_block_to_value(block, ret_ty)
            }

            // FLS §6.3: Path expression — a reference to a local variable or
            // an enum unit variant.
            //
            // A single-segment path is either a const item reference or a
            // local variable reference.
            //
            // FLS §7.1:10: Every use of a constant is replaced with its value.
            // For const items, emit `LoadImm` with the compile-time value.
            //
            // FLS §6.3: A local variable path emits `Load` at runtime.
            // FLS §6.1.2:37–45: The load is a runtime instruction — even if
            // the variable holds a statically-known value, we must load it.
            ExprKind::Path(segments) if segments.len() == 1 => {
                let var_name = segments[0].text(self.source);
                // Check const_vals first: FLS §7.1 substitution takes
                // precedence over any local shadowing (rustc warns on shadow).
                if let Some(&const_val) = self.const_vals.get(var_name) {
                    let r = self.alloc_reg()?;
                    self.instrs.push(Instr::LoadImm(r, const_val));
                    return Ok(IrValue::Reg(r));
                }
                // Check static_names: FLS §7.2 — all references to a static
                // go through its memory address (ADRP + ADD + LDR).
                if self.static_names.contains(var_name) {
                    let r = self.alloc_reg()?;
                    self.instrs.push(Instr::LoadStatic { dst: r, name: var_name.to_owned() });
                    return Ok(IrValue::Reg(r));
                }
                let slot = self.locals.get(var_name).copied().ok_or_else(|| {
                    LowerError::Unsupported(format!("undefined variable `{var_name}`"))
                })?;
                let dst = self.alloc_reg()?;
                self.instrs.push(Instr::Load { dst, slot });
                Ok(IrValue::Reg(dst))
            }

            // FLS §6.3 + §15: Two-segment path expression — an enum unit variant.
            //
            // `Color::Red` resolves to the integer discriminant of `Red` in
            // the `Color` enum. Emits `LoadImm(discriminant)` at runtime.
            //
            // FLS §6.1.2:37–45: Even though the discriminant is a compile-time
            // constant, we emit a runtime `mov` — consistent with how integer
            // literals are handled in non-const contexts.
            //
            // FLS §15 AMBIGUOUS: The spec does not specify the default discriminant
            // values for unit variants. Galvanic assigns 0, 1, 2, ... in declaration
            // order, which matches rustc's default behavior.
            ExprKind::Path(segments) if segments.len() == 2 => {
                let enum_name = segments[0].text(self.source);
                let variant_name = segments[1].text(self.source);
                let discriminant = self.enum_defs
                    .get(enum_name)
                    .and_then(|variants| variants.get(variant_name))
                    .map(|(disc, _)| *disc)
                    .ok_or_else(|| {
                        LowerError::Unsupported(format!(
                            "unknown path `{enum_name}::{variant_name}` (not an enum variant)"
                        ))
                    })?;
                let r = self.alloc_reg()?;
                self.instrs.push(Instr::LoadImm(r, discriminant));
                Ok(IrValue::Reg(r))
            }

            // FLS §6.5.5: Arithmetic binary operations — emit runtime instructions.
            // FLS §6.5.6: Bit operator expressions (`&`, `|`, `^`).
            // FLS §6.5.7: Shift operator expressions (`<<`, `>>`).
            //
            // Both operands are lowered recursively, producing LoadImm/BinOp
            // instructions. The result is in a virtual register.
            //
            // Division and remainder are included here. FLS §6.23: division by
            // zero panics at runtime; galvanic does not yet insert a check
            // (FLS §6.23 AMBIGUOUS: the panic mechanism is not yet specified).
            //
            // FLS §6.5.6: "The type of a bit expression is the type of the left
            // operand." Both operands must have the same integer type.
            //
            // FLS §6.5.7: For signed integer types, `>>` is arithmetic shift
            // (sign-extending). The shift amount is taken modulo the bit width.
            // FLS §6.5.7 AMBIGUOUS: the spec says the shift amount is taken modulo
            // the bit width, but does not specify the exact register width used for
            // the modulo (ARM64 uses 6 bits of the shift register for 64-bit shifts).
            //
            // FLS §6.1.2:37–45: All these operators emit runtime instructions —
            // no constant folding even when both operands are literals.
            ExprKind::Binary { op, lhs, rhs }
                if matches!(
                    op,
                    BinOp::Add
                        | BinOp::Sub
                        | BinOp::Mul
                        | BinOp::Div
                        | BinOp::Rem
                        | BinOp::BitAnd
                        | BinOp::BitOr
                        | BinOp::BitXor
                        | BinOp::Shl
                        | BinOp::Shr
                ) =>
            {
                match ret_ty {
                    IrTy::I32 => {
                        let lhs_val = self.lower_expr(lhs, ret_ty)?;

                        // Spill lhs to a stack slot if rhs contains a call.
                        //
                        // ARM64 calling convention: x0–x17 are caller-saved.
                        // A `bl` instruction in the rhs lowers to a call that
                        // clobbers every register in that range, including the
                        // register that holds the lhs result. Without a spill,
                        // `fib(n-1) + fib(n-2)` would compute the wrong answer
                        // because fib(n-2)'s call sequence overwrites the
                        // register that holds fib(n-1)'s result.
                        //
                        // Only spill when lhs is already in a register (not an
                        // unresolved immediate like `IrValue::I32`). Immediates
                        // are not in any register yet so they are unaffected by
                        // the call.
                        //
                        // FLS §6.12.1: Call expressions follow ARM64 AAPCS64
                        // calling convention (caller-saved: x0–x17).
                        let lhs_spill: Option<u8> = if let IrValue::Reg(r) = lhs_val {
                            if expr_contains_call(rhs) {
                                let slot = self.alloc_slot()?;
                                self.instrs.push(Instr::Store { src: r, slot });
                                Some(slot)
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        let rhs_val = self.lower_expr(rhs, ret_ty)?;

                        // Reload lhs from its spill slot, if we spilled it.
                        let lhs_val = if let Some(slot) = lhs_spill {
                            let dst = self.alloc_reg()?;
                            self.instrs.push(Instr::Load { dst, slot });
                            IrValue::Reg(dst)
                        } else {
                            lhs_val
                        };

                        let lhs_reg = self.val_to_reg(lhs_val)?;
                        let rhs_reg = self.val_to_reg(rhs_val)?;

                        let dst = self.alloc_reg()?;
                        let ir_op = match op {
                            BinOp::Add => IrBinOp::Add,
                            BinOp::Sub => IrBinOp::Sub,
                            BinOp::Mul => IrBinOp::Mul,
                            BinOp::Div => IrBinOp::Div,
                            BinOp::Rem => IrBinOp::Rem,
                            BinOp::BitAnd => IrBinOp::BitAnd,
                            BinOp::BitOr => IrBinOp::BitOr,
                            BinOp::BitXor => IrBinOp::BitXor,
                            BinOp::Shl => IrBinOp::Shl,
                            BinOp::Shr => IrBinOp::Shr,
                            _ => unreachable!("matched above"),
                        };
                        self.instrs.push(Instr::BinOp { op: ir_op, dst, lhs: lhs_reg, rhs: rhs_reg });
                        Ok(IrValue::Reg(dst))
                    }
                    // FLS §4.1: Unsigned integer arithmetic. Same spill/reload
                    // logic as signed, but division uses `udiv` (IrBinOp::UDiv)
                    // and right shift uses `lsr` (IrBinOp::UShr) for correct
                    // unsigned semantics. Add/sub/mul/bitwise are identical to
                    // signed at the hardware level on ARM64.
                    IrTy::U32 => {
                        let lhs_val = self.lower_expr(lhs, ret_ty)?;

                        let lhs_spill: Option<u8> = if let IrValue::Reg(r) = lhs_val {
                            if expr_contains_call(rhs) {
                                let slot = self.alloc_slot()?;
                                self.instrs.push(Instr::Store { src: r, slot });
                                Some(slot)
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        let rhs_val = self.lower_expr(rhs, ret_ty)?;

                        let lhs_val = if let Some(slot) = lhs_spill {
                            let dst = self.alloc_reg()?;
                            self.instrs.push(Instr::Load { dst, slot });
                            IrValue::Reg(dst)
                        } else {
                            lhs_val
                        };

                        let lhs_reg = self.val_to_reg(lhs_val)?;
                        let rhs_reg = self.val_to_reg(rhs_val)?;

                        let dst = self.alloc_reg()?;
                        // FLS §4.1: unsigned uses udiv and lsr; all others identical to signed.
                        let ir_op = match op {
                            BinOp::Add => IrBinOp::Add,
                            BinOp::Sub => IrBinOp::Sub,
                            BinOp::Mul => IrBinOp::Mul,
                            BinOp::Div => IrBinOp::UDiv,
                            BinOp::Rem => IrBinOp::Rem, // unsigned rem: sdiv step replaced by udiv in codegen
                            BinOp::BitAnd => IrBinOp::BitAnd,
                            BinOp::BitOr => IrBinOp::BitOr,
                            BinOp::BitXor => IrBinOp::BitXor,
                            BinOp::Shl => IrBinOp::Shl,
                            BinOp::Shr => IrBinOp::UShr,
                            _ => unreachable!("matched above"),
                        };
                        self.instrs.push(Instr::BinOp { op: ir_op, dst, lhs: lhs_reg, rhs: rhs_reg });
                        Ok(IrValue::Reg(dst))
                    }
                    _ => Err(LowerError::Unsupported("bitwise/arithmetic on non-integer type".into())),
                }
            }

            // FLS §6.17: If expressions.
            //
            // An if expression evaluates the condition, then executes exactly one
            // of the two branches at runtime. The result value of the taken branch
            // is the value of the whole expression.
            //
            // Lowering strategy:
            //   1. Allocate a result stack slot (the "phi slot") before either branch.
            //   2. Lower the condition to a register.
            //   3. Emit `CondBranch` (cbz): if condition == 0 (false), jump to else.
            //   4. Lower the then-branch, store its result to the phi slot.
            //   5. Emit `Branch` (b) to end label.
            //   6. Emit `Label` for else.
            //   7. Lower the else-branch (or unit if absent), store result to phi slot.
            //   8. Emit `Label` for end.
            //   9. Load from phi slot into a fresh register.
            //
            // FLS §6.17: "The type of the if expression is the type of the last
            // expression in the block." Both branches must have the same type.
            //
            // FLS §6.1.2:37–45: The condition and both branches emit runtime
            // instructions. A `true` condition still emits `mov x0, #1; cbz x0, ...`
            // — the branch resolves at runtime, not compile time.
            //
            ExprKind::If { cond, then_block, else_expr } => {
                match ret_ty {
                    // FLS §6.17: If expression producing an i32 or bool value.
                    //
                    // Both types use the same phi-slot lowering pattern — the
                    // result is a 0/1 or signed integer in a register. The
                    // branches write to a shared stack slot; the result is
                    // loaded once after the if expression completes.
                    //
                    // Conditions are always lowered as `IrTy::Bool` so that
                    // `!bool_var` in a condition emits logical NOT (`eor`) rather
                    // than bitwise NOT (`mvn`). FLS §6.5.4: `!` on bool is logical.
                    //
                    // FLS §6.17: "The type of the if expression is the type of
                    // the last expression in the block." Both branches must have
                    // the same type.
                    //
                    // FLS §6.1.2:37–45: Even statically-known conditions emit
                    // a `cbz` — no constant folding of `if true { ... }`.
                    //
                    // Cache-line note: the phi slot is one 8-byte stack entry;
                    // read exactly once after the if expression completes.
                    IrTy::I32 | IrTy::Bool | IrTy::U32 => {
                        let else_label = self.alloc_label();
                        let end_label = self.alloc_label();

                        // Allocate the phi slot before entering either branch so
                        // both branches write to the same stack location.
                        let phi_slot = self.alloc_slot()?;

                        // Lower condition as bool so that `!bool_var` emits
                        // logical NOT (BoolNot) rather than bitwise NOT (Not).
                        // FLS §6.5.4: `!` on bool must be logical NOT.
                        let cond_val = self.lower_expr(cond, &IrTy::Bool)?;
                        let cond_reg = self.val_to_reg(cond_val)?;

                        // CondBranch: jump to else_label if condition is false (0).
                        self.instrs.push(Instr::CondBranch { reg: cond_reg, label: else_label });

                        // ── Then branch ───────────────────────────────────────────
                        let then_val = self.lower_block_to_value(then_block, ret_ty)?;
                        let then_reg = self.val_to_reg(then_val)?;
                        self.instrs.push(Instr::Store { src: then_reg, slot: phi_slot });
                        self.instrs.push(Instr::Branch(end_label));

                        // ── Else branch ───────────────────────────────────────────
                        self.instrs.push(Instr::Label(else_label));
                        let else_val = match else_expr {
                            Some(e) => self.lower_expr(e, ret_ty)?,
                            None => {
                                // FLS §6.17: if without else has type `()`. Using it
                                // where i32/bool is expected is a type error — unsupported.
                                return Err(LowerError::Unsupported(
                                    "if expression without else in non-unit context".into(),
                                ));
                            }
                        };
                        let else_reg = self.val_to_reg(else_val)?;
                        self.instrs.push(Instr::Store { src: else_reg, slot: phi_slot });

                        // ── End ───────────────────────────────────────────────────
                        self.instrs.push(Instr::Label(end_label));
                        let result_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: result_reg, slot: phi_slot });
                        Ok(IrValue::Reg(result_reg))
                    }

                    // FLS §6.17: If expression with unit type (no value needed).
                    //
                    // Used when the if expression is a statement (value discarded)
                    // or when both branches produce `()`. The body of a loop uses
                    // this path, so `if cond { break; }` lowers correctly.
                    //
                    // No phi slot is allocated — the branches run for side effects.
                    //
                    // Conditions are lowered as `IrTy::Bool` so that `!b` as an
                    // if condition emits logical NOT. FLS §6.5.4.
                    //
                    // FLS §6.17: "If an if expression does not have an else expression,
                    // its type is the unit type."
                    //
                    // FLS §6.1.2:37–45: The condition still emits a runtime `cbz`.
                    IrTy::Unit => {
                        let else_label = self.alloc_label();
                        let end_label = self.alloc_label();

                        // Lower condition as bool — ensures `!bool_var` uses BoolNot.
                        // FLS §6.5.4: `!` on bool is logical NOT (eor), not bitwise (mvn).
                        let cond_val = self.lower_expr(cond, &IrTy::Bool)?;
                        let cond_reg = self.val_to_reg(cond_val)?;

                        // Jump to else (or end) if condition is false.
                        self.instrs.push(Instr::CondBranch { reg: cond_reg, label: else_label });

                        // ── Then branch (unit, side effects only) ─────────────────
                        self.lower_block_to_value(then_block, &IrTy::Unit)?;
                        self.instrs.push(Instr::Branch(end_label));

                        // ── Else branch ───────────────────────────────────────────
                        self.instrs.push(Instr::Label(else_label));
                        if let Some(e) = else_expr {
                            self.lower_expr(e, &IrTy::Unit)?;
                        }

                        self.instrs.push(Instr::Label(end_label));
                        Ok(IrValue::Unit)
                    }
                }
            }

            // FLS §6.17: If-let expression.
            //
            // `if let pat = scrutinee { then_block } [else { else_expr }]`
            //
            // Lowering strategy:
            //   1. Lower scrutinee → spill to a stack slot.
            //   2. Emit a pattern check (same logic as match arm) with a
            //      conditional branch to else_label on no-match.
            //   3. If the pattern has an identifier binding, install it before
            //      lowering the then block and remove it after.
            //   4. Lower the then block.
            //   5. Branch to end_label.
            //   6. Emit else_label, lower else_expr (if any).
            //   7. Emit end_label; return result via phi slot (i32/bool) or unit.
            //
            // FLS §6.17: "An if let expression is syntactic sugar for a match
            // expression with a single arm." Lowered directly for clarity.
            // FLS §6.1.2:37–45: All checks emit runtime instructions.
            // Cache-line note: same instruction count as a 2-arm match.
            ExprKind::IfLet { pat, scrutinee, then_block, else_expr } => {
                // FLS §15: If the scrutinee is a plain variable holding an enum
                // value, record its base slot for TupleStruct/StructVariant pattern
                // field access. `scrut_slot` holds the discriminant copy; fields are
                // at `enum_base_slot + 1..N`.
                let enum_base_slot: Option<u8> =
                    if let ExprKind::Path(segs) = &scrutinee.kind
                        && segs.len() == 1
                    {
                        let var_name = segs[0].text(self.source);
                        self.locals
                            .get(var_name)
                            .copied()
                            .filter(|s| self.local_enum_types.contains_key(s))
                    } else {
                        None
                    };
                // FLS §5.3: If the scrutinee is a plain struct variable, record its
                // base slot for struct pattern field access. Fields are at base_slot +
                // field_idx (no discriminant offset, unlike enum variants).
                let struct_base_slot: Option<u8> =
                    if let ExprKind::Path(segs) = &scrutinee.kind
                        && segs.len() == 1
                    {
                        let var_name = segs[0].text(self.source);
                        self.locals
                            .get(var_name)
                            .copied()
                            .filter(|s| self.local_struct_types.contains_key(s))
                    } else {
                        None
                    };
                // Infer scrutinee type from the pattern (bool literal → Bool, else I32).
                let scrut_ty = match pat {
                    Pat::LitBool(_) => IrTy::Bool,
                    Pat::Or(alts) if alts.iter().any(|p| matches!(p, Pat::LitBool(_))) => {
                        IrTy::Bool
                    }
                    _ => IrTy::I32,
                };
                let scrut_val = self.lower_expr(scrutinee, &scrut_ty)?;
                let scrut_reg = self.val_to_reg(scrut_val)?;
                let scrut_slot = self.alloc_slot()?;
                self.instrs.push(Instr::Store { src: scrut_reg, slot: scrut_slot });

                // Bindings introduced by TupleStruct/StructVariant/Ident patterns;
                // all are removed after the then block (before the else branch).
                let mut bound_names: Vec<&str> = Vec::new();

                let else_label = self.alloc_label();
                let end_label = self.alloc_label();

                // Emit pattern check: branch to else_label on no-match.
                //
                // FLS §6.17: Wildcard and identifier patterns always match;
                // literal and range patterns require a runtime comparison.
                match pat {
                    Pat::Wildcard => {
                        // Always matches — no conditional branch needed.
                    }
                    Pat::Ident(_) => {
                        // Always matches — no conditional branch needed.
                        // Binding is installed below, before the then block.
                    }
                    Pat::LitInt(n) => {
                        let s_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                        let p_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(p_reg, *n as i32));
                        let eq_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Eq,
                            dst: eq_reg,
                            lhs: s_reg,
                            rhs: p_reg,
                        });
                        self.instrs.push(Instr::CondBranch { reg: eq_reg, label: else_label });
                    }
                    Pat::NegLitInt(n) => {
                        let s_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                        let p_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(p_reg, -(*n as i32)));
                        let eq_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Eq,
                            dst: eq_reg,
                            lhs: s_reg,
                            rhs: p_reg,
                        });
                        self.instrs.push(Instr::CondBranch { reg: eq_reg, label: else_label });
                    }
                    Pat::LitBool(b) => {
                        let s_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                        let p_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(p_reg, *b as i32));
                        let eq_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Eq,
                            dst: eq_reg,
                            lhs: s_reg,
                            rhs: p_reg,
                        });
                        self.instrs.push(Instr::CondBranch { reg: eq_reg, label: else_label });
                    }
                    // FLS §5.5 + §15: Path pattern — enum unit variant.
                    // Resolves to the variant's integer discriminant; then compares
                    // against the scrutinee exactly like a LitInt pattern.
                    Pat::Path(segs) if segs.len() == 2 => {
                        let enum_name = segs[0].text(self.source);
                        let variant_name = segs[1].text(self.source);
                        let discriminant = self.enum_defs
                            .get(enum_name)
                            .and_then(|v| v.get(variant_name))
                            .map(|(disc, _)| *disc)
                            .ok_or_else(|| LowerError::Unsupported(format!(
                                "unknown enum variant `{enum_name}::{variant_name}` in pattern"
                            )))?;
                        let s_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                        let p_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(p_reg, discriminant));
                        let eq_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Eq,
                            dst: eq_reg,
                            lhs: s_reg,
                            rhs: p_reg,
                        });
                        self.instrs.push(Instr::CondBranch { reg: eq_reg, label: else_label });
                    }
                    Pat::Path(_) => {
                        return Err(LowerError::Unsupported(
                            "path pattern must have exactly two segments (EnumName::Variant)".into(),
                        ));
                    }
                    Pat::RangeInclusive { lo, hi } => {
                        let s_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                        let lo_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(lo_reg, *lo as i32));
                        let cmp1 = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Ge,
                            dst: cmp1,
                            lhs: s_reg,
                            rhs: lo_reg,
                        });
                        let hi_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(hi_reg, *hi as i32));
                        let cmp2 = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Le,
                            dst: cmp2,
                            lhs: s_reg,
                            rhs: hi_reg,
                        });
                        let matched = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::BitAnd,
                            dst: matched,
                            lhs: cmp1,
                            rhs: cmp2,
                        });
                        self.instrs.push(Instr::CondBranch { reg: matched, label: else_label });
                    }
                    Pat::RangeExclusive { lo, hi } => {
                        let s_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                        let lo_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(lo_reg, *lo as i32));
                        let cmp1 = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Ge,
                            dst: cmp1,
                            lhs: s_reg,
                            rhs: lo_reg,
                        });
                        let hi_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(hi_reg, *hi as i32));
                        let cmp2 = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Lt,
                            dst: cmp2,
                            lhs: s_reg,
                            rhs: hi_reg,
                        });
                        let matched = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::BitAnd,
                            dst: matched,
                            lhs: cmp1,
                            rhs: cmp2,
                        });
                        self.instrs.push(Instr::CondBranch { reg: matched, label: else_label });
                    }
                    Pat::Or(alts) => {
                        // FLS §5.1.11: OR pattern in if-let — match if any alternative matches.
                        // Strategy: accumulate equality into matched_reg (starts at 0).
                        let matched_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(matched_reg, 0));
                        for alt in alts {
                            match alt {
                                Pat::Wildcard => {
                                    self.instrs.push(Instr::LoadImm(matched_reg, 1));
                                    break;
                                }
                                Pat::Or(_) | Pat::Ident(_) => {
                                    return Err(LowerError::Unsupported(
                                        "nested OR or identifier inside if-let OR pattern".into(),
                                    ));
                                }
                                _ => {
                                    let alt_imm = self.pat_scalar_imm(alt)?;
                                    let si_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load {
                                        dst: si_reg,
                                        slot: scrut_slot,
                                    });
                                    let alt_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(alt_reg, alt_imm));
                                    let eq_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Eq,
                                        dst: eq_reg,
                                        lhs: si_reg,
                                        rhs: alt_reg,
                                    });
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::BitOr,
                                        dst: matched_reg,
                                        lhs: matched_reg,
                                        rhs: eq_reg,
                                    });
                                }
                            }
                        }
                        self.instrs.push(Instr::CondBranch {
                            reg: matched_reg,
                            label: else_label,
                        });
                    }
                    // FLS §5.4 + §15: TupleStruct pattern in if-let.
                    //
                    // `if let Enum::Variant(f0, f1, ..) = x { then } else { else }`
                    // Strategy: compare discriminant at scrut_slot against the variant's
                    // discriminant; branch to else_label on mismatch; then install
                    // positional field bindings from enum_base_slot + 1 + idx.
                    //
                    // FLS §6.1.2:37–45: All instructions are runtime.
                    // Cache-line note: ~5 instructions (ldr + mov + cmp + cset + cbz)
                    // for the discriminant check, plus 2×N for N field bindings.
                    Pat::TupleStruct { path: segs, fields } => {
                        if segs.len() != 2 {
                            return Err(LowerError::Unsupported(
                                "tuple struct pattern path must have two segments".into(),
                            ));
                        }
                        let enum_name = segs[0].text(self.source);
                        let variant_name = segs[1].text(self.source);
                        let discriminant = self
                            .enum_defs
                            .get(enum_name)
                            .and_then(|v| v.get(variant_name))
                            .map(|(disc, _)| *disc)
                            .ok_or_else(|| {
                                LowerError::Unsupported(format!(
                                    "unknown enum variant `{enum_name}::{variant_name}`"
                                ))
                            })?;
                        let base = enum_base_slot.ok_or_else(|| {
                            LowerError::Unsupported(
                                "TupleStruct pattern in if-let requires enum variable scrutinee"
                                    .into(),
                            )
                        })?;
                        // Discriminant check — branch to else on mismatch.
                        let s_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                        let p_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(p_reg, discriminant));
                        let cmp_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Eq,
                            dst: cmp_reg,
                            lhs: s_reg,
                            rhs: p_reg,
                        });
                        self.instrs.push(Instr::CondBranch { reg: cmp_reg, label: else_label });
                        // Install positional field bindings into bound_names.
                        for (fi, fp) in fields.iter().enumerate() {
                            if let Pat::Ident(span) = fp {
                                let fname = span.text(self.source);
                                let fslot = base + 1 + fi as u8;
                                let bslot = self.alloc_slot()?;
                                let breg = self.alloc_reg()?;
                                self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                self.locals.insert(fname, bslot);
                                bound_names.push(fname);
                            } else if !matches!(fp, Pat::Wildcard) {
                                return Err(LowerError::Unsupported(
                                    "only ident/wildcard fields in TupleStruct if-let pattern"
                                        .into(),
                                ));
                            }
                        }
                    }
                    // FLS §5.3 + §15.3: Named-field struct or variant pattern in if-let.
                    //
                    // One-segment path (`Point { x, y }`): plain struct pattern — irrefutable,
                    // no discriminant check, fields at struct_base_slot + field_idx.
                    // Two-segment path (`Enum::Variant { field, … }`): enum variant — compare
                    // discriminant, branch to else_label on mismatch, bind fields at
                    // enum_base_slot + 1 + field_idx.
                    //
                    // FLS §5.3: "A struct pattern matches a struct or enum struct variant
                    // by its field patterns."
                    // FLS §6.1.2:37–45: All instructions are runtime.
                    // Cache-line note: 2×N instructions (plain struct); ~5 + 2×N (enum variant).
                    Pat::StructVariant { path: segs, fields: pat_fields } => {
                        if segs.len() == 1 {
                            // FLS §5.3: Plain struct pattern — always matches (irrefutable).
                            let struct_name = segs[0].text(self.source);
                            let base = struct_base_slot.ok_or_else(|| {
                                LowerError::Unsupported(
                                    "plain struct pattern in if-let requires struct variable scrutinee".into(),
                                )
                            })?;
                            let field_names = self.struct_defs
                                .get(struct_name)
                                .cloned()
                                .ok_or_else(|| LowerError::Unsupported(format!(
                                    "unknown struct `{struct_name}`"
                                )))?;
                            // No CondBranch — plain struct patterns are irrefutable.
                            for (fname_span, fp) in pat_fields.iter() {
                                let fname = fname_span.text(self.source);
                                let field_idx = field_names.iter().position(|n| n == fname)
                                    .ok_or_else(|| LowerError::Unsupported(format!(
                                        "struct `{struct_name}` has no field `{fname}`"
                                    )))?;
                                match fp {
                                    Pat::Ident(bind_span) => {
                                        let bind_name = bind_span.text(self.source);
                                        let fslot = base + field_idx as u8;
                                        let bslot = self.alloc_slot()?;
                                        let breg = self.alloc_reg()?;
                                        self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                        self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                        self.locals.insert(bind_name, bslot);
                                        bound_names.push(bind_name);
                                    }
                                    Pat::Wildcard => {}
                                    _ => return Err(LowerError::Unsupported(
                                        "only ident/wildcard sub-patterns in struct if-let patterns".into(),
                                    )),
                                }
                            }
                        } else if segs.len() == 2 {
                            let enum_name = segs[0].text(self.source);
                            let variant_name = segs[1].text(self.source);
                            let (discriminant, field_names) = self
                                .enum_defs
                                .get(enum_name)
                                .and_then(|v| v.get(variant_name))
                                .cloned()
                                .ok_or_else(|| {
                                    LowerError::Unsupported(format!(
                                        "unknown enum variant `{enum_name}::{variant_name}`"
                                    ))
                                })?;
                            let base = enum_base_slot.ok_or_else(|| {
                                LowerError::Unsupported(
                                    "StructVariant pattern in if-let requires enum variable scrutinee"
                                        .into(),
                                )
                            })?;
                            // Discriminant check — branch to else on mismatch.
                            let s_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                            let p_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::LoadImm(p_reg, discriminant));
                            let cmp_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::BinOp {
                                op: IrBinOp::Eq,
                                dst: cmp_reg,
                                lhs: s_reg,
                                rhs: p_reg,
                            });
                            self.instrs.push(Instr::CondBranch { reg: cmp_reg, label: else_label });
                            // Install named field bindings by declaration order.
                            for (fname_span, fp) in pat_fields.iter() {
                                let fname = fname_span.text(self.source);
                                let field_idx = field_names
                                    .iter()
                                    .position(|n| n == fname)
                                    .ok_or_else(|| {
                                        LowerError::Unsupported(format!(
                                            "enum variant `{enum_name}::{variant_name}` has no field `{fname}`"
                                        ))
                                    })?;
                                match fp {
                                    Pat::Ident(bind_span) => {
                                        let bind_name = bind_span.text(self.source);
                                        let fslot = base + 1 + field_idx as u8;
                                        let bslot = self.alloc_slot()?;
                                        let breg = self.alloc_reg()?;
                                        self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                        self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                        self.locals.insert(bind_name, bslot);
                                        bound_names.push(bind_name);
                                    }
                                    Pat::Wildcard => {}
                                    _ => return Err(LowerError::Unsupported(
                                        "only ident/wildcard sub-patterns in StructVariant if-let fields".into(),
                                    )),
                                }
                            }
                        } else {
                            return Err(LowerError::Unsupported(
                                "struct/variant if-let pattern path must have 1 or 2 segments".into(),
                            ));
                        }
                    }
                    Pat::Tuple(_) => {
                        return Err(LowerError::Unsupported(
                            "tuple pattern in if-let not yet supported".into(),
                        ));
                    }
                }

                // Install identifier binding (if any) before the then block.
                // FLS §5.1.4: The binding is in scope for the then block only.
                // TupleStruct/StructVariant field bindings were already pushed to
                // bound_names inside the pattern check above.
                if let Pat::Ident(span) = pat {
                    let name = span.text(self.source);
                    let bind_slot = self.alloc_slot()?;
                    let bind_reg = self.alloc_reg()?;
                    self.instrs.push(Instr::Load { dst: bind_reg, slot: scrut_slot });
                    self.instrs.push(Instr::Store { src: bind_reg, slot: bind_slot });
                    self.locals.insert(name, bind_slot);
                    bound_names.push(name);
                }

                match ret_ty {
                    IrTy::I32 | IrTy::Bool | IrTy::U32 => {
                        let phi_slot = self.alloc_slot()?;

                        // Then branch.
                        let then_val = self.lower_block_to_value(then_block, ret_ty)?;
                        for name in &bound_names {
                            self.locals.remove(*name);
                        }
                        let then_reg = self.val_to_reg(then_val)?;
                        self.instrs.push(Instr::Store { src: then_reg, slot: phi_slot });
                        self.instrs.push(Instr::Branch(end_label));

                        // Else branch.
                        self.instrs.push(Instr::Label(else_label));
                        let else_val = match else_expr {
                            Some(e) => self.lower_expr(e, ret_ty)?,
                            None => {
                                return Err(LowerError::Unsupported(
                                    "if-let without else in non-unit context".into(),
                                ));
                            }
                        };
                        let else_reg = self.val_to_reg(else_val)?;
                        self.instrs.push(Instr::Store { src: else_reg, slot: phi_slot });

                        self.instrs.push(Instr::Label(end_label));
                        let result_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: result_reg, slot: phi_slot });
                        Ok(IrValue::Reg(result_reg))
                    }
                    IrTy::Unit => {
                        // Then branch (side effects only).
                        self.lower_block_to_value(then_block, &IrTy::Unit)?;
                        for name in &bound_names {
                            self.locals.remove(*name);
                        }
                        self.instrs.push(Instr::Branch(end_label));

                        // Else branch.
                        self.instrs.push(Instr::Label(else_label));
                        if let Some(e) = else_expr {
                            self.lower_expr(e, &IrTy::Unit)?;
                        }

                        self.instrs.push(Instr::Label(end_label));
                        Ok(IrValue::Unit)
                    }
                }
            }

            // FLS §6.18: Match expression.
            // FLS §5.1.4: Identifier patterns — bind scrutinee to a name.
            //
            // `match scrutinee { pat0 => body0, ..., default => bodyN }`
            //
            // Lowering strategy (comparison chain):
            //   1. Lower scrutinee → spill to a stack slot so it is safe across
            //      subsequent instruction sequences (each arm re-loads it).
            //   2. For each arm except the last: test `scrutinee == pat_val`
            //      using the existing BinOp(Eq) path and CondBranch; if the
            //      check fails, jump to the next arm's label.
            //   3. The last arm is emitted unconditionally (the compiler assumes
            //      exhaustiveness; a future type-checking pass will enforce it).
            //   4. Each arm stores its result to a phi slot (non-unit result)
            //      and branches to exit_label, or executes for side effects
            //      (unit result).
            //
            // FLS §6.18: "A match expression evaluates the scrutinee and then
            // matches it against each pattern in order. The first matching arm
            // executes."
            //
            // FLS §6.1.2:37–45: The scrutinee load and all pattern comparisons
            // emit runtime instructions — even a match on a literal constant
            // emits `ldr` + `mov` + `cmp` + `cset` + `cbz`.
            //
            // FLS §6.18 AMBIGUOUS: The spec requires exhaustiveness but does not
            // specify the checking algorithm. Galvanic defers exhaustiveness
            // checking to a future pass; the last arm is always emitted
            // unconditionally.
            //
            // Cache-line note: each arm costs ~5 instructions (ldr scrut + mov
            // pat + cmp + cset + cbz = 20 bytes); with the branch (4 bytes) and
            // label (0 bytes) that is 24 bytes per arm — two arms per 64-byte line.
            ExprKind::Match { scrutinee, arms } => {
                if arms.is_empty() {
                    return Err(LowerError::Unsupported("match expression with no arms".into()));
                }

                // Lower the scrutinee and spill to a stack slot.
                // The scrutinee may be i32 or bool; both use integer registers.
                // We infer the scrutinee type from context: use i32 by default,
                // and use bool if any arm has a bool literal pattern (including
                // bool literals inside OR patterns, FLS §5.1.11).
                let has_bool_pat = |pat: &Pat| -> bool {
                    match pat {
                        Pat::LitBool(_) => true,
                        // FLS §5.1.11: OR patterns can contain bool literal alternatives.
                        Pat::Or(alts) => alts.iter().any(|p| matches!(p, Pat::LitBool(_))),
                        // FLS §5.1.4: Identifier patterns are type-agnostic; they
                        // don't determine the scrutinee type on their own.
                        _ => false,
                    }
                };
                let scrut_ty = if arms.iter().any(|a| has_bool_pat(&a.pat)) {
                    IrTy::Bool
                } else {
                    IrTy::I32
                };
                // FLS §15: If the scrutinee is a plain variable holding an enum
                // tuple variant, record its base slot for TupleStruct pattern field
                // access. `scrut_slot` holds the discriminant copy; fields are at
                // `enum_base_slot + 1..N`.
                let enum_base_slot: Option<u8> =
                    if let ExprKind::Path(segs) = &scrutinee.kind
                        && segs.len() == 1
                    {
                        let var_name = segs[0].text(self.source);
                        self.locals
                            .get(var_name)
                            .copied()
                            .filter(|s| self.local_enum_types.contains_key(s))
                    } else {
                        None
                    };
                // FLS §5.3: If the scrutinee is a plain struct variable, record its
                // base slot for struct pattern field access. Fields are at base_slot +
                // field_idx (no discriminant offset, unlike enum variants).
                let struct_base_slot: Option<u8> =
                    if let ExprKind::Path(segs) = &scrutinee.kind
                        && segs.len() == 1
                    {
                        let var_name = segs[0].text(self.source);
                        self.locals
                            .get(var_name)
                            .copied()
                            .filter(|s| self.local_struct_types.contains_key(s))
                    } else {
                        None
                    };

                let scrut_val = self.lower_expr(scrutinee, &scrut_ty)?;
                let scrut_reg = self.val_to_reg(scrut_val)?;
                let scrut_slot = self.alloc_slot()?;
                self.instrs.push(Instr::Store { src: scrut_reg, slot: scrut_slot });

                // Split into checked arms (all but the last) and the default arm.
                // The default arm is emitted unconditionally (exhaustiveness deferred).
                let (checked_arms, default_arm) = arms.split_at(arms.len() - 1);

                let exit_label = self.alloc_label();

                match ret_ty {
                    IrTy::I32 | IrTy::Bool | IrTy::U32 => {
                        let phi_slot = self.alloc_slot()?;

                        for arm in checked_arms {
                            let next_label = self.alloc_label();

                            match &arm.pat {
                                Pat::Wildcard => {
                                    // Wildcard in non-last position — pattern always matches.
                                    // Check guard (if any) before executing the body.
                                    //
                                    // FLS §6.18: "A match arm guard is an additional condition
                                    // that must hold for the arm to be selected."
                                    // FLS §6.1.2:37–45: Guard check emits runtime instructions.
                                    if let Some(guard) = &arm.guard {
                                        let gv = self.lower_expr(guard, &IrTy::Bool)?;
                                        let gr = self.val_to_reg(gv)?;
                                        self.instrs.push(Instr::CondBranch {
                                            reg: gr,
                                            label: next_label,
                                        });
                                    }
                                    let body_val = self.lower_expr(&arm.body, ret_ty)?;
                                    let body_reg = self.val_to_reg(body_val)?;
                                    self.instrs.push(Instr::Store { src: body_reg, slot: phi_slot });
                                    self.instrs.push(Instr::Branch(exit_label));
                                    self.instrs.push(Instr::Label(next_label));
                                    continue;
                                }
                                // FLS §5.1.4: Identifier pattern in non-last position —
                                // always matches (like Wildcard) and binds scrutinee to name.
                                //
                                // Load scrutinee into a new slot keyed by the identifier name.
                                // The arm body accesses it via a normal path expression (Load).
                                // The binding is removed after the arm to avoid leaking into
                                // subsequent arms (correct scoping per FLS §5.1.4).
                                //
                                // FLS §6.18: Guard (if present) is evaluated after the binding
                                // is installed; the guard expression may reference the bound name.
                                // If the guard fails, the arm is skipped (CondBranch to next_label)
                                // and the binding is removed from `self.locals` before the label.
                                //
                                // FLS §6.1.2:37–45: The ldr/str pair emits at runtime.
                                // Cache-line note: 2 instructions (ldr + str = 8 bytes).
                                Pat::Ident(span) => {
                                    let name = span.text(self.source);
                                    let bind_slot = self.alloc_slot()?;
                                    let bind_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load { dst: bind_reg, slot: scrut_slot });
                                    self.instrs.push(Instr::Store { src: bind_reg, slot: bind_slot });
                                    self.locals.insert(name, bind_slot);
                                    // Guard check: binding is visible to the guard expression.
                                    if let Some(guard) = &arm.guard {
                                        let gv = self.lower_expr(guard, &IrTy::Bool)?;
                                        let gr = self.val_to_reg(gv)?;
                                        self.instrs.push(Instr::CondBranch {
                                            reg: gr,
                                            label: next_label,
                                        });
                                    }
                                    let body_val = self.lower_expr(&arm.body, ret_ty)?;
                                    let body_reg = self.val_to_reg(body_val)?;
                                    self.instrs.push(Instr::Store { src: body_reg, slot: phi_slot });
                                    self.instrs.push(Instr::Branch(exit_label));
                                    // Remove binding before the label so subsequent arms
                                    // do not see a stale entry in `self.locals`.
                                    self.locals.remove(name);
                                    self.instrs.push(Instr::Label(next_label));
                                    continue;
                                }
                                // FLS §5.1.11: OR pattern — accumulate equality results.
                                //
                                // Strategy: matched_reg starts at 0; for each alternative,
                                // load scrutinee, compare with pattern value, OR the 0/1
                                // equality result into matched_reg. After all alternatives,
                                // cbz matched_reg → next_label (arm not taken).
                                //
                                // FLS §6.1.2:37–45: All comparisons emit runtime instructions.
                                //
                                // Cache-line note: each alternative adds ~4 instructions
                                // (ldr + mov + cmp/cset + orr = 16 bytes), so 4 alternatives
                                // fit in a 64-byte instruction cache line.
                                Pat::Or(alts) => {
                                    let matched_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(matched_reg, 0));
                                    for alt in alts {
                                        match alt {
                                            Pat::Wildcard => {
                                                // Wildcard inside OR — always matches.
                                                self.instrs.push(Instr::LoadImm(matched_reg, 1));
                                                break;
                                            }
                                            Pat::Or(_) => {
                                                return Err(LowerError::Unsupported(
                                                    "nested OR patterns".into(),
                                                ));
                                            }
                                            // FLS §5.1.4: Identifier patterns inside OR are
                                            // not yet supported (binding semantics are complex
                                            // when combined with OR alternatives).
                                            Pat::Ident(_) => {
                                                return Err(LowerError::Unsupported(
                                                    "identifier pattern inside OR pattern".into(),
                                                ));
                                            }
                                            _ => {
                                                let alt_imm = self.pat_scalar_imm(alt)?;
                                                let si_reg = self.alloc_reg()?;
                                                self.instrs.push(Instr::Load {
                                                    dst: si_reg,
                                                    slot: scrut_slot,
                                                });
                                                let alt_reg = self.alloc_reg()?;
                                                self.instrs.push(Instr::LoadImm(alt_reg, alt_imm));
                                                let eq_reg = self.alloc_reg()?;
                                                self.instrs.push(Instr::BinOp {
                                                    op: IrBinOp::Eq,
                                                    dst: eq_reg,
                                                    lhs: si_reg,
                                                    rhs: alt_reg,
                                                });
                                                self.instrs.push(Instr::BinOp {
                                                    op: IrBinOp::BitOr,
                                                    dst: matched_reg,
                                                    lhs: matched_reg,
                                                    rhs: eq_reg,
                                                });
                                            }
                                        }
                                    }
                                    // cbz: skip arm if no alternative matched.
                                    self.instrs.push(Instr::CondBranch {
                                        reg: matched_reg,
                                        label: next_label,
                                    });
                                }
                                // FLS §5.1.9: Inclusive range pattern `lo..=hi`.
                                //
                                // Emit: lo <= scrutinee && scrutinee <= hi.
                                // Strategy: compare scrutinee >= lo (IrBinOp::Ge) → cmp1,
                                // compare scrutinee <= hi (IrBinOp::Le) → cmp2,
                                // AND the two results → matched, cbz on matched.
                                //
                                // FLS §6.1.2:37–45: All comparisons emit runtime instructions.
                                //
                                // Cache-line note: 7 instructions per range arm (ldr + 2×mov
                                // + 2×cmp + and + cbz = 28 bytes) — two arms per 64-byte line.
                                Pat::RangeInclusive { lo, hi } => {
                                    let s_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                                    let lo_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(lo_reg, *lo as i32));
                                    let cmp1 = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Ge,
                                        dst: cmp1,
                                        lhs: s_reg,
                                        rhs: lo_reg,
                                    });
                                    let hi_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(hi_reg, *hi as i32));
                                    let cmp2 = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Le,
                                        dst: cmp2,
                                        lhs: s_reg,
                                        rhs: hi_reg,
                                    });
                                    let matched = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::BitAnd,
                                        dst: matched,
                                        lhs: cmp1,
                                        rhs: cmp2,
                                    });
                                    self.instrs.push(Instr::CondBranch {
                                        reg: matched,
                                        label: next_label,
                                    });
                                }
                                // FLS §5.1.9: Exclusive range pattern `lo..hi`.
                                //
                                // Emit: lo <= scrutinee && scrutinee < hi.
                                // Same as inclusive but uses Lt for the upper bound.
                                Pat::RangeExclusive { lo, hi } => {
                                    let s_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                                    let lo_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(lo_reg, *lo as i32));
                                    let cmp1 = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Ge,
                                        dst: cmp1,
                                        lhs: s_reg,
                                        rhs: lo_reg,
                                    });
                                    let hi_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(hi_reg, *hi as i32));
                                    let cmp2 = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Lt,
                                        dst: cmp2,
                                        lhs: s_reg,
                                        rhs: hi_reg,
                                    });
                                    let matched = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::BitAnd,
                                        dst: matched,
                                        lhs: cmp1,
                                        rhs: cmp2,
                                    });
                                    self.instrs.push(Instr::CondBranch {
                                        reg: matched,
                                        label: next_label,
                                    });
                                }
                                // FLS §5.5 + §15: Path pattern — enum unit variant.
                                // Resolve to discriminant; emit equality check like LitInt.
                                // FLS §6.1.2:37–45: Comparison emits runtime instructions.
                                Pat::Path(segs) => {
                                    let pat_imm = if segs.len() == 2 {
                                        let enum_name = segs[0].text(self.source);
                                        let variant_name = segs[1].text(self.source);
                                        self.enum_defs
                                            .get(enum_name)
                                            .and_then(|v| v.get(variant_name))
                                            .map(|(disc, _)| *disc)
                                            .ok_or_else(|| LowerError::Unsupported(format!(
                                                "unknown enum variant `{enum_name}::{variant_name}` in match pattern"
                                            )))?
                                    } else {
                                        return Err(LowerError::Unsupported(
                                            "path pattern must have exactly two segments".into(),
                                        ));
                                    };
                                    let s_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                                    let pat_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(pat_reg, pat_imm));
                                    let cmp_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Eq,
                                        dst: cmp_reg,
                                        lhs: s_reg,
                                        rhs: pat_reg,
                                    });
                                    self.instrs.push(Instr::CondBranch {
                                        reg: cmp_reg,
                                        label: next_label,
                                    });
                                }
                                // FLS §5.4 + §15: Tuple struct/variant pattern.
                                //
                                // Strategy: compare discriminant at scrut_slot against
                                // the variant's discriminant, branch on mismatch, then
                                // install field bindings from enum_base_slot + 1 + idx.
                                //
                                // FLS §6.1.2:37–45: All instructions are runtime.
                                // Cache-line note: ~5 instructions (ldr + mov + cmp +
                                // cbz + N×ldr per field binding = 20+ bytes).
                                Pat::TupleStruct { path: segs, fields } => {
                                    if segs.len() != 2 {
                                        return Err(LowerError::Unsupported(
                                            "tuple struct pattern path must have two segments".into(),
                                        ));
                                    }
                                    let enum_name = segs[0].text(self.source);
                                    let variant_name = segs[1].text(self.source);
                                    let discriminant = self.enum_defs
                                        .get(enum_name)
                                        .and_then(|v| v.get(variant_name))
                                        .map(|(disc, _)| *disc)
                                        .ok_or_else(|| LowerError::Unsupported(format!(
                                            "unknown enum variant `{enum_name}::{variant_name}`"
                                        )))?;
                                    let base = enum_base_slot.ok_or_else(|| {
                                        LowerError::Unsupported(
                                            "TupleStruct pattern requires enum variable scrutinee".into(),
                                        )
                                    })?;
                                    // Discriminant check.
                                    let s_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                                    let p_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(p_reg, discriminant));
                                    let cmp_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Eq,
                                        dst: cmp_reg,
                                        lhs: s_reg,
                                        rhs: p_reg,
                                    });
                                    self.instrs.push(Instr::CondBranch { reg: cmp_reg, label: next_label });
                                    // Install field bindings.
                                    let mut bound: Vec<&str> = Vec::new();
                                    for (fi, fp) in fields.iter().enumerate() {
                                        if let Pat::Ident(span) = fp {
                                            let fname = span.text(self.source);
                                            let fslot = base + 1 + fi as u8;
                                            let bslot = self.alloc_slot()?;
                                            let breg = self.alloc_reg()?;
                                            self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                            self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                            self.locals.insert(fname, bslot);
                                            bound.push(fname);
                                        } else if !matches!(fp, Pat::Wildcard) {
                                            return Err(LowerError::Unsupported(
                                                "only ident/wildcard field patterns in TupleStruct".into(),
                                            ));
                                        }
                                    }
                                    if let Some(guard) = &arm.guard {
                                        let gv = self.lower_expr(guard, &IrTy::Bool)?;
                                        let gr = self.val_to_reg(gv)?;
                                        self.instrs.push(Instr::CondBranch { reg: gr, label: next_label });
                                    }
                                    let body_val = self.lower_expr(&arm.body, ret_ty)?;
                                    let body_reg = self.val_to_reg(body_val)?;
                                    self.instrs.push(Instr::Store { src: body_reg, slot: phi_slot });
                                    self.instrs.push(Instr::Branch(exit_label));
                                    for name in &bound { self.locals.remove(*name); }
                                    self.instrs.push(Instr::Label(next_label));
                                    continue;
                                }
                                // FLS §5.3 + §15.3: Named-field struct or enum variant pattern
                                // (value-returning match).
                                //
                                // One-segment path (`Point { x, y }`): plain struct pattern —
                                // irrefutable, no discriminant check, fields at base_slot + idx.
                                // Two-segment path (`Enum::Variant { field, … }`): enum variant —
                                // check discriminant, then bind fields at enum_base_slot + 1 + idx.
                                //
                                // FLS §5.3: "A struct pattern matches a struct or enum struct variant
                                // by its field patterns."
                                // FLS §6.1.2:37–45: All instructions are runtime.
                                // Cache-line note: ~5 + 2×N instructions per arm (enum variant);
                                // 2×N instructions per arm (plain struct, no discriminant check).
                                Pat::StructVariant { path: segs, fields: pat_fields } => {
                                    let mut bound_sv: Vec<&str> = Vec::new();
                                    if segs.len() == 1 {
                                        // FLS §5.3: Plain struct pattern — always matches.
                                        let struct_name = segs[0].text(self.source);
                                        let base = struct_base_slot.ok_or_else(|| {
                                            LowerError::Unsupported(
                                                "plain struct pattern requires struct variable scrutinee".into(),
                                            )
                                        })?;
                                        let field_names = self.struct_defs
                                            .get(struct_name)
                                            .cloned()
                                            .ok_or_else(|| LowerError::Unsupported(format!(
                                                "unknown struct `{struct_name}`"
                                            )))?;
                                        // No CondBranch — plain struct patterns are irrefutable.
                                        for (fname_span, fp) in pat_fields.iter() {
                                            let fname = fname_span.text(self.source);
                                            let field_idx = field_names.iter().position(|n| n == fname)
                                                .ok_or_else(|| LowerError::Unsupported(format!(
                                                    "struct `{struct_name}` has no field `{fname}`"
                                                )))?;
                                            match fp {
                                                Pat::Ident(bind_span) => {
                                                    let bind_name = bind_span.text(self.source);
                                                    let fslot = base + field_idx as u8;
                                                    let bslot = self.alloc_slot()?;
                                                    let breg = self.alloc_reg()?;
                                                    self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                                    self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                                    self.locals.insert(bind_name, bslot);
                                                    bound_sv.push(bind_name);
                                                }
                                                Pat::Wildcard => {}
                                                _ => return Err(LowerError::Unsupported(
                                                    "only ident/wildcard sub-patterns in struct patterns".into(),
                                                )),
                                            }
                                        }
                                    } else if segs.len() == 2 {
                                        let enum_name = segs[0].text(self.source);
                                        let variant_name = segs[1].text(self.source);
                                        let (discriminant, field_names) = self.enum_defs
                                            .get(enum_name)
                                            .and_then(|v| v.get(variant_name))
                                            .cloned()
                                            .ok_or_else(|| LowerError::Unsupported(format!(
                                                "unknown enum variant `{enum_name}::{variant_name}`"
                                            )))?;
                                        let base = enum_base_slot.ok_or_else(|| {
                                            LowerError::Unsupported(
                                                "StructVariant pattern requires enum variable scrutinee".into(),
                                            )
                                        })?;
                                        // Discriminant check.
                                        let s_reg = self.alloc_reg()?;
                                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                                        let p_reg = self.alloc_reg()?;
                                        self.instrs.push(Instr::LoadImm(p_reg, discriminant));
                                        let cmp_reg = self.alloc_reg()?;
                                        self.instrs.push(Instr::BinOp {
                                            op: IrBinOp::Eq,
                                            dst: cmp_reg,
                                            lhs: s_reg,
                                            rhs: p_reg,
                                        });
                                        self.instrs.push(Instr::CondBranch { reg: cmp_reg, label: next_label });
                                        // Install named field bindings by declaration order.
                                        for (fname_span, fp) in pat_fields.iter() {
                                            let fname = fname_span.text(self.source);
                                            let field_idx = field_names.iter().position(|n| n == fname)
                                                .ok_or_else(|| LowerError::Unsupported(format!(
                                                    "enum variant `{enum_name}::{variant_name}` has no field `{fname}`"
                                                )))?;
                                            match fp {
                                                Pat::Ident(bind_span) => {
                                                    let bind_name = bind_span.text(self.source);
                                                    let fslot = base + 1 + field_idx as u8;
                                                    let bslot = self.alloc_slot()?;
                                                    let breg = self.alloc_reg()?;
                                                    self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                                    self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                                    self.locals.insert(bind_name, bslot);
                                                    bound_sv.push(bind_name);
                                                }
                                                Pat::Wildcard => {}
                                                _ => return Err(LowerError::Unsupported(
                                                    "only ident/wildcard sub-patterns in StructVariant fields".into(),
                                                )),
                                            }
                                        }
                                    } else {
                                        return Err(LowerError::Unsupported(
                                            "struct/variant pattern path must have 1 or 2 segments".into(),
                                        ));
                                    }
                                    if let Some(guard) = &arm.guard {
                                        let gv = self.lower_expr(guard, &IrTy::Bool)?;
                                        let gr = self.val_to_reg(gv)?;
                                        self.instrs.push(Instr::CondBranch { reg: gr, label: next_label });
                                    }
                                    let body_val = self.lower_expr(&arm.body, ret_ty)?;
                                    let body_reg = self.val_to_reg(body_val)?;
                                    self.instrs.push(Instr::Store { src: body_reg, slot: phi_slot });
                                    self.instrs.push(Instr::Branch(exit_label));
                                    for name in &bound_sv { self.locals.remove(*name); }
                                    self.instrs.push(Instr::Label(next_label));
                                    continue;
                                }
                                _ => {
                                    // Single literal pattern: load scrutinee, compare, cbz.
                                    let s_reg = self.alloc_reg()?;
                                    self.instrs
                                        .push(Instr::Load { dst: s_reg, slot: scrut_slot });
                                    let pat_imm = match &arm.pat {
                                        Pat::LitInt(n) => *n as i32,
                                        // FLS §5.2: Negative literal pattern.
                                        Pat::NegLitInt(n) => -(*n as i32),
                                        Pat::LitBool(b) => *b as i32,
                                        other => return Err(LowerError::Unsupported(format!("unsupported literal pattern kind: {other:?}"))),
                                    };
                                    let pat_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(pat_reg, pat_imm));
                                    // Compare scrutinee == pattern → 1 if equal, 0 if not.
                                    let cmp_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Eq,
                                        dst: cmp_reg,
                                        lhs: s_reg,
                                        rhs: pat_reg,
                                    });
                                    // cbz: skip this arm if condition is 0 (not equal).
                                    self.instrs.push(Instr::CondBranch {
                                        reg: cmp_reg,
                                        label: next_label,
                                    });
                                }
                            }

                            // Guard check (if any): emitted after the pattern check.
                            // The pattern check already branched to next_label on failure;
                            // if we reach here, the pattern matched. Now check the guard.
                            //
                            // FLS §6.18: "A match arm guard is an additional condition
                            // that must hold for the arm to be selected."
                            // FLS §6.1.2:37–45: Guard evaluation emits runtime instructions.
                            // Cache-line note: guard check adds 1 CondBranch instruction (4 bytes).
                            if let Some(guard) = &arm.guard {
                                let gv = self.lower_expr(guard, &IrTy::Bool)?;
                                let gr = self.val_to_reg(gv)?;
                                self.instrs.push(Instr::CondBranch {
                                    reg: gr,
                                    label: next_label,
                                });
                            }

                            // Arm body (reached when pattern matched AND guard passed).
                            let body_val = self.lower_expr(&arm.body, ret_ty)?;
                            let body_reg = self.val_to_reg(body_val)?;
                            self.instrs.push(Instr::Store { src: body_reg, slot: phi_slot });
                            self.instrs.push(Instr::Branch(exit_label));

                            self.instrs.push(Instr::Label(next_label));
                        }

                        // Default arm — unconditional (guard on last arm is not yet supported).
                        // FLS §6.18 AMBIGUOUS: If the last arm has a guard that fails, the
                        // match is non-exhaustive. Galvanic defers this check to a future
                        // exhaustiveness pass.
                        if default_arm[0].guard.is_some() {
                            return Err(LowerError::Unsupported(
                                "guard on last match arm (exhaustiveness deferred; use `_` without guard as last arm)".into(),
                            ));
                        }

                        // FLS §5.1.4: If the default arm has an identifier pattern,
                        // bind the scrutinee to the name before lowering the body.
                        // FLS §5.4 + §15: If the default arm has a TupleStruct pattern,
                        // install field bindings from enum_base_slot.
                        let default_bindings: Vec<&str> = match &default_arm[0].pat {
                            Pat::Ident(span) => {
                                let name = span.text(self.source);
                                let bind_slot = self.alloc_slot()?;
                                let bind_reg = self.alloc_reg()?;
                                self.instrs.push(Instr::Load { dst: bind_reg, slot: scrut_slot });
                                self.instrs.push(Instr::Store { src: bind_reg, slot: bind_slot });
                                self.locals.insert(name, bind_slot);
                                vec![name]
                            }
                            Pat::TupleStruct { path: segs, fields } if segs.len() == 2 => {
                                let base = enum_base_slot.ok_or_else(|| LowerError::Unsupported(
                                    "TupleStruct default arm requires enum variable scrutinee".into(),
                                ))?;
                                let mut names = Vec::new();
                                for (fi, fp) in fields.iter().enumerate() {
                                    if let Pat::Ident(span) = fp {
                                        let fname = span.text(self.source);
                                        let fslot = base + 1 + fi as u8;
                                        let bslot = self.alloc_slot()?;
                                        let breg = self.alloc_reg()?;
                                        self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                        self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                        self.locals.insert(fname, bslot);
                                        names.push(fname);
                                    }
                                }
                                names
                            }
                            // FLS §5.3 + §15.3: Named-field struct or variant default arm
                            // (value-returning match).
                            // One-segment: plain struct pattern (irrefutable, fields at base + idx).
                            // Two-segment: enum variant pattern (fields at enum_base + 1 + idx).
                            Pat::StructVariant { path: segs, fields: pat_fields } => {
                                let mut names: Vec<&str> = Vec::new();
                                if segs.len() == 1 {
                                    let struct_name = segs[0].text(self.source);
                                    let base = struct_base_slot.ok_or_else(|| LowerError::Unsupported(
                                        "plain struct default-arm pattern requires struct variable scrutinee".into(),
                                    ))?;
                                    let field_names = self.struct_defs
                                        .get(struct_name)
                                        .cloned()
                                        .ok_or_else(|| LowerError::Unsupported(format!(
                                            "unknown struct `{struct_name}`"
                                        )))?;
                                    for (fname_span, fp) in pat_fields.iter() {
                                        let fname = fname_span.text(self.source);
                                        if let Pat::Ident(bind_span) = fp {
                                            let bind_name = bind_span.text(self.source);
                                            if let Some(idx) = field_names.iter().position(|n| n == fname) {
                                                let fslot = base + idx as u8;
                                                let bslot = self.alloc_slot()?;
                                                let breg = self.alloc_reg()?;
                                                self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                                self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                                self.locals.insert(bind_name, bslot);
                                                names.push(bind_name);
                                            }
                                        }
                                    }
                                } else if segs.len() == 2 {
                                    let enum_name = segs[0].text(self.source);
                                    let variant_name = segs[1].text(self.source);
                                    let field_names = self.enum_defs
                                        .get(enum_name)
                                        .and_then(|v| v.get(variant_name))
                                        .map(|(_, names)| names.clone())
                                        .unwrap_or_default();
                                    let base = enum_base_slot.ok_or_else(|| LowerError::Unsupported(
                                        "StructVariant default arm requires enum variable scrutinee".into(),
                                    ))?;
                                    for (fname_span, fp) in pat_fields.iter() {
                                        let fname = fname_span.text(self.source);
                                        if let Pat::Ident(bind_span) = fp {
                                            let bind_name = bind_span.text(self.source);
                                            if let Some(idx) = field_names.iter().position(|n| n == fname) {
                                                let fslot = base + 1 + idx as u8;
                                                let bslot = self.alloc_slot()?;
                                                let breg = self.alloc_reg()?;
                                                self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                                self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                                self.locals.insert(bind_name, bslot);
                                                names.push(bind_name);
                                            }
                                        }
                                    }
                                }
                                names
                            }
                            _ => vec![],
                        };
                        let body_val = self.lower_expr(&default_arm[0].body, ret_ty)?;
                        for name in &default_bindings {
                            self.locals.remove(*name);
                        }
                        let body_reg = self.val_to_reg(body_val)?;
                        self.instrs.push(Instr::Store { src: body_reg, slot: phi_slot });

                        self.instrs.push(Instr::Label(exit_label));
                        let result_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: result_reg, slot: phi_slot });
                        Ok(IrValue::Reg(result_reg))
                    }

                    IrTy::Unit => {
                        for arm in checked_arms {
                            let next_label = self.alloc_label();

                            match &arm.pat {
                                Pat::Wildcard => {
                                    // FLS §6.18: Guard check before body execution.
                                    if let Some(guard) = &arm.guard {
                                        let gv = self.lower_expr(guard, &IrTy::Bool)?;
                                        let gr = self.val_to_reg(gv)?;
                                        self.instrs.push(Instr::CondBranch {
                                            reg: gr,
                                            label: next_label,
                                        });
                                    }
                                    self.lower_expr(&arm.body, &IrTy::Unit)?;
                                    self.instrs.push(Instr::Branch(exit_label));
                                    self.instrs.push(Instr::Label(next_label));
                                    continue;
                                }
                                // FLS §5.1.4: Identifier pattern — always matches, binds name.
                                // FLS §6.18: Guard (if present) may reference the bound name.
                                Pat::Ident(span) => {
                                    let name = span.text(self.source);
                                    let bind_slot = self.alloc_slot()?;
                                    let bind_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load { dst: bind_reg, slot: scrut_slot });
                                    self.instrs.push(Instr::Store { src: bind_reg, slot: bind_slot });
                                    self.locals.insert(name, bind_slot);
                                    if let Some(guard) = &arm.guard {
                                        let gv = self.lower_expr(guard, &IrTy::Bool)?;
                                        let gr = self.val_to_reg(gv)?;
                                        self.instrs.push(Instr::CondBranch {
                                            reg: gr,
                                            label: next_label,
                                        });
                                    }
                                    self.lower_expr(&arm.body, &IrTy::Unit)?;
                                    self.locals.remove(name);
                                    self.instrs.push(Instr::Branch(exit_label));
                                    self.instrs.push(Instr::Label(next_label));
                                    continue;
                                }
                                // FLS §5.1.11: OR pattern — same accumulation strategy as
                                // the I32|Bool branch above, but for unit-result arms.
                                Pat::Or(alts) => {
                                    let matched_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(matched_reg, 0));
                                    for alt in alts {
                                        match alt {
                                            Pat::Wildcard => {
                                                self.instrs.push(Instr::LoadImm(matched_reg, 1));
                                                break;
                                            }
                                            Pat::Or(_) => {
                                                return Err(LowerError::Unsupported(
                                                    "nested OR patterns".into(),
                                                ));
                                            }
                                            Pat::Ident(_) => {
                                                return Err(LowerError::Unsupported(
                                                    "identifier pattern inside OR pattern".into(),
                                                ));
                                            }
                                            _ => {
                                                let alt_imm = self.pat_scalar_imm(alt)?;
                                                let si_reg = self.alloc_reg()?;
                                                self.instrs.push(Instr::Load {
                                                    dst: si_reg,
                                                    slot: scrut_slot,
                                                });
                                                let alt_reg = self.alloc_reg()?;
                                                self.instrs.push(Instr::LoadImm(alt_reg, alt_imm));
                                                let eq_reg = self.alloc_reg()?;
                                                self.instrs.push(Instr::BinOp {
                                                    op: IrBinOp::Eq,
                                                    dst: eq_reg,
                                                    lhs: si_reg,
                                                    rhs: alt_reg,
                                                });
                                                self.instrs.push(Instr::BinOp {
                                                    op: IrBinOp::BitOr,
                                                    dst: matched_reg,
                                                    lhs: matched_reg,
                                                    rhs: eq_reg,
                                                });
                                            }
                                        }
                                    }
                                    self.instrs.push(Instr::CondBranch {
                                        reg: matched_reg,
                                        label: next_label,
                                    });
                                }
                                // FLS §5.1.9: Inclusive range pattern `lo..=hi` (unit branch).
                                Pat::RangeInclusive { lo, hi } => {
                                    let s_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                                    let lo_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(lo_reg, *lo as i32));
                                    let cmp1 = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Ge,
                                        dst: cmp1,
                                        lhs: s_reg,
                                        rhs: lo_reg,
                                    });
                                    let hi_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(hi_reg, *hi as i32));
                                    let cmp2 = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Le,
                                        dst: cmp2,
                                        lhs: s_reg,
                                        rhs: hi_reg,
                                    });
                                    let matched = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::BitAnd,
                                        dst: matched,
                                        lhs: cmp1,
                                        rhs: cmp2,
                                    });
                                    self.instrs.push(Instr::CondBranch {
                                        reg: matched,
                                        label: next_label,
                                    });
                                }
                                // FLS §5.1.9: Exclusive range pattern `lo..hi` (unit branch).
                                Pat::RangeExclusive { lo, hi } => {
                                    let s_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                                    let lo_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(lo_reg, *lo as i32));
                                    let cmp1 = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Ge,
                                        dst: cmp1,
                                        lhs: s_reg,
                                        rhs: lo_reg,
                                    });
                                    let hi_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(hi_reg, *hi as i32));
                                    let cmp2 = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Lt,
                                        dst: cmp2,
                                        lhs: s_reg,
                                        rhs: hi_reg,
                                    });
                                    let matched = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::BitAnd,
                                        dst: matched,
                                        lhs: cmp1,
                                        rhs: cmp2,
                                    });
                                    self.instrs.push(Instr::CondBranch {
                                        reg: matched,
                                        label: next_label,
                                    });
                                }
                                // FLS §5.5 + §15: Path pattern in unit-return match arm.
                                Pat::Path(segs) => {
                                    let pat_imm = if segs.len() == 2 {
                                        let enum_name = segs[0].text(self.source);
                                        let variant_name = segs[1].text(self.source);
                                        self.enum_defs
                                            .get(enum_name)
                                            .and_then(|v| v.get(variant_name))
                                            .map(|(disc, _)| *disc)
                                            .ok_or_else(|| LowerError::Unsupported(format!(
                                                "unknown enum variant `{enum_name}::{variant_name}` in match pattern"
                                            )))?
                                    } else {
                                        return Err(LowerError::Unsupported(
                                            "path pattern must have exactly two segments".into(),
                                        ));
                                    };
                                    let s_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                                    let pat_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(pat_reg, pat_imm));
                                    let cmp_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Eq,
                                        dst: cmp_reg,
                                        lhs: s_reg,
                                        rhs: pat_reg,
                                    });
                                    self.instrs.push(Instr::CondBranch {
                                        reg: cmp_reg,
                                        label: next_label,
                                    });
                                }
                                // FLS §5.4 + §15: Tuple struct/variant pattern (unit return).
                                Pat::TupleStruct { path: segs, fields } => {
                                    if segs.len() != 2 {
                                        return Err(LowerError::Unsupported(
                                            "tuple struct pattern path must have two segments".into(),
                                        ));
                                    }
                                    let enum_name = segs[0].text(self.source);
                                    let variant_name = segs[1].text(self.source);
                                    let discriminant = self.enum_defs
                                        .get(enum_name)
                                        .and_then(|v| v.get(variant_name))
                                        .map(|(disc, _)| *disc)
                                        .ok_or_else(|| LowerError::Unsupported(format!(
                                            "unknown enum variant `{enum_name}::{variant_name}`"
                                        )))?;
                                    let base = enum_base_slot.ok_or_else(|| LowerError::Unsupported(
                                        "TupleStruct pattern requires enum variable scrutinee".into(),
                                    ))?;
                                    let s_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                                    let p_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(p_reg, discriminant));
                                    let cmp_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Eq,
                                        dst: cmp_reg,
                                        lhs: s_reg,
                                        rhs: p_reg,
                                    });
                                    self.instrs.push(Instr::CondBranch { reg: cmp_reg, label: next_label });
                                    // Install field bindings.
                                    let mut bound: Vec<&str> = Vec::new();
                                    for (fi, fp) in fields.iter().enumerate() {
                                        if let Pat::Ident(span) = fp {
                                            let fname = span.text(self.source);
                                            let fslot = base + 1 + fi as u8;
                                            let bslot = self.alloc_slot()?;
                                            let breg = self.alloc_reg()?;
                                            self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                            self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                            self.locals.insert(fname, bslot);
                                            bound.push(fname);
                                        } else if !matches!(fp, Pat::Wildcard) {
                                            return Err(LowerError::Unsupported(
                                                "only ident/wildcard fields in TupleStruct pattern".into(),
                                            ));
                                        }
                                    }
                                    if let Some(guard) = &arm.guard {
                                        let gv = self.lower_expr(guard, &IrTy::Bool)?;
                                        let gr = self.val_to_reg(gv)?;
                                        self.instrs.push(Instr::CondBranch { reg: gr, label: next_label });
                                    }
                                    self.lower_expr(&arm.body, &IrTy::Unit)?;
                                    self.instrs.push(Instr::Branch(exit_label));
                                    for name in &bound { self.locals.remove(*name); }
                                    self.instrs.push(Instr::Label(next_label));
                                    continue;
                                }
                                // FLS §5.3 + §15.3: Named-field struct or variant pattern
                                // (unit-return match). Same 1/2-segment dispatch as I32 branch.
                                Pat::StructVariant { path: segs, fields: pat_fields } => {
                                    let mut bound_sv: Vec<&str> = Vec::new();
                                    if segs.len() == 1 {
                                        // FLS §5.3: Plain struct pattern — irrefutable.
                                        let struct_name = segs[0].text(self.source);
                                        let base = struct_base_slot.ok_or_else(|| LowerError::Unsupported(
                                            "plain struct pattern requires struct variable scrutinee".into(),
                                        ))?;
                                        let field_names = self.struct_defs
                                            .get(struct_name)
                                            .cloned()
                                            .ok_or_else(|| LowerError::Unsupported(format!(
                                                "unknown struct `{struct_name}`"
                                            )))?;
                                        for (fname_span, fp) in pat_fields.iter() {
                                            let fname = fname_span.text(self.source);
                                            let field_idx = field_names.iter().position(|n| n == fname)
                                                .ok_or_else(|| LowerError::Unsupported(format!(
                                                    "struct `{struct_name}` has no field `{fname}`"
                                                )))?;
                                            match fp {
                                                Pat::Ident(bind_span) => {
                                                    let bind_name = bind_span.text(self.source);
                                                    let fslot = base + field_idx as u8;
                                                    let bslot = self.alloc_slot()?;
                                                    let breg = self.alloc_reg()?;
                                                    self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                                    self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                                    self.locals.insert(bind_name, bslot);
                                                    bound_sv.push(bind_name);
                                                }
                                                Pat::Wildcard => {}
                                                _ => return Err(LowerError::Unsupported(
                                                    "only ident/wildcard sub-patterns in struct patterns".into(),
                                                )),
                                            }
                                        }
                                    } else if segs.len() == 2 {
                                        let enum_name = segs[0].text(self.source);
                                        let variant_name = segs[1].text(self.source);
                                        let (discriminant, field_names) = self.enum_defs
                                            .get(enum_name)
                                            .and_then(|v| v.get(variant_name))
                                            .cloned()
                                            .ok_or_else(|| LowerError::Unsupported(format!(
                                                "unknown enum variant `{enum_name}::{variant_name}`"
                                            )))?;
                                        let base = enum_base_slot.ok_or_else(|| LowerError::Unsupported(
                                            "StructVariant pattern requires enum variable scrutinee".into(),
                                        ))?;
                                        let s_reg = self.alloc_reg()?;
                                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                                        let p_reg = self.alloc_reg()?;
                                        self.instrs.push(Instr::LoadImm(p_reg, discriminant));
                                        let cmp_reg = self.alloc_reg()?;
                                        self.instrs.push(Instr::BinOp {
                                            op: IrBinOp::Eq,
                                            dst: cmp_reg,
                                            lhs: s_reg,
                                            rhs: p_reg,
                                        });
                                        self.instrs.push(Instr::CondBranch { reg: cmp_reg, label: next_label });
                                        for (fname_span, fp) in pat_fields.iter() {
                                            let fname = fname_span.text(self.source);
                                            let field_idx = field_names.iter().position(|n| n == fname)
                                                .ok_or_else(|| LowerError::Unsupported(format!(
                                                    "enum variant `{enum_name}::{variant_name}` has no field `{fname}`"
                                                )))?;
                                            match fp {
                                                Pat::Ident(bind_span) => {
                                                    let bind_name = bind_span.text(self.source);
                                                    let fslot = base + 1 + field_idx as u8;
                                                    let bslot = self.alloc_slot()?;
                                                    let breg = self.alloc_reg()?;
                                                    self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                                    self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                                    self.locals.insert(bind_name, bslot);
                                                    bound_sv.push(bind_name);
                                                }
                                                Pat::Wildcard => {}
                                                _ => return Err(LowerError::Unsupported(
                                                    "only ident/wildcard sub-patterns in StructVariant fields".into(),
                                                )),
                                            }
                                        }
                                    } else {
                                        return Err(LowerError::Unsupported(
                                            "struct/variant pattern path must have 1 or 2 segments".into(),
                                        ));
                                    }
                                    if let Some(guard) = &arm.guard {
                                        let gv = self.lower_expr(guard, &IrTy::Bool)?;
                                        let gr = self.val_to_reg(gv)?;
                                        self.instrs.push(Instr::CondBranch { reg: gr, label: next_label });
                                    }
                                    self.lower_expr(&arm.body, &IrTy::Unit)?;
                                    self.instrs.push(Instr::Branch(exit_label));
                                    for name in &bound_sv { self.locals.remove(*name); }
                                    self.instrs.push(Instr::Label(next_label));
                                    continue;
                                }
                                _ => {
                                    let s_reg = self.alloc_reg()?;
                                    self.instrs
                                        .push(Instr::Load { dst: s_reg, slot: scrut_slot });
                                    let pat_imm = match &arm.pat {
                                        Pat::LitInt(n) => *n as i32,
                                        Pat::NegLitInt(n) => -(*n as i32),
                                        Pat::LitBool(b) => *b as i32,
                                        other => return Err(LowerError::Unsupported(format!("unsupported literal pattern kind: {other:?}"))),
                                    };
                                    let pat_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(pat_reg, pat_imm));
                                    let cmp_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Eq,
                                        dst: cmp_reg,
                                        lhs: s_reg,
                                        rhs: pat_reg,
                                    });
                                    self.instrs.push(Instr::CondBranch {
                                        reg: cmp_reg,
                                        label: next_label,
                                    });
                                }
                            }

                            // Guard check for fall-through patterns (unit result).
                            // FLS §6.18: Guard evaluated after pattern matches.
                            if let Some(guard) = &arm.guard {
                                let gv = self.lower_expr(guard, &IrTy::Bool)?;
                                let gr = self.val_to_reg(gv)?;
                                self.instrs.push(Instr::CondBranch {
                                    reg: gr,
                                    label: next_label,
                                });
                            }

                            self.lower_expr(&arm.body, &IrTy::Unit)?;
                            self.instrs.push(Instr::Branch(exit_label));
                            self.instrs.push(Instr::Label(next_label));
                        }

                        // Default arm — unconditional (guard on last arm not yet supported).
                        if default_arm[0].guard.is_some() {
                            return Err(LowerError::Unsupported(
                                "guard on last match arm (exhaustiveness deferred)".into(),
                            ));
                        }

                        // FLS §5.1.4: If the default arm has an identifier pattern, bind.
                        // FLS §5.4 + §15: TupleStruct default arm installs field bindings.
                        let default_bindings_unit: Vec<&str> = match &default_arm[0].pat {
                            Pat::Ident(span) => {
                                let name = span.text(self.source);
                                let bind_slot = self.alloc_slot()?;
                                let bind_reg = self.alloc_reg()?;
                                self.instrs.push(Instr::Load { dst: bind_reg, slot: scrut_slot });
                                self.instrs.push(Instr::Store { src: bind_reg, slot: bind_slot });
                                self.locals.insert(name, bind_slot);
                                vec![name]
                            }
                            Pat::TupleStruct { path: segs, fields } if segs.len() == 2 => {
                                let base = enum_base_slot.ok_or_else(|| LowerError::Unsupported(
                                    "TupleStruct default arm requires enum variable scrutinee".into(),
                                ))?;
                                let mut names = Vec::new();
                                for (fi, fp) in fields.iter().enumerate() {
                                    if let Pat::Ident(span) = fp {
                                        let fname = span.text(self.source);
                                        let fslot = base + 1 + fi as u8;
                                        let bslot = self.alloc_slot()?;
                                        let breg = self.alloc_reg()?;
                                        self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                        self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                        self.locals.insert(fname, bslot);
                                        names.push(fname);
                                    }
                                }
                                names
                            }
                            // FLS §5.3 + §15.3: Named-field struct or variant default arm
                            // (unit-return match). 1-segment: plain struct; 2-segment: enum variant.
                            Pat::StructVariant { path: segs, fields: pat_fields } => {
                                let mut names: Vec<&str> = Vec::new();
                                if segs.len() == 1 {
                                    let struct_name = segs[0].text(self.source);
                                    let base = struct_base_slot.ok_or_else(|| LowerError::Unsupported(
                                        "plain struct default-arm pattern requires struct variable scrutinee".into(),
                                    ))?;
                                    let field_names = self.struct_defs
                                        .get(struct_name)
                                        .cloned()
                                        .ok_or_else(|| LowerError::Unsupported(format!(
                                            "unknown struct `{struct_name}`"
                                        )))?;
                                    for (fname_span, fp) in pat_fields.iter() {
                                        let fname = fname_span.text(self.source);
                                        if let Pat::Ident(bind_span) = fp {
                                            let bind_name = bind_span.text(self.source);
                                            if let Some(idx) = field_names.iter().position(|n| n == fname) {
                                                let fslot = base + idx as u8;
                                                let bslot = self.alloc_slot()?;
                                                let breg = self.alloc_reg()?;
                                                self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                                self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                                self.locals.insert(bind_name, bslot);
                                                names.push(bind_name);
                                            }
                                        }
                                    }
                                } else if segs.len() == 2 {
                                    let enum_name = segs[0].text(self.source);
                                    let variant_name = segs[1].text(self.source);
                                    let field_names = self.enum_defs
                                        .get(enum_name)
                                        .and_then(|v| v.get(variant_name))
                                        .map(|(_, names)| names.clone())
                                        .unwrap_or_default();
                                    let base = enum_base_slot.ok_or_else(|| LowerError::Unsupported(
                                        "StructVariant default arm requires enum variable scrutinee".into(),
                                    ))?;
                                    for (fname_span, fp) in pat_fields.iter() {
                                        let fname = fname_span.text(self.source);
                                        if let Pat::Ident(bind_span) = fp {
                                            let bind_name = bind_span.text(self.source);
                                            if let Some(idx) = field_names.iter().position(|n| n == fname) {
                                                let fslot = base + 1 + idx as u8;
                                                let bslot = self.alloc_slot()?;
                                                let breg = self.alloc_reg()?;
                                                self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                                self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                                self.locals.insert(bind_name, bslot);
                                                names.push(bind_name);
                                            }
                                        }
                                    }
                                }
                                names
                            }
                            _ => vec![],
                        };
                        self.lower_expr(&default_arm[0].body, &IrTy::Unit)?;
                        for name in &default_bindings_unit {
                            self.locals.remove(*name);
                        }
                        self.instrs.push(Instr::Label(exit_label));
                        Ok(IrValue::Unit)
                    }
                }
            }

            // FLS §6.12.1: Call expression — lower each argument, then emit
            // `Instr::Call`. The callee must be a simple path (function name).
            //
            // ARM64 ABI: arguments go into x0–x{n-1}. If the argument's current
            // virtual register happens to already be register i (the required
            // ARM64 slot for argument i), no move is needed — this is tracked
            // in the `args` field of `Instr::Call` and resolved in codegen.
            //
            // FLS §6.4:14: Arguments are evaluated left-to-right before the
            // call. The sequential lowering loop preserves this order because
            // `lower_expr` for each argument emits its instructions before
            // moving on to the next.
            //
            // Limitation: only direct (named) calls are supported; function
            // pointers and method calls are deferred to a future milestone.
            ExprKind::Call { callee, args } => {
                // Resolve the callee to a function name.
                // FLS §15: Two-segment path callees are enum tuple variant constructors.
                // As standalone expressions they are only supported inside let-binding
                // initializers (handled in lower_stmt). Other contexts are future work.
                let fn_name = match &callee.kind {
                    ExprKind::Path(segments) if segments.len() == 1 => {
                        segments[0].text(self.source).to_owned()
                    }
                    ExprKind::Path(segments) if segments.len() == 2 => {
                        let tn = segments[0].text(self.source);
                        let fn_seg = segments[1].text(self.source);
                        // Check for enum variant constructor first.
                        if self.enum_defs.get(tn).and_then(|v| v.get(fn_seg)).is_some() {
                            return Err(LowerError::Unsupported(format!(
                                "enum variant `{tn}::{fn_seg}(...)` as expression; use in `let` binding"
                            )));
                        }
                        let mangled = format!("{tn}__{fn_seg}");
                        // Struct-returning associated functions cannot be used as scalar
                        // expressions — the caller needs a destination slot.
                        if self.struct_return_fns.contains_key(&mangled) {
                            return Err(LowerError::Unsupported(format!(
                                "`{tn}::{fn_seg}` returns a struct; use it in a `let` binding"
                            )));
                        }
                        // FLS §10.1: Scalar-returning associated function call.
                        // Resolve to the mangled name `TypeName__fn_name`.
                        mangled
                    }
                    _ => {
                        return Err(LowerError::Unsupported(
                            "call expression with non-path callee (function pointers not yet supported)".into(),
                        ));
                    }
                };

                if args.len() > 8 {
                    return Err(LowerError::Unsupported(
                        "call with more than 8 arguments (exceeds ARM64 register window)".into(),
                    ));
                }

                // Lower each argument to virtual registers, left-to-right.
                //
                // FLS §11 / §6.12.2: If an argument is a struct variable
                // (recorded in `local_struct_types`), it expands to N registers:
                // one per field in declaration order. This matches the struct
                // parameter calling convention in `lower_fn`.
                //
                // FLS §15: If an argument is an enum variable (recorded in
                // `local_enum_types`), it expands to multiple registers:
                // discriminant in the first, fields in subsequent registers.
                // This matches the enum parameter calling convention in `lower_fn`.
                //
                // FLS §6.1.2:37–45: All loads are runtime instructions.
                let mut arg_regs = Vec::with_capacity(args.len());
                for arg in args {
                    // Check whether this argument is a struct variable.
                    //
                    // For nested structs, use the total slot count from `struct_sizes`
                    // (e.g., Rect with two Point fields has 4 total slots, not 2).
                    // This ensures the correct number of registers are loaded and
                    // passed to the callee, matching the parameter spill in `lower_fn`.
                    //
                    // FLS §4.11: Nested struct fields occupy consecutive slots in
                    // declaration order of the outermost struct.
                    // FLS §6.1.2:37–45: All loads are runtime instructions.
                    let struct_info: Option<(u8, usize)> = if let ExprKind::Path(segs) = &arg.kind
                        && segs.len() == 1
                    {
                        let var_name = segs[0].text(self.source);
                        if let Some(&base_slot) = self.locals.get(var_name) {
                            if let Some(st_name) = self.local_struct_types.get(&base_slot) {
                                // Use struct_sizes for total slot count (handles nested structs).
                                let n_slots = self.struct_sizes
                                    .get(st_name.as_str())
                                    .copied()
                                    .unwrap_or_else(|| {
                                        self.struct_defs
                                            .get(st_name.as_str())
                                            .map(|f| f.len())
                                            .unwrap_or(0)
                                    });
                                Some((base_slot, n_slots))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    if let Some((base_slot, n_fields)) = struct_info {
                        // Pass each struct field as a separate register.
                        // FLS §11: field 0 → x{i}, field 1 → x{i+1}, etc.
                        for fi in 0..n_fields {
                            let field_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::Load {
                                dst: field_reg,
                                slot: base_slot + fi as u8,
                            });
                            arg_regs.push(field_reg);
                        }
                        // Unit struct: no registers to pass; continue.
                        continue;
                    }

                    // Check whether this argument is an enum variable.
                    let enum_info: Option<(u8, usize)> = if let ExprKind::Path(segs) = &arg.kind
                        && segs.len() == 1
                    {
                        let var_name = segs[0].text(self.source);
                        let base_slot_opt = self.locals.get(var_name).copied();
                        if let Some(base_slot) = base_slot_opt {
                            if let Some(en) = self.local_enum_types.get(&base_slot) {
                                let en_name = en.clone();
                                let max_fields = self.enum_defs
                                    .get(en_name.as_str())
                                    .map(|v| v.values().map(|(_, names)| names.len()).max().unwrap_or(0))
                                    .unwrap_or(0);
                                Some((base_slot, max_fields))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    if let Some((base_slot, max_fields)) = enum_info {
                        // Pass discriminant register.
                        let disc_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: disc_reg, slot: base_slot });
                        arg_regs.push(disc_reg);
                        // Pass field registers (uninitialized for unit variants, but
                        // discriminant check in callee prevents any use of them).
                        for fi in 0..max_fields {
                            let field_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::Load {
                                dst: field_reg,
                                slot: base_slot + 1 + fi as u8,
                            });
                            arg_regs.push(field_reg);
                        }
                    } else if let ExprKind::Call { callee, args: variant_args } = &arg.kind
                        && let ExprKind::Path(segs) = &callee.kind
                        && segs.len() == 2
                    {
                        // FLS §15: Enum tuple variant constructor used directly as
                        // a function argument — e.g., `compute(Shape::Circle(7))`.
                        //
                        // Inline-construct the enum value into registers without
                        // allocating a named variable. Emit the discriminant as an
                        // immediate, then each field expression, then zero-pad to
                        // the enum's max_fields count so the callee's slot layout
                        // matches regardless of which variant is passed.
                        //
                        // FLS §6.1.2:37–45: All instructions emitted at runtime.
                        let tn = segs[0].text(self.source);
                        let vn = segs[1].text(self.source);
                        if let Some((discriminant, field_names)) = self.enum_defs
                            .get(tn)
                            .and_then(|v| v.get(vn))
                            .cloned()
                        {
                            let field_count = field_names.len();
                            let max_fields_enum = self.enum_defs
                                .get(tn)
                                .map(|v| v.values().map(|(_, n)| n.len()).max().unwrap_or(0))
                                .unwrap_or(0);
                            // Discriminant register.
                            let disc_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::LoadImm(disc_reg, discriminant));
                            arg_regs.push(disc_reg);
                            // Field registers.
                            for variant_arg in variant_args.iter() {
                                let val = self.lower_expr(variant_arg, &IrTy::I32)?;
                                let reg = self.val_to_reg(val)?;
                                arg_regs.push(reg);
                            }
                            // Padding for unused field slots.
                            for _ in field_count..max_fields_enum {
                                let pad_reg = self.alloc_reg()?;
                                self.instrs.push(Instr::LoadImm(pad_reg, 0));
                                arg_regs.push(pad_reg);
                            }
                        } else {
                            let val = self.lower_expr(arg, &IrTy::I32)?;
                            let reg = self.val_to_reg(val)?;
                            arg_regs.push(reg);
                        }
                    } else if let ExprKind::StructLit {
                        name: struct_name_span,
                        fields: lit_fields,
                        ..
                    } = &arg.kind
                    {
                        // FLS §6.11 + §11: Struct literal used directly as a function
                        // argument — e.g., `sum(Point { x: 3, y: 4 })`.
                        //
                        // Inline-evaluate each field expression in declaration order and
                        // pass the results as separate registers. This mirrors the struct
                        // parameter calling convention set up in `lower_fn` (field 0 →
                        // x{i}, field 1 → x{i+1}, …) without allocating a named slot.
                        //
                        // FLS §6.11: Field initializers may appear in any source order;
                        // galvanic stores and passes them in struct declaration order for
                        // layout stability.
                        // FLS §6.1.2:37–45: All field evaluations emit runtime instructions.
                        // Cache-line note: N field registers = N × 4-byte `mov` or load
                        // instructions, one per field.
                        let struct_name = struct_name_span.text(self.source);
                        let field_names = self.struct_defs
                            .get(struct_name)
                            .cloned()
                            .ok_or_else(|| LowerError::Unsupported(format!(
                                "unknown struct type `{struct_name}` in function argument"
                            )))?;
                        for field_name in &field_names {
                            let (_, field_val_expr) = lit_fields
                                .iter()
                                .find(|(f, _)| f.text(self.source) == field_name.as_str())
                                .ok_or_else(|| LowerError::Unsupported(format!(
                                    "missing field `{field_name}` in `{struct_name}` literal argument"
                                )))?;
                            let val = self.lower_expr(field_val_expr, &IrTy::I32)?;
                            let reg = self.val_to_reg(val)?;
                            arg_regs.push(reg);
                        }
                    } else {
                        let val = self.lower_expr(arg, &IrTy::I32)?;
                        let reg = self.val_to_reg(val)?;
                        arg_regs.push(reg);
                    }
                }

                // Allocate the destination register for the return value.
                let dst = self.alloc_reg()?;

                self.has_calls = true;
                self.instrs.push(Instr::Call { dst, name: fn_name, args: arg_regs });

                Ok(IrValue::Reg(dst))
            }

            // FLS §6.5.10: Assignment expression `place = value`.
            //
            // The LHS must be a local variable path (a place expression). The
            // RHS is evaluated at runtime and stored to the variable's stack
            // slot, updating its value in place for subsequent reads.
            //
            // FLS §6.5.10: "The type of an assignment expression is the unit
            // type ()."
            // FLS §6.1.2:37–45: The store is a runtime instruction; no
            // compile-time constant folding of the RHS is permitted.
            // FLS §14.1 AMBIGUOUS: The spec does not enumerate valid place
            // expressions for assignment; we restrict to simple variable paths.
            //
            // Cache-line note: the emitted `str` is 4 bytes — same footprint
            // as the `str` emitted by a let-binding initializer.
            ExprKind::Binary { op: BinOp::Assign, lhs, rhs } => {
                // Resolve the LHS to a stack slot (must be a declared local or field).
                //
                // FLS §6.5.10: The left operand must be a place expression. Galvanic
                // supports two place expression forms here:
                //   1. A simple variable path (e.g., `x = value`)
                //   2. A struct field access (e.g., `s.field = value`) — FLS §6.13
                //
                // FLS §6.13: Field access on a struct variable resolves to the slot
                // at `base_slot + field_index`, where the layout mirrors struct literal
                // construction (field 0 at base_slot, field 1 at base_slot+1, etc.).
                //
                // Cache-line note: field store emits one `str` instruction (4 bytes),
                // identical in cost to a plain variable store.
                let slot = match &lhs.kind {
                    ExprKind::Path(segments) if segments.len() == 1 => {
                        let var_name = segments[0].text(self.source);
                        self.locals.get(var_name).copied().ok_or_else(|| {
                            LowerError::Unsupported(format!(
                                "assignment to undefined variable `{var_name}`"
                            ))
                        })?
                    }
                    ExprKind::FieldAccess { .. } => {
                        // FLS §6.5.10: The left operand must be a place expression.
                        // FLS §6.13: Struct field assignment — resolve using the same
                        // `resolve_place` helper as field read access, which supports
                        // chained access (`r.b.x = value`) via recursive slot computation.
                        //
                        // Cache-line note: field store emits one `str` instruction (4 bytes),
                        // identical in cost to a plain variable store.
                        let (slot, _) = self.resolve_place(lhs)?;
                        slot
                    }
                    // FLS §6.5.10: Assignment to an indexed place expression `arr[index] = value`.
                    // FLS §6.9: The base must be a known array variable; the index is a runtime value.
                    //
                    // Lowering strategy:
                    // 1. Resolve base to its array stack slot.
                    // 2. Lower index to a virtual register.
                    // 3. Lower RHS to a virtual register.
                    // 4. Allocate a scratch register for base address computation.
                    // 5. Emit `StoreIndexed { src, base_slot, index_reg, scratch }`.
                    //
                    // Evaluation order: FLS §6.5.10 evaluates the place (base, index)
                    // before the value, but since both are side-effect-free in the
                    // current subset, we lower RHS first to keep register allocation
                    // linear (index_reg < scratch so scratch doesn't alias either).
                    //
                    // FLS §14.1 AMBIGUOUS: The spec does not enumerate which place
                    // expressions are valid LHS for assignment. We restrict the base
                    // to a simple variable path (consistent with plain assignment).
                    ExprKind::Index { base, index } => {
                        // Resolve base array variable to its stack slot.
                        let base_slot = match &base.kind {
                            ExprKind::Path(segs) if segs.len() == 1 => {
                                let var_name = segs[0].text(self.source);
                                let slot =
                                    self.locals.get(var_name).copied().ok_or_else(|| {
                                        LowerError::Unsupported(format!(
                                            "undefined variable `{var_name}` in index assignment"
                                        ))
                                    })?;
                                if !self.local_array_lens.contains_key(&slot) {
                                    return Err(LowerError::Unsupported(format!(
                                        "variable `{var_name}` is not an array (index assignment on non-arrays not supported)"
                                    )));
                                }
                                slot
                            }
                            _ => {
                                return Err(LowerError::Unsupported(
                                    "index assignment on non-variable base not yet supported"
                                        .into(),
                                ));
                            }
                        };

                        // Lower the RHS (value to store).
                        let rhs_val = self.lower_expr(rhs, &IrTy::I32)?;
                        let src_reg = self.val_to_reg(rhs_val)?;

                        // Lower the index.
                        let idx_val = self.lower_expr(index, &IrTy::I32)?;
                        let index_reg = self.val_to_reg(idx_val)?;

                        // Allocate scratch for base-address computation.
                        let scratch = self.alloc_reg()?;

                        self.instrs.push(Instr::StoreIndexed {
                            src: src_reg,
                            base_slot,
                            index_reg,
                            scratch,
                        });

                        // FLS §6.5.10: assignment expressions have type `()`.
                        return Ok(IrValue::Unit);
                    }
                    // FLS §6.5.10: Assignment through a mutable reference `*ref_var = value`.
                    //
                    // When the LHS is a dereference expression, the operand must resolve to
                    // a register holding a pointer (an `&mut T` reference). The value is
                    // stored through that pointer using `StorePtr`.
                    //
                    // Lowering strategy:
                    // 1. Lower the pointer operand to a register (e.g., the `&mut` parameter).
                    // 2. Lower the RHS value to a register.
                    // 3. Emit `StorePtr { src: rhs_reg, addr: ptr_reg }`.
                    //
                    // FLS §6.1.2:37–45: Both the pointer computation and the store are
                    // runtime instructions; no compile-time evaluation is permitted.
                    //
                    // Cache-line note: one `str` instruction (4 bytes) — identical footprint
                    // to a plain variable store (`str xS, [sp, #offset]`).
                    ExprKind::Unary {
                        op: crate::ast::UnaryOp::Deref,
                        operand,
                    } => {
                        // Lower the pointer (e.g., the &mut parameter).
                        let ptr_val = self.lower_expr(operand, &IrTy::I32)?;
                        let ptr_reg = self.val_to_reg(ptr_val)?;

                        // Lower the RHS.
                        let rhs_val = self.lower_expr(rhs, &IrTy::I32)?;
                        let src_reg = self.val_to_reg(rhs_val)?;

                        self.instrs.push(Instr::StorePtr { src: src_reg, addr: ptr_reg });

                        // FLS §6.5.10: assignment has type `()`.
                        return Ok(IrValue::Unit);
                    }
                    _ => {
                        return Err(LowerError::Unsupported(
                            "assignment to non-variable place expression not yet supported".into(),
                        ));
                    }
                };

                // FLS §9, §15: If the LHS is an enum variable and the RHS is a call
                // to an enum-returning free function, use CallMut-style write-back
                // to populate the enum variable's discriminant + field slots.
                if self.local_enum_types.contains_key(&slot)
                    && let ExprKind::Call { callee, args } = &rhs.kind
                    && let ExprKind::Path(segs) = &callee.kind
                    && segs.len() == 1
                {
                    let fn_name = segs[0].text(self.source);
                    if let Some(enum_name) = self.enum_return_fns.get(fn_name).cloned() {
                        let max_fields = self.enum_defs
                            .get(enum_name.as_str())
                            .map(|v| v.values().map(|(_, names)| names.len()).max().unwrap_or(0))
                            .unwrap_or(0);
                        let n_ret = 1 + max_fields as u8;
                        // Lower arguments.
                        let mut arg_regs = Vec::with_capacity(args.len());
                        for arg in args.iter() {
                            let val = self.lower_expr(arg, &IrTy::I32)?;
                            let reg = self.val_to_reg(val)?;
                            arg_regs.push(reg);
                        }
                        self.has_calls = true;
                        // CallMut: bl fn_name; str x0..[sp+slot*8..]
                        self.instrs.push(Instr::CallMut {
                            name: fn_name.to_owned(),
                            args: arg_regs,
                            write_back_slot: slot,
                            n_fields: n_ret,
                        });
                        return Ok(IrValue::Unit);
                    }
                }

                // Lower RHS as i32 — all current locals are i32.
                // FLS §8.1 AMBIGUOUS: The spec does not describe how type
                // inference constrains the RHS type at the assignment site
                // when no annotation is present. We assume i32 to match the
                // existing let-binding convention.
                let rhs_val = self.lower_expr(rhs, &IrTy::I32)?;
                let src = self.val_to_reg(rhs_val)?;
                self.instrs.push(Instr::Store { src, slot });

                // FLS §6.5.10: assignment expressions have type `()`.
                Ok(IrValue::Unit)
            }

            // FLS §6.5.11: Compound assignment expression `target op= value`.
            //
            // Desugars at the IR level to: load target, binop, store target.
            // No new IR instructions needed — reuses Load, BinOp, Store.
            //
            // FLS §6.5.11: "The type of a compound assignment expression is the unit type ()."
            // FLS §6.1.2:37–45: The load, binop, and store must all emit runtime instructions —
            // even `x += 1` where x and 1 are statically known must emit ldr + add + str.
            //
            // FLS §14.1 AMBIGUOUS: The spec does not enumerate which place expressions are
            // valid on the left-hand side of compound assignment. We restrict to simple
            // variable paths, consistent with the restriction on plain assignment.
            //
            // Cache-line note: emits 3 instructions (ldr + binop + str) = 12 bytes.
            // For a simple add, this is three sequential 4-byte instructions that will
            // often land in the same 64-byte cache line.
            ExprKind::CompoundAssign { op, target, value } => {
                // Handle compound assignment through a pointer: `*ptr op= value`.
                //
                // FLS §6.5.11: The LHS of compound assignment may be a dereference
                // expression `*ptr`. Desugars to LoadPtr + BinOp + StorePtr.
                //
                // FLS §6.5.10: `*ptr = value` writes through the pointer; compound
                // assignment adds a LoadPtr before the store.
                //
                // FLS §6.1.2:37–45: All three instructions are runtime — no folding.
                //
                // Cache-line note: LoadPtr + BinOp + StorePtr = 3 instructions (12 bytes),
                // same footprint as the stack-slot variant (Load + BinOp + Store).
                if let ExprKind::Unary { op: crate::ast::UnaryOp::Deref, operand } = &target.kind {
                    // Lower the pointer operand to a register.
                    let ptr_val = self.lower_expr(operand, &IrTy::I32)?;
                    let ptr_reg = self.val_to_reg(ptr_val)?;

                    // Load current value through the pointer.
                    let lhs_reg = self.alloc_reg()?;
                    self.instrs.push(Instr::LoadPtr { dst: lhs_reg, src: ptr_reg });

                    // Lower the RHS.
                    let rhs_val = self.lower_expr(value, &IrTy::I32)?;
                    let rhs_reg = self.val_to_reg(rhs_val)?;

                    // Apply the binary operation.
                    let ir_op = match op {
                        BinOp::Add    => IrBinOp::Add,
                        BinOp::Sub    => IrBinOp::Sub,
                        BinOp::Mul    => IrBinOp::Mul,
                        BinOp::Div    => IrBinOp::Div,
                        BinOp::Rem    => IrBinOp::Rem,
                        BinOp::BitAnd => IrBinOp::BitAnd,
                        BinOp::BitOr  => IrBinOp::BitOr,
                        BinOp::BitXor => IrBinOp::BitXor,
                        BinOp::Shl    => IrBinOp::Shl,
                        BinOp::Shr    => IrBinOp::Shr,
                        _ => unreachable!("compound assignment operator must be arithmetic or bitwise"),
                    };
                    let res_reg = self.alloc_reg()?;
                    self.instrs.push(Instr::BinOp { op: ir_op, dst: res_reg, lhs: lhs_reg, rhs: rhs_reg });

                    // Store the result back through the pointer.
                    self.instrs.push(Instr::StorePtr { src: res_reg, addr: ptr_reg });

                    return Ok(IrValue::Unit);
                }

                // Resolve target to a stack slot.
                //
                // FLS §6.5.11: Supports two place expression forms on the LHS:
                //   1. Simple variable path (`x += 1`)
                //   2. Struct field access (`self.n += 1`) — FLS §6.13
                //
                // FLS §10.1: `&mut self` field mutations via `self.n += 1` use
                // this field-access path to locate the field's stack slot.
                let slot = match &target.kind {
                    ExprKind::Path(segments) if segments.len() == 1 => {
                        let var_name = segments[0].text(self.source);
                        self.locals.get(var_name).copied().ok_or_else(|| {
                            LowerError::Unsupported(format!(
                                "compound assignment to undefined variable `{var_name}`"
                            ))
                        })?
                    }
                    ExprKind::FieldAccess { .. } => {
                        // FLS §6.5.11: Compound assignment on a struct field or chained
                        // field access — resolve using `resolve_place` which handles
                        // both `self.field += n` and `r.b.x += n` uniformly.
                        let (slot, _) = self.resolve_place(target)?;
                        slot
                    }
                    _ => {
                        return Err(LowerError::Unsupported(
                            "compound assignment to non-variable place expression not yet supported".into(),
                        ));
                    }
                };

                // Load current value of target at runtime.
                // FLS §6.1.2:37–45: this load is a runtime instruction.
                let lhs_reg = self.alloc_reg()?;
                self.instrs.push(Instr::Load { dst: lhs_reg, slot });

                // Lower the RHS value expression.
                let rhs_val = self.lower_expr(value, &IrTy::I32)?;
                let rhs_reg = self.val_to_reg(rhs_val)?;

                // Map the compound operator to its IR binary op.
                let ir_op = match op {
                    BinOp::Add    => IrBinOp::Add,
                    BinOp::Sub    => IrBinOp::Sub,
                    BinOp::Mul    => IrBinOp::Mul,
                    BinOp::Div    => IrBinOp::Div,
                    BinOp::Rem    => IrBinOp::Rem,
                    BinOp::BitAnd => IrBinOp::BitAnd,
                    BinOp::BitOr  => IrBinOp::BitOr,
                    BinOp::BitXor => IrBinOp::BitXor,
                    BinOp::Shl    => IrBinOp::Shl,
                    BinOp::Shr    => IrBinOp::Shr,
                    _ => unreachable!(
                        "compound assignment operator must be arithmetic or bitwise (got {op:?})"
                    ),
                };
                let dst = self.alloc_reg()?;
                self.instrs.push(Instr::BinOp { op: ir_op, dst, lhs: lhs_reg, rhs: rhs_reg });

                // Store the result back to the same stack slot.
                // FLS §6.5.11: "Compound assignment is equivalent to the binary expression
                // followed by assignment." The store makes this observable to subsequent reads.
                self.instrs.push(Instr::Store { src: dst, slot });

                // FLS §6.5.11: compound assignment has type `()`.
                Ok(IrValue::Unit)
            }

            // FLS §6.5.3: Comparison operator expressions.
            //
            // Comparisons evaluate both operands (i32) at runtime and produce a
            // boolean result as 0 (false) or 1 (true). ARM64 codegen emits
            // `cmp x{lhs}, x{rhs}` followed by `cset x{dst}, <cond>` to
            // materialise the result into a register.
            //
            // FLS §6.1.2:37–45: Even statically-known comparisons emit runtime
            // instructions — `5 < 10` emits `cmp`+`cset`, not `mov x0, #1`.
            //
            // The result type is boolean (represented as i32: 0 or 1). This matches
            // the representation used by `CondBranch` (cbz tests for zero).
            ExprKind::Binary { op, lhs, rhs }
                if matches!(
                    op,
                    BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::Eq | BinOp::Ne
                ) =>
            {
                // Both operands must be i32 at this milestone.
                // FLS §6.5.3 AMBIGUOUS: the spec does not separately describe
                // the type-checking rules for comparisons in the absence of
                // type inference. We assume both sides are i32.
                let lhs_val = self.lower_expr(lhs, &IrTy::I32)?;
                // Spill lhs register if rhs contains a call (ARM64 caller-save
                // convention; same rationale as arithmetic BinOp above).
                let lhs_spill: Option<u8> = if let IrValue::Reg(r) = lhs_val {
                    if expr_contains_call(rhs) {
                        let slot = self.alloc_slot()?;
                        self.instrs.push(Instr::Store { src: r, slot });
                        Some(slot)
                    } else {
                        None
                    }
                } else {
                    None
                };
                let rhs_val = self.lower_expr(rhs, &IrTy::I32)?;
                let lhs_val = if let Some(slot) = lhs_spill {
                    let dst = self.alloc_reg()?;
                    self.instrs.push(Instr::Load { dst, slot });
                    IrValue::Reg(dst)
                } else {
                    lhs_val
                };
                let lhs_reg = self.val_to_reg(lhs_val)?;
                let rhs_reg = self.val_to_reg(rhs_val)?;
                let dst = self.alloc_reg()?;
                let ir_op = match op {
                    BinOp::Lt => IrBinOp::Lt,
                    BinOp::Le => IrBinOp::Le,
                    BinOp::Gt => IrBinOp::Gt,
                    BinOp::Ge => IrBinOp::Ge,
                    BinOp::Eq => IrBinOp::Eq,
                    BinOp::Ne => IrBinOp::Ne,
                    _ => unreachable!("matched above"),
                };
                self.instrs.push(Instr::BinOp { op: ir_op, dst, lhs: lhs_reg, rhs: rhs_reg });
                Ok(IrValue::Reg(dst))
            }

            // FLS §6.15.3: While loop expression.
            //
            // A while loop evaluates the condition before each iteration. If the
            // condition is true, the body executes; the loop then repeats. If the
            // condition is false, the loop terminates with value `()`.
            //
            // Lowering strategy (standard "loop with pre-test" pattern):
            //   1. Emit header label — the branch-back target for the loop back-edge.
            //   2. Lower condition → cond_reg.
            //   3. Emit CondBranch (cbz): jump to exit_label if cond_reg == 0 (false).
            //   4. Lower body block (statements; tail value discarded — type is `()`).
            //   5. Emit unconditional Branch back to header_label.
            //   6. Emit exit_label.
            //   7. Return IrValue::Unit (while loops always have type `()`).
            //
            // FLS §6.15.3: "A while expression has the unit type."
            // FLS §6.1.2:37–45: The condition is evaluated at runtime every iteration,
            // even when statically known — `while true { ... }` emits a `mov`+`cbz`.
            //
            // Cache-line note: the header and exit labels carry no instruction cost.
            // The back-edge `b .L{header}` is one 4-byte instruction — it fits in
            // the same cache line as the last instruction of the body.
            ExprKind::While { cond, body } => {
                let header_label = self.alloc_label();
                let exit_label = self.alloc_label();

                // Save the register watermark before entering the loop.
                // After the loop exits (exit_label), all registers allocated inside
                // the loop are dead — the loop returns unit and carries no value
                // forward. Resetting next_reg here allows subsequent code (e.g., the
                // function tail expression) to reuse those register numbers, keeping
                // the total register count well below x30 (the ARM64 link register).
                let reg_mark = self.next_reg;

                // Push a loop context so that `break`/`continue` inside the body
                // can resolve to the correct labels.
                // FLS §6.15.3, §6.15.6, §6.15.7.
                // `while` loops do not support break-with-value (FLS §6.15.6).
                self.loop_stack.push(LoopCtx { header_label, exit_label, break_slot: None, break_ret_ty: IrTy::Unit });

                // Loop top: the branch target for the back-edge.
                self.instrs.push(Instr::Label(header_label));

                // Evaluate condition as IrTy::Bool so that `!bool_var` used
                // as a while condition emits logical NOT (BoolNot/eor) rather
                // than bitwise NOT (Not/mvn). FLS §6.5.4.
                // FLS §6.15.3: the condition of a while expression must be bool.
                let cond_val = self.lower_expr(cond, &IrTy::Bool)?;
                let cond_reg = self.val_to_reg(cond_val)?;

                // cbz: exit if condition is false (0).
                self.instrs.push(Instr::CondBranch { reg: cond_reg, label: exit_label });

                // Execute body. The body has type `()` and its value is discarded.
                // FLS §6.15.3: "The block of the while loop is repeatedly executed
                // as long as the condition is true."
                self.lower_block_to_value(body, &IrTy::Unit)?;

                // Back-edge: unconditionally re-evaluate the condition.
                self.instrs.push(Instr::Branch(header_label));

                // Exit: the while expression produces `()`.
                self.instrs.push(Instr::Label(exit_label));

                self.loop_stack.pop();

                // Restore register watermark. Registers allocated inside the loop
                // are only referenced by instructions between header_label and the
                // back-edge Branch. After exit_label, none of them are live.
                self.next_reg = reg_mark;

                // FLS §6.15.3: "The type of a while expression is the unit type ()."
                Ok(IrValue::Unit)
            }

            // FLS §6.15.4: While-let loop expression.
            //
            // `while let Pattern = scrutinee { body }`
            //
            // FLS §6.15.4: "A while let loop expression is syntactic sugar for
            // a loop expression containing a match expression that breaks on
            // mismatch." Lowered directly to a header + pattern-check + body
            // structure for clarity and efficiency.
            //
            // Lowering strategy:
            //   1. Emit header_label (back-edge target for both continue and
            //      each new iteration).
            //   2. Lower scrutinee → spill to scrut_slot (re-evaluated each
            //      iteration, consistent with FLS §6.1.2:37–45).
            //   3. Pattern check: emit comparison and CondBranch to exit_label
            //      on no-match (same logic as IfLet pattern check).
            //   4. Install identifier binding (if any) before the body.
            //   5. Lower the body as unit.
            //   6. Remove binding (if any).
            //   7. Branch to header_label.
            //   8. Emit exit_label.
            //   9. Return Unit — FLS §6.15.4 says the loop type is `()`.
            //
            // `break` in body → exit_label (no break-with-value).
            // `continue` in body → header_label (re-evaluates scrutinee).
            //
            // FLS §6.15.6: while-let does not support break-with-value.
            // FLS §6.1.2:37–45: All checks emit runtime instructions.
            // Cache-line note: loop header costs ~2 instr (str scrut + pattern
            // cmp) plus body; back-edge is 1 branch (4 bytes).
            ExprKind::WhileLet { pat, scrutinee, body } => {
                let header_label = self.alloc_label();
                let exit_label = self.alloc_label();

                // FLS §15: If the scrutinee is a plain variable holding an enum
                // value, record its base slot for TupleStruct/StructVariant pattern
                // field access. The base slot is a compile-time constant; each
                // loop iteration loads field values from that fixed stack location.
                let enum_base_slot: Option<u8> =
                    if let ExprKind::Path(segs) = &scrutinee.kind
                        && segs.len() == 1
                    {
                        let var_name = segs[0].text(self.source);
                        self.locals
                            .get(var_name)
                            .copied()
                            .filter(|s| self.local_enum_types.contains_key(s))
                    } else {
                        None
                    };
                // FLS §5.3: Plain struct variable scrutinee for struct pattern.
                let struct_base_slot: Option<u8> =
                    if let ExprKind::Path(segs) = &scrutinee.kind
                        && segs.len() == 1
                    {
                        let var_name = segs[0].text(self.source);
                        self.locals
                            .get(var_name)
                            .copied()
                            .filter(|s| self.local_struct_types.contains_key(s))
                    } else {
                        None
                    };

                // Save register watermark — same rationale as While above.
                let reg_mark = self.next_reg;

                // Push loop context — while-let has no break-with-value.
                // FLS §6.15.4, §6.15.6.
                self.loop_stack.push(LoopCtx {
                    header_label,
                    exit_label,
                    break_slot: None,
                    break_ret_ty: IrTy::Unit,
                });

                self.instrs.push(Instr::Label(header_label));

                // Infer scrutinee type from the pattern.
                let scrut_ty = match pat {
                    Pat::LitBool(_) => IrTy::Bool,
                    Pat::Or(alts) if alts.iter().any(|p| matches!(p, Pat::LitBool(_))) => {
                        IrTy::Bool
                    }
                    _ => IrTy::I32,
                };
                let scrut_val = self.lower_expr(scrutinee, &scrut_ty)?;
                let scrut_reg = self.val_to_reg(scrut_val)?;
                let scrut_slot = self.alloc_slot()?;
                self.instrs.push(Instr::Store { src: scrut_reg, slot: scrut_slot });

                // Bindings introduced by TupleStruct/StructVariant/Ident patterns;
                // all are removed before the back-edge (end of loop body).
                let mut bound_names: Vec<&str> = Vec::new();

                // Pattern check — branch to exit_label on mismatch.
                // Uses the same per-pattern logic as IfLet.
                // FLS §6.15.4: mismatch breaks out of the loop.
                match pat {
                    Pat::Wildcard | Pat::Ident(_) => {
                        // Always matches — no conditional branch.
                    }
                    Pat::LitInt(n) => {
                        let s_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                        let p_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(p_reg, *n as i32));
                        let eq_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Eq,
                            dst: eq_reg,
                            lhs: s_reg,
                            rhs: p_reg,
                        });
                        self.instrs.push(Instr::CondBranch { reg: eq_reg, label: exit_label });
                    }
                    Pat::NegLitInt(n) => {
                        let s_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                        let p_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(p_reg, -(*n as i32)));
                        let eq_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Eq,
                            dst: eq_reg,
                            lhs: s_reg,
                            rhs: p_reg,
                        });
                        self.instrs.push(Instr::CondBranch { reg: eq_reg, label: exit_label });
                    }
                    Pat::LitBool(b) => {
                        let s_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                        let p_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(p_reg, *b as i32));
                        let eq_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Eq,
                            dst: eq_reg,
                            lhs: s_reg,
                            rhs: p_reg,
                        });
                        self.instrs.push(Instr::CondBranch { reg: eq_reg, label: exit_label });
                    }
                    Pat::RangeInclusive { lo, hi } => {
                        let s_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                        let lo_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(lo_reg, *lo as i32));
                        let cmp1 = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Ge,
                            dst: cmp1,
                            lhs: s_reg,
                            rhs: lo_reg,
                        });
                        let hi_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(hi_reg, *hi as i32));
                        let cmp2 = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Le,
                            dst: cmp2,
                            lhs: s_reg,
                            rhs: hi_reg,
                        });
                        let matched = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::BitAnd,
                            dst: matched,
                            lhs: cmp1,
                            rhs: cmp2,
                        });
                        self.instrs.push(Instr::CondBranch { reg: matched, label: exit_label });
                    }
                    Pat::RangeExclusive { lo, hi } => {
                        let s_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                        let lo_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(lo_reg, *lo as i32));
                        let cmp1 = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Ge,
                            dst: cmp1,
                            lhs: s_reg,
                            rhs: lo_reg,
                        });
                        let hi_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(hi_reg, *hi as i32));
                        let cmp2 = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Lt,
                            dst: cmp2,
                            lhs: s_reg,
                            rhs: hi_reg,
                        });
                        let matched = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::BitAnd,
                            dst: matched,
                            lhs: cmp1,
                            rhs: cmp2,
                        });
                        self.instrs.push(Instr::CondBranch { reg: matched, label: exit_label });
                    }
                    Pat::Or(alts) => {
                        let matched_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(matched_reg, 0));
                        for alt in alts {
                            match alt {
                                Pat::Wildcard => {
                                    self.instrs.push(Instr::LoadImm(matched_reg, 1));
                                    break;
                                }
                                Pat::Or(_) | Pat::Ident(_) => {
                                    return Err(LowerError::Unsupported(
                                        "nested OR or identifier inside while-let OR pattern".into(),
                                    ));
                                }
                                _ => {
                                    let alt_imm = self.pat_scalar_imm(alt)?;
                                    let si_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load {
                                        dst: si_reg,
                                        slot: scrut_slot,
                                    });
                                    let alt_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(alt_reg, alt_imm));
                                    let eq_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Eq,
                                        dst: eq_reg,
                                        lhs: si_reg,
                                        rhs: alt_reg,
                                    });
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::BitOr,
                                        dst: matched_reg,
                                        lhs: matched_reg,
                                        rhs: eq_reg,
                                    });
                                }
                            }
                        }
                        self.instrs.push(Instr::CondBranch {
                            reg: matched_reg,
                            label: exit_label,
                        });
                    }
                    // FLS §5.5 + §15: Path pattern in while-let — enum unit variant.
                    Pat::Path(segs) => {
                        let pat_imm = if segs.len() == 2 {
                            let enum_name = segs[0].text(self.source);
                            let variant_name = segs[1].text(self.source);
                            self.enum_defs
                                .get(enum_name)
                                .and_then(|v| v.get(variant_name))
                                .map(|(disc, _)| *disc)
                                .ok_or_else(|| LowerError::Unsupported(format!(
                                    "unknown enum variant `{enum_name}::{variant_name}` in while-let pattern"
                                )))?
                        } else {
                            return Err(LowerError::Unsupported(
                                "path pattern must have exactly two segments".into(),
                            ));
                        };
                        let s_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                        let p_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(p_reg, pat_imm));
                        let eq_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Eq,
                            dst: eq_reg,
                            lhs: s_reg,
                            rhs: p_reg,
                        });
                        self.instrs.push(Instr::CondBranch { reg: eq_reg, label: exit_label });
                    }
                    // FLS §5.4 + §15: TupleStruct pattern in while-let.
                    //
                    // `while let Enum::Variant(f0, f1, ..) = x { body }`
                    // Strategy: compare discriminant at scrut_slot against the variant's
                    // discriminant; branch to exit_label on mismatch; then install
                    // positional field bindings from enum_base_slot + 1 + idx.
                    // Bindings are removed before the back-edge each iteration.
                    //
                    // FLS §6.15.4: mismatch terminates the loop.
                    // FLS §6.1.2:37–45: All instructions are runtime.
                    // Cache-line note: ~5 + 2×N instructions per iteration header.
                    Pat::TupleStruct { path: segs, fields } => {
                        if segs.len() != 2 {
                            return Err(LowerError::Unsupported(
                                "tuple struct pattern path must have two segments".into(),
                            ));
                        }
                        let enum_name = segs[0].text(self.source);
                        let variant_name = segs[1].text(self.source);
                        let discriminant = self
                            .enum_defs
                            .get(enum_name)
                            .and_then(|v| v.get(variant_name))
                            .map(|(disc, _)| *disc)
                            .ok_or_else(|| {
                                LowerError::Unsupported(format!(
                                    "unknown enum variant `{enum_name}::{variant_name}`"
                                ))
                            })?;
                        let base = enum_base_slot.ok_or_else(|| {
                            LowerError::Unsupported(
                                "TupleStruct pattern in while-let requires enum variable scrutinee"
                                    .into(),
                            )
                        })?;
                        // Discriminant check — branch to exit on mismatch.
                        let s_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                        let p_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(p_reg, discriminant));
                        let cmp_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Eq,
                            dst: cmp_reg,
                            lhs: s_reg,
                            rhs: p_reg,
                        });
                        self.instrs.push(Instr::CondBranch { reg: cmp_reg, label: exit_label });
                        // Install positional field bindings into bound_names.
                        for (fi, fp) in fields.iter().enumerate() {
                            if let Pat::Ident(span) = fp {
                                let fname = span.text(self.source);
                                let fslot = base + 1 + fi as u8;
                                let bslot = self.alloc_slot()?;
                                let breg = self.alloc_reg()?;
                                self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                self.locals.insert(fname, bslot);
                                bound_names.push(fname);
                            } else if !matches!(fp, Pat::Wildcard) {
                                return Err(LowerError::Unsupported(
                                    "only ident/wildcard fields in TupleStruct while-let pattern"
                                        .into(),
                                ));
                            }
                        }
                    }
                    // FLS §5.3 + §15.3: Named-field struct or variant pattern in while-let.
                    // 1-segment: plain struct (irrefutable — loop runs forever unless broken).
                    // 2-segment: enum variant (terminates loop on discriminant mismatch).
                    Pat::StructVariant { path: segs, fields: pat_fields } => {
                        if segs.len() == 1 {
                            // FLS §5.3: Plain struct pattern — always matches.
                            let struct_name = segs[0].text(self.source);
                            let base = struct_base_slot.ok_or_else(|| {
                                LowerError::Unsupported(
                                    "plain struct pattern in while-let requires struct variable scrutinee".into(),
                                )
                            })?;
                            let field_names = self.struct_defs
                                .get(struct_name)
                                .cloned()
                                .ok_or_else(|| LowerError::Unsupported(format!(
                                    "unknown struct `{struct_name}`"
                                )))?;
                            // No CondBranch — plain struct patterns are irrefutable.
                            for (fname_span, fp) in pat_fields.iter() {
                                let fname = fname_span.text(self.source);
                                let field_idx = field_names.iter().position(|n| n == fname)
                                    .ok_or_else(|| LowerError::Unsupported(format!(
                                        "struct `{struct_name}` has no field `{fname}`"
                                    )))?;
                                match fp {
                                    Pat::Ident(bind_span) => {
                                        let bind_name = bind_span.text(self.source);
                                        let fslot = base + field_idx as u8;
                                        let bslot = self.alloc_slot()?;
                                        let breg = self.alloc_reg()?;
                                        self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                        self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                        self.locals.insert(bind_name, bslot);
                                        bound_names.push(bind_name);
                                    }
                                    Pat::Wildcard => {}
                                    _ => return Err(LowerError::Unsupported(
                                        "only ident/wildcard sub-patterns in struct while-let patterns".into(),
                                    )),
                                }
                            }
                        } else if segs.len() == 2 {
                            let enum_name = segs[0].text(self.source);
                            let variant_name = segs[1].text(self.source);
                            let (discriminant, field_names) = self
                                .enum_defs
                                .get(enum_name)
                                .and_then(|v| v.get(variant_name))
                                .cloned()
                                .ok_or_else(|| {
                                    LowerError::Unsupported(format!(
                                        "unknown enum variant `{enum_name}::{variant_name}`"
                                    ))
                                })?;
                            let base = enum_base_slot.ok_or_else(|| {
                                LowerError::Unsupported(
                                    "StructVariant pattern in while-let requires enum variable scrutinee"
                                        .into(),
                                )
                            })?;
                            // Discriminant check — branch to exit on mismatch.
                            let s_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                            let p_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::LoadImm(p_reg, discriminant));
                            let cmp_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::BinOp {
                                op: IrBinOp::Eq,
                                dst: cmp_reg,
                                lhs: s_reg,
                                rhs: p_reg,
                            });
                            self.instrs.push(Instr::CondBranch { reg: cmp_reg, label: exit_label });
                            // Install named field bindings by declaration order.
                            for (fname_span, fp) in pat_fields.iter() {
                                let fname = fname_span.text(self.source);
                                let field_idx = field_names
                                    .iter()
                                    .position(|n| n == fname)
                                    .ok_or_else(|| {
                                        LowerError::Unsupported(format!(
                                            "enum variant `{enum_name}::{variant_name}` has no field `{fname}`"
                                        ))
                                    })?;
                                match fp {
                                    Pat::Ident(bind_span) => {
                                        let bind_name = bind_span.text(self.source);
                                        let fslot = base + 1 + field_idx as u8;
                                        let bslot = self.alloc_slot()?;
                                        let breg = self.alloc_reg()?;
                                        self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                        self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                        self.locals.insert(bind_name, bslot);
                                        bound_names.push(bind_name);
                                    }
                                    Pat::Wildcard => {}
                                    _ => return Err(LowerError::Unsupported(
                                        "only ident/wildcard sub-patterns in StructVariant while-let fields".into(),
                                    )),
                                }
                            }
                        } else {
                            return Err(LowerError::Unsupported(
                                "struct/variant while-let pattern path must have 1 or 2 segments".into(),
                            ));
                        }
                    }
                    Pat::Tuple(_) => {
                        return Err(LowerError::Unsupported(
                            "tuple pattern in while-let not yet supported".into(),
                        ));
                    }
                }

                // Install identifier binding (if any) — in scope for body only.
                // FLS §5.1.4: identifier pattern binds the scrutinee.
                // TupleStruct/StructVariant field bindings were already pushed to
                // bound_names inside the pattern check above.
                if let Pat::Ident(span) = pat {
                    let name = span.text(self.source);
                    let bind_slot = self.alloc_slot()?;
                    let bind_reg = self.alloc_reg()?;
                    self.instrs.push(Instr::Load { dst: bind_reg, slot: scrut_slot });
                    self.instrs.push(Instr::Store { src: bind_reg, slot: bind_slot });
                    self.locals.insert(name, bind_slot);
                    bound_names.push(name);
                }

                // Execute body.
                self.lower_block_to_value(body, &IrTy::Unit)?;

                // Remove all bindings before back-edge.
                for name in &bound_names {
                    self.locals.remove(*name);
                }

                // Back-edge: re-evaluate scrutinee and re-check pattern.
                self.instrs.push(Instr::Branch(header_label));

                self.instrs.push(Instr::Label(exit_label));
                self.loop_stack.pop();

                // Restore register watermark (see While above for rationale).
                self.next_reg = reg_mark;

                // FLS §6.15.4: "The type of a while let loop expression is the unit type ()."
                Ok(IrValue::Unit)
            }

            // FLS §6.15.1: For loop expression `for pat in start..end { body }`.
            //
            // Galvanic supports `for i in start..end` (exclusive integer range) and
            // `for i in start..=end` (inclusive integer range) as the first for-loop
            // milestone. The range iterator is desugared directly to a while-loop
            // equivalent — no library `IntoIterator` trait is involved.
            //
            // Lowering strategy:
            //   1. Lower `start` → store in a new stack slot for the loop variable.
            //   2. Lower `end` → store in a new stack slot for the end bound.
            //   3. Emit cond_label (back-edge target after increment).
            //   4. Load loop var, load end bound, compare (<  for exclusive, <= for inclusive).
            //   5. CondBranch to exit_label if condition is false (loop var >= end).
            //   6. Lower body (loop var visible in `locals`).
            //   7. emit incr_label (target for `continue`).
            //   8. Increment loop var slot by 1.
            //   9. Branch cond_label.
            //  10. Emit exit_label.
            //
            // `continue` in the body jumps to incr_label (increments then re-checks).
            // `break` in the body jumps to exit_label.
            //
            // FLS §6.15.1: "A for loop expression iterates over an iterator."
            // FLS §6.16: Range expressions produce iterators over integers.
            // FLS §6.1.2:37–45: The back-edge is a runtime branch — never elided.
            // FLS §6.15.7: `continue` advances to the next iteration (= increment step).
            //
            // Cache-line note: the loop generates ~7 instructions for the control
            // flow skeleton (load, cmp, cbz, load, add imm, str, b) — 28 bytes, fits
            // in one 32-byte half of a 64-byte instruction cache line.
            ExprKind::For { pat, iter, body } => {
                // Only integer range iterators are supported at this milestone.
                let (start_expr, end_expr, inclusive) = match iter.as_ref() {
                    Expr { kind: ExprKind::Range { start: Some(s), end: Some(e), inclusive }, .. } => {
                        (s.as_ref(), e.as_ref(), *inclusive)
                    }
                    _ => return Err(LowerError::Unsupported(
                        "for loop requires an integer range iterator (start..end or start..=end)".into(),
                    )),
                };

                // Allocate the loop variable slot and record the name.
                let i_slot = self.alloc_slot()?;
                let pat_name = pat.text(self.source);

                // Allocate a slot for the end bound (evaluated once before the loop).
                // FLS §6.16: The range bounds are evaluated once, not on each iteration.
                let end_slot = self.alloc_slot()?;

                // Save register watermark — same rationale as While above. Saved
                // BEFORE lowering start/end expressions so those temp registers are
                // also recyclable (they are consumed by Store and not live after it).
                let reg_mark = self.next_reg;

                // Lower and store the start bound into the loop variable slot.
                // FLS §6.16: `start` is evaluated first (left-to-right, FLS §6:3).
                let start_val = self.lower_expr(start_expr, &IrTy::I32)?;
                let start_reg = self.val_to_reg(start_val)?;
                self.instrs.push(Instr::Store { src: start_reg, slot: i_slot });

                // Lower and store the end bound.
                let end_val = self.lower_expr(end_expr, &IrTy::I32)?;
                let end_reg = self.val_to_reg(end_val)?;
                self.instrs.push(Instr::Store { src: end_reg, slot: end_slot });

                // Labels: cond (condition check / back-edge), incr (increment / continue
                // target), exit (after loop / break target).
                let cond_label = self.alloc_label();
                let incr_label = self.alloc_label();
                let exit_label = self.alloc_label();

                // Push loop context: `continue` → incr_label, `break` → exit_label.
                // FLS §6.15.7: `continue` in a for loop advances to the next iteration.
                // For a range-based for loop, the "next iteration" is the increment step.
                // `for` loops do not support break-with-value (FLS §6.15.6).
                self.loop_stack.push(LoopCtx { header_label: incr_label, exit_label, break_slot: None, break_ret_ty: IrTy::Unit });

                // Bind the loop variable name so body can load it via Path.
                self.locals.insert(pat_name, i_slot);

                // Condition check: load loop var and end bound, compare.
                self.instrs.push(Instr::Label(cond_label));
                let i_reg = self.alloc_reg()?;
                self.instrs.push(Instr::Load { dst: i_reg, slot: i_slot });
                let end_reg2 = self.alloc_reg()?;
                self.instrs.push(Instr::Load { dst: end_reg2, slot: end_slot });
                let cmp_reg = self.alloc_reg()?;
                // Exclusive `..`: continue while i < end (IrBinOp::Lt).
                // Inclusive `..=`: continue while i <= end (IrBinOp::Le).
                let cmp_op = if inclusive { IrBinOp::Le } else { IrBinOp::Lt };
                self.instrs.push(Instr::BinOp { op: cmp_op, dst: cmp_reg, lhs: i_reg, rhs: end_reg2 });
                // Exit if condition is false (i >= end for exclusive, i > end for inclusive).
                self.instrs.push(Instr::CondBranch { reg: cmp_reg, label: exit_label });

                // Lower body. The body result is discarded (for loops yield `()`).
                // FLS §6.15.1: "A for loop evaluates to the unit type."
                self.lower_block_to_value(body, &IrTy::Unit)?;

                // Increment step: i += 1. This is the `continue` target.
                // FLS §6.15.7: After a `continue`, the loop variable is incremented
                // before the condition is re-evaluated.
                self.instrs.push(Instr::Label(incr_label));
                let i_reg2 = self.alloc_reg()?;
                self.instrs.push(Instr::Load { dst: i_reg2, slot: i_slot });
                let one_reg = self.alloc_reg()?;
                self.instrs.push(Instr::LoadImm(one_reg, 1));
                let inc_reg = self.alloc_reg()?;
                self.instrs.push(Instr::BinOp { op: IrBinOp::Add, dst: inc_reg, lhs: i_reg2, rhs: one_reg });
                self.instrs.push(Instr::Store { src: inc_reg, slot: i_slot });

                // Back-edge: jump to condition check.
                self.instrs.push(Instr::Branch(cond_label));

                // Exit label: loop done.
                self.instrs.push(Instr::Label(exit_label));

                self.loop_stack.pop();

                // Restore register watermark (see While above for rationale).
                self.next_reg = reg_mark;

                // FLS §6.15.1: "The type of a for loop expression is the unit type ()."
                Ok(IrValue::Unit)
            }

            // FLS §6.15.2: Infinite loop expression `loop { body }`.
            //
            // A loop expression repeatedly executes its body until a `break`
            // expression transfers control past the loop. Unlike `while`, there
            // is no condition — the only exit is an explicit `break`.
            //
            // Lowering strategy:
            //   1. Emit header label (back-edge target for continue / back-edge).
            //   2. Push LoopCtx so `break`/`continue` resolve to the right labels.
            //   3. Lower body block (unit type — value discarded).
            //   4. Emit unconditional back-edge Branch to header_label.
            //   5. Emit exit_label (where `break` branches to).
            //   6. Pop LoopCtx.
            //   7. Return Unit (loop without break value has type `()`).
            //
            // FLS §6.15.2: "A loop expression evaluates its block repeatedly
            // until a break expression is encountered."
            // FLS §6.15.2: "The type of a loop expression without a break value
            // is the unit type ()."
            // FLS §6.1.2:37–45: The back-edge is a runtime branch instruction
            // — it is not eliminated even if the body contains no side effects.
            //
            // Cache-line note: the header and exit labels have no instruction cost.
            // The back-edge `b .L{header}` is 4 bytes — one instruction slot.
            ExprKind::Loop(body) => {
                let header_label = self.alloc_label();
                let exit_label = self.alloc_label();

                // FLS §6.15.6: Only `loop` expressions support break-with-value.
                // Scan the body for `break <value>` at this loop level. If any
                // are present, allocate a stack slot to hold the result — the
                // same phi-slot pattern used for if-else expressions.
                //
                // The break_slot is allocated BEFORE entering the loop body so
                // that `break <value>` can store into it during body lowering.
                let break_slot = if block_contains_break_with_value(body) {
                    Some(self.alloc_slot()?)
                } else {
                    None
                };

                // Save register watermark — same rationale as While above.
                // Saved AFTER break_slot allocation (a stack slot, not a register)
                // and BEFORE any register allocations inside the body.
                let reg_mark = self.next_reg;

                self.loop_stack.push(LoopCtx { header_label, exit_label, break_slot, break_ret_ty: *ret_ty });

                // Loop top: the branch-back target.
                self.instrs.push(Instr::Label(header_label));

                // Execute body. Value is discarded; only side effects matter.
                // The `break <value>` arms inside lower_expr will store to break_slot.
                self.lower_block_to_value(body, &IrTy::Unit)?;

                // Back-edge: jump unconditionally to the top of the loop.
                // FLS §6.15.2: execution continues indefinitely until `break`.
                self.instrs.push(Instr::Branch(header_label));

                // Exit: where `break` transfers control.
                self.instrs.push(Instr::Label(exit_label));

                self.loop_stack.pop();

                // Restore register watermark. Any registers used inside the loop body
                // are only live between header_label and the back-edge Branch. After
                // exit_label they are dead, so we can reuse their numbers. The result
                // register (if any) is freshly allocated below from the restored mark.
                self.next_reg = reg_mark;

                // FLS §6.15.2: "The type of a loop expression is determined by
                // its break expressions." If break-with-value was used, load the
                // result from the break slot. Otherwise the loop has type `()`.
                //
                // Cache-line note: the Load after the exit label is typically the
                // first instruction in a new cache line (the back-edge `b` and
                // the exit label occupy the end of the previous line). One Load
                // = 4 bytes.
                if let Some(slot) = break_slot {
                    let result_reg = self.alloc_reg()?;
                    self.instrs.push(Instr::Load { dst: result_reg, slot });
                    Ok(IrValue::Reg(result_reg))
                } else {
                    Ok(IrValue::Unit)
                }
            }

            // FLS §6.15.6: Break expression — exit the innermost enclosing loop.
            //
            // An unqualified `break` jumps to the exit label of the innermost
            // loop. A `break value` expression would additionally store the value
            // to a break slot; break-with-value is not yet supported.
            //
            // FLS §6.15.6: "A break expression exits the innermost enclosing loop
            // expression or block expression labelled with a block label."
            // FLS §6.15.6: "The type of a break expression is the never type `!`."
            // We approximate `!` as Unit since the never type is not yet in the IR.
            //
            // FLS §6.1.2:37–45: The branch is a runtime `b` instruction — even if
            // the condition leading to `break` is statically known, the branch
            // resolves at runtime.
            //
            // Cache-line note: `b .L{exit}` is 4 bytes — one instruction slot.
            // FLS §6.15.6: Break expression — exit the innermost enclosing loop.
            //
            // Two forms:
            //   - `break` (no value): branch to exit_label.
            //   - `break <expr>` (with value): lower the value, store it to the
            //     loop's break_slot, then branch to exit_label. The slot was
            //     allocated by the enclosing `loop` lowering if any break-with-value
            //     was detected via `block_contains_break_with_value`.
            //
            // FLS §6.15.6: "Only `loop` expressions support break-with-value."
            // Attempting `break value` in `while` or `for` (which have
            // `break_slot: None`) produces an unsupported error — consistent
            // with the spec restriction.
            //
            // FLS §6.1.2:37–45: The branch is a runtime `b` instruction.
            // Cache-line note: `store + b` = 8 bytes — two instructions, fits
            // in one half of a 16-byte bundle.
            ExprKind::Break(value) => {
                // Resolve the exit label and optional break slot from the
                // innermost loop context.
                let (exit_label, break_slot, break_ret_ty) = self.loop_stack.last()
                    .map(|ctx| (ctx.exit_label, ctx.break_slot, ctx.break_ret_ty))
                    .ok_or_else(|| LowerError::Unsupported(
                        "break expression outside of a loop".into()
                    ))?;

                if let Some(val_expr) = value {
                    // break-with-value: store result to the break_slot, then jump.
                    // Use the loop's break_ret_ty (not the break statement's ret_ty,
                    // which is Unit in the body context) to lower the value correctly.
                    let slot = break_slot.ok_or_else(|| LowerError::Unsupported(
                        "break with value is only valid in a `loop` expression (FLS §6.15.6)".into(),
                    ))?;
                    let v = self.lower_expr(val_expr, &break_ret_ty)?;
                    let r = self.val_to_reg(v)?;
                    self.instrs.push(Instr::Store { src: r, slot });
                }

                // Emit the branch to the loop exit.
                self.instrs.push(Instr::Branch(exit_label));

                // FLS §6.15.6: break has type `!` (never). Approximated as Unit.
                Ok(IrValue::Unit)
            }

            // FLS §6.15.7: Continue expression — restart the innermost loop.
            //
            // A `continue` transfers control to the header of the innermost
            // enclosing loop, skipping any remaining statements in the body.
            //
            // FLS §6.15.7: "A continue expression advances to the next iteration
            // of the innermost enclosing loop expression."
            // FLS §6.15.7: "The type of a continue expression is the never type `!`."
            // We approximate `!` as Unit since the never type is not yet in the IR.
            //
            // Cache-line note: `b .L{header}` is 4 bytes — same cost as `break`.
            ExprKind::Continue => {
                // Resolve the header label from the innermost loop context.
                let header_label = self.loop_stack.last()
                    .map(|ctx| ctx.header_label)
                    .ok_or_else(|| LowerError::Unsupported(
                        "continue expression outside of a loop".into()
                    ))?;

                self.instrs.push(Instr::Branch(header_label));

                // FLS §6.15.7: continue has type `!` (never). Approximated as Unit.
                Ok(IrValue::Unit)
            }

            // FLS §6.19: Return expression — transfer control to the caller.
            //
            // A `return` expression exits the current function immediately,
            // yielding the given value (or unit if none) to the caller.
            //
            // Lowering strategy:
            //   1. If a value is present, lower it using the function's return
            //      type (`fn_ret_ty`), not the current expression context type.
            //      The function return type is stored in `LowerCtx::fn_ret_ty`
            //      precisely for this purpose.
            //   2. Emit `Instr::Ret` with the value.
            //   3. Return `IrValue::Unit` to the caller of `lower_expr` — any
            //      code after a `return` in the same block is unreachable but
            //      the surrounding block still lowers it. Dead instructions
            //      after `ret` are ignored by the assembler.
            //
            // FLS §6.19: "The type of a return expression is the never type `!`."
            // We approximate `!` as Unit since the never type is not yet in the IR.
            //
            // FLS §6.1.2:37–45: The `ret` is a runtime instruction — no constant
            // folding of the returned value.
            //
            // Cache-line note: a `return <literal>` emits one `LoadImm` (4 bytes)
            // + one `ret` (4 bytes) = two instructions, one half a cache line.
            ExprKind::Return(opt_val) => {
                let fn_ret_ty = self.fn_ret_ty;
                let ret_val = match opt_val {
                    Some(val) => {
                        let v = self.lower_expr(val, &fn_ret_ty)?;
                        let r = self.val_to_reg(v)?;
                        IrValue::Reg(r)
                    }
                    None => IrValue::Unit,
                };
                self.instrs.push(Instr::Ret(ret_val));
                // FLS §6.19: return has type `!` (never). Approximated as Unit.
                Ok(IrValue::Unit)
            }

            // FLS §6.5.8: Lazy boolean AND operator `&&`.
            //
            // Short-circuit semantics: the RHS is only evaluated if the LHS is true.
            // If the LHS evaluates to false (0), the RHS is not evaluated and the
            // result is false (0). If the LHS is true (1), the result is the RHS value.
            //
            // Equivalent to: if lhs { rhs } else { false }
            //
            // Lowering strategy (same phi-slot pattern as if/else):
            //   1. Lower LHS → lhs_reg.
            //   2. Allocate phi slot for the result.
            //   3. CondBranch (cbz): if lhs_reg == 0 (false), jump to .Lfalse.
            //   4. Lower RHS → rhs_reg. Store rhs_reg → phi slot. Branch to .Lend.
            //   5. .Lfalse: store 0 → phi slot.
            //   6. .Lend: load phi slot → result_reg. Return Reg(result_reg).
            //
            // FLS §6.5.8: "The right operand is only evaluated if the left operand
            // is true." The CondBranch skips the RHS evaluation entirely.
            //
            // FLS §6.1.2:37–45: Even statically-known `true && x` emits a runtime
            // `cbz` — no constant folding of the short-circuit condition.
            //
            // Cache-line note: the phi slot is one 8-byte stack entry, same as
            // the if/else phi slot. The short-circuit adds one `cbz` (4 bytes) to
            // the instruction stream.
            ExprKind::Binary { op: BinOp::And, lhs, rhs } => {
                let false_label = self.alloc_label();
                let end_label = self.alloc_label();
                // Allocate the phi slot before entering either branch.
                let phi_slot = self.alloc_slot()?;

                // Evaluate LHS. Booleans are represented as i32 (0 or 1).
                let lhs_val = self.lower_expr(lhs, &IrTy::I32)?;
                let lhs_reg = self.val_to_reg(lhs_val)?;

                // Short-circuit: if LHS is false (0), skip RHS entirely.
                self.instrs.push(Instr::CondBranch { reg: lhs_reg, label: false_label });

                // ── LHS is true: evaluate RHS and use it as the result ────────────
                let rhs_val = self.lower_expr(rhs, &IrTy::I32)?;
                let rhs_reg = self.val_to_reg(rhs_val)?;
                self.instrs.push(Instr::Store { src: rhs_reg, slot: phi_slot });
                self.instrs.push(Instr::Branch(end_label));

                // ── LHS was false: result = 0 ─────────────────────────────────────
                self.instrs.push(Instr::Label(false_label));
                let zero_reg = self.alloc_reg()?;
                self.instrs.push(Instr::LoadImm(zero_reg, 0));
                self.instrs.push(Instr::Store { src: zero_reg, slot: phi_slot });

                // ── End: load and return result ───────────────────────────────────
                self.instrs.push(Instr::Label(end_label));
                let result_reg = self.alloc_reg()?;
                self.instrs.push(Instr::Load { dst: result_reg, slot: phi_slot });
                Ok(IrValue::Reg(result_reg))
            }

            // FLS §6.5.8: Lazy boolean OR operator `||`.
            //
            // Short-circuit semantics: the RHS is only evaluated if the LHS is false.
            // If the LHS evaluates to true (non-zero), the RHS is not evaluated and
            // the result is the LHS value (which is 1 for a boolean). If the LHS is
            // false (0), the result is the RHS value.
            //
            // Equivalent to: if lhs { lhs } else { rhs }
            // Simplified (since lhs is 0/1): if lhs { 1 } else { rhs }
            //
            // Lowering strategy (cbz branches to .Lrhs when LHS is false):
            //   1. Lower LHS → lhs_reg.
            //   2. Allocate phi slot for the result.
            //   3. CondBranch (cbz): if lhs_reg == 0 (false), jump to .Lrhs.
            //   4. Store lhs_reg → phi slot (lhs is 1). Branch to .Lend.
            //   5. .Lrhs: Lower RHS → rhs_reg. Store rhs_reg → phi slot.
            //   6. .Lend: load phi slot → result_reg. Return Reg(result_reg).
            //
            // FLS §6.5.8: "The right operand is only evaluated if the left operand
            // is false." The CondBranch skips the RHS evaluation entirely when true.
            //
            // FLS §6.1.2:37–45: Even statically-known `false || x` emits a runtime
            // `cbz` — no constant folding of the short-circuit condition.
            //
            // Cache-line note: identical phi-slot footprint to `&&` lowering.
            ExprKind::Binary { op: BinOp::Or, lhs, rhs } => {
                let rhs_label = self.alloc_label();
                let end_label = self.alloc_label();
                // Allocate the phi slot before entering either branch.
                let phi_slot = self.alloc_slot()?;

                // Evaluate LHS. Booleans are represented as i32 (0 or 1).
                let lhs_val = self.lower_expr(lhs, &IrTy::I32)?;
                let lhs_reg = self.val_to_reg(lhs_val)?;

                // Short-circuit: if LHS is false (0), must evaluate RHS.
                self.instrs.push(Instr::CondBranch { reg: lhs_reg, label: rhs_label });

                // ── LHS is true: result = LHS (which is 1) ───────────────────────
                self.instrs.push(Instr::Store { src: lhs_reg, slot: phi_slot });
                self.instrs.push(Instr::Branch(end_label));

                // ── LHS was false: evaluate RHS and use it as result ──────────────
                self.instrs.push(Instr::Label(rhs_label));
                let rhs_val = self.lower_expr(rhs, &IrTy::I32)?;
                let rhs_reg = self.val_to_reg(rhs_val)?;
                self.instrs.push(Instr::Store { src: rhs_reg, slot: phi_slot });

                // ── End: load and return result ───────────────────────────────────
                self.instrs.push(Instr::Label(end_label));
                let result_reg = self.alloc_reg()?;
                self.instrs.push(Instr::Load { dst: result_reg, slot: phi_slot });
                Ok(IrValue::Reg(result_reg))
            }

            // FLS §6.5.4: Unary negation `-operand` — arithmetic two's complement negation.
            //
            // Lowering:
            //   1. Lower the operand to a register.
            //   2. Emit `Instr::Neg { dst, src }` → `neg x{dst}, x{src}` on ARM64.
            //
            // FLS §6.1.2:37–45: Even `-5` in a non-const context emits a runtime `neg`
            // instruction — no compile-time folding to a negative immediate.
            //
            // FLS §6.5.4: "The type of a negation expression is the type of the operand."
            //
            // Cache-line note: `neg` is 4 bytes (alias for `sub xD, xzr, xS`).
            ExprKind::Unary { op: crate::ast::UnaryOp::Neg, operand } => {
                let val = self.lower_expr(operand, &IrTy::I32)?;
                let src = self.val_to_reg(val)?;
                let dst = self.alloc_reg()?;
                self.instrs.push(Instr::Neg { dst, src });
                Ok(IrValue::Reg(dst))
            }

            // FLS §6.5.4: Negation operator `!operand` — two cases:
            //
            //   1. `!bool_value` → logical NOT: `false` (0) ↔ `true` (1).
            //      ARM64: `eor x{dst}, x{src}, #1` (XOR with 1 flips bit 0).
            //      Emits `Instr::BoolNot`. Triggered when `ret_ty == IrTy::Bool`.
            //
            //   2. `!integer_value` → bitwise NOT: flip all bits.
            //      ARM64: `mvn x{dst}, x{src}` (alias for orn xD, xzr, xS).
            //      Emits `Instr::Not`. Triggered for all other `ret_ty`.
            //
            // The distinction is necessary because `mvn` of 0 is -1 (not 1),
            // and `mvn` of 1 is -2 (not 0). Using `mvn` for booleans produces
            // non-boolean results that break downstream `cbz` conditions.
            //
            // `ret_ty` determines which case applies:
            //   - `IrTy::Bool`: the expression is expected to produce a bool → logical NOT.
            //   - `IrTy::I32` or other: integer context → bitwise NOT.
            //
            // FLS §6.5.4: "The type of a negation expression is the type of the operand."
            // FLS §6.1.2:37–45: Both variants emit runtime instructions — no folding.
            //
            // Cache-line note: both `eor` (bool) and `mvn` (int) are 4 bytes.
            ExprKind::Unary { op: crate::ast::UnaryOp::Not, operand } => {
                if *ret_ty == IrTy::Bool {
                    // Logical NOT: lower operand as bool, emit BoolNot (eor).
                    // The operand is a bool expression (0 or 1); XOR with 1 flips it.
                    let val = self.lower_expr(operand, &IrTy::Bool)?;
                    let src = self.val_to_reg(val)?;
                    let dst = self.alloc_reg()?;
                    self.instrs.push(Instr::BoolNot { dst, src });
                    Ok(IrValue::Reg(dst))
                } else {
                    // Bitwise NOT: lower operand as i32, emit Not (mvn).
                    // FLS §6.5.4: `!n` for integers flips all bits.
                    let val = self.lower_expr(operand, &IrTy::I32)?;
                    let src = self.val_to_reg(val)?;
                    let dst = self.alloc_reg()?;
                    self.instrs.push(Instr::Not { dst, src });
                    Ok(IrValue::Reg(dst))
                }
            }

            // FLS §6.5.2: Dereference expression `*expr`.
            //
            // The operand must evaluate to a reference (pointer). Lower the
            // operand to obtain the address in a register, then emit `LoadPtr`
            // to load the value at that address.
            //
            // ARM64: `ldr x{dst}, [x{src}]` — register-indirect load.
            //
            // FLS §6.5.2: "A dereference expression also called a deref
            // expression is a unary operator expression that uses the
            // dereference operator."
            //
            // FLS §6.1.2:37–45: Runtime instruction — even if the reference
            // is statically known, the load must execute at runtime.
            //
            // Cache-line note: one 4-byte instruction. The load hits the cache
            // line that contains the referent on the stack.
            ExprKind::Unary { op: crate::ast::UnaryOp::Deref, operand } => {
                // Lower operand as I32 (a pointer is a register-width integer).
                let addr_val = self.lower_expr(operand, &IrTy::I32)?;
                let src = self.val_to_reg(addr_val)?;
                let dst = self.alloc_reg()?;
                self.instrs.push(Instr::LoadPtr { dst, src });
                Ok(IrValue::Reg(dst))
            }

            // FLS §6.5.1: Borrow expression `&place` or `&mut place`.
            //
            // Computes the address of the operand's stack slot and returns it
            // as a pointer value (a 64-bit integer on ARM64).
            //
            // ARM64: `add x{dst}, sp, #{slot * 8}` — forms the stack address.
            //
            // FLS §6.5.1: "A borrow expression also called a reference
            // expression is a unary operator expression that uses the borrow
            // operator."
            //
            // Supported place expressions (FLS §6.1.4):
            //   - Simple local variables: `&x`, `&mut x`
            //   - Named struct fields:    `&p.a`, `&mut p.a`
            //   - Tuple fields:           `&t.0`, `&mut t.0`
            //   - Chained field access:   `&p.inner.x`
            //
            // Resolution delegates to `resolve_place`, which recursively
            // converts the place expression to a flat stack-slot index.
            // Since all fields occupy contiguous numbered slots in the
            // function's stack frame, `add xD, sp, #(slot * 8)` is correct
            // for any depth of field access.
            //
            // FLS §6.1.2:37–45: Runtime instruction — the address is formed
            // at runtime even if the stack layout is statically known.
            //
            // Cache-line note: one 4-byte instruction. The resulting pointer
            // occupies one 8-byte register slot — same footprint as any integer.
            ExprKind::Unary {
                op: crate::ast::UnaryOp::Ref | crate::ast::UnaryOp::RefMut,
                operand,
            } => {
                // Resolve the operand to a stack slot via resolve_place, which
                // handles simple paths, named-field access, and tuple-field access.
                // FLS §6.1.4: Place expressions denote memory locations;
                // borrowing yields a pointer to the underlying slot.
                let (slot, _) = self.resolve_place(operand)?;
                let dst = self.alloc_reg()?;
                self.instrs.push(Instr::AddrOf { dst, slot });
                Ok(IrValue::Reg(dst))
            }

            // FLS §6.5.9: Type cast expression `expr as Ty`.
            //
            // A type cast expression converts the value of `expr` to the
            // target type. All numeric integer types are supported: signed
            // types lower with IrTy::I32 context (arithmetic: asr, sdiv),
            // unsigned types with IrTy::U32 context (arithmetic: lsr, udiv).
            //
            // FLS §6.5.9: Numeric casts. On ARM64 all integer types occupy a
            // 64-bit register. Same-width casts (i32↔u32, i64↔u64) are
            // identity at the register level — no instruction emitted. Widening
            // casts (i32→i64) are also identity since the register already
            // holds 64 bits. Narrowing casts (i64→i8) should truncate the upper
            // bits; galvanic does not yet emit explicit truncation — correct for
            // values within the target range.
            //
            // FLS §6.5.9 AMBIGUOUS: The spec says narrowing integer casts
            // truncate to the target type's bit width, but does not specify the
            // mechanism. Galvanic defers explicit truncation.
            //
            // FLS §6.1.2:37–45: The operand is lowered at runtime even if its
            // value is statically known — no constant folding.
            //
            // Cache-line note: identity casts emit zero instructions. The
            // source register is reused directly by the caller.
            ExprKind::Cast { expr: inner, ty } => {
                // Determine the target type name from the type path.
                let target_name = match &ty.kind {
                    crate::ast::TyKind::Path(segments) if segments.len() == 1 => {
                        segments[0].text(self.source)
                    }
                    _ => {
                        return Err(LowerError::Unsupported(
                            "cast to non-path type (only named types supported)".into(),
                        ));
                    }
                };

                match target_name {
                    // FLS §6.5.9: Signed integer targets.
                    // Includes bool → i32 (0/1 → 0/1 identity), all signed
                    // integer types. Narrowing (i64→i8, i64→i16) is identity
                    // at the register level for values within the target range.
                    "i8" | "i16" | "i32" | "i64" | "i128" | "isize" => {
                        self.lower_expr(inner, &IrTy::I32)
                    }

                    // FLS §6.5.9: Unsigned integer targets.
                    // Division uses `udiv` and right shift uses `lsr` when the
                    // result is subsequently used in arithmetic with U32 context.
                    // Narrowing casts (u64→u8, u64→u16) are identity for small
                    // values; truncation deferred (see FLS §6.5.9 AMBIGUOUS above).
                    "u8" | "u16" | "u32" | "u64" | "u128" | "usize" => {
                        self.lower_expr(inner, &IrTy::U32)
                    }

                    // FLS §6.5.9: Cast to bool: nonzero → true, zero → false.
                    // Not yet implemented — requires a comparison instruction.
                    "bool" => Err(LowerError::Unsupported(
                        "cast to bool not yet supported (FLS §6.5.9)".into(),
                    )),

                    // FLS §6.5.9: Floating-point targets not yet implemented
                    // (galvanic has no floating-point IR type or codegen).
                    "f32" | "f64" => Err(LowerError::Unsupported(
                        "cast to floating-point not yet supported (FLS §6.5.9)".into(),
                    )),

                    other => Err(LowerError::Unsupported(format!(
                        "cast to `{other}` not yet supported (FLS §6.5.9)"
                    ))),
                }
            }

            // FLS §6.13: Field access expression `receiver.field`.
            //
            // Supported form: the receiver must be a simple path expression
            // (single-segment) naming a local variable whose type was recorded
            // in `local_struct_types` when the struct literal was stored.
            //
            // Layout: struct variable `p` of type `Point { x: i32, y: i32 }`
            // stores `x` at slot `base` and `y` at slot `base + 1`. Field
            // access loads from `base + field_index`.
            //
            // Cache-line note: field access emits one `ldr` instruction (4 bytes).
            // For a struct with N fields, the Nth field load touches slot
            // `base + N - 1`; if base is cache-line-aligned, fields 0–7 fit
            // within one 64-byte cache line (8 × 8-byte slots).
            //
            // FLS §6.13: Field access expression `receiver.field`.
            //
            // Supports chained field access (`r.b.x`) via the `resolve_place`
            // helper, which recursively resolves the receiver and computes the
            // final stack slot using per-struct field offsets.
            //
            // FLS §6.13 AMBIGUOUS: The spec does not specify whether field
            // access on a temporary (non-place) expression is well-formed.
            // Galvanic restricts to named local variables and chained field
            // access — temporary struct values are not yet supported as receivers.
            //
            // Cache-line note: field access emits one `ldr` instruction (4 bytes).
            // For a struct with N fields, the Nth field load touches slot
            // `base + field_offset[N-1]`; if base is cache-line-aligned, all
            // scalar fields fit within the same 64-byte cache lines as their slots.
            ExprKind::FieldAccess { .. } => {
                // Use resolve_place to handle both simple (`p.x`) and chained
                // (`r.b.x`) field access in a uniform way.
                //
                // FLS §6.13: The result of a field access is the value stored in
                // the field's stack slot. For scalar fields (None type), emit `ldr`.
                // For struct-type fields (Some type), returning only the base slot
                // is correct for read access — the caller can further chain accesses.
                let (slot, _field_ty) = self.resolve_place(expr)?;
                let reg = self.alloc_reg()?;
                self.instrs.push(Instr::Load { dst: reg, slot });
                Ok(IrValue::Reg(reg))
            }

            // FLS §6.12.2 / §10.1: Method call expression `receiver.method(args)`.
            //
            // Supported form: the receiver must be a simple variable path whose
            // type was recorded in `local_struct_types`. The method is resolved
            // to a mangled function `TypeName__method_name`.
            //
            // Lowering strategy for `&self` and `self` methods: each struct field
            // is passed as an individual register argument (a value copy). This
            // matches the method's calling convention emitted by `lower_fn` when
            // `impl_type` is set. The method can read but not mutate the caller's
            // fields (correct for `&self`; sufficient for `self` at this milestone).
            //
            // ARM64 ABI: fields → x0..x{N-1}, extra args → x{N}..x{N+M-1}.
            // Return value in x0.
            //
            // FLS §6.12.2 AMBIGUOUS: The spec does not specify how many
            // auto-deref steps are legal. Galvanic restricts to zero deref steps
            // (receiver must already be the correct struct type).
            //
            // Cache-line note: loading N fields emits N `ldr` instructions (4 bytes
            // each). For a 2-field struct this is 8 bytes — fits in one cache line
            // alongside the `bl` instruction.
            ExprKind::MethodCall { receiver, method, args } => {
                // Resolve the receiver to a struct or enum variable's base slot and type.
                //
                // FLS §6.12.2: Method call expressions — `receiver.method(args)`.
                // FLS §10.1, §11: Methods may be defined on both struct and enum types.
                let (recv_base_slot, recv_type_name) = match &receiver.kind {
                    ExprKind::Path(segs) if segs.len() == 1 => {
                        let var_name = segs[0].text(self.source);
                        let base_slot =
                            self.locals.get(var_name).copied().ok_or_else(|| {
                                LowerError::Unsupported(format!(
                                    "undefined variable `{var_name}` in method call"
                                ))
                            })?;
                        // Check both struct and enum type registries.
                        // FLS §10.1, §11, §14.2: Check struct, enum, and tuple struct registries.
                        let type_name = self
                            .local_struct_types
                            .get(&base_slot)
                            .or_else(|| self.local_enum_types.get(&base_slot))
                            .or_else(|| self.local_tuple_struct_types.get(&base_slot))
                            .ok_or_else(|| {
                                LowerError::Unsupported(format!(
                                    "variable `{var_name}` is not a struct, enum, or tuple struct; \
                                     method calls on primitive types are not yet supported"
                                ))
                            })?
                            .clone();
                        (base_slot, type_name)
                    }
                    _ => {
                        return Err(LowerError::Unsupported(
                            "method call on non-variable receiver not yet supported".into(),
                        ));
                    }
                };

                // Build mangled function name: TypeName__method_name.
                let method_name = method.text(self.source);
                let mangled = format!("{recv_type_name}__{method_name}");

                // Load receiver fields into registers to pass as leading arguments.
                //
                // For struct receivers: one register per field (field 0 → x0, ...).
                // For enum receivers: discriminant in x0, then field registers x1..x{max_fields}.
                //
                // FLS §10.1: `self` is passed by value in declaration order, matching
                // the parameter-spill order in lower_fn.
                // FLS §6.1.2:37–45: All loads are runtime instructions.
                // Cache-line note: N loads = N × 4-byte `ldr` instructions.
                let mut arg_regs: Vec<u8> = Vec::with_capacity(8);
                let n_self_regs: usize;

                if let Some(field_names) = self.struct_defs.get(recv_type_name.as_str()).cloned() {
                    // Struct receiver: load each field.
                    let n_fields = field_names.len();
                    n_self_regs = n_fields;
                    for fi in 0..n_fields {
                        let slot = recv_base_slot + fi as u8;
                        let reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: reg, slot });
                        arg_regs.push(reg);
                    }
                } else if let Some(variants) = self.enum_defs.get(recv_type_name.as_str()).cloned() {
                    // Enum receiver: load discriminant then field registers.
                    //
                    // FLS §15: Enum calling convention — discriminant first, fields follow.
                    // FLS §10.1: `&mut self` on enums not yet supported (value semantics
                    // for mutation would require write-back of discriminant + all fields).
                    let max_fields = variants.values().map(|(_, names)| names.len()).max().unwrap_or(0);
                    n_self_regs = 1 + max_fields;
                    // Load discriminant.
                    let disc_reg = self.alloc_reg()?;
                    self.instrs.push(Instr::Load { dst: disc_reg, slot: recv_base_slot });
                    arg_regs.push(disc_reg);
                    // Load field registers.
                    for fi in 0..max_fields {
                        let slot = recv_base_slot + 1 + fi as u8;
                        let reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: reg, slot });
                        arg_regs.push(reg);
                    }
                } else if let Some(&n_fields) = self.tuple_struct_defs.get(recv_type_name.as_str()) {
                    // FLS §14.2, §10.1: Tuple struct receiver — load N fields from
                    // consecutive slots. Same register convention as named struct.
                    //
                    // FLS §6.1.2:37–45: All loads are runtime instructions.
                    // Cache-line note: N × 4-byte `ldr` instructions per method call.
                    n_self_regs = n_fields;
                    for fi in 0..n_fields {
                        let slot = recv_base_slot + fi as u8;
                        let reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: reg, slot });
                        arg_regs.push(reg);
                    }
                } else {
                    return Err(LowerError::Unsupported(format!(
                        "unknown type `{recv_type_name}` in method call"
                    )));
                }

                // Lower explicit arguments (left-to-right, FLS §6.4:14).
                for arg_expr in args {
                    let val = self.lower_expr(arg_expr, &IrTy::I32)?;
                    let reg = self.val_to_reg(val)?;
                    arg_regs.push(reg);
                }

                if arg_regs.len() > 8 {
                    return Err(LowerError::Unsupported(
                        "method call with more than 8 total arguments (exceeds ARM64 register window)".into(),
                    ));
                }

                // FLS §10.1: Check whether this is a `&mut self` method.
                // For struct receivers: emit `CallMut` which writes modified fields
                // back to the caller's struct slots after the `bl`.
                // For enum receivers: `&mut self` is not yet supported.
                //
                // FLS §6.12.2: Method call expressions are dispatched to the
                // mangled function `TypeName__method_name`.
                let is_mut_self = self.method_self_kinds.get(&mangled)
                    == Some(&SelfKind::RefMut);

                self.has_calls = true;
                // Guard: struct-returning &self methods cannot be used as scalar expressions.
                // The caller needs a destination slot (use in a `let` binding).
                //
                // FLS §10.1 AMBIGUOUS: No spec-defined way to discard a struct return.
                if self.struct_return_methods.contains_key(&mangled) {
                    return Err(LowerError::Unsupported(format!(
                        "`{mangled}` returns a struct; use it in a `let` binding"
                    )));
                }

                if is_mut_self {
                    // `&mut self` call: write back x0..x{n_self_regs-1} to receiver slots.
                    // The callee emits `RetFields` (or `RetFieldsAndValue` for scalar returns),
                    // placing modified field values in x0..x{N-1} before returning.
                    // We store them back here.
                    //
                    // If the method has a scalar return type, also capture x{N} as the result.
                    //
                    // FLS §10.1: Write-back convention — callee returns modified fields;
                    // caller stores them. Scalar return in x{N} for non-unit methods.
                    //
                    // Cache-line note: N write-back stores = N × 4-byte `str`.
                    // For a 2-field struct: 8 bytes — fits in half a cache line.
                    let has_scalar_return = self.mut_self_scalar_return_fns.contains(&mangled);
                    if has_scalar_return {
                        // `&mut self` method returning a scalar value.
                        // Emit CallMutReturn: write-back fields, then capture x{N} into dst.
                        let dst = self.alloc_reg()?;
                        self.instrs.push(Instr::CallMutReturn {
                            name: mangled,
                            args: arg_regs,
                            write_back_slot: recv_base_slot,
                            n_fields: n_self_regs as u8,
                            dst,
                        });
                        Ok(IrValue::Reg(dst))
                    } else {
                        self.instrs.push(Instr::CallMut {
                            name: mangled,
                            args: arg_regs,
                            write_back_slot: recv_base_slot,
                            n_fields: n_self_regs as u8,
                        });
                        Ok(IrValue::Unit)
                    }
                } else {
                    let dst = self.alloc_reg()?;
                    self.instrs.push(Instr::Call { dst, name: mangled, args: arg_regs });
                    Ok(IrValue::Reg(dst))
                }
            }

            // FLS §6.9: Indexing expression `base[index]`.
            //
            // Lowering strategy:
            // 1. Resolve `base` to a simple variable path and look up its stack slot.
            // 2. Lower `index` to a virtual register.
            // 3. Emit `LoadIndexed { dst, base_slot, index_reg }`.
            //
            // FLS §6.9: The base must be an array (or slice). Galvanic restricts
            // the base to a simple variable path whose type is a known array
            // (registered in `local_array_lens`).
            //
            // FLS §6.9 AMBIGUOUS: Out-of-bounds access must panic, but the panic
            // mechanism without the standard library is not specified. No bounds
            // check is emitted at this milestone.
            //
            // FLS §6.1.2:37–45: All instructions are runtime (no constant folding).
            //
            // Cache-line note: `LoadIndexed` emits two 4-byte instructions (add + ldr),
            // so an indexed read costs 8 bytes of instruction cache.
            ExprKind::Index { base, index } => {
                // Resolve the base to an array variable's stack slot.
                let base_slot = match &base.kind {
                    ExprKind::Path(segs) if segs.len() == 1 => {
                        let var_name = segs[0].text(self.source);
                        let slot = *self.locals.get(var_name).ok_or_else(|| {
                            LowerError::Unsupported(format!(
                                "undefined variable `{var_name}` in index expression"
                            ))
                        })?;
                        // Verify this variable is a known array.
                        if !self.local_array_lens.contains_key(&slot) {
                            return Err(LowerError::Unsupported(format!(
                                "variable `{var_name}` is not an array (indexing non-arrays not yet supported)"
                            )));
                        }
                        slot
                    }
                    _ => {
                        return Err(LowerError::Unsupported(
                            "index expression on non-variable base not yet supported".into(),
                        ));
                    }
                };

                // Lower the index expression to a register.
                // FLS §6.9: The index is of type `usize`; galvanic uses i32 here.
                let idx_val = self.lower_expr(index, &IrTy::I32)?;
                let index_reg = self.val_to_reg(idx_val)?;

                // Emit LoadIndexed: adds base_slot*8 to sp, then indexed ldr.
                let dst = self.alloc_reg()?;
                self.instrs.push(Instr::LoadIndexed { dst, base_slot, index_reg });
                Ok(IrValue::Reg(dst))
            }

            // FLS §6.10: Tuple expression as a value (not as a let initializer).
            // This path is reached when a tuple literal appears as a tail expression
            // or in a context where it's used as a value directly (rare; most tuple
            // usage goes through the `let` path above). Not yet supported — tuples
            // must be bound to a named variable first.
            ExprKind::Tuple(_) => Err(LowerError::Unsupported(
                "tuple expression must be bound to a `let` variable at this milestone".into(),
            )),

            // Anything else: not yet supported as runtime codegen.
            _ => Err(LowerError::Unsupported(
                "expression kind in non-const context (runtime codegen not yet implemented)".into(),
            )),
        }
    }
}
