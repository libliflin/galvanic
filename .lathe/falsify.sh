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

UNSAFE_BLOCKS=$(grep -rn 'unsafe\s*{' src/ 2>/dev/null | grep -v '^src/main\.rs:' || true)
UNSAFE_FNS=$(grep -rn 'unsafe\s*fn\b' src/ 2>/dev/null | grep -v '^src/main\.rs:' || true)
UNSAFE_IMPLS=$(grep -rn 'unsafe\s*impl\b' src/ 2>/dev/null | grep -v '^src/main\.rs:' || true)

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
