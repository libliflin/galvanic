# Goal: Claim 4q — §6.23 Signed Integer MIN/-1 Overflow Guard

## What

Two complementary changes that close the remaining §6.23 gap and correct stale
documentation left over from Claims 4o and 4p. This goal has been set twice
(cycle 1 and cycle 3) because the builder has not yet implemented it. The
implementation is fully specified below — follow it exactly.

---

### 1. Thread a label counter through `emit_instr` in `src/codegen.rs`

`emit_instr` (line ~377) currently has this signature:

```rust
fn emit_instr(out: &mut String, instr: &Instr, frame_size: u32, saves_lr: bool, fn_name: &str) -> Result<(), CodegenError>
```

Add a `label_ctr: &mut usize` parameter at the end:

```rust
fn emit_instr(out: &mut String, instr: &Instr, frame_size: u32, saves_lr: bool, fn_name: &str, label_ctr: &mut usize) -> Result<(), CodegenError>
```

Add a `let mut label_ctr: usize = 0;` local in `emit_fn` and pass `&mut label_ctr`
at the one call site where `emit_instr` is called. No other callers exist.

---

### 2. Add the MIN/-1 overflow guard to `IrBinOp::Div`

The current `IrBinOp::Div` arm (line ~545) emits:

```asm
    cbz     x{rhs}, _galvanic_panic         // div-by-zero guard
    sdiv    x{dst}, x{lhs}, x{rhs}
```

Replace it with (increment `label_ctr` to get a unique `n`):

```rust
IrBinOp::Div => {
    writeln!(out, "    cbz     x{rhs}, _galvanic_panic         // FLS §6.23: div-by-zero guard")?;
    // FLS §6.23: AMBIGUOUS — signed overflow: i32::MIN / -1 panics in Rust debug mode.
    // ARM64 sdiv returns i32::MIN for this input (CONSTRAINED UNPREDICTABLE).
    // Guard fires for i64::MIN / -1 too (false positive); galvanic has no type system
    // to distinguish — acceptable at this milestone, documented in fls-ambiguities.md.
    let n = *label_ctr;
    *label_ctr += 1;
    writeln!(out, "    cmn     x{rhs}, #1                      // Z=1 if rhs == -1 (sign-extended)")?;
    writeln!(out, "    b.ne    .Lsdiv_ok_{fn_name}_{n}")?;
    writeln!(out, "    movz    x9, #0x8000, lsl #16             // x9 = 0x0000_0000_8000_0000")?;
    writeln!(out, "    sxtw    x9, w9                           // sign-extend: x9 = 0xFFFF_FFFF_8000_0000 = i32::MIN")?;
    writeln!(out, "    cmp     x{lhs}, x9                       // compare lhs with i32::MIN")?;
    writeln!(out, "    b.eq    _galvanic_panic                  // FLS §6.23: signed overflow panic")?;
    writeln!(out, ".Lsdiv_ok_{fn_name}_{n}:")?;
    writeln!(out, "    sdiv    x{dst}, x{lhs}, x{rhs}          // FLS §6.5.5: div (signed)")?;
}
```

---

### 3. Add the MIN/-1 overflow guard to `IrBinOp::Rem`

The current `IrBinOp::Rem` arm (line ~564) emits:

```asm
    cbz     x{rhs}, _galvanic_panic         // rem-by-zero guard
    sdiv    x{dst}, x{lhs}, x{rhs}
    msub    x{dst}, x{dst}, x{rhs}, x{lhs}
```

Replace it with (same pattern, different label prefix):

```rust
IrBinOp::Rem => {
    writeln!(out, "    cbz     x{rhs}, _galvanic_panic         // FLS §6.23: rem-by-zero guard")?;
    // FLS §6.23: signed overflow guard — i32::MIN % -1 also panics (same as div).
    let n = *label_ctr;
    *label_ctr += 1;
    writeln!(out, "    cmn     x{rhs}, #1")?;
    writeln!(out, "    b.ne    .Lsrem_ok_{fn_name}_{n}")?;
    writeln!(out, "    movz    x9, #0x8000, lsl #16")?;
    writeln!(out, "    sxtw    x9, w9")?;
    writeln!(out, "    cmp     x{lhs}, x9")?;
    writeln!(out, "    b.eq    _galvanic_panic")?;
    writeln!(out, ".Lsrem_ok_{fn_name}_{n}:")?;
    writeln!(out, "    sdiv    x{dst}, x{lhs}, x{rhs}          // FLS §6.5.5: rem step 1: quotient")?;
    writeln!(out, "    msub    x{dst}, x{dst}, x{rhs}, x{lhs}  // FLS §6.5.5: rem step 2: lhs - q*rhs")?;
}
```

`IrBinOp::UDiv` does NOT get this guard — unsigned division cannot overflow.

---

### 4. Update stale tests in `tests/e2e.rs`

**4a. Rename the gap-marker test.**

Find:
```rust
fn runtime_sdiv_no_min_neg_one_guard() {
```

This test currently asserts the guard is ABSENT. Delete the entire test body and
replace with a POSITIVE assertion that the guard IS present:

```rust
fn runtime_sdiv_emits_both_zero_and_overflow_guards() {
    let asm = compile_to_asm("fn f(a: i32, b: i32) -> i32 { a / b }\nfn main() -> i32 { f(1, 1) }\n");
    assert!(asm.contains("cbz"), "expected cbz div-by-zero guard");
    assert!(asm.contains("cmn"), "expected cmn overflow guard");
    assert!(asm.contains("_galvanic_panic"), "expected panic trampoline");
    // Verify ordering: cbz before cmn before sdiv
    let cbz_pos = asm.find("cbz").unwrap();
    let cmn_pos = asm.find("cmn").unwrap();
    let sdiv_pos = asm.find("sdiv").unwrap();
    assert!(cbz_pos < cmn_pos, "cbz must precede cmn");
    assert!(cmn_pos < sdiv_pos, "cmn must precede sdiv");
}
```

**4b. Remove a stale negative assertion.**

Find the test `claim_4o_sdiv_emits_cbz_guard`. It contains a line:
```rust
assert!(!asm.contains("0x80000000"), ...);
```
Remove ONLY that one assertion (and its associated string if any). The rest of
the test stays.

---

### 5. Add new Claim 4q tests to `tests/e2e.rs`

Add a `// --- Claim 4q: §6.23 signed MIN/-1 overflow ---` section with these tests:

```rust
#[test]
fn claim_4q_sdiv_emits_cmn_overflow_guard() {
    let asm = compile_to_asm("fn f(a: i32, b: i32) -> i32 { a / b }\nfn main() -> i32 { f(1, 1) }\n");
    assert!(asm.contains("cmn"), "expected cmn guard for signed overflow");
    assert!(asm.contains("sdiv"), "expected sdiv instruction");
    let cmn_pos = asm.find("cmn").unwrap();
    let sdiv_pos = asm.find("sdiv").unwrap();
    assert!(cmn_pos < sdiv_pos, "cmn must appear before sdiv");
}

#[test]
fn claim_4q_rem_emits_cmn_overflow_guard() {
    let asm = compile_to_asm("fn f(a: i32, b: i32) -> i32 { a % b }\nfn main() -> i32 { f(1, 1) }\n");
    assert!(asm.contains("cmn"), "expected cmn guard in rem path");
    assert!(asm.contains("msub"), "expected msub in rem path");
    let cmn_pos = asm.find("cmn").unwrap();
    let msub_pos = asm.find("msub").unwrap();
    assert!(cmn_pos < msub_pos, "cmn must appear before msub");
}

#[test]
fn claim_4q_runtime_min_div_neg_one_exits_101() {
    // i32::MIN / -1 overflows — must panic
    let exit = compile_and_run(
        "fn main() -> i32 { let a: i32 = -2147483648; let b: i32 = -1; a / b }\n",
    );
    assert_eq!(exit, Some(101));
}

#[test]
fn claim_4q_runtime_min_rem_neg_one_exits_101() {
    // i32::MIN % -1 overflows — must panic
    let exit = compile_and_run(
        "fn main() -> i32 { let a: i32 = -2147483648; let b: i32 = -1; a % b }\n",
    );
    assert_eq!(exit, Some(101));
}

#[test]
fn claim_4q_runtime_min_div_neg_one_via_param_exits_101() {
    let exit = compile_and_run(
        "fn div(a: i32, b: i32) -> i32 { a / b }\nfn main() -> i32 { div(-2147483648, -1) }\n",
    );
    assert_eq!(exit, Some(101));
}

#[test]
fn claim_4q_runtime_non_min_div_neg_one_succeeds() {
    // -100 / -1 = 100 — guard must NOT fire
    let exit = compile_and_run(
        "fn main() -> i32 { let a: i32 = -100; let b: i32 = -1; a / b }\n",
    );
    assert_eq!(exit, Some(100));
}

#[test]
fn claim_4q_udiv_no_overflow_guard() {
    // udiv path must NOT contain cmn (unsigned division cannot overflow)
    let asm = compile_to_asm(
        "fn f(a: u32, b: u32) -> u32 { a / b }\nfn main() -> i32 { 0 }\n",
    );
    // udiv section should have cbz (zero guard) but not cmn (overflow guard)
    assert!(asm.contains("udiv"), "expected udiv instruction");
    // Find the udiv and check no cmn appears between cbz and udiv
    // (simple check: udiv path should not contain cmn at all in the asm for this fn)
    let f_start = asm.find("// fn f").unwrap_or(0);
    let main_start = asm.find("// fn main").unwrap_or(asm.len());
    let f_section = &asm[f_start..main_start];
    assert!(!f_section.contains("cmn"), "udiv must not emit cmn overflow guard");
}
```

---

### 6. Update `refs/fls-ambiguities.md`

**Entry §4.9 — Bounds Checking Mechanism:**

Find the current text that says something like "No bounds check is emitted at
this milestone" and replace it with:

```markdown
**Resolution (Claim 4p):** Every array and slice index emits a runtime bounds
check before the address computation:
- `cmp x{idx}, #{len}` compares the (zero-extended) index against the array length.
- `b.hs _galvanic_panic` branches if `idx >= len` (unsigned ≥, so negative signed
  indices also trigger the guard via wraparound).
- The `_galvanic_panic` trampoline executes `exit(101)`.
This matches Rust's debug-mode behavior: out-of-bounds indexing panics.
```

**Entry §6.9/§6.23 — Panic Mechanism:**

Find the current text that says something like "Non-literal zero divisors...are
not checked" and replace it with a complete description of all guards:

```markdown
**Resolution (Claims 4m, 4o, 4q):** Three distinct division-panic guards are
implemented:

1. **Literal-zero divisors (Claim 4m):** Caught at compile time. Dividing by a
   literal `0` produces a compile error before codegen.

2. **Runtime zero divisors (Claim 4o):** Guarded by `cbz x{rhs}, _galvanic_panic`
   immediately before every `sdiv` and `udiv` instruction.

3. **Signed overflow — MIN/-1 (Claim 4q):** Guarded by `cmn x{rhs}, #1` +
   `cmp x{lhs}, x9` (where x9 = i32::MIN sign-extended) before every `sdiv`.
   Fires when `rhs == -1` AND `lhs == i32::MIN`. ARM64 `sdiv` returns i32::MIN
   for this input (CONSTRAINED UNPREDICTABLE); Rust debug mode panics.

4. **Unsigned division overflow:** Not applicable — unsigned division cannot
   produce an unrepresentable result.

**AMBIGUOUS (§6.23):** The MIN/-1 guard also fires for i64::MIN / -1 (false
positive), since galvanic lacks a full type system. At this milestone, the test
suite exercises only i32, so this is acceptable. Documented as a limitation.

**Integer overflow in `+`, `-`, `*`:** No trap is emitted. Arithmetic wraps
in 64-bit registers; narrow-type masking (AND / SXTB / SXTH) happens but does
not detect overflow. This is a known gap — a separate goal.

`_galvanic_panic`: bare `mov x8, #93; mov x0, #101; svc #0` — `exit(101)`.
```

---

## Scope constraints

- Do NOT add overflow traps for `+`, `-`, `*`.
- Do NOT add i64-specific MIN/-1 detection — always-emit is correct and documented.
- Do NOT change `IrBinOp::UDiv` — no guard needed.
- Do NOT change the zero-divisor `cbz` guard.
- Do NOT change the bounds-check guard (Claim 4p).

## Acceptance criteria

- `cargo build` passes with no warnings.
- `cargo test` passes — all 1772 existing tests pass; 7 new tests added (1779 total).
- Test `runtime_sdiv_no_min_neg_one_guard` is deleted/renamed to `runtime_sdiv_emits_both_zero_and_overflow_guards`.
- Stale `!asm.contains("0x80000000")` assertion removed from `claim_4o_sdiv_emits_cbz_guard`.
- `cargo clippy -- -D warnings` passes.
- `refs/fls-ambiguities.md` §4.9 entry describes Claim 4p bounds-check behavior.
- `refs/fls-ambiguities.md` §6.9/§6.23 entry describes all three division guards.
