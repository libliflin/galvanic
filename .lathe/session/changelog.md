# Changelog — Cycle 91

## Who This Helps
- **William (researcher)**: M167 closes the next natural gap after M166. `Self::X` in method signatures was covered; `T::X` in generic free function signatures was not. A function like `fn use_it<C: Container>(c: C) -> C::Item` was an unsupported return type ("multi-segment return type" error). Now it compiles, monomorphizes, and emits correct runtime dispatch.
- **Compiler Researchers**: The fix is a four-line expansion in `lower_fn` — when monomorphizing `fn use_it<C>` with `C = Counter`, the alias map is extended with `C::Item → Counter::Item`'s IrTy. This is the smallest possible correct fix: no new IR nodes, no new codegen paths.

## Observed
- All 1399 tests pass; CI was green on the previous cycle.
- The "Next" from Cycle 90 explicitly called M167 as the next feature: `T::AssocType` in generic function signatures, after `Self::AssocType` (M166) was fully covered.
- Quick probe confirmed: `fn use_it<C: Container>(c: C) -> C::Item` hit `Err("multi-segment return type")` in `lower_fn`. The two-segment path (`C::Item`) fell through `lower_ty`'s Err branch to the `segs.len() == 2` arm at line 2501, which just errored rather than consulting the per-monomorphization alias expansion.

## Applied
- **`src/lower.rs`**: After building `effective_aliases` from `generic_subst`, added a loop over `generic_type_subst` entries. For each `(T, CT)` pair, any `CT::X` key in `effective_aliases` gets a corresponding `T::X` entry. This gives `lower_ty` everything it needs to resolve `C::Item` in the function signature during monomorphization.
- **`tests/e2e.rs`**: Added 8 compile-and-run tests (`milestone_167_*`) and 2 assembly inspection tests (`runtime_proj_*`). The assembly tests confirm: (1) the monomorphized label `use_it__Counter` is emitted, (2) body dispatches via `bl Counter__get` at runtime, (3) result is not constant-folded.

## Validated
- `cargo test` — 1409 passed (up from 1399; +10 new M167 tests)
- `cargo clippy -- -D warnings` — clean
- All 10 M167 tests pass on first run

## FLS Notes
- **FLS §10.2 / §12.1**: The spec does not address the interaction between associated type projections and generic type parameters in free function signatures. `fn use_it<C: Container>(c: C) -> C::Item` requires knowing that `C::Item` resolves to the concrete type of `C`'s `type Item = ...` impl. The FLS describes `C::Item` as a "qualified path" (§10.2) but does not specify how it resolves during generic instantiation. Galvanic's approach (expand the per-monomorphization alias map) is pragmatic and correct but the spec is silent on the mechanism.

## Next
- Add Claim 49 to `falsify.sh`/`claims.md` for M167: `runtime_proj_return_emits_bl_not_folded` and `runtime_proj_two_types_both_monomorphized` should be adversarially guarded. This follows the pattern of Claims 45–48 (feature added one cycle, falsification added the next).
- Or: Add fixture `tests/fixtures/fls_10_2_assoc_type_proj.rs` for `T::AssocType` in generic function signatures (FLS §10.2 fixture coverage).
