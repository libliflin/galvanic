# Changelog — Cycle 137

## Who This Helps
- **William (researcher)**: Programs can now pass `&str` to functions and call
  `.len()` on the parameter. This is a concrete step from "only toy programs"
  toward "programs that look like real Rust." A function that takes a string
  argument and operates on it (e.g., `fn greeting_len(s: &str) -> usize { s.len() }`)
  now compiles and produces correct ARM64 output. The FLS §4.7 ambiguity
  (fat-pointer ABI for `str`) is documented explicitly in code.
- **Compiler Researchers**: The `local_str_slots` mechanism is extended from
  let-bindings to function parameters with a single targeted change. The comment
  explains why galvanic deviates from the fat-pointer ABI.

## Observed
- `fn foo(s: &str) -> usize { s.len() }` compiled without error but produced
  wrong code: `s.len()` inside the function could not match the `local_str_slots`
  path (Case B) because `local_str_slots` was only populated from `ExprKind::LitStr`
  let-binding initializers, never from parameters.
- The call site was already correct: `LitStr` lowering emits `LoadImm(r, byte_len)`,
  so the byte length arrives in `x0` at the callee. The only gap was that the callee
  didn't recognize its own parameter as a str slot.
- No existing milestone test covered `fn f(s: &str) -> usize { s.len() }` —
  all milestone 93 tests operate on let-bound string literals within `main`.

## Applied
- **`src/lower.rs`** (scalar parameter loop): After spilling the parameter register
  to its stack slot, check whether the declared type is `&str`
  (`TyKind::Ref { inner: TyKind::Path(["str"]) }`). If so, insert the slot into
  `ctx.local_str_slots`. The FLS §4.7 AMBIGUOUS deviation (single byte-length
  register vs. fat pointer) is documented in the new comment block.
- **`tests/e2e.rs`**: Added 10 new tests (milestone 192):
  - `milestone_192_str_param_len_hello` — basic 5-byte string
  - `milestone_192_str_param_len_empty` — empty string (0)
  - `milestone_192_str_param_len_in_arithmetic` — `str_len("hi") * 3 == 6`
  - `milestone_192_str_param_len_in_if` — `if str_len(...) > 3 { 1 } else { 0 }`
  - `milestone_192_str_param_len_let_binding` — result stored in let, then returned
  - `milestone_192_str_param_len_passed_to_fn` — result passed to second function
  - `milestone_192_str_param_two_params` — `fn f(a: &str, b: &str) -> usize { a.len() + b.len() }`
  - `milestone_192_str_param_called_twice` — two calls with different literals, summed
  - `runtime_str_param_len_emits_ldr_not_constant` — callee emits `ldr` from slot + `bl`
  - `runtime_str_param_len_not_folded` — combined result not folded to constant

## Validated
- `cargo build` — clean
- `cargo test --test e2e milestone_192` — 8 passed (compile_and_run skip on macOS)
- `cargo test --test e2e runtime_str_param` — 2 passed
- `cargo test` — 1661 e2e tests passed (was 1651), all suites clean
- `cargo clippy -- -D warnings` — clean

## FLS Notes
- **FLS §4.7 AMBIGUOUS**: The spec defines `str` as an "unsized slice of bytes
  encoded in UTF-8" and `&str` as a fat pointer (data pointer + byte length). The
  FLS does not specify the ABI for passing `&str` to functions. Galvanic passes
  `&str` as a single byte-length register. This matches the useful subset (`.len()`)
  while deferring pointer handling until string dereferencing is needed.
- **FLS §4.8**: Reference types are not their own value kind in galvanic's IR;
  `&str` resolves to `IrTy::I32` (the byte length). This is consistent with how
  the existing str literal code works but is a known divergence from the spec's
  fat-pointer semantics.

## Next
- The `&str` parameter support opens the door to more string-intensive programs.
  The next natural step is `&[T]` slice parameters (fat pointer: ptr + len) with
  `.len()` method, enabling functions to operate on arrays of unknown size.
- Alternatively, look at whether the two-parameter `&str` case would expose the
  register window limit (each `&str` uses one register, so eight parameters work;
  but a fat-pointer ABI would halve that capacity).
