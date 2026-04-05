//! Minimal intermediate representation for galvanic.
//!
//! The IR sits between the AST and code generation. It strips surface-syntax
//! detail (spans, source text) and exposes only the information the backend
//! needs to emit instructions.
//!
//! # Design (milestone 1)
//!
//! The IR is deliberately minimal: just enough to represent a function that
//! returns an integer constant. Each subsequent milestone extends it in the
//! direction required by the next target program.
//!
//! FLS §9: Functions — each `IrFn` corresponds to one function definition.
//! FLS §18.1: The top-level `Program` corresponds to one compiled crate.

use crate::ast::{ExprKind, ItemKind, SourceFile};

// ── IR types ──────────────────────────────────────────────────────────────────

/// A compiled program: a flat list of IR functions.
///
/// FLS §18.1: A crate is the compilation unit; a `Program` holds all
/// functions after lowering from the AST.
#[derive(Debug)]
pub struct Program {
    /// The functions in this program, in definition order.
    pub fns: Vec<IrFn>,
}

/// A function in the IR.
///
/// FLS §9: A function definition — name, parameters, body.
///
/// # Cache-line note
///
/// `name` is a heap-allocated `String`. Function names are accessed once
/// during emission and there are very few of them in early milestones. An
/// interned string table (`u32` indices into a flat byte buffer) would be
/// more cache-friendly at scale; that is future work.
#[derive(Debug)]
pub struct IrFn {
    /// The function name as it will appear in the assembly output.
    pub name: String,
    /// The function body as a flat list of IR instructions.
    pub body: Vec<IrInst>,
}

/// An IR instruction.
///
/// Milestone 1 supports one instruction: returning an integer constant.
/// Each subsequent milestone extends this enum as new language features
/// require new instruction forms.
#[derive(Debug)]
pub enum IrInst {
    /// Return an integer constant.
    ///
    /// FLS §6.19: Return expressions.
    /// FLS §6.2: Literal expressions — specifically §2.4.4.1 integer literals.
    ///
    /// The `i64` value is the return value. The codegen places it in `x0`
    /// (the AArch64 return-value / first-argument register) and invokes the
    /// Linux exit syscall.
    ReturnInt(i64),
}

// ── Lowering errors ───────────────────────────────────────────────────────────

/// An error encountered while lowering the AST to IR.
#[derive(Debug)]
pub struct LowerError {
    pub message: String,
}

impl std::fmt::Display for LowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "lower error: {}", self.message)
    }
}

// ── Lowering ──────────────────────────────────────────────────────────────────

/// Lower a parsed source file to the minimal IR.
///
/// FLS §9: For each function item, produce one `IrFn`.
/// FLS §18.1: The `main` function is the program entry point.
///
/// Milestone 1 limitations:
/// - Only functions named `main` are lowered; others are silently skipped.
/// - Only integer-literal tail expressions are handled.
/// - Bodies with statements are not yet supported.
pub fn lower(sf: &SourceFile, src: &str) -> Result<Program, LowerError> {
    let mut fns = Vec::new();

    for item in &sf.items {
        if let ItemKind::Fn(f) = &item.kind {
            let name = f.name.text(src).to_string();

            // Milestone 1: only lower `main`. Other functions are future work.
            if name != "main" {
                continue;
            }

            let body = lower_fn_body(f.body.as_ref(), src)?;
            fns.push(IrFn { name, body });
        }
    }

    Ok(Program { fns })
}

/// Lower a function body to a flat list of IR instructions.
fn lower_fn_body(
    body: Option<&crate::ast::Block>,
    src: &str,
) -> Result<Vec<IrInst>, LowerError> {
    let body = body.ok_or_else(|| LowerError {
        message: "function without body is not yet supported".to_string(),
    })?;

    // Milestone 1: only handle bodies with no statements and an integer tail.
    // FLS §8.1: Let statements and FLS §8.3: expression statements are
    // deferred to later milestones.
    if !body.stmts.is_empty() {
        return Err(LowerError {
            message: format!(
                "function body with {} statement(s) is not yet supported — \
                 milestone 1 handles only a single integer-literal return",
                body.stmts.len()
            ),
        });
    }

    let inst = match &body.tail {
        Some(tail) => lower_expr_to_return(tail, src)?,
        // Empty body → implicit `()` return → exit 0.
        // FLS §9: "If no return type is specified, the return type is `()`."
        None => IrInst::ReturnInt(0),
    };

    Ok(vec![inst])
}

/// Lower a tail expression to a `ReturnInt` instruction.
fn lower_expr_to_return(expr: &crate::ast::Expr, _src: &str) -> Result<IrInst, LowerError> {
    match &expr.kind {
        // FLS §2.4.4.1: Integer literals.
        // The AST stores u128; we narrow to i64 here, which covers all
        // integer types Rust programs realistically return from `main`.
        ExprKind::LitInt(n) => {
            // FLS §6.23: Arithmetic overflow — if the literal doesn't fit
            // in i64 (and was used as an exit code), the kernel will truncate
            // it to 8 bits anyway. We cast and let the OS handle the rest.
            Ok(IrInst::ReturnInt(*n as i64))
        }
        other => Err(LowerError {
            message: format!(
                "expression kind not yet supported in codegen: {:?} — \
                 milestone 1 handles only integer literals",
                std::mem::discriminant(other)
            ),
        }),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer, parser};

    fn lower_src(src: &str) -> Result<Program, LowerError> {
        let tokens = lexer::tokenize(src).expect("lex failed");
        let sf = parser::parse(&tokens, src).expect("parse failed");
        lower(&sf, src)
    }

    /// FLS §9: `fn main() -> i32 { 0 }` lowers to ReturnInt(0).
    #[test]
    fn milestone_1_fn_main_return_0() {
        let p = lower_src("fn main() -> i32 { 0 }").expect("lower failed");
        assert_eq!(p.fns.len(), 1);
        assert_eq!(p.fns[0].name, "main");
        assert!(matches!(p.fns[0].body[..], [IrInst::ReturnInt(0)]));
    }

    /// FLS §9: empty body lowers to ReturnInt(0) (implicit unit return → exit 0).
    #[test]
    fn empty_body_lowers_to_return_0() {
        let p = lower_src("fn main() {}").expect("lower failed");
        assert!(matches!(p.fns[0].body[..], [IrInst::ReturnInt(0)]));
    }

    /// Non-main functions are silently skipped in milestone 1.
    #[test]
    fn non_main_fn_skipped() {
        let p = lower_src("fn helper() -> i32 { 42 } fn main() -> i32 { 0 }")
            .expect("lower failed");
        assert_eq!(p.fns.len(), 1);
        assert_eq!(p.fns[0].name, "main");
    }

    /// Milestone 1: function body with statements is a LowerError.
    #[test]
    fn body_with_stmts_is_error() {
        let result = lower_src("fn main() -> i32 { let x = 1; x }");
        assert!(result.is_err(), "expected lowering error for body with stmts");
    }
}
