# Changelog — Cycle 124

## Who This Helps
- **CI / Validation Infrastructure**: The e2e CI job was failing on 7 milestone_183
  compile-and-run tests. Those tests now pass because the underlying bug is fixed.
- **William (researcher)**: Tuple structs can now be returned from functions,
  which is needed for any real-world program using tuple struct types — not just
  in-line construction.

## Observed
- CI e2e job failed with 7 failures, all `milestone_183_*` compile-and-run tests.
- Error (visible via `compile_to_asm`): `lower failed: Unsupported("return type 'Byte'
  (not a known struct, enum, or primitive)")`.
- Root cause: functions returning tuple struct types (`fn f() -> Byte`) were not
  recognised by the lowering pass. Five code sites all only checked `struct_defs`
  (named structs) but not `tuple_struct_defs` (tuple structs).
- The assembly inspection tests for milestone_183 passed locally because they used
  in-body construction (`let n = Byte(...)`) rather than function return. Only the
  compile-and-run tests (which use a separate `build()` function) exposed the gap.

## Applied
- **`src/lower.rs`** — five coordinated fixes:
  1. `struct_return_free_fns` population: also register functions whose return type
     is a tuple struct name.
  2. Return type path resolution: accept tuple struct names (same `RetFields` ABI
     as named struct returns).
  3. Callee body field-count lookup: check `tuple_struct_defs` when the struct name
     is not found in `struct_defs`.
  4. `lower_struct_expr_into`: add `ExprKind::Call` arm for tuple struct constructor
     expressions in return position (`Byte(a + b)` as function tail), with
     narrow-type normalisation (TruncU8/SextI8/TruncU16/SextI16).
  5. Call-site slot registration: for tuple struct returns, insert into
     `local_tuple_lens` and `local_tuple_struct_types` (not `local_struct_types`)
     so that `.0`/`.1` field access resolves via integer index.
- **`tests/e2e.rs`** — removed two temporary debug tests added during diagnosis.

## Validated
- `cargo build` — clean
- `cargo test` — 1581 e2e + 211 unit + 30 fixture + 1 smoke, all passed
- `cargo clippy -- -D warnings` — clean

## FLS Notes
- FLS §14.2 (tuple struct constructors) and §9 (functions) are both implicated.
  The spec describes tuple struct construction syntactically as a call expression
  but does not spell out the calling convention for returning tuple struct values
  from functions. Galvanic uses the same `RetFields` convention as named struct
  returns. No new ambiguity — existing AMBIGUOUS notes cover this.

## Next
- CI should now pass. The next cycle can resume red-team / falsification work or
  advance to the next FLS section.
- Candidate: add a `claims.md` claim for "tuple struct return from function does
  not constant-fold" (Claim 71), to prevent future regression of this exact class
  of bug.
