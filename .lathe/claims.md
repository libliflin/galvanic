# Claims Registry

Load-bearing promises this project makes to its stakeholders. Each claim has an owner (the stakeholder who relies on it), a description of what it asserts, and a lifecycle note. The falsification suite (`falsify.sh`) checks these every cycle.

A failing claim is treated the same as a failing CI check: fix before any new work.

---

## Claim 1: Build succeeds

**Stakeholder**: All (William, contributors, the FLS)
**Type**: Behavioral

`cargo build` exits 0 with no errors. The compiler library and binary must compile cleanly.

If this fails, nothing else is possible. It outranks all other claims.

**Falsification check**: `cargo build -q` exit code.

**Lifecycle**: Permanent.

---

## Claim 2: Token is 8 bytes

**Stakeholder**: William (cache-aware codegen research goal)
**Type**: Structural — `size_of::<Token>() == 8`

The `Token` struct must be exactly 8 bytes. This ensures 8 tokens fit in a single 64-byte cache line, which is the concrete cache-line efficiency argument made throughout the lexer and its documentation.

This is not a documentation claim — it is checked via `std::mem::size_of::<Token>()` in a unit test. Editing comments to say "8 bytes" while the struct grows does not satisfy this claim.

**Falsification check**: `cargo test --lib -- lexer::tests::token_is_eight_bytes`

**Lifecycle**: Permanent as long as the cache-line design hypothesis is the research goal. If the design deliberately changes, update this claim with reasoning.

---

## Claim 3: All FLS parse-acceptance fixtures pass

**Stakeholder**: William (FLS coverage research), contributors (coherent test suite)
**Type**: Behavioral

Every test in `tests/fls_fixtures.rs` passes. These tests verify that galvanic's lexer and parser accept real Rust programs derived from the FLS without error.

A failing parse-acceptance test means either:
- The parser regressed on a previously-supported construct, or
- A newly added fixture exercises a construct the parser doesn't handle yet (the fixture is ahead of the parser — fix the parser or mark the fixture `#[ignore]` with a comment)

**Falsification check**: `cargo test --test fls_fixtures`

**Lifecycle**: Grows with the project. Each new FLS section covered by the parser gets a fixture and a test here.

---

## Claim 4: Non-const code emits runtime instructions

**Stakeholder**: William (core FLS compliance; research conclusions depend on this)
**Type**: Behavioral — structural check on emitted assembly

When galvanic compiles a non-const function containing arithmetic on runtime-valued operands, it must emit runtime ARM64 instructions (e.g., `add x1, x0, x2`) — not a constant-folded result (e.g., `mov x0, #3`).

This is the single most important correctness property of galvanic. A compiler that produces the right exit code by evaluating non-const code at compile time is an interpreter, not a compiler, and produces wrong evidence about the FLS.

Four adversarial cases (from weakest to strongest, reflecting the litmus test in `fls-constraints.md`):

**4a** — literal operands: `fn main() -> i32 { 1 + 2 }` must emit `add`, not `mov x0, #3`.

**4b** — runtime parameter operands: `fn add(a: i32, b: i32) -> i32 { a + b }` must emit `add` in the function body. Parameters are runtime values; no constant folding is possible. If the compiler cannot handle this case, it is an interpreter.

**4c** — runtime loop with parameter bound: `fn count(n: i32) -> i32 { let mut x = 0; while x < n { x += 1; } x }` must emit control-flow instructions (`cbz` or `b.`) rather than a folded constant. A loop with a runtime-valued bound cannot be unrolled or eliminated at compile time.

**4d** — recursive function call: a recursive `fib(n)` must emit `bl fib` instructions (runtime call to itself). If galvanic pre-computes `fib(5) == 5` at compile time and emits only `mov x0, #5`, the call is being interpreted. Recursive calls with a runtime parameter cannot be pre-computed. (FLS §6.12.1.)

**4e** — capturing closure: a closure that captures a runtime variable (`let n = 5; let f = |x| x + n; f(3)`) must emit a hidden `__closure_*` function label in the assembly. If galvanic folds the closure call to `mov x0, #8`, it is interpreting closure application. (FLS §6.14, §6.22.)

**4f** — method call dispatch: a method call on a struct (`w.get()`) must emit a `bl` instruction to the mangled method label at runtime. If galvanic inlines or pre-evaluates the method body, no `bl` appears. (FLS §6.12.2.)

**4g** — `const fn` called from a non-const context: `const fn double(n: i32) -> i32 { n * 2 }` called from `fn main()` must emit a `bl` instruction — not fold the call to `mov x0, #42`. FLS §9:41–43 permits compile-time evaluation of `const fn` only when called from a const context (const items, const blocks, array lengths, etc.). `fn main()` is not a const context. (fls-constraints.md Constraint 2.)

**4h** — `if-else` expression with a runtime condition: `fn classify(x: i32) -> i32 { if x > 0 { 1 } else { -1 } }` must emit a conditional branch instruction (`cbz`, `b.le`, `b.gt`, `cmp`, or similar) — not fold the result to `mov x0, #1` based on the call site `classify(5)`. The condition `x > 0` depends on the runtime parameter `x`; the branch cannot be eliminated at compile time. This case is distinct from claim 4c (while-loop): it tests the `if-else` lowering path specifically. (FLS §6.17; fls-constraints.md Constraint 1.)

**Falsification check**: Build galvanic, compile each case, inspect emitted `.s` file for the expected instruction class. If the binary is not built, skip (don't fail — Claim 1 covers the build).

**Lifecycle**: Permanent. This claim cannot be retired. If the project ever introduces constant-folding as an optimization pass, add a separate claim that the pass only fires in const contexts.

---

## Claim 5: Adversarial inputs exit cleanly (no panics, no hangs)

**Stakeholder**: William (research tool reliability), contributors (first impressions)
**Type**: Behavioral

The galvanic binary must not panic or hang on adversarial inputs:
- Empty file → exit 0
- Binary garbage (random bytes) → non-zero exit, no signal death (exit code ≤ 128)
- Deeply nested braces (300 levels) → any clean exit, no stack overflow (block parser recursion)
- Deeply nested parenthesized expressions (300 levels) → any clean exit, no stack overflow (expression parser recursion — separate codepath from block nesting)

A panic or signal death (exit > 128) on any of these inputs is a bug. A non-zero exit code is acceptable.

**Falsification check**: Build galvanic binary, run against adversarial inputs, check exit codes.

**Lifecycle**: Permanent. Remove specific cases if the input class becomes genuinely unsupported and documented.

---

## Claim 6: Span is 8 bytes

**Stakeholder**: William (cache-aware design research goal), contributors (architectural invariant)
**Type**: Structural — `size_of::<Span>() == 8`

The `Span` struct must be exactly 8 bytes. This is the layout note stated in the `Span` doc comment and in the architecture document: two `u32` fields, no padding. 8 bytes means a `Span` fits alongside a `Token` (also 8 bytes) in a single 64-byte cache line.

This is not a documentation claim — it is checked via `std::mem::size_of::<Span>()` in a unit test in `src/ast.rs`. Editing comments to say "8 bytes" while the struct grows does not satisfy this claim.

**Falsification check**: `cargo test --lib -- ast::tests::span_is_eight_bytes`

**Lifecycle**: Permanent as long as the cache-line design hypothesis is the research goal. If the design deliberately changes (e.g., adding a file-id field), update this claim with reasoning and adjust the layout doc.

---

## Claim 4j: match expression with runtime scrutinee emits comparison instructions

**Stakeholder**: William (FLS §6.18 compliance; research conclusions depend on correct match codegen)
**Type**: Behavioral — structural check on emitted assembly

When galvanic compiles a `match` expression whose scrutinee is a runtime-valued parameter with range-pattern arms, it must emit runtime comparison instructions (e.g., `cmp`, `cbz`, `b.lt`, `b.le`) — not constant-fold the result to the matching arm's value based on the call site.

Match expressions are the most prevalent pattern-matching construct in Rust and cover the widest set of FLS §6.18 semantics. A broken optimizer that sees `grade(85)` and folds it to `mov x0, #3` would silently produce wrong results whenever `grade` is called with any other value. The arms with range patterns (`90..=100 => 4`, `80..=89 => 3`) require runtime range comparisons that cannot be eliminated without knowing the value of `score` at compile time.

This claim is complementary to:
- **4b** (parameter arithmetic): operands in expressions
- **4c** (while-loop): branch instructions for loops
- **4h** (if-else): branch instructions for conditionals

Match is a separate lowering path from if-else and while; each must be defended independently.

**Falsification case**:
```rust
fn grade(score: i32) -> i32 {
    match score {
        90..=100 => 4,
        80..=89 => 3,
        _ => 1,
    }
}
fn main() -> i32 { grade(85) }
```
Must emit at least one `cmp` instruction (for the range boundary checks). If the compiler emits only `mov x0, #3` (the correct result for `grade(85)`), it is constant-folding a non-const function — violating fls-constraints.md Constraint 1.

**Falsification check**: Build galvanic, compile the case above, inspect the `.s` file for `cmp`.

**Lifecycle**: Permanent. Match expression lowering is load-bearing for FLS §6.18 coverage.

---

## Claim 4k: while-let with range pattern on runtime scrutinee emits comparison instructions

**Stakeholder**: William (FLS §6.15.4 compliance; research conclusions depend on correct while-let codegen)
**Type**: Behavioral — structural check on emitted assembly

When galvanic compiles a `while let` expression whose scrutinee is a runtime-valued variable and whose pattern is a range (`1..=100`), it must emit runtime comparison instructions (e.g., `cmp`, `cbz`) — not constant-fold the loop body for the call site's specific value.

`while let` (FLS §6.15.4) is a distinct lowering path from `while` (FLS §6.15.3). Claim 4c guards `while x < n`; this claim guards `while let RANGE_PATTERN = x`. Both are while-loop forms; both must emit runtime branches.

A broken optimizer seeing `count_down(5)` might emit `mov x0, #4` (the correct result for 5 steps) without emitting any `cmp` — it would be interpreting the while-let body, not compiling it.

This claim is complementary to:
- **4c** (`while x < n`): regular while loop with parameter bound
- **4h** (if-else): branch instructions for conditionals
- **4j** (match): comparison for range-pattern match arms

**Falsification case**:
```rust
fn count_down(n: i32) -> i32 {
    let mut x = n;
    let mut steps = 0;
    while let 1..=100 = x {
        steps += 1;
        x -= 1;
    }
    steps
}
fn main() -> i32 { count_down(5) }
```
Must emit at least one `cmp` instruction (for the `1..=100` range check on `x`). If the compiler emits only `mov x0, #4` (the correct result for `count_down(5)`), it is constant-folding a non-const function — violating fls-constraints.md Constraint 1.

**Falsification check**: Build galvanic, compile the case above, inspect the `.s` file for `cmp`.

**Lifecycle**: Permanent. `while let` lowering is a distinct path from `while` and must be defended independently.

---

## Retired Claims

*(none yet)*

When a claim is retired, move it here with a date and reasoning, rather than deleting it. The retirement reason is research data.
