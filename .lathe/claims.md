# Load-Bearing Claims

These are the promises galvanic makes to its stakeholders. Each one, if broken,
would undermine the project's research value. The falsification suite in
`falsify.sh` tests these every cycle.

The runtime agent extends this list when new features create new promises.
Do not remove claims — if a claim is no longer testable, document why.

---

## Claim 1: Compiled programs generate runtime code, not compile-time constants

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: Galvanic is a compiler, not an interpreter. When it compiles
`fn main() -> i32 { 1 + 2 }`, the emitted assembly must contain an `add`
instruction — not `mov x0, #3`. The fact that inputs are statically known
does not justify constant folding for non-const code (FLS §6.1.2:37–45).

**Violated if**: `compile_to_asm("fn main() -> i32 { 1 + 2 }\n")` returns
assembly that contains `mov     x0, #3` instead of an `add` instruction.

**Test**: `cargo test --test e2e -- runtime_add_emits_add_instruction`

---

## Claim 2: Token is exactly 8 bytes

**Stakeholder**: Compiler Researchers, William

**Promise**: The `Token` struct is 8 bytes, enabling 8 tokens per 64-byte cache
line. Every cache-line comment in `lexer.rs` (and the design rationale in the
README) depends on this. A silent growth to 16 bytes invalidates the research
premise.

**Violated if**: `std::mem::size_of::<Token>() != 8`

**Test**: `cargo test --lib -- lexer::tests::token_is_eight_bytes`

---

## Claim 3: No unsafe code in library source

**Stakeholder**: Compiler Researchers, CI / Validation

**Promise**: Galvanic is written in safe Rust. `unsafe` blocks, `unsafe fn`,
and `unsafe impl` are forbidden in `src/` except `src/main.rs` (which shuts
out to the assembler/linker). This is verified by both CI `audit` job and the
falsification suite.

**Violated if**: `grep -rn 'unsafe\s*{' src/` finds matches outside `main.rs`,
or `grep -rn 'unsafe\s*fn\b' src/` finds matches outside `main.rs`.

**Test**: direct grep in falsify.sh

---

## Claim 4: The pipeline accepts valid milestone programs without panicking

**Stakeholder**: William, CI / Validation

**Promise**: Galvanic either succeeds or exits cleanly with a diagnostic. It
never panics (exits with a signal) on valid Rust programs. `fn main() {}` must
exit 0. `fn main() -> i32 { 42 }` must exit 0 (the galvanic process, not the
compiled program).

**Violated if**: Running `galvanic` on `fn main() {}` exits with code > 128
(indicating death by signal, i.e., a panic).

**Test**: runs galvanic binary on minimal inputs in falsify.sh

---

## Claim 5: FLS citations in source are structurally valid

**Stakeholder**: FLS / Ferrocene Ecosystem, Compiler Researchers

**Promise**: `FLS §X.Y` references in source code use real section numbers
from the specification table of contents. A citation that refers to a
nonexistent section is worse than no citation — it creates misleading
documentation.

**Current status**: This claim is difficult to falsify automatically without
fetching the spec (network dependency). It is enforced by code review and the
documented FLS TOC in `refs/fls-pointer.md`. The runtime agent is responsible
for checking citations against the TOC when adding or modifying code.

**Test**: Not currently automated. The runtime agent must verify manually.
When a mechanism for automated checking is possible (e.g., a local copy of
the spec's section list), add it here.

---

## Claim 6: Function calls with literal arguments emit branch instructions, not folded constants

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: Galvanic does not inline functions and constant-fold their results. When
a function is called with statically-known literal arguments, the call must emit `bl`
at the call site and compute the result at runtime. `square(6)` must emit `bl square`
and NOT `mov x0, #36`. This is a specific attack on Claim 1: a compiler could pass
the `1 + 2 → add` check while still folding function calls with known inputs.

**Violated if**: `compile_to_asm("fn square(x: i32) -> i32 { x * x }\nfn main() -> i32 { square(6) }\n")`
returns assembly containing `mov x0, #36` instead of `bl square`.

**Test**: `cargo test --test e2e -- runtime_fn_call_result_not_folded`

---

## Claim 7: Generic function and method calls emit runtime branch instructions, not folded constants

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: Galvanic's monomorphization path is a compiler pass, not a macro-evaluator.
When a generic function or method is called — even with statically-known literal arguments —
the call must produce runtime `bl` instructions to the monomorphized specialization, not
inline the body and constant-fold the result. `use_identity(7)` must emit `bl use_identity`
in main AND `bl identity__i32` inside it. This claim extends Claim 6 to the generic
monomorphization code path, which is a separate lowering pass from regular function calls.

**Violated if**: `compile_to_asm(...)` for `fn identity<T>(x: T) -> T { x }` / `use_identity(7)` 
returns assembly that lacks `bl use_identity` — indicating the outer call was inlined and
constant-propagated away without a runtime call.

**Red-team finding (2026-04-07)**: The original `runtime_generic_fn_not_folded` and
`runtime_generic_method_not_folded` tests had a vacuously weak negative assertion:
`!asm.contains("mov x0, #7") || asm.contains("ldr")`. Since any non-trivial ARM64
program contains `ldr` instructions, this condition was always true regardless of whether
folding occurred. The assertions were replaced with stronger positive checks: the outer
wrapper call (`bl use_identity`, `bl use_wrapper`) must be present in the assembly.

**Test**: `cargo test --test e2e -- runtime_generic_fn_not_folded runtime_generic_method_not_folded`

---

## Claim 8: Named block `break` emits an unconditional branch instruction

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: A `break 'label value` inside a named block expression must emit an
unconditional branch instruction (`b .Lxxx`) that exits the block. If the break
does not produce a branch, the code following the break point would execute
unconditionally — the block's early-exit semantics would be silently lost.

**Violated if**: `compile_to_asm(...)` for a function using `break 'work value`
returns assembly that does NOT contain `b .Lxxx` (an unconditional branch to a
generated exit label).

**Red-team finding (2026-04-07)**: The original assertion was `asm.contains('b')`,
which checks for the *character* `'b'` — vacuously true in any ARM64 program since
`bl`, `blr`, `cbz`, `sub`, and virtually every instruction or label name contains
that letter. The check was indistinguishable from a no-op. Replaced with a real
assertion: `asm.contains("b       .L") || asm.contains("b .L")`.

**Test**: `cargo test --test e2e -- runtime_named_block_emits_branch_not_folded`

---

## Claim 9: Generic trait impl calls emit monomorphized runtime branch, not folded constants

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: Galvanic's generic trait impl monomorphization produces runtime code. When
`impl<T> Trait for Type<T>` is used, the emitted assembly must contain (a) a label for
the monomorphized specialization (e.g., `Wrapper__get__i32`) and (b) a runtime `bl` to
the outer caller (not constant-folded away). This extends Claims 6–7 to the trait impl
code path, which combines the `generic_method_defs` pass with trait name resolution.

**Violated if**: `compile_to_asm(...)` for a generic trait impl + caller fails to contain
`Wrapper__get__i32` (monomorphization absent) or fails to contain `bl use_wrapper`
(outer call was constant-folded away).

**Test**: `cargo test --test e2e -- runtime_generic_trait_impl_emits_mangled_call`

---

## Claim 10: Default trait method dispatch emits a type-specific monomorphized label at runtime

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: When a trait provides a default method body, galvanic must emit a
type-specific monomorphized function — `Foo__doubled:` — NOT inline the body
at each call site or share a single generic body across types. The default
method body must be emitted as `TypeName__methodName`, and calls to it must
emit `bl TypeName__methodName` at the call site. Furthermore, the result must
not be constant-folded: calling a default method with a runtime value must
produce a real `bl` instruction, not a `mov` of the precomputed constant.

This claim guards the default method dispatch path (trait body resolution)
separately from the regular method path. Claims 6–9 cover regular functions,
generic functions, and generic trait impls — but none cover the case where the
method body comes from the trait definition itself, which is resolved differently
in the lowering pass.

**Violated if**: `compile_to_asm(...)` for a trait with a default method fails
to contain `Foo__doubled:` (monomorphized label absent) or fails to contain
`bl      Foo__doubled` (call not dispatched through monomorphized label), OR
contains `mov     x0, #42` when called with a runtime argument.

**Test**: `cargo test --test e2e -- runtime_default_method_emits_mangled_label runtime_default_method_result_not_folded`

---

## Claim 11: Closures compile to hidden function labels with runtime body instructions

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: Galvanic compiles closures to named hidden functions, not inline code. A closure
`|x: i32| -> i32 { x * 2 }` must produce a label like `__closure_*` in the assembly, and
the body of that hidden function must contain runtime instructions (e.g., `mul` for `x * 2`).

This guards two things simultaneously:
1. The closure is emitted as a callable function, not inlined/elided.
2. The closure body emits runtime instructions — not a constant folded from the
   statically-known operand `2`.

For capturing closures, the mechanism changes: the hidden function must still be emitted
(`__closure_main_0`) and the call site must use an indirect call (`blr`) to reach it via
the function pointer stored in the closure environment. If capturing closures regress to
direct inline expansion, the `blr` would disappear.

These two checks together guard both the non-capturing path (FLS §6.14) and the capturing
path (FLS §6.22) — the two closure archetypes in galvanic's current implementation.

**Violated if**: `compile_to_asm(...)` for a non-capturing closure fails to contain a
`__closure_*` label, or the closure body lacks a `mul` instruction for `x * 2`;
OR for a capturing closure, the assembly fails to contain `__closure_main_0` or `blr`.

**Test**: `cargo test --test e2e -- runtime_closure_emits_hidden_function_label runtime_capturing_closure_emits_capture_load_before_explicit_arg`

---

## Claim 12: Trait-bound generic function dispatch emits monomorphized runtime code

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: When a generic function with a trait bound is called — whether using
inline bounds (`fn f<T: Trait>(t: T)`) or where-clause bounds (`fn f<T>(t: T) where T: Trait`)
— galvanic must:
1. Emit a monomorphized function label: `apply_scale__Foo:` for concrete type `Foo`.
2. Inside that label, dispatch to the concrete type's method via a runtime `bl Foo__scale`.
3. Not constant-fold the result: calling with a runtime parameter must not produce a `mov`.

This guards the dispatch path introduced in milestones 139 and 140 (FLS §12.1, §4.14).
It is distinct from Claims 7 and 9:
- Claim 7: generic function without trait dispatch (data-only type param, e.g., `identity__i32`)
- Claim 9: generic *trait impl* for a generic type (e.g., `impl<T> Trait for Type<T>`)
- Claim 12: generic function with a *trait bound* that dispatches to the concrete type's method body

The code path in `lower.rs` that resolves `T = Foo` at call sites and routes `t.method()` to
`Foo__method` was added in cycle 17. A regression here would silence the method dispatch
entirely (calling an unmangeld label or no label) without being caught by Claims 7 or 9.

**Violated if**: `compile_to_asm(...)` for `fn apply_scale<T: Scalable>(t: T, n: i32)` fails to
contain `apply_scale__Foo:` (monomorphization absent), or fails to contain `bl Foo__scale`
(method dispatch absent), or contains `mov x0, #12` (constant-folded instead of runtime).

**Test**: `cargo test --test e2e -- runtime_trait_bound_result_not_folded`

---

## Claim 13: Associated constant used in runtime computation emits runtime add, not folded constant

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: When an associated constant (`Config::MAX`) is used in a computation that
also involves a function parameter, the result must not be constant-folded at the call
site. `fn compute(x: i32) -> i32 { x + Config::MAX }` called as `compute(5)` must:
1. Inline `Config::MAX` as an immediate (this is correct — constants are substituted per FLS §7.1:10).
2. Emit a runtime `add` instruction to combine it with parameter `x`.
3. NOT emit `mov x0, #15` — the sum must be computed at runtime, not folded.

This claim guards a specific interaction: constant inlining (step 1) is correct behavior,
but it must not cascade into constant-folding the entire expression when a runtime value
is also present. The attack vector is: galvanic inlines `Config::MAX = 10`, then at the
call site sees `compute(5)` with a literal, and folds `5 + 10 → 15`.

This is distinct from Claim 1 (inline `1 + 2` arithmetic) and Claim 6 (function calls with
literal args). Claim 13 specifically targets the *constant inlining + runtime combination*
path introduced in milestone 128 (FLS §10.3). A compiler that correctly handles Claims 1
and 6 could still regress on Claim 13 if associated constant inlining triggers a folding
optimization that doesn't apply to stack-loaded variables.

**Violated if**: `compile_to_asm(...)` for `fn compute(x: i32) -> i32 { x + Config::MAX }`
called from `main` as `compute(5)` returns assembly that:
- does NOT contain `add` (the runtime addition was optimized away), OR
- contains `mov x0, #15` or `mov     x0, #15` (constant-folded the sum).

**Test**: `cargo test --test e2e -- runtime_assoc_const_in_computation_not_folded`

---

## Claim 14: Field method calls on struct fields emit runtime `bl` to the concrete type's method

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: When a method is called on a field of a struct — `c.inner.get()` where
`inner: Counter` — galvanic must:
1. Emit a callable function body label (`Counter__get:`) for the concrete type's method.
2. Dispatch to it at runtime via `bl Counter__get`, not fold the result.
3. When the result is combined with a runtime parameter (`c.inner.get() * factor`), emit
   a runtime `mul` instruction and NOT fold the product to a constant.

This guards the `ExprKind::FieldAccess` arm in the method call receiver resolution added
in milestone 142 (cycle 23). Before this cycle, no falsification claim covered the field
access receiver path (`resolve_place` → field slot → concrete struct type → method dispatch).
A regression in `resolve_place` or the `FieldAccess` arm would not be caught by Claims 6–13.

This is distinct from all prior claims:
- Claim 7: generic function calls (data-only type param)
- Claim 9: generic trait impl (impl<T> for Type<T>)
- Claim 10: default trait methods
- Claim 12: trait-bound generic functions
None of these test the `receiver = field access` code path.

**Violated if**: `compile_to_asm(...)` for `fn run(c: Container) -> i32 { c.inner.get() }` fails
to contain `Counter__get:` (method body absent) or `bl Counter__get` (runtime dispatch absent);
OR for `fn scale(c: Container, factor: i32) -> i32 { c.inner.get() * factor }` returns assembly
that contains `mov x0, #12` (product 3*4 constant-folded) or lacks `mul`.

**Red-team finding (2026-04-07)**: The original negative assertion in `runtime_field_method_call_emits_bl_not_folded`
was `!asm.contains("mov x0, #7") || asm.contains("ldr")` — vacuously true since any ARM64 struct
program uses `ldr`. This is the same class of bug found in Claims 7 and 8. Fixed: replaced with
a positive assertion that `Counter__get:` label is emitted, and added the adversarial companion
`runtime_field_method_call_result_not_folded` that checks the combined-with-runtime-param path.

**Test**: `cargo test --test e2e -- runtime_field_method_call_emits_bl_not_folded runtime_field_method_call_result_not_folded`

---

## Claim 15: Multiple trait bounds monomorphize ALL methods for ALL concrete types

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: When a generic function with multiple bounds (`<T: Trait1 + Trait2>`) is
instantiated with TWO different concrete types, galvanic must monomorphize all bound
methods for EVERY concrete type — not just the first one.

Specifically: if `apply_both<T: Adder + Doubler>` is called with both `Foo` and `Bar`,
the emitted assembly must contain:
1. `Foo__add_one:` and `Foo__double:` — both of Foo's monomorphized method bodies.
2. `Bar__add_one:` and `Bar__double:` — both of Bar's monomorphized method bodies.
3. No constant-folding of either wrapper's result.

**Attack vector**: Galvanic's `pending_monos` accumulator could correctly handle the first
concrete type seen at a call site but fail to register all bound methods for a subsequent
type. The compile-and-run test would still produce the correct exit code if the omitted
label happened to be inlined elsewhere, but the assembly would be missing a function body —
a correctness hole for any call from a different context.

This is distinct from Claim 12 (single-type, single-bound trait dispatch) and from the
existing `runtime_multiple_bounds_emits_both_trait_calls` test (which only checks one
concrete type). The two-type case exercises a different code path in the
`pending_monos` accumulation loop.

**Violated if**: `compile_to_asm(...)` for `fn apply_both<T: Adder + Doubler>(x: T)`
called with both `Foo` and `Bar` via wrapper functions returns assembly that:
- lacks `Bar__add_one:` or `Bar__double:` (second type's methods not monomorphized), OR
- lacks `Foo__add_one:` or `Foo__double:` (first type's methods not monomorphized), OR
- contains `mov x0, #7` or `mov x0, #14` (wrapper results constant-folded).

**Test**: `cargo test --test e2e -- runtime_multiple_bounds_two_types_both_monomorphized`

---

## Claim 19: Galvanic exits non-zero when the lower pass fails

**Stakeholder**: CI / Validation, William (researcher)

**Promise**: When `galvanic::lower::lower` returns an error, galvanic must exit with a
non-zero exit code. It must NOT silently return 0 while producing no output. A zero exit
code is a contract: it tells `compile_and_run` that compilation succeeded and that the
output binary can be run.

**Background**: In cycle 36, the test `milestone_149_fn_mut_with_param` failed on CI with
"got 1, expected 10." The root cause was that a lower error caused galvanic to print
"note: skipping codegen" and `return` (exit 0). `compile_and_run` interpreted exit 0 as
success, then ran qemu against a nonexistent binary, which exited 1. The test saw exit 1
and produced a confusing failure. The cycle 36 fix repaired the specific lower error, but
the exit-code contract was still broken — any future lower error would silently produce the
same class of confusion.

**Violated if**: Running galvanic with `-o output` on a valid program that causes a lower
error exits with code 0 while `output` does not exist.

**Structural fix**: main.rs `lower` error handler changed from `return` (exit 0) to
`process::exit(1)` — a lower failure is a hard error, not a skippable warning.

**Test**: `cargo build` followed by running galvanic on a valid program without `-o`;
exit 0 implies the `.s` file was written (lower and codegen succeeded). Verified in
falsify.sh Claim 19.

---

## Claim 20: `@` binding patterns emit runtime sub-pattern checks before binding

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: When a pattern `n @ subpat` is matched (FLS §5.1.4), galvanic must:
1. Emit a runtime comparison to check `subpat` (e.g., `cmp` for a range or literal check).
2. Only install the binding `n` if the sub-pattern matches (conditional execution, not always-bind).
3. NOT constant-fold the body expression that uses `n`: `n @ 1..=5 => n * 2` with `x=3` must emit `mul`, not `mov x0, #6`.

This guards the full `at_bind` code path added in milestone 150 (cycle 39, FLS §5.1.4). The attack vector is:
- Removing or breaking the `Pat::Bound` lowering → lower error, galvanic exits 1, `compile_and_run` skips (caught by CI e2e).
- Binding `n` to a wrong value (e.g., always 0) → wrong exit code in compile-and-run tests.
- Omitting the sub-pattern check → the arm fires even when `x` is out of range → wrong exit code.
- Constant-folding through the binding → `n * 2` with `n=3` folds to `mov x0, #6` instead of emitting `mul`.

The assembly inspection tests catch the last case without requiring cross tools. They use a function parameter `x` as the scrutinee so that constant folding through the match is impossible even if galvanic tried.

**Violated if**: `compile_to_asm(...)` for `fn classify(x: i32) -> i32 { match x { n @ 1..=5 => n * 2, _ => 0 } }` returns assembly that:
- does NOT contain `cmp` (sub-pattern check absent), OR
- contains `mov     x0, #6` (result constant-folded, treating n=3 as compile-time known).

**Test**: `cargo test --test e2e -- runtime_bound_pattern_range_emits_cmp_and_binding runtime_bound_pattern_literal_emits_eq_check`

---

## Claim 22: while-let OR patterns emit runtime orr accumulation and cbz loop exit

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: When a `while let A | B | C = x { ... }` loop is compiled, galvanic must:
1. Emit `orr` to accumulate equality results across all alternatives — not just check the first alternative.
2. Emit `cbz` to branch out of the loop when no alternative matched (accumulated flag is 0).
3. Emit a back-edge `b .L` branch to the loop header (loop structure is runtime, not compile-time unrolled).
4. NOT constant-fold the loop counter when the scrutinee is derived from a function parameter.

This claim guards the while-let OR pattern code path added in milestone 154 (cycle 50,
FLS §5.1.11 + §6.15.4). The existing compile-and-run tests for milestone 154 verify
correct iteration counts — but those tests require QEMU on CI and don't run locally.
This claim provides local, cross-tool-free coverage of the same correctness property.

**Attack vector**: A regression that drops the OR accumulation loop and replaces it with
a single equality check against only the first alternative. Such a regression would:
- Make `while let 1 | 2 | 3 = x` behave like `while let 1 = x` (exits after first non-1 value)
- Be invisible locally (no assembly inspection test, no QEMU)
- Fail the compile-and-run tests on CI (wrong iteration count) but only with cross tools

The positive assertion (`orr` present) directly detects this regression without running the program.

This is distinct from Claims 20–21 (@ binding patterns, let-else): those guard match-arm and
let-else paths. This claim specifically covers the loop condition re-evaluation path in while-let,
which re-checks the pattern on every iteration.

**Violated if**: `compile_to_asm(...)` for `while let 1 | 2 | 3 = x { n += 1; x += 1; }` with
`x` derived from a function parameter `start` returns assembly that:
- lacks `orr` (OR accumulation dropped — only first alternative checked), OR
- lacks `cbz` (loop exit branch absent — loop runs forever or never), OR
- lacks `b .L` (back-edge absent — loop unrolled or eliminated at compile time).

**Test**: `cargo test --test e2e -- runtime_while_let_or_emits_orr_accumulation runtime_while_let_or_result_not_folded`

---

## Claim 40: For loops emit runtime control flow, not constant-folded results

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: When galvanic compiles a `for` loop over a statically-known range
(e.g., `for i in 0..5 { acc += i; }`), it must emit runtime control flow instructions —
`cbz` for the exit test, `add` for the loop body, and an unconditional `b` back-edge —
rather than constant-folding the accumulated result to `mov x0, #10`.

The adversarial threat: `0+1+2+3+4 = 10` is statically computable. A constant-folding
interpreter could evaluate the entire loop at compile time and emit a single `mov x0, #10`.
The exit code would still be correct (10), making the regression invisible to all
compile-and-run tests.

This claim guards the for-loop lowering path (FLS §6.15.1) and range expression materialization
(FLS §6.16). FLS §6.1.2 Constraint 1 applies: `fn main()` is not a const context. Even when
every operand is a literal, a for loop over a range must execute at runtime via branch
instructions, not be unrolled and constant-folded.

**Attack vectors**:
1. Evaluate the range `0..5` at compile time and unroll the loop → emit five adds
   followed by `mov x0, #10` (no back-edge, no cbz).
2. Recognize that `acc` starts at 0 and `i` increments by 1 → fold to arithmetic formula
   `n*(n-1)/2` → `mov x0, #10`.
3. Dropping the for-loop lowering entirely and calling a builtin sum function → different
   instruction pattern, incorrect for arbitrary loop bodies.

The negative assertion (`!asm.contains("mov     x0, #10")`) is the adversarial gate.
The positive assertions (cbz, add, b) verify the loop structure is present.

**Violated if**: `compile_to_asm("fn main() -> i32 { let mut acc = 0; for i in 0..5 { acc += i; } acc }\n")` returns assembly that:
- lacks `cbz` (no exit branch), OR
- lacks `add` (loop body not emitted), OR
- lacks back-edge `b ` (loop structure absent), OR
- contains `mov     x0, #10` (result constant-folded from compile-time loop evaluation).

**Test**: `cargo test --test e2e -- runtime_for_loop_emits_cmp_cbz_add_and_back_branch`

---

## Not Yet Claims (honest gaps)

These are promises the project will eventually make but cannot yet be falsified:

- **Correct runtime behavior for function parameters**: `fn f(x: i32) -> i32 { x + 1 }` called with x=5 must exit 6. This requires the cross toolchain and QEMU to run, which falsify.sh cannot use. It is covered by `cargo test --test e2e` on CI.

- **Arithmetic overflow behavior**: In non-const code at runtime, integer overflow must panic in debug mode and wrap in release mode (FLS §6.1.2:49–50). Galvanic does not yet enforce this — no bounds checking is emitted. When it does, add a claim here.

- **Unicode identifier handling**: FLS §2.3 requires NFC normalization for Unicode identifiers. Galvanic accepts non-ASCII identifiers but does not normalize them. This is a known gap, documented in `lexer.rs`.

---

## Claim 16: dyn Trait dispatch uses vtable indirection, not constant folding

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: When a `&dyn Trait` value is passed to a function and a method is called
on it, galvanic must emit a vtable indirect call (`blr`) rather than resolving the
method statically or constant-folding the result.

Specifically: for `fn print_area(s: &dyn Shape) -> i32 { s.area() }` called with
`Circle { r: 5 }`, the emitted assembly must:
1. Contain the vtable label `vtable_Shape_Circle` in `.rodata`.
2. Contain `blr` for the indirect method dispatch.
3. NOT contain `mov x0, #25` (constant-folded result of 5*5=25).

**Attack vector**: A naive optimizer could detect that the only concrete type passed
to `print_area` is `Circle` and devirtualize the call, substituting the direct call
`Circle__area` or even folding `5*5` to `25`. This would defeat the purpose of
`dyn Trait` — runtime polymorphism — and would break programs where multiple concrete
types are passed to the same dyn Trait parameter.

This is distinct from Claim 12 (impl Trait static dispatch) which SHOULD monomorphize.
`dyn Trait` must preserve vtable dispatch even when the concrete type is known at
the call site.

**Violated if**: `compile_to_asm(DYN_TRAIT_BASIC)` returns assembly that:
- lacks `vtable_Shape_Circle` (vtable not emitted), OR
- lacks `blr` (no indirect dispatch), OR
- contains `mov x0, #25` (result constant-folded).

**Test**: `cargo test --test e2e -- milestone_147_dyn_trait_asm_inspection`

---

## Claim 17: When two concrete types are used behind the same dyn Trait parameter, BOTH vtables must be emitted

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: Galvanic's vtable accumulation pass (`pending_vtables`) must register ALL
(trait, concrete_type) pairs encountered across all call sites — not just the first.
When `print_area(&dyn Shape)` is called with both `Circle` and `Square`, the assembly
must contain both `vtable_Shape_Circle` AND `vtable_Shape_Square` in `.rodata`.

Claim 16 only tests a single concrete type. This claim extends coverage to the
multi-type case, which exercises a different code path: the accumulation loop that
deduplicates vtable requirements must correctly handle multiple entries without
dropping any after the first.

**Attack vector**: The `pending_vtables` accumulator could correctly register the
first (trait, concrete_type) pair but silently discard subsequent pairs — e.g., via
an off-by-one in a deduplication check, an early `return`, or a shadowed `insert`
call. The single-type test (Claim 16) would still pass. The compile-and-run test
(`milestone_147_dyn_trait_two_concrete_types`) might still produce the correct exit
code if the second vtable's shim happened to read correct memory by coincidence —
but the label would be absent, breaking any other call context.

Furthermore, neither method result must be constant-folded:
- Circle { r: 3 }.area() = 9 → must NOT see `mov x0, #9`
- Square { side: 4 }.area() = 16 → must NOT see `mov x0, #16`

**Violated if**: `compile_to_asm(...)` for `print_area(&c) + print_area(&sq)` with
two concrete types returns assembly that:
- lacks `vtable_Shape_Circle` (first vtable absent), OR
- lacks `vtable_Shape_Square` (second vtable absent), OR
- lacks `blr` (no indirect dispatch), OR
- contains `mov x0, #9` or `mov x0, #16` (either method result was constant-folded).

**Test**: `cargo test --test e2e -- runtime_dyn_trait_two_concrete_types_both_vtables_emitted`

---

## Claim 18: The second method in a dyn Trait vtable is accessed at offset 8, not offset 0

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: When a trait has two methods, galvanic lays out their fn-ptrs in the vtable at
offsets 0 and 8 (8 bytes per slot, in trait declaration order). Calling the SECOND method
(index 1) must emit `ldr x10, [x9, #8]` — NOT `ldr x10, [x9, #0]` (which would dispatch to
the FIRST method regardless of which method was called).

This is adversarial against a specific implementation bug: if `method_idx` lookup in the
`trait_method_order` map silently returns 0 for all methods, every vtable dispatch would call
method 0, producing wrong behavior only when method 1 is actually called. The two-method
compile-and-run test (`milestone_147_dyn_trait_two_method_vtable`) catches this at runtime on
CI — but requires qemu. This claim catches it in assembly without cross tools.

Galvanic's vtable layout (FLS §4.13 AMBIGUOUS — layout is implementation-defined):
- Dense `.rodata` array of 8-byte fn-ptrs in trait declaration order.
- Method at declaration index i → vtable offset i * 8.
- First method: offset 0; second method: offset 8.

**Violated if**: `compile_to_asm` for a two-method trait where the second method is called
returns assembly that:
- lacks `ldr x10, [x9, #8]` (second method loaded at wrong offset), OR
- lacks `ldr x10, [x9, #0]` (first method offset incorrect), OR
- lacks `blr` (no indirect dispatch), OR
- contains `mov x0, #12` (result of 3*4 constant-folded).

**Test**: `cargo test --test e2e -- runtime_dyn_trait_second_method_emits_vtable_offset_8`

---

## Claim 21: let-else patterns emit runtime discriminant checks and do not constant-fold the extracted binding

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: When a `let Enum::Variant(v) = expr else { diverge }` statement is compiled,
galvanic must:
1. Emit a runtime discriminant comparison and conditional branch (`cbz`) to the else block —
   not assume the pattern always matches.
2. Load the bound value `v` from the enum's field slot at runtime — not substitute a compile-time
   constant even when the call site passes a literal (`Opt::Some(5)`).
3. When the extracted binding is used in computation with a second runtime parameter (`v + n`),
   emit a runtime `add` instruction and NOT fold the result to a constant.

This claim guards the let-else lowering path added in milestone 153 (cycle 43, FLS §8.1). The
attack vectors are:
- Removing the discriminant check → else block never reached (wrong runtime behavior when pattern fails).
- Constant-folding through the enum field load → `v` always has the compile-time value from the
  call site literal, not the runtime value of `o`'s payload.
- Constant-folding `v + n` when `n` is also known at the call site → result is `mov x0, #N` instead
  of a runtime `add`.

The adversarial test (`runtime_let_else_binding_combined_with_param_not_folded`) uses TWO function
parameters — one for the enum and one for the addend — making the result impossible to fold for any
correct compiler. A folded result (`mov x0, #7`) is conclusive evidence of an interpreter.

**Violated if**: `compile_to_asm(...)` for `fn compute(o: Opt, n: i32) -> i32 { let Opt::Some(v) = o else { return 0 }; v + n }` returns assembly that:
- lacks `cbz` (discriminant check absent), OR
- lacks `add` (field load + addition was constant-folded), OR
- contains `mov x0, #7` (3+4 was folded at the call site).

**Test**: `cargo test --test e2e -- runtime_let_else_emits_discriminant_check runtime_let_else_binding_not_folded runtime_let_else_binding_combined_with_param_not_folded`

---

## Claim 23: while-let OR patterns with enum variants emit runtime orr accumulation

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: When a `while let Enum::A | Enum::B = s { ... }` loop is compiled with an
enum-variant OR pattern, galvanic must emit `orr` to accumulate discriminant equality
results across all variants — not just check the first variant. The exit condition must
emit `cbz`. The loop body must not be constant-folded even when the call site passes a
literal enum value.

This extends Claim 22 (scalar literals in while-let OR) to the enum-variant case. The
code path diverges: scalar OR patterns compare integer values; enum-variant OR patterns
compare discriminants. A regression could drop OR accumulation for the enum-variant path
while leaving the scalar path intact.

**Attack vector**: Dropping OR accumulation for enum variants makes `while let A | B = s`
behave like `while let A = s`. A call with `Status::Pending` would exit immediately instead
of executing the body — wrong behavior, invisible without assembly inspection locally.

**Violated if**: `compile_to_asm(...)` for `while let Status::Active | Status::Pending = s { return 1; }` returns assembly that:
- lacks `orr` (OR accumulation for enum variants dropped), OR
- lacks `cbz` (loop exit branch absent), OR
- contains `mov     x0, #1\n\tret` (result constant-folded for the enum-variant case).

**Test**: `cargo test --test e2e -- runtime_while_let_or_enum_emits_orr_accumulation`

---

## Claim 24: match guard predicates with function parameters emit runtime comparison code

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: When a match arm has a guard (`n if n > 5`), the guard condition must be
evaluated at runtime via a comparison instruction. This must hold even when the match
scrutinee is a function parameter (not a local literal) — the FLS §6.1.2 litmus test.

The existing `runtime_match_guard_emits_cbz_for_guard_condition` uses `let x = 7` as the
scrutinee (a local variable with a literal initializer). This claim uses a function
parameter, which is the definitive "compiler not interpreter" test: if replacing a literal
with a parameter would break the implementation, galvanic is interpreting, not compiling.

The adversarial scenario: for `fn guarded(n: i32) -> i32 { match n { x if x > 5 => x + 10, _ => 0 } }`,
a folding interpreter would evaluate `guarded(7)` at compile time:
1. `7 > 5` → true → take arm 1
2. `7 + 10` → 17
3. Emit `mov x0, #17; ret` in main

The negative assertion (`!asm.contains("mov     x0, #17")`) directly catches this.

**Violated if**: `compile_to_asm(...)` for `fn guarded(n: i32) -> i32 { match n { x if x > 5 => x + 10, _ => 0 } }` called as `guarded(7)` returns assembly that:
- lacks `cmp` or `cset` (guard comparison absent — guard not evaluated at runtime), OR
- lacks `cbz` or `cbnz` (conditional branch absent — guard not tested at runtime), OR
- contains `mov     x0, #17` (guard result constant-folded to the literal call-site value).

**Test**: `cargo test --test e2e -- runtime_match_guard_with_param_emits_runtime_comparison`

---

## Claim 25: let-else with mixed-kind OR alternatives emits runtime check for all alternatives

**Stakeholder**: William (researcher), FLS / Ferrocene Ecosystem

**Promise**: When a `let-else` statement has an OR pattern mixing kinds (literal `|` range),
both alternatives are evaluated at runtime via OR accumulation. For example:

```rust
fn classify(n: i32) -> i32 {
    let 1 | 10..=20 = n else { return 0 };
    1
}
```

The pattern `1 | 10..=20` must emit:
- A literal equality check (n == 1)
- A range check (10 ≤ n ≤ 20)
- An `orr` to accumulate both results
- A `cbz` to branch to the else block if neither matched

**Why this claim matters**: Cycle 59 added `accum_or_alt` to the let-else lowering path,
but tests covered only scalar-literal OR and enum-variant OR in let-else — NOT mixed kinds.
The parser's `parse_let_pattern` function had a separate `LitInteger` branch that did NOT
check for `..=`/`..` after the literal, silently returning `Pat::LitInt(10)` and leaving
`..=20` as the next token (causing "expected Semi, found DotDotEq").

This cycle fixed the parser bug and registered the claim so any regression
(in parser or lowerer) is caught adversarially.

**Attack vector**: Reverting the parser fix causes `let 1 | 10..=20 = n else { ... }` to
fail at parse time with a confusing "expected Semi" error — valid Rust rejected by galvanic.
Alternatively, if `accum_or_alt` is not called for the range alternative in let-else,
`classify(15)` would exit 0 (range check skipped, only literal checked, 15 ≠ 1 → else taken).

**Violated if**: `compile_to_asm(...)` for `fn classify(n: i32) -> i32 { let 1 | 10..=20 = n else { return 0 }; 1 }` called with `classify(15)` returns assembly that:
- causes a parse failure (parser bug not fixed), OR
- lacks `orr` (range alternative not OR-accumulated), OR
- lacks `cbz` (no else-branch on no-match), OR
- contains `mov     x0, #1\n\tret` (result constant-folded).

**Test**: `cargo test --test e2e -- runtime_let_else_or_mixed_emits_orr_accumulation`

---

## Claim 26: `@` binding patterns in let-else emit runtime sub-pattern check and binding

**Stakeholder**: William (researcher), FLS / Ferrocene Ecosystem

**Promise**: When a `let-else` statement uses an `@` binding pattern (e.g., `let n @ 1..=5 = x else { return 0 }`),
galvanic emits:
- A runtime sub-pattern check (range check: `cmp` instructions) — not constant-folded
- A runtime conditional branch (`cbz`) to the else block on mismatch
- A runtime binding of the scrutinee value to `n` (ldr + str)
- A runtime use of `n` in subsequent expressions (e.g., `n * 2` emits `mul`/`add`, NOT `mov x0, #6`)

For example:
```rust
fn f(x: i32) -> i32 {
    let n @ 1..=5 = x else { return 0 };
    n * 2
}
fn main() -> i32 { f(3) }
```

`f(3)` must emit a range check for `1..=5`, bind `x` (= 3) to `n`, then multiply `n * 2`.
Result is 6 at runtime. An interpreter would emit `mov x0, #6` directly.

**Why this claim matters**: Cycle 61 unified `parse_let_pattern` into `parse_single_pattern`,
which as a side-effect enabled `@` patterns to parse in `let-else` position. Cycle 62 implements
the lowering path. Without the lowering, parsed programs would fail at runtime with an
`Unsupported` error. Without this claim, a regression that re-introduces the `Unsupported`
catch-all (or that constant-folds the range check) would go undetected.

**Attack vector**:
1. Restoring the `_ => Unsupported` catch-all before `Pat::Bound` causes any `let n @ pat = x else` program to fail at compile time with "let-else only supports TupleStruct or OR patterns".
2. Constant-folding `f(3)` emits `mov x0, #6` — assembly contains the literal result with no runtime check.
3. Skipping the `CondBranch` means `else` is never taken even on a mismatch (out-of-range values are bound instead of diverging).

**Violated if**: `compile_to_asm(...)` for `fn f(x: i32) -> i32 { let n @ 1..=5 = x else { return 0 }; n * 2 }` returns assembly that:
- lacks `cmp` (range check not emitted), OR
- lacks `cbz` (else-branch not emitted), OR
- lacks `mul`/`add` (binding result not used at runtime), OR
- contains `mov     x0, #6` (result constant-folded).

**Test**: `cargo test --test e2e -- runtime_let_else_bound_pattern_emits_cmp_and_binding_not_folded`

---

## Claim 27: @ binding with OR sub-pattern emits orr accumulation (not folded)

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: `n @ (pat1 | pat2)` in any pattern position (let-else, if-let, while-let, match)
must emit `orr` for the OR alternative accumulation and must not constant-fold the binding.

For example:
```rust
fn f(x: i32) -> i32 {
    let n @ (1 | 5..=10) = x else { return 0 };
    n * 2
}
fn main() -> i32 { f(6) }
```

`f(6)` must emit OR-alternative checks (equality check for `1`, range check for `5..=10`),
accumulate results via `orr`, branch to else on no match, bind `n = 6`, then emit runtime
multiply. An interpreter would emit `mov x0, #12` directly.

**Why this claim matters**: Cycle 63 extends the `Pat::Bound` lowering to accept `Pat::Or`
as the sub-pattern (via `accum_or_alt`). Without this, `n @ (1 | 5..=10)` emits
`Unsupported` at compile time. Without this claim, a regression that removes the `Pat::Or`
arm from `accum_or_alt` (or that constant-folds the OR check) would go undetected.

**Attack vector**:
1. Removing the `Pat::Or` arm from `accum_or_alt` causes any `n @ (pat1 | pat2)` program
   to fail with "unsupported pattern kind inside OR pattern alternative".
2. Constant-folding `f(6)` emits `mov x0, #12` — assembly contains the literal result.
3. Skipping `orr` means only one alternative is checked (the other is silently ignored).

**Violated if**: `compile_to_asm(...)` for `fn f(x: i32) -> i32 { let n @ (1 | 5..=10) = x else { return 0 }; n * 2 }` returns assembly that:
- lacks `orr` (OR accumulation not emitted), OR
- contains `mov     x0, #12` (result constant-folded).

**Test**: `cargo test --test e2e -- runtime_at_bound_or_subpat_emits_orr_accumulation runtime_at_bound_or_subpat_result_not_folded`

---

## Claim 28: @ binding with OR sub-pattern in if-let and match positions emit runtime orr accumulation

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: `n @ (pat1 | pat2)` in **if-let** and **match** positions (not just let-else) must emit
`orr` for OR alternative accumulation and must NOT constant-fold the bound value.

Claim 27 covers the let-else lowering path. The if-let and match lowering paths for `Pat::Bound`
with `Pat::Or` sub-pattern are **distinct code paths** in `lower.rs` — a regression in one
would not be caught by Claim 27's tests.

For example:

```rust
// if-let position:
fn f(x: i32) -> i32 { if let n @ (1 | 5..=10) = x { n * 2 } else { 0 } }
fn main() -> i32 { f(6) }

// match position:
fn f(x: i32) -> i32 { match x { n @ (1 | 5..=10) => n * 2, _ => 0 } }
fn main() -> i32 { f(6) }
```

Both must emit `orr` and must NOT emit `mov x0, #12` (the constant-folded result).

**Why this claim matters**: Cycle 63 added the `Pat::Or` arm in the if-let `Pat::Bound` handler
(line ~10466) and match `Pat::Bound` handler. These are separate from the let-else path tested
by Claim 27. The milestone 158 tests for if-let and match positions are all compile-and-run
(require QEMU on CI); without this claim, a regression in those paths would be invisible locally
and would only surface on CI.

**Attack vector**:
1. Removing the `Pat::Or` arm from the if-let `Pat::Bound` handler causes any
   `if let n @ (p1 | p2) = x { ... }` to fail with "@ binding sub-pattern not yet supported in if-let".
2. Removing the `Pat::Or` arm from the match `Pat::Bound` handler causes similar failure in match.
3. Constant-folding `f(6)` in either position emits `mov x0, #12`.
4. Dropping OR accumulation in either position means only the first alternative is checked
   (e.g., `n @ (1 | 5..=10)` would only check `= 1`, silently failing for `x = 6`).

**Violated if**: `compile_to_asm(...)` for the if-let or match programs above returns assembly that:
- lacks `orr` (OR accumulation dropped for that position), OR
- contains `mov     x0, #12` (result constant-folded for `n*2` with `n=6`).

**Test**: `cargo test --test e2e -- runtime_at_bound_or_subpat_if_let_emits_orr_not_folded runtime_at_bound_or_subpat_match_emits_orr_not_folded`

---

## Claim 29: Struct-returning match with parameter-dependent fields emits runtime add (not folded)

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: A struct-returning function whose match arm computes fields from a parameter
(`n + 10`, `n * 3`) must emit a runtime `add` instruction and must NOT constant-fold the
result to a `mov #N` immediate, even when the caller passes a literal argument.

```rust
struct Pair { a: i32, b: i32 }
fn make(n: i32) -> Pair {
    match n {
        1 => Pair { a: n + 10, b: n * 3 },
        _ => Pair { a: 0, b: 0 },
    }
}
fn main() -> i32 { make(1).a }
```

Must emit `cmp` (runtime match scrutinee comparison), `add` (runtime `n + 10`), and must NOT
emit `mov x0, #11` (the constant-folded value of `make(1).a`).

**Why this claim matters**: Struct-returning match is a compound codegen path: match lowering,
struct field storage, and function return convention all interact. A regression that folds any
one of these to compile-time constants produces the correct exit code while violating the
runtime-codegen invariant.

**Attack vector**:
1. Constant-folding `make(1)` at the call site emits `mov x0, #11` (the `.a` field) directly.
2. Treating `n` as a const-context value within the match arm folds `n + 10` to `#11`.
3. Dropping the match scrutinee comparison means the wrong arm body is executed silently.

**Violated if**: `compile_to_asm(...)` for the program above returns assembly that:
- lacks `cmp` (match scrutinee comparison dropped), OR
- lacks `add` (`n + 10` was folded), OR
- contains `mov x0, #11` (constant-folded result of `make(1).a`).

**Test**: `cargo test --test e2e -- runtime_struct_match_field_not_folded`

---

## Claim 30: Struct-returning if-else with parameter-dependent fields emits runtime add (not folded)

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: A struct-returning function whose if-else branch computes fields from a parameter
(`n + 1`, `n * 2`) must emit a runtime conditional branch and `add` instruction, and must NOT
constant-fold the result to a `mov #N` immediate.

```rust
struct Point { x: i32, y: i32 }
fn make(n: i32) -> Point {
    if n > 0 { Point { x: n + 1, y: n * 2 } } else { Point { x: 0, y: 0 } }
}
fn main() -> i32 { make(1).x }
```

Must emit `cbz` or `b.` (runtime if branch), `add` (runtime `n + 1`), and must NOT emit
`mov x0, #2` (the constant-folded value of `make(1).x`).

**Why this claim matters**: Complements Claim 29 for the if-else path. The existing
`runtime_struct_return_if_else_emits_cbz` test only verifies that a branch instruction is
present — it does not assert that field arithmetic executes at runtime. A compiler that folds
`n + 1` to `#2` but still emits the branch for the if-else condition would pass the old test
while violating the runtime-codegen invariant.

**Attack vector**:
1. Constant-folding `make(1)` at the call site emits `mov x0, #2` (the `.x` field) directly.
2. Treating `n` as a const-context value within the if branch folds `n + 1` to `#2`.
3. Both violations produce the correct exit code (2) but bypass runtime computation.

**Violated if**: `compile_to_asm(...)` for the program above returns assembly that:
- lacks a conditional branch in `make` (`cbz` or `b.`), OR
- lacks `add` (`n + 1` was folded), OR
- contains `mov x0, #2` (constant-folded result of `make(1).x`).

**Test**: `cargo test --test e2e -- runtime_struct_return_if_else_not_folded`

---

## Claim 31: dyn Trait method with struct field arithmetic emits runtime add (not folded)

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: When a dyn Trait method accesses struct fields and uses them in arithmetic,
the computation must execute at runtime via vtable dispatch — not be constant-folded from
the call site.

```rust
trait Dist {
    fn manhattan(&self) -> i32;
}
struct Point { x: i32, y: i32 }
impl Dist for Point {
    fn manhattan(&self) -> i32 { self.x + self.y }
}
fn measure(d: &dyn Dist) -> i32 {
    d.manhattan()
}
fn make_and_measure(a: i32, b: i32) -> i32 {
    let p = Point { x: a, y: b };
    measure(&p)
}
fn main() -> i32 { make_and_measure(3, 4) }
```

Must emit `vtable_Dist_Point` (vtable label), `blr` (indirect call), `add` (runtime field sum),
and must NOT emit `mov x0, #7` (the constant-folded result of `make_and_measure(3, 4)`).

**Why this claim matters**: Claims 16–18 verify vtable label presence, both-vtable emission,
and vtable offset correctness. None of them verify that field arithmetic inside the method body
executes at runtime when field values come from function parameters. The
`milestone_147_dyn_trait_two_field_struct` test is compile-and-run only (no assembly inspection).
A constant-folding interpreter that evaluates `a + b` at compile time for literal call sites would
pass all existing dyn Trait tests while violating the runtime-codegen invariant.

**Attack vector**:
1. Constant-folding `measure(3, 4)` at the call site emits `mov x0, #7` directly.
2. Treating `a` and `b` as const-context values within `manhattan` folds `self.x + self.y`
   to `#7` before the vtable dispatch even occurs.
3. Both violations produce exit code 7 and pass compile-and-run tests.

**Violated if**: `compile_to_asm(...)` for the program above returns assembly that:
- lacks `vtable_Dist_Point` (vtable label not emitted), OR
- lacks `blr` (vtable dispatch omitted), OR
- lacks `add` (`self.x + self.y` was folded), OR
- contains `mov x0, #7` (constant-folded result of `measure(3, 4)`).

**Test**: `cargo test --test e2e -- runtime_dyn_trait_field_arithmetic_not_folded`

---

## Claim 32: FnMut closures pass mutable captures by address and write back through the pointer

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: A `FnMut` closure that mutates a captured variable must pass the capture
by address (not by copy) and write the updated value back through the pointer after each
call. Each successive call must observe the state changes made by all prior calls.

```rust
fn main() -> i32 {
    let mut n = 0;
    let mut inc = || { n += 1; n };
    inc();
    inc()
}
```

Must emit `add x_, sp, #N` (address-of the captured slot), an indirect `ldr xN, [xM]`
(LoadPtr through pointer), and an indirect `str xN, [xM]` (StorePtr write-back). Must NOT
emit `mov x0, #2` (the constant-folded result of the second `inc()` call).

**Why this claim matters**: The entire FnMut contract depends on capture-by-address with
write-back. If galvanic snapshots the capture (copies the value instead of its address),
each call sees the initial value — `inc()` always returns 1 regardless of how many times
it's called. This would produce wrong runtime behavior while still passing any test that
only checks the instruction sequence in isolation (e.g., "does `add` appear?"). The only
thing that distinguishes a correct write-back from a snapshot is the presence of the
address-of pattern (`add x, sp, #N`) combined with indirect-through-register load/store
(`ldr xN, [xM]` / `str xN, [xM]`).

**Attack vector**:
1. Snapshot capture: galvanic passes `n` by value. First `inc()` returns 1 (0+1). Second
   `inc()` also returns 1 (still reading original 0). Result: 1 instead of 2. Exit code wrong.
2. Address-of without write-back: galvanic passes `&n` but the closure does not store back.
   Same symptom as (1).
3. Constant-fold: `inc(); inc()` compiled as `mov x0, #1; ret`. Passes compile-and-run for
   the wrong reason. The `!mov x0, #2` assertion catches the constant-fold-of-final-result case.

**Violated if**: `compile_to_asm(...)` for the program above returns assembly that:
- lacks `add x_, sp, #N` (address-of not emitted — capture is by copy), OR
- lacks `ldr xN, [xM]` not through sp (no indirect-through-register load), OR
- lacks `str xN, [xM]` not through sp (no write-back through pointer), OR
- contains `mov x0, #2` (constant-folded result of second `inc()`).

**Test**: `cargo test --test e2e -- runtime_fn_mut_emits_addr_of_and_load_store_ptr`

---

## Claim 33: FnOnce closures capture by value and emit runtime add for closure body

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: A `FnOnce` closure that captures a variable by move must emit a runtime `add`
instruction for the closure body — not fold the result to a constant — and must use `blr`
for the indirect call through the function pointer. This completes the Fn/FnMut/FnOnce
falsification triangle alongside Claims 11 (Fn, non-capturing) and 32 (FnMut, mutable
captures by address).

```rust
fn consume(f: impl FnOnce() -> i32) -> i32 { f() }
fn run(x: i32) -> i32 { consume(move || x + 1) }
fn main() -> i32 { run(41) }
```

Must emit:
- `blr` — indirect call through function pointer (not a direct `bl __closure_*`)
- `add` — `x + 1` in the closure body is a runtime instruction (not folded)
- NOT `mov     x0, #42` — `run(41)` must not be constant-folded through the capture

**Why this claim matters**: An interpreter-style galvanic could fold `run(41)` by evaluating:
`consume(move || 41 + 1)` → `42` and emit `mov x0, #42; ret`. The compile-and-run test
passes (exit code 42 is correct), but galvanic is not a compiler — it never emitted `add`.
The distinction between `impl FnOnce` and `impl Fn`/`impl FnMut` is that FnOnce moves the
capture rather than borrowing or mutably borrowing it. A regression that copies the capture
into a `blr`-called closure but folds the body would be invisible to Claims 11 and 32.

**Attack vectors**:
1. Constant-fold the call site: evaluate `run(41)` → `42` and emit `mov x0, #42`.
   Caught by `!asm.contains("mov     x0, #42")`.
2. Inline the closure body without emitting `add`: emit result constant directly.
   Caught by `asm.contains("add")`.
3. Use a direct call instead of `blr`: emit `bl __closure_run_0` instead of loading the
   function pointer and calling via `blr`.
   Caught by `asm.contains("blr")`.

**Violated if**: `compile_to_asm(...)` for the program above returns assembly that:
- lacks `blr` (indirect call through function pointer not emitted), OR
- lacks `add` (closure body `x + 1` was not emitted as a runtime instruction), OR
- contains `mov     x0, #42` (constant-folded result of `run(41)`).

**Test**: `cargo test --test e2e -- runtime_fn_once_capture_emits_runtime_add`

---

## Claim 34: Associated type method results are not constant-folded

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: A trait method that uses an associated type (`type Area = i32`) must emit
runtime `mul` instructions — not fold the result to a constant — even when the call
arguments happen to be literals. This covers the §10.2 associated type dispatch path,
which is distinct from the generic trait dispatch paths in Claims 9–12.

```rust
trait Shape {
    type Area;
    fn scaled_area(&self, scale: i32) -> i32;
}
struct Square { side: i32 }
impl Shape for Square {
    type Area = i32;
    fn scaled_area(&self, scale: i32) -> i32 { self.side * self.side * scale }
}
fn main() -> i32 {
    let s = Square { side: 3 };
    s.scaled_area(5)
}
```

Must emit:
- `mul` — `self.side * self.side * scale` emits runtime multiply instructions
- NOT `mov     x0, #45` — must not fold `3 * 3 * 5 = 45` to a constant

**Why this claim matters**: Associated types are resolved at monomorphization time, but
the method body executes at runtime. An interpreter-style galvanic could evaluate the
entire method at compile time when the receiver fields and arguments are known literals,
folding `3 * 3 * 5` to `45` and emitting `mov x0, #45; ret`. The compile-and-run test
passes (exit code 45 is correct), but galvanic is not a compiler. The §10.2 associated
type path is separate from the generic dispatch paths (Claims 9–12) — it uses direct
method calls through the concrete type, not vtable or generic-parameter dispatch.

**Attack vectors**:
1. Fold the entire method body at compile time: evaluate `3 * 3 * 5 = 45` when the
   `Square` literal and `scale` literal are both known at the call site.
   Caught by `!asm.contains("mov     x0, #45")`.
2. Emit a single `mul` but fold the second multiplication: `9 * 5 = 45` partially
   constant-folded. Still caught by the negative assertion (`mov x0, #45` absent).

**Violated if**: `compile_to_asm(...)` for the program above returns assembly that:
- lacks `mul` (runtime multiply not emitted), OR
- contains `mov     x0, #45` or `mov x0, #45` (constant-folded result).

**Test**: `cargo test --test e2e -- runtime_assoc_type_method_emits_mul_not_folded`

---

## Claim 35: Generic functions with associated type bounds emit monomorphized calls (not folded)

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: When a generic function is bounded by a trait constraint that includes an
associated type binding (`T: Container<Item = i32>`), galvanic must:
1. Emit a monomorphized function label for each distinct concrete type (`extract__Wrapper`,
   `extract__Doubler`).
2. Emit a `bl` to the monomorphized label — not evaluate the result at compile time.
3. NOT constant-fold the call result even when the struct is initialized with a literal.

This covers the §10.2 + §12.1 associated type bound path (`T: Trait<Assoc = U>`), which is
distinct from:
- Claims 9–12 (plain trait bounds `T: Trait` without associated type binding)
- Claim 34 (direct associated type method dispatch, not via generic parameter)

```rust
trait Container {
    type Item;
    fn get_val(&self) -> i32;
}
struct Wrapper { val: i32 }
impl Container for Wrapper {
    type Item = i32;
    fn get_val(&self) -> i32 { self.val + 1 }
}
fn extract<T: Container<Item = i32>>(c: T) -> i32 { c.get_val() }
fn main() -> i32 {
    let w = Wrapper { val: 9 };
    extract(w)
}
```

Must emit `bl extract__Wrapper` and must NOT emit `mov x0, #10` (constant-folded result).

The two-type variant also tests that `Wrapper__get_val` and `Doubler__get_val` both appear —
two distinct monomorphizations for two distinct concrete types.

**Why this claim matters**: The associated type binding in the bound (`<Item = i32>`) is
parsed and lowered through a separate code path from plain bounds. A regression in the
parser (failing to parse `<Item = i32>`) or in the lowerer (failing to match the bound
during monomorphization) would cause programs using `T: Container<Item = i32>` to fail at
compile time, or to dispatch to the wrong monomorphization. Without this claim, such a
regression is invisible — the compile-and-run tests that exercise this path require QEMU.

**FLS §10.2**: The spec defines associated type bindings in trait bounds but does not specify
how implementations should match them during monomorphization. FLS §10.2: AMBIGUOUS — the
spec does not distinguish between "associated type declared" (impl site) and "associated type
constrained in a bound" (generic parameter). Galvanic resolves this by recording the assoc
type binding in the trait bound and verifying the concrete type's impl satisfies it.

**Attack vectors**:
1. Parser regression: `T: Container<Item = i32>` fails to parse → compile error.
   Caught by the test itself failing with a parse error.
2. Lowerer ignores the `<Item = i32>` binding: it dispatches to any impl of `Container`,
   including impls with a different associated type. Would produce wrong runtime behavior
   for programs that depend on the type constraint.
3. Constant-fold the call site: evaluate `extract(Wrapper { val: 9 })` → `10` and emit
   `mov x0, #10`. Caught by `!asm.contains("mov     x0, #10")`.
4. Two-type regression: only one of the two concrete types is monomorphized. The two-type
   test asserts both `Wrapper__get_val` and `Doubler__get_val` labels exist.

**Violated if**: `compile_to_asm(...)` for the program above returns assembly that:
- lacks `bl      extract__Wrapper` (monomorphized call not emitted), OR
- contains `mov     x0, #10` (constant-folded result of `extract(w)`).

Or for the two-type variant:
- lacks `Wrapper__get_val` or `Doubler__get_val` (one monomorphization missing), OR
- contains `mov     x0, #17` (constant-folded sum `7 + 5*2 = 17`).

**Test**: `cargo test --test e2e -- runtime_assoc_type_bound_emits_monomorphized_bl_not_folded runtime_assoc_type_bound_two_types_both_monomorphized`

---

## Claim 36: `impl FnMut` dispatch through trampoline passes capture by address and does not fold mutation results

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: When a mutable closure is passed as `impl FnMut` to a generic function, galvanic must:
1. Emit a `_trampoline` function for the closure (the mechanism that receives the closure
   state pointer in x27 and dispatches to the actual closure body).
2. Use x27 to pass the closure's mutable capture by address — not by copy (snapshot).
3. Emit `blr` in the consuming function for the indirect call through the function pointer.
4. NOT constant-fold the accumulated mutation result even when the capture is initialized
   from a function parameter.

```rust
fn apply_mut(mut f: impl FnMut() -> i32) -> i32 { f() + f() }
fn run(start: i32) -> i32 {
    let mut n = start;
    apply_mut(|| { n += 1; n })
}
fn main() -> i32 { run(10) }
```

`run(10)` must produce 23 at runtime:
- First `f()`: n = 10+1 = 11, returns 11
- Second `f()`: n = 11+1 = 12, returns 12
- 11 + 12 = 23

Must emit `_trampoline`, `x27` (capture address), `blr`, and `add` — and must NOT emit:
- `mov x0, #23` (constant-folded correct result from run(10))
- `mov x0, #3` (snapshot-from-zero: two calls each start from 0, return 1 and 2, sum = 3)

**Why this claim matters**: This covers a distinct codegen path from Claims 32 and 33:
- Claim 32: Direct FnMut closure mutation in the same scope (no generic dispatch, no trampoline).
  `let mut inc = || { n += 1; n }; inc(); inc()` — capture address passed at each call site.
- Claim 33: FnOnce move-capture via `impl FnOnce` (single call, no write-back needed).
- Claim 36: FnMut via `impl FnMut` bound — requires a trampoline function that receives the
  capture-state pointer in x27 and enables write-back across multiple calls through a generic
  function that doesn't know the concrete closure type.

A regression in the trampoline generation (dropping `_trampoline`, forgetting x27, or
snapshot-copying the capture) produces wrong runtime behavior: `run(10)` returns 3 (from
snapshot starting at 0) or 2 (snapshot starting at start but resetting). Neither 3 nor 23
appears in the source — neither is a literal. But a folding interpreter would compute 23 and
emit `mov x0, #23`. All three failure modes produce exit codes that differ from 23, and are
caught by CI compile-and-run tests — but only on CI with QEMU. Assembly inspection catches
them locally and without cross tools.

**Attack vectors**:
1. Trampoline dropped: galvanic forgets to emit `_trampoline` for the `impl FnMut` path.
   The `apply_mut` call then has no function pointer to dereference. Compile error or
   wrong code. Caught by `asm.contains("_trampoline")`.
2. Snapshot-by-value (copy n, not &n): x27 holds the value of n (e.g., 10) instead of its
   address. The closure body mutates a local register, never writes back to the caller's n.
   First call returns 11 (10+1), second call also sees start=10, returns 11. Sum = 22.
   Or from initial 0: both calls return 1, sum = 2. Exit code wrong, caught by CI.
   Caught locally by absence of `x27` as address register.
3. Constant-fold `run(10)` → evaluate 11+12=23 at compile time → `mov x0, #23`.
   Caught by `!asm.contains("mov     x0, #23")`.
4. Snapshot-from-zero fold: fold two calls each returning 1 and 2 → `mov x0, #3`.
   Caught by `!asm.contains("mov     x0, #3")`.

**Violated if**: `compile_to_asm(...)` for the program above returns assembly that:
- lacks `_trampoline` (trampoline function not emitted — impl FnMut path broken), OR
- lacks `x27` (capture-state pointer convention dropped — write-back impossible), OR
- lacks `blr` (indirect call through function pointer absent), OR
- lacks `add` (mutation n += 1 was not emitted as a runtime instruction), OR
- contains `mov     x0, #23` (correct result constant-folded), OR
- contains `mov     x0, #3` (snapshot-from-zero result constant-folded).

**Test**: `cargo test --test e2e -- runtime_fn_mut_as_impl_fn_mut_emits_trampoline runtime_fn_mut_as_impl_fn_mut_mutation_not_folded`

---

## Claim 37: &dyn Trait let binding emits fat pointer load (not constant-folded)

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: When `let x: &dyn Trait = &val;` is compiled, galvanic materializes a
fat pointer (data_ptr, vtable_ptr) in two stack slots and loads them at runtime.
The vtable dispatch must use `blr` (indirect call), and the result must not be
constant-folded. This is distinct from the inline-borrow path (`f(&val)` at a
call site) — the fat pointer is stored in a local variable and loaded on demand.

**Why this matters**: The let binding path introduces a new mechanism: storing a fat
pointer in local slots and reloading both slots when passing to a `&dyn Trait`
parameter. A regression that skips the slot store/load (e.g., inlining the address
as a constant) would produce wrong behavior when the concrete value is in a different
stack frame or when multiple fat-pointer locals coexist. The distinction between
"inline borrow" and "stored fat pointer" is FLS-relevant: both paths must produce
correct vtable dispatch via `blr`, not constant folding.

**Violated if**: `compile_to_asm` for a program using `let s: &dyn Shape = &c; print_area(s)`:
- lacks `blr` (vtable dispatch not emitted), OR
- contains `mov     x0, #25` (Circle{r:5}.area()=25 constant-folded), OR
- lacks `vtable_Shape_Circle` (vtable label not emitted for the concrete type).

**Test**: `cargo test --test e2e -- runtime_dyn_trait_let_binding_not_folded runtime_dyn_trait_let_binding_emits_load_from_slot`

---

## Claim 38: Unsafe block bodies emit runtime instructions (not constant-folded)

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: An `unsafe { ... }` block is NOT a const context (FLS §6.4.4, FLS §6.1.2
Constraint 1). Code inside an unsafe block in a regular function body must emit runtime
instructions — not be evaluated at compile time. In particular, `unsafe { n * 3 }` where
`n` is a function parameter must emit a `mul` instruction at runtime and must NOT fold
the result to a constant `mov`.

```rust
fn triple(n: i32) -> i32 {
    unsafe { n * 3 }
}
fn main() -> i32 { triple(4) }
```

Must emit:
- `mul` — `n * 3` executes at runtime inside the unsafe block
- NOT `mov     x0, #12` — must not fold `triple(4)` to the literal result

**Why this claim matters**: Claims 1–37 cover the core runtime-codegen invariant for many
different code paths. The unsafe block path (FLS §6.4.4) is a distinct code path in the
lowerer — it is not covered by any existing falsification claim. An unsafe block is
syntactically a block expression with an `unsafe` keyword; a regression that introduces
const-evaluation for unsafe blocks (treating `unsafe` as implying "evaluated early") would
be invisible to all existing claims. The assembly inspection test
`runtime_unsafe_block_emits_runtime_instructions_not_folded` covers this, but without a
falsification claim it runs only in `cargo test --test e2e`, not as an adversarial gate.

**Attack vectors**:
1. Constant-fold the unsafe block body: evaluate `n * 3` at compile time when `n` is
   known at the call site (`triple(4)` → `12`). Emit `mov x0, #12; ret`.
   Caught by `!asm.contains("mov     x0, #12")`.
2. Elide the unsafe block entirely: treat `unsafe { expr }` as just `expr` and then
   fold that. Same symptom, same catch.
3. Drop the `mul` instruction and emit the result as a shift or add chain that constant-
   evaluates to the same value. Caught by `asm.contains("mul")`.

**FLS §6.4.4 note**: The spec defines unsafe blocks as block expressions that enter an
unsafe context for the purpose of unsafety checks (raw pointers, unsafe fns) — NOT for
the purpose of const evaluation. FLS §6.4.4 does not say unsafe blocks are const contexts.
FLS §6.1.2 Constraint 1 applies: only explicit const contexts trigger compile-time eval.

**Violated if**: `compile_to_asm(...)` for the program above returns assembly that:
- lacks `mul` (runtime multiply not emitted — unsafe block body was constant-folded or elided), OR
- contains `mov     x0, #12` or `mov x0, #12` (constant-folded result of `triple(4)`).

**Test**: `cargo test --test e2e -- runtime_unsafe_block_emits_runtime_instructions_not_folded`

---

## Claim 39: `const fn` called from non-const context emits runtime `bl` (not folded)

**Stakeholder**: William (researcher), Compiler Researchers, FLS / Ferrocene Ecosystem

**Promise**: A `const fn` is only eligible for compile-time evaluation when called from a
const context (FLS §9:41–43, Constraint 2 in `refs/fls-constraints.md`). When called from
a regular function body (non-const context), it must emit a real runtime `bl` instruction —
not be constant-folded. `fn main() -> i32 { add(20, 22) }` is not a const context even
though `add` is declared `const fn`.

```rust
const fn add(a: i32, b: i32) -> i32 { a + b }
fn main() -> i32 { add(20, 22) }
```

Must emit:
- `bl add` — runtime call to the function (not folded)
- NOT `mov x0, #42` — must not fold `add(20, 22)` to the literal result

**Why this claim matters**: `milestone_123_const_fn_runtime_call` verifies exit code 42,
which is correct whether galvanic emits a runtime call or constant-folds the result. Only
the assembly inspection test distinguishes them. A regression that treated `const fn` as
"always const-evaluated regardless of call context" would pass every compile-and-run test
but violate FLS §9:41–43. This claim ensures the context-sensitivity of `const fn` dispatch
is an adversarially-guarded invariant.

**Attack vectors**:
1. Evaluate `add(20, 22)` at compile time because the arguments are known constants, emit
   `mov x0, #42; ret`. Exit code 42 is correct; the test would still pass. Only the assembly
   inspection gate (no `#42`) catches this.
2. Partially fold: evaluate the `const fn` body inline without emitting `bl add` at all.
   Caught by `asm.contains("bl add")`.
3. Conflate `const fn` with `const`: treat `const fn add(...)` as if every call site is a
   const context. Same symptom as (1). Caught by both positive and negative assertions.

**FLS §9:41–43 note**: The spec states that `const fn` bodies may be evaluated at compile time
only when called from a const context. The spec does not prescribe how an implementation
distinguishes const from non-const call contexts, but the rule is clear: without an explicit
const context at the call site (const item, const block, array length, etc.), the call must
execute at runtime.

**Violated if**: `compile_to_asm(...)` for the program above returns assembly that:
- lacks `bl add` (runtime call not emitted — `const fn` was constant-folded at call site), OR
- contains `#42` (constant-folded result of `add(20, 22)` emitted as an immediate).

**Test**: `cargo test --test e2e -- runtime_const_fn_runtime_call_emits_bl_not_folded`

---

## Claim 41: &dyn Trait fat pointer re-bind copies fat pointer and dispatches via blr (not folded)

**Stakeholder**: William (researcher), Compiler Researchers, FLS / Ferrocene Ecosystem

**Promise**: When a `&dyn Trait` fat pointer local is re-bound (`let y = x`), galvanic
copies both the data pointer and the vtable pointer to new consecutive stack slots and
registers `y` in `local_dyn_types`. Subsequent method calls on `y` and calls passing `y`
to `fn f(&dyn Trait)` must emit vtable dispatch (`blr`) at runtime — not constant-fold
the result (FLS §4.13, §6.1.2 Constraint 1).

```rust
trait Shape { fn area(&self) -> i32; }
struct Rect { w: i32, h: i32 }
impl Shape for Rect { fn area(&self) -> i32 { self.w * self.h } }
fn use_shape(s: &dyn Shape) -> i32 { s.area() }
fn main() -> i32 {
    let r = Rect { w: 3, h: 4 };
    let x: &dyn Shape = &r;
    let y = x;        // fat pointer re-bind
    use_shape(y)      // must dispatch via blr, result = 12
}
```

Must emit:
- `ldr` — loading the fat pointer slots (data_ptr, vtable_ptr) from stack
- `blr` — vtable dispatch (indirect call through the vtable pointer)
- `vtable_Shape_Rect` label — the vtable for the concrete type
- NOT `mov     x0, #12` — must not fold 3*4=12 to a compile-time constant

**Why this claim matters**: The re-bind case copies an existing fat pointer without
creating a new vtable entry. An implementation that treated `let y = x` as a thin
copy (missing the vtable slot copy) would produce wrong results when `y` is used for
dispatch. An implementation that constant-folded `3*4` because the Rect dimensions are
statically known would pass every compile-and-run test but violate FLS §6.1.2.

**Attack vectors**:
1. Copy only the data_slot during re-bind, leaving the vtable slot uninitialised. Dispatch
   via `y` would read garbage or crash. Caught by `blr` presence.
2. Fold `Rect{w:3, h:4}.area()` to `mov x0, #12` because both fields are literals. Caught
   by absence of `mov     x0, #12`.
3. Skip the re-bind entirely and treat `y` as an alias for `x`. Would work for single-use
   but is not a correct copy — caught indirectly by the `ldr` assertion.

**FLS §4.13 note**: AMBIGUOUS — The spec does not define how `&dyn Trait` type information
propagates through unannotated let bindings. Galvanic's choice: propagate `local_dyn_types`
registration to `y` using the same two-slot layout as the source binding.

**Violated if**: `compile_to_asm(DYN_TRAIT_REBIND_BASIC)` returns assembly that:
- lacks `ldr` (fat pointer slots not copied from stack), OR
- lacks `blr` (vtable dispatch not emitted — folded or direct call), OR
- contains `mov     x0, #12` (result constant-folded)

**Tests**: `cargo test --test e2e -- runtime_dyn_trait_rebind_emits_load_for_fat_pointer runtime_dyn_trait_rebind_not_folded`

---

## Claim 42: while loop emits runtime control flow (not constant-folded)

**Stakeholder**: William (researcher), Compiler Researchers, FLS / Ferrocene Ecosystem

**Promise**: A `while` loop whose body modifies the loop variable must emit runtime
control flow — `cmp`, `cset`, `cbz`, and a back-edge `b` — at every iteration. The
loop result must NOT be constant-folded to an immediate. A while loop is not a const
context (FLS §6.15.3, §6.1.2 Constraint 1).

```rust
fn main() -> i32 { let mut x = 0; while x < 5 { x = x + 1; } x }
```

Must emit:
- `cmp` — runtime comparison for the loop condition `x < 5`
- `cset` — materialise comparison result into a register
- `cbz` — conditional exit branch when condition is false
- `b .L{n}` — back-edge returning to the top of the loop
- NOT `mov     x0, #5` — must not fold the 5-iteration result to a compile-time constant

**Why this claim matters**: The loop runs x from 0 to 5 — a statically-determined result.
An interpreter could evaluate the loop at compile time and emit `mov x0, #5`. The positive
assertions (cmp/cset/cbz/b) verify loop structure is present, but they would pass even if
dead loop code co-existed with a constant-folded result. The negative assertion closes this
gap: the result value 5 must not appear as a literal move instruction.

**Attack vectors**:
1. Fold `while x < 5 { x += 1; }` starting from x=0 to `mov x0, #5`. Positive assertions
   might still pass if dead loop instructions are emitted. Caught by absence of `mov x0, #5`.
2. Omit the back-edge `b` instruction, converting the loop to a single conditional branch
   (dead loop). Caught by back-edge `b` assertion.
3. Replace the runtime comparison with a compile-time constant condition. Caught by `cmp`
   and `cset` presence assertions.

**FLS §6.15.3**: While loop expressions. The condition is checked at runtime each iteration.
**FLS §6.1.2 Constraint 1**: `fn main()` is not a const context — loop must execute at runtime.

**Violated if**: `compile_to_asm(WHILE_SOURCE)` returns assembly that:
- lacks `cmp` (condition not checked at runtime), OR
- lacks `cbz` (no conditional exit branch), OR
- lacks `b` back-edge (no loop at all), OR
- contains `mov     x0, #5` (loop result was constant-folded)

**Test**: `cargo test --test e2e -- runtime_while_emits_cmp_cset_cbz_and_b`

---

## Claim 43: if expression emits runtime conditional branch (not constant-folded)

**Stakeholder**: William (researcher), Compiler Researchers, FLS / Ferrocene Ecosystem

**Promise**: An `if` expression with a statically-known boolean condition must emit
a runtime conditional branch (`cbz`) and store the result through the phi slot
(`str`/`ldr`). The result must NOT be constant-folded to an immediate. An if
expression is not a const context (FLS §6.17, §6.1.2 Constraint 1).

```rust
fn main() -> i32 { if true { 7 } else { 0 } }
```

Must emit:
- `cbz` — runtime conditional branch on the boolean condition
- `str` and `ldr` — phi slot stores/loads to merge the two branches
- NOT `mov     x0, #7` — must not fold the then-branch result to a compile-time constant

**Why this claim matters**: The condition `true` is statically known — an interpreter
could skip branching entirely and emit `mov x0, #7`. The positive assertions (cbz, str/ldr)
verify that branching infrastructure is present, but they would pass even if dead branch
code co-existed with a constant-folded result. The negative assertion closes this gap:
the result value 7 must not appear as a direct literal move into the return register.

**Attack vectors**:
1. Fold `if true { 7 } else { 0 }` to `mov x0, #7`. The condition is `true` (always true),
   so always returns 7. Positive assertions (cbz/str/ldr) might still pass with dead code.
   Caught by absence of `mov x0, #7`.
2. Eliminate the else branch entirely (since condition is statically true), producing a
   codegen path with no conditional branch. Caught by `cbz` presence assertion.
3. Replace runtime phi slot merge with direct register assignment. Caught by `str`/`ldr`
   presence assertions.

**FLS §6.17**: If expressions evaluate their condition at runtime and branch accordingly.
**FLS §6.1.2 Constraint 1**: `fn main()` is not a const context — even statically-known
conditions must be evaluated at runtime; the if expression result must not be folded.

**Violated if**: `compile_to_asm(IF_SOURCE)` returns assembly that:
- lacks `cbz` (condition not checked at runtime), OR
- lacks `str`/`ldr` (phi slot not used for branch merge), OR
- contains `mov     x0, #7` (result was constant-folded, bypassing the runtime branch)

**Test**: `cargo test --test e2e -- runtime_if_emits_cbz`

---

## Claim 44: &dyn Trait-returning function emits runtime bl + vtable blr (not constant-folded)

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: A function with return type `&dyn Trait` must:
1. Emit `bl forward` at the call site — a real runtime function call, not inlined or folded.
2. Emit `blr` for the subsequent vtable dispatch on the returned fat pointer — not devirtualized or constant-folded.
3. Emit `ldr x0` / `ldr x1` in the callee (RetFields for fat pointer).
4. Emit `str x0` / `str x1` at the call site (CallRetFatPtr, storing the returned halves).
5. NOT emit a constant-folded result (e.g., `mov x0, #7`).

```rust
trait Animal { fn sound(&self) -> i32; }
struct Dog { x: i32 }
impl Animal for Dog { fn sound(&self) -> i32 { self.x } }
fn forward(a: &dyn Animal) -> &dyn Animal { a }
fn main() -> i32 {
    let d = Dog { x: 7 };
    let a: &dyn Animal = &d;
    let b = forward(a);
    b.sound()
}
```

Must emit:
- `bl      forward` — runtime call to the dyn-returning function
- `blr` — vtable indirect dispatch on the returned fat pointer
- `ldr     x0,` and `ldr     x1,` — fat pointer halves loaded at callee return
- `str     x0,` and `str     x1,` — fat pointer halves stored at call site
- NOT `mov     x0, #7` — result must not be constant-folded

**Why this claim matters**: Milestone 162 added `&dyn Trait` as a function return type.
Without this claim, a regression that:
- devirtualizes the returned fat pointer (replacing `blr` with a direct `bl Dog__sound`), OR
- constant-folds the whole pipeline (`mov x0, #7`), OR
- drops the fat pointer ABI conventions (treating return as scalar)
would be invisible to all compile-and-run tests (exit code is still 7).

**Attack vectors**:
1. Constant-fold `forward(a).sound()` to `mov x0, #7` — passes exit-code check, misses ABI.
2. Devirtualize the returned trait object (call `Dog__sound` directly via `bl`, not `blr`).
3. Return only one slot (data ptr) from `forward`, treating `&dyn Trait` as a scalar pointer.
   The other tests' `str x1` assertion catches this.

**FLS §4.13**: Trait objects are fat pointers (data_ptr, vtable_ptr). A function returning
`&dyn Trait` must propagate both halves.
**FLS §4.13 AMBIGUOUS**: The ABI for fat pointer returns is not defined by the spec.
Galvanic uses (x0=data_ptr, x1=vtable_ptr), matching the parameter convention.

**Violated if**: `compile_to_asm(DYN_TRAIT_RETURN_BASIC)` returns assembly that:
- lacks `bl      forward` (call to dyn-returning fn was inlined/folded), OR
- lacks `blr` (returned fat pointer not dispatched via vtable), OR
- lacks `ldr     x0,` or `ldr     x1,` (callee not loading fat pointer halves), OR
- lacks `str     x0,` or `str     x1,` (call site not storing returned fat pointer), OR
- contains `mov     x0, #7` (result constant-folded)

**Tests**: `cargo test --test e2e -- runtime_dyn_return_emits_fat_ptr_loads_and_stores runtime_dyn_return_not_folded`

---

## Claim 45: impl Trait return uses static bl dispatch (not vtable blr) and result is not constant-folded

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: A function with return type `impl Trait` must use static (monomorphized) dispatch —
not vtable dispatch. The call to the `impl Trait`-returning function must emit `bl <fn_name>`,
the subsequent method call must emit `bl <concrete_method>` (not `blr`), and the method result
must NOT be constant-folded to an immediate.

```rust
trait Score { fn score(&self) -> i32; }
struct Points { n: i32 }
impl Score for Points { fn score(&self) -> i32 { self.n + 1 } }
fn make_points(n: i32) -> impl Score { Points { n } }
fn main() -> i32 { let p = make_points(6); p.score() }
```

Must emit:
- `bl      make_points` — runtime call to the impl-Trait-returning function (not inlined/folded)
- `bl` to the concrete method (static dispatch — monomorphized at compile time)
- `add` — method body emits runtime arithmetic (not constant-folded)
- NOT `blr` — vtable dispatch is wrong for `impl Trait` (only correct for `&dyn Trait`)

**Why this claim matters**: Milestone 163 added `impl Trait` in return position. The key
distinction from `&dyn Trait` (Claim 44) is the dispatch mechanism:
- `&dyn Trait` → `blr` (vtable indirect dispatch)
- `impl Trait` → `bl` (static monomorphized dispatch)

Without this claim, a regression that accidentally emits vtable dispatch for `impl Trait`
return (treating it like `&dyn Trait`) would pass all compile-and-run tests while
violating the static dispatch guarantee. Similarly, constant-folding `make_points(6).score()`
to `mov x0, #7` passes exit-code checks but breaks the "compiler not interpreter" premise.

**Attack vectors**:
1. Constant-fold `make_points(6).score()` to `mov x0, #7` — passes exit-code check.
   Caught by `add` presence assertion.
2. Emit `blr` for the method call (treating `impl Trait` return like `&dyn Trait`).
   Caught by `!asm.contains("blr")` assertion in `runtime_impl_trait_return_emits_bl_not_blr`.
3. Inline `make_points` and eliminate the `bl make_points` call entirely.
   Caught by `bl make_points` presence assertion.

**FLS §9**: Functions are called via `bl` at runtime. Static dispatch preserves this.
**FLS §11**: `impl Trait` uses static dispatch — the concrete type is resolved at compile
time, not through a vtable.
**FLS §11 AMBIGUOUS**: The spec does not define the mechanism by which the concrete return
type for `impl Trait` is determined at call sites. Galvanic infers from the body tail
expression (struct literal).
**FLS §6.1.2 Constraint 1**: `fn main()` is not a const context — even with literal
arguments, method bodies must execute at runtime.

**Violated if**: `compile_to_asm(IMPL_TRAIT_RETURN_BASIC)` returns assembly that:
- lacks `bl      make_points` (call to impl-Trait-returning fn was inlined/folded), OR
- contains `blr` (vtable dispatch used instead of static dispatch), OR
- lacks `add` (method body constant-folded instead of emitting runtime arithmetic)

**Tests**: `cargo test --test e2e -- runtime_impl_trait_return_emits_bl_not_blr runtime_impl_trait_return_not_folded`

---

## Claim 46: Supertrait method dispatch uses static bl (not constant-folded) and both methods emit runtime code

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: A generic function with a supertrait bound (`T: Derived` where `Derived: Base`)
must dispatch both supertrait and subtrait method calls via `bl` to monomorphized labels.
Neither the supertrait call (`t.base_val()`) nor the combined result of calling both methods
must be constant-folded to an immediate.

```rust
trait Base { fn base_val(&self) -> i32; }
trait Derived: Base { fn derived_val(&self) -> i32; }
struct Foo { x: i32 }
impl Base for Foo { fn base_val(&self) -> i32 { self.x + 3 } }
impl Derived for Foo { fn derived_val(&self) -> i32 { self.x + 1 } }
fn get_base<T: Derived>(t: T) -> i32 { t.base_val() }
fn sum_both<T: Derived>(t: T) -> i32 { t.base_val() + t.derived_val() }
```

Must emit:
- `add` — supertrait method bodies emit runtime arithmetic
- NOT `mov x0, #11` — `base_val` result for `x=8` must not be folded
- NOT `mov x0, #9` — `base_val + derived_val` for `x=4` must not be folded

**Why this claim matters**: Milestone 164 added supertrait bounds (`trait D: B { ... }`).
The monomorphization path for supertrait methods (`t.base_val()` when `T: Derived`) goes
through the same generic dispatch as regular trait methods, but traverses the supertrait
relationship. Without this claim, a regression that either folds supertrait results or
drops the supertrait method call entirely would pass all exit-code tests.

**Attack vectors**:
1. Constant-fold `get_base(Foo { x: 8 })` to `mov x0, #11` — exit code correct, folded.
   Caught by `!asm.contains("mov     x0, #11")`.
2. Fold `sum_both(Foo { x: 4 })` to `mov x0, #9` — exit code correct, folded.
   Caught by `!asm.contains("mov     x0, #9")`.
3. Fail to emit `add` for supertrait method body (`self.x + 3` or `self.x + something`).
   Caught by `asm.contains("add")`.

**FLS §4.14**: Supertrait bounds require that any type satisfying `T: Derived` also satisfies
`T: Base`. The compiler must resolve supertrait method calls through the same monomorphization
path as regular trait calls.
**FLS §4.14 AMBIGUOUS**: The spec does not specify how supertrait method availability
propagates to generic call sites. Galvanic resolves via monomorphization: `T__base_val`
exists because the concrete type implements `Base`.
**FLS §6.1.2 Constraint 1**: `fn main()` is not a const context — supertrait method
bodies must execute at runtime.

**Violated if**: `compile_to_asm(SUPERTRAIT_BASIC)` or `compile_to_asm(SUPERTRAIT_BOTH)` returns assembly that:
- lacks `add` (supertrait method body constant-folded), OR
- contains `mov     x0, #11` (base_val result folded for x=8), OR
- contains `mov     x0, #9` (sum of both method results folded for x=4)

**Tests**: `cargo test --test e2e -- runtime_supertrait_call_emits_bl_not_folded runtime_supertrait_both_methods_not_folded`

---

## Claim 47: Default methods calling supertrait methods emit runtime bl (not constant-folded) and chained defaults emit separate runtime bl dispatches

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: A default method that calls a supertrait abstract method must dispatch via
`bl` to the concrete monomorphized label at runtime. Chained default methods (default
calling default) must each emit a separate `bl` dispatch — the chain must not be
constant-folded to a single immediate.

```rust
// Pattern 1: default method calls supertrait abstract method
trait Base { fn base_val(&self) -> i32; }
trait Derived: Base {
    fn combined(&self) -> i32 { self.base_val() + 1 }
}
struct Foo { x: i32 }
impl Base for Foo { fn base_val(&self) -> i32 { self.x } }
impl Derived for Foo {}
fn make_foo(n: i32) -> Foo { Foo { x: n } }
// main: make_foo(9).combined() → 10

// Pattern 2: chained default methods
trait Scalable {
    fn value(&self) -> i32;
    fn doubled(&self) -> i32 { self.value() * 2 }
    fn quadrupled(&self) -> i32 { self.doubled() * 2 }
}
struct Foo { x: i32 }
impl Scalable for Foo { fn value(&self) -> i32 { self.x } }
fn make_foo(n: i32) -> Foo { Foo { x: n } }
// main: make_foo(3).quadrupled() → 12
```

Must emit (Pattern 1):
- `bl      Foo__base_val` — default method calls supertrait abstract via bl
- `add` — the `+1` in the default body must be a runtime add
- NOT `mov x0, #10` — result for `n=9` must not be folded

Must emit (Pattern 2):
- `bl      Foo__doubled` — quadrupled calls doubled at runtime
- `bl      Foo__value` — doubled calls value at runtime
- NOT `mov x0, #12` — result for `n=3` must not be folded

**Why this claim matters**: Milestone 165 added default methods that call supertrait
abstract methods. Without this claim, a regression that constant-folds the call chain
(e.g., inlines `value` into `doubled` into `quadrupled` and emits `mov x0, #12`) would
pass all exit-code tests invisibly. The chain must execute at runtime.

**Attack vectors**:
1. Constant-fold `make_foo(9).combined()` to `mov x0, #10` — exit code correct, folded.
   Caught by `!asm.contains("mov     x0, #10")`.
2. Fail to emit `bl Foo__base_val` in the default method body (inlined instead).
   Caught by `asm.contains("bl      Foo__base_val")`.
3. Constant-fold `make_foo(3).quadrupled()` to `mov x0, #12` — exit code correct, folded.
   Caught by `!asm.contains("mov     x0, #12")`.
4. Inline `doubled` into `quadrupled`, dropping `bl Foo__doubled`.
   Caught by `asm.contains("bl      Foo__doubled")`.
5. Inline `value` into `doubled`, dropping `bl Foo__value`.
   Caught by `asm.contains("bl      Foo__value")`.

**FLS §4.14**: Supertrait bounds — calling a supertrait method from a default body requires
traversing the supertrait relationship at monomorphization time.
**FLS §10.1.1**: Default method bodies are emitted per concrete type. The body must execute
at runtime, not at compile time.
**FLS §10.1.1 AMBIGUOUS**: The spec does not specify whether a default method body that
calls supertrait methods should inline those calls or dispatch via `bl`. Galvanic uses `bl`
(no inlining), consistent with the general no-constant-folding constraint.
**FLS §6.1.2 Constraint 1**: `fn main()` is not a const context — default method chains
must execute at runtime.

**Violated if**: `compile_to_asm(DEFAULT_SUPERTRAIT_CALL)` returns assembly that:
- lacks `bl      Foo__base_val` (supertrait call was inlined/folded), OR
- lacks `add` (default body arithmetic was constant-folded), OR
- contains `mov     x0, #10` (result for n=9 was folded)

**Violated if**: `compile_to_asm(DEFAULT_CHAIN)` returns assembly that:
- lacks `bl      Foo__doubled` (quadrupled inlined doubled), OR
- lacks `bl      Foo__value` (doubled inlined value), OR
- contains `mov     x0, #12` (chain result for n=3 was folded)

**Tests**: `cargo test --test e2e -- runtime_supertrait_default_call_emits_bl_not_folded runtime_supertrait_default_chain_not_folded`

---

## Claim 48: Self::AssocType in method signatures emits runtime dispatch (not constant-folded)

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: When `Self::AssocType` is used in a method signature (return type or
parameter type), the emitted assembly must dispatch via `bl` to the concrete
monomorphized label at runtime. The associated type projection is resolved per-impl
at lowering time — not constant-folded at compile time.

```rust
// Pattern 1: Self::AssocType in return position
trait Wrapper {
    type Output;
    fn value(&self) -> Self::Output;
}
struct IntWrap { x: i32 }
impl Wrapper for IntWrap {
    type Output = i32;
    fn value(&self) -> Self::Output { self.x }
}
fn make_wrap(n: i32) -> IntWrap { IntWrap { x: n } }
// main: make_wrap(7).value() → 7

// Pattern 2: Self::AssocType in parameter position
trait Scalable {
    type Factor;
    fn scale(&self, f: Self::Factor) -> i32;
}
struct Val { n: i32 }
impl Scalable for Val {
    type Factor = i32;
    fn scale(&self, f: Self::Factor) -> i32 { self.n * f }
}
fn make_val(n: i32) -> Val { Val { n } }
// main: make_val(3).scale(4) → 12
```

Must emit (Pattern 1):
- `bl      IntWrap__value` — method with Self::Output return must dispatch at runtime
- NOT `mov x0, #7` — result for n=7 must not be folded

Must emit (Pattern 2):
- `mul` — Self::Factor parameter method must emit runtime multiply
- NOT `mov x0, #12` — result for n=3, f=4 must not be constant-folded

**Why this claim matters**: Milestone 166 added `Self::AssocType` in method signatures.
Without this claim, a regression that constant-folds the method call (e.g., evaluates
`make_wrap(7).value()` at compile time and emits `mov x0, #7`) would pass all exit-code
tests invisibly. The method body must execute at runtime.

**Attack vectors**:
1. Constant-fold `make_wrap(7).value()` to `mov x0, #7` — exit code correct, folded.
   Caught by `!asm.contains("mov     x0, #7")`.
2. Fail to emit `bl IntWrap__value` (inlined or skipped).
   Caught by `asm.contains("bl      IntWrap__value")`.
3. Constant-fold `make_val(3).scale(4)` to `mov x0, #12` — exit code correct, folded.
   Caught by `!asm.contains("mov     x0, #12")`.
4. Skip the `mul` instruction in `Val__scale` (constant multiply instead).
   Caught by `asm.contains("mul")`.

**FLS §10.2**: Associated types in trait definitions — `Self::X` refers to the concrete
type bound by the implementing type's associated type declaration.
**FLS §10.2 AMBIGUOUS**: The spec does not specify how `Self::X` projections are resolved
when `Self` appears in a trait method signature vs. impl method signature. Galvanic
resolves via per-impl type alias registry (impl override takes precedence over default).
**FLS §6.1.2 Constraint 1**: `fn main()` is not a const context — method calls with
Self::AssocType signatures must execute at runtime.

**Violated if**: `compile_to_asm(SELF_ASSOC_TYPE_RETURN)` returns assembly that:
- lacks `bl      IntWrap__value` (dispatch was inlined/folded), OR
- contains `mov     x0, #7` (result for n=7 was constant-folded)

**Violated if**: `compile_to_asm(SELF_ASSOC_TYPE_PARAM)` returns assembly that:
- lacks `mul` (Self::Factor multiply was constant-folded), OR
- contains `mov     x0, #12` (result for n=3,f=4 was constant-folded)

**Tests**: `cargo test --test e2e -- runtime_self_assoc_type_return_emits_bl_not_folded runtime_self_assoc_type_param_emits_mul_not_folded`

---

## Claim 49: T::AssocType in generic function return position emits monomorphized dispatch (not constant-folded)

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: When a generic free function uses `T::AssocType` as its return type
(e.g., `fn use_it<C: Container>(c: C) -> C::Item`), the emitted assembly must:
1. Emit a monomorphized label per concrete type (e.g., `use_it__Counter`).
2. Dispatch to the concrete method via `bl` at runtime.
3. Not constant-fold the result.

Both the single-type and two-type (both concrete types) cases are guarded.

```rust
// Pattern 1: single generic fn with T::Item return
trait Container {
    type Item;
    fn get(&self) -> Self::Item;
}
struct Counter { val: i32 }
impl Container for Counter {
    type Item = i32;
    fn get(&self) -> Self::Item { self.val }
}
fn use_it<C: Container>(c: C) -> C::Item { c.get() }
fn make_and_call(n: i32) -> i32 { let c = Counter { val: n }; use_it(c) }
// main: make_and_call(5) → 5

// Pattern 2: two concrete types, both monomorphized
trait Measure {
    type Unit;
    fn measure(&self) -> Self::Unit;
}
struct Meters { val: i32 }
struct Feet { val: i32 }
impl Measure for Meters { type Unit = i32; fn measure(&self) -> Self::Unit { self.val } }
impl Measure for Feet   { type Unit = i32; fn measure(&self) -> Self::Unit { self.val * 3 } }
fn get_measure<M: Measure>(m: M) -> M::Unit { m.measure() }
// main: get_measure(Meters{val:2}) + get_measure(Feet{val:1}) → 5
```

Must emit (Pattern 1):
- `use_it__Counter` — monomorphized label for the concrete instantiation
- `bl      Counter__get` — runtime dispatch to the trait method
- NOT `mov x0, #0` — result must not be folded to a constant

Must emit (Pattern 2):
- Both `Meters` and `Feet` labels in the assembly
- NOT `mov     x0, #5` — result must not be constant-folded to 5

**Why this claim matters**: Milestone 167 added `T::AssocType` in generic free
function return position. Without this claim, a regression that folds
`use_it__Counter` to a constant return (e.g., evaluating `c.get()` at compile
time) would pass all exit-code tests. The generic function must remain a real
runtime dispatch site, not an inlined constant.

**Attack vectors**:
1. Constant-fold `use_it__Counter` to `mov x0, #5; ret` — exit code correct, folded.
   Caught by `!asm.contains("mov     x0, #0")` and absence of `bl Counter__get`.
2. Fail to emit `use_it__Counter` monomorphized label (generic fn not instantiated).
   Caught by `asm.contains("use_it__Counter")`.
3. Fold two-type case to `mov x0, #5` — both concrete results summed at compile time.
   Caught by `!asm.contains("mov     x0, #5")`.
4. Emit only one concrete type's label (second instantiation missing).
   Caught by checking both `Meters` and `Feet` labels present.

**FLS §10.2**: Associated type projections `T::X` resolve to the concrete type
bound by `T`'s impl of the trait that declares `type X`.
**FLS §12.1 / §10.2 AMBIGUOUS**: The FLS does not specify how `T::X` is resolved
during generic instantiation (monomorphization). Galvanic extends the per-monomorphization
alias map: when `C` → `Counter`, add `C::Item` → `Counter::Item`'s IrTy. The spec
is silent on the mechanism.
**FLS §6.1.2 Constraint 1**: `fn main()` is not a const context — generic calls
with `T::AssocType` return types must execute at runtime.

**Violated if**: `compile_to_asm(PROJ_RETURN)` returns assembly that:
- lacks `use_it__Counter` (monomorphization missing), OR
- lacks `bl      Counter__get` (runtime dispatch missing), OR
- contains `mov     x0, #0` (result constant-folded)

**Violated if**: `compile_to_asm(PROJ_TWO_TYPES)` returns assembly that:
- lacks both `Meters` and `Feet` labels (one or both instantiations missing), OR
- contains `mov     x0, #5` (two-type sum was constant-folded)

**Tests**: `cargo test --test e2e -- runtime_proj_return_emits_bl_not_folded runtime_proj_two_types_both_monomorphized`

---

## Claim 50: T::AssocType in generic function parameter position emits monomorphized dispatch (not constant-folded)

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: When a generic free function uses `T::AssocType` as a **parameter** type
(e.g., `fn add_extra<C: Container>(c: C, extra: C::Item) -> i32`), the emitted assembly must:
1. Emit a monomorphized label per concrete type (e.g., `add_extra__Counter`).
2. Dispatch to the concrete method via `bl` at runtime.
3. Emit an `add` instruction for the runtime arithmetic.
4. Not constant-fold the result.

Both the single-type and two-type (both concrete types) cases are guarded.

```rust
// Pattern 1: C::Item as extra parameter type
trait Container {
    type Item;
    fn get(&self) -> Self::Item;
}
struct Counter { val: i32 }
impl Container for Counter {
    type Item = i32;
    fn get(&self) -> Self::Item { self.val }
}
fn add_extra<C: Container>(c: C, extra: C::Item) -> i32 { c.get() + extra }
fn make_and_call(n: i32) -> i32 { let c = Counter { val: n }; add_extra(c, n) }
// main: make_and_call(4) → 8

// Pattern 2: two concrete types, both monomorphized
trait Measure {
    type Unit;
    fn measure(&self) -> Self::Unit;
}
struct Meters { val: i32 }
struct Feet { val: i32 }
impl Measure for Meters { type Unit = i32; fn measure(&self) -> Self::Unit { self.val } }
impl Measure for Feet   { type Unit = i32; fn measure(&self) -> Self::Unit { self.val * 3 } }
fn with_offset<M: Measure>(m: M, offset: M::Unit) -> i32 { m.measure() + offset }
// main: with_offset(Meters{val:2}, 1) + with_offset(Feet{val:1}, 0) → 6
```

Must emit (Pattern 1):
- `add_extra__Counter` — monomorphized label for the concrete instantiation
- `bl      Counter__get` — runtime dispatch to the trait method
- `add` — runtime arithmetic on the projected parameter
- NOT `mov     x0, #8` — result must not be folded to a constant

Must emit (Pattern 2):
- Both `with_offset__Meters` and `with_offset__Feet` labels in the assembly
- NOT `mov     x0, #6` — result must not be constant-folded to 6

**Why this claim matters**: Milestone 168 added `T::AssocType` in generic function
parameter position. Without this claim, a regression that folds `add_extra__Counter`
to a constant return would pass all exit-code tests. The generic function must spill
the `C::Item` parameter to a stack slot and use it in runtime arithmetic.

**Attack vectors**:
1. Constant-fold `add_extra__Counter` to `mov x0, #8; ret` — exit code correct, folded.
   Caught by `!asm.contains("mov     x0, #8")` and absence of `add`.
2. Fail to emit `add_extra__Counter` monomorphized label (generic fn not instantiated).
   Caught by `asm.contains("add_extra__Counter")`.
3. Fold two-type case to `mov x0, #6` — both concrete results summed at compile time.
   Caught by `!asm.contains("mov     x0, #6")`.
4. Emit only one concrete type's label (second instantiation missing).
   Caught by checking both `Meters` and `Feet` labels present.

**FLS §10.2**: Associated type projections `T::X` resolve to the concrete type
bound by `T`'s impl of the trait that declares `type X`.
**FLS §12.1 / §10.2 AMBIGUOUS**: The FLS does not specify how `T::X` in parameter
position resolves during monomorphization. Galvanic extends the per-monomorphization
alias map so `C::Item → IrTy::I32` is available when lowering parameter types. The
spec is silent on the mechanism.
**FLS §6.1.2 Constraint 1**: `fn main()` is not a const context — generic calls
with `T::AssocType` parameter types must execute at runtime.

**Violated if**: `compile_to_asm(PARAM_PROJ)` returns assembly that:
- lacks `add_extra__Counter` (monomorphization missing), OR
- lacks `bl      Counter__get` (runtime dispatch missing), OR
- lacks `add` (runtime arithmetic skipped), OR
- contains `mov     x0, #8` (result constant-folded)

**Violated if**: `compile_to_asm(PARAM_PROJ_TWO_TYPES)` returns assembly that:
- lacks both `with_offset__Meters` and `with_offset__Feet` (instantiation missing), OR
- contains `mov     x0, #6` (two-type sum was constant-folded)

**Tests**: `cargo test --test e2e -- runtime_param_proj_emits_add_not_folded runtime_param_proj_two_types_both_monomorphized`

---

## Claim 51: where C::Item: Trait predicate emits monomorphized bl (not constant-folded)

**Stakeholder**: William (researcher), FLS / Ferrocene Ecosystem

**Promise**: A generic function with a `where C::Item: Trait` where-clause
predicate monomorphizes at call sites: `process<Holder>` emits a dedicated
`process__Holder` label with a `bl Holder__get_val` runtime dispatch and does
not fold the result to a constant. Two concrete types both get independent
monomorphized labels.

**Program (single type)**:
```rust
trait Container { type Item; fn get_val(&self) -> i32; }
trait Marker {}
struct Holder { val: i32 }
impl Container for Holder { type Item = i32; fn get_val(&self) -> i32 { self.val } }
impl Marker for i32 {}
fn process<C: Container>(c: C, extra: i32) -> i32 where C::Item: Marker {
    c.get_val() + extra
}
// main: process(Holder{val:3}, 2) → 5
```

Must emit:
- `process__Holder` — monomorphized label
- `bl` dispatching to `process__Holder`
- NOT `mov     x0, #5` — result must not be folded

**Why this claim matters**: Milestone 169 adds `where C::Item: Trait` predicate
parsing. Without this claim, a regression that folds `process__Holder` to a
constant would pass all exit-code tests. The where clause must not suppress
monomorphization or cause the compiler to pre-evaluate the body.

**Attack vectors**:
1. Fold `process__Holder` to `mov x0, #5; ret` — exit code correct, folded.
   Caught by `!asm.contains("mov     x0, #5")`.
2. Fail to emit `process__Holder` label (generic fn not instantiated due to where clause).
   Caught by `asm.contains("process__Holder")`.
3. Emit only one of `process__Holder` / `process__Counter` (second instantiation missing).
   Caught by the two-type test checking both labels.
4. Fold two-type result to `mov x0, #9`.
   Caught by `!asm.contains("mov     x0, #9")`.

**FLS §4.14**: Where clause predicates do not alter the calling convention or
cause compile-time evaluation of non-const functions.
**FLS §10.2 / §4.14 AMBIGUOUS**: The FLS does not specify how `where C::Item: Trait`
constrains dispatch. Galvanic parses the predicate; enforcement relies on the concrete
impl being present at the call site (monomorphization handles it implicitly).
**FLS §6.1.2 Constraint 1**: `fn main()` is not a const context — generic calls
with `where C::Item: Trait` where clauses must execute at runtime.

**Violated if**: `compile_to_asm(WHERE_PROJ)` returns assembly that:
- lacks `process__Holder` (monomorphization missing), OR
- contains `mov     x0, #5` (result constant-folded)

**Violated if**: `compile_to_asm(WHERE_PROJ_TWO_TYPES)` returns assembly that:
- lacks either `process__Holder` or `process__Counter`, OR
- contains `mov     x0, #9` (two-type sum constant-folded)

**Tests**: `cargo test --test e2e -- runtime_where_proj_emits_bl_not_folded runtime_where_proj_two_types_both_monomorphized`

---

## Claim 52: unsafe fn body emits runtime instructions (not constant-folded)

**FLS §19, §9.1**: The `unsafe fn` qualifier does not change codegen. The body of
an `unsafe fn` must emit runtime ARM64 instructions, not compile-time constants.
Calling an `unsafe fn` from an `unsafe { }` block emits a normal `bl` instruction.

**Falsification strategy**: Write two programs:
1. `unsafe fn double(x: i32) -> i32 { x * 2 }` — body must emit `mul`, not fold to `#10`.
2. `unsafe fn add(a: i32, b: i32) -> i32 { a + b }` — call must emit `bl`, not fold to `#7`.

**What would break this**:
1. Folding `unsafe fn` body to a constant (treating it like a const fn).
2. Inlining the call and constant-folding to `mov x0, #10` or `mov x0, #7`.
3. Failing to emit `bl` at the call site.

**FLS §19 AMBIGUOUS**: The spec does not define the enforcement mechanism for
ensuring callers use an unsafe context. Galvanic records `is_unsafe` on `FnDef`
but does not yet enforce the calling context constraint — this would require a
borrow-checker-level pass.

**FLS §6.1.2 Constraint 1**: `fn main()` and `unsafe fn` bodies are not const
contexts — they must execute at runtime.

**Violated if**: `compile_to_asm(UNSAFE_FN)` returns assembly that:
- lacks `mul` in the body of `double`, OR
- contains `mov     x0, #10` (result constant-folded)

**Violated if**: `compile_to_asm(UNSAFE_FN_ADD)` returns assembly that:
- lacks `bl` at the call site, OR
- contains `mov     x0, #7` (result constant-folded)

**Tests**: `cargo test --test e2e -- runtime_unsafe_fn_body_emits_mul_not_folded runtime_unsafe_fn_call_emits_bl_not_folded`

---

## Claim 53: unsafe trait method call emits runtime bl and mul (not constant-folded)

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: `unsafe trait` + `unsafe impl` codegen is identical to regular
trait + impl. The qualifier is a static safety contract only — no difference
in emitted assembly. An `unsafe impl` method body must emit runtime instructions
(e.g., `mul`), not compile-time constants. The call site must emit `bl`, not a
folded constant.

**What this guards against**:
1. Treating `unsafe impl` as a special case that folds the method body.
2. Inlining the call and constant-folding to `mov x0, #14`.
3. Failing to emit `bl` at the call site.

**FLS §19 AMBIGUOUS**: The spec requires `unsafe impl` when implementing an
`unsafe trait`, but does not specify how the compiler verifies this pairing.
Galvanic records `is_unsafe` on both `TraitDef` and `ImplDef` but does not
yet enforce the pairing — enforcement is deferred.

**FLS §6.1.2 Constraint 1**: Method bodies are not const contexts — they must
execute at runtime.

**Violated if**: `compile_to_asm(UNSAFE_TRAIT_SCALE)` returns assembly that:
- lacks `mul` in the method body, OR
- lacks `bl` at the call site, OR
- contains `mov     x0, #14` (result constant-folded)

**Tests**: `cargo test --test e2e -- runtime_unsafe_trait_method_emits_bl_not_folded runtime_unsafe_trait_body_emits_mul_not_folded`

---

## Claim 54: `unsafe fn` inside `unsafe trait` emits runtime bl and mul (not constant-folded)

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: The combination of `unsafe fn` declared inside `unsafe trait` and
implemented in `unsafe impl` — M170 + M171 together — must produce correct
runtime code. The qualifier is a static safety contract only. The method body
must emit runtime instructions (e.g., `mul`), the call site must emit `bl`, and
the result must not be constant-folded.

This is distinct from:
- Claim 52: standalone top-level `unsafe fn` (not inside a trait)
- Claim 53: `unsafe trait` with regular (non-unsafe) `fn` methods

**What this guards against**:
1. A special case that treats `unsafe fn` inside `unsafe impl` differently from
   a top-level `unsafe fn`, causing the body to fold.
2. Constant-folding `3 * 4 + 5 = 17` when both qualifiers are present.
3. Failing to emit `bl` for `m.compute(4, 5)` when the method is `unsafe fn`.

**FLS §19 AMBIGUOUS**: The spec does not specify how `unsafe fn` inside an
`unsafe trait` interacts with enforcement. Galvanic defers enforcement of both
qualifiers — the `is_unsafe` flags on `FnDef`, `TraitDef`, and `ImplDef` are
recorded but not enforced.

**FLS §6.1.2 Constraint 1**: `unsafe fn` bodies are not const contexts — they
must execute at runtime, identical to regular fn bodies.

**Violated if**: `compile_to_asm(UNSAFE_FN_IN_UNSAFE_TRAIT)` returns assembly that:
- lacks `mul` in the method body, OR
- lacks `bl` at the call site, OR
- contains `mov x0, #17` (result 3*4+5=17 constant-folded)

**Tests**: `cargo test --test e2e -- runtime_unsafe_fn_in_unsafe_trait_emits_bl_and_mul_not_folded`

---

## Claim 55: `unsafe impl<T>` for a generic type emits runtime bl and mul (not constant-folded)

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: `unsafe impl<T> Trait for Wrapper<T>` codegen is identical to
`impl<T> Trait for Wrapper<T>` (M138). The `unsafe` qualifier is a static
safety contract — no difference in emitted assembly. A method body with an
arithmetic expression (`self.inner * n`) must emit a runtime `mul` instruction,
not a constant. The call site must emit `bl`.

This is distinct from:
- Claim 53: `unsafe impl` without generics (non-generic impl block)
- Claim 9/12: generic trait impl without `unsafe`

**What this guards against**:
1. Treating `unsafe impl<T>` as a special case that folds the method body.
2. Inlining the monomorphized call and constant-folding to `mov x0, #12`.
3. Failing to emit `bl` at the call site for a generic unsafe method.

**FLS §19 AMBIGUOUS**: The spec does not specify how `unsafe impl<T>` interacts
with generic monomorphization. Galvanic defers enforcement — `is_unsafe` on
`ImplDef` is recorded but not checked against the corresponding `unsafe trait`.

**FLS §12.1 AMBIGUOUS**: The spec does not specify the disambiguation rule for
`<` immediately after `unsafe impl`. Galvanic treats it as always starting a
generic parameter list.

**FLS §6.1.2 Constraint 1**: Method bodies are not const contexts — they must
execute at runtime.

**Violated if**: `compile_to_asm(UNSAFE_GENERIC_SCALE)` returns assembly that:
- lacks `mul` in the method body, OR
- lacks `bl` at the call site, OR
- contains `mov x0, #12` (result 3*4=12 constant-folded)

**Tests**: `cargo test --test e2e -- runtime_unsafe_generic_impl_body_emits_mul_not_folded runtime_unsafe_generic_impl_call_emits_bl_not_folded`

---

## Claim 56: `unsafe impl<T: Bound>` emits runtime bl and mul (not constant-folded)

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: `unsafe impl<T: Bound> Trait for Wrapper<T>` codegen is identical to
`unsafe impl<T> Trait for Wrapper<T>` (Claim 55). Adding a trait bound to the
type parameter is a compile-time safety contract — it constrains which concrete
types may be used but does not change the emitted assembly. A method body with
arithmetic (`self.inner * n`) must emit a runtime `mul` instruction. The call
site must emit `bl`. The bound `T: Marker` must not cause the method body to be
inlined or constant-folded.

This is distinct from:
- Claim 55: `unsafe impl<T>` without bounds
- Claim 9/12: generic trait impl without `unsafe`

**What this guards against**:
1. The inline bound `T: Bound` causing the parser or lowerer to mishandle the impl.
2. Treating `unsafe impl<T: Bound>` as a special case that folds the method body.
3. Failing to emit `bl` at the call site for a bounded unsafe generic method.

**FLS §19 AMBIGUOUS**: The spec does not specify how `unsafe impl<T: Bound>`
interacts with generic monomorphization. Galvanic records `is_unsafe` and the
bound on `ImplDef` but enforces neither at runtime.

**FLS §12.1, §4.14**: Inline bounds (`T: TraitName`) in generic parameter lists
are parsed and discarded during lowering — they constrain which types are valid
at the call site (static property) but do not affect codegen.

**FLS §6.1.2 Constraint 1**: Method bodies are not const contexts — they must
execute at runtime regardless of trait bounds on the type parameter.

**Violated if**: `compile_to_asm(UNSAFE_BOUNDED_SCALE)` returns assembly that:
- lacks `mul` in the method body, OR
- lacks `bl` at the call site, OR
- contains `mov x0, #12` (result 3*4=12 constant-folded)

**Tests**: `cargo test --test e2e -- runtime_unsafe_bounded_impl_body_emits_mul_not_folded runtime_unsafe_bounded_impl_call_emits_bl_not_folded`

---

## Claim 57: large-value integer arithmetic emits runtime instructions, not constant-folded

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: Integer arithmetic with large operands (e.g., `f(2000000000, 1)`)
must emit runtime ARM64 `add`/`mul` instructions at the call site. The result
must not be constant-folded into an immediate load sequence. This guards against
a regression where the constant propagation pass evaluates function-call arguments
at compile time and replaces the call with a `mov #result`.

This also documents FLS §6.23 conformance: galvanic emits 64-bit ARM64 arithmetic
instructions and does not insert overflow checks. This differs from both Rust
debug mode (which panics on overflow) and release mode (which wraps at 32-bit
two's complement boundaries) because galvanic uses 64-bit registers for i32
arithmetic throughout.

**FLS §6.23 AMBIGUOUS**: The spec requires:
- Debug mode: integer overflow panics at runtime.
- Release mode: integer overflow wraps in two's complement.
Galvanic does neither — it uses 64-bit arithmetic without a mode distinction.
The conformance gap is a research output of this project.

**FLS §6.5.5**: The addition and multiplication operators must emit `add` and
`mul` ARM64 instructions respectively.

**FLS §6.1.2 Constraint 1**: Function call bodies are not const contexts —
the arithmetic must execute at runtime even when inputs are statically known.

**Violated if**: `compile_to_asm(LARGE_ADD)` or `compile_to_asm(LARGE_MUL)` returns
assembly that:
- lacks `add`/`mul` in the function body, OR
- lacks `bl` at the call site, OR
- contains the folded constant (`2000000001` or `2000000000`) as an immediate

**Tests**: `cargo test --test e2e -- runtime_large_int_add_emits_add_not_folded runtime_large_int_mul_emits_mul_not_folded`

---

## Claim 58: large-value integer subtraction and division emit runtime instructions, not constant-folded

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: Integer subtraction and division with large operands (e.g., `f(2000000000, 1)`
for sub, `f(2000000000, 4)` for div) must emit runtime ARM64 `sub`/`sdiv` instructions
at the call site. The result must not be constant-folded into an immediate load sequence.

This completes the Claim 57 story: Claim 57 verifies `add` and `mul`; Claim 58 verifies
`sub` and `sdiv`. Together they cover all four basic arithmetic operators with the
adversarial function-parameter pattern. A folding interpreter that special-cased addition
and multiplication but constant-evaluated subtraction or division would pass Claim 57 and
fail Claim 58.

**FLS §6.1.2 Constraint 1**: Function call bodies are not const contexts —
arithmetic must execute at runtime even when inputs are statically known at the call site.

**FLS §6.5.5**: The subtraction and division operators must emit `sub` and `sdiv`
ARM64 instructions respectively.

**Violated if**: `compile_to_asm(LARGE_SUB)` or `compile_to_asm(LARGE_DIV)` returns
assembly that:
- lacks `sub`/`sdiv` in the function body, OR
- lacks `bl` at the call site, OR
- contains the folded constant (`1999999999` or `500000000`) as an immediate

**Tests**: `cargo test --test e2e -- runtime_large_int_sub_emits_sub_not_folded runtime_large_int_div_emits_sdiv_not_folded`

---

## Claim 59: large negative i32 constants are sign-extended for correct 64-bit comparisons

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: A large negative i32 constant (absolute value > 65535, e.g., -100000)
loaded via the MOVZ+MOVK+sxtw sequence must be correctly sign-extended to 64 bits.
A pattern match `match x { -100000 => 1, _ => 0 }` where `x` is a function parameter
carrying -100000 must return 1.

Without sign extension, MOVZ+MOVK loads 0x00000000FFFE7960 into the 64-bit register.
A function parameter carrying -100000 holds 0xFFFFFFFFFFFE7960 (from the 64-bit `neg`
instruction). These differ in 64-bit signed comparison → the match arm is never taken.
With `sxtw`, both representations agree at 0xFFFFFFFFFFFE7960.

**FLS §2.4.4.1**: Integer literals have i32 type; the ARM64 encoding must preserve
signed semantics in 64-bit registers.
**FLS §5.2**: Literal patterns (including negative literal patterns) use LoadImm to
materialise the pattern value; this value must compare correctly against parameters.
**FLS §6.5.7**: Comparison operators use signed 64-bit `cmp`; operands must be
correctly sign-extended.

**Violated if**: `compile_to_asm(LARGE_NEG_PATTERN)` returns assembly that:
- lacks `sxtw` (no sign extension), OR
- `compile_and_run(LARGE_NEG_PATTERN)` returns 0 instead of 1 (wrong arm taken)

**Tests**: `cargo test --test e2e -- runtime_large_neg_const_emits_sxtw runtime_large_neg_const_pattern_not_folded milestone_175_neg_large_pattern_match_taken`

---

## Claim 60: remainder operator emits runtime `sdiv`+`msub`, not constant-folded

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: The remainder operator `%` must emit `sdiv` and `msub` ARM64 instructions
at runtime for a function whose operands are parameters (unknown at compile time).
The call site must emit `bl`. The result must not be constant-folded into an immediate.

The original `runtime_rem_emits_sdiv_and_msub` test used `fn main() -> i32 { 10 % 3 }` —
inline literals. A constant-folding interpreter could emit `mov x0, #1` and pass that
test. Claim 60 requires function parameters, which are unknown at compile time.

This completes the Claim 57/58 story for all five arithmetic operators:
`add` (Claim 57), `mul` (Claim 57), `sub` (Claim 58), `sdiv` (Claim 58), `rem` (Claim 60).

**FLS §6.5.5**: The remainder operator must execute at runtime in non-const contexts.
**FLS §6.1.2 Constraint 1**: Function bodies are not const contexts; inputs are not
statically known when the function is called with runtime parameters.

**Violated if**: `compile_to_asm("fn f(x: i32, y: i32) -> i32 { x % y } ...")`:
- lacks `sdiv` in the function body, OR
- lacks `msub` in the function body, OR
- lacks `bl` at the call site, OR
- contains `mov x0, #1` (constant-folded result of 10 % 3) in main

**Tests**: `cargo test --test e2e -- runtime_rem_emits_sdiv_and_msub_not_folded`

## Claim 61: Shift operators emit runtime `lsl`/`asr` instructions with function parameters (not constant-folded)

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: The left-shift (`<<`) and right-shift (`>>`) operators must emit
ARM64 `lsl` and `asr` instructions at runtime when the operands are function
parameters (unknown at compile time). The call site must emit `bl`. The result
must not be constant-folded into an immediate. For signed `i32` right-shift,
the instruction must be `asr` (arithmetic, sign-extending) and NOT `lsr`
(logical, zero-filling).

The original `runtime_shl_emits_lsl_instruction` and `runtime_shr_emits_asr_instruction`
tests used `fn main() -> i32 { 1 << 3 }` / `fn main() -> i32 { 16 >> 2 }` —
inline literals in `main`. A constant-folding interpreter could emit `mov x0, #8`
or `mov x0, #4` respectively. These tests had positive assertions only (no check
that the folded constant is absent), and no falsification claim.

Claim 61 uses function parameters (`fn shl(x: i32, n: i32) -> i32 { x << n }`,
`fn shr_i32(x: i32, n: i32) -> i32 { x >> n }`) which are unknown at compile time.
It also adds negative assertions confirming the folded constants are absent, and
guards the signed/unsigned correctness: `i32 >> n` must use `asr`, not `lsr`.

**FLS §6.5.7**: Shift operators. Right shift on signed integers is arithmetic
(sign-extending). Right shift on unsigned integers is logical (zero-filling).
**FLS §6.1.2 Constraint 1**: Function bodies are not const contexts; shift operands
from function parameters are not statically known.

**Violated if** `compile_to_asm("fn shl(x: i32, n: i32) -> i32 { x << n } ...")`:
- lacks `lsl` in the function body, OR
- lacks `bl shl` at the call site, OR
- contains `mov x0, #8` (constant-folded result of shl(1, 3))

**Violated if** `compile_to_asm("fn shr_i32(x: i32, n: i32) -> i32 { x >> n } ...")`:
- lacks `asr` in the function body, OR
- lacks `bl shr_i32` at the call site, OR
- contains `mov x0, #4` (constant-folded result of shr_i32(16, 2)), OR
- contains `lsr` (wrong instruction for signed right-shift)

**Tests**: `cargo test --test e2e -- runtime_shl_emits_lsl_not_folded runtime_shr_emits_asr_not_folded`

## Claim 62: Bitwise operators emit runtime `and`/`orr`/`eor` instructions with function parameters (not constant-folded)

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: The bitwise AND (`&`), OR (`|`), and XOR (`^`) operators must emit
ARM64 `and`, `orr`, and `eor` instructions at runtime when the operands are function
parameters (unknown at compile time). The call site must emit `bl`. The results
must not be constant-folded into immediates.

The original `runtime_and_emits_and_instruction`, `runtime_or_emits_orr_instruction`,
and `runtime_xor_emits_eor_instruction` tests used inline literals in `main`
(`5 & 3`, `5 | 3`, `5 ^ 3`). A constant-folding interpreter could emit `mov x0, #1`,
`mov x0, #7`, or `mov x0, #6` respectively and pass the positive-only assertions.
`runtime_or_emits_orr_instruction` and `runtime_xor_emits_eor_instruction` had
no negative assertions at all.

Claim 62 uses function parameters (`fn bitand(x: i32, y: i32) -> i32 { x & y }`,
etc.) which are unknown at compile time, preventing constant folding. It adds
negative assertions confirming the folded constants are absent.

**FLS §6.5.6**: Bitwise expression operators (`&`, `|`, `^`).
**FLS §6.1.2 Constraint 1**: Function bodies are not const contexts; operands from
function parameters are not statically known at compile time.

**Violated if** `compile_to_asm("fn bitand(x: i32, y: i32) -> i32 { x & y } ...")`:
- lacks `and` in the function body, OR
- lacks `bl bitand` at the call site, OR
- contains `mov x0, #1` (constant-folded result of 5 & 3)

**Violated if** `compile_to_asm("fn bitor(x: i32, y: i32) -> i32 { x | y } ...")`:
- lacks `orr` in the function body, OR
- lacks `bl bitor` at the call site, OR
- contains `mov x0, #7` (constant-folded result of 5 | 3)

**Violated if** `compile_to_asm("fn bitxor(x: i32, y: i32) -> i32 { x ^ y } ...")`:
- lacks `eor` in the function body, OR
- lacks `bl bitxor` at the call site, OR
- contains `mov x0, #6` (constant-folded result of 5 ^ 3)

**Tests**: `cargo test --test e2e -- runtime_and_emits_and_not_folded runtime_or_emits_orr_not_folded runtime_xor_emits_eor_not_folded`

## Claim 63: Unary operators emit runtime `neg`/`mvn`/`eor` instructions with function parameters (not constant-folded)

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: The unary negation (`-`), bitwise NOT (`!` on integers), and logical
NOT (`!` on bool) operators must emit ARM64 `neg`, `mvn`, and `eor` instructions
at runtime when the operand is a function parameter (unknown at compile time).
The call site must emit `bl`. The results must not be constant-folded into immediates.

The existing `runtime_neg_emits_neg_instruction` used a function parameter but lacked
a negative assertion for the call site and no Claim entry. The existing
`runtime_not_emits_mvn_instruction` used `fn main() -> i32 { !5 }` — an inline
literal in main, not a function parameter. The existing
`runtime_bool_not_emits_eor_instruction` used a parameter and had a partial negative
assertion (`!mvn`) but no call-site `bl` check and no Claim entry.

Claim 63 adds parameter-based tests with complete positive assertions (`neg`/`mvn`/`eor`),
call-site assertions (`bl neg_i32` / `bl bitwise_not` / `bl bool_not`), and negative
assertions confirming the folded constants are absent.

**FLS §6.5.4**: Negation operator expressions. Unary `-` on integers emits `neg`.
Bitwise `!` on integers emits `mvn`. Logical `!` on bool emits `eor x{dst}, x{src}, #1`.
**FLS §6.1.2 Constraint 1**: Function bodies are not const contexts; operands from
function parameters are not statically known at compile time.

**Violated if** `compile_to_asm("fn neg_i32(x: i32) -> i32 { -x } ...")`:
- lacks `neg` in the function body, OR
- lacks `bl neg_i32` at the call site, OR
- contains `mov x0, #-5` (constant-folded result of neg_i32(5))

**Violated if** `compile_to_asm("fn bitwise_not(x: i32) -> i32 { !x } ...")`:
- lacks `mvn` in the function body, OR
- lacks `bl bitwise_not` at the call site, OR
- contains `mov x0, #-6` (constant-folded result of bitwise_not(5))

**Violated if** `compile_to_asm("fn bool_not(b: bool) -> bool { !b } ...")`:
- lacks `eor` and `#1` in the function body, OR
- lacks `bl bool_not` at the call site, OR
- contains `mvn` (wrong instruction for bool NOT)

**Tests**: `cargo test --test e2e -- runtime_neg_emits_neg_not_folded runtime_not_emits_mvn_not_folded runtime_bool_not_emits_eor_not_folded`

---

## Claim 64: u8 arithmetic emits runtime instructions and `and` truncation (not constant-folded)

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: The `u8` type wraps at 256 per FLS §6.23. When a function returns
`u8`, galvanic must emit both:
1. The arithmetic instruction (`add`, `sub`, `mul`, etc.) at runtime — proving this is
   a compiler, not an interpreter, and
2. An `and w{r}, w{r}, #255` instruction (TruncU8) at the return boundary —
   proving the overflow wrapping semantics are correctly implemented for ALL operators,
   not just `add`.

Without TruncU8, `200_u8 + 100_u8` would return 300 instead of 44. Similarly,
`15_u8 * 20_u8 = 300` must wrap to 44. The falsification suite tests both add and mul
to prevent operator-specific regressions (e.g., TruncU8 disabled for mul only).

**FLS §4.1**: "The unsigned integer types have a range of [0, 2^N - 1]."
**FLS §6.23**: At runtime, integer arithmetic wraps in two's complement. For u8, the
modulus is 256.
**FLS §6.1.2 Constraint 1**: Function parameters are not const contexts; the arithmetic
must be emitted at runtime even though the values happen to be known at the call site.

**Violated if** `compile_to_asm("fn add_u8(a: u8, b: u8) -> u8 { a + b } ...")`:
- lacks `add` in the function body (no arithmetic emitted), OR
- lacks `and ... #255` (no truncation, wrapping semantics broken), OR
- contains `mov x0, #44` (constant-folded to the wrapped result without runtime code)

**Violated if** `compile_to_asm("fn mul_u8(a: u8, b: u8) -> u8 { a * b } ...")`:
- lacks `mul` in the function body (no arithmetic emitted), OR
- lacks `and ... #255` (TruncU8 missing for mul — operator-specific regression), OR
- contains `mov x0, #44` (constant-folded — `15 * 20 = 300 mod 256 = 44`)

**Tests**: `cargo test --test e2e -- runtime_u8_add_emits_and_truncation runtime_u8_mul_emits_and_truncation milestone_176_u8_add_wraps milestone_176_u8_mul_wraps`

---

## Claim 65: i8 arithmetic emits runtime instructions and `sxtb` sign-extension (not constant-folded)

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: The `i8` type wraps at -128/127 per FLS §6.23. When a function returns
`i8`, galvanic must emit both:
1. The arithmetic instruction (`add`, `sub`, `mul`, etc.) at runtime — proving this is
   a compiler, not an interpreter, and
2. A `sxtb w{r}, w{r}` instruction (SextI8) at the return boundary — proving the
   overflow wrapping semantics are correctly implemented for signed 8-bit arithmetic.

Without SextI8, `100_i8 + 50_i8` would return 150 instead of -106. The falsification
suite tests both the assembly inspection (SextI8 present) and the runtime correctness
(wrapping result matches the expected value).

**FLS §4.1**: "The signed integer types have a range of [-2^(N-1), 2^(N-1)-1]."
**FLS §6.23**: At runtime, integer arithmetic wraps in two's complement. For i8, the
range is -128..=127.
**FLS §6.1.2 Constraint 1**: Function parameters are not const contexts; the arithmetic
must be emitted at runtime even though the values happen to be known at the call site.

**Violated if** `compile_to_asm("fn add_i8(a: i8, b: i8) -> i8 { a + b } ...")`:
- lacks `add` in the function body (no arithmetic emitted), OR
- lacks `sxtb` (no sign-extension, wrapping semantics broken), OR
- contains `mov x0, #150` or `mov x0, #-106` (constant-folded without runtime code)

**Tests**: `cargo test --test e2e -- runtime_i8_add_emits_sxtb_sign_extension milestone_177_i8_add_wraps milestone_177_i8_sub_wraps`

---

## Claim 66: narrow integer compound assignment wraps correctly mid-body (not only at return boundaries)

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: When a `u8`, `i8`, `u16`, or `i16` local variable is updated via compound
assignment (`+=`, `-=`, `*=`, etc.) and then read back within the same function body,
the value must already be in the type's range — regardless of whether the function's
return type is a narrow integer or not. This applies to ALL compound-assignment operators,
not only `+=`.

The existing TruncU8/SextI8/TruncU16/SextI16 at function return boundaries is
insufficient because a variable can be read in comparisons, casts, or passed to other
functions before the enclosing function returns. Without normalization at the
compound-assignment store, the slot holds an unwrapped value and mid-body reads see
wrong results.

This was a real latent bug for `+=`; the same path applies to `*=`. The normalization
at `lower.rs:14527–14539` applies after ANY BinOp in a compound-assignment, but was
only tested for `+=` prior to milestone 186.

**FLS §4.1**: narrow integer ranges (u8: 0..=255, i8: -128..=127, u16: 0..=65535, i16: -32768..=32767).
**FLS §6.23**: At runtime, integer arithmetic wraps. The wrapping must be observable
immediately after the operation, not only at function boundaries.
**FLS §6.1.2 Constraint 1**: The operation is runtime; so is the wrapping normalization.

**Violated if** for `fn test(a: u8, b: u8) -> i32 { let mut x: u8 = a; x *= b; if x < 50 { 1 } else { 0 } }` called with `(15, 20)`:
- returns 0 instead of 1 (comparison sees 300 instead of wrapped 44), OR
- the function body lacks `and ... #255` before the store-back for `*=`

**Tests**: `cargo test --test e2e -- runtime_u8_compound_add_emits_trunc_u8 runtime_i8_compound_add_emits_sext_i8 milestone_178_u8_compound_add_wraps_mid_body milestone_178_i8_compound_add_wraps_mid_body runtime_u8_compound_mul_emits_trunc_u8 runtime_i8_compound_mul_emits_sext_i8 milestone_186_u8_compound_mul_wraps_mid_body milestone_186_i8_compound_mul_wraps_mid_body runtime_u16_compound_mul_emits_trunc_u16 runtime_i16_compound_mul_emits_sext_i16 milestone_186_u16_compound_mul_wraps_mid_body milestone_186_i16_compound_mul_wraps_mid_body`

---

## Claim 67: `x as u8` and `x as i8` narrowing casts truncate correctly (not identity)

**FLS §6.5.9**: An integer-to-integer cast to a narrower type retains only the low N bits
of the source value. `300_i32 as u8` → 44 (300 & 255). `200_i32 as i8` → -56 (0xC8
sign-extended). Previously galvanic treated these as identity casts.

**ARM64 implementation**:
- `as u8` → `and w{dst}, w{dst}, #255` (TruncU8)
- `as i8` → `sxtb x{dst}, w{dst}` (SextI8)

Without explicit truncation, `300_i32 as u8` would produce 300 in the destination
register. Any subsequent comparison, store, or arithmetic would use the unwrapped value,
producing incorrect results.

**FLS §6.5.9**: "An integer-to-integer cast to a narrower integer type truncates to the
least-significant bits." This is not ambiguous — truncation is required.

**Violated if** for `fn f(x: i32) -> i32 { (x as u8) as i32 }` called with 300:
- returns 300 instead of 44, OR
- the function body lacks `and ... #255` in the assembly output

**Tests**: `cargo test --test e2e -- runtime_cast_to_u8_emits_and_truncation runtime_cast_to_i8_emits_sxtb milestone_179_cast_u8_truncates_300_to_44 milestone_179_cast_i8_sign_extends_200_to_negative`

---

## Claim 68: `x as u16` and `x as i16` narrowing casts truncate correctly (not identity)

**FLS §6.5.9**: An integer-to-integer cast to a narrower type retains only the low N bits.
`70000_i32 as u16` → 4464 (70000 mod 65536). `40000_i32 as i16` → negative (bit 15 set).
Previously galvanic treated these as identity casts — no truncation emitted.

**ARM64 implementation**:
- `as u16` → `and w{dst}, w{dst}, #65535` (TruncU16)
- `as i16` → `sxth x{dst}, w{dst}` (SextI16)

Without explicit truncation, `70000_i32 as u16` would produce 70000 in the destination
register. Any subsequent comparison, store, or arithmetic would use the unwrapped value,
producing incorrect results.

**Violated if** for `fn f(x: i32) -> i32 { (x as u16) as i32 }` called with 70000:
- returns 70000 instead of 4464, OR
- the function body lacks `and ... #65535` in the assembly output

**Tests**: `cargo test --test e2e -- runtime_cast_to_u16_emits_and_truncation runtime_cast_to_i16_emits_sxth milestone_180_cast_u16_truncates_70000_to_4464 milestone_180_cast_i16_sign_extends_40000_to_negative`

---

## Claim 69: u16/i16 arithmetic wraps at 16-bit boundaries (not identity through 32-bit registers)

**Stakeholder**: William (researcher), FLS / Ferrocene ecosystem

**Promise**: When a function declares `u16` or `i16` parameters and return type,
arithmetic on those values wraps at 16-bit boundaries. `65500_u16 + 100_u16`
returns 64 (not 65600). `32760_i16 + 100_i16` returns -32676 (not 32860).

Previously `u16` mapped to `IrTy::U32` and `i16` to `IrTy::I32` — no boundary
normalization was emitted. Return values were not truncated/sign-extended, so
overflow was silently swallowed in the 32-bit register representation.

**ARM64 implementation**:
- `-> u16` return boundary: `and w{r}, w{r}, #65535` (TruncU16)
- `-> i16` return boundary: `sxth x{r}, w{r}` (SextI16)
- `x += b` where `x: u16`: TruncU16 before store-back
- `x += b` where `x: i16`: SextI16 before store-back

**FLS §4.1**: u16 range is 0..=65535; i16 range is -32768..=32767.
**FLS §6.23**: At runtime without overflow checks, integer arithmetic wraps in
two's complement. The wrapping must be observable at the type's own boundaries.
**FLS §6.1.2 Constraint 1**: Non-const code generates runtime instructions.

**Violated if** for `fn add_u16(a: u16, b: u16) -> u16 { a + b }` called with `(65500, 100)`:
- returns 65600 instead of 64, OR
- the function body lacks `and ... #65535` in the assembly output

**Tests**: `cargo test --test e2e -- runtime_u16_add_emits_and_truncation runtime_i16_add_emits_sxth milestone_181_u16_add_wraps milestone_181_i16_add_wraps milestone_181_u16_compound_add_wraps_mid_body milestone_181_i16_compound_add_wraps_mid_body`

---

## Claim 70: narrow integer types in struct fields are normalised at construction time

**Stakeholder**: William (researcher), FLS / Ferrocene ecosystem

**Promise**: When a named struct has a field declared as `u8`, `i8`, `u16`, or
`i16`, constructing the struct with an overflowing arithmetic expression for that
field stores the correctly wrapped value. A subsequent comparison against the
field (not a return) must see the wrapped value.

Previously, `Narrow { val: a + b }` where `val: u16` stored the raw i32 sum
(e.g., 66000) instead of the wrapped u16 value (464). A comparison `n.val < 500`
then incorrectly returned false (66000 > 500) instead of true (464 < 500).

**ARM64 implementation**: TruncU8/SextI8/TruncU16/SextI16 emitted immediately
before the Store instruction for each narrow-typed field in every struct literal
construction path.

**FLS §4.1**: Narrow integer types have their own value ranges.
**FLS §6.23**: Runtime arithmetic wraps within the type's declared bounds.
**FLS §6.11**: Struct literal field initializers are evaluated and stored as the
declared field type — not as a wider i32.

**Violated if** for `struct Narrow { val: u16 }`, `fn build(a: u16, b: u16) -> Narrow`:
- `build(65000, 1000).val < 500` returns false instead of true (464 < 500), OR
- the assembly for `Narrow { val: a + b }` lacks `and ... #65535` before the Store

**Tests**: `cargo test --test e2e -- runtime_u16_struct_field_construction_applies_trunc milestone_182_u16_struct_field_wraps_on_construction milestone_182_u8_struct_field_wraps_on_construction milestone_182_i16_struct_field_wraps_on_construction`

---

## Claim 71: Narrow integer enum tuple variant fields normalised at construction time

**Promise**: When constructing an enum tuple variant whose field type is `u8`, `i8`, `u16`, or `i16`, galvanic normalises the stored value at construction time — truncating for unsigned types (`and w_, w_, #255` / `#65535`) or sign-extending for signed types (`sxtb` / `sxth`). This ensures that subsequent reads of the field see a correctly wrapped value, not the raw 32-bit arithmetic result.

**Why this matters**: This is the last construction form that must normalise narrow types (named structs — Milestone 182; tuple structs — Milestone 183; enum tuple variants — Milestone 184). Without it, an enum variant `Wrap::Val(200_u8 + 100_u8)` would store 300 and the pattern binding `Wrap::Val(v)` would give `v = 300` instead of `44`, breaking the semantics of narrow integer types.

**ARM64 implementation**: TruncU8/SextI8/TruncU16/SextI16 emitted immediately before the Store instruction for each narrow-typed field in every enum variant construction path (both tuple and named variants, both in let-binding and return contexts).

**FLS §4.1**: Narrow integer types have their own value ranges.
**FLS §6.23**: Runtime arithmetic wraps within the type's declared bounds.
**FLS §15**: Enum variant constructors evaluate field initializers and store them as the declared field type.

**Violated if** for `enum Wrap { Val(u8) }`, `fn build(a: u8, b: u8) -> Wrap`:
- `match build(200, 100) { Wrap::Val(v) => v < 100 }` returns false instead of true (44 < 100), OR
- the assembly for `Wrap::Val(a + b)` lacks `and w_, w_, #255` before the Store

**Tests**: `cargo test --test e2e -- runtime_u8_enum_variant_field_construction_applies_trunc runtime_u16_enum_variant_field_construction_not_constant_folded milestone_184_u8_enum_tuple_variant_wraps_on_construction milestone_184_u16_enum_tuple_variant_wraps_on_construction milestone_184_i16_enum_tuple_variant_wraps_on_construction`


---

## Claim 72: Narrow integer named enum variant fields normalised at construction time

**Promise**: When constructing a named-field enum variant whose field type is `u8`, `i8`, `u16`, or `i16`, galvanic normalises the stored value at construction time — truncating for unsigned types (`and w_, w_, #255` / `#65535`) or sign-extending for signed types (`sxtb` / `sxth`). This ensures that subsequent pattern bindings see a correctly wrapped value.

**Why this matters**: Cycle 126 implemented narrow normalisation for named enum variants but added no tests. Without this claim, a regression in the named-variant construction path would be invisible — all existing tests use only tuple variants. `enum Color { Red { val: u8 } }` and `Wrap::Val(x: u8)` must both normalise on construction.

**ARM64 implementation**: TruncU8/SextI8/TruncU16/SextI16 emitted immediately before the Store instruction for each narrow-typed named field, using `enum_variant_field_narrow_ty` with the field's positional index derived from the named field list.

**FLS §4.1**: Narrow integer types have their own value ranges.
**FLS §6.23**: Runtime arithmetic wraps within the type's declared bounds.
**FLS §15**: Enum variant constructors evaluate field initializers and store them as the declared field type — named and tuple variants must behave identically.

**Violated if** for `enum Wrap { Val { x: u8 } }`, `fn build(a: u8, b: u8) -> Wrap`:
- `match build(200, 100) { Wrap::Val { x: v } => v < 100 }` returns false instead of true (44 < 100), OR
- the assembly for `Wrap::Val { x: a + b }` lacks `and w_, w_, #255` before the Store

**Tests**: `cargo test --test e2e -- runtime_u8_named_enum_variant_field_construction_applies_trunc runtime_i16_named_enum_variant_field_construction_applies_sxth milestone_185_u8_named_variant_wraps_on_construction milestone_185_u16_named_variant_wraps_on_construction milestone_185_i16_named_variant_wraps_on_construction`


---

## Claim 73: Built-in integer type associated constants resolve to correct compile-time immediates

**Promise**: `i32::MAX`, `i32::MIN`, `u8::MAX`, `u8::MIN`, `i8::MAX`, `i8::MIN`, `u16::MAX`, `u16::MIN`, `i16::MAX`, `i16::MIN` are resolved at compile time to the correct values and emitted as `LoadImm` instructions. They are NOT loaded from memory at runtime.

**Why this matters**: These constants appear frequently in real Rust code as boundary values for arithmetic, comparisons, and type casts. If galvanic does not recognise them, programs that use `i32::MAX` as a sentinel or comparison bound would fail to compile. If they were emitted as runtime stack loads instead of immediates, the cache-line and code density advantages of the approach would be lost.

**ARM64 implementation**: Values are pre-seeded into `assoc_const_vals` before the user-defined impl scan. The existing two-segment path lowering (FLS §10.3) emits `LoadImm(r, value)` → 1–3 ARM64 instructions (movz / movz+movk / movz+movk+sxtw depending on magnitude and sign).

**FLS §10.3**: Associated constants are substituted at each use site as compile-time immediates.
**FLS §4.1 AMBIGUOUS**: The spec specifies value ranges for integer types but does not enumerate `MAX`/`MIN` by name. Galvanic derives these from the range bounds. The exact naming is a language convention, not mandated by the FLS.

**Violated if**:
- `if i32::MAX > 0 { 1 } else { 0 }` returns 0 instead of 1, OR
- `if i32::MIN < 0 { 1 } else { 0 }` returns 0 instead of 1, OR
- The assembly for `i32::MAX` contains `ldr x0, [sp` (stack load) instead of `movz`/`movk`

**Tests**: `cargo test --test e2e -- runtime_i32_max_emits_loadimm milestone_187_i32_max_is_positive milestone_187_i32_min_is_negative milestone_187_u8_max_as_i32 milestone_187_i8_max_as_i32`


---

## Claim 74: Built-in integer associated constants work in const item initializers (not just runtime expressions)

**Promise**: `i32::MAX`, `i32::MIN`, and their narrow-integer counterparts can appear as operands in `const` item initializers. `const LIMIT: i32 = i32::MAX;` evaluates correctly at compile time, and a subsequent `const DERIVED: i32 = LIMIT - 1;` also resolves correctly via the fixed-point evaluator. Neither resolves to zero, None, or fails silently.

**Why this matters**: Claim 73 verifies that `i32::MAX` works in *runtime expressions* (the two-segment path is lowered as an `Instr::LoadImm`). Claim 74 verifies the distinct code path: `eval_const_expr` with `assoc_known` threaded correctly, so that two-segment paths inside const item initializers can look up built-in constants. If this parameter is ever dropped or mis-threaded, `const LIMIT = i32::MAX` silently resolves to `None`, the fixed-point loop fails to populate `LIMIT`, and any program that uses `LIMIT` produces the wrong value (likely 0) with no error message. This is the "fails silently" failure mode the previous cycle's Next section called out.

**ARM64 implementation**: `eval_const_expr` accepts an `assoc_known: &HashMap<String, i32>` parameter that is built from `builtin_assoc_consts` before the const fixed-point loop runs. Two-segment path arms (`Type::CONST`) are looked up in this map. The fixed-point loop re-evaluates consts that returned `None` on earlier passes, so ordering between `const A = i32::MAX` and `const B = A - 1` does not matter.

**FLS §7.1:10**: Const items must be fully evaluated before first use.
**FLS §10.3**: All constant expression forms — including associated constant paths — are valid in const item initializers.
**FLS §4.1 AMBIGUOUS**: The spec does not name `MAX`/`MIN` explicitly; they are a language convention derived from value ranges.

**Violated if**:
- `const LIMIT: i32 = i32::MAX; fn main() -> i32 { if LIMIT > 0 { 1 } else { 0 } }` returns 0, OR
- `const LIMIT = i32::MAX; const DERIVED = LIMIT - 1; fn check(x: i32) -> i32 { if x == 2147483646 { 1 } else { 0 } } fn main() -> i32 { check(DERIVED) }` returns 0, OR
- The assembly for `const LIMIT = i32::MAX; fn main() { LIMIT }` contains `ldr x0, [sp` (LIMIT was not inlined as an immediate)

**Tests**: `cargo test --test e2e -- claim_74_const_chain_through_builtin_assoc_not_zero runtime_const_from_i32_max_emits_loadimm milestone_188_const_from_i32_max_positive milestone_188_const_from_i32_min_negative milestone_188_const_arithmetic_with_i32_max`


---

## Claim 75: Narrow integer const items wrap at their declared bit-width

**Promise**: `const Z: u8 = 200 + 100` stores `44` (300 wrapped to 8 bits), not `300`. The const evaluator applies the declared type's narrowing (u8 → mask to 8 bits, i8 → sign-extend from 8 bits, u16 → mask to 16 bits, i16 → sign-extend from 16 bits) before storing the result in `const_vals`. Downstream uses of the const emit the correctly-wrapped immediate.

**Why this matters**: `eval_const_expr` works entirely in `i32`. Without `narrow_const_value`, a `const X: u8 = 200 + 100` would store 300 in `const_vals`. The assembly for `fn main() -> i32 { X as i32 }` would emit `LoadImm(300)`, which is wrong — the u8 semantic requires wrapping. Since OS exit codes are truncated to 8 bits, exit(300) == exit(44) at the kernel level, making the bug invisible to naive runtime tests. Only assembly inspection and comparison-based runtime tests can distinguish the two.

**ARM64 implementation**: `narrow_const_value(raw: i32, ty: &Ty, source: &str) -> i32` inspects the declared type of the const item. For `u8` it casts through `u8` (mask to 8 bits). For `i8` it casts through `i8` (sign-extend from 8 bits). For `u16`/`i16` similarly. `i32` and other types pass through unchanged.

**FLS §6.23**: Arithmetic overflow in const contexts is a compile-time error per the spec; galvanic wraps instead as a pragmatic choice.
**FLS §4.1**: Narrow integer types have specific bit-widths; values must be representable.
**FLS §6.23 AMBIGUOUS**: The FLS does not explicitly enumerate which narrowing rules apply to non-i32 const item initializers. Galvanic wraps silently (does not error) which diverges from strict Rust behavior.

**Violated if**:
- `const Z: u8 = 200 + 100; fn check(v: i32) -> i32 { if v == 44 { 1 } else { 0 } } fn main() -> i32 { check(Z as i32) }` returns 0 (Z resolved to 300, not 44), OR
- The assembly for `const X: u8 = 300; fn main() -> i32 { X as i32 }` contains `#300` or `#0x12c` (unwrapped immediate) instead of `#44` or `#0x2c`

**Tests**: `cargo test --test e2e -- claim_75_u8_const_item_wraps_at_8_bits_not_i32 runtime_u8_const_wraps_emits_correct_loadimm milestone_189_u8_const_wraps_at_8_bits milestone_189_u16_const_addition_wraps`

---

## Claim 76: Chained narrow integer const item references preserve bit-width

**Stakeholder**: William (researcher), CI / Validation Infrastructure

**Promise**: When a narrow-typed `const` item references another narrow-typed `const` item, the narrowing is applied per-item at storage time. `const X: u8 = 200; const Y: u8 = X + 100` must yield Y = 44 (not 300). The evaluator fetches X's already-narrowed value (200), computes 200+100=300 in i32, then narrows the result to the declared u8 width (300 % 256 = 44) before inserting Y into `const_vals`.

**Why this is load-bearing**: Without the call to `narrow_const_value` at storage time, Y would store 300. Since `exit(300) == exit(44)` at the OS level (exit codes are 8-bit), this bug is invisible to compile-and-run tests. Only assembly inspection or comparison-based runtime tests (`check(Y) { if Y == 44 { 1 } else { 0 } }`) can catch it.

**ARM64 implementation**: The const fixed-point loop in `lower()` calls `narrow_const_value(raw, &c.ty, source)` before every `const_vals.insert(name, val)`. This applies the declared type's bit-width constraint to the evaluated result regardless of how the result was computed (literal, arithmetic, or reference to another const).

**FLS §7.1:10**: Every use of a constant is replaced with its value.
**FLS §4.1**: Narrow integer types have specific bit-widths; stored values must be representable.
**FLS §6.23 AMBIGUOUS**: Overflow in const contexts should be a compile-time error; galvanic wraps instead as a pragmatic choice. The FLS does not specify per-item narrowing rules for non-i32 types.

**Violated if**:
- `const X: u8 = 200; const Y: u8 = X + 100; fn check(v: i32) -> i32 { if v == 44 { 1 } else { 0 } } fn main() -> i32 { check(Y as i32) }` returns 0 (Y stored as 300), OR
- The assembly for that program contains `#300` or `#0x12c` instead of `#44` or `#0x2c`

**Tests**: `cargo test --test e2e -- claim_76_u8_chained_const_ref_wraps_at_8_bits runtime_u8_const_chain_ref_emits_correct_loadimm milestone_190_u8_const_ref_chain_wraps milestone_190_u16_const_ref_chain_wraps`
