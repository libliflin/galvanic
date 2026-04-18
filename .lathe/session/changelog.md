# Verification — Cycle 023, Round 2 (Verifier)

## What I compared

- **Goal on one side:** At back-edge branches (unconditional `Branch` whose target precedes it
  in the instruction array), emit "back-edge — cache: loop body = N instr × 4 B = K B, spans M
  cache line(s)". Makes the loop body footprint verifiable from the assembly alone.
- **Round 1 verifier's changes on the other:** Two e2e tests added in round 1 —
  `fls_6_15_two_variable_while_spans_two_cache_lines` (18 instr → 2 cache lines) and
  `fls_6_15_forward_break_branch_not_annotated_as_back_edge` (forward break not mislabeled).

**What I ran:**
- `cargo test` — 2109 pass (was 2108), 0 fail ✓
- `cargo clippy -- -D warnings` — clean ✓
- `cargo run -- tests/fixtures/fls_6_15_loop_expressions.rs` → 11 back-edge annotations in .s ✓
- All 8 `fls_6_15_*` e2e tests pass ✓
- New boundary test passes: `fn foo(n: i32) -> i32 { for x in 0..n { acc = acc + x; } }` → 16 instr × 4 B = 64 B, spans 1 cache line(s) ✓

## What's here, what was asked

Matches from the builder's work. The round 1 verifier added the 2-cache-line case and the
forward-branch non-annotation guard. One remaining gap from my comparative lens:

**Missing test: the exact cache-line boundary (16 instr = 64 B = exactly 1 cache line).**
The existing tests cover under-boundary (12 instr, 48 B, 1 line) and over-boundary (18 instr,
72 B, 2 lines), but not the exact 64-byte case. At exactly 64 bytes, `div_ceil(64, 64) = 1` —
the annotation must say "spans 1 cache line(s)", not 2. The `for_range_sum` fixture demonstrates
this case but with no dedicated unit test to pin it. Added below.

## What I added

**`tests/e2e.rs` — one additional test:**

`fls_6_15_for_range_body_at_exact_cache_line_boundary`: compiles a for-range loop
(`fn foo(n: i32) -> i32 { for x in 0..n { acc = acc + x; } }`) with a main caller, asserts
`loop body = 16 instr × 4 B = 64 B, spans 1 cache line(s)`. This is the exact boundary:
`div_ceil(64, 64) = 1`, not 2. A for-range loop that fills exactly one cache line should
show one fill — the arithmetic at the boundary must not round up.

This completes the coverage set for the cache-line span annotation:
- 12 instr → 1 line (well under boundary) ✓
- 16 instr → 1 line (exact boundary) ✓  ← added this round
- 18 instr → 2 lines (over boundary) ✓

**Files:** `tests/e2e.rs`

Test count: 2108 → 2109 (1 added, 0 failed).

## Notes for the goal-setter

- The `machine_instr_count` / `emit_instr` parallel-implementation drift risk (noted by round 1
  verifier) remains the primary structural follow-up. Any future instruction added to `emit_instr`
  must be mirrored in `machine_instr_count`. A property-based test comparing both functions'
  output over a battery of IR instructions would be the structural fix.
- The empty-body edge case (`header_cum == cumulative`) from round 1 verifier's notes: still
  latent, still not a present bug given galvanic's current feature set.
- None other.
