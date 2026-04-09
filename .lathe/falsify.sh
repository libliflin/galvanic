#!/usr/bin/env bash
# .lathe/falsify.sh — Adversarial claim checks for galvanic.
#
# Runs every cycle. Exit 0 = all claims hold. Exit non-zero = at least one failed.
# Fast: seconds, not minutes. No network. Uses the project's own toolchain.
# Must always print the "=== Summary ===" line — even if a claim check dies.
#
# Do NOT invoke this script from snapshot.sh. The engine calls it separately
# and appends its output to the snapshot under "## Falsification".

# -u: catch unbound variable typos. No -e: we handle per-claim failures ourselves.
# No pipefail: grep legitimately exits 1 on no-match; we don't want that to kill us.
set -u

PASS=0
FAIL=0

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

GALVANIC_BIN="$PROJECT_ROOT/target/debug/galvanic"

# ── Helpers ───────────────────────────────────────────────────────────────────

pass() {
    echo "  ok: $1"
    PASS=$((PASS + 1))
}

fail() {
    echo "FAIL: $1 — $2"
    FAIL=$((FAIL + 1))
}

# ── Claim 1: Token is 8 bytes ─────────────────────────────────────────────────
# Stakeholder: cache-line codegen researcher
# Promise: size_of::<Token>() == 8 — the "8 tokens per cache line" contract.
# Adversarial: any added field or widened type in Token triggers failure.

echo "Claim 1: Token is 8 bytes"
TOKEN_OUT=$(cargo test --lib -- --exact lexer::tests::token_is_eight_bytes 2>&1) || true
if echo "$TOKEN_OUT" | grep -q "token_is_eight_bytes ... ok"; then
    pass "token-is-8-bytes"
elif echo "$TOKEN_OUT" | grep -q "FAILED"; then
    fail "token-is-8-bytes" "Token exceeded 8 bytes — cache-line density is broken"
else
    fail "token-is-8-bytes" "test not found or cargo error (check that lexer::tests::token_is_eight_bytes exists)"
fi

# ── Claim 2: Non-const functions emit runtime instructions ────────────────────
# Stakeholder: FLS researcher, maintainer
# Promise: fn f(a: i32, b: i32) -> i32 { a + b } emits a runtime 'add' instruction.
# Adversarial: parameters are not statically known — a const-folding interpreter
# would produce 'mov x0, #7' from main without ever emitting 'add' in f's body.

echo "Claim 2: Non-const fn emits runtime instructions (not constant-folded)"
if [[ ! -x "$GALVANIC_BIN" ]]; then
    fail "runtime-codegen" "galvanic binary not built — run 'cargo build' first"
else
    TMP_SRC=$(mktemp /tmp/galvanic_falsify_XXXXXX.rs)
    TMP_ASM="${TMP_SRC%.rs}.s"

    cat > "$TMP_SRC" << 'EOF'
fn add(a: i32, b: i32) -> i32 { a + b }
fn main() -> i32 { add(3, 4) }
EOF

    "$GALVANIC_BIN" "$TMP_SRC" > /dev/null 2>&1 || true

    if [[ -f "$TMP_ASM" ]]; then
        # Check that the 'add' function body contains a runtime add instruction.
        # GAS syntax: 'add\t' or 'add ' (instruction mnemonic followed by space/tab).
        # We look specifically past the function label to avoid matching the label name.
        if grep -qE '^\s+add\b' "$TMP_ASM"; then
            pass "runtime-codegen"
        else
            fail "runtime-codegen" "fn add(a,b){a+b} emitted no ARM64 'add' instruction — possible const-fold violation"
            echo "  (emitted assembly for inspection:)"
            cat "$TMP_ASM" | head -40 || true
        fi
        rm -f "$TMP_ASM"
    else
        fail "runtime-codegen" "galvanic produced no .s file for parametric add fn (lowering or codegen failed)"
    fi

    rm -f "$TMP_SRC"
fi

# ── Claim 3: No unsafe code in library modules ────────────────────────────────
# Stakeholder: safety researcher, library users
# Promise: no unsafe blocks/fn/impl in src/ outside src/main.rs.
# Adversarial: any unsafe added for convenience (bounds skip, raw pointer cast).

echo "Claim 3: No unsafe in library code"
# Exclude main.rs (CLI driver is permitted to use unsafe for assembly/linking).
# Exclude comment lines (lines where the code content starts with //).
UNSAFE_FOUND=$(grep -rn 'unsafe[[:space:]]*{' src/ 2>/dev/null | \
    grep -v '^src/main\.rs:' | \
    grep -Ev ':[0-9]+:[[:space:]]*//' || true)
if [[ -z "$UNSAFE_FOUND" ]]; then
    pass "no-unsafe-in-lib"
else
    fail "no-unsafe-in-lib" "unsafe block found outside main.rs: $UNSAFE_FOUND"
fi

# ── Claim 4: Library never shells out ─────────────────────────────────────────
# Stakeholder: library user
# Promise: no std::process::Command in src/ outside src/main.rs.
# Adversarial: a helper in lower.rs or codegen.rs that invokes an external tool.

echo "Claim 4: Library never shells out"
CMD_FOUND=$(grep -rn 'process::Command' src/ 2>/dev/null | \
    grep -v '^src/main\.rs:' || true)
if [[ -z "$CMD_FOUND" ]]; then
    pass "no-command-in-lib"
else
    fail "no-command-in-lib" "process::Command found outside main.rs: $CMD_FOUND"
fi

# ── Claim 5: Clean exit on empty input ────────────────────────────────────────
# Stakeholder: CLI user
# Promise: galvanic exits 0 on an empty .rs file, without panic or signal.

echo "Claim 5: Clean exit on empty input"
if [[ ! -x "$GALVANIC_BIN" ]]; then
    fail "clean-exit-empty-input" "galvanic binary not built"
else
    TMP_EMPTY=$(mktemp /tmp/galvanic_empty_XXXXXX.rs)
    EMPTY_EXIT=0
    "$GALVANIC_BIN" "$TMP_EMPTY" > /dev/null 2>/dev/null || EMPTY_EXIT=$?
    rm -f "$TMP_EMPTY"

    if [[ "$EMPTY_EXIT" -eq 0 ]]; then
        pass "clean-exit-empty-input"
    elif [[ "$EMPTY_EXIT" -gt 128 ]]; then
        SIGNAL=$((EMPTY_EXIT - 128))
        fail "clean-exit-empty-input" "binary died on signal $SIGNAL (exit $EMPTY_EXIT)"
    else
        fail "clean-exit-empty-input" "unexpected non-zero exit $EMPTY_EXIT on empty input"
    fi
fi

# ── Claim 6: Clean error on missing file ──────────────────────────────────────
# Stakeholder: CLI user
# Promise: galvanic exits non-zero (≤ 128) on a nonexistent file path.

echo "Claim 6: Clean error on missing file"
if [[ ! -x "$GALVANIC_BIN" ]]; then
    fail "clean-error-missing-file" "galvanic binary not built"
else
    MISSING_EXIT=0
    "$GALVANIC_BIN" /tmp/galvanic_does_not_exist_$(date +%s).rs > /dev/null 2>/dev/null \
        || MISSING_EXIT=$?

    if [[ "$MISSING_EXIT" -gt 0 && "$MISSING_EXIT" -le 128 ]]; then
        pass "clean-error-missing-file"
    elif [[ "$MISSING_EXIT" -gt 128 ]]; then
        SIGNAL=$((MISSING_EXIT - 128))
        fail "clean-error-missing-file" "binary died on signal $SIGNAL on missing file (exit $MISSING_EXIT)"
    else
        fail "clean-error-missing-file" "expected non-zero exit on missing file, got 0"
    fi
fi

# ── Summary ───────────────────────────────────────────────────────────────────

echo ""
echo "=== Summary === passed: $PASS failed: $FAIL"

[[ "$FAIL" -eq 0 ]]
