# Changelog — Customer Champion Cycle 013

## Stakeholder: The Spec Researcher

**Became:** A Spec Researcher — an FLS contributor studying galvanic's documented findings
to bring concrete ambiguities back to spec authors. They want each finding to be
verifiable: a specific gap, a concrete program, and galvanic's documented resolution.

**Rotation rationale:** Cycle 012 served the Lead Researcher (most recently). Cycle 011
served the Compiler Contributor. Cycle 010 served the Spec Researcher (most under-served,
3 cycles ago). The Spec Researcher is due.

---

## Floor check

2063 pass, 0 fail. Clippy OK. Build OK. Unsafe audit OK. Floor intact.

---

## What I experienced

Walked steps 2–8 of the Spec Researcher journey.

Step 2: `refs/fls-ambiguities.md` has a 47-entry TOC, navigable by section number.
Looked at the TOC — several entries near §10–§12 were interesting (generics territory,
recently active in the commit log).

Step 4: `grep -r 'AMBIGUOUS' src/ | grep '§11'` — found annotations at `src/ast.rs:384`
and `src/ast.rs:388` about `impl<T>` disambiguation and `unsafe impl` enforcement.

Step 5: Navigated to the §11 entry in refs. Found it, read the finding. The gap is real
and specific: the FLS doesn't say how to disambiguate `impl<T> Foo<T>` from
`impl Foo<SomeType>` when `SomeType` is a single-letter identifier.

Step 6: The finding is specific enough to act on. But then:

> **Minimal reproducer:** Not yet demonstrable — generic `impl<T>` disambiguation
> and `unsafe impl` enforcement both involve features not compiled end-to-end
> at this milestone.

The Spec Researcher reads "Not yet demonstrable" and skips step 7. The entry tells them
it's impossible to verify. They move on.

**But I tried the reproducer anyway:**

```rust
struct Wrapper<T> { inner: T }
impl<T> Wrapper<T> { fn get(self) -> T { self.inner } }
fn main() -> i32 {
    let w = Wrapper { inner: 7_i32 };
    w.get()
}
```

Output: `galvanic: emitted /tmp/generic_impl.s` — **compiled clean.** The §11 finding IS
demonstrable. Galvanic's disambiguation choice (treat single-letter params as type params
in `impl<T>`, not as concrete types) is verifiable in the emitted assembly. The stale note
blocked discovery that didn't need to be blocked.

Checked §10.2 ("Self::X Projection Resolution in Default Methods"): also says "Not yet
demonstrable — requires generic trait machinery...`fls_12_1_generic_trait_impl.rs` is
parse-only." Tried a `type Item = i32` associated type program. Also compiled clean.

Checked §12.1 ("Generic `>>` Token Disambiguation"): says "not demonstrable because
fixture is parse-only." Tried `fn unwrap<T>(x: W<W<T>>) -> T` — still fails with
"expected CloseParen, found Eof." But the reason given in the note is wrong: the fixture
compiles now (cycle 011 fixed it). The `>>` disambiguation still fails — but because `>>`
in type annotations doesn't parse, not because any fixture is "parse-only."

**The worst moment:** Step 6 — reading the §11 entry and stopping because "Not yet
demonstrable." I had already verified in 20 seconds that it compiles. The note was
written against a compiler state that's 3 milestones old. The Spec Researcher who trusts
the note loses a real finding.

**The hollowest moment:** The §10.2 entry also cites `fls_12_1_generic_trait_impl.rs` as
"parse-only" — a fixture that compiles end-to-end since cycle 011. Two entries cite the
same stale reason. The "parse-only" note was accurate when it was written, became stale
when cycle 011 shipped, and no one updated the refs.

---

## The class

These three entries share a pattern: milestone-stamped "not yet demonstrable" language
tied to a specific fixture's status. When the fixture changes, the note goes stale. The
fix is not to rewrite the language — it's to replace fixture-based reasons with
capability-based reasons, and to verify each entry against the current compiler.

The right language for a finding that IS now demonstrable: add an actual working
reproducer. The right language for a finding that STILL isn't demonstrable: say why in
terms of what capability is missing ("`>>` in type annotations doesn't parse") not which
fixture happened to be parse-only at the time.

---

## Goal

**Update the three stale "Not yet demonstrable" entries in `refs/fls-ambiguities.md`
(§10.2, §11, and §12.1) to reflect current compiler capabilities.**

### §11 — `impl` Generics (now demonstrable)

Replace the current "Not yet demonstrable" block with a working reproducer:

```rust
struct Wrapper<T> { inner: T }
impl<T> Wrapper<T> { fn get(self) -> T { self.inner } }
fn main() -> i32 {
    let w = Wrapper { inner: 7_i32 };
    w.get()
}
```

Assembly signature: look for a mangled call like `bl Wrapper__get__i32` — galvanic treats
`T` as a type parameter (not a concrete type alias), confirmed by monomorphization to the
concrete type. This demonstrates galvanic's choice for the disambiguation gap.

Note: the `unsafe impl` enforcement half of this entry is still not demonstrable — no
reproducer can show what a compiler must CHECK when `unsafe impl` is written, because
galvanic parses it and enforces nothing. Keep that half's "not demonstrable" note with
the accurate reason.

### §10.2 — Self::X Projection (now demonstrable)

Replace the current "Not yet demonstrable" block with a working reproducer:

```rust
trait Container {
    type Item;
    fn get(self) -> Self::Item;
}
struct Box<T> { val: T }
impl Container for Box<i32> {
    type Item = i32;
    fn get(self) -> i32 { self.val }
}
fn main() -> i32 {
    let b = Box { val: 42_i32 };
    b.get()
}
```

Assembly signature: look for the concrete method call `bl Box__get` (or similar) — galvanic
resolves `Self::Item` to `i32` at monomorphization time (not at trait-definition time),
which is galvanic's documented choice for the §10.2 gap.

### §12.1 — Generic `>>` Disambiguation (still not demonstrable, wrong reason)

Update the "Not yet demonstrable" note to state the accurate current reason:

> **Minimal reproducer:** Not yet demonstrable. `>>` in generic type annotations still
> fails to parse — `fn unwrap<T>(x: W<W<T>>) -> T` produces "expected CloseParen, found
> Eof". The `>>` split described in Galvanic's choice operates at parse depth tracking but
> does not yet handle `>>` in function-signature type positions. When nested generic type
> annotations parse correctly, the assembly signature will show: the correct return value
> loaded via nested field accesses, confirming the split was applied at parse time.

Remove the incorrect reference to `fls_12_1_generic_trait_impl.rs` being "parse-only" —
that fixture compiles end-to-end since cycle 011.

### Why this and not something else

The refs_reproducers_all_compile test (cycle 010) catches reproducers that fail when they
should compile. But "Not yet demonstrable" entries have NO code block — so the test
doesn't catch them. They're a blind spot: code the Spec Researcher would try, that would
compile, but they're told not to try.

A stale "not yet demonstrable" note actively blocks discovery. The Spec Researcher reads
it, trusts it, and skips the one step (step 7 — running the reproducer) that would
confirm the finding. That's the exact moment the experience breaks: they had a finding in
hand and a note told them they couldn't verify it.

### The specific moment

Step 6 of the Spec Researcher journey, §11 entry: "Not yet demonstrable — generic
`impl<T>` disambiguation...not compiled end-to-end at this milestone." The finding is
specific, the source annotation is real. But the Spec Researcher stops here. They don't
try the reproducer because the docs say not to. The reproducer compiles in 0.3 seconds.
