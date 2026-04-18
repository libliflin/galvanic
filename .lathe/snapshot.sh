#!/usr/bin/env bash
set -euo pipefail

# macOS-compatible timeout wrapper
_timeout() {
  local secs=$1; shift
  if command -v gtimeout &>/dev/null; then
    gtimeout "$secs" "$@"
  elif command -v timeout &>/dev/null; then
    timeout "$secs" "$@"
  else
    "$@"
  fi
}

echo "# Project Snapshot"
echo "Timestamp: $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
echo

# ── Git status ───────────────────────────────────────────────────────────────
echo "## Git Status"
git status --short
echo

# ── Recent commits ───────────────────────────────────────────────────────────
echo "## Recent Commits"
git log --oneline -10
echo

# ── Build ────────────────────────────────────────────────────────────────────
echo "## Build"
BUILD_OUT=$(_timeout 120 cargo build 2>&1) && BUILD_OK=true || BUILD_OK=false
if $BUILD_OK; then
  echo "OK — builds clean"
else
  echo "FAILED"
  echo "$BUILD_OUT" | grep -E '^error' | head -10
fi
echo

# ── Tests ────────────────────────────────────────────────────────────────────
echo "## Tests"
TEST_OUT=$(_timeout 120 cargo test 2>&1) || true
PASS=$(echo "$TEST_OUT" | grep -c '^test .* ok$' || true)
FAIL=$(echo "$TEST_OUT" | grep -c '^test .* FAILED$' || true)
IGNORED=$(echo "$TEST_OUT" | grep -c '^test .* ignored$' || true)
echo "Pass: $PASS | Fail: $FAIL | Ignored: $IGNORED"
if [ "$FAIL" -gt 0 ]; then
  echo
  echo "### Failures"
  echo "$TEST_OUT" | grep -A3 'FAILED\|failures:' | head -20
fi
echo

# ── Clippy ───────────────────────────────────────────────────────────────────
echo "## Clippy"
CLIPPY_OUT=$(_timeout 120 cargo clippy -- -D warnings 2>&1) && CLIPPY_OK=true || CLIPPY_OK=false
if $CLIPPY_OK; then
  echo "OK — no warnings"
else
  WARN_COUNT=$(echo "$CLIPPY_OUT" | grep -c '^error\[' || true)
  echo "FAILED — $WARN_COUNT error(s)"
  echo "$CLIPPY_OUT" | grep '^error\[' | head -5
fi
echo

# ── CI config ────────────────────────────────────────────────────────────────
echo "## CI"
if [ -f .github/workflows/ci.yml ]; then
  JOBS=$(grep '^  [a-z]' .github/workflows/ci.yml | awk '{print $1}' | tr -d ':' | tr '\n' ' ')
  echo "GitHub Actions: .github/workflows/ci.yml"
  echo "Jobs: $JOBS"
else
  echo "No CI config found"
fi
echo

# ── Unsafe / audit signals ────────────────────────────────────────────────────
echo "## Audit"
UNSAFE=$(grep -rn 'unsafe\s*{\|unsafe\s*fn\b\|unsafe\s*impl\b' src/ | grep -Ev ':[0-9]+:[[:space:]]*//' || true)
if [ -z "$UNSAFE" ]; then
  echo "No unsafe code — clean"
else
  echo "WARN: unsafe blocks found:"
  echo "$UNSAFE" | head -5
fi
