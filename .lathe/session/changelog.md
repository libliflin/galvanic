# Changelog — Cycle 107

## Who This Helps
- **William (researcher)**: The remainder operator `%` now has adversarial falsification
  coverage. Previously `runtime_rem_emits_sdiv_and_msub` used `fn main() -> i32 { 10 % 3 }`
  — inline literals that a constant-folding interpreter could evaluate to `mov x0, #1`
  without emitting any runtime instructions. Claim 60 uses function parameters, closing
  the last gap in the five-operator arithmetic coverage story.
- **Compiler Researchers**: The arithmetic operator coverage is now symmetric:
  `add` (Claim 57), `mul` (Claim 57), `sub` (Claim 58), `sdiv` (Claim 58), `rem`
  (Claim 60). Each has a parameter-based adversarial test. The pattern is consistent
  and complete for this operator class.

## Observed
- Previous cycle's "Next" noted: "`runtime_rem_emits_sdiv_and_msub` still uses inline
  literals without function parameters and lacks a negative 'not folded' assertion."
- Claims 57 and 58 (cycles 103–104) added parameter-based tests for add, mul, sub,
  and sdiv. Remainder was skipped because it was already tested — but the existing test
  was weaker than the adversarial pattern.
- The original test: `compile_to_asm("fn main() -> i32 { 10 % 3 }\n")`. No negative
  assertion prevents a constant-folding interpreter from passing it.

## Applied
- **`tests/e2e.rs`**: Added `runtime_rem_emits_sdiv_and_msub_not_folded`:
  - Uses `fn f(x: i32, y: i32) -> i32 { x % y }` with `f(10, 3)` — parameters prevent
    constant folding.
  - Positive assertions: `sdiv` in function body, `msub` in function body, `bl` at call site.
  - Negative assertion: `!asm.contains("mov     x0, #1\n")` — the folded result of 10 % 3.
- **`.lathe/claims.md`**: Added Claim 60 entry completing the arithmetic operator story.
- **`.lathe/falsify.sh`**: Added Claim 60 check.
- **Branch**: Rebased onto `origin/main` after M174 was merged (PR #203). M174,
  Cycle 103 changelog, Claim 58, and Fix-CI commits dropped as already-upstream;
  only M175 (Claim 59) carried forward.

## Validated
- `cargo build` — clean
- `cargo test --quiet` — 1493 passed; 0 failed (1 new test)
- `cargo clippy -- -D warnings` — clean
- The new test passes: `sdiv`, `msub`, and `bl` are present; `mov x0, #1` is absent.

## FLS Notes
- **FLS §6.5.5**: The remainder operator is defined as `a % b = a - (a / b) * b`. The
  ARM64 two-instruction sequence (sdiv + msub) implements this definition exactly.
  The spec does not specify the instruction sequence — implementation choice. No ambiguity.
- **FLS §6.1.2 Constraint 1**: Function bodies are non-const contexts; remainder
  executes at runtime.

## Next
- All five basic arithmetic operators now have adversarial parameter-based falsification
  coverage (Claims 57, 58, 60).
- The next gap worth closing: additional integer types (i8, i16, u8, u16). Currently
  only i32, u32, i64, usize are fully tested. FLS §4.2 numeric types include all of these.
  Adding u8 arithmetic would be a natural next milestone, building on the existing
  byte literal infrastructure (milestones 90/92).
