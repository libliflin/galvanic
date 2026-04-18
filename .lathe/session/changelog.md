# Changelog — Cycle 016, Round 1 (Builder)

## Goal
Add FLS section citations to "not yet supported" error strings in `src/lower.rs`
to fulfill the architecture invariant: every error names the failing function,
the FLS section, and the specific construct. The goal counted 42 total messages,
3 already cited, 39 uncited.

## Who This Helps
- **Stakeholder:** Compiler Contributor
- **Impact:** A contributor who reads "not yet supported: complex let pattern"
  can now navigate directly to FLS §8.1 to understand the spec context — without
  having to read surrounding code or know the FLS TOC. The wall between "error
  message" and "spec section" is removed.

## Applied
Added FLS section citations to all remaining "not yet supported" error strings
(21 `.into()` strings and 5 `format!` strings), prioritizing hot-path errors as
specified. Every string now follows the pattern established by the three existing
cited messages.

**Changes by category:**

- **Let patterns:** `"complex let pattern not yet supported"` → `(FLS §8.1, §5)`;
  `"nested tuple pattern not yet supported"` → `(FLS §5.10.3, §8.1)`
- **If-let patterns:** tuple, slice/array, @ binding → `(FLS §6.17, ...)` with
  appropriate §5 subsection
- **While-let patterns:** tuple, slice/array, @ binding → `(FLS §6.15.4, ...)`
- **Match patterns:** all three "match arm pattern type not yet supported in
  *-returning match" strings → `(FLS §6.18)`; @ binding in match and unit match
  → `(FLS §6.18, §5.1.4)`
- **Nested struct:** `"expected struct literal for nested struct field"` → `(FLS §6.11, §5.10.2)`
- **Method errors:** `"method call on non-variable receiver"`,
  `"method calls on primitive types"`, `"dyn Trait method calls with extra arguments"`,
  `"&mut self methods on enum types"` → `(FLS §6.12.2, ...)` with §10.1 or §4.13 as applicable
- **Indexing/assignment:** index assignment, assignment to non-variable place,
  compound assignment, index expression on non-variable base → `(FLS §6.9)`,
  `(FLS §6.5.10)`, `(FLS §6.5.11)`
- **Literals:** unsigned MOVZ+MOVK → `(FLS §2.4.4.1)`
- **Function signatures:** multi-segment return type, complex return type,
  impl Trait return only struct-literal → `(FLS §9, §4)` / `(FLS §9, §11)`

**Files:** `src/lower.rs`

Final count: 0 "not yet supported" strings without FLS citations (excluding the
format impl at line 79 which is infrastructure, not a call site).

## Validated
- `cargo test` — 2082 pass, 0 fail (unchanged from pre-change count)
- `cargo clippy -- -D warnings` — clean
- `cargo run -- tests/fixtures/fls_5_patterns.rs` — the nested struct field
  error now reads: `not yet supported: expected struct literal \`Inner { .. }\`
  for nested struct field (FLS §6.11, §5.10.2)`

The verifier should run `cargo run -- tests/fixtures/fls_5_patterns.rs` and
confirm the error message includes the FLS citation. Additional spot-check:
`grep -c 'not yet supported.*FLS\|FLS.*not yet supported' src/lower.rs` should
return ≥ 20.
