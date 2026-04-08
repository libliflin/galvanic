#!/usr/bin/env bash
# falsify.sh — Adversarial verification of galvanic's load-bearing claims.
#
# Runs every cycle as part of snapshot collection. Exit 0 = all claims hold.
# Exit non-zero = at least one claim is violated; the agent must fix it before
# any other work.
#
# Must be fast (seconds, not minutes). No network. No external services.
# See .lathe/claims.md for the claims this suite defends.

set -euo pipefail

# ── Setup ─────────────────────────────────────────────────────────────────────

export PATH="/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$PATH"

PASS=0
FAIL=0
ERRORS=""

fail() {
    local claim="$1"
    local msg="$2"
    echo "FAIL [$claim]: $msg"
    FAIL=$((FAIL + 1))
    ERRORS="$ERRORS\n  - [$claim] $msg"
}

pass() {
    local claim="$1"
    echo "OK   [$claim]"
    PASS=$((PASS + 1))
}

# ── Claim 1: Runtime codegen, not compile-time interpretation ─────────────────
# Tests that `1 + 2` emits an `add` instruction, not `mov x0, #3`.
# This is the fundamental "compiler vs interpreter" check.
# References: claims.md Claim 1, refs/fls-constraints.md Constraint 1.

echo "--- Claim 1: runtime_add_emits_add_instruction ---"
if cargo test --test e2e --quiet -- runtime_add_emits_add_instruction 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 1" "runtime_add_emits_add_instruction FAILED — galvanic may be folding constants instead of emitting runtime instructions"
else
    pass "Claim 1: runtime codegen (not interpreter)"
fi

# ── Claim 2: Token is exactly 8 bytes ─────────────────────────────────────────
# Enforces the cache-line layout rationale documented throughout lexer.rs.
# References: claims.md Claim 2.

echo "--- Claim 2: token_is_eight_bytes ---"
if cargo test --lib --quiet -- lexer::tests::token_is_eight_bytes 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 2" "token_is_eight_bytes FAILED — Token grew beyond 8 bytes, invalidating all cache-line layout comments"
else
    pass "Claim 2: Token is 8 bytes"
fi

# ── Claim 3: No unsafe code in library source ─────────────────────────────────
# main.rs may shell out to the assembler/linker (allowed).
# All other src/ files must be safe Rust.
# References: claims.md Claim 3.

echo "--- Claim 3: no unsafe in library source ---"

# Self-test: verify the comment filter does not accidentally suppress real unsafe code.
# If the filter is too aggressive, Claim 3 becomes blind — that's worse than no check.
_TMP_UNSAFE=$(mktemp /tmp/falsify_unsafe_XXXXXX.rs)
trap 'rm -f "$_TMP_UNSAFE"' EXIT
printf 'fn foo() { unsafe { let _ = 0; } }\n' > "$_TMP_UNSAFE"
_SELFTEST=$(grep -n 'unsafe\s*{' "$_TMP_UNSAFE" | grep -Ev ':[0-9]+:[[:space:]]*//' || true)
if [[ -z "$_SELFTEST" ]]; then
    fail "Claim 3 self-test" "the comment-exclusion filter in Claim 3 is suppressing real unsafe blocks — the check is blind; fix the grep pattern"
fi
rm -f "$_TMP_UNSAFE"

UNSAFE_BLOCKS=$(grep -rn 'unsafe\s*{' src/ 2>/dev/null | grep -v '^src/main\.rs:' | grep -Ev ':[0-9]+:[[:space:]]*//' || true)
UNSAFE_FNS=$(grep -rn 'unsafe\s*fn\b' src/ 2>/dev/null | grep -v '^src/main\.rs:' | grep -Ev ':[0-9]+:[[:space:]]*//' || true)
UNSAFE_IMPLS=$(grep -rn 'unsafe\s*impl\b' src/ 2>/dev/null | grep -v '^src/main\.rs:' | grep -Ev ':[0-9]+:[[:space:]]*//' || true)

if [[ -n "$UNSAFE_BLOCKS" || -n "$UNSAFE_FNS" || -n "$UNSAFE_IMPLS" ]]; then
    fail "Claim 3" "unsafe code found in library source (outside main.rs):\n$(echo "$UNSAFE_BLOCKS$UNSAFE_FNS$UNSAFE_IMPLS" | head -5)"
else
    pass "Claim 3: no unsafe in library"
fi

# ── Claim 4: Pipeline does not panic on valid minimal programs ────────────────
# galvanic must exit cleanly (exit code <= 128) on valid Rust.
# Exit > 128 means death by signal (panic, segfault, etc.).
# References: claims.md Claim 4.

echo "--- Claim 4: pipeline accepts minimal programs without panicking ---"

# Build the binary first (silently).
if ! cargo build --quiet 2>/dev/null; then
    fail "Claim 4" "cargo build failed — cannot test pipeline behavior"
else
    BINARY="./target/debug/galvanic"

    # Test: empty file exits 0
    TMP_EMPTY=$(mktemp /tmp/falsify_empty_XXXXXX.rs)
    trap 'rm -f "$TMP_EMPTY"' EXIT
    echo "" > "$TMP_EMPTY"
    set +e
    "$BINARY" "$TMP_EMPTY" > /dev/null 2>&1
    EMPTY_EXIT=$?
    set -e
    if [ "$EMPTY_EXIT" -gt 128 ]; then
        fail "Claim 4" "galvanic died with signal on empty file (exit $EMPTY_EXIT — signal $((EMPTY_EXIT - 128)))"
    fi

    # Test: minimal valid program exits 0
    TMP_MAIN=$(mktemp /tmp/falsify_main_XXXXXX.rs)
    trap 'rm -f "$TMP_EMPTY" "$TMP_MAIN"' EXIT
    printf 'fn main() {}\n' > "$TMP_MAIN"
    set +e
    "$BINARY" "$TMP_MAIN" > /dev/null 2>&1
    MAIN_EXIT=$?
    set -e
    if [ "$MAIN_EXIT" -gt 128 ]; then
        fail "Claim 4" "galvanic died with signal on 'fn main() {}' (exit $MAIN_EXIT — signal $((MAIN_EXIT - 128)))"
    elif [ "$MAIN_EXIT" -ne 0 ]; then
        fail "Claim 4" "galvanic exited non-zero ($MAIN_EXIT) on 'fn main() {}' — expected 0"
    fi

    # Test: adversarial input — deeply repeated arithmetic (150 binops)
    # Exercises the parser's expression stack depth without hanging.
    TMP_DEEP=$(mktemp /tmp/falsify_deep_XXXXXX.rs)
    trap 'rm -f "$TMP_EMPTY" "$TMP_MAIN" "$TMP_DEEP"' EXIT
    python3 -c "
print('fn main() -> i32 {')
print('    ' + ' + '.join(str(i % 10) for i in range(150)))
print('}')
" > "$TMP_DEEP"
    set +e
    timeout 10 "$BINARY" "$TMP_DEEP" > /dev/null 2>&1
    DEEP_EXIT=$?
    set -e
    if [ "$DEEP_EXIT" -gt 128 ]; then
        fail "Claim 4" "galvanic died with signal on deep arithmetic expression (exit $DEEP_EXIT)"
    fi

    # If all sub-checks passed
    if [ "$EMPTY_EXIT" -le 128 ] && [ "$MAIN_EXIT" -eq 0 ] && [ "$DEEP_EXIT" -le 128 ]; then
        pass "Claim 4: pipeline handles valid programs without panicking"
    fi
fi

# ── Claim 6: Function calls with literal args emit branch instructions ────────
# Tests that `square(6)` emits `bl square`, NOT `mov x0, #36`.
# Guards against function inlining + constant propagation regressions.
# Claim 1 only tests inline arithmetic — this catches the complementary attack:
# a fold at the call site, not inside the arithmetic expression itself.
# References: claims.md Claim 6.

echo "--- Claim 6: function calls with literal args emit bl, not folded constant ---"
if cargo test --test e2e --quiet -- runtime_fn_call_result_not_folded 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 6" "runtime_fn_call_result_not_folded FAILED — galvanic may be inlining and folding function calls with literal arguments"
else
    pass "Claim 6: function calls emit runtime bl (not folded constant)"
fi

# ── Claim 7: Generic function/method calls emit runtime bl, not folded constants ─
# Tests that a generic function called with a literal argument:
#   (a) calls the monomorphized specialization (bl identity__i32)
#   (b) calls the outer wrapper at runtime (bl use_identity) — not folded away
# Extends Claim 6 to the generic monomorphization code path (a separate lowering pass).
# Red-team finding: the original negative assertions were vacuous — fixed 2026-04-07.
# References: claims.md Claim 7.

echo "--- Claim 7: generic function/method calls emit runtime bl (not folded) ---"
if cargo test --test e2e --quiet -- runtime_generic_fn_not_folded runtime_generic_method_not_folded 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 7" "runtime_generic_fn_not_folded or runtime_generic_method_not_folded FAILED — galvanic may be inlining and folding generic calls with literal arguments"
else
    pass "Claim 7: generic function/method calls emit runtime bl (not folded)"
fi

# ── Claim 8: Named block `break` emits unconditional branch ─────────────────
# Tests that `break 'label value` emits a real unconditional branch instruction
# (`b .Lxxx`), not just the character 'b' somewhere in the output.
# Red-team finding (2026-04-07): the original assertion was `asm.contains('b')`
# which checked for the character 'b' — vacuously true in any ARM64 program.
# Any instruction (bl, blr, cbz, sub) or label name contains 'b'. The assertion
# was indistinguishable from a no-op. Fixed to check `b       .L` pattern.
# References: claims.md Claim 8.

echo "--- Claim 8: named block break emits unconditional branch (not vacuous 'b') ---"
if cargo test --test e2e --quiet -- runtime_named_block_emits_branch_not_folded 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 8" "runtime_named_block_emits_branch_not_folded FAILED — named block break may not emit an unconditional branch to the exit label"
else
    pass "Claim 8: named block break emits unconditional branch to exit label"
fi

# ── Claim 9: Generic trait impl calls emit monomorphized runtime branch ───────
# Tests that a generic trait impl produces:
#   (a) a monomorphized label (Wrapper__get__i32) in the assembly
#   (b) a runtime bl to the outer wrapper (bl use_wrapper) — not constant-folded
# Extends Claims 6–7 to the trait impl monomorphization path (milestone 138).
# References: claims.md Claim 9.

echo "--- Claim 9: generic trait impl emits monomorphized call (not folded) ---"
if cargo test --test e2e --quiet -- runtime_generic_trait_impl_emits_mangled_call 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 9" "runtime_generic_trait_impl_emits_mangled_call FAILED — generic trait impl may not be monomorphizing or may be constant-folding the result"
else
    pass "Claim 9: generic trait impl emits monomorphized runtime call (not folded)"
fi

# ── Claim 10: Default trait method dispatch emits monomorphized label ─────────
# Tests that a default method body emits `Foo__doubled:` label (monomorphized)
# and that calling it with a runtime arg emits `bl Foo__doubled` (not folded).
# This guards the default method resolution path separately from Claims 6–9,
# which cover regular functions, generic functions, and generic trait impls.
# References: claims.md Claim 10.

echo "--- Claim 10: default trait method emits monomorphized label and runtime call ---"
if cargo test --test e2e --quiet -- runtime_default_method_emits_mangled_label runtime_default_method_result_not_folded 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 10" "runtime_default_method_emits_mangled_label or runtime_default_method_result_not_folded FAILED — default trait method may not be emitting a type-specific monomorphized label or may be constant-folding the result"
else
    pass "Claim 10: default trait method emits monomorphized label and runtime call"
fi

# ── Claim 11: Closures compile to hidden function labels ─────────────────────
# Tests that:
#   (a) a non-capturing closure emits a `__closure_*` label AND a `mul` instruction
#       in the closure body (not constant-folded from `x * 2`)
#   (b) a capturing closure emits `__closure_main_0` AND uses `blr` (indirect call)
# These guard both code paths separately: FLS §6.14 (non-capturing) and §6.22 (capturing).
# A regression where closures are inlined at call sites would lose the hidden label.
# A regression where the body is constant-folded would lose the `mul` instruction.
# References: claims.md Claim 11.

echo "--- Claim 11: closures emit hidden function labels with runtime body instructions ---"
if cargo test --test e2e --quiet -- runtime_closure_emits_hidden_function_label runtime_capturing_closure_emits_capture_load_before_explicit_arg 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 11" "runtime_closure_emits_hidden_function_label or runtime_capturing_closure_emits_capture_load_before_explicit_arg FAILED — closures may not be emitting hidden function labels or may be constant-folding closure bodies"
else
    pass "Claim 11: closures emit hidden function labels with runtime body instructions"
fi

# ── Claim 12: Trait-bound generic function dispatch emits monomorphized runtime code ─
# Tests that a generic function with `<T: Trait>` bound:
#   (a) emits a monomorphized label (apply_scale__Foo) in the assembly
#   (b) dispatches to the concrete type's method via runtime `bl Foo__scale`
#   (c) does NOT constant-fold the result when called with a runtime parameter
# This is distinct from Claim 7 (data-only generics) and Claim 9 (generic trait impls).
# The code path: call-site type inference → `pending_monos` → `generic_type_subst` →
# param spilling with concrete struct name → `local_struct_types` → method dispatch.
# Introduced in cycle 17 (milestone 139, FLS §12.1 + §4.14). No prior falsification.
# References: claims.md Claim 12.

echo "--- Claim 12: trait-bound generic dispatch emits monomorphized label and runtime bl ---"
if cargo test --test e2e --quiet -- runtime_trait_bound_result_not_folded 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 12" "runtime_trait_bound_result_not_folded FAILED — trait-bound generic dispatch may not be monomorphizing or may be constant-folding the result"
else
    pass "Claim 12: trait-bound generic dispatch emits monomorphized label and runtime bl (not folded)"
fi

# ── Claim 13: Associated constant in runtime computation emits add, not folded ─
# Tests that `fn compute(x: i32) -> i32 { x + Config::MAX }` called as `compute(5)`:
#   (a) emits a runtime `add` instruction (not constant-folded away)
#   (b) does NOT emit `mov x0, #15` (the sum was not folded at the call site)
# This guards the specific interaction: assoc const inlining (correct per FLS §7.1:10)
# must NOT cascade into constant-folding when combined with a runtime parameter.
# Distinct from Claim 1 (inline arithmetic) and Claim 6 (function call folding).
# Introduced in cycle 22 (red-team, milestone 128 path, FLS §10.3).
# References: claims.md Claim 13.

echo "--- Claim 13: assoc const in runtime computation emits add (not folded) ---"
if cargo test --test e2e --quiet -- runtime_assoc_const_in_computation_not_folded 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 13" "runtime_assoc_const_in_computation_not_folded FAILED — assoc const may be triggering constant-folding of runtime computation when combined with a literal argument"
else
    pass "Claim 13: assoc const in runtime computation emits add (not folded)"
fi

# ── Claim 14: Field method calls emit runtime bl, not folded constants ────────
# Tests two things:
#   (a) `c.inner.get()` where `inner: Counter` emits `Counter__get:` label AND `bl Counter__get`
#       (method body and runtime dispatch both present — not elided by constant folding)
#   (b) `c.inner.get() * factor` where `factor` is a runtime parameter emits `mul` and
#       does NOT fold the product (3 * 4 = 12) to a constant `mov x0, #12`
# This guards the `ExprKind::FieldAccess` receiver arm introduced in milestone 142 (cycle 23).
# No prior claim covers this path — a regression in `resolve_place` for field access would
# have been invisible to Claims 6–13.
# Red-team finding (2026-04-07): the original negative assertion in the sibling test used
# `!asm.contains("mov x0, #7") || asm.contains("ldr")` — vacuously true (same bug as
# Claims 7 and 8 before they were fixed). Replaced with real adversarial checks.
# References: claims.md Claim 14.

echo "--- Claim 14: field method calls emit runtime bl (not folded) ---"
if cargo test --test e2e --quiet -- runtime_field_method_call_emits_bl_not_folded runtime_field_method_call_result_not_folded 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 14" "runtime_field_method_call_emits_bl_not_folded or runtime_field_method_call_result_not_folded FAILED — field method call may not be emitting the method body label, runtime dispatch, or may be constant-folding the result"
else
    pass "Claim 14: field method calls dispatch at runtime via bl (not folded)"
fi

# ── Claim 15: Multiple bounds monomorphize ALL methods for ALL concrete types ─
# Tests that when two concrete types are both used with a multi-bound generic,
# all bound methods are emitted as labels for EACH type (not just the first).
# Attack: pending_monos may register methods for the first concrete type but
# silently drop method labels for the second type.
# Distinct from Claim 12 (single-type, single-bound) and from the existing
# runtime_multiple_bounds_emits_both_trait_calls (single-type, multi-bound).
# References: claims.md Claim 15.

echo "--- Claim 15: multiple bounds monomorphize all methods for all concrete types ---"
if cargo test --test e2e --quiet -- runtime_multiple_bounds_two_types_both_monomorphized 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 15" "runtime_multiple_bounds_two_types_both_monomorphized FAILED — multi-bound generic may not emit all method labels for all concrete type monomorphizations"
else
    pass "Claim 15: multiple bounds monomorphize all methods for all concrete types"
fi

echo "--- Claim 16: dyn Trait dispatch uses vtable indirection, not constant folding ---"
if cargo test --test e2e --quiet -- milestone_147_dyn_trait_asm_inspection 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 16" "milestone_147_dyn_trait_asm_inspection FAILED — dyn Trait dispatch may be constant-folded or vtable not emitted"
else
    pass "Claim 16: dyn Trait dispatch uses vtable indirection, not constant folding"
fi

# ── Claim 17: Two concrete types behind dyn Trait both emit vtable labels ─────
# Tests that when two different concrete types are passed to the same dyn Trait
# parameter at different call sites, BOTH vtables are emitted in the assembly.
# Claim 16 only tests a single concrete type. This guards the pending_vtables
# accumulation loop against a regression where the second (trait, concrete_type)
# pair is silently dropped — leaving that type's dispatch broken while Claim 16
# and the two_concrete_types compile-and-run test still pass.
# References: claims.md Claim 17.

echo "--- Claim 17: two concrete types behind dyn Trait both emit vtable labels ---"
if cargo test --test e2e --quiet -- runtime_dyn_trait_two_concrete_types_both_vtables_emitted 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 17" "runtime_dyn_trait_two_concrete_types_both_vtables_emitted FAILED — second concrete type vtable may not be emitted, or a method result was constant-folded"
else
    pass "Claim 17: both concrete type vtables emitted, no constant folding"
fi

# ── Claim 18: Second dyn Trait method accesses vtable at offset 8 ─────────────
# Tests that calling the second method (index 1) of a two-method dyn Trait emits
# `ldr x10, [x9, #8]` — NOT `ldr x10, [x9, #0]`. A bug where method_idx is always
# 0 would make every vtable call dispatch to the first method, producing wrong behavior
# silently when method 1 is called.
# This complements Claim 16 (single method, vtable present) and Claim 17 (two concrete
# types) by testing the vtable OFFSET calculation for methods beyond index 0.
# Introduced in cycle 33 (red-team, milestone 147 follow-up, FLS §4.13).
# References: claims.md Claim 18.

echo "--- Claim 18: second dyn Trait method emits vtable offset #8 (not #0) ---"
if cargo test --test e2e --quiet -- runtime_dyn_trait_second_method_emits_vtable_offset_8 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 18" "runtime_dyn_trait_second_method_emits_vtable_offset_8 FAILED — second method may be loading from vtable offset #0 instead of #8, meaning method_idx computation is broken"
else
    pass "Claim 18: second dyn Trait method loads fn-ptr at vtable offset #8"
fi

# ── Claim 19: Exit 0 on valid program implies .s output was written ───────────
# Tests the structural contract: galvanic exit 0 means compilation succeeded
# and produced output — not a silent skip due to a lower/codegen failure.
#
# Background: in cycle 36, a lower error caused galvanic to `return` (exit 0)
# without producing any output. compile_and_run then ran qemu on a nonexistent
# binary and got exit 1, producing a confusing test failure with "expected 10,
# got 1". The fix: change the lower error handler from `return` to
# `process::exit(1)`. This test verifies the positive case holds after that fix.
#
# We test the positive case: a valid program exits 0 AND writes the .s file.
# (The negative case — lower error → non-zero exit — is enforced structurally
# by the main.rs change; no portable way to trigger a lower error in falsify.sh.)
# References: claims.md Claim 19.

echo "--- Claim 19: exit 0 on valid program implies .s output written ---"

_C19_BINARY="./target/debug/galvanic"
if ! cargo build --quiet 2>/dev/null; then
    fail "Claim 19" "cargo build failed — cannot verify output contract"
else
    _C19_SRC=$(mktemp /tmp/falsify_c19_XXXXXX.rs)
    _C19_S="${_C19_SRC%.rs}.s"
    printf 'fn main() -> i32 { 42 }\n' > "$_C19_SRC"

    set +e
    "$_C19_BINARY" "$_C19_SRC" > /dev/null 2>&1
    _C19_EXIT=$?
    set -e

    rm -f "$_C19_SRC"

    if [ "$_C19_EXIT" -ne 0 ]; then
        rm -f "$_C19_S"
        fail "Claim 19" "galvanic exited $_C19_EXIT on 'fn main() -> i32 { 42 }' — expected 0"
    elif [ ! -f "$_C19_S" ]; then
        fail "Claim 19" "galvanic exited 0 but did not write the .s output file — exit 0 without output means lower or codegen silently failed"
    else
        rm -f "$_C19_S"
        pass "Claim 19: exit 0 on valid program implies .s output was written"
    fi
fi

# ── Claim 20: @ binding patterns emit runtime sub-pattern checks ──────────────
# Tests that `n @ 1..=5 => n * 2` with a function-parameter scrutinee:
#   (a) emits `cmp` instructions for the range check (not unconditionally binding)
#   (b) does NOT fold `n * 2` to `mov x0, #6` (binding value is runtime, not compile-time)
# And that `n @ 42 => n + 1` emits `cmp` for the literal equality check.
# Both use a function parameter as the scrutinee — constant folding the result
# through the match arm is impossible given only compile-time information.
# Introduced in cycle 40 (red-team, milestone 150, FLS §5.1.4).
# References: claims.md Claim 20.

echo "--- Claim 20: @ binding patterns emit runtime sub-pattern checks (not folded) ---"
if cargo test --test e2e --quiet -- runtime_bound_pattern_range_emits_cmp_and_binding runtime_bound_pattern_literal_emits_eq_check 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 20" "runtime_bound_pattern_range_emits_cmp_and_binding or runtime_bound_pattern_literal_emits_eq_check FAILED — @ binding pattern may not be emitting sub-pattern checks or may be constant-folding the bound expression"
else
    pass "Claim 20: @ binding patterns emit runtime sub-pattern checks (not folded)"
fi

# ── Claim 21: let-else emits runtime discriminant check, binding not folded ───
# Tests three things for `let Enum::Variant(v) = o else { return 0 }`:
#   (a) A runtime discriminant check (cbz) is emitted — the else branch is reachable.
#   (b) The extracted binding is loaded at runtime (ldr present, mov x0, #5 absent).
#   (c) When the binding is used with a second runtime parameter (v + n), the result
#       is NOT constant-folded: assembly must contain `add`, not `mov x0, #7`.
#
# Attack vector: galvanic could constant-fold through the enum field load when the
# call site passes a literal (`Opt::Some(3)`). The adversarial test uses TWO function
# parameters — one for the enum and one for the addend — making folding to a constant
# impossible for any correct compiler.
#
# Introduced in cycle 44 (red-team, milestone 153 follow-up, FLS §8.1).
# References: claims.md Claim 21.

echo "--- Claim 21: let-else emits runtime discriminant check and binding not folded ---"
if cargo test --test e2e --quiet -- runtime_let_else_emits_discriminant_check runtime_let_else_binding_not_folded runtime_let_else_binding_combined_with_param_not_folded 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 21" "let-else falsification FAILED — discriminant check may be missing, binding may be constant-folded, or combined computation was folded to a constant"
else
    pass "Claim 21: let-else emits runtime discriminant check and binding is not folded"
fi

# ── Claim 22: while-let OR patterns emit runtime orr+cbz for loop condition ───
# Tests two things:
#   (a) `while let 1 | 2 | 3 = x { ... }` with x from a function parameter emits:
#       - `orr` for OR accumulation across alternatives (not just first alt checked)
#       - `cbz` for the conditional loop exit on accumulated flag = 0
#       - `b .L` for the back-edge (loop is runtime, not compile-time unrolled)
#   (b) The counter variable `n` is not constant-folded — ldr is present.
#
# Attack this guards against: dropping OR accumulation and falling back to a single
# equality check against only the first alternative. The compile-and-run tests for
# milestone 154 catch this on CI (wrong iteration count) but require QEMU. This
# assembly inspection test catches the same regression locally.
#
# Distinct from Claim 20 (@ binding patterns, match-arm path) and Claim 21 (let-else).
# This covers the loop condition re-evaluation path in while-let (FLS §5.1.11 + §6.15.4).
# Introduced in cycle 52 (red-team, milestone 154 follow-up).
# References: claims.md Claim 22.

echo "--- Claim 22: while-let OR pattern emits orr accumulation and cbz (not folded) ---"
if cargo test --test e2e --quiet -- runtime_while_let_or_emits_orr_accumulation runtime_while_let_or_result_not_folded 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 22" "runtime_while_let_or_emits_orr_accumulation or runtime_while_let_or_result_not_folded FAILED — while-let OR pattern may not be emitting orr accumulation, cbz exit, or back-edge; or result was constant-folded"
else
    pass "Claim 22: while-let OR pattern emits runtime orr+cbz+back-edge (not folded)"
fi

# ── Claim 23: while-let OR enum variants emit runtime orr accumulation ────────
# Tests that `while let Status::Active | Status::Pending = s { ... }` emits:
#   (a) `orr` to accumulate discriminant equality across both variants (not just first)
#   (b) `cbz` for the conditional loop exit
#   (c) result NOT constant-folded to `mov x0, #1`
# Extends Claim 22 (scalar literals) to enum-variant patterns — a different code path.
# A regression that drops OR accumulation for enum-variant while-let would be invisible
# to Claim 22. Introduced in cycle 56 (red-team, milestone 154 follow-up, FLS §5.1.11).
# References: claims.md Claim 23.

echo "--- Claim 23: while-let OR enum variants emit runtime orr+cbz ---"
if cargo test --test e2e --quiet -- runtime_while_let_or_enum_emits_orr_accumulation 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 23" "runtime_while_let_or_enum_emits_orr_accumulation FAILED — while-let OR pattern with enum variants may not emit orr accumulation for all variants, cbz exit, or result was constant-folded"
else
    pass "Claim 23: while-let OR enum variant pattern emits runtime orr+cbz (not folded)"
fi

# ── Claim 24: match guard with function parameter emits runtime comparison ─────
# Tests that `fn guarded(n: i32) -> i32 { match n { x if x > 5 => x + 10, _ => 0 } }`
# compiled with `guarded(7)` in main:
#   (a) emits `cmp` or `cset` — guard comparison is runtime (not folded away)
#   (b) emits `cbz` or `cbnz` — conditional branch for guard evaluation
#   (c) does NOT emit `mov x0, #17` — result is NOT constant-folded from guarded(7)
#
# This is the FLS §6.1.2 litmus test for match guards: the existing test
# `runtime_match_guard_emits_cbz_for_guard_condition` uses `let x = 7` (literal);
# this claim uses a function parameter, which cannot be folded by any correct compiler.
# A folding interpreter would evaluate `7 > 5 → 7 + 10 = 17` at compile time.
# Introduced in cycle 56 (red-team, FLS §6.18 + §6.1.2:37–45).
# References: claims.md Claim 24.

echo "--- Claim 24: match guard with param emits runtime comparison (not folded) ---"
if cargo test --test e2e --quiet -- runtime_match_guard_with_param_emits_runtime_comparison 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 24" "runtime_match_guard_with_param_emits_runtime_comparison FAILED — match guard may not be emitting runtime comparison/branch, or guard result was constant-folded to 17"
else
    pass "Claim 24: match guard with function parameter emits runtime comparison (not folded)"
fi

# ── Claim 25: let-else mixed OR (literal + range) emits runtime check for both alts ─
# Tests that `let 1 | 10..=20 = n else { return 0 }` in a function with parameter n:
#   (a) emits `orr` — both the literal and range alternatives are checked (not just first)
#   (b) emits `cbz` — else block branch fires when no alternative matches
#   (c) does NOT emit `mov x0, #1; ret` — result not constant-folded
#
# This guards against two failure modes:
#   1. Parser regression: `parse_let_pattern` did not handle `..=` after a LitInteger,
#      causing "expected Semi, found DotDotEq" on valid Rust code.
#   2. Lowering regression: `accum_or_alt` not called for range alternatives in let-else,
#      causing `classify(15)` to incorrectly take the else branch (only `1` checked).
#
# Introduced in cycle 60 (red-team, discovered parser bug, FLS §5.1.9 + §5.1.11 + §8.1).
# References: claims.md Claim 25.

echo "--- Claim 25: let-else mixed OR (literal + range) emits runtime orr+cbz ---"
if cargo test --test e2e --quiet -- runtime_let_else_or_mixed_emits_orr_accumulation 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 25" "runtime_let_else_or_mixed_emits_orr_accumulation FAILED — let-else with mixed OR (literal|range) may not emit orr accumulation for all alternatives, or result was constant-folded"
else
    pass "Claim 25: let-else mixed-kind OR emits runtime orr+cbz for both alternatives (not folded)"
fi

# ── Claim 26: @ binding pattern in let-else emits runtime check and binding ───
#
# Promise: `let n @ 1..=5 = x else { return 0 }; n * 2` must emit cmp (range
# check), cbz (else-branch), and mul/add (binding use) — not `mov x0, #6`.
# Without the Pat::Bound arm in let-else lowering, the program fails at
# compile time with "Unsupported". With constant folding, the result is #6.
#
# Introduced in cycle 62 (FLS §5.1.4 + §8.1 — let-else @ binding lowering).
# References: claims.md Claim 26.

echo "--- Claim 26: @ binding pattern in let-else emits runtime check and binding (not folded) ---"
if cargo test --test e2e --quiet -- runtime_let_else_bound_pattern_emits_cmp_and_binding_not_folded 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 26" "runtime_let_else_bound_pattern_emits_cmp_and_binding_not_folded FAILED — let-else @ binding may not emit runtime range check, else-branch, or binding use; or result was constant-folded"
else
    pass "Claim 26: let-else @ binding emits runtime cmp+cbz+binding (not folded)"
fi

# ── Claim 27: @ binding with OR sub-pattern emits orr accumulation ───────────
#
# Promise: `let n @ (1 | 5..=10) = x else { return 0 }; n * 2` must emit orr
# (OR accumulation for the alternatives) and must NOT constant-fold the result.
# Without the Pat::Or arm in accum_or_alt, the program fails at compile time
# with "unsupported pattern kind inside OR pattern alternative".
#
# Introduced in cycle 63 (FLS §5.1.4 + §5.1.11 — @ binding with OR sub-pat).
# References: claims.md Claim 27.

echo "--- Claim 27: @ binding pattern with OR sub-pattern emits orr accumulation (not folded) ---"
if cargo test --test e2e --quiet -- runtime_at_bound_or_subpat_emits_orr_accumulation runtime_at_bound_or_subpat_result_not_folded 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 27" "runtime_at_bound_or_subpat_emits_orr_accumulation or runtime_at_bound_or_subpat_result_not_folded FAILED — @ binding OR sub-pattern may not emit orr accumulation, or result was constant-folded"
else
    pass "Claim 27: @ binding OR sub-pattern emits runtime orr accumulation (not folded)"
fi

# ── Claim 28: @ binding with OR sub-pattern in if-let and match positions ────
#
# Promise: `n @ (pat1 | pat2)` in if-let and match positions must emit `orr` for
# OR accumulation and must NOT constant-fold the bound value.
# These are distinct lowering paths from the let-else path tested by Claim 27.
# The milestone 158 compile-and-run tests for if-let and match require QEMU;
# without this claim, a regression in those paths is invisible locally.
#
# Tests two positions:
#   (a) if-let: `if let n @ (1 | 5..=10) = x { n * 2 } else { 0 }` with x param
#       — must emit orr, must NOT emit `mov x0, #12` (n*2 where n=6).
#   (b) match: `match x { n @ (1 | 5..=10) => n * 2, _ => 0 }` with x param
#       — same assertions.
#
# Introduced in cycle 64 (red-team, FLS §5.1.4 + §5.1.11 + §6.17 + §6.18).
# References: claims.md Claim 28.

echo "--- Claim 28: @ binding with OR sub-pattern in if-let/match emits runtime orr (not folded) ---"
if cargo test --test e2e --quiet -- runtime_at_bound_or_subpat_if_let_emits_orr_not_folded runtime_at_bound_or_subpat_match_emits_orr_not_folded 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 28" "runtime_at_bound_or_subpat_if_let_emits_orr_not_folded or runtime_at_bound_or_subpat_match_emits_orr_not_folded FAILED — @ binding OR sub-pattern in if-let or match may not emit orr accumulation, or result was constant-folded"
else
    pass "Claim 28: @ binding OR sub-pattern in if-let and match emit runtime orr accumulation (not folded)"
fi

# ── Claim 29: struct-returning match with parameter-dependent field arithmetic ─
#
# A struct-returning function with a match arm that computes fields from a
# parameter must emit runtime cmp (scrutinee) and add (field arithmetic).
# The result must NOT be constant-folded to `mov #N`.
#
# fn make(n: i32) -> Pair { match n { 1 => Pair { a: n + 10, b: n * 3 }, _ => Pair { a: 0, b: 0 } } }
# fn main() -> i32 { make(1).a }
#
# References: claims.md Claim 29.

echo "--- Claim 29: struct-returning match field arithmetic not folded ---"
if cargo test --test e2e --quiet -- runtime_struct_match_field_not_folded 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 29" "runtime_struct_match_field_not_folded FAILED — struct-returning match may be constant-folding n+10 to #11 instead of emitting runtime add"
else
    pass "Claim 29: struct-returning match field arithmetic emits runtime add (not folded)"
fi

# ── Claim 30: struct-returning if-else with parameter-dependent field arithmetic ─
#
# A struct-returning function with an if-else branch that computes fields from a
# parameter must emit a runtime conditional branch and add instruction.
# The result must NOT be constant-folded to `mov #N`.
#
# fn make(n: i32) -> Point { if n > 0 { Point { x: n + 1, y: n * 2 } } else { Point { x: 0, y: 0 } } }
# fn main() -> i32 { make(1).x }
#
# Complements Claim 29 for the if-else path. The existing cbz test only checks
# that a branch is present; this checks that field arithmetic is also runtime.
# References: claims.md Claim 30.

echo "--- Claim 30: struct-returning if-else field arithmetic not folded ---"
if cargo test --test e2e --quiet -- runtime_struct_return_if_else_not_folded 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 30" "runtime_struct_return_if_else_not_folded FAILED — struct-returning if-else may be constant-folding n+1 to #2 instead of emitting runtime add"
else
    pass "Claim 30: struct-returning if-else field arithmetic emits runtime add (not folded)"
fi

# ── Claim 31: dyn Trait method with struct field arithmetic emits runtime add ──
#
# A dyn Trait method that sums two struct fields must execute at runtime via
# vtable dispatch. When the struct is constructed from function parameters,
# the field values are unknown at compile time and must not be constant-folded.
#
# fn measure(d: &dyn Dist) -> i32 { d.manhattan() }
# fn make_and_measure(a: i32, b: i32) -> i32 { let p = Point { x: a, y: b }; measure(&p) }
# fn main() -> i32 { make_and_measure(3, 4) }
#
# Must emit vtable_Dist_Point, blr, and add — and must NOT emit `mov x0, #7`.
# References: claims.md Claim 31.

echo "--- Claim 31: dyn Trait field arithmetic not folded ---"
if cargo test --test e2e --quiet -- runtime_dyn_trait_field_arithmetic_not_folded 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 31" "runtime_dyn_trait_field_arithmetic_not_folded FAILED — dyn Trait field arithmetic may be constant-folded to #7 instead of emitting runtime add"
else
    pass "Claim 31: dyn Trait method field arithmetic emits runtime add (not folded)"
fi

# ── Claim 32: FnMut closures pass mutable captures by address and write back ──
#
# The distinguishing invariant of FnMut: the captured variable is passed by
# address (AddrOf: `add xN, sp, #offset`) so that writes inside the closure
# body propagate back to the caller. Without write-back, every call sees the
# initial value — `inc()` always returns 1 instead of accumulating.
#
# The test verifies three structural guarantees:
#   1. `add x_, sp, #N` — address-of the captured stack slot is computed.
#   2. `ldr xN, [xM]` (not [sp]) — value is read through an indirect pointer.
#   3. `str xN, [xM]` (not [sp]) — updated value is written back through
#      the pointer.
# Plus a negative assertion that the second `inc()` result is not folded.
#
# References: claims.md Claim 32.

echo "--- Claim 32: FnMut closures pass mutable captures by address (addr-of + write-back) ---"
if cargo test --test e2e --quiet -- runtime_fn_mut_emits_addr_of_and_load_store_ptr 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 32" "runtime_fn_mut_emits_addr_of_and_load_store_ptr FAILED — FnMut may be passing captures by copy (snapshot) instead of address, breaking write-back across calls"
else
    pass "Claim 32: FnMut mutable capture passes address and writes back through pointer (not snapshot)"
fi

# ── Claim 33: FnOnce closures capture by value and emit runtime add ───────────
#
# A `move || x + 1` closure called via `impl FnOnce() -> i32` must:
#   (a) emit `blr` — indirect call through the closure function pointer
#   (b) emit `add` — `x + 1` is a runtime instruction (not folded)
#   (c) NOT emit `mov x0, #42` — `run(41)` must not be constant-folded through the capture
#
# This completes the Fn/FnMut/FnOnce falsification triangle:
#   Claim 11 = Fn (non-capturing + capturing, hidden function label + blr)
#   Claim 32 = FnMut (mutable capture by address + write-back)
#   Claim 33 = FnOnce (move capture, runtime body, blr dispatch)
#
# References: claims.md Claim 33.

echo "--- Claim 33: FnOnce closure capture emits runtime add and blr (not folded) ---"
if cargo test --test e2e --quiet -- runtime_fn_once_capture_emits_runtime_add 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 33" "runtime_fn_once_capture_emits_runtime_add FAILED — FnOnce closure may be constant-folding the body (mov x0, #42) instead of emitting runtime add, or blr dispatch is missing"
else
    pass "Claim 33: FnOnce capture emits runtime add and blr (not constant-folded)"
fi

# ── Claim 34: Associated type method results are not constant-folded ───────────
#
# A trait method on a type with an associated type (`type Area = i32`) must emit
# runtime `mul` instructions even when call arguments are literals:
#
#   trait Shape { type Area; fn scaled_area(&self, scale: i32) -> i32; }
#   impl Shape for Square {
#       type Area = i32;
#       fn scaled_area(&self, scale: i32) -> i32 { self.side * self.side * scale }
#   }
#   fn main() -> i32 { let s = Square { side: 3 }; s.scaled_area(5) }
#
# The §10.2 associated type dispatch path is distinct from the generic trait dispatch
# in Claims 9–12 — it uses direct method dispatch through the concrete type, not
# vtable or generic-parameter monomorphization. Both must emit runtime instructions.
#
# References: claims.md Claim 34.

echo "--- Claim 34: assoc type method emits runtime mul (not folded to constant) ---"
if cargo test --test e2e --quiet -- runtime_assoc_type_method_emits_mul_not_folded 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 34" "runtime_assoc_type_method_emits_mul_not_folded FAILED — associated type method may be constant-folding 3*3*5=45 (mov x0, #45) instead of emitting runtime mul"
else
    pass "Claim 34: assoc type method result not folded (runtime mul emitted)"
fi

# ── Claim 35: Generic functions with assoc type bounds emit monomorphized calls ─
#
# A generic function `fn extract<T: Container<Item = i32>>(c: T) -> i32 { c.get_val() }`
# must:
#   (a) emit `bl extract__Wrapper` — monomorphized call, not constant-folded
#   (b) NOT emit `mov x0, #10` — `extract(Wrapper { val: 9 })` must not be folded
#
# The two-type variant additionally checks that both `Wrapper__get_val` and
# `Doubler__get_val` labels are present, and that the sum `7 + 5*2 = 17` is
# NOT folded to `mov x0, #17`.
#
# This covers the §10.2 + §12.1 associated type bound path (`T: Trait<Assoc = U>`),
# distinct from:
#   Claims 9–12  = plain trait bounds without associated type binding
#   Claim 34     = direct associated type method dispatch, not via generic parameter
#
# Introduced in cycle 71 (FLS §10.2 + §12.1 — assoc type bounds falsification).
# References: claims.md Claim 35.

echo "--- Claim 35: generic fn with assoc type bound emits monomorphized bl (not folded) ---"
if cargo test --test e2e --quiet -- runtime_assoc_type_bound_emits_monomorphized_bl_not_folded runtime_assoc_type_bound_two_types_both_monomorphized 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 35" "runtime_assoc_type_bound_emits_monomorphized_bl_not_folded or runtime_assoc_type_bound_two_types_both_monomorphized FAILED — generic fn with assoc type bound may not emit monomorphized call, or result was constant-folded"
else
    pass "Claim 35: generic fn with assoc type bound emits monomorphized bl (not folded)"
fi

# ── Claim 36: impl FnMut trampoline passes capture by address, mutation not folded ─
#
# When a mutable closure is passed as `impl FnMut`, galvanic must:
#   (a) emit `_trampoline` — the wrapper that receives closure state via x27
#   (b) use x27 for the capture-state pointer (capture by address, not by copy)
#   (c) emit `blr` in the consuming function for indirect closure call
#   (d) emit `add` — n+=1 in the closure body is a runtime instruction
#   (e) NOT emit `mov x0, #23` (correct result constant-folded from run(10))
#   (f) NOT emit `mov x0, #3` (snapshot-from-zero fold: two calls return 1+2=3)
#
# fn apply_mut(mut f: impl FnMut() -> i32) -> i32 { f() + f() }
# fn run(start: i32) -> i32 { let mut n = start; apply_mut(|| { n += 1; n }) }
# fn main() -> i32 { run(10) }
#
# Distinct from:
#   Claim 32 = direct FnMut closure (no generic dispatch, no trampoline)
#   Claim 33 = FnOnce via impl FnOnce (move capture, single call, no write-back)
# This claim covers the impl FnMut trampoline mechanism — a separate code path
# where galvanic must emit a trampoline that shuttles x27 (capture address) into
# the closure call, enabling mutable state accumulation across multiple calls.
#
# Attack vector: snapshot capture (copy n, not &n) causes each call to see the
# initial value → wrong result (2 or 22 depending on where snapshot is taken).
# Constant-fold: evaluate run(10) = 23 at compile time → mov x0, #23.
#
# Introduced in cycle 72 (red-team, milestone 152 path, FLS §6.22 + §4.13).
# References: claims.md Claim 36.

echo "--- Claim 36: impl FnMut trampoline mutation not folded ---"
if cargo test --test e2e --quiet -- runtime_fn_mut_as_impl_fn_mut_emits_trampoline runtime_fn_mut_as_impl_fn_mut_mutation_not_folded 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 36" "runtime_fn_mut_as_impl_fn_mut_emits_trampoline or runtime_fn_mut_as_impl_fn_mut_mutation_not_folded FAILED — impl FnMut trampoline may be missing, capture may be by copy (not address), or mutation result was constant-folded"
else
    pass "Claim 36: impl FnMut trampoline passes capture address (x27) and mutation is not folded"
fi

echo "--- Claim 37: &dyn Trait let binding emits fat pointer load and blr (not folded) ---"
if cargo test --test e2e --quiet -- runtime_dyn_trait_let_binding_not_folded runtime_dyn_trait_let_binding_emits_load_from_slot 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 37" "runtime_dyn_trait_let_binding_not_folded or runtime_dyn_trait_let_binding_emits_load_from_slot FAILED — &dyn Trait let binding may not be materializing the fat pointer, may be missing blr dispatch, or the result was constant-folded"
else
    pass "Claim 37: &dyn Trait let binding stores fat pointer and dispatches via blr (not folded)"
fi

# Unsafe block bodies must emit runtime instructions — unsafe is not a const context.
# Attack: constant-fold triple(4) = 12 to mov x0, #12 because the body is statically known.
# FLS §6.4.4 + §6.1.2 Constraint 1: unsafe block is not a const context.
#
# Introduced in cycle 74 (red-team, unsafe block path, FLS §6.4.4 + §6.1.2).
# References: claims.md Claim 38.

echo "--- Claim 38: unsafe block body emits runtime mul (not constant-folded) ---"
if cargo test --test e2e --quiet -- runtime_unsafe_block_emits_runtime_instructions_not_folded 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 38" "runtime_unsafe_block_emits_runtime_instructions_not_folded FAILED — unsafe block body may be constant-folded (unsafe is NOT a const context per FLS §6.4.4)"
else
    pass "Claim 38: unsafe block body emits runtime mul (not constant-folded)"
fi

# `const fn` called from a non-const context must emit a runtime bl, not be folded.
# A `const fn` is only eligible for compile-time evaluation when called from a const
# context (const item, const block, array length, etc.). When called from `fn main()`
# the arguments may be dynamic at runtime; the compiler must emit a real function call.
# FLS §9:41–43 (Constraint 2): `const fn` called outside a const context runs as
# normal code and must emit a runtime bl instruction.
#
# Attack: constant-fold `add(20, 22)` to `mov x0, #42` because both arguments are
# known constants. The exit code would still be 42 (correct), but the program is
# semantically wrong — the function must execute at runtime.
#
# Introduced in cycle 75 (red-team, const fn context-sensitivity, FLS §9:41–43).
# References: claims.md Claim 39.

echo "--- Claim 39: const fn called from non-const context emits runtime bl (not folded) ---"
if cargo test --test e2e --quiet -- runtime_const_fn_runtime_call_emits_bl_not_folded 2>&1 | grep -q "FAILED\|error\["; then
    fail "Claim 39" "runtime_const_fn_runtime_call_emits_bl_not_folded FAILED — const fn may be constant-folded when called from non-const context (violates FLS §9:41–43)"
else
    pass "Claim 39: const fn called from non-const context emits runtime bl (not folded)"
fi

# ── Summary ───────────────────────────────────────────────────────────────────

echo ""
echo "Falsification result: $PASS passed, $FAIL failed"

if [ "$FAIL" -gt 0 ]; then
    echo ""
    echo "FAILED claims (agent must fix before any other work):"
    printf '%b\n' "$ERRORS"
    exit 1
fi

exit 0
