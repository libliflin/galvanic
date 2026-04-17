# Changelog — Customer Champion Cycle 006

## Stakeholder: The Lead Researcher

**Became:** The Lead Researcher — the author using galvanic as a research instrument, extending
the compiler feature by feature, checking that the emitted assembly is correct and FLS-compliant.

**Rotation rationale:** Cycle 004 served the Spec Researcher. Cycle 005 served the Compiler
Contributor. The Lead Researcher has not been served for two cycles — their turn.

---

## Floor check

Build: OK. Tests: 2052 pass, 0 fail. Clippy: OK. Unsafe audit: OK. Floor intact.

---

## What I experienced

Walked step 3 of the Lead Researcher journey: picked a parse-only fixture. The snapshot lists three:
`fls_4_14_where_clauses_on_types.rs`, `fls_2_4_literals.rs`, and `fls_12_1_generic_trait_impl.rs`.

Chose `fls_2_4_literals.rs` — the most foundational of the three: FLS §2.4 covers the entire
literal syntax, the fixture contains examples straight from the spec, and it has a `fn main` so it
should produce assembly when it compiles.

```
cargo run -- tests/fixtures/fls_2_4_literals.rs
```

Output:
```
galvanic: compiling fls_2_4_literals.rs
parsed 1 item(s)
error: lower failed in 'main': not yet supported: cannot parse float literal: `8_031.4_e-12f64`
lowered 0 of 1 functions (1 failed)
```

**The worst moment:** The failing literal `8_031.4_e-12f64` is a verbatim example from FLS §2.4.4.2.
It's in galvanic's own fixture file. The spec gives us the example; we put it in the fixture;
the fixture is parse-only because galvanic can't compile it. That's the definition of a gap
between the documented intent and the actual behavior.

**Root cause (traced in source):** `parse_float_value` in `src/lower.rs:4089` strips the suffix by
trying `strip_suffix("_f64")` then `strip_suffix("_f32")`. The literal `8_031.4_e-12f64` ends with
`f64` (no leading underscore), so neither strip succeeds. After stripping underscores from the full
text `8_031.4_e-12f64`, the result is `8031.4e-12f64` — which Rust's `f64::from_str` rejects
because `f64` is not a valid float token for the standard parser. Same bug exists in
`parse_float32_value` for bare `f32` suffixes.

The FLS §2.4.4.2 ambiguities entry (`refs/fls-ambiguities.md:90`) currently reads: "Only decimal
float literals with optional `_f32`/`_f64` suffix are supported." The bare `f32`/`f64` form is
listed as unsupported when it should be a parse-level fix, not a fundamental limitation.

---

## Goal

**Fix `parse_float_value` and `parse_float32_value` in `src/lower.rs` to handle bare `f64`/`f32`
suffixes (without the leading underscore separator).**

**What to change:**
- In `parse_float_value` (line ~4091): after trying `strip_suffix("_f64")`, also try
  `strip_suffix("f64")`. Same for `f32` variants. Try in longest-first order so `_f64` is
  matched before `f64` (this avoids incorrectly treating `_f64` as `_f` + `64`).
- In `parse_float32_value` (line ~4107): same fix for bare `f32`.
- In the `ends_with("_f32")` check at line ~10716 (the f32 context dispatch): also check
  `text.ends_with("f32")` so that bare-suffix f32 literals route to the f32 codepath.
- Update `refs/fls-ambiguities.md` §2.4.4.2: remove the claim that only `_f32`/`_f64` are
  supported; bare suffixes will now work. The remaining gap (NaN, infinity, hex floats) is
  unchanged.

**After this fix:** `fls_2_4_literals.rs` should progress past the float literal. If it
then hits another error, the summary line will show which function failed and what succeeded —
the Lead Researcher can iterate. If all literals work, this fixture moves from parse-only to
end-to-end, and the FLS §2.4 literal suite will be fully covered.

**Why this is the most valuable change right now:**

The Lead Researcher's signal is momentum — the compiler getting smarter, new constructs working.
The `fls_2_4_literals.rs` fixture is the most foundational parse-only fixture: it's FLS §2.4
from first principles. It has a `fn main`, so fixing it produces assembly. The blocker is a
two-line suffix-stripping bug. Fixing it is a direct FLS compliance gain: the spec's own example
(`8_031.4_e-12f64`) compiles in galvanic.

**The specific moment:** Step 6 of the Lead Researcher journey, running
`galvanic tests/fixtures/fls_2_4_literals.rs`. The error was "cannot parse float literal:
`8_031.4_e-12f64`" — the spec's own example in galvanic's own fixture file. The fix is in
`parse_float_value` at `src/lower.rs:4091`: also strip bare `f64`/`f32` suffixes.
