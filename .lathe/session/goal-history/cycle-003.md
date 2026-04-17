# Changelog — Customer Champion Cycle 003

## Stakeholder: The Lead Researcher

**Became:** The Lead Researcher — the author using galvanic as a research instrument to answer whether the FLS is independently implementable, extending the compiler feature by feature.

**Rotation rationale:** Cycle 001 served the Spec Researcher. Cycle 002 served the Compiler Contributor. The Lead Researcher has not been served in either of the last two cycles. Serving them now.

---

## Floor check

Build: OK. Tests: 2048 pass, 0 fail. Clippy: OK. Unsafe audit: OK. Floor intact.

---

## What I experienced

Walked step 3 of the Lead Researcher journey: picked a parse-only fixture to drive toward e2e coverage. Chose `fls_6_18_match_expressions.rs` — the most semantically rich of the 5 parse-only fixtures, with 11 functions covering literal patterns, guards, bool scrutinee, or-patterns, enum variants, tuple variants, named variants, match-in-let, tuple scrutinee, range patterns, and nested match.

```
cargo run -- tests/fixtures/fls_6_18_match_expressions.rs
```

Output:
```
galvanic: compiling fls_6_18_match_expressions.rs
parsed 16 item(s)
error: lower failed in 'match_tuple': not yet supported: tuple expression must be bound to a `let` variable at this milestone
```

The error names the failing function (good — that's the cycle 002 improvement). But it stops there. The output gives no indication of how many of the other 10 functions compiled successfully.

To find out whether the other functions worked, I had to manually split the fixture into three test files and run them separately. The result:

- `match_i32_literals`, `match_as_value`, `match_bool`, `match_or_pattern` — all compile ✓
- `match_unit_enum`, `match_tuple_variant`, `match_named_variant`, `match_in_let` — all compile ✓
- `match_ranges`, `match_guard_uses_binding`, `nested_match` — all compile ✓
- `match_tuple` — **fails** (tuple scrutinee not supported) ✗

**10 of 11 functions compile.** The fixture is one function away from full e2e coverage. But the output gave no indication of this.

**The worst moment:** Running the fixture and seeing "error: lower failed in 'match_tuple'" with no sense of progress. The output makes "1 of 11 fails" look identical to "11 of 11 fail." The momentum signal is completely suppressed.

**The hollowest moment:** The fixture file is well-structured — each function has clear FLS citations and tests one specific construct. But the compiler's output discards all of that structure. "Parsed 16 items" is the last successful progress marker before the error. The researcher has to do manual archaeology to recover what the output should have said.

---

## Goal

**When lowering a source file with multiple top-level items, galvanic should report all per-function lower errors (not just the first), and print a summary line stating how many items succeeded and how many failed.**

Before (current behavior):
```
galvanic: compiling fls_6_18_match_expressions.rs
parsed 16 item(s)
error: lower failed in 'match_tuple': not yet supported: tuple expression must be bound to a `let` variable at this milestone
```

After (desired behavior):
```
galvanic: compiling fls_6_18_match_expressions.rs
parsed 16 item(s)
error: lower failed in 'match_tuple': not yet supported: tuple expression must be bound to a `let` variable at this milestone
lowered 10 of 11 functions (1 failed)
```

(Non-function items — struct defs, enum defs, type aliases — are not counted in the function tally since they don't fail in isolation.)

If there is more than one failing function, all errors are reported before the summary:
```
error: lower failed in 'fn_a': not yet supported: ...
error: lower failed in 'fn_b': not yet supported: ...
lowered 9 of 11 functions (2 failed)
```

When all functions succeed, no summary line is printed (the existing "galvanic: emitted" line serves this purpose).

**No assembly is emitted when any function fails** — the exit code remains 1, the `.s` file is not written. This is not a partial-compilation feature. It is a diagnostic feature that shows the full error landscape in a single run.

**Why this is the most valuable change right now:** The Lead Researcher's primary signal is momentum. Momentum comes from knowing the compiler is getting smarter — new constructs working, coverage expanding. When the output treats "1 of 11 functions fails" the same as "all 11 fail," the researcher cannot read their own progress. They have to do manual fixture surgery to recover information the compiler already had. This is a class-level fix: it eliminates "how much of this fixture works?" as a question for every parse-only fixture, now and in the future.

**The specific moment:** Step 3 of the Lead Researcher journey, running `galvanic tests/fixtures/fls_6_18_match_expressions.rs`. The error output correctly names the failing function (cycle 002's improvement) but provides no count of successes — leaving the researcher unable to tell they were 10/11 of the way to full coverage.
