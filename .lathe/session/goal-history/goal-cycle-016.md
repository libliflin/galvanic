# Goal — Cycle 016

**Stakeholder:** Compiler Contributor

**What to change:** Add FLS section citations to the "not yet supported" error strings in `src/lower.rs` — bringing the error messages into compliance with the documented invariant that errors name the failing function, the FLS section, and the specific construct.

**Why this stakeholder:** The last four goals served Cache-Line Performance Researcher (cycle 015), Lead Researcher (cycle 014), Spec Researcher (cycle 013), Lead Researcher (cycle 012). Compiler Contributor was last served at cycle 011 — four cycles ago, the most under-served stakeholder.

**Why now:** The architecture doc (`lower.rs` section) explicitly documents: *"When lowering fails (unsupported construct), it emits an error naming the failing function, the FLS section, and the specific construct."* The implementation violates this invariant in 39 of 42 "not yet supported" occurrences — confirmed by grep: `grep -c "not yet supported.*FLS" src/lower.rs` returns 3, while the total is 42. A Compiler Contributor following Step 4 of their journey ("which FLS section does the comment cite?") hits a dead end on almost every error they encounter. They have no spec anchor point — they must read surrounding code and cross-reference the FLS TOC from scratch.

**The specific moment:** I ran `cargo run -- tests/fixtures/fls_5_patterns.rs` and got:
```
error: lower failed in 'main': not yet supported: expected struct literal `Inner { .. }` for nested struct field
```
The error names the failing function ("main") — good. It describes the construct it expected. But it cites no FLS section. The Compiler Contributor now has to trace `store_nested_struct_lit` at lower.rs:6397–6453, find the FLS §6.11 comment embedded in the function doc, and separately look up §5.10.2 and §9.2 in the FLS TOC. This is recoverable — but it shouldn't require archaeology. Compare with the 3 messages that do cite: `"cast to bool not yet supported (FLS §6.5.9)"` — the spec section is right there. That version lets the Contributor go directly to the spec. The others don't.

**The class of fix, not the instance:** The architecture doc already defines the contract; the implementation just doesn't fulfill it. The fix is: add `(FLS §X.Y)` tags to the error strings in `LowerError::Unsupported(...)` calls throughout `lower.rs`. Three messages do this correctly already — they're the template. Apply that template to the remaining 39. Once done, any reviewer can verify the invariant at a glance: every "not yet supported" string ends with a FLS section. Any new error without one breaks the pattern and gets caught in review.

The builder should cover at minimum the hot-path errors a new contributor would encounter first:
- Let patterns: "complex let pattern not yet supported" (line 8815)
- If-let: "tuple pattern in if-let not yet supported" (line 11958), "slice/array pattern in if-let not yet supported" (line 11963)
- While-let: "tuple pattern in while-let not yet supported" (line 15969), "slice/array pattern in while-let not yet supported" (line 15974)
- Match: "match arm pattern type not yet supported" variants (lines 7224, 7473, 7809)
- @ binding: "@ binding sub-pattern not yet supported" variants (lines 12058, 12963, 13642, 16030)
- Nested struct: "expected struct literal `{struct_name} {{ .. }}` for nested struct field" (line 6406)
- Enum/struct methods: "&mut self methods on enum types not yet supported" (line 3802)

Full coverage of all 42 is preferable if achievable in one pass.

**Lived experience note:** I became the Compiler Contributor. I cloned the repo (already had it), ran `cargo test` (2082 tests, all green — solid floor). Then I went hunting for a "not yet supported" error to contribute to. I ran the patterns fixture: one function failed. The error said "not yet supported: expected struct literal `Inner { .. }` for nested struct field." I searched for the string, found it at lower.rs:6406 inside `store_nested_struct_lit`. The function has an FLS comment in its doc (`// FLS §6.11: Struct expressions`). But the error string itself doesn't carry that citation. I wanted to find which FLS section to read — I had to dig into the function documentation to find it. Then I checked other errors to see if this was systematic or a one-off. I ran `grep -c "not yet supported.*FLS" src/lower.rs` and got 3. I ran the total count and got 42. The hollowest moment: realizing the architecture doc makes a specific promise ("names the FLS section") that 93% of the error messages break. The Contributor isn't missing context because the spec is ambiguous — they're missing it because the error message left it out when it shouldn't have.
