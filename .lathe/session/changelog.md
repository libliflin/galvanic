# Changelog — Cycle 96

## Who This Helps
- **William (researcher)**: Claim 50 prevents a specific regression class: a future
  change that constant-folds `add_extra__Counter` (T::AssocType in parameter position)
  would silently pass all exit-code tests. Now it can't. The falsification suite
  adversarially guards M168 with the same rigour as M167 (Claim 49).
- **Compiler Researchers**: The parameter-position case (`fn f<C: Container>(c: C, extra: C::Item)`)
  is now as well-defended as the return-position case. Both patterns are in `claims.md`
  with explicit attack vectors, and both are run every cycle.

## Observed
- Falsification suite had 48 passing claims (Claims 1–49) after cycle 95.
- Cycle 95's "Next" explicitly called out: "Add Claim 50 to `falsify.sh`/`claims.md`
  for M168: `runtime_param_proj_emits_add_not_folded` and
  `runtime_param_proj_two_types_both_monomorphized` should be adversarially guarded."
- The tests exist in `tests/e2e.rs` (lines 28009, 28056) but were not yet in
  `falsify.sh` or `claims.md`. The claim was unregistered.

## Applied
- **`.lathe/claims.md`**: Added Claim 50 documenting the M168 promise — T::AssocType
  in generic function parameter position emits monomorphized dispatch, runtime `add`,
  and does not fold. Includes the two patterns (single-type and two-type), attack
  vectors (fold to #8 / fold to #6 / missing monomorphized labels), FLS citations,
  and the AMBIGUOUS note.
- **`.lathe/falsify.sh`**: Added Claim 50 block running
  `runtime_param_proj_emits_add_not_folded` and
  `runtime_param_proj_two_types_both_monomorphized` with `fail`/`pass` guards.

## Validated
- `bash .lathe/falsify.sh` — **49 passed, 0 failed** (was 48 before this cycle)

## FLS Notes
- **FLS §10.2 / §12.1: AMBIGUOUS** — The FLS does not specify how `T::X` in parameter
  position resolves during monomorphization. Galvanic's mechanism (extend the
  per-monomorphization alias map so `C::Item → IrTy::I32` is available when
  lowering parameter types) is symmetric to the return-position case. The spec
  is silent on this.

## Next
- Advance to M169: `T::AssocType` in where clause bounds — the next natural
  extension in §10.2/§12.1 territory (e.g., `where C::Item: SomeTrait`).
- Or: add assembly inspection coverage for any milestone in the 91–128 range
  that has only exit-code tests and no `compile_to_asm` assertion.
