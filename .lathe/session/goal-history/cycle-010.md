# Changelog — Customer Champion Cycle 010

## Stakeholder: The Spec Researcher

**Became:** A Spec Researcher trying to audit FLS findings in galvanic — reading
`refs/fls-ambiguities.md`, picking a section, running the minimal reproducer, checking
if the output confirms the finding.

**Rotation rationale:** Cycle 007 served the Spec Researcher. Cycle 008 served the
Compiler Contributor. Cycle 009 served the Lead Researcher. The Spec Researcher was last
served 3 cycles ago — their turn.

---

## Floor check

Build: OK. Tests: 2058 pass, 0 fail. Clippy: OK. Unsafe audit: OK. Floor intact.

---

## What I experienced

Walked steps 2–7 of the Spec Researcher journey. The TOC is 47 entries, navigable.
Picked §4.2 — newly added in cycle-009, the most recently documented finding.
Searched source: `grep -r 'AMBIGUOUS.*§4\.2\|§4\.2.*AMBIGUOUS' src/` → found the annotation
at `src/lower.rs:9487`.

Step 7: ran the minimal reproducer from refs:

```rust
struct Foo { x: i32 }
enum Maybe<T> { Some(T), None }
fn main() -> i32 {
    let m = Maybe::Some(Foo { x: 7 });
    match m { Maybe::Some(v) => v.x, Maybe::None => 0 }
}
```

Output:
```
error: lower failed in 'main': not yet supported: field access on scalar value (field `x`)
```

**The worst moment:** The entry looks complete — it has a Gap, a Galvanic's choice, a
Source reference, a Minimal reproducer with code, and an Assembly signature. A Spec
Researcher reads it and expects to run the code and see the finding confirmed. They get
a compile error instead. There is no indication in the entry that the reproducer is
aspirational (for a future state of the compiler) rather than current.

Verified that the _simpler_ form works — `Maybe::Some(Foo { x: 7 })` with a wildcard
arm in the match compiles and emits `str x1, [sp, #8]` confirming the inline layout.
The finding is real and demonstrable; the reproducer just goes one step further than
the current implementation allows (`v.x` field access on a match-bound struct variable
is not yet implemented).

Also spot-checked §6.22 (closure capture) and §6.5.7 (bitwise AND disambiguation) —
both reproducers compile and demonstrate their findings correctly.

**The hollowest moment:** The §4.2 entry was added alongside the cycle-009 feature
implementation and states an assembly signature ("look for `str w<N>` after storing
discriminant — confirms `x = 7` is stored inline"), but the reproducer that would
let you observe that signature doesn't compile. The Spec Researcher cannot get to the
assembly inspection step.

---

## Goal

**Add a test that compiles all `fn main`-containing rust code blocks in
`refs/fls-ambiguities.md`, and fix the §4.2 reproducer to use a form that works today.**

### Part 1: Add a refs-reproducers test

Add a new test in `tests/e2e.rs` (or a new `tests/refs_reproducers.rs`) named
`refs_reproducers_all_compile`:

- Read the file `refs/fls-ambiguities.md` as a string (relative to the workspace root,
  using `include_str!` or `std::fs::read_to_string` with the path relative to the
  cargo manifest dir).
- Extract all ```` ```rust ... ``` ```` blocks that contain `fn main(` — these are
  executable reproducers, not API snippets.
- For each extracted block, call `compile_to_asm(block)` (the existing helper in
  `tests/e2e.rs`). Wrap with a name so failures identify which block failed:
  `assert!(compile_to_asm(src).contains("main:"), "reproducer block N failed:\n{src}")`.
- If any block panics or returns an error, the test fails.

This test eliminates the whole class of "reproducer added for future state, silently
breaks the Spec Researcher journey." Every new cycle that adds a reproducer to refs
must ensure the code compiles — CI catches it immediately.

### Part 2: Fix the §4.2 reproducer

When the test above runs, the §4.2 entry will fail immediately. Fix it by replacing
the current reproducer (which uses `v.x` field access that's not yet implemented) with:

```rust
struct Foo { x: i32 }
enum Maybe<T> { Some(T), None }
fn main() -> i32 {
    let m = Maybe::Some(Foo { x: 7 });
    match m { Maybe::Some(_) => 1, Maybe::None => 0 }
}
```

Update the Assembly signature in the §4.2 entry:

> **Assembly signature:** Look for `mov x1, #7` followed by `str x1, [sp, #8]` —
> the struct's `x` field is stored at the second stack slot (slot 1), immediately
> after the discriminant at slot 0. This confirms inline storage: no pointer
> indirection. A form using `v.x` field access after matching is not yet
> implemented; that extension is tracked as future work.

Add the sentence about `v.x` not yet implemented so the limitation is visible to
future readers.

### Why Part 2 cannot be skipped

The test in Part 1 will fail on the current §4.2 entry. The builder must fix the
reproducer to make the test pass. Documenting the limitation honestly means the Spec
Researcher understands what IS verifiable now vs. what requires future implementation.

### Why this is the most valuable change right now

The Spec Researcher's signal is **Discovery** — a specific, grounded finding they can
take to spec authors. "I can take this to the spec authors" requires the finding to be
verifiable, not just documented. The §4.2 entry breaks this: it's the only entry that
has a runnable code block but fails at compile time (entries that say "Not yet
demonstrable" are honest; this one isn't).

The test eliminates the class, not just the instance. Future cycles will add reproducers
as features grow (cycle-009 just did exactly this). CI catching broken reproducers at
the moment they're added — rather than when a Spec Researcher happens to try them —
keeps the ref file trustworthy.

### The specific moment

Step 7 of the Spec Researcher journey: "Run `galvanic <program.rs>` — does the output
confirm the finding?" Picked the §4.2 entry (newest, most recently added finding).
Ran the reproducer. Got:
```
error: lower failed in 'main': not yet supported: field access on scalar value (field `x`)
```
The entry looks complete but the finding cannot be verified.
