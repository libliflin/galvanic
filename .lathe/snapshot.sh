#!/usr/bin/env bash
set -euo pipefail

# macOS: 'timeout' lives in coreutils as 'gtimeout'
if command -v gtimeout &>/dev/null; then
  TIMEOUT=gtimeout
elif command -v timeout &>/dev/null; then
  TIMEOUT=timeout
else
  TIMEOUT=""
fi

run() {
  local secs=$1; shift
  if [[ -n "$TIMEOUT" ]]; then
    $TIMEOUT "$secs" "$@"
  else
    "$@"
  fi
}

echo "# Project Snapshot"
echo "Generated: $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
echo ""

# --- Git Status ---
echo "## Git Status"
git status --short
echo ""

# --- Recent Commits ---
echo "## Recent Commits"
git log --oneline -10
echo ""

# --- Build ---
echo "## Build"
BUILD_OUT=$(run 120 cargo build 2>&1) && BUILD_OK=true || BUILD_OK=false
if $BUILD_OK; then
  echo "OK — builds clean"
else
  echo "FAILED"
  echo "\`\`\`"
  echo "$BUILD_OUT" | grep -E '^error' | head -10
  echo "\`\`\`"
fi
echo ""

# --- Tests ---
echo "## Tests"
TEST_OUT=$(run 120 cargo test 2>&1) || true
PASS=$(echo "$TEST_OUT" | grep -c '^test .* ok$' || true)
FAIL=$(echo "$TEST_OUT" | grep -c '^test .* FAILED$' || true)
IGNORED=$(echo "$TEST_OUT" | grep -c '^test .* ignored$' || true)
echo "Pass: $PASS | Fail: $FAIL | Ignored: $IGNORED"
if [[ "$FAIL" -gt 0 ]]; then
  echo ""
  echo "Failed tests:"
  echo "\`\`\`"
  echo "$TEST_OUT" | grep '^test .* FAILED$' | head -10
  echo ""
  echo "$TEST_OUT" | grep -A 10 'FAILED$' | head -30
  echo "\`\`\`"
fi
echo ""

# --- Clippy ---
echo "## Clippy"
CLIPPY_OUT=$(run 120 cargo clippy -- -D warnings 2>&1) && CLIPPY_OK=true || CLIPPY_OK=false
if $CLIPPY_OK; then
  echo "OK"
else
  WARN_COUNT=$(echo "$CLIPPY_OUT" | grep -c '^error' || true)
  echo "FAILED — $WARN_COUNT error(s)"
  echo "\`\`\`"
  echo "$CLIPPY_OUT" | grep -E '^error' | head -8
  echo "\`\`\`"
fi
echo ""

# --- Unsafe Audit ---
echo "## Unsafe Audit"
UNSAFE=$(grep -rn 'unsafe\s*{\|unsafe\s*fn\b\|unsafe\s*impl\b' src/ | grep -Ev ':[0-9]+:[[:space:]]*//' || true)
if [[ -z "$UNSAFE" ]]; then
  echo "OK — no unsafe blocks in src/"
else
  UNSAFE_COUNT=$(echo "$UNSAFE" | wc -l | tr -d ' ')
  echo "WARNING — $UNSAFE_COUNT unsafe occurrence(s)"
  echo "\`\`\`"
  echo "$UNSAFE" | head -5
  echo "\`\`\`"
fi
echo ""

# --- FLS Coverage ---
echo "## FLS Coverage"
FIXTURES_DIR="tests/fixtures"
if [[ -d "$FIXTURES_DIR" ]]; then
  TOTAL_FIXTURES=$(find "$FIXTURES_DIR" -name '*.rs' | wc -l | tr -d ' ')
  # Fixtures with a corresponding .s file have been compiled end-to-end
  COMPILED_FIXTURES=$(find "$FIXTURES_DIR" -name '*.rs' | while read f; do
    base="${f%.rs}"
    [[ -f "${base}.s" ]] && echo "$f"
  done | wc -l | tr -d ' ')
  PARSE_ONLY=$((TOTAL_FIXTURES - COMPILED_FIXTURES))
  echo "Fixtures: $TOTAL_FIXTURES total | $COMPILED_FIXTURES compiled end-to-end | $PARSE_ONLY parse-only (next targets)"
  if [[ "$PARSE_ONLY" -gt 0 ]]; then
    echo "Parse-only fixtures (candidate goals):"
    find "$FIXTURES_DIR" -name '*.rs' | while read f; do
      base="${f%.rs}"
      [[ ! -f "${base}.s" ]] && echo "  - $(basename $f)"
    done | head -10
  fi
else
  echo "No fixtures directory found"
fi
echo ""

# --- Ambiguity Tracking ---
echo "## FLS Ambiguity Tracking"
AMBIG_IN_SOURCE=$(grep -rn 'AMBIGUOUS' src/ 2>/dev/null | grep -c '§' || true)
AMBIG_IN_REF=0
if [[ -f "refs/fls-ambiguities.md" ]]; then
  AMBIG_IN_REF=$(grep -c '^## §' refs/fls-ambiguities.md 2>/dev/null || true)
fi
echo "AMBIGUOUS annotations in source: $AMBIG_IN_SOURCE | Documented entries in refs/fls-ambiguities.md: $AMBIG_IN_REF"

# Check which unique section numbers from source annotations are missing from refs.
# (Compare unique sections, not raw line count — many annotations share a section.)
if [[ -f "refs/fls-ambiguities.md" ]]; then
  MISSING=$(grep -rn 'AMBIGUOUS' src/ 2>/dev/null \
    | grep -oE '§[0-9]+(\.[0-9]+)?' \
    | sort -u \
    | while read sec; do
        grep -qE "^## ${sec}( |—|-)" refs/fls-ambiguities.md || echo "  $sec"
      done)
  if [[ -n "$MISSING" ]]; then
    echo "WARNING: sections annotated in source but missing from refs/fls-ambiguities.md:"
    echo "$MISSING"
  else
    echo "OK — all annotated sections have entries in refs/fls-ambiguities.md"
  fi
fi
echo ""

# --- CI ---
echo "## CI"
if [[ -f .github/workflows/ci.yml ]]; then
  JOBS=$(grep '^  [a-z]' .github/workflows/ci.yml | sed 's/://' | tr -d ' ' | tr '\n' ' ')
  echo "Jobs: $JOBS"
else
  echo "No CI config found"
fi
echo ""
