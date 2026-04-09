# Alignment Summary — galvanic

Read this in 30 seconds before starting cycles. It's a briefing, not documentation.

---

## Who This Serves

**The maintainer (William):** Working through the FLS section by section, implementing each language feature and documenting what the spec says — and doesn't say. Each cycle should advance the FLS frontier by one milestone or harden an existing one.

**The FLS researcher:** Anyone reading the spec or this codebase to understand whether the FLS is independently implementable. Their value is the documented ambiguities: `// FLS §X.Y: AMBIGUOUS — ...` comments and changelog notes. If these are missing, the research record is incomplete.

**The cache-line researcher:** Interested in whether treating cache-line alignment as a first-class constraint (not an optimization pass) produces measurably better code. Their trust depends on `size_of::<Token>() == 8` holding and the cache-line rationale being maintained in new types.

**The CI / lathe pipeline:** Needs clean builds, passing tests, and a falsify.sh that always prints its summary line.

---

## Key Tensions

| Tension | Current resolution |
|---|---|
| FLS fidelity vs. constant-folding shortcuts | **FLS fidelity wins, always.** Galvanic is a compiler, not an interpreter. `fls-constraints.md` documents the constraint; the `runtime-codegen` claim enforces it. |
| Cache-line discipline vs. implementation simplicity | **Maintain discipline on hot paths.** Token, Span, IR instructions: size assertions required. Build-time structs: a brief note is enough. |
| Milestone breadth vs. e2e test depth | **Favor depth when recent milestones lack e2e tests.** A milestone with only a parse-acceptance test is half-done. |

---

## Load-Bearing Claims (what `falsify.sh` defends each cycle)

1. **`token-is-8-bytes`** — `size_of::<Token>() == 8`. Cache-line density is broken if Token grows.
2. **`runtime-codegen`** — `fn add(a: i32, b: i32) -> i32 { a + b }` emits a runtime ARM64 `add` instruction. Verifies galvanic is a compiler, not an interpreter.
3. **`no-unsafe-in-lib`** — No `unsafe` blocks in library code (excluding `main.rs`). The "safe Rust compiler" claim holds.
4. **`no-command-in-lib`** — No `std::process::Command` in library code. The library is pure computation.
5. **`clean-exit-empty-input`** — galvanic exits 0 on an empty `.rs` file. No crash or hang on degenerate input.
6. **`clean-error-missing-file`** — galvanic exits non-zero (cleanly) on a nonexistent file path. No panic on missing input.

---

## Current Focus

The project is at milestone 197+ with active development on for-loops over slices, closures, and `dyn Trait`. The agent should:

1. Check whether the last 3–5 milestones have e2e tests. If not, add one — this is usually the highest-value change.
2. Identify the next FLS section that parses but doesn't lower correctly, and implement it.
3. Check for Clippy warnings or failing falsification claims before any new work.

---

## What Could Be Wrong

**Stakeholders I may have missed:** The project might eventually have external users (other compiler researchers citing galvanic's findings). For now, the codebase shows no external consumers; stakeholder analysis is accurate for the current state.

**The `runtime-codegen` claim assumes the binary is built.** If `cargo build` hasn't been run, `falsify.sh` will mark this claim as "binary not built" rather than failing it. This is intentional — the engine runs `snapshot.sh` (which builds) before `falsify.sh`.

**`span_is_eight_bytes` has no test.** The `Span` struct is documented as 8 bytes and structurally guaranteed (two `u32` fields), but there's no test enforcing it the way `token_is_eight_bytes` does. This is a gap. Adding `ast::tests::span_is_eight_bytes` is a good early cycle and should extend `claims.md`.

**Claim 2 (runtime-codegen) uses ARM64 instruction syntax.** It greps for `^\s+add\b` in the assembly. If galvanic changes its codegen for addition (e.g., uses `adds` for flag-setting, or emits a different instruction form), this check may need updating. The claim is correct in principle; the grep pattern is an approximation.

**CI is Linux-only for e2e.** The `e2e` job requires `aarch64-linux-gnu-as`, `aarch64-linux-gnu-ld`, and `qemu-aarch64`. These are installed explicitly on `ubuntu-latest`. Local macOS runs skip e2e tests gracefully. This is intentional and correctly handled, but means macOS development runs only parse-acceptance and unit tests locally.

**No mutation testing.** The test suite exercises many FLS programs, but mutation testing would reveal whether the tests actually detect regressions. This is a gap worth noting for a future cycle when the feature set stabilizes.

**Repository security:** The repo appears to be public (higher injection risk). CI uses `pull_request` (not `pull_request_target`) and `contents: read` permissions — good posture. The engine's structured-data-only policy (never reading PR free-text fields into agent context) is the main mitigation. Branch protection on `main` should be verified: require at least one PR review before merging, restrict direct pushes.
