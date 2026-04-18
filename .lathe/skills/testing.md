# Testing

## Test Suite Structure

```
tests/
  smoke.rs          — Unit-level CLI and pipeline integration tests (no ARM64 toolchain required)
  fls_fixtures.rs   — Parse acceptance tests for all fixtures in tests/fixtures/
  e2e.rs            — Full pipeline tests: lex → parse → IR → codegen → assemble → link → run (requires aarch64-linux-gnu-as/ld and qemu-aarch64)
  fixtures/
    fls_*.rs        — One file per FLS section. Named fls_<section>_<topic>.rs
```

## Running Tests

```bash
cargo test                    # all unit + smoke + fixtures (no toolchain required)
cargo test --test e2e         # end-to-end (requires ARM64 cross toolchain)
cargo test --test smoke       # smoke tests only
cargo clippy -- -D warnings   # clippy as CI runs it
cargo bench --bench throughput # benchmarks
```

## What Each Suite Checks

**smoke.rs** — The compiler's observable behavior as a CLI tool:
- Empty file exits 0 with "galvanic: compiling" in stdout
- Lower errors name the failing function and cite FLS sections
- Summary line format: "lowered N of M functions (K failed)"
- All errors reported (not just first)
- Partial success emits assembly even when some functions fail
- No-main case prints "lowered N function(s) — no fn main, no assembly emitted"
- Every "not yet supported" string in `lower.rs` carries `(FLS §X.Y)` — static source check

**fls_fixtures.rs** — Parse acceptance: every file in `tests/fixtures/fls_*.rs` must parse without error (even if lowering fails). Ensures no fixture is silently broken at the parse level.

**e2e.rs** — Full pipeline correctness:
- Specific programs produce specific ARM64 assembly patterns
- Cache-line invariants: `mov w0, #N` for constants (no const folding in non-const fns)
- Bounds checks emitted: `cmp`/`b.hs` before every array index
- Panic paths: `_galvanic_panic` for zero-division, out-of-bounds, shift overflow

## Adding a New Test

**For a new FLS section fixture:**
1. Create `tests/fixtures/fls_<section>_<topic>.rs` with a complete program exercising the section
2. The fixture will be auto-detected by `fls_fixtures.rs`
3. Add an assembly inspection test in `e2e.rs` if there's a specific assembly pattern to verify

**For a new invariant:**
- If it's a CLI behavior, add to `smoke.rs`
- If it's an assembly pattern, add to `e2e.rs`
- If it's a source-level invariant (like the FLS citation check), add a static source-scan test to `smoke.rs`

## Test Conventions

- Smoke tests use `tempfile::NamedTempFile` for ephemeral inputs; fixtures for stable inputs
- E2e tests assert specific assembly patterns with `asm.contains("pattern")` — prefer specific over broad
- When a smoke test exercises a fixture file that partially compiles, document which functions fail and why in the test comment
- When a test was added to pin a specific error message, add a comment explaining the stakeholder context (e.g., "cycle 028 goal: contributor seeing this error needed FLS §6.18 + §6.10 citations")

## CI Jobs

| Job | What it checks | Toolchain needed |
|-----|----------------|------------------|
| build | `cargo build` | Rust stable |
| test | `cargo test` | Rust stable |
| clippy | `cargo clippy -- -D warnings` | Rust stable + clippy |
| fuzz-smoke | CLI robustness: no-args, missing file, empty, large input, nested braces, garbage, NUL, long lines | Rust stable (release) |
| audit | No unsafe code, no Command outside main.rs, no network deps | bash + grep |
| e2e | Full pipeline: compile + assemble + link + run | Rust stable + aarch64-linux-gnu-as/ld + qemu-aarch64 |
| bench | Throughput + data structure size assertions | Rust stable |

The `e2e` and `fuzz-smoke` jobs depend on `build`. Everything runs on `ubuntu-latest`.

## Fixture Naming Convention

`tests/fixtures/fls_<section>_<topic>.rs` where `<section>` uses underscores for dots: `fls_6_18_match_expressions.rs` for FLS §6.18. The snapshot script counts fixture files with this pattern.
