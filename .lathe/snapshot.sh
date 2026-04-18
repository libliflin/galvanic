#!/usr/bin/env bash
set -euo pipefail

# macOS-compatible timeout wrapper
if command -v gtimeout &>/dev/null; then
  TO=gtimeout
elif command -v timeout &>/dev/null; then
  TO=timeout
else
  TO="env"  # no-op fallback: just run without timeout
fi

echo "# Project Snapshot"
echo "Timestamp: $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
echo

# ── Git status ────────────────────────────────────────────────────────────────
echo "## Git Status"
git status --short
echo

# ── Recent commits ────────────────────────────────────────────────────────────
echo "## Recent Commits"
git log --oneline -10
echo

# ── Build ─────────────────────────────────────────────────────────────────────
echo "## Build"
BUILD_OUT=$($TO 60 cargo build 2>&1) && BUILD_OK=true || BUILD_OK=false
if $BUILD_OK; then
  echo "OK — builds clean"
else
  echo "FAILED"
  echo "$BUILD_OUT" | grep -E '^error' | head -10
fi
echo

# ── Tests ─────────────────────────────────────────────────────────────────────
echo "## Tests"
TEST_OUT=$($TO 120 cargo test -- --color never 2>&1) || true

# Sum pass/fail across all test binaries (format: "N passed; N failed; N ignored;")
PASSED=$(echo "$TEST_OUT" | grep -E '^test result:' | grep -oE '[0-9]+ passed' | awk '{s+=$1} END{print s+0}')
FAILED=$(echo "$TEST_OUT" | grep -E '^test result:' | grep -oE '[0-9]+ failed' | awk '{s+=$1} END{print s+0}')
IGNORED=$(echo "$TEST_OUT" | grep -E '^test result:' | grep -oE '[0-9]+ ignored' | awk '{s+=$1} END{print s+0}')

echo "Pass: $PASSED | Fail: $FAILED | Ignored: $IGNORED"

if [ "$FAILED" -gt 0 ]; then
  echo "### Failures"
  echo "$TEST_OUT" | grep -E '^(FAILED|test .* FAILED)' | head -10
  echo "$TEST_OUT" | grep -A5 'FAILED' | head -30
fi
echo

# ── Clippy ────────────────────────────────────────────────────────────────────
echo "## Clippy"
CLIPPY_OUT=$($TO 60 cargo clippy -- -D warnings 2>&1) && CLIPPY_OK=true || CLIPPY_OK=false
if $CLIPPY_OK; then
  echo "OK"
else
  WARN_COUNT=$(echo "$CLIPPY_OUT" | grep -c '^error' || true)
  echo "FAILED — $WARN_COUNT error(s)"
  echo "$CLIPPY_OUT" | grep -E '^error' | head -8
fi
echo

# ── Unsafe audit ──────────────────────────────────────────────────────────────
echo "## Unsafe Audit"
UNSAFE=$(grep -rn 'unsafe\s*{\|unsafe\s*fn\b\|unsafe\s*impl\b' src/ | grep -Ev ':[0-9]+:[[:space:]]*//' || true)
if [[ -z "$UNSAFE" ]]; then
  echo "OK — no unsafe blocks"
else
  echo "WARNING — unsafe code detected:"
  echo "$UNSAFE" | head -5
fi
echo

# ── CI config ─────────────────────────────────────────────────────────────────
echo "## CI"
if ls .github/workflows/*.yml &>/dev/null 2>&1; then
  echo "Workflows: $(ls .github/workflows/*.yml | xargs -n1 basename | tr '\n' ' ')"
else
  echo "No CI workflows found"
fi
