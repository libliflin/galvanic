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
# Three adversarial cases (weakest → strongest):
#   4a: literal operands — fn main() { 1 + 2 } must emit 'add', not 'mov x0,#3'
#   4b: runtime parameter operands — fn add(a, b) { a + b } must emit 'add'
#       (the litmus test from fls-constraints.md: replacing a literal with a
#        parameter must not break the implementation)
#   4c: runtime loop with parameter bound — must emit branch instructions, not
#       a constant result
# If the binary isn't built, we skip (Claim 1 covers the build).

echo "--- Claim 4: non-const code emits runtime instructions"
GALVANIC="./target/debug/galvanic"
if [[ ! -x "$GALVANIC" ]]; then
    claim_fail "non-const runtime codegen" "galvanic binary not found at $GALVANIC (did Claim 1 pass?)"
else
    # Helper: compile a source string, check the .s file for a pattern.
    # Returns 0 if found, 1 if not found, 2 if compile failed.
    check_asm_contains() {
        local src="$1" pattern="$2"
        local tmp_rs tmp_s
        tmp_rs=$(mktemp /tmp/galvanic_falsify_XXXXXX.rs)
        tmp_s="${tmp_rs%.rs}.s"
        printf '%s\n' "$src" > "$tmp_rs"
        local rc=2
        if "$GALVANIC" "$tmp_rs" > /dev/null 2>&1 && [[ -f "$tmp_s" ]]; then
            set +o pipefail
            local found
            found=$(grep -cE "$pattern" "$tmp_s" 2>/dev/null || echo 0)
            set -o pipefail
            [[ "$found" -gt 0 ]] && rc=0 || rc=1
            rm -f "$tmp_s"
        fi
        rm -f "$tmp_rs"
        return $rc
    }

    # 4a: literal operands — must emit 'add'
    SRC_4A='fn main() -> i32 { 1 + 2 }'
    if check_asm_contains "$SRC_4A" '\badd\b'; then
        claim_ok "fn main() { 1 + 2 } emits 'add' instruction (not constant-folded)"
    else
        claim_fail "4a non-const literal arithmetic" \
            "no 'add' instruction for '1 + 2' — galvanic may be constant-folding non-const code"
    fi

    # 4b: runtime parameter operands — parameters are runtime values, cannot fold.
    # The litmus test: if a literal is replaced by a parameter, the codegen must still
    # emit runtime arithmetic. (FLS §6.1.2 / fls-constraints.md Constraint 1)
    SRC_4B='fn add(a: i32, b: i32) -> i32 { a + b }
fn main() -> i32 { add(1, 2) }'
    if check_asm_contains "$SRC_4B" '\badd\b'; then
        claim_ok "fn add(a, b) { a + b } emits 'add' for runtime parameters (not interpreter)"
    else
        claim_fail "4b runtime parameter arithmetic" \
            "no 'add' instruction for 'a + b' with parameters — galvanic may be interpreting, not compiling"
    fi

    # 4c: runtime loop with parameter bound — a while loop that iterates n times
    # must emit branch instructions. If galvanic constant-folds the loop away and
    # emits just 'mov x0, #<result>', this claim fails.
    SRC_4C='fn count(n: i32) -> i32 {
    let mut x = 0;
    while x < n { x += 1; }
    x
}
fn main() -> i32 { count(5) }'
    if check_asm_contains "$SRC_4C" '(cbz|cbnz|b\.eq|b\.ne|b\.lt|b\.gt|b\.le|b\.ge|\bb\b)'; then
        claim_ok "fn count(n) while-loop emits branch instructions (not constant-folded iteration)"
    else
        claim_fail "4c runtime loop with parameter bound" \
            "no branch instructions for while loop over runtime parameter — loop may be eliminated at compile time"
    fi

    # 4d: recursive function calls — a recursive function must emit a runtime `bl`
    # to itself. If galvanic pre-computes fib(5) at compile time and emits only
    # `mov x0, #5`, this claim fails.
    # (FLS §6.12.1: call expressions invoke functions at runtime; fls-constraints.md
    # Constraint 1: non-const functions must not be evaluated at compile time.)
    SRC_4D='fn fib(n: i32) -> i32 {
    if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
}
fn main() -> i32 { fib(5) }'
    if check_asm_contains "$SRC_4D" '\bbl\s+fib\b'; then
        claim_ok "recursive fib(n) emits 'bl fib' instruction (not pre-computed)"
    else
        claim_fail "4d recursive call emits runtime bl" \
            "no 'bl fib' instruction in recursive fib — call may be pre-computed or inlined"
    fi

    # 4e: capturing closure — a closure that captures a runtime variable must
    # emit a hidden `__closure_*` function label in the assembly. If galvanic
    # folds the closure call away (treating it as an interpreter), no closure
    # label would appear — the result would be `mov x0, #8` instead.
    # (FLS §6.14: closure expressions; FLS §6.22: variable capturing;
    #  fls-constraints.md Constraint 1: non-const code must emit runtime instructions.)
    SRC_4E='fn main() -> i32 {
    let n = 5;
    let add_n = |x: i32| x + n;
    add_n(3)
}'
    if check_asm_contains "$SRC_4E" '__closure_'; then
        claim_ok "capturing closure emits '__closure_*' label (not constant-folded)"
    else
        claim_fail "4e capturing closure runtime emission" \
            "no '__closure_*' label for capturing closure — closure may be constant-folded or inlined"
    fi

    # 4f: method call emits a mangled `bl` instruction — a method on a struct
    # must dispatch via a named label at runtime, not be inlined or constant-folded.
    # If galvanic evaluated `w.get()` at compile time and emitted `mov x0, #42`,
    # no `bl` instruction would appear.
    # (FLS §6.12.2: method call expressions; fls-constraints.md Constraint 1.)
    SRC_4F='struct Wrap { val: i32 }
impl Wrap { fn get(&self) -> i32 { self.val } }
fn main() -> i32 {
    let w = Wrap { val: 42 };
    w.get()
}'
    if check_asm_contains "$SRC_4F" '\bbl\b'; then
        claim_ok "method call on struct emits 'bl' instruction (not constant-folded)"
    else
        claim_fail "4f method call emits runtime bl" \
            "no 'bl' instruction for w.get() — method call may be constant-folded or inlined"
    fi

    # 4g: const fn called from a non-const context must emit a runtime `bl` call —
    # not be evaluated at compile time. FLS §9:41–43 (fls-constraints.md Constraint 2)
    # permits const fn to be evaluated at compile time ONLY when called from a const
    # context (const item, const block, array length, etc.). A call from fn main() is
    # NOT a const context; the result must be produced at runtime via a bl instruction.
    #
    # Regression scenario: a future "const fn optimization" that forgets the context
    # check and folds all const fn calls, not just those in const contexts.
    SRC_4G='const fn double(n: i32) -> i32 { n * 2 }
fn main() -> i32 { double(21) }'
    if check_asm_contains "$SRC_4G" '\bbl\b'; then
        claim_ok "const fn called from non-const context emits 'bl' (not folded — FLS §9:41-43)"
    else
        claim_fail "4g const fn at runtime emits bl" \
            "no 'bl' for const fn called from main() — const fn may be folded outside const context (violates FLS §9:41-43)"
    fi

    # 4h: if-else expression with a runtime condition must emit a conditional
    # branch instruction — not be constant-folded to the taken branch's value.
    # The condition `x > 0` depends on the runtime parameter `x`; the compiler
    # cannot eliminate the branch at compile time.
    #
    # This case is distinct from claim 4c (while-loop): it specifically guards
    # the if-else lowering path (FLS §6.17). A broken optimizer that sees
    # `classify(5)` might fold the entire call to `mov x0, #1` because 5 > 0.
    # That would be wrong — `x` could be any value at the call site in general.
    # (FLS §6.17; fls-constraints.md Constraint 1.)
    SRC_4H='fn classify(x: i32) -> i32 {
    if x > 0 { 1 } else { -1 }
}
fn main() -> i32 { classify(5) }'
    if check_asm_contains "$SRC_4H" '(cbz|cbnz|b\.eq|b\.ne|b\.lt|b\.gt|b\.le|b\.ge|b\.lo|b\.hi|b\.ls|b\.hs|\bcmp\b)'; then
        claim_ok "if-else with runtime condition emits branch instruction (not constant-folded — FLS §6.17)"
    else
        claim_fail "4h if-else runtime branch" \
            "no branch instruction for 'if x > 0' — if-else may be constant-folded (violates FLS §6.17)"
    fi

    # 4i: named block break value from a runtime parameter must emit runtime
    # instructions — specifically an `add` for `n + 1` where `n` is a parameter.
    # If galvanic constant-folds the named block at compile time (seeing that
    # `named_block_compute(4)` is called with a literal), it would emit
    # `mov x0, #5` with no `add` or branch — that is an interpreter, not a compiler.
    #
    # The named block's conditional break means the runtime control flow depends
    # on `n`; replacing 4 with any parameter value must produce correct runtime
    # behaviour. (FLS §6.4.3: named block expressions; FLS §6.15.6: break with
    # value; fls-constraints.md Constraint 1: non-const code must not be folded.)
    SRC_4I='fn named_block_compute(n: i32) -> i32 {
    '"'"'result: {
        if n > 0 { break '"'"'result n + 1; }
        break '"'"'result 0;
    }
}
fn main() -> i32 { named_block_compute(4) }'
    if check_asm_contains "$SRC_4I" '\badd\b'; then
        claim_ok "named block break value from runtime param emits 'add' (not constant-folded — FLS §6.4.3)"
    else
        claim_fail "4i named block runtime break value" \
            "no 'add' for named block break value 'n + 1' — named block may be constant-folded (violates FLS §6.4.3)"
    fi

    # 4j: match expression with range-pattern arms on a runtime parameter must
    # emit comparison instructions — not be constant-folded to the matched arm's
    # value based on the call site.
    #
    # A broken optimizer seeing `grade(85)` might emit `mov x0, #3` (the result
    # for 85 in the 80..=89 arm) without emitting any `cmp`. That would be wrong
    # whenever `grade` is called with a different value, and constitutes
    # constant-folding a non-const function. Range-pattern match arms require
    # runtime boundary comparisons (`cmp` + conditional branches) that cannot be
    # eliminated unless the scrutinee value is statically known.
    # (FLS §6.18: match expressions; §5.1.9: range patterns;
    #  fls-constraints.md Constraint 1: non-const code must not be folded.)
    SRC_4J='fn grade(score: i32) -> i32 {
    match score {
        90..=100 => 4,
        80..=89 => 3,
        _ => 1,
    }
}
fn main() -> i32 { grade(85) }'
    if check_asm_contains "$SRC_4J" '\bcmp\b'; then
        claim_ok "match with range-pattern arms on runtime param emits 'cmp' (not constant-folded — FLS §6.18)"
    else
        claim_fail "4j match expression runtime comparison" \
            "no 'cmp' for 'match score { 90..=100 => 4, ... }' — match may be constant-folded (violates FLS §6.18)"
    fi

    # 4k: while-let with a range pattern on a runtime scrutinee must emit
    # comparison instructions — not be constant-folded to the loop result
    # for the specific call-site argument.
    #
    # `while let` (FLS §6.15.4) is a distinct lowering path from `while`
    # (claim 4c guards `while x < n`). A broken optimizer seeing
    # `count_down(5)` might emit `mov x0, #4` without any `cmp` — treating
    # the loop as an interpreter over the call-site value. The range check
    # `1..=100` on the runtime variable `x` requires two boundary comparisons
    # at runtime that cannot be eliminated without knowing `x` statically.
    # (FLS §6.15.4: while-let expressions; §5.1.9: range patterns;
    #  fls-constraints.md Constraint 1: non-const code must not be folded.)
    SRC_4K='fn count_down(n: i32) -> i32 {
    let mut x = n;
    let mut steps = 0;
    while let 1..=100 = x {
        steps += 1;
        x -= 1;
    }
    steps
}
fn main() -> i32 { count_down(5) }'
    if check_asm_contains "$SRC_4K" '\bcmp\b'; then
        claim_ok "while-let with range pattern on runtime scrutinee emits 'cmp' (not constant-folded — FLS §6.15.4)"
    else
        claim_fail "4k while-let runtime comparison" \
            "no 'cmp' for 'while let 1..=100 = x' on runtime param — while-let may be constant-folded (violates FLS §6.15.4)"
    fi
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

    # 5d: deeply nested parenthesized expressions → clean exit.
    # Block nesting and expression nesting recurse through different parser paths.
    # 300 levels of `(((1)))` tests the expression parser's recursion depth
    # independently of the block parser fix from Claim 5c.
    TMP_DEEP_EXPR=$(mktemp /tmp/galvanic_falsify_deep_expr_XXXXXX.rs)
    {
        printf 'fn main() -> i32 {\n    '
        printf '%0.s(' $(seq 1 300)
        printf '1'
        printf '%0.s)' $(seq 1 300)
        printf '\n}\n'
    } > "$TMP_DEEP_EXPR"
    set +o pipefail
    "$GALVANIC" "$TMP_DEEP_EXPR" > /dev/null 2>&1
    DEEP_EXPR_EXIT=$?
    set -o pipefail
    rm -f "$TMP_DEEP_EXPR"
    if [[ "$DEEP_EXPR_EXIT" -gt 128 ]]; then
        claim_fail "adversarial: 300-deep nested paren expressions" "exited with signal (code $DEEP_EXPR_EXIT)"
        ADVERSARIAL_FAIL=1
    fi

    if [[ "$ADVERSARIAL_FAIL" -eq 0 ]]; then
        claim_ok "empty file, binary garbage, deep brace nesting, and deep paren nesting all exit cleanly"
    fi
fi

# ── Claim 6: Span is 8 bytes ─────────────────────────────────────────────────

echo "--- Claim 6: Span is 8 bytes"
if "$CARGO" test --lib -q -- ast::tests::span_is_eight_bytes 2>/dev/null; then
    claim_ok "size_of::<Span>() == 8"
else
    claim_fail "Span is 8 bytes" "ast::tests::span_is_eight_bytes failed — Span struct grew beyond 8 bytes"
fi

# ── Summary ───────────────────────────────────────────────────────────────────

echo ""
echo "=== Summary === passed: $PASS failed: $FAIL"

if [[ "$FAIL" -gt 0 ]]; then
    exit 1
fi
