# Changelog — Customer Champion Cycle 005

## Stakeholder: The Compiler Contributor

**Became:** A Compiler Contributor — a CS student who picked galvanic because it's
spec-driven, small enough to understand end-to-end, and has clear contribution
paths via parse-only fixtures. Walked steps 1–7 of the Compiler Contributor journey.

**Rotation rationale:** Cycle 004 served the Spec Researcher. Cycle 003 served the
Lead Researcher. Cycle 002 served the Compiler Contributor. Three cycles have passed
since the Compiler Contributor was served — their turn.

---

## What I experienced

Floor check: build OK, 2050 tests pass, clippy clean. Floor intact.

Step 4: picked `fls_12_1_generic_trait_impl.rs` from the parse-only fixture list
in the snapshot. It covers generic trait impls (`impl<T> Getter for Wrapper<T>`) —
a real FLS §12.1 feature with a single non-main function `use_it`.

Step 5: tried to run galvanic on it to see what happens before making any changes:

```
cargo run -- tests/fixtures/fls_12_1_generic_trait_impl.rs
```

Output:
```
galvanic: compiling fls_12_1_generic_trait_impl.rs
parsed 4 item(s)
```

Exit code 0. No output file. Nothing else.

**The hollowest moment:** I could not tell from this output whether `use_it` was
successfully lowered or whether lowering was silently skipped. The compiler
reported success (exit 0) but produced no assembly and gave no explanation. Was
the feature already working? Was lowering failing silently? Did I need to add a
`fn main` to see anything? The output didn't say.

Compared this to what happens on an error — `fls_9_functions.rs` produces:
```
error: lower failed in 'returns_unit': not yet supported: integer literal with non-integer type
lowered 19 of 20 functions (1 failed)
```

That's informative. The no-error case is a wall of silence.

Read `src/main.rs` lines 107–110 to understand why: when there's no `fn main`,
galvanic returns 0 without printing anything — by design. But that design is
invisible to a contributor who hasn't read `main.rs`.

The on-success path (with `fn main`) also has no lowering summary — you see
"galvanic: emitted foo.s" but no count of functions lowered. At least that tells
you SOMETHING happened. The no-fn-main path tells you nothing.

---

## Goal

**When galvanic successfully lowers a file but produces no assembly because there
is no `fn main`, print a note that names how many functions were lowered and
explains that no assembly was emitted.**

Before:
```
galvanic: compiling fls_12_1_generic_trait_impl.rs
parsed 4 item(s)
```
(exit 0, no output file, no explanation)

After:
```
galvanic: compiling fls_12_1_generic_trait_impl.rs
parsed 4 item(s)
galvanic: lowered 1 function — no fn main, no assembly emitted
```

The count comes from `module.fns.len()` — already computed before the `fn main`
check in `src/main.rs`. No new data needed, one new `println!` in the early-return
branch at line 108–110.

**Why this is the most valuable change right now:** It's a class-level fix for every
library-like fixture without an entry point. A contributor who picks any such
fixture gets: (a) confirmation that lowering ran and succeeded, (b) the count of
what was compiled, (c) a clear explanation of why there's no output file. Without
this, the contributor is left to wonder whether their feature is done, broken, or
was never attempted — with no way to tell from the output alone.

**The specific moment:** Step 4 of the Compiler Contributor journey, running
`galvanic tests/fixtures/fls_12_1_generic_trait_impl.rs`. The output was
`parsed 4 item(s)`, exit 0, nothing else. No way to know if `use_it` was
lowered, or if any error was silently swallowed.
