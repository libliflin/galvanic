# Changelog — Cycle 018 (Customer Champion)

## Stakeholder: Lead Researcher

**Who I became.** The project's primary author — a systems programmer who runs galvanic daily and whose job is to advance FLS coverage. Their emotional signal is momentum: each run should tell them something true, and the assembly output is the research artifact.

**What I did.**
1. Confirmed the floor: `cargo test` — 2084 tests, all green. Build clean. Clippy clean.
2. Picked the loop expressions fixture (`tests/fixtures/fls_6_15_loop_expressions.rs`) — covers §6.15.2, §6.15.3, §6.15.6, §6.15.7.
3. Ran `cargo run -- tests/fixtures/fls_6_15_loop_expressions.rs`. Clean compile: "emitted fls_6_15_loop_expressions.s".
4. Opened the `.s` file. Searched for `§6.15` in the assembly comments. Zero matches.
5. Searched for all FLS citations. Found that every label (`.L0:`, `.L1:`), every conditional branch (`cbz`), and every unconditional branch (`b`) — all the control-flow instructions in the while loops, infinite loops, and break/continue — are annotated `FLS §6.17: branch target`, `FLS §6.17: branch if false`, `FLS §6.17: branch to end`.
6. Confirmed: §6.17 is if/if let expressions. The while loop belongs to §6.15.3.
7. Traced the root cause: `codegen.rs` lines 974–998 hardcode `§6.17` for all `Label`, `Branch`, and `CondBranch` instructions. The IR docstrings (`ir.rs` lines 440–482) describe these instructions exclusively in terms of §6.17 if-expression control flow, even though they're also used for all loop constructs.
8. Ran the litmus test with a parameter-driven while loop — the loop body executes correctly at runtime (runtime `add`, `cmp`, `b` instructions). The codegen is correct. The FLS citations in the comments are wrong.

**The worst moment.** Opening the `.s` file for the loop expressions fixture and finding `FLS §6.17` on every control-flow instruction. The fixture is explicitly titled `FLS §6.15 — Loop expressions`. The assembly correctly implements §6.15.3 while-loop behavior. But as a research artifact it claims §6.17 throughout. A researcher tracing §6.15 implementation through the assembly has no way to find it: the correct section is invisible.

**The goal set.** Add FLS provenance to the control-flow IR instructions (`Label`, `Branch`, `CondBranch`) so that assembly comments cite the correct FLS section — §6.15.3 for while loop branches, §6.15.2 for infinite loop back-edges, §6.15.6 for break, §6.15.7 for continue, §6.17 for if/if let — based on which lowering site emitted the instruction.

**Why now.** The Lead Researcher was last served at cycle 014 — the most under-served stakeholder. The issue is structural: the `Label`/`Branch`/`CondBranch` IR instructions were introduced for if-expression control flow and their §6.17 provenance was never updated when loop lowering reused them. Every loop-containing fixture has the same wrong citations. The assembly is correct; only the research traceability is broken.

---

# Verification — Cycle 017, Round 4 (Verifier)

## What I compared

- Goal: Four-file consistency for §6.9 AMBIGUOUS annotations (ast.rs / ir.rs / lower.rs / codegen.rs) — the Spec Researcher reads a clean, citable finding about bounds checking.
- Builder's Round 3 fixed `StoreIndexed` in `ir.rs` and `codegen.rs`. Verifier's Round 3 fixed `ast.rs`.
- Ran: `cargo test` (2084 pass), `cargo clippy -- -D warnings` (clean).
- Witnessed: grepped `FLS §4.9 AMBIGUOUS` and `FLS §6.9 AMBIGUOUS` across all four source files.

## What's here, what was asked

The four fixes from Rounds 1–3 are all present and correct. One additional defect found:

`src/lower.rs:18427–18428` read:
> "FLS §4.9 AMBIGUOUS: The FLS does not specify bounds checking. Galvanic omits bounds checking at this milestone."

Every other `§4.9 AMBIGUOUS` annotation in `lower.rs` (six sites) is about the `&[T]` fat-pointer ABI (two-register representation). A Spec Researcher tracking `§4.9` entries would encounter a bounds-checking note mixed in with ABI notes — it doesn't belong there thematically. And a Spec Researcher tracking `§6.9` entries would miss this code path's deferred bounds check entirely (since it cited `§4.9`).

The site is in the `local_slice_slots` branch — the code path that handles slice parameter indexing. `lower.rs:18341–18347` already carries a correct `§6.9 AMBIGUOUS` note describing this deferred case at the top of the match arm; the comment at 18427 was a redundant, mislabeled copy.

## What I added

Fixed `src/lower.rs:18427–18429`: corrected `§4.9` → `§6.9` and updated the comment to match the resolution pattern used by all other `§6.9 AMBIGUOUS` entries, with a cross-reference to the canonical note at the top of the match arm.

**Before:**
```
// FLS §4.9 AMBIGUOUS: The FLS does not specify bounds checking.
// Galvanic omits bounds checking at this milestone.
// FLS §6.1.2:37–45: All four instructions are runtime.
```

**After:**
```
// FLS §6.9 AMBIGUOUS: Slice parameters carry a runtime length, but
// galvanic does not yet use it for bounds checking here (deferred —
// see §6.9 AMBIGUOUS note at the top of this match arm for details).
// FLS §6.1.2:37–45: All four instructions are runtime.
```

- **Files:** `src/lower.rs`
- All tests pass (2084), clippy clean.
- All `§4.9 AMBIGUOUS` entries in `lower.rs` now exclusively address the `&[T]` fat-pointer ABI. Both `§6.9 AMBIGUOUS` entries address bounds checking. Thematic consistency holds.

## Notes for the goal-setter

- The §6.9 AMBIGUOUS sweep is now complete across all five annotation sites in the four source files.
- The "at this milestone" sweep (§4.14, §5.1.3, §6.22) flagged in earlier rounds remains a candidate for the next Spec Researcher cycle.
- None of the `§4.9 AMBIGUOUS` entries (fat-pointer ABI) have been touched; they are a separate ambiguity category and consistent with each other.
