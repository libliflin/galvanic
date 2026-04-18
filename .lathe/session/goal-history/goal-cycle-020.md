# Goal — Cycle 020

**Stakeholder:** Compiler Contributor

**What to change:** Add a `//!` pipeline overview doc comment to `src/lib.rs` that describes the six compilation stages (lexer → parser → lower → codegen → assembler → linker), maps each module to its FLS sections, and gives a contributor a clear "where to add a feature" sequence — so that opening `lib.rs` is the start of a navigation, not a dead end.

**Why this stakeholder:** Cycles 019 = Cache-Line Researcher, 018 = Lead Researcher, 017 = Spec Researcher, 016 = Compiler Contributor. Compiler Contributor last served four cycles ago — the most under-served.

**Why now:** Step 3 of the Compiler Contributor's journey is: "Open `lib.rs` to understand the module structure." The current `lib.rs` is six lines of `pub mod` declarations. No pipeline description. No module-to-FLS-section map. No "where to add a feature" guide.

The architecture documentation exists — but it lives in `.lathe/skills/architecture.md`, a file explicitly listed in `.gitignore` (alongside `/target` and `.lathe/session/`). It is invisible to anyone who clones the repo from GitHub. External contributors land at `lib.rs`, find nothing, and must reverse-engineer the pipeline from six module names.

The source files themselves have rich module-level `//!` docs — `codegen.rs`, `lower.rs`, `ir.rs`, `lexer.rs` all have clear FLS traceability sections and cache-line notes. But a contributor doesn't know which file to open first. `lib.rs` is the natural entry point and it gives them no map.

**The specific moment:** I ran `cargo run -- /tmp/test_cast_to_bool.rs` and hit `not yet supported: cast to bool not yet supported (FLS §6.5.9)`. Good error — names the FLS section, identifies the construct. I opened `lib.rs` to understand the pipeline before diving into source. Six lines of `pub mod`. No direction. I knew the error was in `lower.rs` because the error said "lower failed in 'main'" — but that's an incidental signal, not a deliberate navigation aid. A contributor who hit `parse error` would logically look in `parser.rs`; a contributor who hit a codegen mismatch would have no idea where to start.

After opening `lib.rs` I opened each source file in turn looking for pipeline context. I found it — `lower.rs` has a clear `//!` doc describing its role, `codegen.rs` has a target/ABI section, `ir.rs` explains it's the bridge between language semantics and machine instructions. But I had to open all six files to build the mental model that `lib.rs` should have given me in thirty seconds.

**The class of fix:** The architecture knowledge that currently lives only in `.lathe/skills/architecture.md` (a `.gitignore`'d internal file) needs to be available in the source tree at the natural first-read location. `lib.rs` is that location. The fix is not to copy the skills file verbatim — it's to write a `//!` doc comment for `lib.rs` that:

1. Shows the pipeline as a diagram (`source text → lexer → parser → lower → codegen → assembler → binary`)
2. Maps each `pub mod` to its FLS sections and single-sentence responsibility
3. States the key invariants a contributor must not violate (no unsafe, no `Command` outside `main.rs`, every IR node traces to an FLS section)
4. Names where new language constructs go (lowering case in `lower.rs`; codegen case in `codegen.rs`; IR node in `ir.rs` if new runtime behavior; AST node in `ast.rs`/`parser.rs` if new syntax)

The `//!` comment in `lib.rs` should be the single thing a contributor reads to understand where they are and what they're touching. It replaces the silence that currently greets them.

**Constraint:** Do not duplicate the full text of each module's own `//!` doc — those are authoritative and should be read in place. `lib.rs` should give the map (pipeline stage → module → FLS section → brief role), not the territory. Keep it to a diagram and a table or short list; a contributor should be able to read it in under two minutes.

**Lived experience note:** I became the Compiler Contributor — someone who found galvanic through a GitHub search, cloned it, ran tests (green), found `not yet supported: cast to bool (FLS §6.5.9)`, and wanted to add the case. I followed the journey: open `lib.rs`, read the module structure. Dead end. Six `pub mod` declarations. No diagram, no table, no pointer. I reverse-engineered the pipeline from the module names and individual source file headers over the next several minutes. The hollowest moment: reading "open `lib.rs` to understand the module structure" as the canonical first step, and finding that `lib.rs` tells you nothing. The fix already exists — the architecture skill has exactly the right content — but it's been written for the lathe engine, not for the repo. Moving the essential parts into `lib.rs //!` makes the contribution path self-documenting.
