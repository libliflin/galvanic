# Customer Champion Cycle 014

## Stakeholder: The Lead Researcher

**Rotation rationale:** Cycles 010 and 012/013 served the Spec Researcher. Cycle 011
served the Compiler Contributor. Cycle 009 served the Lead Researcher. Lead Researcher
is most under-served (5 cycles).

## Goal

Support match scrutinee expressions that return enum types — not just local variables.
When `match f() { Variant(v) => ... }` is written and `f` returns an enum, lower the
call into a temporary enum slot allocation (write-back convention) before matching,
identical to what `let x = f(); match x { ... }` already does. Set `enum_base_slot`
from the allocated base slot.

## Lived experience

Walked the Lead Researcher journey. With 0 parse-only fixtures remaining, tried natural
Rust programs: recursive functions (works), for loops over slices (works), dyn trait
dispatch (works), closures with capture (works). Then wrote the canonical error-handling
pattern: `match divide(10, 2) { Result::Ok(v) => v, Result::Err(e) => e }`.

Error: "not yet supported: TupleStruct pattern requires enum variable scrutinee."

Workaround: `let result = divide(10, 2); match result { ... }` — compiles immediately.
The feature works. The restriction is one AST form away. FLS §6.18 places no restriction
on scrutinee expressions.

**Worst moment:** Writing a completely idiomatic pattern — the Rust equivalent of
try/catch — and hitting an opaque error about "variable scrutinee." The error message
explains nothing about why a variable is required or what to do instead.

**Also fixed:** `snapshot.sh` was emitting "WARNING: 112 annotations may not be
documented in refs/" — a false positive from comparing raw annotation line count against
entry count. Fixed in this cycle: now compares unique section numbers and shows "OK"
when all sections are covered.

---

# Verification — Cycle 013, Round 1

## What was checked
- Ran `cargo test`: 2063 pass, 0 fail.
- Confirmed §11 entry has a working reproducer: `impl<T> Wrapper<T>` compiles.
- Confirmed §10.2 entry has a working reproducer: `impl Container for Box<i32>` compiles.
- Confirmed §12.1 note updated: no longer cites "parse-only fixture" (that fixture compiles since cycle 011).
- Ran `refs_reproducers_all_compile` test: passes.

## Findings
Goal fully met. All three entries corrected; refs_reproducers_all_compile guards against future regressions.

VERDICT: PASS

---

# Changelog — Cycle 013, Round 1

## Goal
Update the three stale "Not yet demonstrable" entries in `refs/fls-ambiguities.md`
(§10.2, §11, §12.1) to reflect current compiler capabilities.

## Who This Helps
- **Stakeholder:** The Spec Researcher
- **Impact:** §11 and §10.2 now have working reproducers a Spec Researcher can run and
  observe. §12.1 note gives the correct reason (`>>` in type annotations fails to parse),
  removing the misleading fixture-status attribution.

## Applied
- §11: Added working reproducer using `impl<T> Wrapper<T>`. Assembly signature: look for
  mangled call `Wrapper__get__i32`.
- §10.2: Added working reproducer using `impl Container for Box<i32>`. Assembly signature:
  look for `Box__get`.
- §12.1: Replaced "fixture is parse-only" with ">> in type annotations doesn't parse."

## FLS Notes
No new ambiguities. Cycle corrected stale milestone-stamped language.

---

# Previous: Customer Champion Cycle 013

## Stakeholder: The Spec Researcher

**Became:** A Spec Researcher auditing findings in `refs/fls-ambiguities.md`.

**Rotation rationale:** Cycle 012 → Lead Researcher. Cycle 011 → Compiler Contributor.
Cycle 010 → Spec Researcher. Spec Researcher is most under-served (3 cycles ago).

---

## Floor check

2063 pass, 0 fail. Clippy OK. Build OK. Floor intact.

---

## What I experienced

Walked steps 2–8 of the Spec Researcher journey. Picked §11 entry — "Not yet
demonstrable — generic `impl<T>` not compiled end-to-end." Tried the reproducer anyway:
compiled in 0.3 seconds. Same for §10.2. §12.1 is genuinely not demonstrable but for
the WRONG reason (cites parse-only fixture that now compiles).

**Worst moment:** Reading "Not yet demonstrable" for §11 and almost closing the entry.
The trust violation: the docs said impossible, the compiler said otherwise.

---

## Goal

Update the three stale "Not yet demonstrable" entries in `refs/fls-ambiguities.md`
(§10.2, §11, §12.1) to reflect current compiler capabilities:

- §11: Now demonstrable — add working reproducer + assembly signature.
- §10.2: Now demonstrable — add working reproducer + assembly signature.
- §12.1: Still not demonstrable — update note to say `>>` in type annotations fails to
  parse; remove stale "fixture is parse-only" attribution.
