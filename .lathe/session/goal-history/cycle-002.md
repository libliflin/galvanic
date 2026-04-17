# Goal — Cycle 002

## Stakeholder: The Compiler Contributor

**Rotation rationale:** Cycle 001 served the Spec Researcher. The Lead Researcher dominated the 15 cycles before that. The Compiler Contributor has not been served in recent memory and the journey reveals a specific, concrete blocker.

---

## What to Change

When `galvanic` fails during lowering with "not yet supported", the error message must include the name of the item (function, constant, static, etc.) being lowered when the failure occurred.

**Currently:**
```
galvanic: compiling fls_9_functions.rs
parsed 19 item(s)
error: lower failed (not yet supported: integer literal with non-integer type)
```

**After this change:**
```
galvanic: compiling fls_9_functions.rs
parsed 19 item(s)
error: lower failed in 'const_add': not yet supported: integer literal with non-integer type
```

The item name is already known at the point where per-item lowering happens — the AST `Item` node has both a name and a `Span`. This information is discarded before the error is surfaced. Thread it through.

---

## Which Stakeholder This Helps and Why

A Compiler Contributor picks a parse-only fixture (`fls_9_functions.rs`, 19 items, 200+ lines), runs `cargo run -- tests/fixtures/fls_9_functions.rs`, and gets a message that tells them *what* failed but not *where in their 200-line file* to look.

They must then either:
- Comment out functions one by one to binary-search which triggers the error, or
- Add `eprintln!` statements to `src/lower.rs` to trace execution

Neither is how a new contributor should spend their time.

**The specific moment the experience turned:** Step 7 of the contributor journey — running `cargo run -- <fixture>` and reading the error output. The message is accurate but useless as a diagnostic: "not yet supported: integer literal with non-integer type" in a 19-item file. With the item name present, the contributor navigates directly to `const_add` and sees the u8/i128-typed literal that triggered the error.

---

## Why Now

This is a class-level fix. Every future "not yet supported" error — of which there will be many as contributors push parse-only fixtures toward full compilation — will carry context. There are currently 5 parse-only fixtures that hit this wall. The Compiler Contributor journey breaks at the same step for each one, and fixing the error format fixes all of them simultaneously.

The information already exists in the AST. This is not a new feature — it's surfacing what the pipeline already knows.

---

## Floor Check

Build: OK. Tests: 2047 pass, 0 fail. Clippy: OK. Unsafe audit: OK. Floor intact.

---

## Lived Experience Note

**Became:** A Compiler Contributor — a CS student or Rust enthusiast who cloned galvanic to understand compiler internals from first principles, guided by the FLS.

**What I tried:** Walked step 7 of the journey: ran `cargo run --` on each of the 5 parse-only fixtures to see what galvanic reports.

Results:
- `fls_6_18_match_expressions.rs` → `error: lower failed (not yet supported: tuple expression must be bound to a 'let' variable at this milestone)`
- `fls_2_4_literals.rs` → `error: lower failed (not yet supported: cannot parse float literal: '8_031.4_e-12f64')`  
- `fls_9_functions.rs` → `error: lower failed (not yet supported: integer literal with non-integer type)`
- `fls_4_14_where_clauses_on_types.rs` → `error: lower failed (not yet supported: expression kind in non-const context (runtime codegen not yet implemented))`

**The worst moment:** After getting `error: lower failed (not yet supported: integer literal with non-integer type)` for `fls_9_functions.rs`, I searched `src/lower.rs` for the error string and found it at line 10504 inside a `LitInt` match arm. I still didn't know which of the 19 functions in the fixture triggered it. The only path forward was to start commenting out items — the exact kind of archaeology work that signals an opaque pipeline.

**The hollowest moment:** The error message is grammatically correct and technically accurate. It says exactly what failed. But it gives a contributor no foothold — it names the symptom, not the location. A single item name would transform this from a wall into a door.
