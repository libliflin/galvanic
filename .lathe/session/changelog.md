# Changelog — Cycle 022, Round 1 (Builder)

## Goal
When `main` fails to lower but other functions succeed, emit the partial assembly
(without `_start`), annotated "inspection-only — no fn main; this assembly has no
entry point." Exit code stays non-zero. Successful lowerings are never silently
discarded. The only case where zero assembly is emitted is when zero functions lowered.

## Who This Helps
- **Stakeholder:** Lead Researcher
- **Impact:** Running `cargo run -- tests/fixtures/fls_5_patterns.rs` now produces a
  `.s` file covering 20 functions and 10 FLS §5 subsections, instead of printing
  "no assembly emitted" and discarding all 20 successful lowerings. The Lead Researcher
  gets an artifact to inspect — cache-line notes, FLS citations, runtime instructions —
  for every function that lowered successfully, regardless of whether `main` compiled.

## Applied

**`src/codegen.rs`** — Refactored `emit_asm` to extract shared logic into a private
`emit_asm_impl(module, include_entrypoint: bool)` function:

- `emit_asm` (unchanged signature): asserts `has_main`, calls `emit_asm_impl(module, true)`.
- New `emit_asm_inspection_only(module)`: calls `emit_asm_impl(module, false)`.
- `emit_asm_impl`: all existing emit logic; when `!include_entrypoint`, prepends the
  `// inspection-only — no fn main; this assembly has no entry point` annotation and
  skips the `emit_start` call. Data sections (`.data`, `.rodata`, vtables) and the
  `_galvanic_panic` handler are still emitted when needed — inspection-only functions
  may reference statics, float constants, or division guards.

**`src/main.rs`** — Split the "no fn main" early-return into two paths:

- `had_lower_errors && !module.fns.is_empty()` → call `emit_asm_inspection_only`,
  write the `.s` file, print the inspection-only annotation, return 1.
- Clean compile with no `main` (library file) → existing "no assembly emitted" path,
  exit 0.

**`tests/smoke.rs`** — Added `partial_lower_no_main_emits_inspection_assembly`:
- Uses `tests/fixtures/fls_5_patterns.rs` (20 of 21 functions lower, `main` fails).
- Asserts: non-zero exit, "inspection-only" and "no fn main" and "no entry point" in
  stdout, `.s` file exists, `.s` file contains the annotation comment.
- Cleans up the `.s` file after the assertion.

- **Files:** `src/codegen.rs`, `src/main.rs`, `tests/smoke.rs`

## Validated

- `cargo test` — 2103 pass, 0 fail (up from 2102; +1 new smoke test)
- `cargo clippy -- -D warnings` — clean
- `cargo run -- tests/fixtures/fls_5_patterns.rs`:
  - stderr: `error: lower failed in 'main': not yet supported: ...`
  - stderr: `lowered 20 of 21 functions (1 failed)`
  - stdout: `galvanic: emitted tests/fixtures/fls_5_patterns.s (inspection-only — no fn main; this assembly has no entry point)`
  - exit code: 1
  - `head -2 tests/fixtures/fls_5_patterns.s` → `// inspection-only — no fn main; this assembly has no entry point`
- Existing `no_main_prints_lowered_note` (clean compile, no lower errors) still passes.
- Verifier: run `cargo test --test smoke -- partial_lower_no_main_emits_inspection_assembly`
  and then `cargo run -- tests/fixtures/fls_5_patterns.rs && head -5 tests/fixtures/fls_5_patterns.s`
  to witness the annotation and presence of 20 function bodies.
