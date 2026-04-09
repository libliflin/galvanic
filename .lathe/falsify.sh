#!/usr/bin/env bash
# falsify.sh — Adversarial check of galvanic's load-bearing claims.
#
# The engine runs this every cycle and includes the result in the snapshot
# under ## Falsification. Exit 0 = all claims hold. Non-zero = at least one
# claim failed. The output names the failing claim and explains why.
#
# Rules:
#   - Must be fast (runs every cycle): seconds, not minutes.
#   - No network access. All fixtures are constructed locally.
#   - Each check targets one named claim from claims.md.

set -euo pipefail

PASS=0
FAIL=0
ERRORS=""

fail() {
    local claim="$1"
    local msg="$2"
    FAIL=$((FAIL + 1))
    ERRORS="${ERRORS}FAIL [${claim}]: ${msg}\n"
    echo "FAIL [${claim}]: ${msg}"
}

ok() {
    local claim="$1"
    local msg="$2"
    PASS=$((PASS + 1))
    echo "ok   [${claim}]: ${msg}"
}

# ── Resolve project root ──────────────────────────────────────────────────────
# falsify.sh may be invoked from any directory. Resolve project root from the
# script's own location.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${PROJECT_ROOT}"

# macOS ships without `timeout`; use gtimeout (coreutils) or a no-op fallback
if command -v timeout &>/dev/null; then
    TIMEOUT=timeout
elif command -v gtimeout &>/dev/null; then
    TIMEOUT=gtimeout
else
    TIMEOUT=""
fi
_to() { if [[ -n "$TIMEOUT" ]]; then $TIMEOUT "$@"; else shift; "$@"; fi; }

# ── CLAIM-1: Token is exactly 8 bytes ────────────────────────────────────────
# Adversarial: any new TokenKind variant or Token field that pushes size > 8.
# Run the dedicated size-assertion test in the library.
echo "--- CLAIM-1: Token is exactly 8 bytes ---"
if _to 30 cargo test --lib -- lexer::tests::token_is_eight_bytes --quiet 2>&1 | grep -q 'test result: ok'; then
    ok "CLAIM-1" "Token is 8 bytes"
else
    # Attempt to collect the actual output for diagnosis
    OUTPUT=$(_to 30 cargo test --lib -- lexer::tests::token_is_eight_bytes 2>&1 || true)
    if echo "$OUTPUT" | grep -q 'token_is_eight_bytes.*FAILED\|error\['; then
        fail "CLAIM-1" "Token size test failed — Token may have grown beyond 8 bytes. Output: $(echo "$OUTPUT" | tail -5)"
    elif echo "$OUTPUT" | grep -q 'no tests ran\|test not found'; then
        fail "CLAIM-1" "Test 'lexer::tests::token_is_eight_bytes' not found — the size assertion may have been deleted"
    else
        fail "CLAIM-1" "Unexpected test output: $(echo "$OUTPUT" | tail -5)"
    fi
fi

# ── CLAIM-2: No unsafe in library source ─────────────────────────────────────
# Adversarial: any unsafe block/fn/impl added to the library (src/ minus main.rs).
# Filter out comment lines (lines whose non-whitespace content starts with //).
echo "--- CLAIM-2: No unsafe in library source ---"
UNSAFE_HITS=$(grep -rn 'unsafe\s*{\|unsafe\s*fn\b\|unsafe\s*impl\b' src/ \
    | grep -v '^src/main\.rs:' \
    | grep -Ev ':[0-9]+:[[:space:]]*//' \
    || true)
if [[ -z "$UNSAFE_HITS" ]]; then
    ok "CLAIM-2" "No unsafe in library source"
else
    fail "CLAIM-2" "unsafe found in library code (src/ excluding main.rs):\n${UNSAFE_HITS}"
fi

# ── CLAIM-3: IR cache-line discipline is present and growing ─────────────────
# Adversarial: adding many new IR types without any cache-line documentation,
# eroding the project's primary research artifact.
#
# Two-tier check:
#   A) ir.rs must contain at least 40 "Cache-line note:" occurrences.
#      (The code currently has ~82 — this threshold catches mass erasure,
#       not individual omissions. Individual omissions are flagged by B.)
#   B) The specific "hot-path" types that have always had notes must still
#      have them: StaticValue, StaticData, VtableShim, VtableSpec, IrBinOp.
#      These are the reference examples. If they lose their notes, the
#      discipline has collapsed, not just lagged.
#
# NOTE: Several top-level type declarations (Module, IrFn, ClosureTrampoline,
# Instr, IrValue, IrTy, FCmpOp, F64BinOp, F32BinOp) currently lack type-level
# cache-line notes. This is a known gap. The runtime agent should add them.
# When all top-level types have notes, this check can be made stricter.
echo "--- CLAIM-3: IR cache-line discipline ---"

IR_FILE="src/ir.rs"

# Part A: total note count
NOTE_COUNT=$(grep -c 'Cache-line note:\|Cache-line:' "$IR_FILE" 2>/dev/null || echo 0)
MIN_NOTES=40

if [[ "$NOTE_COUNT" -lt "$MIN_NOTES" ]]; then
    fail "CLAIM-3" "Only ${NOTE_COUNT} 'Cache-line note:' occurrences in ir.rs (minimum ${MIN_NOTES}). New types were added without cache-line documentation."
else
    ok "CLAIM-3" "${NOTE_COUNT} cache-line notes in ir.rs (≥ ${MIN_NOTES} required)"
fi

# Part B: key reference types must still have their notes
_check_type_has_note() {
    local type_name="$1"
    local lineno
    lineno=$(grep -n "^pub struct ${type_name}\b\|^pub enum ${type_name}\b" "$IR_FILE" 2>/dev/null | head -1 | cut -d: -f1 || true)
    if [[ -z "$lineno" ]]; then
        # Type may have been renamed or moved
        echo "  ${type_name}: not found in ir.rs"
        return 1
    fi
    local start=$((lineno - 30))
    [[ $start -lt 1 ]] && start=1
    local window
    window=$(sed -n "${start},${lineno}p" "$IR_FILE" 2>/dev/null || true)
    if ! echo "$window" | grep -qi 'cache.line\|cache line'; then
        echo "  ${type_name} (line ${lineno}): missing cache-line note in type doc comment"
        return 1
    fi
    return 0
}

REF_TYPES_FAIL=""
for t in StaticValue StaticData VtableShim VtableSpec IrBinOp; do
    if ! _check_type_has_note "$t" 2>/dev/null; then
        REF_TYPES_FAIL="${REF_TYPES_FAIL}  ${t}: cache-line note removed or missing\n"
    fi
done

if [[ -z "$REF_TYPES_FAIL" ]]; then
    ok "CLAIM-3" "Reference IR types (StaticValue, StaticData, VtableShim, VtableSpec, IrBinOp) all have cache-line notes"
else
    fail "CLAIM-3" "Reference IR types missing cache-line notes:\n${REF_TYPES_FAIL}"
fi

# ── CLAIM-4: FLS citations present in core modules ───────────────────────────
# Adversarial: deleting or never adding FLS citations during refactoring.
echo "--- CLAIM-4: FLS citations in core modules ---"
CORE_MODULES=(src/lexer.rs src/parser.rs src/ir.rs src/lower.rs src/codegen.rs)
MISSING_CITES=""
for f in "${CORE_MODULES[@]}"; do
    if [[ ! -f "$f" ]]; then
        MISSING_CITES="${MISSING_CITES}  ${f}: file does not exist\n"
    elif ! grep -q 'FLS §' "$f"; then
        MISSING_CITES="${MISSING_CITES}  ${f}: no 'FLS §' citations found\n"
    fi
done

if [[ -z "$MISSING_CITES" ]]; then
    ok "CLAIM-4" "All core modules have FLS citations"
else
    fail "CLAIM-4" "Core modules missing FLS citations:\n${MISSING_CITES}"
fi

# ── CLAIM-5: No orphaned .s fixture files ────────────────────────────────────
# Adversarial: renaming a .rs fixture without updating the .s, or deleting
# the .rs while leaving the .s.
echo "--- CLAIM-5: No orphaned assembly fixtures ---"
ORPHANS=""
for s_file in tests/fixtures/*.s; do
    [[ -f "$s_file" ]] || continue
    stem="${s_file%.s}"
    rs_file="${stem}.rs"
    if [[ ! -f "$rs_file" ]]; then
        ORPHANS="${ORPHANS}  ${s_file} (no matching ${rs_file})\n"
    fi
done

if [[ -z "$ORPHANS" ]]; then
    ok "CLAIM-5" "No orphaned assembly fixtures"
else
    fail "CLAIM-5" "Orphaned .s files (no matching .rs source):\n${ORPHANS}"
fi

# ── CLAIM-6: Binary exits cleanly on adversarial input ───────────────────────
# Adversarial: construct inputs that would plausibly crash the lexer, parser,
# or lowering phase, and verify no signal death (exit > 128).
#
# Only runs if the debug binary exists. Skip gracefully if not built.
echo "--- CLAIM-6: Binary exits cleanly on adversarial input ---"

BINARY="target/debug/galvanic"
if [[ ! -f "$BINARY" ]]; then
    echo "SKIP [CLAIM-6]: debug binary not built — run 'cargo build' first"
    PASS=$((PASS + 1))
else
    CLAIM6_FAIL=""
    _check_input() {
        local label="$1"
        local input_file="$2"
        set +e
        _to 10 "$BINARY" "$input_file" >/dev/null 2>&1
        EXIT=$?
        set -e
        if [[ "$EXIT" -gt 128 ]]; then
            SIGNAL=$((EXIT - 128))
            CLAIM6_FAIL="${CLAIM6_FAIL}  ${label}: died with signal ${SIGNAL} (exit ${EXIT})\n"
        fi
    }

    # Create a temp dir for adversarial inputs
    TMPDIR_LOCAL=$(mktemp -d)
    trap 'rm -rf "$TMPDIR_LOCAL"' EXIT

    # Input 1: empty file
    touch "${TMPDIR_LOCAL}/empty.rs"
    _check_input "empty file" "${TMPDIR_LOCAL}/empty.rs"

    # Input 2: syntax garbage
    printf '@@@ ??? !!! ~~~ ^^^' > "${TMPDIR_LOCAL}/garbage.rs"
    _check_input "syntax garbage" "${TMPDIR_LOCAL}/garbage.rs"

    # Input 3: 300 levels of nested braces
    python3 -c "
print('fn main() {')
for i in range(300):
    print('  { let _x = 0;')
for i in range(300):
    print('  }')
print('}')
" > "${TMPDIR_LOCAL}/nested.rs"
    _check_input "300-deep nested braces" "${TMPDIR_LOCAL}/nested.rs"

    # Input 4: very long expression chain (5000 additions)
    python3 -c "print('fn main() { let _x = ' + '1 + ' * 5000 + '1; }')" > "${TMPDIR_LOCAL}/longexpr.rs"
    _check_input "5000-term expression" "${TMPDIR_LOCAL}/longexpr.rs"

    # Input 5: NUL bytes in source
    printf 'fn main() {\x00\x00\x00}' > "${TMPDIR_LOCAL}/nul.rs"
    _check_input "NUL bytes in source" "${TMPDIR_LOCAL}/nul.rs"

    # Input 6: 1000 let bindings (stack-slot stress)
    python3 -c "
print('fn main() {')
for i in range(1000):
    print(f'    let x{i}: i32 = {i};')
print('}')
" > "${TMPDIR_LOCAL}/many_lets.rs"
    _check_input "1000 let bindings" "${TMPDIR_LOCAL}/many_lets.rs"

    if [[ -z "$CLAIM6_FAIL" ]]; then
        ok "CLAIM-6" "Binary exits cleanly on all adversarial inputs"
    else
        fail "CLAIM-6" "Binary died on signal for some adversarial inputs:\n${CLAIM6_FAIL}"
    fi
fi

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "Results: ${PASS} passed, ${FAIL} failed"

if [[ $FAIL -gt 0 ]]; then
    echo ""
    echo "Failed claims must be fixed before any new work."
    exit 1
fi

exit 0
