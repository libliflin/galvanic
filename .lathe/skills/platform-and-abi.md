# Platform & ABI

Full ABI reference: `refs/arm64-platform-abi.md`

## Key points for builders and verifiers

1. **Galvanic targets both macOS and Linux ARM64.** Same instructions, different syscall ABI and binary format. See the ref for the full comparison table.

2. **Only Linux codegen is implemented today.** macOS codegen (Mach-O, `svc #0x80`, `x16` syscall register) is an open gap. Until it's implemented, runtime tests skip on macOS.

3. **Skipped tests look like passing tests.** On macOS, `compile_and_run()` returns `None` and the test harness reports "ok." Hundreds of runtime tests appear green but never ran. Only `compile_to_asm()` assembly inspection tests truly execute.

4. **CI (Linux) is the only runtime truth** until macOS codegen lands. Never declare PASS for codegen changes based on macOS results alone.

5. **Overflow guards hit ALL operations of an IR type.** `IrBinOp::Add` is used for both user i32 arithmetic and pointer/index math. A guard meant for i32 will also fire on address calculations. You won't see this on macOS — only CI catches it.

6. **i32::MIN literal doesn't parse.** `-2147483648` is parsed as negation of `2147483648` which exceeds i32. Use `(-2147483647 - 1)` instead.
