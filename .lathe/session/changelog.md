# Changelog — Cycle 144

## Who This Helps
- **William (researcher)**: Two silent bugs in the `&[T]` fat-pointer path were
  found and fixed. A closure with a `&[T]` explicit parameter would silently produce
  wrong slice lengths. This is the kind of ABI correctness gap that makes a compiler
  untrustworthy — the exit code might be right for a specific test but wrong for
  any call with a different slice length.
- **CI / Validation Infrastructure**: Claim 80 locks in the correct trampoline
  behavior for `&[T]` explicit params. Two prior claims (78, 79) already guarded
  slice params in regular functions and fn-ptr calls; now the closure trampoline
  path is also defended.

## Observed
- Cycle 143's changelog explicitly called out: "The closure trampoline path has not
  been audited for `&[T]` argument passing." This cycle followed up.
- Red-team instructions: inspect a gap that has never been tested against reality.
- The `n_explicit` field in `ClosureTrampoline` was set to `params.len()` (AST param
  count). For a closure `|s: &[i32]|`, `params.len() = 1`, so the trampoline shifted
  only 1 register — moving the data pointer to position 1 while OVERWRITING the
  length already in position 1.
- Additionally, the direct-call arg expansion had no case for `&[T]` slice variables.
  When `slice_len(s)` was called inside a closure where `s: &[i32]` was a closure
  param, only the data pointer was passed (not the length).

## Applied

**Fix 1 — Closure parameter spilling (`src/lower.rs`)**:
- The explicit-parameter spill loop in the closure body lowering (around line 18015)
  now detects `&[T]` params by checking `TyKind::Ref { inner: TyKind::Slice }`.
- For each `&[T]` param, it allocates TWO consecutive stack slots (ptr + len), emits
  two `Store` instructions (from reg and reg+1), and registers the ptr slot in
  `local_slice_slots` so `.len()` and indexing work correctly inside the closure.
- The `reg_offset` counter now increments by 2 for fat-pointer params (not 1).

**Fix 2 — Trampoline `n_explicit` register count (`src/lower.rs`)**:
- `last_closure_n_explicit` now counts register slots (not AST params). A `&[T]`
  param contributes 2 slots; all other param types contribute 1.
- The trampoline codegen loop `for i in (0..t.n_explicit).rev()` shifts each
  register slot from `x{i}` to `x{n_caps+i}`. With `n_explicit = 2` for a single
  `&[T]` param and 1 capture: `x1→x2` (len), `x0→x1` (ptr), `x0←x27` (cap).

**Fix 3 — Direct call arg expansion (`src/lower.rs`)**:
- Added a new `else if` branch before the array-variable case in the direct-call
  argument loop. When an argument is a path variable in `local_slice_slots`, it now
  loads BOTH the ptr slot and the len slot, passing them as two consecutive registers.
- This fixes the case where `slice_len(s)` is called inside a closure body where `s`
  was received as a `&[T]` explicit parameter.

**Tests (`tests/e2e.rs`)**:
- `runtime_closure_trampoline_shifts_fat_ptr_slice_param`: assembly inspection.
  Asserts the trampoline has `mov x2, x1` (len shift) AND `mov x1, x0` (ptr shift),
  and that the closure body loads both ptr+len before calling `slice_len`.
  Asserts result not constant-folded to #13.
- `claim_80_closure_trampoline_slice_param_passes_len_not_just_ptr`: adversarial.
  Two capturing closures, each with a `&[T]` explicit param, passed slices of
  length 3 and 2. Asserts neither length is constant-folded and both trampolines
  emit the len-shift instruction.

**Falsification (`claims.md`, `falsify.sh`)**:
- Claim 80 added with full rationale, ARM64 implementation notes, FLS citations,
  and violation conditions.
- `falsify.sh` extended with Claim 80 check (79 claims now defended).

## Validated
- `cargo test --test e2e -- runtime_closure_trampoline_shifts_fat_ptr_slice_param claim_80_closure_trampoline_slice_param_passes_len_not_just_ptr` — 2 passed
- `cargo test` — 1688 passed, 0 failed
- `cargo clippy -- -D warnings` — clean
- `.lathe/falsify.sh` — 79 passed, 0 failed

## FLS Notes
- **FLS §4.9 AMBIGUOUS**: The fat-pointer ABI (two consecutive registers: data ptr +
  length) is not mandated by the FLS. Every call variant must be audited independently:
  - Direct `bl` (covered from milestone 193)
  - `CallIndirect` via fn-ptr (fixed Cycle 142, Claim 79)
  - Closure trampoline (fixed this cycle, Claim 80)
  - `CallVtable` via dyn Trait (still gated — `&[T]` behind dyn Trait not yet supported)
- **FLS §6.22**: The spec describes capture semantics but does not specify the ABI
  for closures with fat-pointer explicit parameters. Galvanic's choice: the trampoline
  counts register slots (not param count) when determining the shift distance.

## Next
- Milestone 195: the next untouched FLS section adjacent to current capabilities.
  Candidates: `&str` slices with `impl Fn` (similar to `&[T]` but string-specific),
  or method calls on slice params inside closures (`.len()` inside `|s: &[i32]|`
  without delegating to a free function).
- The `CallVtable` path (`dyn Trait` with `&[T]` args) is still unaudited but gated
  by the fact that `&[T]` behind `dyn Trait` is not yet supported — low risk.
