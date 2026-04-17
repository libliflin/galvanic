# Changelog — Customer Champion Cycle 004

## Stakeholder: The Spec Researcher

**Became:** A Spec Researcher trying to find a citable finding to take to a
spec meeting. Walked steps 4–8 of the Spec Researcher journey.

## What I experienced

Traced §6.5.3 (NaN comparison) from source annotation to ref entry to
minimal reproducer. The annotation at `src/ir.rs:1476` links to a ref entry
that describes galvanic's choice (relies on ARM64 `fcmp` / IEEE 754 hardware
behavior) but includes no example program.

Constructed the reproducer myself:
```rust
fn main() -> i32 {
    let x: f64 = 0.0_f64 / 0.0_f64;
    if x != x { 1 } else { 0 }
}
```

It compiled. The emitted assembly confirmed the finding (`fdiv` + `fcmp/cset
ne`). But that required knowing float division was supported, that `0.0/0.0`
produces NaN, and how to read the assembly — none of which is in the ref entry.

Found that all 45 entries have this gap: findings are described but not
demonstrated. The source line citations are also stale (all later `ir.rs`
references are off by ~31 lines).

## Goal set

**Add `Minimal reproducer:` fields to entries in `refs/fls-ambiguities.md`
where galvanic supports the required constructs and the behavior is observable
via assembly inspection.**

Each reproducer: a ≤10-line Rust program + one-line description of the
assembly signature to look for. Entries where galvanic doesn't yet support the
required construct are marked "Reproducer: not yet demonstrable — requires X."

## Why now

The TOC (cycle 001) made findings navigable. The next barrier is verification.
Without a reproducer, the Spec Researcher must construct one from scratch,
guessing whether galvanic supports the required constructs. Adding reproducers
transforms the ref file from a description of choices into a reproducible
record — the difference between documentation and evidence.

Specific moment: Step 7 of the Spec Researcher journey, §6.5.3. The program
required inference from three domains to construct; the ref entry could have
provided it directly.
