# Changelog — Cycle 012, Round 1

## Goal
Parse FLS §4.14 parenthesized trait bounds (`Fn(T) -> R` form) in both
generic-param and where-clause positions.

## Who This Helps
- **Stakeholder:** The Lead Researcher
- **Impact:** They can now write `fn apply<F: Fn(i32) -> i32>(f: F, x: i32) -> i32`
  and `where F: Fn(i32) -> i32` without hitting a parse error. All 43 fixtures
  still compile end-to-end; 3 new tests confirm the parse and codegen path.

## Applied
Added parenthesized bound handling to three bound-parsing sites in `src/parser.rs`:

1. **`parse_fn_def` `'bound_loop`** — after consuming the trait name `Ident`, if
   the next token is `OpenParen`, consume the balanced paren list and optional
   `-> RetType` (via `parse_ty().ok()`).
2. **`parse_impl_def` `'impl_bound_loop`** — identical fix.
3. **`parse_where_clause` bound loop** — same logic; return type consumed manually
   since the function is `-> ()`.

Added `AMBIGUOUS: §4.14` annotations at all three sites. Added matching
`refs/fls-ambiguities.md` entry under §4.14.

Added fixture `tests/fixtures/fls_4_14_fn_bounds.rs`, parse acceptance test
`fls_4_14_fn_bounds` in `tests/fls_fixtures.rs`, and two assembly inspection
tests in `tests/e2e.rs`.

**Files modified:**
- `src/parser.rs` — three bound-parsing sites
- `refs/fls-ambiguities.md` — new §4.14 entry + ToC entry
- `tests/fixtures/fls_4_14_fn_bounds.rs` — new fixture
- `tests/fls_fixtures.rs` — new parse acceptance test
- `tests/e2e.rs` — two new assembly inspection tests

## Validated
- `cargo test` — 2063 pass, 0 fail (3 new tests)
- Verifier entry points:
  - `cargo test fls_4_14_fn_bounds`
  - `cargo test fls_4_14_fn_bound_inline_emits_call`
  - `cargo test fls_4_14_fn_bound_where_clause_emits_call`
  - `grep -n "Parenthesized Trait Bound" refs/fls-ambiguities.md`

## FLS Notes
**AMBIGUOUS §4.14:** The spec introduces parenthesized syntax for `Fn`/`FnMut`/
`FnOnce` but does not say whether this is a syntactic restriction (only those three
traits) or a semantic one (any trait name parses, only Fn-family is valid). Galvanic
defers to syntactic-accept / semantic-defer, matching rustc's parser behavior.
Documented in `refs/fls-ambiguities.md`.

---

# Previous: Customer Champion Cycle 012

## Stakeholder: The Lead Researcher

**Became:** The Lead Researcher — the author extending galvanic feature by feature,
tracking FLS compliance and cache-line correctness.

**Rotation rationale:** Cycle 009 served the Lead Researcher. Cycles 010–011 served the
Spec Researcher and Compiler Contributor. Lead Researcher most under-served (3 cycles).

---

## Floor check

2060 pass, 0 fail. Clippy OK. Build OK. Floor intact.

---

## What I experienced

Step 3 of the Lead Researcher journey: **0 parse-only fixtures** — all 43 compile. No
standard next target. Pivoted to picking a new FLS section.

Natural next step: `Fn(T) -> R` parenthesized trait bounds in generic position (FLS §4.14).
Closures and `impl Fn` already work. Wrote:

```rust
fn apply<F: Fn(i32) -> i32>(f: F, x: i32) -> i32 { f(x) }
fn main() -> i32 { apply(|x| x * 2, 5) }
```

**Wall:** `error: parse error at byte 55: expected Gt, found OpenParen`

Also tried `where F: Fn(i32) -> i32` — same failure: "expected OpenBrace, found OpenParen."

Read `src/parser.rs`. Confirmed: the generic-param bound loop (~lines 522–550) handles
`Trait<T>` args (angle-bracket) but not `Fn(T) -> R` args (parenthesized). Same gap in
`parse_where_clause` (~lines 908–935). Two symmetric spots, both missing `OpenParen`
handling.

**Worst moment:** All 43 fixtures compile. The Lead Researcher feels momentum. They write
the obvious next program — a generic higher-order function using the FLS §4.14 form — and
hit a parse error immediately. The wall is at the parser, not deeper.

---

## Goal

**Parse FLS §4.14 parenthesized trait bounds in generic-param and where-clause positions.**

In both bound-parsing sites in `parser.rs`, when `OpenParen` follows a trait name, consume
the parenthesized arg list and optional `-> ReturnType` tail instead of failing.

Add fixture `tests/fixtures/fls_4_14_fn_bounds.rs` demonstrating both forms. Add an
assembly inspection test confirming `blr` (indirect closure call). Add `AMBIGUOUS: §4.14`
annotation (scope of parenthesized syntax undefined for non-Fn traits) and matching ref
entry in `refs/fls-ambiguities.md`.
