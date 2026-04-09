#!/usr/bin/env bash
# falsify.sh — Adversarial checks for galvanic's load-bearing claims.
#
# Runs every cycle. Exit 0 if all claims hold; non-zero if any fail.
# Must be fast (warm-cache cargo is ~1-5 seconds per invocation).
# Must not require network or external services.
#
# See .lathe/claims.md for the claims this script defends.

set -uo pipefail
# Note: NOT using -e so we can collect all failures rather than stopping at the first.

PASS=0
FAIL=0

ok() {
    echo "  ok: $1"
    PASS=$((PASS + 1))
}

fail() {
    echo "  FAIL: $1"
    FAIL=$((FAIL + 1))
}

echo "=== Falsification Suite ==="
echo ""

# ── Claim 4: No unsafe code in library source ─────────────────────────────────
#
# grep returns 1 when it finds nothing — that's the passing case here.
# We want to fail when grep FINDS matches, so we invert the logic.
echo "--- Claim 4: No unsafe in library source ---"
# Exclude main.rs (the CLI driver may use unsafe for assembler/linker interaction).
# Exclude comment lines (lines starting with //).
# Use || true to prevent pipefail from treating grep-finds-nothing as a script error.
UNSAFE_LINES=$(grep -rn 'unsafe\s*{\|unsafe\s*fn\b\|unsafe\s*impl\b' src/ \
    --include='*.rs' \
    | grep -v '^src/main\.rs:' \
    | grep -Ev ':[0-9]+:[[:space:]]*//' \
    || true)
if [[ -n "$UNSAFE_LINES" ]]; then
    fail "unsafe code found in library source:"
    echo "$UNSAFE_LINES" | head -5
else
    ok "no unsafe in library source"
fi
echo ""

# ── Claim 1: Build integrity — cargo build ────────────────────────────────────
echo "--- Claim 1a: cargo build ---"
if cargo build 2>&1; then
    ok "cargo build clean"
else
    fail "cargo build failed"
fi
echo ""

# ── Claim 1b: Clippy clean ────────────────────────────────────────────────────
echo "--- Claim 1b: cargo clippy ---"
if cargo clippy -- -D warnings 2>&1; then
    ok "clippy clean"
else
    fail "clippy reported warnings or errors"
fi
echo ""

# ── Claim 3: Token is 8 bytes ─────────────────────────────────────────────────
echo "--- Claim 3: Token size == 8 bytes ---"
if cargo test --lib -- --exact lexer::tests::token_is_eight_bytes 2>&1; then
    ok "Token is 8 bytes"
else
    fail "Token size check failed — size_of::<Token>() != 8"
fi
echo ""

# ── Claim 5: Runtime instruction emission (no const-fold in non-const fn) ─────
#
# compile_to_asm() in e2e.rs runs lex→parse→lower→codegen in-process.
# No ARM64 tools or QEMU required — works on macOS and Linux.
echo "--- Claim 5: Runtime instruction emission (1 + 2 emits add, not mov #3) ---"
if cargo test --test e2e -- --exact runtime_add_emits_add_instruction 2>&1; then
    ok "1 + 2 emits runtime add instruction"
else
    fail "const-fold violation: 1 + 2 does not emit runtime add instruction"
fi
echo ""

# ── Claim 6: CLI handles adversarial inputs without panicking ─────────────────
#
# Signal-kill (exit > 128) is a panic/crash. Clean non-zero exit is acceptable.
echo "--- Claim 6: Adversarial inputs do not crash the CLI ---"

# Build release binary first (needed for CLI tests).
if ! cargo build --release 2>&1; then
    fail "release build failed — cannot test CLI"
else
    BINARY="./target/release/galvanic"
    ADVERSARIAL_PASS=0
    ADVERSARIAL_FAIL=0

    _check_no_crash() {
        local label="$1"
        local input_file="$2"
        set +o pipefail
        timeout 10 "$BINARY" "$input_file" 2>/dev/null
        local exit_code=$?
        set -o pipefail
        if [[ $exit_code -gt 128 ]]; then
            local signal=$((exit_code - 128))
            echo "    CRASH on $label (signal $signal)"
            ADVERSARIAL_FAIL=$((ADVERSARIAL_FAIL + 1))
        else
            ADVERSARIAL_PASS=$((ADVERSARIAL_PASS + 1))
        fi
    }

    TMPDIR_ADV=$(mktemp -d)

    # Empty file
    touch "$TMPDIR_ADV/empty.rs"
    _check_no_crash "empty file" "$TMPDIR_ADV/empty.rs"

    # NUL bytes in source
    printf 'fn main() {\x00\x00\x00}' > "$TMPDIR_ADV/nul.rs"
    _check_no_crash "NUL bytes" "$TMPDIR_ADV/nul.rs"

    # Binary garbage (64 bytes of random-ish data)
    printf '\xde\xad\xbe\xef\x00\xff\x80\x7f%.0s' {1..8} > "$TMPDIR_ADV/garbage.rs"
    _check_no_crash "binary garbage" "$TMPDIR_ADV/garbage.rs"

    # Deeply nested braces (200 levels — fast to generate, tests stack depth)
    python3 -c "
print('fn main() {')
for i in range(200):
    print('  { let _x = 0;')
for i in range(200):
    print('  }')
print('}')
" > "$TMPDIR_ADV/nested.rs"
    _check_no_crash "200 levels of nesting" "$TMPDIR_ADV/nested.rs"

    rm -rf "$TMPDIR_ADV"

    if [[ $ADVERSARIAL_FAIL -eq 0 ]]; then
        ok "CLI survived $ADVERSARIAL_PASS adversarial inputs"
    else
        fail "CLI crashed on $ADVERSARIAL_FAIL adversarial inputs (passed $ADVERSARIAL_PASS)"
        FAIL=$((FAIL + ADVERSARIAL_FAIL - 1))  # already counted one fail above
    fi
fi
echo ""

# ── Claim 7: Block/paren disambiguation (FLS §6.21) ──────────────────────────
#
# A `(` following a block-like expression (for, while, loop, if, match) must
# be parsed as a new parenthesized expression, not a call postfix. This was
# silently regressed in the re-init commits and only caught by compile_to_asm.
# No ARM64 tools or QEMU required — compile_to_asm is pure in-process codegen.
echo "--- Claim 7: Block/paren disambiguation (FLS §6.21) ---"
if cargo test --test e2e -- --exact runtime_for_block_then_paren_emits_add_not_blr 2>&1; then
    ok "for {} (expr) emits add not blr"
else
    fail "block/paren parser regression: for {} (expr) may be parsed as a call"
fi
echo ""

# ── Summary ───────────────────────────────────────────────────────────────────
echo "=== Summary === passed: $PASS  failed: $FAIL"
echo ""

if [[ $FAIL -gt 0 ]]; then
    exit 1
fi
exit 0
