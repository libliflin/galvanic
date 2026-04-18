# Stakeholder Journeys

Concrete first-encounter journeys for each stakeholder. Walk one of these each cycle. The steps here are durable — current-state friction belongs in the snapshot, not here.

---

## Lead Researcher

**Emotional signal:** Momentum — "another FLS section is conquered."

**First ten minutes:**
1. `cargo build` — expect OK in under 60s
2. `cargo test` — note which tests pass, which fail, which are ignored
3. `cargo clippy -- -D warnings` — should be clean
4. Pick a fixture targeting the FLS section they care about:
   `ls tests/fixtures/fls_*.rs` to see what's available
5. `cargo run -- tests/fixtures/fls_<section>_<topic>.rs`
6. Read stdout/stderr:
   - All functions lowered + emits `.s` → read the assembly file
   - Partial lower → read the "lowered N of M" summary and the errors
   - Error → read the FLS citation and fix-site hint
7. If a `.s` file was emitted, open it: look for cache-line commentary, proper ARM64 patterns, `_galvanic_panic` for bounds checks
8. Check `refs/fls-ambiguities.md` — is the observed gap documented?

**Where momentum dies:** An error with no FLS citation. A gap that's in the ambiguity registry but the entry is out of section order and hard to find. A fixture that produces wrong assembly silently (no error, wrong code).

**Where momentum lives:** "lowered 12 of 12 functions" for a fixture that used to fail. A new entry in `refs/fls-ambiguities.md` with a minimal reproducer they can run. An error that names the function, the FLS section, and the fix site.

---

## Spec Researcher

**Emotional signal:** Confidence — "this finding is specific, real, and citable."

**First ten minutes:**
1. Find the repo (GitHub, FLS community, Ferrocene discussions)
2. Read `README.md` — understand what galvanic is ("sacrificial anode") and the two research questions
3. Open `refs/fls-ambiguities.md`
4. Check for a table of contents — can they jump to their FLS section without reading 800 lines?
5. Navigate to their target section (e.g., §6.5 for float semantics, §4.13 for dyn Trait)
6. Read: Gap description, galvanic's choice, minimal reproducer
7. Run the minimal reproducer: `cargo run -- /tmp/reproducer.rs` then inspect the `.s` file
8. Verify the assembly signature matches what the entry claims
9. Record the finding with the FLS section, galvanic's choice, and the assembly signature

**Where confidence dies:** No TOC → must scroll 800 lines. Entries not in section order → misses related entries. A reproducer labeled "not demonstrable" with no alternative path. An assembly signature that doesn't match what the compiler actually emits.

**Where confidence lives:** TOC with anchor links. Entries sorted by FLS section. A minimal reproducer that runs in under 10 seconds and produces the claimed output. The "galvanic's choice" section explains *why*, not just *what*.

---

## Compiler Contributor

**Emotional signal:** Clarity — "I know exactly where to start."

**First ten minutes:**
1. Read `src/lib.rs` header — understand the pipeline and the "Adding a new language feature" guide
2. Write a fixture: `tests/fixtures/fls_<section>_<topic>.rs` targeting the construct they want to implement
3. `cargo run -- tests/fixtures/fls_<section>_<topic>.rs`
4. Read the error:
   - Should contain: function name that failed, FLS section, construct name, fix-site hint
   - e.g., `error: lower failed in 'my_fn': not yet supported: tuple scrutinee match (FLS §6.18 — see enum_base_slot/struct_base_slot in lower.rs)`
5. Look up the FLS section — the citation should map to `refs/fls-constraints.md` or the spec directly
6. Find the fix site in the source:
   - For new syntax: `src/ast.rs` + `src/parser.rs`
   - For new runtime behavior: `src/ir.rs` (add variant with FLS comment + size test)
   - For lowering: `src/lower.rs` (the AMBIGUOUS annotation marks where to add the case)
   - For codegen: `src/codegen.rs` (comment register usage and cache-line reasoning)
7. Run `cargo test` — new fixture test should pass

**Where clarity dies:** "not yet supported" with no FLS section and no fix-site hint. `lower.rs` is 18,000+ lines with no navigation to the right place. A size test is missing for a new IR type that has cache-line commentary.

**Where clarity lives:** Three-hop navigation: error → FLS section → AMBIGUOUS annotation → fix location. Every "not yet supported" string carries `(FLS §X.Y)`. The `lower_source_all_unsupported_strings_cite_fls` test enforces this invariant.

---

## Cache-line Performance Researcher

**Emotional signal:** Discovery — "I can see the effect."

**First ten minutes:**
1. `cargo bench --bench throughput -- --warm-up-time 2 --measurement-time 3`
2. Note throughput numbers (tokens/sec for the lexer, the primary cache-line-aware stage)
3. Read `src/lexer.rs` — find the `Token` size assertion and the 8-bytes-per-token rationale
4. Run a fixture: `cargo run -- tests/fixtures/fls_2_4_numeric_literals.rs`
5. Open the emitted `.s` file — look for patterns that reflect cache-line decisions:
   - Stack frame alignment to 16 bytes (`sub sp, sp, #N` where N is a multiple of 16)
   - Struct fields accessed at offsets that fit in cache lines
6. Read `src/ir.rs` for IR types with `cache_line` fields or size assertions
7. Check `refs/arm64-abi.md` for the ABI context that informs codegen layout decisions

**Where discovery dies:** Benchmark numbers exist but there's no way to attribute them to specific cache-line decisions. Cache-line rationale is in comments but no corresponding assembly pattern makes it observable.

**Where discovery lives:** Size assertions in tests that enforce cache-line sizing. Assembly output with clear, consistent stack alignment. A benchmark that measures the thing the cache-line discipline was meant to improve.
