# Changelog — Cycle 87

## Who This Helps
- **William (researcher)**: Milestone 165 covers a natural intersection of M127 (default methods) and M164 (supertrait bounds): default method bodies that call supertrait abstract methods or chain through other default methods. The assembly inspection tests confirm that the entire call chain is runtime-dispatched via `bl`, not constant-folded — even when the chain spans trait boundaries.
- **Compiler Researchers**: The code documents that galvanic's monomorphization resolves supertrait method calls inside default bodies naturally: when `Foo__combined` is emitted, `self.base_val()` becomes `bl Foo__base_val` because the concrete type is known at monomorphization time.

## Observed
- All 45 falsification claims pass; CI was green on the previous cycle.
- Previous cycle's "Next" identified M165 as the natural follow-on: default methods on traits with supertraits.
- M127 tests default methods calling abstract methods on the same trait. M164 tests calling supertrait methods from generic functions. M165 tests the combination: calling supertrait methods from a default method body.
- Both test cases (same-trait abstract + supertrait abstract) compiled correctly without any code changes — monomorphization handles them naturally.
- A pre-existing `unused_parens` clippy warning in e2e.rs (from a previous cycle) was fixed.

## Applied
- **`tests/e2e.rs`**: 10 new tests:
  - 8 `milestone_165_*` compile-and-run tests covering: basic supertrait call from default, calling both supertrait and own abstract, two types using same default, chained defaults, generic bound calling default, result in arithmetic, on parameter, override replacing supertrait-calling default
  - 2 assembly inspection tests: `runtime_supertrait_default_call_emits_bl_not_folded` (verifies `bl Foo__base_val` emitted and result not folded to `#10`) and `runtime_supertrait_default_chain_not_folded` (verifies `bl Foo__doubled` and `bl Foo__value` both present, no fold to `#12`)
  - Fixed pre-existing `unused_parens` warning on line 26691

## Validated
- `cargo build` — clean
- `cargo test` — 1389 passed (was 1379, +10), 26 fixture tests, 211 unit tests, 0 failed
- `cargo clippy -- -D warnings` — clean (warning fixed)
- `.lathe/falsify.sh` — 45/45 pass

## FLS Notes
- **FLS §4.14 + §10.1.1**: No new ambiguities found. The monomorphization approach resolves supertrait method calls in default bodies naturally because the concrete type is available at codegen time. This is consistent with the approach documented in Claim 46.
- **FLS §10.1.1**: The spec does not explicitly state whether a default method body may call supertrait methods (vs. only methods declared in the same trait). Galvanic accepts and compiles this — it is the only reasonable interpretation, but worth noting as an implicit extension.

## Next
- Claim 47: Register `runtime_supertrait_default_call_emits_bl_not_folded` and `runtime_supertrait_default_chain_not_folded` in `falsify.sh` to guard the M165 invariant every cycle.
- M166: Supertrait enforcement — when `impl Derived for Foo` is present, verify `impl Base for Foo` also exists. Currently galvanic silently allows this without checking and may generate incorrect code.
- Or: `where Self: Trait` constraints inside trait method bodies — FLS §4.14 gap.

---

# Changelog — Cycle 86

## Who This Helps
- **William (researcher)**: Claim 46 closes the gap where a regression in supertrait dispatch (Milestone 164) could pass all exit-code tests invisibly. The falsification suite now enforces that supertrait method calls emit runtime `add` and are not constant-folded — every cycle.
- **Compiler Researchers**: The claim documents the architectural distinction for supertrait resolution: `T: Derived` implies `T: Base`, and galvanic resolves supertrait method calls via monomorphization (`T__base_val`) not through a separate vtable lookup. The FLS §4.14 ambiguity around this resolution is now registered in the claim.

## Observed
- All 45 falsification claims pass; CI was green on the previous branch.
- Milestone 164 (supertrait bounds) was added in Cycle 85.
- The two assembly inspection tests `runtime_supertrait_call_emits_bl_not_folded` and `runtime_supertrait_both_methods_not_folded` existed in `tests/e2e.rs` but were not registered in `falsify.sh`.
- Previous cycle's "Next" explicitly identified Claim 46 as the missing guard.

## Applied
- **`.lathe/claims.md`**: Added Claim 46 with full adversarial documentation — promise, attack vectors (fold single call to #11, fold sum of both calls to #9, drop `add` for method body), FLS citations (§4.14 with AMBIGUOUS note), and violated-if conditions.
- **`.lathe/falsify.sh`**: Added Claim 46 check running `runtime_supertrait_call_emits_bl_not_folded` and `runtime_supertrait_both_methods_not_folded`.

## Validated
- `.lathe/falsify.sh` — 45/45 pass (was 44, +1 new claim)
- `cargo build` — clean (no code changes; claims/falsify only)
- Claim 46 passes on first run against existing M164 implementation

## FLS Notes
- **FLS §4.14 AMBIGUOUS**: The spec does not specify how supertrait method availability propagates to generic call sites. Galvanic's approach (monomorphization resolves `T__base_val` naturally because the concrete type implements `Base`) is now documented in the claim.

## Next
- M165: Default methods on traits with supertraits — calling a supertrait's default method from a subtrait. Or `where Self: Base` constraints in trait definitions.
- Or: Supertrait enforcement — verify that `impl Derived for Foo` requires `impl Base for Foo` to also exist. Currently galvanic does not check this (documented as a known limitation in `ast.rs`).

---

# Changelog — Cycle 85

## Who This Helps
- **William (researcher)**: Milestone 164 adds supertrait bounds (`trait Derived: Base { ... }`), a fundamental Rust pattern used throughout real codebases. The FLS §4.14 gap is now covered: the parser accepts supertrait syntax, the AST stores the bounds, and the monomorphization system naturally handles calling supertrait methods from generic functions. The assembly inspection tests confirm the dispatch happens at runtime, not constant-folded.
- **FLS / Ferrocene Ecosystem**: Documents an explicit AMBIGUITY: FLS §4.14 does not specify how supertrait method availability propagates to generic call sites. Galvanic's approach (monomorphization resolves `T__base_val` naturally) is documented in code comments and this changelog.

## Observed
- All 45 falsification claims pass; CI was green on the previous branch.
- The parser for `parse_trait_def` consumed trait name then immediately looked for `where` or `{`. `trait Greet: Speak { ... }` would fail because `:` was neither `where` nor `{`.
- Previous cycle's "Next" suggested M164 as supertrait bounds or multi-field impl Trait return.
- Supertrait bounds are a more impactful FLS gap — they affect every program that uses trait hierarchies.

## Applied
- **`src/ast.rs`**: Added `supertraits: Vec<Span>` field to `TraitDef`. Documents that galvanic stores supertrait names but does not enforce the constraint at the type-system level.
- **`src/parser.rs`**: Extended `parse_trait_def` to consume `: Bound + OtherBound` supertrait syntax before the where clause. Stores spans in `supertraits`. Supports multiple supertraits (`trait D: A + B`).
- **`tests/fixtures/fls_4_14_supertrait_bounds.rs`**: New fixture derived from FLS §4.14 showing a trait hierarchy with `Derived: Base`.
- **`tests/fls_fixtures.rs`**: Added `fls_4_14_supertrait_bounds` parse acceptance test.
- **`tests/e2e.rs`**: 10 new tests — 8 `milestone_164_*` compile-and-run tests and 2 assembly inspection tests (`runtime_supertrait_call_emits_bl_not_folded`, `runtime_supertrait_both_methods_not_folded`).

## Validated
- `cargo build` — clean
- `cargo test` — 1379 passed (was 1369, +10), 26 fixture tests (was 25, +1), 211 unit tests
- `cargo clippy -- -D warnings` — clean
- Assembly inspection confirms: supertrait method calls emit `bl` to monomorphized labels, results are not constant-folded

## FLS Notes
- **FLS §4.14 AMBIGUOUS**: The spec does not specify how supertrait method availability is propagated to generic call sites. Galvanic's approach: `t.base_method()` on a generic `T: Derived` resolves via monomorphization to `T__base_method`, which exists because the concrete type implements the supertrait.
- **FLS §4.14**: Galvanic does not enforce that implementors of `Derived` also implement `Base`. Noted as a known limitation in the AST doc comment.

## Next
- Claim 46: Register `runtime_supertrait_call_emits_bl_not_folded` and `runtime_supertrait_both_methods_not_folded` in `falsify.sh` to guard the supertrait dispatch invariant.
- M165: Default methods on traits with supertraits — calling a supertrait default method from the subtrait body. Or supertrait where clauses (`where Self: Base`).
- Or: supertrait enforcement — when `impl Derived for Foo` is present, verify `impl Base for Foo` also exists.

---

# Changelog — Cycle 84

## Who This Helps
- **William (researcher)**: Claim 45 closes the gap where a M163 regression (wrong dispatch mechanism or constant folding) would pass all exit-code tests invisibly. The falsification suite now enforces that `impl Trait` return uses static `bl` dispatch — not vtable `blr` — every cycle.
- **Compiler Researchers**: The claim documents the architectural distinction between `impl Trait` return (static dispatch, no vtable) and `&dyn Trait` return (vtable `blr`) with precise attack vectors. This is the key design decision in FLS §11.

## Observed
- All 44 falsification claims pass; CI was green on the previous cycle.
- M163 (impl Trait in return position) was added in Cycle 83.
- The two assembly inspection tests `runtime_impl_trait_return_emits_bl_not_blr` and `runtime_impl_trait_return_not_folded` existed in `tests/e2e.rs` but were not registered in `falsify.sh`.
- Previous cycle's "Next" explicitly called out Claim 45 as missing.

## Applied
- **`.lathe/claims.md`**: Added Claim 45 with full adversarial documentation — promise, attack vectors, FLS citations (§9, §11 with AMBIGUOUS note), and violated-if conditions.
- **`.lathe/falsify.sh`**: Added Claim 45 check before the summary section, running `runtime_impl_trait_return_emits_bl_not_blr` and `runtime_impl_trait_return_not_folded`.

## Validated
- `.lathe/falsify.sh` — 44/44 pass (was 43)
- `cargo build` — clean (no code changes; claims/falsify only)
- Claim 45 passes on first run against existing M163 implementation

## FLS Notes
- **FLS §11: AMBIGUOUS** — The spec does not define how the concrete return type for `impl Trait` is resolved at call sites. This ambiguity is now documented in the claim as well as the existing code comment.

## Next
- M164: `impl Trait` return with a richer method body (multi-field struct, method accesses multiple fields) to stress-test the RetFields ABI for impl Trait. The current IMPL_TRAIT_RETURN_BASIC test uses a single-field struct — a multi-field test would catch regressions in field offset calculation.
- Or: supertrait bounds (`trait D: B { ... }`) — FLS §4.14 gap, the next untouched section naturally extending the trait hierarchy work.

---

# Changelog — Cycle 83

## Who This Helps
- **William (researcher)**: Milestone 163 adds `impl Trait` in return position, a feature that frequently appears in real Rust. Previously any function with `-> impl Trait` failed at the lowering stage with "complex return type". Now the feature compiles correctly with static dispatch — distinct from `&dyn Trait` return (M162) which uses vtable dispatch.
- **Compiler Researchers**: The code now documents a clear FLS §11 AMBIGUITY: the spec is silent on how the concrete return type for `impl Trait` is resolved at call sites. Galvanic's approach (scan the body tail expression for a struct literal) is explicitly documented.

## Observed
- All 44 falsification claims pass; CI was green on the previous branch.
- Milestones M159–M162 (dyn Trait progression) are well-defended with assembly inspection tests.
- The previous cycle's "Next" suggested M163 as a natural extension.
- `-> impl Trait` was already parsed (→ `TyKind::ImplTrait`) but fell through to `LowerError::Unsupported("complex return type")` in `lower_fn`.

## Applied
- **`src/lower.rs` — pre-pass**: Extended `struct_return_free_fns` population to detect functions with `-> impl Trait` return type. When the body tail expression is a struct literal of a known struct type, the function is registered with the concrete struct name.
- **`src/lower.rs` — `lower_fn`**: Added `TyKind::ImplTrait` branch in the return type match. Scans the body tail for a struct literal, sets `struct_ret_name`, enabling the existing RetFields ABI path.
- **`tests/e2e.rs`**: 10 new tests — 8 `milestone_163_*` compile-and-run tests and 2 assembly inspection tests (`runtime_impl_trait_return_emits_bl_not_blr`, `runtime_impl_trait_return_not_folded`).

## Validated
- `cargo build` — clean
- `cargo test` — 1369 passed, 0 failed (was 1359; +10 new)
- `cargo clippy -- -D warnings` — clean
- `.lathe/falsify.sh` — 43/43 pass

## FLS Notes
- **FLS §11: AMBIGUOUS** — The spec does not specify how the concrete return type for `impl Trait` is resolved at call sites. Galvanic's approach (tail expression must be a struct literal) is an implementation choice documented in code and changelog.
- **FLS §9: AMBIGUOUS** — Functions with `-> impl Trait` use the same RetFields ABI as explicit struct returns. The spec does not address the ABI for opaque return types.

## Next
- Add Claim 45 to `falsify.sh`: impl Trait return uses static `bl` dispatch (not vtable `blr`) and method result is not constant-folded.
- M164: impl Trait return with multi-field structs and richer method bodies, ensuring the non-folded guarantee holds.
- Or: supertrait bounds (`trait D: B { ... }`) as the next FLS §4.14 coverage gap.
