# Goal: Claim 4r — §6.5.9 Shift Overflow Guard (Invalid Shift Amount)

## What

Add a runtime panic guard before every `lsl` and `asr`/`lsr` instruction
for shift amounts that are invalid per Rust's debug-mode semantics:
1. **Negative shift amount** — shift by any value where bit 63 is set (sign bit).
2. **Shift amount ≥ 64** — shift that exceeds the register width.

Both cases must branch to `_galvanic_panic` (exit 101).

---

### 1. Add a shift-guard helper in `src/codegen.rs`

Directly before emitting `lsl`/`asr`/`lsr`, emit two guards:

```rust
// FLS §6.5.9: Shift amount must be in [0, 63]; negative or >= 64 panics.
// Guard 1: negative shift amount — test bit 63 (sign bit of rhs).
writeln!(out, "    tbnz    x{rhs}, #63, _galvanic_panic  // FLS §6.5.9: negative shift → panic")?;
// Guard 2: shift amount >= 64.
writeln!(out, "    cmp     x{rhs}, #64                   // FLS §6.5.9: shift >= 64?")?;
writeln!(out, "    b.ge    _galvanic_panic                // FLS §6.5.9: shift >= 64 → panic")?;
```

Apply this guard to `IrBinOp::Shl`, `IrBinOp::Shr`, and `IrBinOp::UShr`
(left shift, signed right shift, unsigned right shift).

**AMBIGUOUS (§6.5.9):** Rust panics for `i32 << 32` (shift ≥ type width, not
register width). Galvanic operates on 64-bit registers throughout and has no
type system to distinguish i32 from i64 at this milestone. The guard fires for
shift ≥ 64 (register width), which is a false negative for i32 (shifts 32–63
are not caught). This is acceptable and must be documented in
`refs/fls-ambiguities.md`.

---

### 2. Update `needs_panic` in `emit_asm`

The `needs_panic` predicate (line ~159) currently checks for `Div | Rem | UDiv`
and indexed loads/stores. Extend it to also return `true` for shift operations:

```rust
Instr::BinOp { op: IrBinOp::Div | IrBinOp::Rem | IrBinOp::UDiv
             | IrBinOp::Shl | IrBinOp::Shr | IrBinOp::UShr, .. } => true,
```

Only add variants that actually exist in `IrBinOp` — check `src/ir.rs` first.

---

### 3. Add tests to `tests/e2e.rs`

Add a `// --- Claim 4r: §6.5.9 shift overflow ---` section with these tests:

```rust
#[test]
fn claim_4r_shl_emits_tbnz_and_cmp_guards() {
    let asm = compile_to_asm("fn f(a: i32, b: i32) -> i32 { a << b }\nfn main() -> i32 { f(1, 1) }\n");
    assert!(asm.contains("tbnz"), "expected tbnz negative-shift guard");
    assert!(asm.contains("lsl"), "expected lsl instruction");
    let tbnz_pos = asm.find("tbnz").unwrap();
    let lsl_pos = asm.find("    lsl ").unwrap();
    assert!(tbnz_pos < lsl_pos, "tbnz must precede lsl");
}

#[test]
fn claim_4r_shr_emits_tbnz_guard() {
    let asm = compile_to_asm("fn f(a: i32, b: i32) -> i32 { a >> b }\nfn main() -> i32 { f(4, 1) }\n");
    assert!(asm.contains("tbnz"), "expected tbnz negative-shift guard in shr path");
    assert!(asm.contains("asr"), "expected asr instruction");
}

#[test]
fn claim_4r_runtime_negative_shift_exits_101() {
    // shift by -1 (negative) must panic
    let exit = compile_and_run(
        "fn main() -> i32 { let a: i32 = 1; let b: i32 = -1; a << b }\n",
    );
    assert_eq!(exit, Some(101));
}

#[test]
fn claim_4r_runtime_shift_by_64_exits_101() {
    // shift by 64 must panic (>= register width)
    let exit = compile_and_run(
        "fn main() -> i32 { let a: i32 = 1; let b: i32 = 64; a << b }\n",
    );
    assert_eq!(exit, Some(101));
}

#[test]
fn claim_4r_runtime_shift_by_1_succeeds() {
    // shift by 1 is valid — guard must NOT fire
    let exit = compile_and_run(
        "fn main() -> i32 { let a: i32 = 1; let b: i32 = 1; a << b }\n",
    );
    assert_eq!(exit, Some(2));
}

#[test]
fn claim_4r_runtime_negative_shr_exits_101() {
    let exit = compile_and_run(
        "fn f(a: i32, b: i32) -> i32 { a >> b }\nfn main() -> i32 { f(8, -1) }\n",
    );
    assert_eq!(exit, Some(101));
}

#[test]
fn claim_4r_runtime_valid_shl_via_param_succeeds() {
    // 3 << 2 = 12; guard must not fire for valid params
    let exit = compile_and_run(
        "fn f(a: i32, b: i32) -> i32 { a << b }\nfn main() -> i32 { f(3, 2) }\n",
    );
    assert_eq!(exit, Some(12));
}
```

---

### 4. Update `refs/fls-ambiguities.md`

Add a new entry for §6.5.9 (insert after the §6.5 section or near the §6.9/§6.23 entry):

```markdown
## §6.5.9 — Shift Operator Overflow

**Gap:** The FLS requires that shifting by a negative amount or an amount ≥ the
type's bit width panics at runtime (debug mode).

**Resolution (Claim 4r):** Two guards are emitted before every `lsl`/`asr`/`lsr`
instruction:
- `tbnz x{rhs}, #63, _galvanic_panic` — fires if rhs is negative (bit 63 set).
- `cmp x{rhs}, #64; b.ge _galvanic_panic` — fires if rhs ≥ 64 (register width).

**AMBIGUOUS (§6.5.9):** Rust panics for `i32 << 32` (shift ≥ 32, the i32 type
width). Galvanic uses 64-bit registers throughout and has no type system at this
milestone. The guard fires for shift ≥ 64 (false negative for i32: shifts 32–63
escape detection). This is a known limitation.

**Source:** `src/codegen.rs` (shift guard emission)
```

---

## Scope constraints

- Do NOT add guards for shift variants that don't exist in `IrBinOp`.
- Do NOT change the zero-divisor `cbz` guard.
- Do NOT change the MIN/-1 `cmn`/`cmp` guard.
- Do NOT change the bounds-check guard.
- Do NOT add overflow traps for `+`, `-`, `*`.

## Acceptance criteria

- `cargo build` passes with no warnings.
- `cargo test` passes — all 1779 existing tests pass; 7 new tests added (1786 total).
- `cargo clippy -- -D warnings` passes.
- `refs/fls-ambiguities.md` has a new §6.5.9 entry describing the shift guard
  and the i32 false-negative ambiguity.
- Assembly inspection test confirms `tbnz` precedes `lsl` in shift output.
- Runtime tests confirm negative shift and shift ≥ 64 both exit 101.
- Runtime test confirms valid shift succeeds (exit 2 for `1 << 1`).
