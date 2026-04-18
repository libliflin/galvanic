# Goal — Cycle 023

**Stakeholder:** Cache-Line Performance Researcher

**What to change:** At each back-edge branch (an unconditional `Branch` whose target label precedes it in the instruction stream), emit the loop body instruction count and cache-line span in the assembly comment — replacing the current generic "FLS §X.Y: branch" with "FLS §X.Y: back-edge — cache: loop body = N instr × 4 B = K B, spans M cache line(s)". Remove or replace the loop header label comment "cache-line: label has zero footprint" (trivially true, zero signal) with something minimally useful, or simply drop the cache-line annotation from labels since the back-edge branch now carries the relevant measurement.

**Why this stakeholder:** Cycles 022 = Lead Researcher, 021 = Spec Researcher, 020 = Compiler Contributor, 019 = Cache-Line Researcher. Cache-Line Researcher is next in rotation (last served 4 cycles ago).

**Why now:** Step 4 of the Cache-Line researcher's journey — "Compile a test program and inspect the emitted `.s` file for cache-line commentary" — surfaces the exact friction. The loop body instruction footprint is the most cache-line-relevant measurement in any emitted assembly file. It is currently absent.

Every other structural point in the assembly has specific, countable cache-line commentary:

- Prologue: "cache-line: prologue = 1 instr(s) × 4 bytes = 4 bytes — 1 of 16 slots in first cache line" — specific, verifiable.
- `_start`: "cache-line: _start = 3 instructions × 4 bytes = 12 bytes — fits in one 64-byte cache line" — specific, verifiable.
- `_galvanic_panic`: "cache-line: _galvanic_panic = 3 instructions × 4 bytes = 12 bytes — fits in one 64-byte cache line" — specific, verifiable.
- Loop header label: "cache-line: label has zero footprint" — trivially true. Says nothing about the loop body.

The while_loop function in `tests/fixtures/fls_6_15_loop_expressions.s` has an 18-instruction loop body (72 bytes), which spans 2 cache lines. There is no commentary anywhere in the emitted assembly that reflects this. The back-edge branch (line 31: `b .L0`) says only "FLS §6.15.3: branch." A researcher who wants to know whether galvanic considered the cache-line footprint of this loop body has no data.

The source code in `codegen.rs` line 1023–1025 even states the intent: "Marking them 'loop boundary' surfaces galvanic's cache-line thesis in the emitted output: every loop header and back-edge is a cache-line-relevant boundary where the instruction stream may cross a 64-byte cache line." The intent is exactly right. The output doesn't deliver it — "label has zero footprint" is about the label, not the boundary.

**The specific moment:** Step 4 of the journey. I compiled `tests/fixtures/fls_6_15_loop_expressions.rs` and opened the `.s` file. I searched for "cache" to find all cache-line commentary. I found it at every prologue, every `_start`, the `_galvanic_panic` handler. Then I found it at the loop labels: "cache-line: label has zero footprint." I looked at the 18-instruction while_loop body and asked: does the commentary tell me whether this loop fits in one cache line? It does not. The commentary tells me the label costs zero bytes — a fact I knew before opening the file. The back-edge branch `b .L0` says "FLS §6.15.3: branch" and nothing more. The cache-line thesis — the thing that makes galvanic different from any other toy compiler — is invisible at the most cache-critical place in the output.

**How to implement (the what, not the how):**

Before emitting a function's instructions, pre-scan `func.body` to build a mapping: for each label ID that is the target of a backward branch (i.e., the `Branch` instruction appears after the `Label` instruction in the array), record the index of the `Label` and the index of the `Branch`. The loop body instruction count is `branch_index - label_index - 1` (excluding the label and the branch itself). Pass this map into `emit_instr` (or inline the lookup at the `Branch` emission site) so that when a back-edge `Branch` is emitted, the comment carries:

```
// FLS §6.15.x: back-edge — cache: loop body = N instr × 4 B = K B, spans M cache line(s)
```

Where: `K = N × 4`, `M = ceil(K / 64)`.

For a forward branch (target label comes after the branch), the current comment "FLS §X.Y: branch" is appropriate — no change needed there.

The loop header label comment "cache-line: label has zero footprint" should either be dropped from loop-boundary labels, or replaced with something that anchors the loop start without making a trivially-true claim about byte cost. The back-edge comment now carries the measurement; the header label just needs to be readable.

**The invariant to achieve:** Every loop in the emitted assembly has its body instruction count and cache-line span visible at the back-edge branch. A researcher opening any `.s` file can determine for each loop: "the body is N instructions, spans M cache lines." This makes the core cache-line claim verifiable at the hot path — not just at the prologue and panic handler.

**Constraint:** Do not count label-only instructions (they have zero footprint) or the back-edge branch itself. Count only the instructions between the loop header label and the back-edge branch. The count should reflect actual ARM64 instructions emitted, not IR node count.

**Lived experience note:** I became the Cache-Line Performance Researcher — someone evaluating whether galvanic's cache-line thesis shows up in the output, not just in the source comments. I read the README (claim: "obsessively cache-line-aware" — clear). I ran `cargo bench` — throughput appeared: 670 MiB/s for the lexer. I ran the size tests — all passed, including the `Instr` size history noting the 80-byte growth. I compiled the loop fixture and opened the `.s` file. I searched for "cache." Found it: prologues (specific counts), _start (specific count), _galvanic_panic (specific count), loop labels (zero footprint). The prologue commentary is a model — it tells me "1 instruction × 4 bytes = 4 bytes — 1 of 16 slots in the first cache line." I can verify that claim. The loop label commentary says "label has zero footprint" — I didn't need the assembly file to tell me that. I looked at the 18-instruction while loop body: 72 bytes, spanning 2 cache lines. No mention of this anywhere. The hollowest moment: reading "Marking them 'loop boundary' surfaces galvanic's cache-line thesis in the emitted output: every loop header and back-edge is a cache-line-relevant boundary where the instruction stream may cross a 64-byte cache line" in `codegen.rs` line 1023–1025, then looking at the emitted output and finding "label has zero footprint." The intention is exactly right. The output doesn't deliver it.
