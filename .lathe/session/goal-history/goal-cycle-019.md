# Goal — Cycle 019

**Stakeholder:** Cache-Line Performance Researcher

**What to change:** Emit cache-line commentary at key structural points in the assembly output — function entry, loop headers, `.align` directives in the data/rodata section, and the `_galvanic_panic` handler — so that the assembly file itself shows evidence of cache-line reasoning, not just FLS section citations.

**Why this stakeholder:** Last served at cycle 015 (four cycles ago — the most under-served stakeholder). Cycles 016, 017, 018 served Compiler Contributor, Spec Researcher, and Lead Researcher respectively.

**Why now:** The README claims galvanic's codegen is "obsessively cache-line-aware." The architecture document says "Cache-line reasoning lives here [codegen.rs]: how many instructions fit in a cache line, where `.align` directives are needed, how to lay out `_start` and function prologues for minimal cache pressure." The `codegen.rs` source is full of this reasoning — inline comments like "Cache-line note: `sub sp, sp, #N` is one 4-byte instruction — the frame setup occupies one slot in the first cache line of the function body" appear on dozens of lines.

None of it surfaces in the emitted assembly.

**The specific moment:** Step 4 of the Cache-Line researcher's journey — "Compile a test program and inspect the emitted `.s` file for cache-line commentary." I compiled `tests/fixtures/fls_6_15_loop_expressions.rs` (a loop-heavy fixture where cache-line discipline is most visible) and opened the `.s` file. Every instruction has an FLS section citation. I searched for "cache", "align", "line". Zero results. The `.align` directives in the data section exist but have no comment explaining why alignment matters there. The loop header label `.L0:` has `// FLS §6.15.3: branch target` but says nothing about where it falls relative to a cache-line boundary. The prologue `sub sp, sp, #32` has `// FLS §8.1: frame for 3 slot(s)` but no mention of its instruction-cache footprint.

If you showed this assembly to someone who hadn't read the source code, they would have no idea cache-line alignment was considered. The FLS reasoning is present and rich. The cache-line reasoning is invisible.

**The class of fix:** The cache-line thesis is currently private to `codegen.rs` source comments. These comments already exist and are well-reasoned. They need to surface in the emitted output — the primary research artifact — not just live as internal developer notes. The builder should determine which subset of the existing codegen.rs cache-line commentary belongs in the output and at which structural points. Suggested candidates:

1. **Function entry** — a comment on the prologue instruction: `// cache: N-instruction prologue (~N×4 bytes)`
2. **Loop headers** — a comment on the back-edge branch explaining its instruction-cache position within the loop body
3. **`.align` directives in `.data`/`.rodata`** — a comment explaining the alignment is for cache-line slot packing, not just ABI requirement
4. **`_galvanic_panic`** — a comment noting its 3-instruction / 12-byte / fits-in-one-cache-line footprint

The invariant to achieve: opening any emitted `.s` file, a researcher who reads only the assembly (not `codegen.rs`) can understand that cache-line decisions were made and what they were. The FLS citations tell the language story; the cache-line comments tell the hardware story.

**Constraint:** Do not add a cache-line comment to every instruction — that would overwhelm the FLS citations. Add them at structural decision points where the cache-line reasoning is non-obvious (prologue, loop structure, data alignment). The output should remain readable.

**Lived experience note:** I became the Cache-Line Performance Researcher — someone evaluating galvanic's thesis for their own compiler project. I read the README (claim is prominent and clear). I ran `cargo bench` (throughput in MiB/s, Criterion output, solid). I found the size tests (`token_is_eight_bytes`, `instr_size_is_documented` — all pass). Then I compiled a program and opened the `.s` file. I searched for cache-line evidence. Nothing. The assembly is an FLS citation machine — precise, well-structured, informative about language semantics. But the cache-line claim — the thing that makes this project distinct — is invisible in the output. The hollowest moment: reading "Cache-line note: `sub sp, sp, #N` is one 4-byte instruction — the frame setup occupies one slot in the first cache line of the function body" in `codegen.rs`, then opening the emitted assembly and seeing `sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)`. The reasoning exists. It was considered. It just never left the source file.
