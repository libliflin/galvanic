# Goal — Cycle 027

**Stakeholder:** Cache-Line Performance Researcher

**What to change:** Extend `benches/throughput.rs` to add a `full_pipeline` benchmark group
that measures the complete compilation pipeline — lex → parse → lower → emit_asm — reporting
throughput in bytes/sec. Use the same fixtures already present (`fls_functions`,
`fls_expressions`) and a stress variant (`n=100` and `n=1_000` let bindings). Mirror the
existing `end_to_end` group's structure: one `BenchmarkId` per fixture, `Throughput::Bytes`,
same warm-up and sample counts as the rest of the bench suite.

Do NOT rename or remove the existing `end_to_end/lex_and_parse` benchmarks — they are used
for historical comparison in Criterion's HTML reports. Add the new benchmarks as a new
group called `full_pipeline` (or extend `end_to_end` with a `lex_parse_lower_emit` variant
— the builder should choose whichever keeps the bench structure consistent).

Also update the `benches/throughput.rs` module comment to explicitly describe all four stages
measured and which groups cover which stages.

**Why this stakeholder:** Cycles 026 = Lead Researcher, 025 = Spec Researcher,
024 = Compiler Contributor, 023 = Cache-Line Researcher. Cache-Line Researcher is the
most under-served — last served four cycles ago.

**Why now:** At step 2 of the Cache-Line researcher's journey (`cargo bench`), the output
shows throughput for the lexer (~670 MiB/s) and parser, and an `end_to_end` group — but
the `end_to_end` group only covers `lex_and_parse`. The lowering and codegen stages produce
no throughput number.

The `end_to_end` group name sets up an expectation — "here is the full pipeline measured" —
that the current benchmarks do not fulfill. The codegen stage is the stage where galvanic's
cache-line thesis lives: every instruction selection decision, every loop alignment, every
`.align` directive. That stage is completely absent from the benchmark output.

**The claim → test → benchmark chain breaks at Instr:**

For `Token` (in the lexer), the full chain exists:
- Claim: `src/lexer.rs:14-16` — "Token is 8 bytes, 8 tokens per cache line"
- Code: `Token` struct with `repr(u8)` kind + `Span`
- Test: `token_is_eight_bytes`
- Benchmark: `lexer/tokenize/fls_literals` at ~673 MiB/s — verifiable

For `Instr` (in the codegen stage), the chain stops at the test:
- Claim: `src/ir.rs:22-27` — "`Instr` is 80 bytes — larger than a single cache line; `Vec<Instr>` has 80 bytes per instruction in contiguous storage"
- Code: `Instr` enum
- Test: `instr_size_is_documented` ✓
- Benchmark: **nothing** — the codegen stage has no throughput measurement

The `Instr` size claim is important: at 80 bytes per instruction, a 16-instruction loop body
(the `for_range_sum` loop that perfectly fits one 64-byte instruction cache line at runtime)
requires 16 × 80 = 1280 bytes of IR storage in the Vec. The cache-conscious design in the
emitted code is served by an IR layer that is itself not cache-conscious. This is a
documented tradeoff — but without a benchmark, the researcher cannot see it.

**The class of fix:** A benchmark that doesn't cover the full pipeline is not an
end-to-end benchmark. The wrong states made unrepresentable: after this fix, any new stage
added to the pipeline that doesn't have a corresponding benchmark entry is visibly absent
from the `cargo bench` output — a gap is noticeable rather than invisible.

**What the benchmark needs to produce:** Running `cargo bench` must show output like:

```
full_pipeline/lex_parse_lower_emit/fls_functions
                    time:   [...]
                    thrpt:  [X.XX MiB/s Y.YY MiB/s Z.ZZ MiB/s]
```

The throughput should be in bytes/sec (bytes of source processed per second), consistent
with every other benchmark in the suite.

**Constraint:** Do not filter out `lower` or `codegen` errors in the benchmark — if
either returns an error, propagate it with `unwrap()` so benchmark failures surface
immediately rather than silently measuring no-op paths. The benchmark should measure the
happy path (all fixtures parse and lower without error).

Also: if `lower::lower()` or `codegen::emit_asm()` are not yet re-exported via a convenient
path from `lib.rs`, do not add convenience wrappers — import them by full path
(`galvanic::lower::lower(...)`, `galvanic::codegen::emit_asm(...)`).

**Lived experience note:** I became the Cache-Line Performance Researcher — someone who
arrived because the README says "obsessively cache-line-aware codegen" and wants to verify
that claim is real. I read the README (clear, two research questions). I ran `cargo bench`.
The lexer numbers appeared: 673 MiB/s for fls_literals. Impressive. I saw
`lexer/tokenize_stress/100` at 175 MiB/s — notably lower throughput for the stress input.
Interesting, and I made a mental note. Then I saw `end_to_end/lex_and_parse` — good, I
thought, here's where I'll see the full pipeline number. But the `end_to_end` group only
had `lex_and_parse`. I waited for the bench to finish. No codegen number appeared.

I ran `cargo bench` again to confirm. Same result. The benchmark that is named "end to end"
stops before the end. The stage where the cache-line decisions are made — where instructions
are selected, where `.align` directives are placed, where loop bodies are counted and their
cache spans annotated in comments — produces no throughput number. The `Instr` size claim
says 80 bytes, larger than a cache line. I found the `instr_size_is_documented` test.
I verified it. But I couldn't connect it to a performance observation. The hollowest moment:
reading `ir.rs:18-27`, which precisely documents the cache-line tradeoff for `Instr`
(80-byte headers in contiguous Vec storage), and then running `cargo bench` and finding
that the pipeline stage that uses those `Instr` values to produce output has never been
measured. The `end_to_end` group's name promised the answer and didn't deliver it.
