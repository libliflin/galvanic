# Goal — Cycle 017

**Stakeholder:** Spec Researcher

**What to change:** Update the §4.9 entry in `refs/fls-ambiguities.md` so that "Galvanic's choice" describes what galvanic actually does today (emits `cmp`/`b.hs` bounds checks before every array/slice access), moves the original decision to a "Historical note" subsection, and removes the contradictory patch note from the "Assembly signature" section.

**Why this stakeholder:** The last four goals served Compiler Contributor (cycle 016), Cache-Line Performance Researcher (cycle 015), Lead Researcher (cycle 014), and Spec Researcher (cycle 013). Spec Researcher was last served four cycles ago — the most under-served stakeholder.

**Why now:** The §4.9 entry's "Galvanic's choice" section says:
> No bounds check is emitted at this milestone. Out-of-bounds access produces undefined behavior at the assembly level.

The entry's "Assembly signature" note then immediately contradicts this:
> bounds checks **are** now emitted (see §6.9/§6.23 entry for the full mechanism added in later claims). The §4.9 entry documents the original decision before the panic infrastructure was added.

I ran `cargo run -- /tmp/test_bounds.rs` on `fn get(arr: [i32; 3], i: usize) -> i32 { arr[i] }` and inspected the emitted assembly. Two `cmp`/`b.hs` instructions appear before the `ldr` — confirming bounds checks ARE emitted. The "Galvanic's choice" section is wrong about the current state.

A Spec Researcher reading §4.9 cannot cite this finding. The body says "No bounds check" and the note says "bounds checks ARE now emitted." They arrive at a finding and leave more confused than before — exactly the emotional signal (curiosity NOT satisfied) the Spec Researcher persona defines as the worst outcome.

**The specific moment:** Step 3 of the Spec Researcher's journey — "Read one finding end-to-end." §4.9 fails this step entirely. The "Galvanic's choice" section is the primary claim a researcher reads to understand what galvanic does; it must be accurate. Burying the correction in the "Assembly signature" section — a detail section — makes the finding uncitable.

**The class of fix:** When "Galvanic's choice" changes, the choice section must be updated (not just patched via a note in another section). The fixed §4.9 entry becomes the template:
- "Galvanic's choice (current):" — describes what the implementation does today, with a cross-reference to §6.9/§6.23 for the full mechanism
- "Historical note:" — records the original decision and the cycle/claim that changed it, so the research trail is preserved without corrupting the primary finding
- "Assembly signature:" — describes only what to look for in current output, with no contradictory caveats

**Lived experience note:** I became the Spec Researcher. I opened `refs/fls-ambiguities.md`, found the TOC (present and navigable — this is solid), and navigated to §4.9 to understand galvanic's bounds-checking posture. I read "Galvanic's choice: No bounds check is emitted at this milestone." I started to take notes for a spec discussion. Then I read the assembly signature: "bounds checks **are** now emitted." I stopped. I read back to "Galvanic's choice." I read forward again. I had no idea which statement to believe. I went to §6.9/§6.23 and found the current behavior clearly described there. So the information exists — it's just in the wrong entry, and §4.9 now actively misleads. The hollowest moment: realizing I'd have to read two entries, compare them, infer the history, and reconstruct the current state — for something the document is supposed to answer directly.
