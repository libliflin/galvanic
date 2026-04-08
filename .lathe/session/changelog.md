# Changelog — Cycle 101

## Who This Helps
- **William (researcher)**: M172 completes the §19 unsafe coverage for generic
  impl blocks. `unsafe impl<T> Trait for Wrapper<T>` was accepted by the parser
  but entirely untested. Any regression in the combined generic+unsafe impl
  lowering path would now be caught by 8 runtime tests + 2 assembly inspection
  tests + Claim 55 in the falsification suite.
- **CI / Validation**: Falsification suite grows from 54 to 55 claims. Claim 55
  is adversarial — negative assertion on `mov x0, #12` (3*4 constant-folded).

## Observed
- All 1450 tests pass, 54 claims hold, CI clean on previous PR.
- Previous cycle's "Next" pointed to M172: `unsafe impl<T>` for a generic type.
- The parser already handled this path (parse_impl_def with is_unsafe=true
  handles `<T>` generics since M136/M171). No AST/parser/lower/codegen changes
  were needed — only tests and a new claim.
- Checking the pattern: M138 (generic trait impl without unsafe) works; M171
  (unsafe impl without generics) works; M172 is their intersection.

## Applied
- **`tests/e2e.rs`**: Added 8 compile-and-run milestone tests + 2 assembly
  inspection tests for M172:
  - `runtime_unsafe_generic_impl_body_emits_mul_not_folded`: positive `mul`,
    positive `bl`, negative `mov x0, #12` (adversarial)
  - `runtime_unsafe_generic_impl_call_emits_bl_not_folded`: positive `bl`,
    positive monomorphized label `Wrapper__get_inner`
  - Note: the second inspection test uses label presence (not `mov x0, #N`
    absence) because `mov x0, #7` legitimately appears as struct initialization —
    asserting `!mov x0, #7` would be a false negative.
- **`.lathe/claims.md`**: Added Claim 55 with FLS citations (§19, §12.1, §6.1.2),
  AMBIGUOUS notes, and attack vectors.
- **`.lathe/falsify.sh`**: Added Claim 55 adversarial check.

## Validated
- `cargo test --test e2e -- milestone_172 runtime_unsafe_generic_impl` — 10 passed
- `cargo test` — 1460 e2e (was 1450), 211 unit, 29 fixture, 1 smoke; 0 failed
- `bash .lathe/falsify.sh` — 55 passed (was 54), 0 failed

## FLS Notes
- **FLS §19 AMBIGUOUS**: The spec does not specify how `unsafe impl<T>` interacts
  with generic monomorphization. Is the `unsafe` qualifier replicated for each
  monomorphized instance, or is it purely a source-level annotation? Galvanic
  records `is_unsafe` on `ImplDef` before generic parameter processing — the
  qualifier is present on the generic template. Deferred.
- **FLS §12.1 AMBIGUOUS**: Pre-existing ambiguity — `<` immediately after
  `unsafe impl` is treated as always starting a generic parameter list.

## Next
- §6.23 Arithmetic Overflow: const vs runtime overflow semantics. This directly
  tests the core research question — does galvanic produce debug-mode panic
  behavior or silent wrapping for `255_u8 + 1` in a non-const function? This
  would require adding a new IR instruction (Panic/Trap) and documenting the
  ambiguity between debug and release mode behavior.
- Or: M173 — `unsafe impl<T: Bound>` for a bounded generic, adding where-clause
  constraints to the unsafe generic impl pattern.
