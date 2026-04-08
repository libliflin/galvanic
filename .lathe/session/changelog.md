# Changelog — Cycle 95

## Who This Helps
- **William (researcher)**: M168 fills the symmetric gap left by M167. Return position (`fn f<C: Container>(c: C) -> C::Item`) was covered; parameter position (`fn f<C: Container>(c: C, item: C::Item) -> i32`) was not. Both patterns now work. The feature required no new lowering code — the `effective_aliases` expansion added in cycle 91 already handles parameter position via `lower_ty`'s two-segment path arm.
- **Compiler Researchers**: The `T::AssocType` in parameter position case is a zero-code-change feature: the alias map expansion in `lower_fn` produces `effective_aliases["C::Item"] = IrTy::I32`, and the parameter spilling loop calls `lower_ty(&param.ty, source, type_aliases)` which looks up `C::Item` in `type_aliases` (now bound to `effective_aliases`). The two-segment path arm at `lower_ty:3883` handles the lookup.

## Observed
- All 1409 tests pass; CI was green on main (PR #194 merged).
- Cycle 94's "Next" explicitly listed M168 as the next feature: `T::AssocType` in generic function **parameter** position.
- Quick probe confirmed the feature already works without new code: `fn process<C: Container>(c: C, item: C::Item) -> i32 { item }` compiled and emitted correct ARM64 with `item` spilled to slot 1.

## Applied
- **`tests/e2e.rs`**: Added 8 compile-and-run tests (`milestone_168_*`) and 2 assembly inspection tests (`runtime_param_proj_*`).
  - `milestone_168_param_proj_basic` — `item: C::Item` returned directly
  - `milestone_168_param_proj_in_arithmetic` — `C::Item` param used in `c.get() + extra`
  - `milestone_168_param_proj_result_in_if` — result used in conditional
  - `milestone_168_param_proj_result_passed_to_fn` — result passed to helper fn
  - `milestone_168_param_proj_two_impls` — two concrete types, both monomorphized
  - `milestone_168_param_proj_on_parameter` — outer fn wraps generic call
  - `milestone_168_param_proj_called_twice` — same monomorphization called twice
  - `milestone_168_param_proj_with_assoc_const` — combined with `S::FACTOR` assoc const
  - `runtime_param_proj_emits_add_not_folded` — positive: `add_extra__Counter` + `bl Counter__get`; negative: result not folded to `#8`
  - `runtime_param_proj_two_types_both_monomorphized` — both `with_offset__Meters` and `with_offset__Feet` emitted; result not folded to `#6`

## Validated
- `cargo test --test e2e milestone_168` — 8 passed
- `cargo test --test e2e runtime_param_proj` — 2 passed
- `cargo test` — 1419 passed; 0 failed (up from 1409)
- `cargo clippy -- -D warnings` — clean

## FLS Notes
- **FLS §10.2 / §12.1: AMBIGUOUS** — The FLS does not specify how `T::X` in parameter position resolves during monomorphization. Galvanic's approach (extend the per-monomorphization alias map so `C::Item → IrTy::I32` is available when lowering parameter types) is symmetric to the return-position case and requires no new machinery.

## Next
- Add Claim 50 to `falsify.sh`/`claims.md` for M168: `runtime_param_proj_emits_add_not_folded` and `runtime_param_proj_two_types_both_monomorphized` should be adversarially guarded. Follows the pattern of Claims 45–49.
- Or: advance to M169 — `T::AssocType` in where clause bounds, the next natural extension in §10.2/§12.1 territory.
