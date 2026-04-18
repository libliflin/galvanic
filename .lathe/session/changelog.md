# Changelog — Cycle 016 (Customer Champion)

## Stakeholder: Compiler Contributor

**Who I became.** A Rust programmer, comfortable with compilers, who found galvanic interesting and wants to contribute. They're not the primary author. They have a feature target in mind and are tracing the pipeline to find where to add it.

**What I did.**
1. Confirmed the floor: `cargo test` — 2082 tests, all green. Build clean. Clippy clean.
2. Ran the patterns fixture to find a live "not yet supported" error: `cargo run -- tests/fixtures/fls_5_patterns.rs`. Got: "not yet supported: expected struct literal `Inner { .. }` for nested struct field" (function: main).
3. Searched for the error string in source — found it at `lower.rs:6406` inside `store_nested_struct_lit`. The function has FLS commentary in its docstring but the error string itself cites no FLS section.
4. Ran `grep -c "not yet supported.*FLS" src/lower.rs` — returned 3. Total "not yet supported" count: 42. 39 error messages violate the documented invariant.
5. Confirmed the architecture doc explicitly promises: "it emits an error naming the failing function, the FLS section, and the specific construct." The implementation delivers the function and the construct, but omits the FLS section in 93% of cases.

**The worst moment.** Getting "not yet supported: complex let pattern not yet supported" — a message that describes *what* failed but gives zero spec anchor. To find the FLS section I had to read the surrounding code, locate a nearby comment, cross-reference the FLS TOC. A Contributor who doesn't know the FLS well (which is the whole point — they're new to this) is stuck.

**The goal set.** Add FLS section citations to the "not yet supported" error strings in `src/lower.rs` — specifically the `LowerError::Unsupported(...)` call site strings — to fulfill the documented architecture invariant. Three messages already do this (`(FLS §6.5.9)`, etc.); apply the same pattern to the remaining 39, prioritizing hot-path errors (let patterns, if-let, while-let, match patterns, @ binding, nested struct, method errors).

**Why now.** The Compiler Contributor has been absent four cycles. The architecture doc makes a specific, testable promise about error message format. The implementation breaks it systematically. Every Compiler Contributor who reads an error and tries to navigate to the spec hits this wall. The fix is low-risk (string edits), high-signal (makes the invariant enforceable by reading), and makes a wrong state (FLS-free error messages) visible to reviewers going forward.

---

# Verification — Cycle 015, Round 1 (Verifier)

## What I compared

**Goal:** Add `#[cfg(test)]` size assertion tests to `src/ir.rs` for `Instr`,
`IrValue`, and `StaticValue` — making the cache-line size claims in the module
docs enforceable by CI. Model: `token_is_eight_bytes` in `lexer.rs`.

**Code I read:**
- `src/ir.rs` — confirmed zero `#[cfg(test)]` blocks (grep returned no matches).
- Module-level cache-line notes: `StaticValue` claimed "fits in 16 bytes",
  `IrValue` and `Instr` claimed "fit comfortably in a single cache line."
- Builder's diff: changelog and goal files only — no implementation code.
  The builder set the goal; the implementation was the verifier's job this round.

**What I ran:**
- `cargo test --lib ir::tests` to confirm the three new tests pass.
- Temporary `eprintln!("Instr size = {}", size_of::<Instr>())` test to find
  the actual `Instr` size before hardcoding it (result: 80 bytes).
- `cargo test` (full suite: 2082 pass, 0 fail).
- `cargo clippy -- -D warnings` (clean).

## What's here, what was asked

The builder set the goal precisely and correctly diagnosed the gap. No
implementation code was present — the gap was the full implementation.

One additional finding: the module-level cache-line note claimed `Instr`
"fits comfortably in a single cache line" — a claim that was true at milestone
1 but became stale once `Call`/`ClosureCall`/`TraitCall` variants were added
with `String` (24 bytes) and `Vec<u8>` (24 bytes) fields. The actual size is
80 bytes, larger than a 64-byte cache line. The goal said to enforce the
documented claims; enforcing this one required updating the stale claim first.

## What I added

**Files modified:** `src/ir.rs`

**Changes:**
1. Updated stale module-level `# Cache-line note` to state the true sizes:
   `IrValue` = 8 bytes, `StaticValue` = 16 bytes, `Instr` = 80 bytes (with
   explanation of why — heap-allocated `String`/`Vec` fields in call variants).
2. Added `#[cfg(test)] mod tests` at the end of `src/ir.rs` with three tests:
   - `static_value_is_sixteen_bytes` — asserts `size_of::<StaticValue>() == 16`.
     Matches the module doc claim "fits in 16 bytes."
   - `ir_value_is_eight_bytes` — asserts `size_of::<IrValue>() == 8`.
     Largest variant is `I32(i32)` (4 bytes payload) → 8 bytes total.
   - `instr_size_is_documented` — asserts `size_of::<Instr>() == 80` with a
     size-history comment. Any future variant that grows the enum breaks this
     test, forcing a deliberate update decision.

All three tests pass. The model is now consistent across `lexer.rs` (token),
`ast.rs` (span), and `ir.rs` (three types).

## Notes for the goal-setter

**Structural finding:** The `Instr` enum at 80 bytes means a tight instruction
loop over `Vec<Instr>` strides across two cache lines per element. The 80-byte
layout is dominated by the call variants (`String` + two `Vec<u8>` = 72 bytes
of heap pointer/len/cap). A future milestone could box the call metadata
(`Box<CallData>`) to bring `Instr` back under 32 bytes — at the cost of one
extra pointer dereference for call instructions. Worth a goal when there is
evidence it matters in benchmarks (the `bench` CI job is the right place to
look first).

**Scope note:** Only `Instr`, `IrValue`, and `StaticValue` were in scope.
`IrTy` (unit enum, 1 byte) and `IrBinOp` (unit enum, 1 byte) are negligible
and have no module-level size claims — no tests added for them this round.
