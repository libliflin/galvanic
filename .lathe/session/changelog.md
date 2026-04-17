# Changelog — Customer Champion Cycle 011

## Stakeholder: The Compiler Contributor

**Became:** A Compiler Contributor — a CS student who found galvanic through the FLS,
cloned it, confirmed tests pass, and went straight to the snapshot's "Parse-only fixtures
(candidate goals)" section to find the next piece of work.

**Rotation rationale:** Cycle 008 served the Compiler Contributor. Cycle 009 served the
Lead Researcher. Cycle 010 served the Spec Researcher. The Compiler Contributor was last
served 3 cycles ago — their turn.

---

## Floor check

Build: OK. Tests: 2059 pass, 0 fail. Clippy: OK. Unsafe audit: OK. Floor intact.

---

## What I experienced

Walked steps 1–6 of the Compiler Contributor journey.

Steps 1–3: `cargo build && cargo test` passes. README is clear. Module docs at the top
of each `src/` file explain each stage's job. Architecture is navigable.

**Step 4:** Opened the snapshot to find a fixture to work on. The "Parse-only fixtures
(candidate goals)" section shows one entry:

```
  - fls_12_1_generic_trait_impl.rs
```

Read the fixture. It's a clean FLS §12.1 example with a generic trait, a generic struct,
a `impl<T> Trait for Type<T>` block, and a function `use_it(w: Wrapper<i32>) -> i32`.
No `fn main`.

```
cargo run -- tests/fixtures/fls_12_1_generic_trait_impl.rs
```

Output:
```
galvanic: compiling fls_12_1_generic_trait_impl.rs
parsed 4 item(s)
galvanic: lowered 2 function(s) — no fn main, no assembly emitted
```

Not an error — but also not a hint about whether the feature is implemented or not.

**The worst moment:** Searched `tests/e2e.rs` for `generic_trait_impl`. Found 9 tests:
`milestone_138_generic_trait_impl_basic`, `_arithmetic`, `_on_parameter`,
`_result_in_arithmetic`, `_called_twice`, `_two_impls`, `_with_inherent`,
`_called_from_non_generic`, `runtime_generic_trait_impl_emits_mangled_call`.

The feature is **fully implemented**. The "candidate goal" signal is false. The fixture
just lacks `fn main`. There is no unimplemented work here — but the snapshot says there is.

**The hollowest moment:** Running `cargo run --` on the fixture and seeing "no fn main,
no assembly emitted." Looking at `fls_12_1_generic_impl.rs` — it has `fn main` and a
`.s` snapshot. The pattern is clear. For `generic_trait_impl.rs`, the same pattern was
simply never applied.

---

## Goal

**Add `fn main` to `tests/fixtures/fls_12_1_generic_trait_impl.rs`, generate its `.s`
snapshot, and add a fixture-level assembly inspection test in `tests/e2e.rs`.**

This clears the snapshot's false "candidate goal" signal and gives the fixture the same
end-to-end coverage as every other compiled fixture.

### What to add

1. `fn main() -> i32 { let w = Wrapper { inner: 5 }; use_it(w) }` at the end of the
   fixture file.
2. Run `cargo run -- tests/fixtures/fls_12_1_generic_trait_impl.rs` and save the output
   as `tests/fixtures/fls_12_1_generic_trait_impl.s`.
3. Add a `fls_fixture_generic_trait_impl_compiles` test in `tests/e2e.rs` using
   `include_str!("fixtures/fls_12_1_generic_trait_impl.rs")` that asserts the assembly
   contains `Wrapper__get__i32` (the monomorphized call).

### Why now

The snapshot's "candidate goals" section is the first signal a Compiler Contributor
reads at step 4 of their journey. The only entry points at a fully-implemented feature
whose fixture just lacks `fn main`. Clearing it makes the signal honest: zero parse-only
entries means "all fixture-level work is done; pick a new FLS section."

---

## Applied (Builder)

1. Added `fn main() -> i32 { let w = Wrapper { inner: 5 }; use_it(w) }` to `tests/fixtures/fls_12_1_generic_trait_impl.rs`.
2. Ran `cargo run -- tests/fixtures/fls_12_1_generic_trait_impl.rs` to generate `tests/fixtures/fls_12_1_generic_trait_impl.s`.
3. Added `fls_fixture_generic_trait_impl_compiles` test in `tests/e2e.rs` using `include_str!` that asserts the assembly contains `Wrapper__get__i32` (monomorphized call) and `bl use_it` (no constant folding).

**Files:**
- `tests/fixtures/fls_12_1_generic_trait_impl.rs` — added `fn main`
- `tests/fixtures/fls_12_1_generic_trait_impl.s` — generated assembly snapshot
- `tests/e2e.rs` — added `fls_fixture_generic_trait_impl_compiles` test

## Validated

- `cargo test fls_fixture_generic_trait_impl_compiles` — passes
- `cargo test` — 2060 pass, 0 fail (up from 2059)
- Assembly contains `Wrapper__get__i32` and `bl use_it` — runtime instructions, not constant-folded
- Verifier: `cargo test fls_fixture_generic_trait_impl_compiles` or `grep Wrapper__get__i32 tests/fixtures/fls_12_1_generic_trait_impl.s`
