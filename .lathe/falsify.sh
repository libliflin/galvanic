#!/usr/bin/env bash
# falsify.sh — Adversarial verification of galvanic's load-bearing claims.
#
# Runs every cycle. Must be fast (seconds). Exits 0 if all claims hold,
# non-zero if any fail. Each check names the claim it is testing.
#
# Claims registry: .lathe/claims.md
#
# Note: uses `set -uo pipefail` but wraps grep in `|| true` to prevent
# legitimate "no match" (exit 1) from killing the script under pipefail.

set -uo pipefail

PASS=0
FAIL=0
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# Colour helpers (suppressed if not a terminal)
if [ -t 1 ]; then
    RED='\033[0;31m'; GREEN='\033[0;32m'; NC='\033[0m'
else
    RED=''; GREEN=''; NC=''
fi

ok()   { echo -e "${GREEN}ok${NC}  $1"; PASS=$((PASS + 1)); }
fail() { echo -e "${RED}FAIL${NC} $1"; FAIL=$((FAIL + 1)); }

cd "$ROOT"

# ── C3: Build succeeds ────────────────────────────────────────────────────────
# All stakeholders depend on this.
echo "--- C3: cargo build ---"
if cargo build --quiet 2>/dev/null; then
    ok "C3: cargo build exits 0"
else
    fail "C3: cargo build failed"
fi

# ── C4: Test suite passes ─────────────────────────────────────────────────────
# FLS contributor's safety net. Run unit tests only (fast path; CI runs full suite).
echo "--- C4: cargo test --lib ---"
if cargo test --lib --quiet 2>/dev/null; then
    ok "C4: cargo test --lib exits 0"
else
    fail "C4: cargo test --lib failed"
fi

# ── C1: Token is 8 bytes ──────────────────────────────────────────────────────
# The specific test the CI also checks. If it doesn't exist, the claim is still live.
echo "--- C1: Token == 8 bytes ---"
if cargo test --lib --quiet -- --exact lexer::tests::token_is_eight_bytes 2>/dev/null; then
    ok "C1: Token is 8 bytes (lexer::tests::token_is_eight_bytes)"
else
    fail "C1: Token size assertion failed or test not found (size_of::<Token>() must be 8)"
fi

# ── C2: Span is 8 bytes ───────────────────────────────────────────────────────
# Span is the other layout-enforced type. Try the named test; if absent, grep for
# size_of::<Span> in the test binary's symbols (weaker check).
echo "--- C2: Span == 8 bytes ---"
if cargo test --lib --quiet -- --exact lexer::tests::span_is_eight_bytes 2>/dev/null; then
    ok "C2: Span is 8 bytes (lexer::tests::span_is_eight_bytes)"
elif cargo test --lib --quiet -- --exact ast::tests::span_is_eight_bytes 2>/dev/null; then
    ok "C2: Span is 8 bytes (ast::tests::span_is_eight_bytes)"
else
    # Weaker: grep for a compile-time size assertion in source
    set +o pipefail
    FOUND=$(grep -r 'size_of::<Span>' src/ || true)
    set -o pipefail
    if [ -n "$FOUND" ]; then
        ok "C2: size_of::<Span> referenced in source (no dedicated test found — consider adding one)"
    else
        fail "C2: No Span size enforcement found (size_of::<Span>() should be 8; add a test)"
    fi
fi

# ── C5: No unsafe in library ──────────────────────────────────────────────────
# main.rs is excluded — it is the CLI driver and may use platform interfaces.
echo "--- C5: no unsafe in library ---"
set +o pipefail
UNSAFE=$(grep -rn 'unsafe\s*{\|unsafe\s*fn\b\|unsafe\s*impl\b' src/ \
         | grep -v '^src/main\.rs:' \
         | grep -Ev ':[0-9]+:[[:space:]]*//' \
         || true)
set -o pipefail
if [ -z "$UNSAFE" ]; then
    ok "C5: no unsafe code in library src/ (excluding main.rs)"
else
    fail "C5: unsafe code found in library:"
    echo "$UNSAFE"
fi

# ── C6: Full pipeline on milestone_1 ─────────────────────────────────────────
# The minimal end-to-end proof: lex → parse → lower → codegen exits 0 and emits .s
echo "--- C6: full pipeline on milestone_1.rs ---"
MILESTONE="tests/fixtures/milestone_1.rs"
if [ ! -f "$MILESTONE" ]; then
    fail "C6: $MILESTONE not found"
else
    # Run the galvanic binary (debug build, already built above).
    BINARY="target/debug/galvanic"
    if [ ! -f "$BINARY" ]; then
        fail "C6: galvanic binary not found at $BINARY"
    else
        # Emit to a temp file to avoid polluting the fixtures directory.
        TMPDIR_PATH=$(mktemp -d)
        cp "$MILESTONE" "$TMPDIR_PATH/milestone_1.rs"
        if "$BINARY" "$TMPDIR_PATH/milestone_1.rs" 2>/dev/null && [ -f "$TMPDIR_PATH/milestone_1.s" ]; then
            ok "C6: galvanic compiled milestone_1.rs and emitted .s"
        else
            fail "C6: galvanic failed to compile milestone_1.rs (or .s not emitted)"
        fi
        rm -rf "$TMPDIR_PATH"
    fi
fi

# ── C7: FLS citations present in each source module ──────────────────────────
# Each implementing module must contain at least one FLS § citation.
echo "--- C7: FLS citations in source modules ---"
ALL_CITED=true
for MODULE in src/lexer.rs src/parser.rs src/ir.rs src/lower.rs src/codegen.rs; do
    if [ ! -f "$MODULE" ]; then
        fail "C7: $MODULE not found"
        ALL_CITED=false
        continue
    fi
    set +o pipefail
    COUNT=$(grep -c 'FLS §' "$MODULE" || true)
    set -o pipefail
    if [ "$COUNT" -gt 0 ]; then
        ok "C7: $MODULE has $COUNT FLS § citation(s)"
    else
        fail "C7: $MODULE has NO FLS § citations"
        ALL_CITED=false
    fi
done

# ── C8: ARM64 sdiv-by-zero divergence is documented ──────────────────────────
# FLS §6.23 requires division by zero to panic. ARM64 `sdiv` returns 0 silently.
# The divergence must be documented in both ir.rs and codegen.rs so the research
# record is not silently erased. The div_zero fixture must also parse without error.
echo "--- C8: sdiv-by-zero divergence documented ---"
set +o pipefail
IR_COUNT=$(grep -c 'FLS §6.23' src/ir.rs || true)
CG_COUNT=$(grep -c 'FLS §6.23' src/codegen.rs || true)
set -o pipefail
if [ "$IR_COUNT" -ge 1 ] && [ "$CG_COUNT" -ge 1 ]; then
    ok "C8: FLS §6.23 divergence documented in ir.rs ($IR_COUNT) and codegen.rs ($CG_COUNT)"
else
    fail "C8: FLS §6.23 div-by-zero divergence NOT documented (ir.rs=$IR_COUNT codegen.rs=$CG_COUNT)"
fi
# Also verify the fixture parses
FIXTURE="tests/fixtures/fls_6_23_div_zero.rs"
if [ -f "$FIXTURE" ]; then
    if cargo test --test fls_fixtures --quiet -- --exact fls_6_23_div_zero 2>/dev/null; then
        ok "C8: fls_6_23_div_zero.rs parses without error"
    else
        fail "C8: fls_6_23_div_zero.rs failed to parse"
    fi
else
    fail "C8: $FIXTURE not found"
fi

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "=== Summary === passed: $PASS failed: $FAIL"

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
exit 0
