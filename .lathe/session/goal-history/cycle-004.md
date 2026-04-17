# Changelog — Customer Champion Cycle 004

## Stakeholder: The Spec Researcher

**Became:** A Spec Researcher — an FLS contributor trying to find a specific,
citable finding they can take to a spec meeting. They arrived at galvanic
because it documents where the spec is silent.

**Rotation rationale:** Cycle 001 served the Spec Researcher (TOC + sort in
fls-ambiguities.md). Cycle 002 served the Compiler Contributor (named errors).
Cycle 003 served the Lead Researcher (all-errors summary line). The Spec
Researcher is next in rotation.

---

## Floor check

Build: OK. Tests: 2050 pass, 0 fail. Clippy: OK. Unsafe audit: OK. Floor intact.

---

## What I experienced

Walked the Spec Researcher journey, steps 4–8. Started at step 4:

```
grep -r 'AMBIGUOUS' src/ | grep '§6.5.3'
```

Found: `src/ir.rs:1476 — FLS §6.5.3 AMBIGUOUS: The spec does not specify NaN comparison behaviour.`

Navigated to `refs/fls-ambiguities.md`, found the §6.5.3 entry:

> **Galvanic's choice:** ARM64 `fcmp` sets flags per IEEE 754. `cset` then
> produces 0 or 1. NaN comparisons produce 0 for ordered comparisons (`<`, `>`,
> `<=`, `>=`) and 1 for `!=` — matching IEEE 754 but relying on hardware
> behavior rather than a spec guarantee.

**Source:** `src/ir.rs:1445`, `src/lower.rs:14875`

Note: the source line is stale — the annotation is actually at line 1476, not
1445. (All later ir.rs citations are off by approximately 31 lines — a block of
code was inserted above them after the ref file was last updated.) This caused a
moment of disorientation when navigating to the source.

But the more significant problem came at step 7: "Try to write a minimal Rust
program that demonstrates the ambiguity."

The ref entry says galvanic "relies on hardware behavior" — but gives no example
program that demonstrates this. I had to construct one from scratch:

```rust
fn main() -> i32 {
    let x: f64 = 0.0_f64 / 0.0_f64;
    if x != x { 1 } else { 0 }
}
```

This required knowing that (1) galvanic supports f64 division, (2) `0.0/0.0`
produces NaN in IEEE 754, and (3) f64 comparisons emit `fcmp`/`cset`. None of
this is in the ref entry. The researcher must already know these things to
construct the program.

The program compiled successfully. The emitted assembly confirmed the finding:

```asm
fdiv    d2, d0, d1           // produces NaN (0.0/0.0)
fcmp    d3, d4               // FLS §6.5.3: f64 compare
cset    x5, ne               // x5 = 1 (IEEE 754: NaN ≠ NaN is true)
```

**The worst moment:** The assembly confirmed exactly what the entry claimed —
but I had to do all the investigative work myself. The ref entry described the
finding without showing it. I found a specific, demonstrable thing the FLS
doesn't say — and the ref file that's supposed to document it gave me no
foothold to get there.

**The hollowest moment:** Reading the §6.5.3 entry after walking through the
assembly: the entry was correct. The finding is real. The resolution is
documented. But there was nothing I could take directly to a spec meeting and
say "here is the program, here is what galvanic emits, here is why this
confirms the gap." I had to produce all three myself.

This same gap exists for all 45 entries — the file documents what galvanic
chose but not how to observe the choice.

---

## Goal

**Add a `Minimal reproducer:` field to entries in `refs/fls-ambiguities.md`
where galvanic currently supports the required constructs and the behavior is
observable through assembly inspection.**

Each reproducer is a short Rust program (≤10 lines) and a one-line description
of what to look for in the emitted assembly (e.g., `fdiv + fcmp/cset ne` for
§6.5.3, or `cbz xRHS, _galvanic_panic` for §6.9 non-literal division). The
program should be minimal — just enough to trigger the behavior described by the
entry.

Not every entry needs a reproducer:
- Entries about ABI layout (vtable format, tuple-return calling convention) are
  better confirmed by reading the emitted assembly for any function that uses
  that construct — a note pointing to an existing test fixture is sufficient.
- Entries about constructs galvanic doesn't yet support (e.g., `+` overflow
  check, which is documented as a known deviation) should be marked
  `Reproducer: not yet demonstrable — requires §6.23 overflow guard
  (currently missing for +/-/*)`.

The format:

```
**Minimal reproducer:**
\`\`\`rust
fn main() -> i32 {
    let x: f64 = 0.0_f64 / 0.0_f64;
    if x != x { 1 } else { 0 }
}
\`\`\`
Look for: `fdiv` followed by `fcmp d3, d4; cset x5, ne` — confirms IEEE 754
hardware behavior is relied upon rather than explicit NaN detection.
```

**Why this is the most valuable change now:** The TOC and sort (cycle 001)
made findings navigable. The next barrier is verification — a Spec Researcher
cannot confirm a finding without constructing a reproducer from scratch. Adding
reproducers eliminates the entire "finding documented but not demonstrable"
category for the entries where galvanic has observable behavior. It transforms
the ref file from a description of choices into a reproducible record — the
difference between documentation and evidence.

**The specific moment:** Step 7 of the Spec Researcher journey — "Try to write
a minimal Rust program that demonstrates the ambiguity." For §6.5.3 NaN
comparison, the program I constructed took inference from three domains (float
arithmetic support, IEEE 754 NaN properties, ARM64 fcmp semantics). The ref
entry could have given me the program directly.
