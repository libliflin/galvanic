# Changelog — Customer Champion Cycle 010

## Stakeholder: The Spec Researcher

Walked steps 2–7 of the Spec Researcher journey. Floor intact (2058 pass, 0 fail).

**Rotation:** Spec Researcher last served in cycle 007 (3 cycles ago).

**What I tried:** Read `refs/fls-ambiguities.md` TOC. Picked §4.2 (newest entry, added
cycle-009). Searched source for annotation. Navigated to ref entry. Ran the minimal
reproducer.

**Worst moment:** The §4.2 entry has a complete-looking code block with `fn main()` and
a concrete assembly signature. Running it gives:
```
error: lower failed in 'main': not yet supported: field access on scalar value (field `x`)
```
The finding (inline struct storage in enum variants) is real and observable with a simpler
form — the reproducer just includes `v.x` field access that isn't implemented yet. The
entry gives no indication it's aspirational.

**Contrast:** §6.22 and §6.5.7 both have working reproducers that confirm their findings.
§4.2 looks the same but fails. The Spec Researcher has no way to distinguish broken
reproducers from working ones without trying them all.

**Goal:** Add a `refs_reproducers_all_compile` test that extracts all `fn main`-containing
rust code blocks from `refs/fls-ambiguities.md` and runs each through `compile_to_asm()`.
Fix the §4.2 reproducer (the new test forces this) by replacing `v.x` field access with a
wildcard match arm, and note the limitation explicitly in the entry.

**Why now:** The ref file is the Spec Researcher's primary artifact. One broken reproducer
without any "not yet demonstrable" warning is enough to make the whole file feel unreliable.
A CI test makes the invariant enforceable: every code block in refs either compiles or says
why it doesn't.

**File:** `.lathe/session/goal-history/cycle-010.md`
