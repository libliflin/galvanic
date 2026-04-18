# Brand

**Identity.** Precise, self-aware research instrument — the voice of a compiler that knows exactly what it is, what it isn't, and where the spec ran out. Every claim is anchored to a FLS section; every silence is named rather than papered over. From `Cargo.toml` description: "Clean-room ARM64 Rust compiler built from the Ferrocene Language Specification with cache-line-aware codegen" — functional, technical, zero marketing. From `README.md`: "It's a sacrificial anode — it exists to find ambiguities in the spec and to explore what 'dumb but cache-aware' codegen can do. Nobody needs to use this. Value comes from what we learn."

---

## How we speak

**When we say no:** Direct and explicit about scope. "Do not use this to compile anything you care about" (`README.md`, "What this is not" section). No softening, no apology. Refusals come with a reason — the reason here is "this is a research instrument, not a production tool." The CI enforces its own refusals with the same grammar: `No unsafe code`, `No Command in library code` (`src/lib.rs:53–56`). The refusal names the rule, not a judgment.

**When we fail:** Flat, factual, located. `"not yet supported: {msg}"` (`src/lower.rs`, `LowerError::Unsupported`). Errors chain to their source: `"in '{item}': {inner}"` — you know what function failed before you see the inner cause. When the spec is silent on the right answer, the code says so explicitly: `// FLS §6.9 AMBIGUOUS: Out-of-bounds access must panic; the spec does not...` (`src/lower.rs`). A gap in the spec is not a failure to hide; it's a finding to log.

**When we explain:** Thorough but structured. `src/lib.rs` module-level docs use tables, ASCII pipeline diagrams, and numbered steps. The voice is declarative: "Each stage has one job and a clean boundary. Nothing earlier in the pipeline knows about later stages. The IR is the contract between language semantics and machine instructions." (`src/lib.rs:31–33`). Long explanations earn their length with tables and headers; they don't sprawl.

**When we onboard a new contributor:** Sequential and complete. "Find the FLS section → add AST types → add IR variant with FLS traceability comment → add lowering case → add codegen case → write tests." (`src/lib.rs`, "Adding a new language feature"). The sequence is exhaustive by design — if you follow it, you won't miss a step. The invariants are named before the sequence, so you know the rules of the road before you drive.

**When we report progress:** Terse status narration, no celebration. `"galvanic: compiling {filename}"`, `"galvanic: emitted {path}"`, `"galvanic: emitted {path} (partial — some functions failed)"` (`src/main.rs:54,173,171`). The partial-success case is still reported — honestly, without hiding the failure count. No exclamation marks, no emoji, no fanfare.

---

## The thing we'd never do

We'd never leave a spec gap as an implicit choice. Every place where the FLS is silent and galvanic had to pick something gets an `AMBIGUOUS` annotation and an entry in `refs/fls-ambiguities.md` — the finding, galvanic's resolution, the source location, and a minimal reproducer. The project's identity depends on this traceability: if a choice is undocumented, the research value evaporates. Burying a codegen decision in uncommented assembly would be a category violation, not a style preference. (See `src/lower.rs` AMBIGUOUS annotations, and `refs/fls-ambiguities.md`.)

---

## Signals to preserve

1. **FLS section citations in error messages.** Format: `(FLS §X.Y)` or `(FLS §X.Y, §Z)`. Every `not yet supported` error in `lower.rs` that names a spec section is doing the project's job — it turns a compile failure into a finding. Strip the citation and you strip the research signal.

2. **Lowercase imperative commit prefixes.** `fix:`, `verify:`, `docs:`, `goal:`, `bench:` — action-first, section-cited where applicable: `"fix: §6.13 tuple index access now cites §6.10..."` (`git log`). The convention is load-bearing: `verify:` means CI checked it; `goal:` means the champion named it. These are not cosmetic.

3. **Flat progress narration.** CLI output is `"galvanic: <verb> <noun>"` — no decoration, no hedging, no excitement markers. Partial success says "partial." Failure says so. The tone holds across the happy path and the error path equally.
