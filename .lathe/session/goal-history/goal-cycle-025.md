# Goal — Cycle 025

**Stakeholder:** Spec Researcher

**What to change:** Fix the §6.5.7 (Shift Amount Modulo Behavior) entry in
`refs/fls-ambiguities.md`: (1) replace the stale **Galvanic's choice** field with an
accurate description of current behavior — galvanic emits an explicit range guard (`cmp
x{rhs}, #64` + `b.hs _galvanic_panic`) rather than delegating to hardware mod-64;
(2) remove the trailing Note that says the field is stale; (3) add a **Known gap**
paragraph documenting that shifts in [bit_width, 64) on narrow types (i32, i16, i8) are
not caught by the guard (a false negative documented in codegen.rs as `FLS §6.5.9
AMBIGUOUS`). Also update the `lower.rs` source annotation at line 11044 to reflect the
same change: the old frame ("ARM64 uses 6 bits for mod-64") is no longer accurate because
galvanic panics, not mods, on out-of-range shifts. Finally, add the narrow-type
shift-range false negative as a third gap in the §6.5.9 entry in `refs/fls-ambiguities.md`.

**Why this stakeholder:** Cycles 024 = Compiler Contributor, 023 = Cache-Line Researcher,
022 = Lead Researcher, 021 = Spec Researcher. Spec Researcher is the most under-served —
last served four cycles ago. The journey immediately surfaced a concrete, citable defect.

**Why now:** At step 4 of the Spec Researcher's journey — "try to find all findings
related to a topic in under two minutes" — I searched for float and numeric-operator
coverage. I found §6.5.3 (NaN Comparison) and §6.5.5 (IEEE 754): both clean, citable, and
complete. Then §6.5.7 (Shift Amount Modulo Behavior): not citable.

The entry has three parts that disagree with each other:

1. **Galvanic's choice** (formal field, line 553): "No explicit masking instruction is
   emitted; the ARM64 hardware behavior (implicit mod 64) satisfies the spec requirement
   for 64-bit types."

2. **Minimal reproducer assembly signature** (line 564): "look for `cmp x1, #64` followed
   by `b.hs _galvanic_panic` then `lsl x2, x0, x1` — galvanic panics for shift amounts ≥
   64 rather than relying on hardware mod-64 wrapping."

3. **Note** (line 570): "the `**Galvanic's choice**` description above is stale — galvanic
   now emits a range guard (panic if n ≥ 64), not a bare `lsl` relying on hardware
   behavior."

The formal description says "no guard, hardware mod." The assembly signature and Note say
"explicit guard, panics." A Spec Researcher cannot cite this entry — the authoritative
**Galvanic's choice** field is wrong, and the file itself acknowledges it.

Additionally: the source annotation that corresponds to this entry in `src/lower.rs` at
line 11044 still frames the ambiguity in the old terms ("ARM64 uses 6 bits of the shift
register for 64-bit shifts" — i.e. mod-64 delegation), while `src/codegen.rs` at line
1023 correctly frames the current design (`FLS §6.5.9 AMBIGUOUS: galvanic checks against
64, not bit_width`). Three parts of the project describe the same design decision in three
different ways. None of them agree.

**The class of fix:** A trailing Note that says "the **Galvanic's choice** field above is
stale" creates a research artifact that cannot be cited by anyone. It is worse than no
entry — the reader must hold two contradictory descriptions in mind and decide which is
true. The fix makes wrong states unrepresentable by rule: the **Galvanic's choice** field
is always accurate, and any stale-acknowledgment Note is a signal that the field must be
updated, not annotated. The same applies to source annotations: an AMBIGUOUS comment that
describes the old design after the new design was implemented should be updated in-place,
not left as historical artifact.

**What the fix looks like (the what, not the how):**

1. In `refs/fls-ambiguities.md`, §6.5.7 entry:
   - Update **Galvanic's choice** to describe current behavior: galvanic emits an explicit
     range guard for all shifts. `cmp x{rhs}, #64` + `b.hs _galvanic_panic` ensures shifts
     with amounts ≥ 64 panic at runtime. Hardware mod-64 behavior is deliberately NOT
     relied upon.
   - Add a **Known gap** note: for narrow integer types (i32, i16, i8) stored in 64-bit
     registers, shifts in [bit_width, 64) are not caught. A shift of an i32 value by 32
     should panic in debug mode, but galvanic's guard only checks ≥ 64. This is a known
     false negative documented in `codegen.rs`.
   - Remove the trailing "Note: the **Galvanic's choice** description above is stale"
     paragraph — it will no longer be true.

2. In `refs/fls-ambiguities.md`, §6.5.9 entry:
   - Add a third gap: the shift-amount narrow-type false negative. For i32/i16/i8, galvanic
     checks shift amounts against 64 but the valid range for i32 is [0, 31]. Shifts in
     [32, 63] on i32 values are accepted by galvanic but would panic in rustc debug mode.
     Source: `src/codegen.rs:1023-1028`.

3. In `src/lower.rs` at line 11044, the annotation:
   ```
   // FLS §6.5.7 AMBIGUOUS: the spec says the shift amount is taken modulo
   // the bit width, but does not specify the exact register width used for
   // the modulo (ARM64 uses 6 bits of the shift register for 64-bit shifts).
   ```
   Update to reflect the resolved design: galvanic chose to panic on out-of-range shifts
   (explicit range guard in codegen.rs), not to rely on hardware mod-64. Point to the
   remaining open gap: the narrow-type false negative (§6.5.9 AMBIGUOUS in codegen.rs).

**Constraint:** No functional code changes. This is documentation only — three parts of the
project describing the same design decision must agree on what the decision is. The goal
is citeability: after the fix, a Spec Researcher reading the §6.5.7 entry can write down
galvanic's choice without cross-referencing the source code to find out whether the entry
is accurate.

**Lived experience note:** I became the Spec Researcher. I opened `refs/fls-ambiguities.md`
and read the README (one sentence, two research questions — exactly as branded). I found the
TOC and confirmed it is in sync with the 48 body entries. I chose float and numeric-operator
semantics as my topic and set a two-minute clock. §6.5.3 (NaN): citable, found in 20
seconds. §6.5.5 (IEEE 754): citable, found in another 20 seconds. §6.5.7 (Shift Amount):
I read the **Gap** section — clear. I read **Galvanic's choice** — "No explicit masking
instruction is emitted." I wrote that down. I read the minimal reproducer assembly
signature — "look for `cmp x1, #64` + `b.hs _galvanic_panic`" — wait, that's a panic
guard, not a bare `lsl`. I re-read the choice field. Still says no guard. Then: "Note: the
**Galvanic's choice** description above is stale." The hollowest moment: I crossed out my
note. What is galvanic's actual choice? The Note says "range guard (panic if n ≥ 64)" —
but that's in a Note, not the formal field. Can I cite a Note? The three-part entry leaves
me knowing that something changed and that the formal description is wrong, but not what to
write in its place. I cannot bring this to a spec discussion. The entry is not citable.
