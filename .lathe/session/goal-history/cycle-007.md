# Changelog — Customer Champion Cycle 012

## Stakeholder: The Lead Researcher

**Became:** The Lead Researcher — the author extending galvanic feature by feature,
tracking FLS compliance, and answering the two research questions through every cycle.

**Rotation rationale:** Cycle 009 served the Lead Researcher. Cycle 010 served the
Spec Researcher. Cycle 011 served the Compiler Contributor. The Lead Researcher was
last served 3 cycles ago — most under-served.

---

## Floor check

Build: OK. Tests: 2060 pass, 0 fail. Clippy: OK. Unsafe audit: OK. Floor intact.

---

## What I experienced

Walked steps 1–9 of the Lead Researcher journey.

Steps 1–2: `git pull`, `cargo test` — 2060 pass, 0 fail. Floor intact.

**Step 3:** Find a fixture with only a parse test. Count: **0 parse-only fixtures**. All
43 compile end-to-end. This is good news — but the journey breaks here. There is no next
entry in the "find a fixture to compile" path. The Lead Researcher has to pivot.

**Pivot:** Look for a new FLS section to implement. With closures and `impl Fn` support
already working (verified in prior cycles via `fls_6_14_closure_expressions.rs`), the
natural next step is the generic bound form — the FLS §4.14 parenthesized trait bound
that makes higher-order generic functions possible:

```rust
fn apply<F: Fn(i32) -> i32>(f: F, x: i32) -> i32 { f(x) }
```

**Step 6 (adapted):**

```
echo 'fn apply<F: Fn(i32) -> i32>(f: F, x: i32) -> i32 { f(x) }
fn main() -> i32 { apply(|x| x * 2, 5) }' > /tmp/test_hof.rs
cargo run -- /tmp/test_hof.rs
```

Output:
```
error: parse error at byte 55: expected Gt, found OpenParen
```

Tried the where-clause form:

```
echo 'fn apply<F>(f: F, x: i32) -> i32 where F: Fn(i32) -> i32 { f(x) }
fn main() -> i32 { apply(|x| x * 2, 5) }' > /tmp/test_where_fn.rs
cargo run -- /tmp/test_where_fn.rs
```

Output:
```
error: parse error at byte 44: expected OpenBrace, found OpenParen
```

Both parse error. Confirmed: `impl Fn(i32) -> i32` in parameter position works (parsed
as an opaque impl-trait type), but `F: Fn(i32) -> i32` in generic-bound position and
`where F: Fn(i32) -> i32` in where-clause position both fail.

Read `src/parser.rs` to understand the gap. In the generic-param bound loop (lines ~522–550),
after consuming a bound trait name (`Ident`), the parser checks for `Lt` (`<`) to handle
angle-bracket type args like `Trait<T>`, but does not check for `OpenParen` to handle the
parenthesized form `Fn(T) -> R`. When `(` is encountered, it falls through and expects
`Gt` — hence "expected Gt, found OpenParen."

Same pattern in `parse_where_clause` (lines ~908–935): bound name consumed, `(` not
handled, parse breaks early, caller sees `(` where `{` is expected — "expected OpenBrace,
found OpenParen."

**The worst moment:** All 43 fixtures compile. The Lead Researcher feels momentum — the
compiler is growing. They write the first natural next program: a generic higher-order
function with `F: Fn(i32) -> i32`. Parse error. The feature just one step beyond what's
implemented doesn't parse at all.

**Why it matters:** The `impl Fn` form (`apply(f: impl Fn(i32) -> i32, ...)`) is a
workaround, but it's not equivalent to the generic form: you can't call a generic function
with an explicit type argument when using `impl Fn`, you can't name the type in other
bounds, and the where-clause form is idiomatic for multi-bound generic functions. The FLS
§4.14 explicitly specifies "ParenthesizedTraitBound" as a distinct grammar form — not
implementing it means galvanic claims §4.14 compliance while silently rejecting half of it.

---

## Goal

**Parse FLS §4.14 parenthesized trait bounds (`Fn(T) -> R`) in generic parameter lists
and where clauses, and emit assembly for generic higher-order functions that use them.**

### What to change

**1. Generic parameter bound parsing (parser.rs, ~line 522–550):**

After consuming the bound trait name (`Ident`), the loop that handles `Trait<T>` type
args should also handle `Trait(T1, T2) -> Ret` parenthesized args. When `OpenParen` is
seen after the trait name, consume the parenthesized argument list (matching parens,
skipping tokens) and the optional `-> ReturnType` tail (consume `Arrow` and the return
type tokens). The parsed bound is consumed and discarded — galvanic's monomorphization
treats all generic types as i32 equivalents at this milestone — but the parse must not
fail.

**2. Where-clause bound parsing (parser.rs, ~line 908–935):**

Same fix in `parse_where_clause`: after a bound trait name, also handle `OpenParen` by
consuming the parenthesized arg list and optional `-> Ret` before checking for `+` or `,`.

**3. New fixture `tests/fixtures/fls_4_14_fn_bounds.rs`:**

```rust
// FLS §4.14: Parenthesized trait bounds for callable types (Fn, FnMut, FnOnce).
// Tests both generic-param form and where-clause form.

fn apply_generic<F: Fn(i32) -> i32>(f: F, x: i32) -> i32 {
    f(x)
}

fn apply_where<F>(f: F, x: i32) -> i32
where
    F: Fn(i32) -> i32,
{
    f(x)
}

fn main() -> i32 {
    let r1 = apply_generic(|x| x * 2, 5);
    let r2 = apply_where(|x| x + 3, r1);
    r2
}
```

The expected result: both calls go through the trampoline mechanism already used by
`impl Fn` closures. Assembly should contain a `blr` for the indirect closure call.

**4. Assembly inspection test in `tests/e2e.rs`:**

```rust
#[test]
fn fls_4_14_fn_bounds_emits_indirect_call() {
    let src = include_str!("fixtures/fls_4_14_fn_bounds.rs");
    let asm = compile_to_asm(src);
    assert!(asm.contains("blr"), "expected indirect call via blr for closure dispatch");
    assert!(asm.contains("apply_generic"), "expected apply_generic function");
    assert!(asm.contains("apply_where"), "expected apply_where function");
}
```

**5. AMBIGUOUS annotation and ref entry:**

Add `// FLS §4.14 AMBIGUOUS: The spec defines parenthesized trait bounds for Fn, FnMut,
FnOnce only. It does not specify whether the `Trait(T) -> R` parenthesized syntax is
valid for non-callable traits. Galvanic accepts parenthesized bounds syntactically for
any trait name.` near the parsing change.

Add a matching entry in `refs/fls-ambiguities.md`:
```
## §4.14 — Parenthesized Trait Bound Scope
**Gap:** The FLS defines `Fn(T) -> R` as a special-cased bound syntax for callable traits
(§4.14: "ParenthesizedTraitBound"). It does not specify whether this syntax is permissible
for non-callable traits (e.g., `T: MyTrait(Arg) -> Ret`).
**Galvanic's choice:** Parenthesized bounds are parsed syntactically for any trait name.
The runtime behavior is only defined for `Fn`/`FnMut`/`FnOnce`.
**Minimal reproducer:** `fn apply<F: Fn(i32) -> i32>(f: F, x: i32) -> i32 { f(x) }`
```

### Why this is the most valuable change right now

The Lead Researcher's signal is **momentum** — the compiler getting smarter, new
constructs working, the two research questions inching toward real answers.

With all 43 fixtures compiling, the Lead Researcher needs a new FLS section to implement.
`Fn(T) -> R` parenthesized bounds in generic position is the single nearest-neighbor
feature to what already works: closures compile via `impl Fn`, but the generic-bound form
that FLS §4.14 explicitly specifies doesn't parse. The fix is in the parser (two symmetric
spots), and the underlying lowering via the trampoline mechanism already exists. This is
not a new pipeline stage — it's an extension of an existing one.

The fix also captures a genuine FLS finding: the spec's scope for parenthesized bounds is
underspecified (callable-trait-only or any-trait?). That's a new ambiguity entry in
`refs/fls-ambiguities.md` — the research artifact grows.

### The specific moment

Step 3 of the Lead Researcher journey: 0 parse-only fixtures, so the standard path is
blocked. Pivot to new FLS section. Write:

```rust
fn apply<F: Fn(i32) -> i32>(f: F, x: i32) -> i32 { f(x) }
fn main() -> i32 { apply(|x| x * 2, 5) }
```

`cargo run -- /tmp/test_hof.rs` → `error: parse error at byte 55: expected Gt, found OpenParen`

The feature just one step beyond existing closure support doesn't parse. The wall is at
the parser, not at lowering or codegen.
