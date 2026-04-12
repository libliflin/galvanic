# Goal: Claim 4q — §6.23 Signed Integer MIN/-1 Overflow Guard

## What

Two complementary changes that close the remaining §6.23 gap and correct stale
documentation left over from Claims 4o and 4p:

### 1. Add a MIN/-1 overflow guard to `sdiv` (and `msub`-based `rem`)

Before each `sdiv` instruction in `src/codegen.rs`, add a second guard after
the existing `cbz` (divide-by-zero guard) that panics when the RHS is `-1`
and the LHS is `i32::MIN` (the only i32 overflow case for division):

```asm
    cbz     x{rhs}, _galvanic_panic         // FLS §6.23: div-by-zero
    // FLS §6.23: signed overflow — i32::MIN / -1 panics
    cmn     x{rhs}, #1                      // sets Z if rhs == -1
    b.ne    .Lno_overflow_{label}
    mov     x_tmp, #0x80000000
    cmp     x{lhs}, x_tmp                   // compare lhs with i32::MIN
    b.eq    _galvanic_panic
.Lno_overflow_{label}:
    sdiv    x{dst}, x{lhs}, x{rhs}
```

Apply the same guard to the `sdiv` inside the `msub`-based remainder path
(`IrBinOp::Rem`). The unsigned variants (`udiv`, `IrBinOp::URem`) do not
need this guard — unsigned division cannot overflow.

Use a per-site unique label (e.g. counter-based) to avoid label collisions
across functions.

**ARM64 note:** Galvanic uses 64-bit arithmetic for i32 values. `i32::MIN`
sign-extended to 64 bits is `0xFFFFFFFF_80000000`. The `cmn x, #1` sets Z
if `x == -1 (64-bit)`, which is `0xFFFFFFFF_FFFFFFFF` — the sign-extended
form of i32 `-1`. This is correct because galvanic sign-extends i32 values
when loading from stack slots.

The guard must fire **only** for the i32 case. At this milestone galvanic
does not have a full type system, so the simplest correct approach is to
always emit the guard for `sdiv` — it will have false positives (i64 MIN/-1
would also fire) but galvanic's test suite currently only exercises i32, so
this is acceptable. Document this limitation in `fls-ambiguities.md`.

### 2. Update two stale entries in `refs/fls-ambiguities.md`

**Entry §4.9 — Bounds Checking Mechanism:** The current text says "No bounds
check is emitted at this milestone." This is wrong — Claim 4p added runtime
bounds checks. Update it to describe the current implementation:
- Every array/slice index emits a `cmp` + `b.lo _galvanic_panic` guard.
- Negative indices (when the index is a signed value ≥ 0x8000_0000) trigger
  the guard via the unsigned comparison.
- The `_galvanic_panic` trampoline exits with code 101.

**Entry §6.9/§6.23 — Panic Mechanism:** The current text says "Non-literal
zero divisors...are not checked — they emit `sdiv`/`udiv` without a guard."
This is also wrong — Claim 4o added the `cbz` guard. Update to describe:
- Literal-zero divisors: caught at compile time (Claim 4m).
- Runtime zero divisors: guarded by `cbz x{rhs}, _galvanic_panic` (Claim 4o).
- Signed MIN/-1 overflow: **newly guarded by this claim (Claim 4q)**.
- Unsigned MIN/-1: not applicable (unsigned division cannot overflow).
- Integer overflow (non-division): still no trap — arithmetic wraps.
- `_galvanic_panic`: bare `exit(101)` syscall.

### 3. Add test cases to `tests/e2e.rs`

- **Runtime i32::MIN / -1 panics with exit 101:**
  ```rust
  fn main() -> i32 { let a: i32 = -2147483648; let b: i32 = -1; a / b }
  ```
  Expected: exit code 101.

- **Runtime i32::MIN % -1 panics with exit 101:**
  ```rust
  fn main() -> i32 { let a: i32 = -2147483648; let b: i32 = -1; a % b }
  ```
  Expected: exit code 101.

- **Runtime i32::MIN / -1 via parameter panics:**
  ```rust
  fn div(a: i32, b: i32) -> i32 { a / b }
  fn main() -> i32 { div(-2147483648, -1) }
  ```
  Expected: exit code 101.

- **Normal division near MIN is unaffected:**
  ```rust
  fn main() -> i32 { let a: i32 = -2147483648; let b: i32 = 2; a / b }
  ```
  Expected: exit code 76 (i32::MIN / 2 = -1073741824; -1073741824 as u8 exit
  code wraps). Actually `-1073741824 % 256 = 0`... Let me reconsider: the exit
  code is the low byte of the return value. i32::MIN / 2 = -1073741824.
  As an exit code (0–255), the shell takes the low byte: -1073741824 & 0xFF = 0.
  Use a simpler divisor: `let a: i32 = -2147483648; let b: i32 = 4;` →
  result is -536870912, low byte is 0. Use `let a: i32 = -100; let b: i32 = -1`
  → result is 100, exit 100. This is unambiguous and does not trigger the guard.

  Rewritten:
  ```rust
  fn main() -> i32 { let a: i32 = -100; let b: i32 = -1; a / b }
  ```
  Expected: exit code 100. (Guard does not fire for non-MIN dividends.)

- **Assembly inspection — `cmn` guard appears before `sdiv`:**
  ```rust
  fn f(a: i32, b: i32) -> i32 { a / b }
  fn main() -> i32 { f(1, 1) }
  ```
  Assert the assembly contains `cmn` and `_galvanic_panic` and `sdiv`, with
  `cmn` appearing before `sdiv` in the text.

## Scope constraints

- Do NOT add overflow traps for `+`, `-`, `*` — that is a separate goal.
- Do NOT add i64-specific MIN/-1 detection — use the same guard for all sdiv
  sites and document the limitation.
- Do NOT change the zero-divisor `cbz` guard (Claim 4o) — it stays.
- Do NOT change the bounds-check guard (Claim 4p) — it stays.

## Which stakeholder

**William (the researcher)** and **FLS spec readers (the Ferrocene team)**.

§6.23 says "An arithmetic operation panics if it results in division by zero"
and also "if the operation results in overflow" (where overflow is
implementation-defined for non-const contexts to be the debug panic behavior).

`i32::MIN / -1` is the canonical signed-division overflow case: the mathematical
result `2^31` does not fit in i32. Real Rust panics here in debug mode. ARM64
`sdiv` produces `i32::MIN` (CONSTRAINED UNPREDICTABLE per Armv8-A), which is
silently wrong — the worst kind of UB.

The existing test `runtime_sdiv_no_min_neg_one_guard` explicitly proves this
gap exists today. This goal closes it.

The stale doc entries are a secondary but real problem: `refs/fls-ambiguities.md`
is the primary research output of the project. Two entries describe behavior
that changed two claims ago. Spec readers cannot trust the doc if it is stale.

## Why now

1. The session theme is **de-risk hard problems categorically**. The MIN/-1
   case is the only remaining division overflow not covered by Claim 4o.
2. The `runtime_sdiv_no_min_neg_one_guard` test was written TO DOCUMENT this
   gap — it is a standing invitation for this claim.
3. The `fls-ambiguities.md` staleness is a maintenance debt that compounds:
   future claims will add more correct text on top of wrong text.
4. Both changes are small and self-contained. Together they complete the §6.23
   divide/overflow story through the division operation.

## FLS notes

- **§6.23**: "An arithmetic operation panics if it results in division by zero."
  In Rust, this is understood to include `i32::MIN / -1` (signed overflow via
  division), which also panics in debug mode. The FLS does not enumerate the
  overflow cases explicitly — galvanic's behavior matches Rust's de-facto
  semantics, documented as a resolution in `fls-ambiguities.md`.
- **`_galvanic_panic`**: defined in `src/codegen.rs`, emitted unconditionally.
  Both the cbz guard (Claim 4o) and the new cmn/cmp guard (this claim) branch
  to the same label. Exit code 101.

## Acceptance criteria

- `cargo build` passes.
- `cargo test` passes (all 1772 existing tests pass; new tests added).
- The test `runtime_sdiv_no_min_neg_one_guard` is **deleted** (it documented
  a gap that no longer exists) or updated to confirm the guard IS present.
- `fn main() -> i32 { let a: i32 = -2147483648; let b: i32 = -1; a / b }` 
  exits 101 under qemu-aarch64.
- `fn main() -> i32 { let a: i32 = -100; let b: i32 = -1; a / b }` exits 100.
- `refs/fls-ambiguities.md` §4.9 entry describes current bounds-check behavior.
- `refs/fls-ambiguities.md` §6.9/§6.23 entry describes all three guards:
  literal-zero (Claim 4m), runtime-zero (Claim 4o), signed-overflow (Claim 4q).
- No new FLS citations are wrong or vague.
