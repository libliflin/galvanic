#!/usr/bin/env bash
# falsify.sh — Adversarial checks for galvanic's load-bearing claims.
#
# Each section targets one claim from .lathe/claims.md.
# Exit 0 if all claims hold. Exit non-zero if any fail.
# Prints a summary line at the end regardless of pass/fail.
#
# Must be fast (runs every cycle). Seconds, not minutes.
# Must not require network or external services.

set -uo pipefail

# ── Locate cargo ─────────────────────────────────────────────────────────────

CARGO="${CARGO:-}"
if [[ -z "$CARGO" ]]; then
    for candidate in /opt/homebrew/bin/cargo /usr/local/bin/cargo; do
        if [[ -x "$candidate" ]]; then
            CARGO="$candidate"
            break
        fi
    done
fi
if [[ -z "$CARGO" ]] && command -v cargo &>/dev/null; then
    CARGO="cargo"
fi
if [[ -z "$CARGO" ]]; then
    echo "FATAL: cargo not found — set CARGO env var or install rustup"
    echo "=== Summary === passed: 0 failed: 1"
    exit 1
fi

PASS=0
FAIL=0

claim_ok()   { echo "  ok: $1"; ((PASS++)) || true; }
claim_fail() { echo "FAIL: $1 — $2"; ((FAIL++)) || true; }

# ── Claim 1: build succeeds ──────────────────────────────────────────────────

echo "--- Claim 1: build succeeds"
if "$CARGO" build -q 2>/dev/null; then
    claim_ok "cargo build exits 0"
else
    claim_fail "build succeeds" "cargo build exited non-zero — see 'cargo build' output for details"
fi

# ── Claim 2: Token is 8 bytes ────────────────────────────────────────────────

echo "--- Claim 2: Token is 8 bytes"
if "$CARGO" test --lib -q -- lexer::tests::token_is_eight_bytes 2>/dev/null; then
    claim_ok "size_of::<Token>() == 8"
else
    claim_fail "Token is 8 bytes" "lexer::tests::token_is_eight_bytes failed — Token struct grew beyond 8 bytes"
fi

# ── Claim 3: FLS parse-acceptance fixtures pass ──────────────────────────────

echo "--- Claim 3: FLS parse-acceptance fixtures pass"
if "$CARGO" test --test fls_fixtures -q 2>/dev/null; then
    claim_ok "all fls_fixtures tests pass"
else
    claim_fail "FLS parse fixtures" "one or more fls_fixtures tests failed — run 'cargo test --test fls_fixtures' for detail"
fi

# ── Claim 4: non-const code emits runtime instructions ───────────────────────
#
# Compile fn main() -> i32 { 1 + 2 } and verify the assembly contains 'add'.
# If galvanic constant-folds this to 'mov x0, #3', this claim fails.
# If the binary isn't built, we skip (Claim 1 covers the build).

echo "--- Claim 4: non-const code emits runtime instructions"
GALVANIC="./target/debug/galvanic"
if [[ ! -x "$GALVANIC" ]]; then
    claim_fail "non-const runtime codegen" "galvanic binary not found at $GALVANIC (did Claim 1 pass?)"
else
    TMP_RS=$(mktemp /tmp/galvanic_falsify_XXXXXX.rs)
    TMP_S="${TMP_RS%.rs}.s"
    printf 'fn main() -> i32 { 1 + 2 }\n' > "$TMP_RS"

    if "$GALVANIC" "$TMP_RS" > /dev/null 2>&1; then
        if [[ -f "$TMP_S" ]]; then
            # Check for 'add' instruction — fails if constant-folded to 'mov x0, #3'
            # Disable pipefail around grep: grep exits 1 when nothing matches
            set +o pipefail
            ADD_LINES=$(grep -cE '\badd\b' "$TMP_S" 2>/dev/null || echo 0)
            set -o pipefail
            if [[ "$ADD_LINES" -gt 0 ]]; then
                claim_ok "fn main() { 1 + 2 } emits 'add' instruction (not constant-folded)"
            else
                claim_fail "non-const runtime codegen" \
                    "no 'add' instruction found for '1 + 2' — galvanic may be constant-folding non-const code. Assembly: $(cat "$TMP_S")"
            fi
            rm -f "$TMP_S"
        else
            # galvanic exited 0 but produced no .s file — probably because there
            # is no 'main' function or it returned early. That's unexpected.
            claim_fail "non-const runtime codegen" \
                "galvanic compiled without error but produced no .s file for test input"
        fi
    else
        claim_fail "non-const runtime codegen" \
            "galvanic exited non-zero when compiling test input (fn main() -> i32 { 1 + 2 })"
    fi
    rm -f "$TMP_RS"
fi

# ── Claim 5: adversarial inputs exit cleanly ─────────────────────────────────
#
# The binary must not panic (SIGABRT) or crash (SIGSEGV) on adversarial input.
# A non-zero exit code is acceptable; signal death (exit > 128) is not.

echo "--- Claim 5: adversarial inputs exit cleanly"
if [[ ! -x "$GALVANIC" ]]; then
    claim_fail "adversarial inputs" "galvanic binary not found — did Claim 1 pass?"
else
    ADVERSARIAL_FAIL=0

    # 5a: empty file → exit 0
    TMP_EMPTY=$(mktemp /tmp/galvanic_falsify_empty_XXXXXX.rs)
    printf '' > "$TMP_EMPTY"
    set +o pipefail
    "$GALVANIC" "$TMP_EMPTY" > /dev/null 2>&1
    EMPTY_EXIT=$?
    set -o pipefail
    rm -f "$TMP_EMPTY"
    if [[ "$EMPTY_EXIT" -gt 128 ]]; then
        claim_fail "adversarial: empty file" "exited with signal (code $EMPTY_EXIT)"
        ADVERSARIAL_FAIL=1
    fi

    # 5b: binary garbage → clean exit (non-zero ok, signal not ok)
    TMP_GARBAGE=$(mktemp /tmp/galvanic_falsify_garbage_XXXXXX.rs)
    # 512 bytes of pseudo-random-looking content without needing /dev/urandom
    python3 -c "import os; open('$TMP_GARBAGE','wb').write(os.urandom(512))" 2>/dev/null \
        || printf '%0.s\xde\xad\xbe\xef' {1..128} > "$TMP_GARBAGE"
    set +o pipefail
    "$GALVANIC" "$TMP_GARBAGE" > /dev/null 2>&1
    GARBAGE_EXIT=$?
    set -o pipefail
    rm -f "$TMP_GARBAGE"
    if [[ "$GARBAGE_EXIT" -gt 128 ]]; then
        claim_fail "adversarial: binary garbage" "exited with signal (code $GARBAGE_EXIT)"
        ADVERSARIAL_FAIL=1
    fi

    # 5c: deeply nested braces → clean exit (stack overflow = signal, not ok)
    TMP_NESTED=$(mktemp /tmp/galvanic_falsify_nested_XXXXXX.rs)
    {
        echo 'fn main() {'
        for _ in $(seq 1 300); do echo '  {'; done
        echo '  let _x = 0;'
        for _ in $(seq 1 300); do echo '  }'; done
        echo '}'
    } > "$TMP_NESTED"
    set +o pipefail
    "$GALVANIC" "$TMP_NESTED" > /dev/null 2>&1
    NESTED_EXIT=$?
    set -o pipefail
    rm -f "$TMP_NESTED"
    if [[ "$NESTED_EXIT" -gt 128 ]]; then
        claim_fail "adversarial: 300-deep nested braces" "exited with signal (code $NESTED_EXIT)"
        ADVERSARIAL_FAIL=1
    fi

    if [[ "$ADVERSARIAL_FAIL" -eq 0 ]]; then
        claim_ok "empty file, binary garbage, and deep nesting all exit cleanly"
    fi
fi

# ── Summary ───────────────────────────────────────────────────────────────────

echo ""
echo "=== Summary === passed: $PASS failed: $FAIL"

if [[ "$FAIL" -gt 0 ]]; then
    exit 1
fi
