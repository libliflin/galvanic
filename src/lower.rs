//! AST-to-IR lowering for galvanic.
//!
//! Translates a parsed `SourceFile` into the minimal IR needed for ARM64
//! code generation. Each lowering function corresponds to a FLS section.
//!
//! # FLS traceability
//!
//! - FLS §9: Functions — `lower_fn` maps each `FnDef` to an `IrFn`.
//! - FLS §6.19: Return expressions — tail expressions lower to `Instr::Ret`.
//! - FLS §6.2: Literal expressions — `LitInt` lowers to `IrValue::I32`.
//! - FLS §4.4: Unit type — absent tail / unit type lowers to `IrValue::Unit`.
//! - FLS §8.1: Let statements — processed into a constant environment.
//! - FLS §6.3: Path expressions — variable references resolved from env.
//! - FLS §6.17: If expressions — constant-folded when condition is a literal.
//! - FLS §6.4: Block expressions — evaluated as nested constant scopes.
//! - FLS §6.12.1: Call expressions — compile-time inlining of constant calls.
//! - FLS §6.5.1: Assignment expressions — `x = rhs` updates the env binding.
//! - FLS §6.5.3: Comparison operators — folded when operands are constant i32.
//! - FLS §6.15.2: Loop expressions — simulated at compile time.
//! - FLS §6.15.3: While loop expressions — simulated at compile time.
//! - FLS §6.15.6: Break expressions — propagated as `LowerError::Break`.
//! - FLS §6.15.7: Continue expressions — propagated as `LowerError::Continue`.
//! - FLS §8.2: Empty statements — no-op.
//! - FLS §8.3: Expression statements — assignment, while, loop, and if.
//! - FLS §18.1: Program structure — `lower` produces one `Module` per file.
//!
//! # Scope (milestone 9)
//!
//! Extends milestone 8 to support `continue` expressions (FLS §6.15.7) inside
//! `while` and `loop` bodies. `continue` is propagated up through `exec_stmts`
//! and `lower_expr` as `Err(LowerError::Continue)` until caught by the
//! enclosing `while` or `loop` handler. On `continue`, mutations of outer-scope
//! variables accumulated up to the continue point are propagated back, then the
//! next iteration begins (re-checking the while condition, or re-running the
//! loop body).
//!
//! Body variables that shadow outer bindings are scoped to the iteration: only
//! mutations of pre-existing outer variables propagate back after each iteration.
//!
//! Only calls to named functions (single-segment path callees) are supported.
//! Recursive calls that do not terminate at compile time will loop forever —
//! runtime call support (stack frames, branch-and-link) is deferred.

use std::collections::HashMap;

use crate::ast::{BinOp, Block, Expr, ExprKind, FnDef, ItemKind, SourceFile, Stmt, StmtKind, TyKind};
use crate::ir::{Instr, IrFn, IrTy, IrValue, Module};

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors that can occur during lowering.
#[derive(Debug)]
pub enum LowerError {
    /// A language feature used by the program is not yet implemented.
    Unsupported(String),
    /// Internal: a `break` expression was evaluated.
    ///
    /// During compile-time loop simulation, `Break` propagates up through
    /// `exec_stmts` and `lower_expr` via `?` or explicit matching until
    /// caught by the enclosing `ExprKind::Loop` handler. If it reaches
    /// `lower_block_return` it means `break` was used outside any loop —
    /// a compile error in well-formed Rust.
    ///
    /// FLS §6.15.6: Break expressions.
    Break(IrValue),

    /// Internal: a `continue` expression was evaluated.
    ///
    /// During compile-time loop simulation, `Continue` propagates up through
    /// `exec_stmts` and `lower_expr` via `?` or explicit matching until
    /// caught by the enclosing `while` or `loop` handler. On catching it, the
    /// handler propagates mutations accumulated up to the continue point and
    /// restarts the next iteration. If it reaches `lower_block_return` it means
    /// `continue` was used outside any loop — a compile error in well-formed Rust.
    ///
    /// FLS §6.15.7: Continue expressions.
    Continue,
}

impl std::fmt::Display for LowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LowerError::Unsupported(msg) => write!(f, "not yet supported: {msg}"),
            LowerError::Break(_) => write!(f, "break expression outside of a loop"),
            LowerError::Continue => write!(f, "continue expression outside of a loop"),
        }
    }
}

// ── Function table ────────────────────────────────────────────────────────────

/// A map from function name to its AST definition.
///
/// Built once from the `SourceFile` at the top of `lower()` and threaded
/// through all lowering functions so that call expressions can be inlined.
///
/// FLS §6.12.1: Call expressions resolve the callee by name lookup.
type FnTable<'ast> = HashMap<String, &'ast FnDef>;

// ── Entry point ───────────────────────────────────────────────────────────────

/// Lower a parsed source file to the IR.
///
/// FLS §18.1: A source file is a sequence of items. Each `fn` item is
/// lowered to an `IrFn`. Other item kinds (struct, enum) do not produce
/// code directly and are unsupported at this milestone.
pub fn lower(src: &SourceFile, source: &str) -> Result<Module, LowerError> {
    // Build the function table first so calls can be resolved during lowering.
    // FLS §6.12.1: Call expressions resolve the callee to a function definition.
    let mut fn_table: FnTable<'_> = HashMap::new();
    for item in &src.items {
        if let ItemKind::Fn(fn_def) = &item.kind {
            let name = fn_def.name.text(source).to_owned();
            fn_table.insert(name, fn_def.as_ref());
        }
    }

    let mut fns = Vec::new();

    for item in &src.items {
        match &item.kind {
            ItemKind::Fn(fn_def) => {
                // Functions with parameters cannot be lowered standalone because
                // their bodies reference parameter names that are only in scope
                // when inlined at a call site. They are evaluated via the Call
                // handler in lower_expr instead.
                if fn_def.params.is_empty() {
                    fns.push(lower_fn(fn_def, source, &fn_table)?);
                }
            }
            ItemKind::Struct(_) | ItemKind::Enum(_) => {
                return Err(LowerError::Unsupported(
                    "struct/enum items".into(),
                ));
            }
        }
    }

    Ok(Module { fns })
}

// ── Function lowering ─────────────────────────────────────────────────────────

/// Lower a single function definition to an `IrFn`.
///
/// FLS §9: Functions.
fn lower_fn(fn_def: &FnDef, source: &str, fn_table: &FnTable<'_>) -> Result<IrFn, LowerError> {
    let name = fn_def.name.text(source).to_owned();

    // FLS §9: "If no return type is specified, the return type is `()`."
    let ret_ty = match &fn_def.ret_ty {
        None => IrTy::Unit,
        Some(ty) => lower_ty(ty, source)?,
    };

    let body = match &fn_def.body {
        None => {
            return Err(LowerError::Unsupported(
                "extern / bodyless functions".into(),
            ));
        }
        Some(block) => lower_block_return(block, &ret_ty, source, fn_table)?,
    };

    Ok(IrFn { name, ret_ty, body })
}

// ── Type lowering ─────────────────────────────────────────────────────────────

/// Lower a type expression to an `IrTy`.
///
/// FLS §4: Types. Only `i32` and `()` are supported at this milestone.
fn lower_ty(ty: &crate::ast::Ty, source: &str) -> Result<IrTy, LowerError> {
    match &ty.kind {
        TyKind::Unit => Ok(IrTy::Unit),
        TyKind::Path(segments) if segments.len() == 1 => {
            match segments[0].text(source) {
                "i32" => Ok(IrTy::I32),
                name => Err(LowerError::Unsupported(format!("type `{name}`"))),
            }
        }
        _ => Err(LowerError::Unsupported("complex type".into())),
    }
}

// ── Block / expression lowering ───────────────────────────────────────────────

/// Lower a function body block to a list of IR instructions.
///
/// Builds a fresh constant environment (no enclosing scope), evaluates the
/// block, and wraps the result in a single `Ret` instruction.
///
/// FLS §6.4: Block expressions.
/// FLS §6.19: Return expressions — the tail is the block's return value.
fn lower_block_return(
    block: &Block,
    ret_ty: &IrTy,
    source: &str,
    fn_table: &FnTable<'_>,
) -> Result<Vec<Instr>, LowerError> {
    let env: HashMap<String, IrValue> = HashMap::new();
    let value = lower_block_value(block, ret_ty, source, &env, fn_table)?;
    Ok(vec![Instr::Ret(value)])
}

/// Evaluate a block expression to a compile-time constant value.
///
/// Processes statements via `exec_stmts`, then evaluates the tail expression.
/// Bindings introduced inside this block do not escape it (block scoping).
///
/// FLS §6.4: Block expressions.
/// FLS §8.1: Let statements — each `let x = expr;` binds `x` in the local env.
fn lower_block_value(
    block: &Block,
    expected_ty: &IrTy,
    source: &str,
    parent_env: &HashMap<String, IrValue>,
    fn_table: &FnTable<'_>,
) -> Result<IrValue, LowerError> {
    // Clone parent env so inner bindings don't leak out.
    let mut env = parent_env.clone();
    exec_stmts(&block.stmts, expected_ty, source, &mut env, fn_table)?;
    match &block.tail {
        None => Ok(IrValue::Unit),
        Some(expr) => lower_expr(expr, expected_ty, source, &mut env, fn_table),
    }
}

/// Execute a slice of statements, updating `env` in place.
///
/// This is the core statement-processing loop. It is called by
/// `lower_block_value` for function bodies and by the while-loop simulator
/// for loop body iterations. Taking `env` as `&mut` allows while body
/// mutations (assignments to outer-scope variables) to propagate back.
///
/// FLS §8: Statements.
/// FLS §8.1: Let statements.
/// FLS §8.2: Empty statements.
/// FLS §8.3: Expression statements — assignment (`x = rhs`) and while loops.
fn exec_stmts(
    stmts: &[Stmt],
    expected_ty: &IrTy,
    source: &str,
    env: &mut HashMap<String, IrValue>,
    fn_table: &FnTable<'_>,
) -> Result<(), LowerError> {
    for stmt in stmts {
        match &stmt.kind {
            // FLS §8.1: Let statement with initializer.
            //
            // A let without an initializer produces an uninitialized place.
            // We cannot constant-fold an uninitialized value; reject it.
            // (Use-before-init is caught by the borrow checker in full Rust.)
            StmtKind::Let { name, init, .. } => {
                let init_expr = init.as_ref().ok_or_else(|| {
                    LowerError::Unsupported("let binding without initializer".into())
                })?;
                let val = lower_expr(init_expr, expected_ty, source, env, fn_table)?;
                let binding_name = name.text(source).to_owned();
                env.insert(binding_name, val);
            }

            // FLS §8.2: Empty statement — lone `;` is a no-op.
            StmtKind::Empty => {}

            // FLS §8.3: Expression statement.
            StmtKind::Expr(expr) => match &expr.kind {
                // FLS §6.5.1: Assignment expression `name = rhs`.
                //
                // The lhs must be a single-segment path naming an in-scope
                // let binding. The rhs is evaluated in the current env and
                // the binding is updated.
                //
                // FLS §6.5.1 AMBIGUOUS: the FLS allows arbitrary place
                // expressions on the lhs of `=` (field access, indexing,
                // deref). This implementation restricts to simple variable
                // names; other place expressions are deferred.
                ExprKind::Binary { op: BinOp::Assign, lhs, rhs } => {
                    match &lhs.kind {
                        ExprKind::Path(segments) if segments.len() == 1 => {
                            let name = segments[0].text(source).to_owned();
                            // Infer expected type from the current binding.
                            let assign_ty = match env.get(&name) {
                                Some(IrValue::I32(_)) => IrTy::I32,
                                Some(IrValue::Unit) => IrTy::Unit,
                                None => {
                                    return Err(LowerError::Unsupported(format!(
                                        "assignment to undeclared variable `{name}`"
                                    )))
                                }
                            };
                            let new_val =
                                lower_expr(rhs, &assign_ty, source, env, fn_table)?;
                            env.insert(name, new_val);
                        }
                        _ => {
                            return Err(LowerError::Unsupported(
                                "assignment to non-variable target \
                                 (field, index, deref not yet supported)"
                                    .into(),
                            ))
                        }
                    }
                }

                // FLS §6.15.3: While loop expression — simulate at compile time.
                //
                // The condition is re-evaluated before each iteration using the
                // current outer env. The body runs in a child env (inner let
                // bindings are scoped to the iteration); mutations to
                // pre-existing outer variables are propagated back after each
                // iteration.
                //
                // FLS §6.15.3 AMBIGUOUS: The FLS specifies the semantics of
                // while loops at runtime but does not define how a compiler
                // should handle loops during compile-time evaluation. The step
                // limit below is a practical implementation choice; it will
                // become irrelevant once runtime loop codegen is added.
                ExprKind::While { cond, body } => {
                    // Snapshot outer keys so inner let-bindings don't escape.
                    let outer_keys: Vec<String> = env.keys().cloned().collect();
                    // Guard against infinite loops.
                    const MAX_ITER: u32 = 1000;
                    'while_sim: for step in 0..=MAX_ITER {
                        if step == MAX_ITER {
                            return Err(LowerError::Unsupported(
                                "while loop exceeded compile-time limit of 1000 \
                                 iterations (runtime loop support planned for a \
                                 future milestone)"
                                    .into(),
                            ));
                        }
                        // Re-evaluate condition in the current outer env.
                        if !lower_expr_as_bool(cond, source, env, fn_table)? {
                            break;
                        }
                        // Run body in a child env derived from the current outer env.
                        let mut body_env = env.clone();
                        match exec_stmts(&body.stmts, expected_ty, source, &mut body_env, fn_table) {
                            Ok(()) => {}
                            // FLS §6.15.6: `break` exits the while loop; while always
                            // evaluates to `()`. Propagate mutations up to break point.
                            Err(LowerError::Break(_)) => {
                                for key in &outer_keys {
                                    if let Some(&val) = body_env.get(key) {
                                        env.insert(key.clone(), val);
                                    }
                                }
                                break 'while_sim;
                            }
                            // FLS §6.15.7: `continue` skips the remainder of the loop
                            // body and re-checks the while condition. Propagate mutations
                            // accumulated up to the continue point, then restart.
                            Err(LowerError::Continue) => {
                                for key in &outer_keys {
                                    if let Some(&val) = body_env.get(key) {
                                        env.insert(key.clone(), val);
                                    }
                                }
                                continue 'while_sim;
                            }
                            Err(e) => return Err(e),
                        }
                        // Propagate mutations of outer-scope variables back.
                        for key in &outer_keys {
                            if let Some(&val) = body_env.get(key) {
                                env.insert(key.clone(), val);
                            }
                        }
                    }
                }

                // FLS §6.15.6: Break expression in statement position.
                //
                // Propagated as Err(LowerError::Break) so it travels up through
                // exec_stmts and lower_expr until caught by the enclosing
                // ExprKind::Loop handler. The break value is evaluated in the
                // current env using the outer expected type.
                ExprKind::Break(opt_val) => {
                    let val = match opt_val {
                        None => IrValue::Unit,
                        Some(e) => lower_expr(e, expected_ty, source, env, fn_table)?,
                    };
                    return Err(LowerError::Break(val));
                }

                // FLS §6.15.7: Continue expression in statement position.
                //
                // Propagated as Err(LowerError::Continue) so it travels up
                // through exec_stmts and lower_expr until caught by the enclosing
                // while or loop handler. The handler propagates mutations
                // accumulated up to this point and restarts the next iteration.
                ExprKind::Continue => {
                    return Err(LowerError::Continue);
                }

                _ => {
                    // General expression statement: evaluate and discard the result.
                    // Break signals (Err(LowerError::Break)) propagate naturally via `?`.
                    //
                    // This handles `if cond { break; }`, block expressions, call
                    // expressions, and other expressions used as statements.
                    //
                    // Note: mutations inside nested expressions evaluated via lower_expr
                    // (e.g., assignments in if branches) are scoped to lower_expr's
                    // env clone and do NOT propagate to the outer env. This is a known
                    // limitation of the compile-time simulation approach.
                    lower_expr(expr, expected_ty, source, env, fn_table)?;
                }
            },
        }
    }
    Ok(())
}

/// Evaluate an expression to a compile-time boolean.
///
/// Used to fold `if`/`else` conditions at compile time. Supports literal
/// booleans and comparison operators applied to constant `i32` operands.
///
/// FLS §2.4.7: Boolean literals `true` and `false`.
/// FLS §6.5.3: Comparison expressions.
fn lower_expr_as_bool(
    expr: &Expr,
    source: &str,
    env: &mut HashMap<String, IrValue>,
    fn_table: &FnTable<'_>,
) -> Result<bool, LowerError> {
    match &expr.kind {
        // FLS §2.4.7: Boolean literals.
        ExprKind::LitBool(b) => Ok(*b),

        // FLS §6.5.3: Comparison operators — folded at compile time when
        // both operands evaluate to constant `i32` values.
        //
        // The lhs and rhs are lowered using the `i32` expected type; boolean
        // comparison operators applied to non-integer values are deferred.
        ExprKind::Binary { op, lhs, rhs } => {
            let lhs_val = lower_expr(lhs, &IrTy::I32, source, env, fn_table)?;
            let rhs_val = lower_expr(rhs, &IrTy::I32, source, env, fn_table)?;
            match (op, lhs_val, rhs_val) {
                (BinOp::Lt, IrValue::I32(a), IrValue::I32(b)) => Ok(a < b),
                (BinOp::Le, IrValue::I32(a), IrValue::I32(b)) => Ok(a <= b),
                (BinOp::Gt, IrValue::I32(a), IrValue::I32(b)) => Ok(a > b),
                (BinOp::Ge, IrValue::I32(a), IrValue::I32(b)) => Ok(a >= b),
                (BinOp::Eq, IrValue::I32(a), IrValue::I32(b)) => Ok(a == b),
                (BinOp::Ne, IrValue::I32(a), IrValue::I32(b)) => Ok(a != b),
                _ => Err(LowerError::Unsupported(
                    "non-constant or unsupported comparison expression in if condition".into(),
                )),
            }
        }

        _ => Err(LowerError::Unsupported(
            "non-constant boolean expression in if condition \
             (runtime branches not yet supported)"
                .into(),
        )),
    }
}

/// Lower an expression to an `IrValue`.
///
/// `env` maps in-scope let bindings to their compile-time constant values.
/// `fn_table` maps function names to their AST definitions for call inlining.
///
/// FLS §6.2: Literal expressions.
/// FLS §6.3: Path expressions — single-segment paths resolved from `env`.
/// FLS §6.5: Arithmetic operator expressions.
/// FLS §6.12.1: Call expressions — inlined via compile-time evaluation.
fn lower_expr(
    expr: &Expr,
    expected_ty: &IrTy,
    source: &str,
    env: &mut HashMap<String, IrValue>,
    fn_table: &FnTable<'_>,
) -> Result<IrValue, LowerError> {
    match (&expr.kind, expected_ty) {
        // FLS §2.4.4.1: Integer literal narrowed to i32.
        (ExprKind::LitInt(n), IrTy::I32) => {
            // FLS §2.4.4.1: The value must fit in the target type.
            if *n > i32::MAX as u128 {
                return Err(LowerError::Unsupported(format!(
                    "integer literal {n} out of range for i32"
                )));
            }
            Ok(IrValue::I32(*n as i32))
        }

        // FLS §4.4: Unit literal `()`.
        (ExprKind::Unit, IrTy::Unit) => Ok(IrValue::Unit),

        // FLS §6.3: Path expression — look up a single-segment variable name.
        //
        // Multi-segment paths (e.g. `std::i32::MAX`) are not yet supported;
        // they require name resolution and are deferred to a later milestone.
        (ExprKind::Path(segments), _) if segments.len() == 1 => {
            let name = segments[0].text(source);
            env.get(name).copied().ok_or_else(|| {
                LowerError::Unsupported(format!(
                    "variable `{name}` not found (uninitialized, out of scope, or not a compile-time constant)"
                ))
            })
        }

        // FLS §6.5: Arithmetic binary operations on constant i32 operands.
        //
        // Both sub-expressions are lowered first; if they both reduce to
        // `IrValue::I32` the operation is folded at compile time. This covers
        // the milestone-2 target `fn main() -> i32 { 1 + 2 }`, and now also
        // expressions involving let-bound variables.
        //
        // FLS §6.23: Arithmetic overflow. The FLS states overflow behaviour is
        // implementation-defined. We use wrapping semantics here (matching
        // rustc's debug-mode behaviour) and document that choice.
        (ExprKind::Binary { op, lhs, rhs }, IrTy::I32) => {
            let lhs_val = lower_expr(lhs, expected_ty, source, env, fn_table)?;
            let rhs_val = lower_expr(rhs, expected_ty, source, env, fn_table)?;
            match (op, lhs_val, rhs_val) {
                (BinOp::Add, IrValue::I32(a), IrValue::I32(b)) => {
                    // FLS §6.5.5: Addition operator `+`.
                    Ok(IrValue::I32(a.wrapping_add(b)))
                }
                (BinOp::Sub, IrValue::I32(a), IrValue::I32(b)) => {
                    // FLS §6.5.5: Subtraction operator `-`.
                    Ok(IrValue::I32(a.wrapping_sub(b)))
                }
                (BinOp::Mul, IrValue::I32(a), IrValue::I32(b)) => {
                    // FLS §6.5.5: Multiplication operator `*`.
                    Ok(IrValue::I32(a.wrapping_mul(b)))
                }
                _ => Err(LowerError::Unsupported(
                    "non-constant or unsupported binary expression".into(),
                )),
            }
        }

        // FLS §6.4: Block expression — evaluate the block as a nested constant scope.
        //
        // A block expression `{ stmts... tail }` introduces a new scope; bindings
        // from `env` are visible inside but bindings introduced inside do not leak.
        (ExprKind::Block(block), _) => {
            lower_block_value(block, expected_ty, source, env, fn_table)
        }

        // FLS §6.17: If expression.
        //
        // At this milestone the condition must be a compile-time boolean literal.
        // The live branch is selected statically; the dead branch is not evaluated.
        // This covers `if true { 1 } else { 0 }` and `if false { 1 } else { 0 }`.
        //
        // FLS §6.17 AMBIGUOUS: the spec does not explicitly state what happens
        // when an `if` expression is used without an `else` branch in a value
        // position with a non-unit expected type. We treat absent `else` as
        // returning `IrValue::Unit` and defer the type mismatch to the type checker.
        (ExprKind::If { cond, then_block, else_expr }, _) => {
            let cond_bool = lower_expr_as_bool(cond, source, env, fn_table)?;
            if cond_bool {
                // Condition is true: evaluate the then-branch.
                lower_block_value(then_block, expected_ty, source, env, fn_table)
            } else {
                // Condition is false: evaluate the else-branch (if present).
                match else_expr {
                    Some(else_e) => lower_expr(else_e, expected_ty, source, env, fn_table),
                    None => Ok(IrValue::Unit),
                }
            }
        }

        // FLS §6.12.1: Call expression — compile-time inlining.
        //
        // The callee must be a single-segment path naming a function in this
        // module. Arguments are evaluated as constants, bound to the callee's
        // parameter names in a fresh environment, and the callee's body is
        // evaluated in that environment.
        //
        // This implements constant inlining rather than runtime call emission.
        // Runtime calls (stack frames, bl/ret pairs) are deferred to a later
        // milestone when runtime-variable values are needed.
        //
        // FLS §6.12.1 AMBIGUOUS: the spec describes call expressions but does
        // not specify the evaluation order of arguments. We evaluate left-to-right
        // following the convention established in §6.5 for binary operands.
        (ExprKind::Call { callee, args }, _) => {
            // Resolve the callee to a function name (single-segment path only).
            let callee_name = match &callee.kind {
                ExprKind::Path(segments) if segments.len() == 1 => {
                    segments[0].text(source)
                }
                _ => {
                    return Err(LowerError::Unsupported(
                        "call to non-path callee (closures, method objects not yet supported)".into(),
                    ));
                }
            };

            // Look up the callee in the module's function table.
            let callee_def = fn_table.get(callee_name).ok_or_else(|| {
                LowerError::Unsupported(format!(
                    "call to undefined or external function `{callee_name}`"
                ))
            })?;

            // Arity check.
            if args.len() != callee_def.params.len() {
                return Err(LowerError::Unsupported(format!(
                    "call to `{callee_name}`: expected {} argument(s), got {}",
                    callee_def.params.len(),
                    args.len()
                )));
            }

            // Evaluate each argument and bind it to the corresponding parameter.
            // FLS §9: Parameters are irrefutable patterns with declared types.
            let mut call_env: HashMap<String, IrValue> = HashMap::new();
            for (param, arg_expr) in callee_def.params.iter().zip(args.iter()) {
                let param_ty = lower_ty(&param.ty, source)?;
                let arg_val = lower_expr(arg_expr, &param_ty, source, env, fn_table)?;
                let param_name = param.name.text(source).to_owned();
                call_env.insert(param_name, arg_val);
            }

            // Evaluate the callee's body with the argument environment.
            let callee_ret_ty = match &callee_def.ret_ty {
                None => IrTy::Unit,
                Some(ty) => lower_ty(ty, source)?,
            };
            let body = callee_def.body.as_ref().ok_or_else(|| {
                LowerError::Unsupported(format!(
                    "call to bodyless (extern) function `{callee_name}`"
                ))
            })?;
            lower_block_value(body, &callee_ret_ty, source, &call_env, fn_table)
        }

        // FLS §6.15.2: Loop expression — execute body repeatedly until break.
        //
        // Compile-time simulation: the body is re-executed each iteration.
        // Mutations to variables that existed before the loop propagate back
        // after each iteration via the outer_keys tracking mechanism.
        //
        // The loop exits when exec_stmts (or the tail expression) returns
        // Err(LowerError::Break(v)); v becomes the loop expression's value.
        //
        // FLS §6.15.2 AMBIGUOUS: The FLS specifies that a `loop` without a
        // `break` diverges. The step limit below is an implementation choice;
        // it will become irrelevant once runtime loop codegen is added.
        (ExprKind::Loop(body), _) => {
            let outer_keys: Vec<String> = env.keys().cloned().collect();
            const MAX_ITER: u32 = 1000;
            for step in 0..=MAX_ITER {
                if step == MAX_ITER {
                    return Err(LowerError::Unsupported(
                        "loop exceeded compile-time limit of 1000 iterations \
                         (runtime loop support planned for a future milestone)"
                            .into(),
                    ));
                }
                let mut body_env = env.clone();
                // Execute loop body statements; Break propagates as Err.
                let break_val =
                    match exec_stmts(&body.stmts, expected_ty, source, &mut body_env, fn_table) {
                        Ok(()) => {
                            // Statements completed normally; check the tail expression.
                            match &body.tail {
                                None => None,
                                Some(tail_expr) => {
                                    match lower_expr(
                                        tail_expr,
                                        expected_ty,
                                        source,
                                        &mut body_env,
                                        fn_table,
                                    ) {
                                        Ok(_) => None, // tail ran without breaking
                                        Err(LowerError::Break(v)) => Some(v),
                                        // FLS §6.15.7: `continue` in tail position —
                                        // skip tail, propagate mutations, next iteration.
                                        Err(LowerError::Continue) => None,
                                        Err(e) => return Err(e),
                                    }
                                }
                            }
                        }
                        Err(LowerError::Break(v)) => Some(v),
                        // FLS §6.15.7: `continue` in stmt position — propagate
                        // mutations accumulated up to this point, restart iteration.
                        Err(LowerError::Continue) => None,
                        Err(e) => return Err(e),
                    };
                // Propagate mutations from this iteration back to the outer env.
                for key in &outer_keys {
                    if let Some(&val) = body_env.get(key) {
                        env.insert(key.clone(), val);
                    }
                }
                if let Some(v) = break_val {
                    return Ok(v);
                }
                // No break — continue to next iteration.
            }
            unreachable!() // Covered by MAX_ITER check above.
        }

        // FLS §6.15.6: Break expression in tail/value position.
        //
        // Propagated as Err(LowerError::Break) to travel up through
        // lower_block_value and lower_expr until caught by the enclosing
        // ExprKind::Loop handler. If it escapes to lower_block_return, the
        // program has a `break` outside any loop (compile error).
        //
        // The break value is evaluated using the outer expected type.
        // For bare `break`, the value is IrValue::Unit.
        (ExprKind::Break(opt_val), _) => {
            let val = match opt_val {
                None => IrValue::Unit,
                Some(e) => lower_expr(e, expected_ty, source, env, fn_table)?,
            };
            Err(LowerError::Break(val))
        }

        // FLS §6.15.7: Continue expression in tail/value position (never type).
        //
        // Propagated as Err(LowerError::Continue) to travel up through
        // lower_block_value and lower_expr until caught by the enclosing
        // while or loop handler. If it escapes to lower_block_return, the
        // program has a `continue` outside any loop (compile error).
        (ExprKind::Continue, _) => Err(LowerError::Continue),

        // Any other combination is not yet supported.
        _ => Err(LowerError::Unsupported("expression".into())),
    }
}
