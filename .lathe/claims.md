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
