# Ambition

**Destination.** Galvanic answers both research questions with evidence across the full `no_std` Rust surface: a navigable, citable map of every FLS section where the spec is ambiguous or silent, and a worked demonstration that treating cache-line discipline as a first-class codegen constraint produces inspectable, meaningful differences on programs with real register pressure. (Cited from README.md lines 7–11: "Is the FLS actually implementable by an independent party? The spec claims to be a complete description of Rust. We're testing that claim" and "What happens when a compiler treats cache-line alignment as a first-class concern in every decision? Not as an optimization pass bolted on at the end, but as a constraint woven into layout, register allocation, and instruction selection from the start.")

The destination is the *learning*, not a shipping product — "value comes from what we learn" (README.md line 15). Winning looks like: both research questions have answers backed by real code, with the ambiguity registry as a citable artifact and the codegen as a worked demonstration.

---

## The Gap

**1. No real register allocator — the structural ceiling on "real programs."**
`codegen.rs:716` uses `if *reg >= 31 { 9 }` — any virtual register index ≥31 maps to the x9 scratch register. The comment acknowledges this: "use x9 (the designated intra-function scratch)... LoadImm is always immediately followed by a Store to a stack slot for high-numbered let-binding temporaries, so x9 is safe to reuse across sequential LoadImm+Store pairs." This is a workaround that holds for simple programs and breaks for programs with >30 live variables. Any program complex enough to genuinely stress the cache-line codegen decisions will also exceed 30 virtual registers — so the second research question can't be answered yet on real inputs.

**2. Large-immediate encoding incomplete — hard FLS §2.4.4.1 gap.**
`lower.rs:10770` errors with "unsigned literal {n} > {}: MOVZ+MOVK not yet supported (FLS §2.4.4.1)" on any large integer constant that requires a MOVZ+MOVK multi-instruction sequence. This is a concrete spec section galvanic cannot yet demonstrate compliance with or ambiguity about.

**3. Pattern matching surface is incomplete — blocks realistic `no_std` programs.**
`lower.rs` has "not yet supported" on: tuple scrutinee in match (line 12518), match arm guards on non-last arms (line 13212), nested tuple patterns in if-let (line 12140), @ binding sub-patterns in if-let and match (lines 12240, 13140, 13795). These are FLS §6.18 and §5.1.4 constructs that appear in ordinary Rust. Programs using them fail before the researcher gets to inspect codegen.

**4. Ambiguity registry does not yet cover FLS §15+.**
The current TOC in `refs/fls-ambiguities.md` runs through §14. `src/lib.rs:18` lists the parser as covering "FLS §5–§6, §7–§14, §18." Sections §15 (closures), §16 (generics at depth), §17, §18 have partial or no registry coverage — the first research question is open on these sections.

---

## What On-Ambition Work Looks Like

**A real register allocator is on-ambition.** Any algorithm — linear scan, graph coloring — that emits correct ARM64 for programs with >30 live variables closes the gap between "toy programs the demo handles" and "real programs that test the cache-line claim." Another x9 workaround patch is off-ambition: it extends the workaround without closing the structural gap.

**MOVZ+MOVK large-immediate support is on-ambition.** It closes a named FLS §2.4.4.1 gap and unblocks programs with real constants. Patching around the error on a specific constant value is off-ambition.

**Match/pattern coverage that compiles ordinary Rust idioms is on-ambition.** The measure: can the Lead Researcher paste a realistic `no_std` function using match-with-guards or tuple patterns and compile it? A fix that closes a `lower.rs:Unsupported(...)` path and adds assembly inspection tests for the new case is on-ambition. A smoke test that pins an existing error message is off-ambition unless it unblocks a structural change.

**Ambiguity registry entries for §15+ are on-ambition.** Each new FLS section galvanic encounters — closures, generics, trait objects — should produce either an implementation or a documented gap entry in `refs/fls-ambiguities.md`. The Spec Researcher can't cite what isn't written.

---

## Velocity Signal

Thirty-plus lathe cycles have landed, each with multi-file changes. The pattern alternates between research advance (new FLS section coverage) and quality consolidation (registry clarity, contributor DX, error message precision). The project is in systematic-expansion mode — not approaching a release deadline, not polishing for an external event. The register allocator gap is structural and has persisted across many cycles; when it gets addressed it will likely be a multi-round cycle, not a one-file fix.
