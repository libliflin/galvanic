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

The contributor now has three questions with no answers:
1. Should I add `fn main` to the fixture file? (Will that change the fixture's FLS intent?)
2. Is the fixture intentionally library-style, or was `fn main` just forgotten?
3. If the feature is done, why does the snapshot list this as a "candidate goal"?

**The hollowest moment:** Running `cargo run -- tests/fixtures/fls_12_1_generic_trait_impl.rs`
and seeing "no fn main, no assembly emitted." The build worked, lowering worked, 4 items
parsed — but nothing to show. The contributor can't tell if this is a feature gap or a
fixture gap. Looking at the similar `fls_12_1_generic_impl.rs` fixture — it has `fn main`
and a `.s` snapshot. The pattern is clear for that fixture. For `generic_trait_impl.rs`,
the same pattern was simply not applied.

---

## Goal

**Add `fn main` to `fls_12_1_generic_trait_impl.rs`, generate its `.s` snapshot, and add
a fixture-level assembly inspection test in `tests/e2e.rs`** so the snapshot's parse-only
list becomes empty and the false "candidate goal" signal is cleared.

### What to change

**1. Extend `tests/fixtures/fls_12_1_generic_trait_impl.rs`:**

Add a `fn main` at the end of the file that exercises the fixture's own constructs:

```rust
fn main() -> i32 {
    let w = Wrapper { inner: 5 };
    use_it(w)
}
```

This is consistent with how `fls_12_1_generic_impl.rs` and every other compiled fixture
are structured: a runnable `fn main` that exercises the feature and produces a meaningful
exit code.

**2. Generate `tests/fixtures/fls_12_1_generic_trait_impl.s`:**

Run `cargo run -- tests/fixtures/fls_12_1_generic_trait_impl.rs` to emit the assembly,
then save the output as the `.s` snapshot. This is the same process used for all other
fixtures.

**3. Add a fixture-level assembly inspection test in `tests/e2e.rs`:**

Following the pattern of the `include_str!` fixture tests already in `e2e.rs`, add:

```rust
/// FLS §12.1 + §11.1: generic trait impl fixture — ensures the fixture compiles
/// end-to-end and emits a monomorphized call under the mangled name.
#[test]
fn fls_fixture_generic_trait_impl_compiles() {
    let src = include_str!("fixtures/fls_12_1_generic_trait_impl.rs");
    let asm = compile_to_asm(src);
    assert!(asm.contains("Wrapper__get__i32"), "expected mangled trait method name");
    assert!(asm.contains("bl"), "expected call instruction");
}
```

The mangled name `Wrapper__get__i32` is already emitted (confirmed during journey).

### Why this is a class-level fix

The "parse-only fixtures (candidate goals)" section of the snapshot exists to guide the
Compiler Contributor to the next piece of work. When the only entry in that list points
at a fully-implemented feature whose fixture just lacks `fn main`, the signal tells the
contributor there's work to do when there isn't — and gives no indication of why.

Clearing this entry eliminates the category: after this fix, a zero-entry parse-only list
means "all fixtures are compiled; the next feature work requires adding a new fixture from
an unimplemented FLS section." That's an honest signal. The contributor journey step 4
("find a fixture to work on") now correctly terminates with "nothing to do at the fixture
level; pick a new FLS section."

### Why now

Cycle 008 fixed the catch-all error message (naming the ExprKind variant). Cycle 009
added struct-literal enum args. Cycle 010 added the refs reproducer guard. Three cycles
of infrastructure improvement — the contributor journey was served two of those
(cycle 008) and now returns.

The snapshot is the first thing the Compiler Contributor consults at step 4. The false
signal at the very entry of their next-feature search breaks the journey before the
contributor has even attempted a real contribution.

### The specific moment

Step 4 of the Compiler Contributor journey. Opened the snapshot. Saw:
```
Parse-only fixtures (candidate goals):
  - fls_12_1_generic_trait_impl.rs
```
Ran `cargo run -- tests/fixtures/fls_12_1_generic_trait_impl.rs`. Got "no fn main, no
assembly emitted." Searched `e2e.rs` for the feature. Found 9 milestone_138 tests — the
feature was fully implemented in a prior cycle. The fixture just needs `fn main` and a
`.s` snapshot. The "candidate goal" label was misleading the contributor into thinking
there was unimplemented work.
