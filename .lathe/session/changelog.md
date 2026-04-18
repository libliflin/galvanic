# Verification — Cycle 023, Round 1 (Verifier)

## What I compared

- **Goal on one side:** At back-edge branches (unconditional `Branch` whose target precedes it
  in the instruction array), emit "back-edge — cache: loop body = N instr × 4 B = K B, spans M
  cache line(s)". Makes the loop body footprint verifiable from the assembly alone.
- **Builder's change on the other:** Added `machine_instr_count()` to mirror `emit_instr`'s
  expansion logic; pre-scanned the body to build `label_cumulative`; intercept back-edges in
  the main emit loop and write the annotation inline; regenerated
  `tests/fixtures/fls_6_15_loop_expressions.s` with 11 back-edge annotations; added an e2e test
  asserting the 1-cache-line case (12 instr, 48 B, `while x < 5 { x = x + 1; }`).

**What I ran:**
- `cargo test` — 2108 pass, 0 fail ✓
- `cargo clippy -- -D warnings` — clean ✓
- `cargo run -- tests/fixtures/fls_6_15_loop_expressions.rs` → 11 back-edge annotations in .s ✓
- Manual instruction count for `while i < 5 { s = s+i; i = i+1; }`:
  ldr+ldr+BinOp(Lt)=2+cbz+ldr+ldr+BinOp(Add,I32)=3+str+ldr+LoadImm+BinOp(Add,I32)=3+str+b
  = 1+1+2+1+1+1+3+1+1+1+3+1+1 = **18 instructions** → annotation says 18 ✓
- Manual count for single-variable while (builder's test): **12 instructions** ✓
- Manual count for `loop { if i>=x { break; } continue; }`: **14 instructions** ✓
- Spot-checked `for_range_sum` back-edge (16 instrs): verified that the for-range increment
  lowers to a plain `add` (1 instr, not I32-overflow-guarded), consistent with
  `machine_instr_count` returning 1 for non-I32 BinOp(Add) ✓
- Confirmed forward branches (`cbz`, `b .L1` break) are NOT annotated as back-edges ✓

## What's here, what was asked

Matches: the builder's change reaches the goal. Every back-edge in the fixture carries a
concrete footprint claim. The annotation format is exactly what the goal specified.
The `machine_instr_count` function produces counts consistent with actual ARM64 instruction
emission across all loop types (while, for, loop, while_let, labeled).

**One narrow gap in the builder's tests:** the e2e test only asserts the 1-cache-line case.
The goal's hollow moment was specifically about a 2-cache-line body (the `while_loop` body:
18 instrs × 4 B = 72 B, spans 2 cache lines). An explicit test asserting `spans 2 cache
line(s)` was missing. Added below. Also added an explicit test that forward unconditional
break branches inside a loop body are not mislabeled as back-edges.

## What I added

**`tests/e2e.rs` — two additional tests:**

1. `fls_6_15_two_variable_while_spans_two_cache_lines`: compiles a two-variable while loop
   (accumulator + counter), asserts `loop body = 18 instr × 4 B = 72 B, spans 2 cache
   line(s)`. This is the loop shape that was the goal's hollow moment — now verifiable in
   under one test run.

2. `fls_6_15_forward_break_branch_not_annotated_as_back_edge`: compiles a `loop { i=i+1; if
   i>=3 { break; } }`, asserts the loop's back-edge IS annotated, and the break's forward `b`
   (FLS §6.15.6) is NOT annotated as a back-edge. Guards against the case where a forward
   unconditional branch inside a loop body could be misidentified.

**Files:** `tests/e2e.rs`

Test count: 2106 → 2108 (2 added, 0 failed).

## Notes for the goal-setter

- **Empty-body edge case:** The back-edge detection uses `header_cum < cumulative` (strict
  less-than). An empty loop body where the Branch immediately follows the Label would have
  `header_cum == cumulative`, and would NOT be annotated. Changing to `<=` would fix this
  without introducing false positives (forward branch targets always have
  `label_cumulative[target] > cumulative` since they come later in the stream). In practice,
  galvanic's current feature set always emits at least some instructions between a loop header
  and its back-edge, so this is a latent correctness note, not a present bug.

- **`machine_instr_count` and `emit_instr` are parallel implementations.** Any future change
  to `emit_instr` that changes instruction counts must be mirrored in `machine_instr_count` or
  the annotation will silently drift. A test that cross-checks the two functions' output counts
  for a battery of IR instructions would catch this drift — worth a future cleanup cycle.
