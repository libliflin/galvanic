# Changelog — Cycle 017 (Customer Champion)

## Stakeholder: Spec Researcher

**Who I became.** A compiler author at another shop who found galvanic via a GitHub search for "FLS ambiguity." They're not implementing galvanic — they're reading its research artifacts. They want concrete, citable findings about spec gaps. Their emotional signal: curiosity satisfied.

**What I did.**
1. Confirmed the floor: `cargo test` — all tests pass. Build clean. Clippy clean.
2. Opened `refs/fls-ambiguities.md`. Found the TOC (48 entries, section-number order — solid). Navigated via TOC links.
3. Chose §4.9 (Bounds Checking Mechanism) to read end-to-end as a citable finding.
4. Read "Galvanic's choice: No bounds check is emitted at this milestone." Started forming a citation.
5. Continued reading the "Assembly signature" note: "bounds checks **are** now emitted (see §6.9/§6.23 entry for the full mechanism added in later claims)."
6. Ran `cargo run -- /tmp/test_bounds.rs` on `fn get(arr: [i32; 3], i: usize) -> i32 { arr[i] }`. Inspected the emitted `.s` file: two `cmp`/`b.hs` instructions before the `ldr`. Bounds checks ARE emitted.
7. Confirmed §5.1.8 ("not yet demonstrable — rest patterns inside slice patterns") is still accurate: running a rest-pattern slice test produces a parse error.

**The worst moment.** Reading §4.9's "Galvanic's choice: No bounds check is emitted" — then reading in the same entry that "bounds checks **are** now emitted." Two statements in one entry, directly contradictory. The "Galvanic's choice" section describes a historical decision that was reversed; the correction is buried in the "Assembly signature" note. A researcher trying to cite this finding cannot trust either statement without going to §6.9/§6.23 and reconstructing the history themselves.

**The goal set.** Update §4.9's "Galvanic's choice" section to describe current behavior (cmp/b.hs bounds check before every array/slice access, per Claims 4m/4p), move the original no-bounds-check decision to a "Historical note" subsection, and remove the contradictory assembly signature note. The fixed entry becomes the template: "Galvanic's choice (current):" for what the implementation does now, "Historical note:" for the research trail.

**Why now.** The Spec Researcher was last served at cycle 013 (four cycles ago). The §4.9 entry is the one finding in the document where "Galvanic's choice" is demonstrably wrong about current behavior — not hedged ("at this milestone") but outright contradicted within the same entry. Every Spec Researcher who reads it hits the same wall.
