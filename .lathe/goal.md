# Goal: §6.23 Runtime Panic Infrastructure — Divide-by-Zero Guard

## What

Establish galvanic's first runtime panic infrastructure by:

1. **Emitting a `_galvanic_panic` trampoline** in `src/codegen.rs` — a small
   assembly routine appended after `_start` in every compiled program:

   ```asm
   // FLS §6.23: runtime panic — divide-by-zero guard target
   _galvanic_panic:
       mov     x8, #93          // __NR_exit (ARM64 Linux)
       mov     x0, #101         // panic exit code (galvanic convention)
       svc     #0               // exit(101)
   ```

   This is emitted unconditionally so any code in the program can branch to it.
   The exit code `101` is galvanic's runtime-panic sentinel — distinct from
   normal program exit codes (0–100) used in the test suite.

2. **Adding a runtime divisor-zero guard in `src/codegen.rs`** — before each
   `sdiv`, `udiv`, and their `msub`-based remainder variants, emit:

   ```asm
       cbz     x{rhs}, _galvanic_panic  // FLS §6.23: panic on div-by-zero
   ```

   This covers all four division-like IR instructions: `IrBinOp::Div`,
   `IrBinOp::UDiv`, `IrBinOp::Rem`, `IrBinOp::URem`. (Check `src/codegen.rs`
   for any additional unsigned-remainder variant — add the guard there too.)

3. **Adding test cases to `tests/e2e.rs`**:

   - **Runtime div-by-zero panics with exit 101:**
     ```rust
     fn main() -> i32 { let a: i32 = 10; let b: i32 = 0; a / b }
     ```
     Expected: exit code 101.

   - **Runtime rem-by-zero panics with exit 101:**
     ```rust
     fn main() -> i32 { let a: i32 = 10; let b: i32 = 0; a % b }
     ```
     Expected: exit code 101.

   - **Runtime div-by-zero via parameter panics:**
     ```rust
     fn div(a: i32, b: i32) -> i32 { a / b }
     fn main() -> i32 { div(10, 0) }
     ```
     Expected: exit code 101. (Confirms the guard fires through a function
     call, not just inline.)

   - **Normal division is unaffected:**
     ```rust
     fn main() -> i32 { let a: i32 = 10; let b: i32 = 2; a / b }
     ```
     Expected: exit code 5. (Guard does not fire when divisor is nonzero.)

   - **Assembly inspection — `sdiv` is preceded by `cbz`:**
     ```rust
     fn f(a: i32, b: i32) -> i32 { a / b }
     fn main() -> i32 { f(1, 1) }
     ```
     Check that the assembly contains `cbz` and `_galvanic_panic` and `sdiv`,
     in that order (not that `cbz` immediately precedes `sdiv` — just that all
     three appear and `_galvanic_panic` is defined).

4. **Updating `refs/fls-ambiguities.md`** — the §6.9/§6.23 entry currently
   says "Non-literal zero divisors are not checked." Update it to document:
   - Runtime divide-by-zero and remainder-by-zero now call `_galvanic_panic`,
     which exits with code 101.
   - `_galvanic_panic` is galvanic's first panic primitive: a bare `exit(101)`
     syscall, not a formatted message.
   - What remains deferred: out-of-bounds array indexing (§6.9) still has no
     bounds check. Integer overflow (§6.23) still uses 64-bit arithmetic
     without a debug-mode trap.
   - Note that `_galvanic_panic` is the foundation for future bounds-check
     and overflow-check guards — they can branch to the same label.

## Scope constraints

- Do NOT add bounds-check guards to array indexing — that is a separate goal.
- Do NOT add integer overflow traps — that is a separate goal.
- Do NOT change the literal-divisor compile-time check (Claim 4m) — it stays.
- The `_galvanic_panic` label must be `.global` so it is visible across
  compilation units if galvanic ever gains multi-file support, but for now
  emitting it as a local label is also acceptable.

## Which stakeholder

**William (the researcher)** and **FLS spec readers (the Ferrocene team)**.

The §6.9/§6.23 entry in `refs/fls-ambiguities.md` currently says:
> "Non-literal zero divisors (e.g. `x / y` where `y` may be zero at runtime)
> are not checked — they emit `sdiv`/`udiv` without a guard."

This is the clearest open correctness gap after Claim 4m: `fn f(a: i32, b:
i32) -> i32 { a / b }` called with `b = 0` on ARM64 silently returns 0
(the hardware behavior of `sdiv` with a zero divisor on AArch64 is to return
0, not trap). The FLS is unambiguous: divide-by-zero **panics** (§6.23).

For spec readers: this closes the most prominent runtime-behavior gap without
requiring libc or a signal handler. The `exit(101)` approach is minimal,
auditable, and correct at the syscall level.

For William: this is the first piece of runtime panic infrastructure — the
foundation the `fls-ambiguities.md` has been pointing at since Claim 4m. It
delivers the runtime half of the divide-by-zero story.

## Why now

The session theme is **panic infrastructure**. Claim 4m (compile-time literal
zero) created the clear statement that runtime panic was deferred. Three
conditions now align:

1. The session is explicitly scoped to "panic infrastructure."
2. The §6.9/§6.23 entry names `_galvanic_panic` as the missing primitive.
3. The implementation cost is minimal: one emitted trampoline (4 instructions),
   one `cbz` guard per division IR node, four tests.

No new IR nodes are needed. No new IR passes. The guard is purely in
`src/codegen.rs`, where the `sdiv`/`udiv` instructions are already emitted.

## FLS notes

- **§6.23**: "An arithmetic operation panics if it results in division by zero."
  The spec does not name the panic mechanism — `_galvanic_panic` with
  `exit(101)` is an implementation choice, documented in `fls-ambiguities.md`.
- **§6.9**: Out-of-bounds indexing is a separate gap. This goal does not close
  it, but `_galvanic_panic` is the shared primitive that will serve both.
- **ARM64 behavior**: `sdiv x0, x0, xzr` on AArch64 returns 0 (CONSTRAINED
  UNPREDICTABLE — some implementations may differ). The guard fires before the
  instruction, so hardware behavior is irrelevant.

## Acceptance criteria

- `cargo build` passes.
- `cargo test` passes (all existing tests pass; new panic tests added).
- The emitted assembly for any function containing `/` or `%` includes a `cbz`
  guard and a `_galvanic_panic` label.
- `fn main() -> i32 { let a: i32 = 10; let b: i32 = 0; a / b }` exits 101
  when run under qemu-aarch64.
- `fn main() -> i32 { let a: i32 = 10; let b: i32 = 2; a / b }` exits 5.
- The §6.23 entry in `refs/fls-ambiguities.md` is updated to reflect runtime
  guard status.
- No new FLS citations are wrong or vague.
