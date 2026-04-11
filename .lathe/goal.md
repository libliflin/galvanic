# Goal: §6.23 Divide-by-Zero — Compile-Time Literal Divisor Check

## What

Add a compile-time divide-by-zero guard to galvanic's lowering pass
(`src/lower.rs`) so that a literal zero divisor in `/` or `%` produces a
diagnostic error rather than silently emitting `sdiv`/`udiv` with undefined
behavior.

The builder should:

1. In the arithmetic lowering path for `/` and `%` in `src/lower.rs`, after
   evaluating the right-hand operand, check if it is a compile-time-known
   literal zero. If so, return a `LowerError` with a clear message:
   `"divide by zero: divisor is the literal 0"`.

   - The check applies to integer division (`i32`, `u32`, `i64`, `u64`, `u8`,
     `i8`, `u16`, `i16`, `usize`, `isize`) and remainder (`%`).
   - The check does NOT apply to float division (IEEE 754 defines that
     behavior as producing infinity/NaN).
   - The check does NOT need to handle the `MIN / -1` case (that requires
     runtime knowledge of the dividend). False negatives (missing a runtime
     zero divisor) are acceptable. False positives (rejecting a valid program)
     are not acceptable.
   - If the divisor is a variable (not a literal), no check is needed — let
     the codegen emit `sdiv`/`udiv` as before.

2. Add at least two test cases to `tests/e2e.rs`:
   - A test that verifies galvanic *rejects* `fn main() -> i32 { 10 / 0 }` —
     the divisor is a literal zero, so lowering should return `Err`.
   - A test that verifies galvanic *rejects* `fn main() -> i32 { 10 % 0 }` —
     same guard for remainder.
   - A test that verifies galvanic *accepts* `fn div(x: i32, y: i32) -> i32 { x / y }` —
     variable divisor is not rejected at compile time.
   - Optionally: a test that verifies galvanic accepts `fn main() -> i32 { 10 / 2 }` —
     non-zero literal is not rejected.

3. Update the §6.9/§6.23 entry in `refs/fls-ambiguities.md`:
   - Change "Divide-by-zero: `sdiv`/`udiv` emit no guard; behavior is undefined"
     to document that literal-zero divisors are now caught at compile time.
   - Document what remains deferred: runtime zero divisors, `MIN / -1` overflow,
     out-of-bounds indexing, and integer overflow panics all still require a
     panic infrastructure that does not yet exist.
   - Add the FLS gap: §6.23 requires a panic for divide-by-zero but does not
     specify the mechanism. Galvanic's conservative choice: reject literal zero
     at compile time; runtime zero is undefined behavior at this milestone.

The check does NOT need to be complete. Only the literal-zero case is required.

## Which stakeholder

**William (the researcher)** and **FLS spec readers (the Ferrocene team)**.

The §6.9/§6.23 entry in `refs/fls-ambiguities.md` currently reads:
> "Divide-by-zero: `sdiv`/`udiv` emit no guard; behavior is undefined on ARM64."

This is the most prominent remaining *silent UB* gap that can be partially
closed without a panic runtime. The §6.18 exhaustiveness check established the
pattern: conservative compile-time rejection of the obvious case, false negatives
acceptable, false positives not. Applying the same pattern here closes the most
obvious division-by-zero case.

For spec readers: the FLS says divide-by-zero must panic (§6.23) but provides
no mechanism. The ambiguities doc should document galvanic's chosen heuristic
(same as exhaustiveness: catch the literal case, defer the runtime case).

For William: this is the second compile-time correctness gap closed without
panic infrastructure. Together with §6.18, they form a pattern: galvanic has
a systematic approach to catching the obvious case at compile time even when
full runtime enforcement is deferred.

## Why now

The §6.18 exhaustiveness check (Claim 4l) just landed in the prior session
(commits #262–264). The build is clean at 1992 tests, all passing.

The §6.23 gap is structurally identical to the §6.18 gap:
- The FLS requires something (exhaustiveness / no div-by-zero)
- The spec provides no algorithm for checking it
- The obvious compile-time case (literal zero) can be caught without a panic
  runtime
- The fix is one condition in lowering, one new error kind (or reuse of
  existing `LowerError`), and two tests

Doing this now:
1. Reinforces the "conservative compile-time check" pattern — it's now a
   methodology, not a one-off.
2. Closes the second entry in `refs/fls-ambiguities.md` that says "undefined
   behavior" rather than "checked" or "deferred to panic runtime."
3. Requires no new infrastructure — just a guard in the existing `/` and `%`
   lowering paths.

---

## Acceptance criteria

- `cargo build` passes.
- `cargo test` passes (all 1992 existing tests continue to pass).
- At least one new test demonstrates that `10 / 0` (literal zero divisor) is
  rejected at lowering time with an error result.
- At least one new test demonstrates that `10 % 0` (literal zero remainder) is
  rejected at lowering time.
- At least one new test demonstrates that `x / y` (variable divisor) is
  accepted and compiles.
- The §6.9/§6.23 entry in `refs/fls-ambiguities.md` is updated to reflect the
  new state: literal zero caught at compile time, runtime zero still deferred.
- No new FLS citations are wrong or vague.

## FLS notes

- **§6.23:1–10**: "Integer arithmetic may produce an overflow" and "dividing by
  zero is a panic." The spec requires a panic but does not specify the check
  mechanism. The primary FLS gap: the spec mandates the outcome but leaves the
  detection algorithm to the implementation.
- **§6.9**: Out-of-bounds indexing must panic. Same structural gap — deferred.
- The check for `MIN / -1` (signed integer overflow on division) is a separate
  case; leave it as a deferred note in the ambiguities doc.
- Float division by zero is NOT an error per IEEE 754 — do not reject `1.0 / 0.0`.
