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
