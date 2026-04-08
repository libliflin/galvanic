# Changelog — Cycle 99

## Who This Helps
- **William (researcher)**: M171 adds `unsafe trait` / `unsafe impl` support — the natural companion to M170's `unsafe fn`. This completes the §19 Unsafety coverage for the two primary unsafe items (functions and traits). The FLS ambiguity is documented: the spec requires `unsafe impl` when implementing `unsafe trait` but doesn't specify enforcement.
- **FLS / Ferrocene Ecosystem**: The AMBIGUOUS note on §19 is extended — the spec does not specify how the compiler verifies the `unsafe trait` ↔ `unsafe impl` pairing. This is a second gap in §19 now documented in galvanic.
- **Compiler Researchers**: Claim 53 adversarially guards M171: any future change that constant-folds `unsafe impl` method bodies or suppresses `bl` at call sites fails the falsification suite.

## Observed
- Previous cycle (98) implemented M170 (`unsafe fn`). All 1439 tests passed, 51 claims held.
- Evaluating priority stack: falsification passes, CI passes, recent milestones have assembly inspection. Layer 6 (features) is next.
- `unsafe trait` / `unsafe impl` (§19) is the direct extension of M170 — the parser accepted `unsafe fn` but would fail on `unsafe trait Foo { }` or `unsafe impl Foo for Bar { }`.
- `TraitDef` and `ImplDef` had no `is_unsafe` field; `parse_item` had no `KwUnsafe` arm for `trait` or `impl`.

## Applied
- **`src/ast.rs`**: Added `is_unsafe: bool` to `ImplDef` and `TraitDef`, each with FLS §19 AMBIGUOUS doc comments explaining the unenforced pairing constraint.
- **`src/parser.rs`**: Updated `parse_impl_def` and `parse_trait_def` to accept `is_unsafe: bool` parameter. Added two new `KwUnsafe` arms in `parse_item`: one for `unsafe trait`, one for `unsafe impl`. Updated all call sites to pass `is_unsafe: false` (for the existing `KwImpl`/`KwTrait` arms) or `true` (for the new unsafe arms).
- **`src/lower.rs`**: No changes needed — `unsafe impl` lowers identically to `impl`. The qualifier is a static safety contract with no runtime behavior.
- **`tests/fixtures/fls_19_unsafe_trait.rs`**: FLS §19-derived fixture with `unsafe trait` + `unsafe impl` for two concrete types, multiple methods, callers via inherent dispatch.
- **`tests/fls_fixtures.rs`**: `fls_19_unsafe_trait` parse acceptance test.
- **`tests/e2e.rs`**: 8 compile-and-run tests + 2 assembly inspection tests for M171.
- **`.lathe/claims.md`**: Claim 53 — `unsafe trait` method call emits runtime bl (not folded), body emits runtime mul (not folded).
- **`.lathe/falsify.sh`**: Claim 53 adversarial check.

## Validated
- `cargo build` — clean
- `cargo test` — 1449 e2e tests (was 1439), 29 fixture tests (was 28), 211 unit tests; 0 failed
- `cargo clippy -- -D warnings` — clean
- `bash .lathe/falsify.sh` — 52 passed, 0 failed (was 51 + Claim 53)

## FLS Notes
- **FLS §19 AMBIGUOUS (unsafe trait pairing)**: The spec states implementors of `unsafe trait` must use `unsafe impl`, but does not specify:
  1. Whether enforcement is a type-checking pass or a separate unsafety analysis.
  2. What happens if `unsafe impl` is used for a non-unsafe trait (likely a lint, not an error — but the spec is silent).
  3. How the `unsafe` qualifier interacts with generic impl blocks (`unsafe impl<T> Foo for Bar<T>`).
  Galvanic records `is_unsafe` on both `TraitDef` and `ImplDef`. Enforcement of the pairing constraint is deferred — it would require cross-referencing trait definitions during type resolution.

## Next
- M172: `unsafe impl` for a generic type (`unsafe impl<T> Foo for Bar<T>`) — extends M171 to the generic case already handled by non-unsafe impls.
- Or: Static enforcement of `unsafe impl` pairing (verify that `unsafe impl` is only used when the trait is declared `unsafe trait`). Requires tracking `is_unsafe` from `TraitDef` through name resolution.
- Or: §6.23 Arithmetic Overflow — const vs runtime overflow semantics; directly relevant to galvanic's core research question.
