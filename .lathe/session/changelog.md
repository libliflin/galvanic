# Changelog — Cycle 023, Round 1 (Builder)

## Goal
- At back-edge branches (unconditional `Branch` whose target precedes it in
  the instruction array), emit the loop body instruction count and cache-line
  span: "back-edge — cache: loop body = N instr × 4 B = K B, spans M cache
  line(s)". This makes the loop body footprint verifiable from the assembly
  alone — the same standard the prologue commentary already meets.

## Who This Helps
- **Stakeholder:** Cache-Line Performance Researcher
- **Impact:** The hollow moment identified in the goal — searching the emitted
  `.s` file for "cache" commentary and finding only "label has zero footprint"
  — is resolved. Every loop back-edge now carries a concrete footprint claim:
  instruction count, byte count, and cache-line span. The researcher can read
  the loop body footprint directly from the `.s` file without counting
  instructions by hand.

## Applied
- **`src/codegen.rs`**
  - Added `fn machine_instr_count(instr: &Instr) -> u32` — mirrors `emit_instr`
    logic to count ARM64 machine instructions per IR instruction. Labels → 0;
    simple instructions → 1; comparisons, indexed ops, and call sequences →
    their documented counts; overflow-guarded BinOp(Add/Sub/Mul, I32) → 3.
  - Modified `emit_fn` to pre-scan the body and build
    `label_cumulative: HashMap<u32, u32>`, mapping each label ID to the
    cumulative ARM64 instruction count at that point.
  - In the main emit loop, back-edge branches (where
    `label_cumulative[target] < cumulative`) are intercepted before
    `emit_instr`. The annotation format is:
    `FLS §X.Y: back-edge — cache: loop body = N instr × 4 B = K B, spans M cache line(s)`
    where N = (cumulative − header_cumulative + 1), K = N × 4,
    M = ceil(K / 64).
- **`tests/fixtures/fls_6_15_loop_expressions.s`**
  - Regenerated. All 11 back-edge branches now carry footprint annotations
    (e.g., `while_loop` body: 18 instr × 4 B = 72 B, spans 2 cache line(s)).
- **`tests/e2e.rs`**
  - Added `fls_6_15_back_edge_branch_carries_cache_line_annotation`:
    compiles a single-variable while loop, asserts the annotation text is
    present, and asserts the specific count (12 instr × 4 B = 48 B, spans 1
    cache line(s)).

## Validated
- `cargo test` — 2106 pass (was 2105), 0 fail.
- `cargo clippy -- -D warnings` — clean.
- `cargo run -- tests/fixtures/fls_6_15_loop_expressions.rs` — `.s` file
  contains 11 back-edge annotations.
- Manual spot-check: `while_loop` body = 18 instr × 4 B = 72 B, spans 2
  cache lines — matches the goal's hollow-moment example exactly.

## Where the verifier should look
- `grep "back-edge" tests/fixtures/fls_6_15_loop_expressions.s` — 11 hits,
  each with a concrete footprint claim.
- `cargo test --test e2e fls_6_15_back_edge` — 1 test, should pass.
- `tests/fixtures/fls_6_15_loop_expressions.s` line 31 — the `while_loop`
  back-edge branch: `18 instr × 4 B = 72 B, spans 2 cache line(s)`.
- `tests/fixtures/fls_6_15_loop_expressions.s` line 150 — `for_range_sum`'s
  inner loop: `16 instr × 4 B = 64 B, spans 1 cache line(s)` — exactly one
  cache line.
