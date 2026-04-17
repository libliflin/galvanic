# Goal: emit partial assembly when lowering partially succeeds

## Stakeholder: The Lead Researcher

## What to change

When `lower()` partially succeeds — some functions lower cleanly, others fail — galvanic
should emit assembly for the functions that lowered successfully, alongside the error
messages for those that failed.

Currently, `LowerErrors` carries only the error list and counts; the successfully-lowered
`fns` vector is discarded when the function returns `Err`. The caller in `main.rs` receives
only the error and produces no output.

The change: carry the partial module (the successfully-lowered functions) inside `LowerErrors`,
so that `main.rs` can emit assembly for what worked while still reporting what failed. The
exit code should remain non-zero when any function failed, but the `.s` file should be
written for the functions that succeeded.

## Why this stakeholder, why now

The Lead Researcher's job — at step 6/7 of their journey — is to compile a fixture
end-to-end, then read the emitted assembly to check:
- Do the match arms emit runtime comparison instructions (not constant-folded)?
- Do the branch targets use the correct ABI registers?
- Are there any FLS findings to capture?

This cycle I ran:

```
cargo run -- tests/fixtures/fls_6_18_match_expressions.rs
```

Output:
```
galvanic: compiling fls_6_18_match_expressions.rs
parsed 16 item(s)
error: lower failed in 'match_tuple': not yet supported: tuple expression must be bound to a `let` variable at this milestone
lowered 12 of 13 functions (1 failed)
```

12 of 13 functions lowered successfully — literal patterns, guard patterns, enum variant
patterns, or-patterns, range patterns, named-field variant destructuring, nested match,
match-in-let. All of these emit real runtime instructions and warrant inspection. But no
`.s` file was produced. I looked for the assembly file and found nothing.

The hollowest moment: reading "lowered 12 of 13" and finding no artifact. The compiler did
92% of the work and showed me 0% of the result.

This eliminates a whole class of friction. Every parse-only fixture is likely in this state:
it parses, most functions lower, one construct is unsupported, and the researcher gets
nothing for the partial success. With partial output:
- The researcher can inspect match pattern assembly right now, before tuple-scrutinee is implemented.
- They can verify the FLS §6.1.2 constraint (runtime instructions, not constant-folded results).
- They can document cache-line behavior of the pattern-matching IR nodes.
- They can capture FLS findings from the working functions.

The fix does not require implementing tuple-scrutinee match — it makes the existing partial
success visible.

## Lived-experience note

**Stakeholder:** Lead Researcher.

**Journey walked:** I ran `cargo test` (2051 passed, floor intact), then picked
`fls_6_18_match_expressions.rs` as the most substantive parse-only fixture — 13 functions
covering the full §6.18 surface: literal patterns, guards, boolean scrutinee, or-patterns,
enum unit/tuple/named-field variants, match-in-let, tuple scrutinee, range patterns,
guard-with-binding, nested match. I ran `cargo run --` on it and hit the partial failure.

**What I verified first:** I compiled a simpler match program to confirm match expressions
already emit correct runtime code — `cmp`, `cset`, `cbz`, branch labels, stack loads. The
assembly is right. No constant folding. ABI-correct. The 12 that lowered are real output.

**What the failure was:** `match_tuple` uses `(x, y)` as a scrutinee — a tuple expression,
which is a separate feature from pattern matching. The other 12 functions don't need it.

**The broken moment:** Knowing 12 functions lowered successfully and having no assembly
to show for it. The researcher can't learn anything from "12 of 13 failed" because the
12 that worked are invisible.

**Emotional signal:** Momentum broke not at the failure, but at the silence. The failure
message was clear. The missing artifact is what stopped progress.
