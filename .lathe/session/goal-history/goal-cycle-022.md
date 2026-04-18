# Goal — Cycle 022

**Stakeholder:** Lead Researcher

**What to change:** When `main` fails to lower but other functions succeed, emit the partial assembly for the successfully lowered functions — annotated clearly as "no entry point — inspection only" — instead of discarding all output with "no fn main, no assembly emitted."

**Why this stakeholder:** Cycles 021 = Spec Researcher, 020 = Compiler Contributor, 019 = Cache-Line Researcher, 018 = Lead Researcher. The rotation puts the Lead Researcher next, and the journey immediately surfaced a clear friction point.

**Why now:** At step 3 of the Lead Researcher's journey — running a fixture and reading the output — the patterns fixture (`fls_5_patterns.rs`) produces this:

```
galvanic: compiling fls_5_patterns.rs
parsed 29 item(s)
error: lower failed in 'main': not yet supported: expected struct literal `Inner { .. }` for nested struct field (FLS §6.11, §5.10.2)
lowered 20 of 21 functions (1 failed)
galvanic: lowered 20 function(s) — no fn main, no assembly emitted
```

20 of 21 functions lowered successfully. Those 20 functions cover §5.1.4, §5.1.9, §5.1.11, §5.2, §5.4, §5.5, §5.10.2, §5.10.3, §5.10.4, §6.18 — ten distinct FLS sections' worth of pattern and match coverage, all successfully lowered to ARM64 assembly. None of it appears in the `.s` file. The partial module with 20 functions is constructed, then discarded because there's no `main` in it.

The current behavior is correct about the rule — no entry point, no runnable binary — but it applies a binary-execution requirement to an assembly-inspection use case. The Lead Researcher's primary research artifact is the `.s` file, not the binary. Those 20 functions have FLS citations, cache-line notes, and runtime instruction patterns that are worth inspecting whether or not `main` compiles.

In `src/main.rs` line 114, the check `if !module.fns.iter().any(|f| f.name == "main")` branches to print "no fn main, no assembly emitted" and return without calling `codegen::emit_asm`. This is where 20 functions of research output are discarded.

**The class of fix:** When the partial module has functions but no `main`, pass the module to `codegen::emit_asm` anyway. Emit the `.s` file with a header comment making clear there is no entry point and the output is for inspection only. Do not emit `_start` or `_galvanic_panic` (since there is no entry point to call them). The exit code remains non-zero. The output message changes from the current form to something like:

```
galvanic: emitted fls_5_patterns.s (partial — fn main failed, no entry point)
```

The invariant to achieve: **successfully lowered functions are always emitted as assembly, regardless of whether `main` is present**. The only case where zero assembly is emitted is when zero functions lowered successfully.

This is the right class of fix — not a guard for one fixture, but elimination of a code path that silently discards successful compiler output. "No fn main" should mean "no runnable binary" not "no assembly."

**Constraint:** The `_start` ELF entry point and `_galvanic_panic` handler should only be emitted when `main` is present and successfully lowered — they depend on calling `main`. The partial assembly (without main) should have a comment at the top: `// inspection-only — no fn main; this assembly has no entry point`. The existing partial-output machinery (the `had_lower_errors` flag, the exit code contract) applies unchanged.

**Lived experience note:** I became the Lead Researcher. I ran `cargo test` — clean (2102 pass). I picked the patterns fixture because §5 has the richest variety of forms and I wanted to see how pattern lowering looks in the assembly. I ran `cargo run -- tests/fixtures/fls_5_patterns.rs`. I watched 29 items parse. I watched "20 of 21 functions" succeed. I waited for the `.s` file path. The hollowest moment: "no fn main, no assembly emitted." Not even a partial `.s` file with the 20 functions. I opened the fixture to find what failed in main — a nested struct pattern, `Inner { .. }`, which isn't yet supported. That error is clear and citable. But then I'm left with nothing to inspect. I cannot see the assembly for `range_inclusive`, `swap`, `magnitude_sq`, `unwrap_or_zero`, or any of the other 20 functions that worked. Their assembly exists in the compiler's memory — the partial module was constructed — and is then thrown away. For a research compiler, that is the worst kind of loss: correct output that never reaches the researcher.
