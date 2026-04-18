#!/usr/bin/env bash
set -euo pipefail

# macOS compatibility: prefer gtimeout (coreutils) over timeout
if command -v gtimeout &>/dev/null; then
  TO=gtimeout
elif command -v timeout &>/dev/null; then
  TO=timeout
else
  TO=""
fi
run() { ${TO:+$TO "$1"} "${@:2}"; }

echo "# Project Snapshot"
echo "Generated: $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
echo

# --- Git status ---
echo "## Git Status"
git status --short
echo

# --- Recent commits ---
echo "## Recent Commits"
git log --oneline -10
echo

# --- Build ---
echo "## Build"
BUILD_OUT=$(run 120 cargo build 2>&1) && BUILD_OK=true || BUILD_OK=false
if $BUILD_OK; then
  echo "OK — builds clean"
else
  echo "FAIL"
  echo '```'
  echo "$BUILD_OUT" | grep -E '^error' | head -10
  echo '```'
fi
echo

# --- Tests ---
echo "## Tests"
TEST_OUT=$(run 120 cargo test 2>&1) && TEST_OK=true || TEST_OK=false
PASSED=$(echo "$TEST_OUT" | grep -E '^test .+ \.\.\. ok$' | wc -l | tr -d ' ')
FAILED=$(echo "$TEST_OUT" | grep -E '^test .+ \.\.\. FAILED$' | wc -l | tr -d ' ')
IGNORED=$(echo "$TEST_OUT" | grep -E '^test .+ \.\.\. ignored$' | wc -l | tr -d ' ')
echo "Pass: $PASSED | Fail: $FAILED | Ignored: $IGNORED"
if ! $TEST_OK; then
  echo '```'
  echo "$TEST_OUT" | grep -E '^(test .+ \.\.\. FAILED|FAILED|thread .+ panicked|error\[)' | head -15
  echo '```'
fi
echo

# --- Clippy ---
echo "## Clippy"
CLIPPY_OUT=$(run 120 cargo clippy -- -D warnings 2>&1) && CLIPPY_OK=true || CLIPPY_OK=false
if $CLIPPY_OK; then
  echo "OK — no warnings"
else
  WARN_COUNT=$(echo "$CLIPPY_OUT" | grep -c '^error' || true)
  echo "FAIL — $WARN_COUNT error(s)"
  echo '```'
  echo "$CLIPPY_OUT" | grep -E '^error' | head -10
  echo '```'
fi
echo

# --- Audit (unsafe / Command / network deps) ---
echo "## Audit"
UNSAFE=$(grep -rn 'unsafe\s*{\|unsafe\s*fn\b\|unsafe\s*impl\b' src/ | grep -Ev ':[0-9]+:[[:space:]]*//' || true)
CMD_LEAK=$(grep -rn 'std::process::Command\|process::Command' src/ | grep -v '^src/main\.rs:' || true)
NET_DEPS=$(grep -E '^(reqwest|hyper|tokio|async-std|surf)\b' Cargo.toml || true)
AUDIT_CLEAN=true
if [[ -n "$UNSAFE" ]]; then
  echo "FAIL: unsafe code in src/"
  echo "$UNSAFE" | head -5
  AUDIT_CLEAN=false
fi
if [[ -n "$CMD_LEAK" ]]; then
  echo "FAIL: Command usage outside main.rs"
  echo "$CMD_LEAK" | head -5
  AUDIT_CLEAN=false
fi
if [[ -n "$NET_DEPS" ]]; then
  echo "FAIL: network crate in Cargo.toml"
  echo "$NET_DEPS"
  AUDIT_CLEAN=false
fi
if $AUDIT_CLEAN; then
  echo "OK — no unsafe, no Command leak, no network deps"
fi
echo

# --- CI ---
echo "## CI"
if [[ -d .github/workflows ]]; then
  ls .github/workflows/*.yml 2>/dev/null | xargs -I{} basename {} | sed 's/^/- /' || true
else
  echo "No CI config found"
fi
echo

# --- FLS fixture coverage ---
echo "## FLS Fixtures"
FIXTURE_COUNT=$(ls tests/fixtures/fls_*.rs 2>/dev/null | wc -l | tr -d ' ')
echo "$FIXTURE_COUNT fixture files in tests/fixtures/"
