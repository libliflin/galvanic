#!/usr/bin/env bash
set -euo pipefail

# macOS timeout shim
if ! command -v timeout &>/dev/null; then
  timeout() { local t=$1; shift; gtimeout "$t" "$@"; }
fi

echo "# Project Snapshot"
echo "Timestamp: $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
echo ""

# ── Git status ────────────────────────────────────────────────────────────────
echo "## Git Status"
git status --short
echo ""

# ── Recent commits ────────────────────────────────────────────────────────────
echo "## Recent Commits"
git log --oneline -10
echo ""

# ── Build ─────────────────────────────────────────────────────────────────────
echo "## Build"
BUILD_OUT=$(timeout 120 cargo build 2>&1) && BUILD_OK=true || BUILD_OK=false
if $BUILD_OK; then
  echo "OK — builds clean"
else
  echo "FAILED"
  echo '```'
  echo "$BUILD_OUT" | grep -E '^error' | head -15
  echo '```'
fi
echo ""

# ── Tests ─────────────────────────────────────────────────────────────────────
echo "## Tests"
TEST_OUT=$(timeout 120 cargo test 2>&1) || true
PASS=$(echo "$TEST_OUT" | grep 'test result:' | grep -oE '[0-9]+ passed' | grep -oE '[0-9]+' | awk '{s+=$1} END{print s+0}')
FAIL=$(echo "$TEST_OUT" | grep 'test result:' | grep -oE '[0-9]+ failed' | grep -oE '[0-9]+' | awk '{s+=$1} END{print s+0}')
IGNORE=$(echo "$TEST_OUT" | grep 'test result:' | grep -oE '[0-9]+ ignored' | grep -oE '[0-9]+' | awk '{s+=$1} END{print s+0}')
PASS=${PASS:-0}; FAIL=${FAIL:-0}; IGNORE=${IGNORE:-0}
echo "Pass: $PASS | Fail: $FAIL | Ignored: $IGNORE"
if [[ "$FAIL" -gt 0 ]]; then
  echo '```'
  echo "$TEST_OUT" | grep -A 5 'FAILED' | head -30
  echo '```'
fi
echo ""

# ── Clippy ────────────────────────────────────────────────────────────────────
echo "## Clippy"
CLIPPY_OUT=$(timeout 60 cargo clippy -- -D warnings 2>&1) && CLIPPY_OK=true || CLIPPY_OK=false
if $CLIPPY_OK; then
  echo "OK — no warnings"
else
  WARN_COUNT=$(echo "$CLIPPY_OUT" | grep -c '^error' || true)
  echo "FAILED — $WARN_COUNT error(s)"
  echo '```'
  echo "$CLIPPY_OUT" | grep -E '^error' | head -10
  echo '```'
fi
echo ""

# ── Unsafe audit ─────────────────────────────────────────────────────────────
echo "## Unsafe Audit"
UNSAFE=$(grep -rn 'unsafe[[:space:]]*{' src/ | grep -Ev ':[0-9]+:[[:space:]]*//' || true)
if [[ -z "$UNSAFE" ]]; then
  echo "OK — no unsafe blocks"
else
  echo "WARN — unsafe found:"
  echo "$UNSAFE" | head -5
fi
echo ""

# ── CI config ─────────────────────────────────────────────────────────────────
echo "## CI"
if ls .github/workflows/*.yml &>/dev/null; then
  for f in .github/workflows/*.yml; do
    echo "- $f"
  done
else
  echo "No CI config found"
fi
