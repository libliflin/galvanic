# Goal: Claim 4s — §6.23 Arithmetic Overflow Guard for i32 +, -, *

## What

Add runtime overflow guards for `BinOp::Add`, `BinOp::Sub`, and `BinOp::Mul`
in `src/codegen.rs`, and fix one stale entry in `refs/fls-ambiguities.md`.

Claims 4m through 4r completed the division and shift safety stories. The only
remaining §6.23 gap in galvanic's implemented subset is arithmetic overflow for
+, -, and *. FLS §6.23 requires debug-mode panic on signed integer overflow;
galvanic currently uses 64-bit arithmetic throughout and emits no check.

---

## Implementation: post-instruction i32 range check

The guard uses a 3-instruction sequence appended after the primary arithmetic
instruction. No type information is available at the codegen level (same
constraint as the MIN/-1 guard). The guard is calibrated to i32 semantics
and documented as AMBIGUOUS for i64/u32 — consistent with the established
project pattern.

**Pattern for Add, Sub, Mul:**

After the primary instruction (`add x{dst}`, `sub x{dst}`, or `mul x{dst}`):
```
    sxtw    x9, w{dst}             // sign-extend low 32 bits of result
    cmp     x{dst}, x9             // if 64-bit result != sign-extended 32-bit, i32 overflow
    b.ne    _galvanic_panic        // FLS §6.23: signed arithmetic overflow panic
```

`sxtw` replicates bit 31 of the result across bits 32–63. If the 64-bit result
equals its own sign-extended 32-bit self, the result is representable as i32 and
no overflow occurred. Otherwise, overflow → panic.

**Why this works for i32:**
- `i32::MAX + 1 = 2147483648` (0x80000000 in 32 bits → sxtw = -2147483648 ≠ 2147483648) → panic ✓
- `i32::MIN - 1 = -2147483649` (low32 = 0x7FFFFFFF → sxtw = 2147483647 ≠ -2147483649) → panic ✓
- `i32::MAX * 2 = 4294967294` (low32 = 0xFFFFFFFE → sxtw = -2 ≠ 4294967294) → panic ✓
- `100 + 200 = 300` (sxtw(300) = 300) → no panic ✓
- Narrow types (u8, i8, u16, i16): wrapping arithmetic stays within 32-bit range,
  so sxtw matches the 64-bit value — no false panic ✓

**AMBIGUOUS (document, don't fix):**
- i64 values > i32::MAX stored in 64-bit registers will false-positive. The test
  suite exercises i64 with small values; galvanic has no type system to distinguish.
- u32 addition of two large u32 values (each > i32::MAX) may false-positive.
  No u32 wrap tests exist; existing u32 tests use small values. Documented.

---

## Code changes in `src/codegen.rs`

Find the `IrBinOp::Add`, `IrBinOp::Sub`, and `IrBinOp::Mul` arms (lines ~537–552)
in the `emit_instr` match block. The current code has an AMBIGUOUS comment followed
by a single `writeln!` for each. Replace each arm with the guarded version:

```rust
IrBinOp::Add => {
    writeln!(out, "    add     x{dst}, x{lhs}, x{rhs}          // FLS §6.5.5: add")?;
    // FLS §6.23: AMBIGUOUS — signed overflow: i32 overflow panics in debug mode.
    // Guard fires for i64 values outside i32 range (false positive) and does NOT
    // fire for u32 wrap (unsigned overflow is a different concern). Acceptable at
    // this milestone: test suite exercises i32; documented in fls-ambiguities.md.
    writeln!(out, "    sxtw    x9, w{dst}                          // sign-extend low 32 bits")?;
    writeln!(out, "    cmp     x{dst}, x9                          // i32 overflow check")?;
    writeln!(out, "    b.ne    _galvanic_panic                     // FLS §6.23: overflow panic")?;
}
IrBinOp::Sub => {
    writeln!(out, "    sub     x{dst}, x{lhs}, x{rhs}          // FLS §6.5.5: sub")?;
    // FLS §6.23: AMBIGUOUS — same as Add guard above.
    writeln!(out, "    sxtw    x9, w{dst}")?;
    writeln!(out, "    cmp     x{dst}, x9")?;
    writeln!(out, "    b.ne    _galvanic_panic")?;
}
IrBinOp::Mul => {
    writeln!(out, "    mul     x{dst}, x{lhs}, x{rhs}          // FLS §6.5.5: mul")?;
    // FLS §6.23: AMBIGUOUS — same as Add guard above.
    writeln!(out, "    sxtw    x9, w{dst}")?;
    writeln!(out, "    cmp     x{dst}, x9")?;
    writeln!(out, "    b.ne    _galvanic_panic")?;
}
```

**Important:** Keep the existing AMBIGUOUS comment that precedes these arms. Remove
only the `// FLS §6.23: 64-bit, no i32 wrap` suffix from the primary instruction
comments (replaced by the full guard above). Do NOT change `IrBinOp::Div`,
`IrBinOp::Rem`, `IrBinOp::UDiv`, or any other op.

---

## Update stale test in `tests/e2e.rs`

There are existing assembly inspection tests that look for `"add"` in the assembly.
These should still pass because the guard uses `add` then `sxtw`/`cmp`/`b.ne` —
`add` is still present. No existing tests need modification.

However, update the comment `// §6.23: 64-bit, no i32 wrap` references in any
existing tests that assert the guard is ABSENT for add/sub/mul. Search for
`no i32 wrap` in tests/e2e.rs; if found, remove those negative assertions.

---

## Add new Claim 4s tests to `tests/e2e.rs`

Add a `// --- Claim 4s: §6.23 arithmetic overflow guard ---` section:

```rust
#[test]
fn claim_4s_add_emits_overflow_guard() {
    let asm = compile_to_asm("fn f(a: i32, b: i32) -> i32 { a + b }\nfn main() -> i32 { f(1, 1) }\n");
    assert!(asm.contains("add"), "expected add instruction");
    assert!(asm.contains("sxtw"), "expected sxtw overflow guard");
    assert!(asm.contains("b.ne"), "expected b.ne to _galvanic_panic");
    // Verify ordering: add before sxtw before b.ne
    let add_pos = asm.find("    add ").unwrap_or_else(|| asm.find("add").unwrap());
    let sxtw_pos = asm.find("sxtw").unwrap();
    let bne_pos = asm.find("b.ne").unwrap();
    assert!(add_pos < sxtw_pos, "add must precede sxtw");
    assert!(sxtw_pos < bne_pos, "sxtw must precede b.ne");
}

#[test]
fn claim_4s_sub_emits_overflow_guard() {
    let asm = compile_to_asm("fn f(a: i32, b: i32) -> i32 { a - b }\nfn main() -> i32 { f(2, 1) }\n");
    assert!(asm.contains("sub"), "expected sub instruction");
    assert!(asm.contains("sxtw"), "expected sxtw overflow guard");
    assert!(asm.contains("b.ne"), "expected b.ne to _galvanic_panic");
}

#[test]
fn claim_4s_mul_emits_overflow_guard() {
    let asm = compile_to_asm("fn f(a: i32, b: i32) -> i32 { a * b }\nfn main() -> i32 { f(2, 3) }\n");
    assert!(asm.contains("mul"), "expected mul instruction");
    assert!(asm.contains("sxtw"), "expected sxtw overflow guard");
    assert!(asm.contains("b.ne"), "expected b.ne to _galvanic_panic");
    // Guard must precede any use of result
    let mul_pos = asm.find("    mul ").unwrap_or_else(|| asm.find("mul").unwrap());
    let sxtw_pos = asm.find("sxtw").unwrap();
    assert!(mul_pos < sxtw_pos, "mul must precede sxtw guard");
}

#[test]
fn claim_4s_runtime_i32_max_plus_one_exits_101() {
    // i32::MAX + 1 overflows — must panic
    let exit = compile_and_run(
        "fn main() -> i32 { let a: i32 = 2147483647; let b: i32 = 1; a + b }\n",
    );
    assert_eq!(exit, Some(101));
}

#[test]
fn claim_4s_runtime_i32_min_minus_one_exits_101() {
    // i32::MIN - 1 overflows — must panic
    let exit = compile_and_run(
        "fn main() -> i32 { let a: i32 = -2147483648; let b: i32 = 1; a - b }\n",
    );
    assert_eq!(exit, Some(101));
}

#[test]
fn claim_4s_runtime_i32_max_mul_two_exits_101() {
    // i32::MAX * 2 overflows — must panic
    let exit = compile_and_run(
        "fn main() -> i32 { let a: i32 = 2147483647; let b: i32 = 2; a * b }\n",
    );
    assert_eq!(exit, Some(101));
}

#[test]
fn claim_4s_runtime_i32_max_plus_one_via_param_exits_101() {
    // via function parameters — proves runtime execution, not compile-time folding
    let exit = compile_and_run(
        "fn add(a: i32, b: i32) -> i32 { a + b }\nfn main() -> i32 { add(2147483647, 1) }\n",
    );
    assert_eq!(exit, Some(101));
}

#[test]
fn claim_4s_runtime_normal_add_succeeds() {
    // 100 + 200 = 300 — guard must NOT fire
    let exit = compile_and_run(
        "fn main() -> i32 { let a: i32 = 100; let b: i32 = 200; a + b }\n",
    );
    assert_eq!(exit, Some(300 % 256));  // 300 > 255 so exit code is 300 mod 256 = 44
}

#[test]
fn claim_4s_runtime_normal_sub_succeeds() {
    // 10 - 3 = 7 — guard must NOT fire
    let exit = compile_and_run(
        "fn main() -> i32 { let a: i32 = 10; let b: i32 = 3; a - b }\n",
    );
    assert_eq!(exit, Some(7));
}

#[test]
fn claim_4s_runtime_normal_mul_succeeds() {
    // 6 * 7 = 42 — guard must NOT fire
    let exit = compile_and_run(
        "fn main() -> i32 { let a: i32 = 6; let b: i32 = 7; a * b }\n",
    );
    assert_eq!(exit, Some(42));
}
```

---

## Fix stale `refs/fls-ambiguities.md` §4.9 entry

The §4.9 entry still says "No bounds check is emitted at this milestone" — this
has been wrong since Claim 4p was implemented. Replace the entire §4.9 section:

**Find:**
```markdown
**Galvanic's choice:** No bounds check is emitted at this milestone. Out-of-
bounds access produces undefined behavior at the assembly level (load/store at
wrong address). This is a known deviation; the check is deferred until a panic
infrastructure is in place.

**Source:** `src/ir.rs:730`, `src/codegen.rs:926`, `src/lower.rs:17880`
```

**Replace with:**
```markdown
**Resolution (Claim 4p):** Every array and slice index emits a runtime bounds
check before the address computation:
- `cmp x{idx}, #{len}` compares the (zero-extended) index against the array length.
- `b.hs _galvanic_panic` branches if `idx >= len` (unsigned ≥, so negative signed
  indices also trigger the guard via wraparound).
- The `_galvanic_panic` trampoline executes `exit(101)`.
This matches Rust's debug-mode behavior: out-of-bounds indexing panics.

**Source:** `src/codegen.rs` (bounds check emission), `src/lower.rs` (IndexAccess IR)
```

Also update the §6.9/§6.23 entry to add a fourth bullet describing the new
arithmetic overflow guard:

Find the line:
```
- `+`, `-`, `*` overflow: no overflow check; arithmetic wraps per 64-bit
  hardware. This is a known deviation from debug-mode Rust semantics.
  FLS §6.23 AMBIGUOUS — spec requires debug-mode panic but galvanic uses 64-bit
  arithmetic throughout and does not insert overflow checks for these operators.
```

Replace with:
```
- `+`, `-`, `*` overflow (Claim 4s): guarded by `sxtw x9, w{dst}` + `cmp x{dst}, x9`
  + `b.ne _galvanic_panic` after every `add`, `sub`, and `mul` instruction.
  Fires when the 64-bit result does not equal its own sign-extended 32-bit self
  (i.e., the result does not fit in i32).
  FLS §6.23 AMBIGUOUS: the guard treats all arithmetic as i32 because galvanic
  has no type system. False positives for i64 values outside i32 range; false
  negatives for u32 arithmetic that wraps within [0, 2^32) but outside i32 range.
  At this milestone, the test suite exercises i32; documented as a limitation.
```

---

## Scope constraints

- Do NOT add guards for `BitAnd`, `BitOr`, `BitXor`, `Shl`, `Shr`, `UShr` — these
  already have their own guards (Claim 4r) or have defined wrap behavior.
- Do NOT change `IrBinOp::Div`, `IrBinOp::Rem`, `IrBinOp::UDiv` — already guarded.
- Do NOT add type info to `BinOp` or `IrBinOp` — out of scope for this claim.
- Do NOT add guards for comparison operators (`Lt`, `Le`, `Gt`, `Ge`, `Eq`, `Ne`).
- The `x9` scratch register is already used by the MIN/-1 guard. Using it here too
  is fine — `x9` is caller-saved and not preserved across instructions.

---

## Acceptance criteria

- `cargo build` passes with no warnings.
- `cargo test` passes — all 1784 existing tests pass; 10 new tests added (1794 total).
- `cargo clippy -- -D warnings` passes.
- Assembly for `fn f(a: i32, b: i32) -> i32 { a + b }` contains `sxtw`, `cmp`, `b.ne`.
- `refs/fls-ambiguities.md` §4.9 entry describes Claim 4p bounds-check behavior.
- `refs/fls-ambiguities.md` §6.9/§6.23 entry mentions Claim 4s arithmetic overflow guard.
