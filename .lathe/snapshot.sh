#!/usr/bin/env bash
# snapshot.sh — Collect Rust project state for the lathe agent.
# Output goes to stdout as markdown. The engine captures it.

set -euo pipefail

# macOS ships without `timeout`; use gtimeout (coreutils) or a no-op fallback
if command -v timeout &>/dev/null; then
    TIMEOUT=timeout
elif command -v gtimeout &>/dev/null; then
    TIMEOUT=gtimeout
else
    TIMEOUT=""
fi
_to() { if [[ -n "$TIMEOUT" ]]; then $TIMEOUT "$@"; else shift; "$@"; fi; }

echo "# Project Snapshot"
echo "Generated: $(date)"
echo ""

# Git state
echo "## Git Status"
git status --short 2>/dev/null || echo "(not a git repo)"
echo ""

echo "## Recent Commits (last 10)"
git log --oneline -10 2>/dev/null || echo "(no commits)"
echo ""

# Cargo metadata
echo "## Cargo.toml"
if [[ -f Cargo.toml ]]; then
    head -10 Cargo.toml
else
    echo "(no Cargo.toml found)"
fi
echo ""

# Build
echo "## Build"
echo '```'
_to 30 cargo build 2>&1 && echo "OK — builds clean" || true
echo '```'
echo ""

# Tests
echo "## Tests"
echo '```'
_to 60 cargo test 2>&1 || true
echo '```'
echo ""

# Clippy (if available)
echo "## Clippy"
echo '```'
if cargo clippy --version &>/dev/null 2>&1; then
    _to 30 cargo clippy -- -D warnings 2>&1 || true
else
    echo "(clippy not installed)"
fi
echo '```'
echo ""

# File overview
echo "## Rust Files"
find . -name '*.rs' -not -path './target/*' -not -path './.lathe/*' | sort | head -50
echo ""

# TODOs and FIXMEs
echo "## TODOs"
grep -rn 'TODO\|FIXME\|HACK\|XXX' --include='*.rs' . 2>/dev/null | head -20 || echo "(none)"
echo ""

# Test count
echo "## Test Count"
grep -rn '#\[test\]' --include='*.rs' . 2>/dev/null | wc -l | xargs -I{} echo "{} test functions"
echo ""
