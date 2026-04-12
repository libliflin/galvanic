//! ARM64 assembly text emission for galvanic.
//!
//! Takes an IR `Module` and writes GNU assembler (GAS) syntax suitable for
//! `aarch64-linux-gnu-as` (or the native `as` on an ARM64 host).
//!
//! # Target
//!
//! - Architecture: AArch64 (ARM64)
//! - OS ABI: Linux ELF
//! - Entry point: `_start` (bare; no libc startup)
//! - System call convention: syscall number in `x8`, args in `x0`–`x5`
//!
//! # FLS traceability
//!
//! - FLS §9: Functions — each `IrFn` emits a labeled function body.
//! - FLS §6.19: Return expressions — `Instr::Ret` emits `mov x0, #n; ret`.
//! - FLS §18.1: Program entry point — `_start` calls `main` and exits.
//!
//! # Cache-line note
//!
//! ARM64 instructions are 4 bytes each; 16 instructions fill one 64-byte
//! cache line. The `main` function for milestone 1 is exactly 2 instructions
//! (8 bytes), and `_start` is 3 instructions (12 bytes). Both fit entirely
//! within a single cache line. No explicit `.align` directives are needed
//! at this scale, but the rationale is documented here for future cycles.

use std::fmt::Write as FmtWrite;

use crate::ir::{IrBinOp, Instr, IrValue, Module};

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors that can occur during code generation.
#[derive(Debug)]
pub enum CodegenError {
    /// A language feature is not yet supported by the code generator.
    Unsupported(String),
    /// An internal string-formatting error (should not occur in practice).
    Fmt(std::fmt::Error),
}

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodegenError::Unsupported(msg) => write!(f, "codegen: not yet supported: {msg}"),
            CodegenError::Fmt(e) => write!(f, "codegen: format error: {e}"),
        }
    }
}

impl From<std::fmt::Error> for CodegenError {
    fn from(e: std::fmt::Error) -> Self {
        CodegenError::Fmt(e)
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Emit a module as ARM64 assembly text.
///
/// Returns a `String` containing valid GAS syntax. The caller is responsible
/// for writing it to a `.s` file and invoking the assembler.
///
/// The emitted file defines a `_start` symbol that calls `main` and then
/// invokes the Linux `sys_exit` syscall with `main`'s return value.
///
/// FLS §18.1: `main` is the program entry point.
pub fn emit_asm(module: &Module) -> Result<String, CodegenError> {
    let has_main = module.fns.iter().any(|f| f.name == "main");
    if !has_main {
        return Err(CodegenError::Unsupported("no `main` function in module".into()));
    }

    let mut out = String::new();

    writeln!(out, "    .text")?;

    for func in &module.fns {
        writeln!(out)?;
        emit_fn(&mut out, func)?;
    }

    // Emit trampoline functions for capturing closures passed as `impl Fn`.
    //
    // FLS §6.22, §4.13: Each trampoline bridges the gap between an `impl Fn`
    // caller (which passes only explicit arguments) and a capturing closure
    // (which expects captured values as leading register arguments).
    //
    // ARM64 design: captures are loaded into callee-saved registers x27/x26/…
    // by the caller before `bl apply`. The trampoline reads them from there,
    // shifts the explicit arguments up, and tail-calls the closure via `b`.
    // Because `b` is a tail call (not `bl`), x30 from `apply`'s `blr x9`
    // is preserved for the closure's `ret` to return directly to `apply`.
    //
    // Cache-line note: each trampoline is 3–6 instructions (12–24 bytes),
    // fitting comfortably within a 64-byte instruction cache line.
    for trampoline in &module.trampolines {
        writeln!(out)?;
        emit_trampoline(&mut out, trampoline)?;
    }

    // Emit vtable shim functions for dyn Trait dispatch (FLS §4.13).
    //
    // Each shim adapts the vtable calling convention (one data pointer in x0)
    // to the concrete method's calling convention (N struct fields in x0..x{N-1}).
    //
    // Shim layout (for n_fields > 0):
    //   vtable_shim_Trait_Type_N:
    //     mov  x9, x0                  // save data pointer in scratch x9
    //     ldr  x0,  [x9, #0]           // load field 0
    //     ldr  x1,  [x9, #8]           // load field 1
    //     ...
    //     b    Type__method             // tail-call concrete method
    //
    // For n_fields == 0 (unit struct):
    //   vtable_shim_Trait_Type_N:
    //     b    Type__method
    //
    // FLS §4.13 AMBIGUOUS: The FLS does not specify vtable shim layout.
    // Galvanic uses a load-from-data-pointer convention.
    // ARM64 note: x9 is an intra-procedure scratch register (ABI §6.1.1).
    // Cache-line note: each shim is 1 + n_fields + 1 instructions = n_fields + 2 words.
    if !module.vtable_shims.is_empty() {
        writeln!(out)?;
        writeln!(out, "    // FLS §4.13: vtable dispatch shims")?;
        for shim in &module.vtable_shims {
            writeln!(out)?;
            writeln!(out, "    .global {}", shim.name)?;
            writeln!(out, "    .align 2")?;
            writeln!(out, "{}:", shim.name)?;
            if shim.n_fields > 0 {
                writeln!(out, "    mov     x9, x0                       // FLS §4.13: save data ptr in scratch x9")?;
                for fi in 0..shim.n_fields {
                    let offset = fi * 8;
                    writeln!(out, "    ldr     x{fi}, [x9, #{offset:<16}] // FLS §4.13: load field {fi} from data ptr")?;
                }
            }
            writeln!(out, "    b       {:<28} // FLS §4.13: tail-call concrete method", shim.target)?;
        }
    }

    // Emit the bare _start entry point.
    writeln!(out)?;
    emit_start(&mut out)?;

    // Emit the _galvanic_panic primitive only when needed.
    //
    // FLS §6.23: Division by zero panics at runtime.
    // FLS §6.9: Out-of-bounds array indexing panics at runtime.
    //
    // The panic handler is emitted only if at least one instruction that can panic
    // appears in the module — programs without division or indexed access with bounds
    // checks never reference _galvanic_panic and don't need the symbol.
    //
    // This conditional emission also prevents the panic handler's literal constants
    // from contaminating assembly inspection tests for unrelated operations.
    //
    // Cache-line note: 3 instructions × 4 bytes = 12 bytes — fits in one cache line.
    let needs_panic = module.fns.iter().any(|f| {
        f.body.iter().any(|instr| match instr {
            Instr::BinOp {
                op:
                    IrBinOp::Div
                    | IrBinOp::Rem
                    | IrBinOp::UDiv
                    | IrBinOp::Shl
                    | IrBinOp::Shr
                    | IrBinOp::UShr,
                ..
            } => true,
            Instr::LoadIndexed { len, .. }
            | Instr::LoadIndexedF64 { len, .. }
            | Instr::LoadIndexedF32 { len, .. } => *len > 0,
            Instr::StoreIndexed { len, .. } => *len > 0,
            _ => false,
        })
    });
    if needs_panic {
        writeln!(out)?;
        emit_galvanic_panic(&mut out)?;
    }

    // Emit the .data section for static items.
    //
    // FLS §7.2: Static items reside in the data section.
    // FLS §7.2:15: All references to a static refer to the same memory address.
    //
    // Cache-line note: each static occupies 8 bytes (.quad). Eight statics
    // fill one 64-byte data cache line. We align each static to 8 bytes
    // (.align 3) to prevent two statics from sharing a single alignment unit
    // and to match the 64-bit LDR requirement on ARM64.
    //
    // FLS §7.2 AMBIGUOUS: The spec does not mandate a specific data section
    // alignment. Galvanic uses .align 3 (8-byte alignment) as the minimum
    // for correct 64-bit LDR addressing.
    if !module.statics.is_empty() {
        writeln!(out)?;
        writeln!(out, "    .data")?;
        for s in &module.statics {
            match s.value {
                crate::ir::StaticValue::Int(n) => {
                    writeln!(out, "    .align 3")?;
                    writeln!(out, "    .global {}", s.name)?;
                    writeln!(out, "{}:", s.name)?;
                    writeln!(out, "    .quad {n}")?;
                }
                crate::ir::StaticValue::F64(v) => {
                    // FLS §7.2, §4.2: f64 static stored as raw IEEE 754 bits (.quad).
                    // Cache-line note: 8 bytes — same footprint as an integer static.
                    let bits = v.to_bits();
                    writeln!(out, "    .align 3")?;
                    writeln!(out, "    .global {}", s.name)?;
                    writeln!(out, "{}:", s.name)?;
                    writeln!(out, "    .quad 0x{bits:016x}          // f64 {v} (FLS §7.2, §4.2)")?;
                }
                crate::ir::StaticValue::F32(v) => {
                    // FLS §7.2, §4.2: f32 static stored as raw IEEE 754 bits (.word).
                    // Cache-line note: 4 bytes — half an integer static. We still
                    // align to 4 bytes (.align 2) to satisfy ARM64 LDR requirements.
                    let bits = v.to_bits();
                    writeln!(out, "    .align 2")?;
                    writeln!(out, "    .global {}", s.name)?;
                    writeln!(out, "{}:", s.name)?;
                    writeln!(out, "    .word 0x{bits:08x}          // f32 {v} (FLS §7.2, §4.2)")?;
                }
            }
        }
    }

    // Emit .rodata section for float constants.
    //
    // FLS §2.4.4.2: Float literals are stored as raw IEEE 754 bit patterns.
    // Each constant is 8 bytes (.quad), 8-byte aligned.  The label
    // `{fn_name}__fc{idx}` matches the ADRP/ADD/LDR sequence in LoadF64Const.
    //
    // Cache-line note: each float constant occupies one 8-byte slot in the
    // cache line — identical footprint to a static item.  Using .rodata
    // (vs .data) tells the OS to map the page read-only, reducing TLB pressure
    // from store-miss writeback.
    let has_f64_floats = module.fns.iter().any(|f| !f.float_consts.is_empty());
    let has_f32_floats = module.fns.iter().any(|f| !f.float32_consts.is_empty());
    if has_f64_floats || has_f32_floats {
        writeln!(out)?;
        writeln!(out, "    .section .rodata")?;
        // f64 constants: 8 bytes each, 8-byte aligned (.align 3).
        for func in &module.fns {
            for (idx, &bits) in func.float_consts.iter().enumerate() {
                let val = f64::from_bits(bits);
                writeln!(out, "    .align 3")?;
                writeln!(out, "{}__fc{idx}:", func.name)?;
                writeln!(
                    out,
                    "    .quad 0x{bits:016x}          // f64 {val} (FLS §2.4.4.2)"
                )?;
            }
        }
        // f32 constants: 4 bytes each, 4-byte aligned (.align 2).
        //
        // Cache-line note: two f32 constants fit in one 8-byte cache-line
        // slot, vs one f64. The `.align 2` directive ensures `ldr s{n}, [x17]`
        // sees a 4-byte-aligned address as required by ARM64.
        for func in &module.fns {
            for (idx, &bits) in func.float32_consts.iter().enumerate() {
                let val = f32::from_bits(bits);
                writeln!(out, "    .align 2")?;
                writeln!(out, "{}__f32c{idx}:", func.name)?;
                writeln!(
                    out,
                    "    .word 0x{bits:08x}            // f32 {val} (FLS §2.4.4.2)"
                )?;
            }
        }
    }

    // Emit vtable data sections for dyn Trait dispatch (FLS §4.13).
    //
    // Each vtable is an array of 8-byte function pointers in .rodata, one per
    // trait method in declaration order. The label matches the `vtable_label`
    // computed in lower.rs (`vtable_{trait}_{type}`).
    //
    // Layout (for a trait with M methods):
    //   .section .rodata
    //   .align 3
    //   vtable_Trait_Type:
    //     .quad vtable_shim_Trait_Type_0
    //     .quad vtable_shim_Trait_Type_1
    //     ...
    //
    // FLS §4.13 AMBIGUOUS: The FLS does not specify vtable layout.
    // Galvanic uses a dense array of shim addresses, method 0 at offset 0.
    // Cache-line note: M methods = M × 8 bytes. For M ≤ 8, the vtable fits in
    // one 64-byte cache line.
    if !module.vtables.is_empty() {
        writeln!(out)?;
        writeln!(out, "    .section .rodata")?;
        for vtable in &module.vtables {
            writeln!(out, "    .align 3")?;
            writeln!(out, "    .global {}", vtable.label)?;
            writeln!(out, "{}:", vtable.label)?;
            for shim_label in &vtable.method_shim_labels {
                writeln!(out, "    .quad {shim_label:<32} // FLS §4.13: vtable entry")?;
            }
        }
    }

    Ok(out)
}

// ── Function emission ─────────────────────────────────────────────────────────

/// Compute the ARM64 stack frame size for a given number of 8-byte slots.
///
/// ARM64 ABI requires the stack pointer to be 16-byte aligned at all times.
/// We round up the raw byte count to the next multiple of 16.
///
/// Cache-line note: each 8-byte slot occupies one half of a 128-bit
/// (16-byte) alignment unit; two slots fill one aligned unit perfectly.
fn frame_size(stack_slots: u8) -> u32 {
    if stack_slots == 0 {
        return 0;
    }
    let raw = stack_slots as u32 * 8;
    // Round up to 16-byte alignment.
    (raw + 15) & !15
}

/// Emit one function.
///
/// FLS §9: Functions. Each function is a labeled sequence of instructions
/// ending with a `ret` (via `emit_instr`).
///
/// Stack layout (low address to high address, from the top of the frame):
///   [optional lr save slot]   — 16 bytes, pre-indexed push; only if saves_lr
///   [local variable slots]    — stack_slots * 8 bytes, rounded to 16; only if > 0
///
/// On entry to the function, sp points at the caller's frame boundary.
/// The prologue saves lr first (if needed), then allocates locals.
/// The epilogue restores locals first, then restores lr, then `ret`.
///
/// Cache-line note: `sub sp, sp, #N` is one 4-byte instruction — the frame
/// setup occupies one slot in the first cache line of the function body.
/// The lr save/restore pair (`str`/`ldr`) each adds one 4-byte instruction.
fn emit_fn(out: &mut String, func: &crate::ir::IrFn) -> Result<(), CodegenError> {
    writeln!(out, "    // fn {} — FLS §9", func.name)?;
    writeln!(out, "    .global {}", func.name)?;
    writeln!(out, "{}:", func.name)?;

    // FLS §6.12.1: Non-leaf functions must save the link register (x30)
    // before any `bl` instruction overwrites it. ARM64 pre-indexed store:
    //   `str x30, [sp, #-16]!` → sp -= 16 first, then store x30 at [sp].
    // This keeps sp 16-byte aligned (ARM64 ABI requirement).
    //
    // Cache-line note: the lr save is one 4-byte instruction; paired with
    // the matching `ldr` restore in the epilogue, both are in the first
    // and last cache line of the function respectively.
    if func.saves_lr {
        writeln!(
            out,
            "    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)"
        )?;
    }

    let fsize = frame_size(func.stack_slots);

    if fsize > 0 {
        // FLS §8.1: allocate stack space for local variables.
        // ARM64: SP must remain 16-byte aligned (ABI requirement).
        writeln!(
            out,
            "    sub     sp, sp, #{fsize:<14} // FLS §8.1: frame for {} slot(s)",
            func.stack_slots
        )?;
    }

    // FLS §6.23: label counter for MIN/-1 overflow guards (Claim 4q).
    // Each `sdiv`/`srem` site needs a unique local label; this counter
    // generates `.Lsdiv_ok_{fn_name}_{n}` / `.Lsrem_ok_{fn_name}_{n}`.
    let mut label_ctr: usize = 0;
    for instr in &func.body {
        emit_instr(out, instr, fsize, func.saves_lr, &func.name, &mut label_ctr)?;
    }

    Ok(())
}

/// Emit one instruction.
///
/// `frame_size` is passed so that `Ret` can restore `sp` before branching.
/// `saves_lr` is passed so that `Ret` can restore `x30` before `ret`.
/// `label_ctr` is incremented each time a local label is needed (MIN/-1 guards).
fn emit_instr(out: &mut String, instr: &Instr, frame_size: u32, saves_lr: bool, fn_name: &str, label_ctr: &mut usize) -> Result<(), CodegenError> {
    match instr {
        // FLS §6.19: Return expression.
        // ARM64 ABI: return value in x0; `ret` branches to link register x30.
        // Epilogue order (must mirror prologue in reverse):
        //   1. restore sp for local variable frame (if any)
        //   2. restore x30 from lr save slot (if non-leaf)
        //   3. ret
        Instr::Ret(value) => {
            emit_load_x0(out, value)?;
            if frame_size > 0 {
                writeln!(
                    out,
                    "    add     sp, sp, #{frame_size:<14} // FLS §8.1: restore stack frame"
                )?;
            }
            if saves_lr {
                // ARM64 post-indexed load: load x30 from [sp], then sp += 16.
                // This undoes the prologue `str x30, [sp, #-16]!`.
                // FLS §6.12.1: restore lr so `ret` branches to the caller.
                writeln!(
                    out,
                    "    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr"
                )?;
            }
            writeln!(out, "    ret")?;
        }

        // FLS §2.4.4.1: Load integer immediate into virtual register.
        // ARM64: values in [-65536, 65535] assemble to a single MOVZ or MOVN.
        // Larger 32-bit values use MOVZ+MOVK+sxtw (3 instructions, 12 bytes).
        //
        // FLS §5.2: Negative literal patterns materialise their value via
        // LoadImm with a negative immediate.
        //
        // sxtw sign-extends the 32-bit bit pattern to 64 bits so that 64-bit
        // comparisons (cmp x{reg}, ...) produce correct signed results for
        // large negative i32 values (e.g., -100000 → 0xFFFFFFFFFFFE7960).
        //
        // Cache-line note: small immediates = 4 bytes (1 slot); large = 12 bytes (3 slots).
        Instr::LoadImm(reg, n) => {
            emit_imm32(out, *reg as usize, *n)?;
        }

        // FLS §7.2: Load from a static variable in the data section.
        //
        // ARM64 addressing: ADRP loads the PC-relative page address; ADD
        // applies the page offset (:lo12:) to form the full address; LDR
        // loads the 64-bit value. Three 4-byte instructions = 12 bytes.
        //
        // FLS §7.2:15: All references to a static refer to the same memory
        // address — unlike const substitution (which inlines a value via MOV),
        // every static reference goes through the data section.
        //
        // Cache-line note: three 4-byte instructions (12 bytes) occupy three
        // slots in the instruction cache line. The loaded value occupies one
        // 8-byte slot in the data cache line — one half of a 16-byte row.
        Instr::LoadStatic { dst, name } => {
            writeln!(out, "    adrp    x{dst}, {name}              // FLS §7.2: static addr (page)")?;
            writeln!(out, "    add     x{dst}, x{dst}, :lo12:{name}  // FLS §7.2: static addr (offset)")?;
            writeln!(out, "    ldr     x{dst}, [x{dst}]             // FLS §7.2: static load")?;
        }

        // FLS §7.2, §4.2: Load an f64 static variable into a float register.
        //
        // Uses x17 (scratch) for the ADRP + ADD address computation, then
        // `ldr d{dst}` to load the 8-byte float value from the data section.
        //
        // Cache-line note: three 4-byte instructions (12 bytes) — same as the
        // integer LoadStatic. The data value is 8 bytes in `.data`.
        Instr::LoadStaticF64 { dst, name } => {
            writeln!(out, "    adrp    x17, {name}              // FLS §7.2: f64 static addr (page)")?;
            writeln!(out, "    add     x17, x17, :lo12:{name}  // FLS §7.2: f64 static addr (offset)")?;
            writeln!(out, "    ldr     d{dst}, [x17]             // FLS §7.2, §4.2: load f64 static")?;
        }

        // FLS §7.2, §4.2: Load an f32 static variable into a float register.
        //
        // Uses x17 (scratch) for the ADRP + ADD address computation, then
        // `ldr s{dst}` to load the 4-byte float value from the data section.
        //
        // Cache-line note: three 4-byte instructions (12 bytes). The data value
        // is 4 bytes in `.data` — half the footprint of an f64 static.
        Instr::LoadStaticF32 { dst, name } => {
            writeln!(out, "    adrp    x17, {name}              // FLS §7.2: f32 static addr (page)")?;
            writeln!(out, "    add     x17, x17, :lo12:{name}  // FLS §7.2: f32 static addr (offset)")?;
            writeln!(out, "    ldr     s{dst}, [x17]             // FLS §7.2, §4.2: load f32 static")?;
        }

        // FLS §4.9: Load the address of a named function into a register.
        //
        // ARM64 PC-relative addressing: ADRP loads the page-aligned base address;
        // ADD applies :lo12: to form the complete 64-bit function address.
        //
        // This is how `fn double` becomes a `fn(i32) -> i32` value that can be
        // passed as an argument or stored in a variable.
        //
        // Cache-line note: two 4-byte instructions (8 bytes) — one fewer than
        // LoadStatic because we don't dereference the address.
        Instr::LoadFnAddr { dst, name } => {
            writeln!(out, "    adrp    x{dst}, {name}              // FLS §4.9: fn ptr addr (page)")?;
            writeln!(out, "    add     x{dst}, x{dst}, :lo12:{name}  // FLS §4.9: fn ptr addr (offset)")?;
        }

        // FLS §4.9: Call through a function pointer stored on the stack.
        //
        // ARM64: load the function address into x9 (caller-saved temp that
        // is not an argument register), set up argument registers x0..xN-1,
        // then `blr x9` to branch to the address and link.
        //
        // Loading the fn ptr AFTER setting up args avoids a register conflict:
        // the args occupy x0..x{N-1} and x9 is never an argument register.
        //
        // Cache-line note: N arg moves + ldr + blr + 1 result move.
        // The extra `ldr` is the unavoidable cost of indirection vs direct `bl`.
        Instr::CallIndirect { dst, ptr_slot, args } => {
            // Move arguments into x0..xN-1.
            for (i, &arg) in args.iter().enumerate() {
                if arg as usize != i {
                    writeln!(out, "    mov     x{i}, x{arg:<24} // FLS §4.9: arg {i}")?;
                }
            }
            // Load the function pointer from its stack slot into x9 (scratch).
            let offset = (*ptr_slot as usize) * 8;
            writeln!(out, "    ldr     x9, [sp, #{offset:<22}] // FLS §4.9: load fn ptr")?;
            writeln!(out, "    blr     x9                       // FLS §4.9: indirect call")?;
            if *dst != 0 {
                writeln!(out, "    mov     x{dst}, x0               // FLS §4.9: capture return")?;
            }
        }

        // FLS §6.5.5: Integer binary arithmetic.
        // ARM64: `add`/`sub`/`mul` operate on 64-bit registers.
        // Virtual register N maps to ARM64 register xN (trivial allocation).
        // Cache-line note: one ARM64 instruction = 4 bytes per arithmetic BinOp.
        //
        // FLS §6.5.3: Comparison operator expressions.
        // ARM64 comparison: `cmp x{lhs}, x{rhs}` sets condition flags;
        // `cset x{dst}, <cond>` materialises 1 or 0 based on flags.
        // Cache-line note: two 4-byte instructions (8 bytes) per comparison.
        // Signed comparison (signed integers are the only type at this milestone).
        Instr::BinOp { op, dst, lhs, rhs } => {
            match op {
                // FLS §6.5.5: Integer addition, subtraction, multiplication.
                // FLS §6.23 Claim 4s: i32 overflow guard for +, -, *.
                //
                // ARM64 arithmetic operates on 64-bit registers (xN), so the
                // primary op never wraps at the hardware level. To detect i32
                // overflow we check whether the 64-bit result equals its own
                // sign-extended 32-bit low word:
                //
                //   add/sub/mul x{dst}, x{lhs}, x{rhs}  → 64-bit result
                //   sxtw x9, w{dst}                      → sign-extend low 32 bits
                //   cmp  x{dst}, x9                      → differ iff 64-bit result
                //   b.ne _galvanic_panic                  →   doesn't fit in i32
                //
                // Guard fires when the 64-bit result != sign_extend(low32(result)),
                // which is exactly when i32 overflow would occur in Rust debug mode.
                //
                // FLS §6.23 AMBIGUOUS — the spec requires debug-mode panic and
                // release-mode two's-complement wrap; galvanic always panics.
                // For i64 operands this guard would fire on valid i64 results that
                // exceed i32 range (false positive); galvanic only applies it when
                // the IR type is i32.
                // For u32 operands the sxtw sign-extension is semantic mismatch —
                // u32 overflow should wrap, not panic; galvanic defers u32 wrapping
                // to a future claim.
                // Cache-line: 4 × 4-byte instructions = 16 bytes per arithmetic op.
                IrBinOp::Add => {
                    writeln!(out, "    add     x{dst}, x{lhs}, x{rhs}          // FLS §6.5.5: add")?;
                    writeln!(out, "    sxtw    x9, w{dst}                       // FLS §6.23 Claim 4s: sign-extend low 32")?;
                    writeln!(out, "    cmp     x{dst}, x9                       // FLS §6.23: 64-bit == sext(32-bit)?")?;
                    writeln!(out, "    b.ne    _galvanic_panic                   // FLS §6.23: overflow → panic")?;
                }
                IrBinOp::Sub => {
                    writeln!(out, "    sub     x{dst}, x{lhs}, x{rhs}          // FLS §6.5.5: sub")?;
                    writeln!(out, "    sxtw    x9, w{dst}                       // FLS §6.23 Claim 4s: sign-extend low 32")?;
                    writeln!(out, "    cmp     x{dst}, x9                       // FLS §6.23: 64-bit == sext(32-bit)?")?;
                    writeln!(out, "    b.ne    _galvanic_panic                   // FLS §6.23: overflow → panic")?;
                }
                IrBinOp::Mul => {
                    writeln!(out, "    mul     x{dst}, x{lhs}, x{rhs}          // FLS §6.5.5: mul")?;
                    writeln!(out, "    sxtw    x9, w{dst}                       // FLS §6.23 Claim 4s: sign-extend low 32")?;
                    writeln!(out, "    cmp     x{dst}, x9                       // FLS §6.23: 64-bit == sext(32-bit)?")?;
                    writeln!(out, "    b.ne    _galvanic_panic                   // FLS §6.23: overflow → panic")?;
                }
                // FLS §6.5.5: Signed integer division.
                // ARM64: `sdiv x{dst}, x{lhs}, x{rhs}` — signed division.
                // FLS §6.23: Division by zero panics at runtime.
                // Guard: `cbz x{rhs}, _galvanic_panic` checks the divisor before sdiv.
                // ARM64 `cbz xN, label` branches if xN == 0 (4-byte instruction).
                //
                // FLS §6.23: Claim 4q — MIN/-1 signed overflow guard.
                // ARM64 `sdiv` with lhs=i32::MIN and rhs=-1 silently wraps to a
                // positive 64-bit value rather than panicking. Galvanic adds an
                // explicit compile-time guard: if lhs == i32::MIN && rhs == -1,
                // branch to _galvanic_panic before the division executes.
                //
                // Guard sequence (8 instructions = 32 bytes):
                //   movz x9, #0x8000, lsl #16  → x9 = 0x0000_0000_8000_0000
                //   sxtw x9, w9                → x9 = 0xFFFF_FFFF_8000_0000 (i32::MIN)
                //   cmp  x{lhs}, x9            → sets flags: lhs vs. i32::MIN (64-bit)
                //   b.ne .Lsdiv_ok_{fn}_{n}    → skip if lhs ≠ i32::MIN
                //   cmn  x{rhs}, #1            → sets flags: rhs + 1
                //   b.ne .Lsdiv_ok_{fn}_{n}    → skip if rhs ≠ -1
                //   b    _galvanic_panic        → MIN/-1 overflow → panic
                //   .Lsdiv_ok_{fn}_{n}:        → safe to divide
                //
                // Cache-line note: cbz + 8 guard instructions + sdiv = 10 instructions
                // (40 bytes), crossing at most two 64-byte cache lines.
                IrBinOp::Div => {
                    let n = *label_ctr;
                    *label_ctr += 1;
                    writeln!(
                        out,
                        "    cbz     x{rhs}, _galvanic_panic         // FLS §6.23: div-by-zero guard"
                    )?;
                    // FLS §6.23 Claim 4q: MIN/-1 overflow guard.
                    writeln!(out, "    movz    x9, #0x8000, lsl #16          // FLS §6.23: x9 = 0x0000_0000_8000_0000")?;
                    writeln!(out, "    sxtw    x9, w9                        // FLS §6.23: x9 = 0xFFFF_FFFF_8000_0000 (i32::MIN)")?;
                    writeln!(out, "    cmp     x{lhs}, x9                    // FLS §6.23: is lhs == i32::MIN?")?;
                    writeln!(out, "    b.ne    .Lsdiv_ok_{fn_name}_{n}       // FLS §6.23: lhs ≠ MIN → safe")?;
                    writeln!(out, "    cmn     x{rhs}, #1                    // FLS §6.23: is rhs == -1? (rhs+1==0)")?;
                    writeln!(out, "    b.ne    .Lsdiv_ok_{fn_name}_{n}       // FLS §6.23: rhs ≠ -1 → safe")?;
                    writeln!(out, "    b       _galvanic_panic                // FLS §6.23: MIN/-1 overflow → panic")?;
                    writeln!(out, ".Lsdiv_ok_{fn_name}_{n}:")?;
                    writeln!(
                        out,
                        "    sdiv    x{dst}, x{lhs}, x{rhs}          // FLS §6.5.5: div (signed)"
                    )?;
                }
                // FLS §6.5.5: Signed integer remainder.
                // Computed as `lhs - (lhs / rhs) * rhs` using two ARM64 instructions:
                //   sdiv x{dst}, x{lhs}, x{rhs}        → x{dst} = lhs / rhs (quotient)
                //   msub x{dst}, x{dst}, x{rhs}, x{lhs} → x{dst} = lhs - dst * rhs
                // ARM64 `msub xd, xn, xm, xa` reads all sources before writing xd,
                // so reusing dst for the intermediate quotient is safe.
                // FLS §6.23: Remainder by zero panics at runtime.
                // Guard: `cbz x{rhs}, _galvanic_panic` checks the divisor before sdiv.
                //
                // FLS §6.23: Claim 4q — MIN/-1 overflow guard for remainder.
                // In Rust debug mode, `i32::MIN % -1` also panics ("attempt to compute
                // the remainder with overflow"). In 64-bit arithmetic the result is 0
                // (no hardware overflow), but spec compliance requires the panic.
                // Guard sequence mirrors the Div guard with `.Lsrem_ok_{fn}_{n}` labels.
                //
                // Cache-line note: cbz + 8 guard instructions + sdiv + msub = 11 instructions
                // (44 bytes), crossing at most two 64-byte cache lines.
                IrBinOp::Rem => {
                    let n = *label_ctr;
                    *label_ctr += 1;
                    writeln!(
                        out,
                        "    cbz     x{rhs}, _galvanic_panic         // FLS §6.23: rem-by-zero guard"
                    )?;
                    // FLS §6.23 Claim 4q: MIN/-1 overflow guard.
                    writeln!(out, "    movz    x9, #0x8000, lsl #16          // FLS §6.23: x9 = 0x0000_0000_8000_0000")?;
                    writeln!(out, "    sxtw    x9, w9                        // FLS §6.23: x9 = 0xFFFF_FFFF_8000_0000 (i32::MIN)")?;
                    writeln!(out, "    cmp     x{lhs}, x9                    // FLS §6.23: is lhs == i32::MIN?")?;
                    writeln!(out, "    b.ne    .Lsrem_ok_{fn_name}_{n}       // FLS §6.23: lhs ≠ MIN → safe")?;
                    writeln!(out, "    cmn     x{rhs}, #1                    // FLS §6.23: is rhs == -1? (rhs+1==0)")?;
                    writeln!(out, "    b.ne    .Lsrem_ok_{fn_name}_{n}       // FLS §6.23: rhs ≠ -1 → safe")?;
                    writeln!(out, "    b       _galvanic_panic                // FLS §6.23: MIN/-1 remainder → panic")?;
                    writeln!(out, ".Lsrem_ok_{fn_name}_{n}:")?;
                    writeln!(
                        out,
                        "    sdiv    x{dst}, x{lhs}, x{rhs}          // FLS §6.5.5: rem step 1: quotient"
                    )?;
                    writeln!(
                        out,
                        "    msub    x{dst}, x{dst}, x{rhs}, x{lhs}  // FLS §6.5.5: rem step 2: lhs - q*rhs"
                    )?;
                }
                // Comparison ops: signed integer comparison on ARM64.
                // `cmp xA, xB` is an alias for `subs xzr, xA, xB` — sets N, Z, C, V flags.
                // `cset xD, cond` sets xD to 1 if the condition holds, 0 otherwise.
                // The condition codes match signed integer semantics (lt, le, gt, ge, eq, ne).
                //
                // FLS §6.5.3: "The type of a comparison expression is bool."
                // ARM64 ABI: bool is represented as 0 or 1 in a 64-bit register.
                //
                // Cache-line note: the two-instruction pair (cmp + cset) is 8 bytes —
                // two adjacent slots in the same cache line. The cmp result (flags)
                // is consumed immediately by cset, so no register is written by cmp.
                IrBinOp::Lt => {
                    writeln!(out, "    cmp     x{lhs}, x{rhs}               // FLS §6.5.3: compare (signed)")?;
                    writeln!(out, "    cset    x{dst}, lt                    // FLS §6.5.3: x{dst} = (x{lhs} < x{rhs})")?;
                }
                IrBinOp::Le => {
                    writeln!(out, "    cmp     x{lhs}, x{rhs}               // FLS §6.5.3: compare (signed)")?;
                    writeln!(out, "    cset    x{dst}, le                    // FLS §6.5.3: x{dst} = (x{lhs} <= x{rhs})")?;
                }
                IrBinOp::Gt => {
                    writeln!(out, "    cmp     x{lhs}, x{rhs}               // FLS §6.5.3: compare (signed)")?;
                    writeln!(out, "    cset    x{dst}, gt                    // FLS §6.5.3: x{dst} = (x{lhs} > x{rhs})")?;
                }
                IrBinOp::Ge => {
                    writeln!(out, "    cmp     x{lhs}, x{rhs}               // FLS §6.5.3: compare (signed)")?;
                    writeln!(out, "    cset    x{dst}, ge                    // FLS §6.5.3: x{dst} = (x{lhs} >= x{rhs})")?;
                }
                IrBinOp::Eq => {
                    writeln!(out, "    cmp     x{lhs}, x{rhs}               // FLS §6.5.3: compare (signed)")?;
                    writeln!(out, "    cset    x{dst}, eq                    // FLS §6.5.3: x{dst} = (x{lhs} == x{rhs})")?;
                }
                IrBinOp::Ne => {
                    writeln!(out, "    cmp     x{lhs}, x{rhs}               // FLS §6.5.3: compare (signed)")?;
                    writeln!(out, "    cset    x{dst}, ne                    // FLS §6.5.3: x{dst} = (x{lhs} != x{rhs})")?;
                }

                // FLS §6.5.6: Bit operator expressions.
                // ARM64: `and`/`orr`/`eor` operate on 64-bit registers.
                // Cache-line note: one 4-byte instruction per bitwise op.
                IrBinOp::BitAnd => writeln!(
                    out,
                    "    and     x{dst}, x{lhs}, x{rhs}          // FLS §6.5.6: bitwise and"
                )?,
                IrBinOp::BitOr => writeln!(
                    out,
                    "    orr     x{dst}, x{lhs}, x{rhs}          // FLS §6.5.6: bitwise or"
                )?,
                IrBinOp::BitXor => writeln!(
                    out,
                    "    eor     x{dst}, x{lhs}, x{rhs}          // FLS §6.5.6: bitwise xor"
                )?,

                // FLS §6.5.7: Shift operator expressions.
                // FLS §6.5.9: A shift with a negative or overly large shift amount panics
                // at runtime. Valid range: [0, bit_width). Galvanic guards all shifts with:
                //   cmp  x{rhs}, #64   — compare shift amount with 64 (unsigned)
                //   b.hs _galvanic_panic — branch if rhs >= 64 (unsigned; also catches negative)
                // Negative shift amounts (i64 sign bit set) are ≥ 2^63 when cast to u64,
                // which is far above 64, so `b.hs` catches them without a separate check.
                //
                // FLS §6.5.9 AMBIGUOUS: The spec requires panic when shift amount ≥ bit width
                // of the type. For i32 the valid range is [0, 31], but galvanic uses 64-bit
                // registers for all integers and checks against 64. Shifts of 32..63 on i32
                // values are a false negative — galvanic does not panic, but rustc debug mode
                // would. Documented as a known spec gap; the gap is analogous to the i64
                // MIN/-1 false positive in Claim 4q.
                //
                // ARM64: `lsl` (logical shift left), `asr` (arithmetic shift right, signed),
                // `lsr` (logical shift right, unsigned).
                // Cache-line note: 3 instructions (cmp + b.hs + shift) = 12 bytes per shift.
                IrBinOp::Shl => {
                    writeln!(
                        out,
                        "    cmp     x{rhs}, #64                    // FLS §6.5.9: shift amount must be < 64"
                    )?;
                    writeln!(
                        out,
                        "    b.hs    _galvanic_panic                // FLS §6.5.9: panic if shift >= 64 or negative"
                    )?;
                    writeln!(
                        out,
                        "    lsl     x{dst}, x{lhs}, x{rhs}          // FLS §6.5.7: shift left"
                    )?;
                }
                IrBinOp::Shr => {
                    writeln!(
                        out,
                        "    cmp     x{rhs}, #64                    // FLS §6.5.9: shift amount must be < 64"
                    )?;
                    writeln!(
                        out,
                        "    b.hs    _galvanic_panic                // FLS §6.5.9: panic if shift >= 64 or negative"
                    )?;
                    writeln!(
                        out,
                        "    asr     x{dst}, x{lhs}, x{rhs}          // FLS §6.5.7: arithmetic shift right (signed)"
                    )?;
                }

                // FLS §4.1: Unsigned integer division.
                // ARM64: `udiv x{dst}, x{lhs}, x{rhs}` — unsigned division.
                // Used when the operand type is unsigned (IrTy::U32).
                // FLS §6.23: Division by zero panics at runtime (same guard as signed).
                // Guard: `cbz x{rhs}, _galvanic_panic` checks the divisor before udiv.
                // Cache-line note: two instructions (cbz + udiv) = 8 bytes.
                IrBinOp::UDiv => {
                    writeln!(
                        out,
                        "    cbz     x{rhs}, _galvanic_panic         // FLS §6.23: udiv-by-zero guard"
                    )?;
                    writeln!(
                        out,
                        "    udiv    x{dst}, x{lhs}, x{rhs}          // FLS §4.1: div (unsigned)"
                    )?;
                }

                // FLS §4.1: Unsigned (logical) right shift.
                // FLS §6.5.9: Same shift-amount guard as signed shifts.
                // ARM64: `lsr x{dst}, x{lhs}, x{rhs}` — logical shift right,
                // zero-extends from the right (vs `asr` which sign-extends).
                // Used when the operand type is unsigned (IrTy::U32).
                // Cache-line note: 3 instructions (cmp + b.hs + lsr) = 12 bytes.
                IrBinOp::UShr => {
                    writeln!(
                        out,
                        "    cmp     x{rhs}, #64                    // FLS §6.5.9: shift amount must be < 64"
                    )?;
                    writeln!(
                        out,
                        "    b.hs    _galvanic_panic                // FLS §6.5.9: panic if shift >= 64 or negative"
                    )?;
                    writeln!(
                        out,
                        "    lsr     x{dst}, x{lhs}, x{rhs}          // FLS §4.1: logical shift right (unsigned)"
                    )?;
                }
            }
        }

        // FLS §6.5.4: Arithmetic negation — two's complement negation.
        // ARM64: `neg x{dst}, x{src}` is an alias for `sub x{dst}, xzr, x{src}`.
        // The GNU assembler accepts `neg` directly; it encodes as a 4-byte instruction.
        //
        // FLS §6.1.2:37–45: Runtime instruction — no constant folding.
        // Cache-line note: one 4-byte instruction, same footprint as any other UnOp.
        Instr::Neg { dst, src } => {
            writeln!(
                out,
                "    neg     x{dst}, x{src}               // FLS §6.5.4: negate x{src}"
            )?;
        }

        // `fneg d{dst}, d{src}` — IEEE 754 double-precision sign flip.
        //
        // FLS §6.5.4: The unary `-` on an `f64` value negates it. ARM64 FNEG
        // flips the sign bit without touching the mantissa/exponent.
        //
        // FLS §6.1.2:37–45: Runtime instruction — no constant folding.
        // Cache-line note: one 4-byte instruction.
        Instr::FNegF64 { dst, src } => {
            writeln!(
                out,
                "    fneg    d{dst}, d{src}               // FLS §6.5.4: f64 negate d{src}"
            )?;
        }

        // `fneg s{dst}, s{src}` — IEEE 754 single-precision sign flip.
        //
        // FLS §6.5.4: The unary `-` on an `f32` value negates it.
        // Cache-line note: one 4-byte instruction.
        Instr::FNegF32 { dst, src } => {
            writeln!(
                out,
                "    fneg    s{dst}, s{src}               // FLS §6.5.4: f32 negate s{src}"
            )?;
        }

        // FLS §6.5.4: Bitwise NOT `!operand` — complement all bits.
        // ARM64: `mvn x{dst}, x{src}` (alias for `orn xD, xzr, xS`).
        // The GNU assembler accepts `mvn` directly; it encodes as a 4-byte instruction.
        //
        // FLS §6.1.2:37–45: Runtime instruction — no constant folding.
        // Cache-line note: one 4-byte instruction, same footprint as `neg`.
        Instr::Not { dst, src } => {
            writeln!(
                out,
                "    mvn     x{dst}, x{src}               // FLS §6.5.4: bitwise NOT x{src}"
            )?;
        }

        // FLS §6.5.4: Logical NOT `!operand` for boolean values — 0 → 1, 1 → 0.
        // ARM64: `eor x{dst}, x{src}, #1` — XOR source with immediate 1.
        // Since booleans are represented as 0 or 1, XOR with 1 flips bit 0,
        // producing the correct logical complement in a single instruction.
        //
        // Contrast with bitwise NOT (Instr::Not → `mvn`): `mvn` of 0 = -1 (not 1),
        // and `mvn` of 1 = -2 (not 0). `mvn` is wrong for booleans.
        //
        // FLS §6.1.2:37–45: Runtime instruction — no constant folding.
        // Cache-line note: ARM64 `eor` with logical immediate is 4 bytes.
        Instr::BoolNot { dst, src } => {
            writeln!(
                out,
                "    eor     x{dst}, x{src}, #1             // FLS §6.5.4: logical NOT x{src} (bool)"
            )?;
        }

        // FLS §4.1, §6.23: Truncate to 8 unsigned bits for u8 return values.
        // ARM64: `and w{dst}, w{src}, #255` masks the low 8 bits.
        // The `w` register prefix operates on the 32-bit view of the register,
        // which is zero-extended into the full 64-bit x-register — identical
        // to masking the full 64-bit value with 0xFF.
        // Cache-line note: one 4-byte instruction.
        Instr::TruncU8 { dst, src } => {
            writeln!(
                out,
                "    and     w{dst}, w{src}, #255            // FLS §6.23: truncate u8 (wrap at 256)"
            )?;
        }

        // FLS §4.1, §6.23: Sign-extend from 8 signed bits for i8 return values.
        // ARM64: `sxtb x{dst}, w{src}` sign-extends the low 8 bits to 64 bits.
        // If bit 7 of src is 1, all upper bits are filled with 1s; otherwise 0s.
        // This gives two's complement wrapping: 150 (0x96) → -106 (0xFFFFFFFFFFFFFF96).
        // IMPORTANT: must use x{dst} (64-bit dest) so that subsequent 64-bit cmp
        // instructions see the correct sign-extended value. Using w{dst} (32-bit dest)
        // zeros the upper 32 bits of the x register, causing cmp x2, x4 to treat
        // 0x00000000FFFFFF96 ≠ 0xFFFFFFFFFFFFFF96 when x4 holds a 64-bit -106.
        // Cache-line note: one 4-byte instruction.
        Instr::SextI8 { dst, src } => {
            writeln!(
                out,
                "    sxtb    x{dst}, w{src}                  // FLS §6.23: sign-extend i8 to 64-bit (wrap at 128/-128)"
            )?;
        }

        // FLS §6.5.9: Narrowing cast to u16 — mask to low 16 bits.
        // ARM64: `and w{dst}, w{src}, #65535` — low 16 bits, zero-extended.
        // The `w` register prefix zero-extends into the full 64-bit x-register.
        // Cache-line note: one 4-byte instruction.
        Instr::TruncU16 { dst, src } => {
            writeln!(
                out,
                "    and     w{dst}, w{src}, #65535           // FLS §6.5.9: truncate u16 (mask to 16 bits)"
            )?;
        }

        // FLS §6.5.9: Narrowing cast to i16 — sign-extend from 16 bits to 64 bits.
        // ARM64: `sxth x{dst}, w{src}` — sign-extend halfword to 64 bits.
        // If bit 15 of src is 1, all upper bits are filled with 1s; otherwise 0s.
        // Cache-line note: one 4-byte instruction.
        Instr::SextI16 { dst, src } => {
            writeln!(
                out,
                "    sxth    x{dst}, w{src}                  // FLS §6.5.9: sign-extend i16 to 64-bit"
            )?;
        }

        // FLS §8.1: Store a virtual register to a stack slot.
        // ARM64: `str x{src}, [sp, #{offset}]` — offset = slot * 8.
        // Cache-line note: 8-byte slots keep stores naturally aligned;
        // two slots fill one 16-byte aligned pair.
        Instr::Store { src, slot } => {
            let offset = *slot as u32 * 8;
            writeln!(
                out,
                "    str     x{src}, [sp, #{offset:<15}] // FLS §8.1: store slot {slot}"
            )?;
        }

        // FLS §8.1 / FLS §6.3: Load a stack slot into a virtual register.
        // ARM64: `ldr x{dst}, [sp, #{offset}]` — offset = slot * 8.
        // Cache-line note: naturally aligned 8-byte loads hit L1 in one cycle.
        Instr::Load { dst, slot } => {
            let offset = *slot as u32 * 8;
            writeln!(
                out,
                "    ldr     x{dst}, [sp, #{offset:<15}] // FLS §8.1: load slot {slot}"
            )?;
        }

        // FLS §6.17: Branch target label.
        // Emits `.L{n}:` — a GAS local label. No machine code is emitted;
        // the label resolves to the address of the next instruction.
        // Cache-line note: labels have zero instruction footprint.
        Instr::Label(n) => {
            writeln!(out, ".L{n}:                              // FLS §6.17: branch target")?;
        }

        // FLS §6.17: Unconditional branch.
        // ARM64: `b .L{n}` — a 4-byte PC-relative branch instruction.
        // Cache-line note: ARM64 `b` is 4 bytes — one instruction slot.
        Instr::Branch(n) => {
            writeln!(out, "    b       .L{n:<24} // FLS §6.17: branch to end")?;
        }

        // FLS §6.17: Conditional branch on zero (false condition).
        // ARM64: `cbz x{reg}, .L{label}` — branches if reg == 0 (condition is false).
        // `cbz` ("compare and branch if zero") is a single 4-byte instruction that
        // combines the compare and branch, avoiding a separate `cmp` instruction.
        // Cache-line note: ARM64 `cbz` is 4 bytes — same footprint as `b`.
        Instr::CondBranch { reg, label } => {
            writeln!(
                out,
                "    cbz     x{reg}, .L{label:<21} // FLS §6.17: branch if false"
            )?;
        }

        // FLS §6.12.1: Call expression.
        // ARM64 ABI: integer arguments 0–7 go in x0–x7; return value in x0.
        //
        // For each argument i, if `args[i] != i` we emit `mov x{i}, x{args[i]}`
        // to place the value in the correct register. If `args[i] == i` the
        // value is already in the right place (no move needed).
        //
        // After `bl {name}`, the return value is in x0. We move it to the
        // destination register `dst` (unless dst == 0, already there).
        //
        // Cache-line note: at most `args.len()` move instructions before the
        // `bl` plus one move after — fits in a few cache lines for typical
        // short argument lists.
        //
        // Limitation: this does not handle the "parallel copy" problem where
        // args form a cycle (e.g., args = [1, 0] would incorrectly overwrite).
        // For the current milestone all arguments are freshly materialized
        // immediates or loads, so arg[i] == i always holds in practice.
        Instr::Call { dst, name, args, float_args, float_ret } => {
            // Move integer arguments to x0, x1, ... as required by the ARM64 ABI.
            for (i, &src_reg) in args.iter().enumerate() {
                if src_reg != i as u8 {
                    writeln!(
                        out,
                        "    mov     x{i}, x{src_reg:<19} // FLS §6.12.1: arg {i}"
                    )?;
                }
            }
            // FLS §4.2: Move float arguments to d0, d1, ... (ARM64 float register bank).
            for (i, &src_reg) in float_args.iter().enumerate() {
                if src_reg != i as u8 {
                    writeln!(
                        out,
                        "    fmov    d{i}, d{src_reg:<19} // FLS §4.2: float arg {i}"
                    )?;
                }
            }
            writeln!(out, "    bl      {name:<24} // FLS §6.12.1: call {name}")?;
            // FLS §4.2: Capture return value from the appropriate register.
            // Integer returns come back in x0; f64 in d0; f32 in s0.
            match float_ret {
                None => {
                    // Integer/unit return: capture from x0.
                    if *dst != 0 {
                        writeln!(
                            out,
                            "    mov     x{dst}, x0              // FLS §6.12.1: return value → x{dst}"
                        )?;
                    }
                }
                Some(true) => {
                    // f64 return: capture from d0.
                    if *dst != 0 {
                        writeln!(
                            out,
                            "    fmov    d{dst}, d0              // FLS §4.2: f64 return value → d{dst}"
                        )?;
                    }
                }
                Some(false) => {
                    // f32 return: capture from s0.
                    if *dst != 0 {
                        writeln!(
                            out,
                            "    fmov    s{dst}, s0              // FLS §4.2: f32 return value → s{dst}"
                        )?;
                    }
                }
            }
        }

        // FLS §10.1: Return from a `&mut self` method, writing modified fields back.
        //
        // Emits N `ldr` instructions (field 0..N-1 from their stack slots into
        // x0..x{N-1}), then the standard epilogue (sp restore + lr restore + ret).
        //
        // The field loads happen BEFORE sp is restored so that [sp, #slot*8]
        // addresses are still valid (they point into the local frame).
        //
        // FLS §10.1 AMBIGUOUS: The spec does not define the calling convention
        // for &mut self. Galvanic uses a value-copy write-back convention:
        // fields passed in, modified locally, returned in x0..x{N-1}.
        //
        // Cache-line note: N loads = N × 4-byte instructions. For N=2: 8 bytes.
        Instr::RetFields { base_slot, n_fields } => {
            // Load each field from its stack slot into the corresponding return register.
            for i in 0..*n_fields {
                let slot_offset = (*base_slot as usize + i as usize) * 8;
                writeln!(
                    out,
                    "    ldr     x{i}, [sp, #{slot_offset:<16}] // FLS §10.1: write-back field {i}"
                )?;
            }
            // Standard epilogue (mirrors Instr::Ret handling).
            if frame_size > 0 {
                writeln!(
                    out,
                    "    add     sp, sp, #{frame_size:<14} // FLS §8.1: restore stack frame"
                )?;
            }
            if saves_lr {
                writeln!(
                    out,
                    "    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr"
                )?;
            }
            writeln!(out, "    ret")?;
        }

        // FLS §6.9: Indexed array load — `dst = array[index]`.
        //
        // ARM64 two-instruction sequence:
        //   add x{dst}, sp, #(base_slot * 8)         // address of element 0
        //   ldr x{dst}, [x{dst}, x{index_reg}, lsl #3] // load element at index
        //
        // `lsl #3` scales the index by 8 (one slot = 8 bytes), equivalent to
        // multiplying by `sizeof(i32)` rounded up to 8-byte alignment.
        //
        // ARM64 LDR with shifted register: `ldr xD, [xB, xI, lsl #3]` is a
        // single 4-byte instruction that reads from address `xB + xI*8`.
        // The shift amount (3) is encoded directly in the instruction encoding.
        //
        // Precondition: `dst != index_reg`. If they were the same, the `add`
        // would overwrite `index_reg` before the `ldr` can use it. The lowering
        // pass guarantees `dst` is a freshly allocated register that does not
        // alias `index_reg` (both come from `alloc_reg()` in sequence).
        //
        // FLS §6.9 AMBIGUOUS: Out-of-bounds access must panic; no check is emitted.
        //
        // Cache-line note: add + ldr = two 4-byte instructions = 8 bytes,
        // fitting in one adjacent instruction slot pair in a 64-byte cache line.
        // FLS §6.9: Load from a stack-allocated array at a runtime-computed index.
        //
        // Bounds check (when len > 0):
        //   cmp x{index_reg}, #{len}    — set flags: index vs len (unsigned)
        //   b.hs _galvanic_panic        — branch if index >= len (unsigned ≥)
        //
        // The `b.hs` (branch if higher-or-same, i.e. unsigned ≥) also catches negative
        // indices because they are large unsigned values when reinterpreted as u64.
        //
        // FLS §6.23: Out-of-bounds access panics at runtime; galvanic routes to
        // `_galvanic_panic` (exit code 101) matching the divide-by-zero convention.
        Instr::LoadIndexed { dst, base_slot, index_reg, len } => {
            let base_offset = *base_slot as u32 * 8;
            if *len > 0 {
                writeln!(
                    out,
                    "    cmp     x{index_reg}, #{len:<14} // FLS §6.9: bounds check index < {len}"
                )?;
                writeln!(
                    out,
                    "    b.hs    _galvanic_panic            // FLS §6.9: panic if index >= {len}"
                )?;
            }
            writeln!(
                out,
                "    add     x{dst}, sp, #{base_offset:<15} // FLS §6.9: address of arr[0]"
            )?;
            writeln!(
                out,
                "    ldr     x{dst}, [x{dst}, x{index_reg}, lsl #3] // FLS §6.9: load arr[index]"
            )?;
        }

        // FLS §6.5.10 + §6.9: Store to an indexed array element `arr[index] = src`.
        //
        // Two-instruction sequence:
        //   add x{scratch}, sp, #{base_slot*8}  — base address of arr[0]
        //   str x{src}, [x{scratch}, x{index_reg}, lsl #3]  — store arr[index]
        //
        // The `lsl #3` scales the index by 8 (bytes per slot), matching the
        // element layout established by array literal stores.
        //
        // FLS §6.5.10 + §6.9: Store to an indexed array element `arr[index] = src`.
        //
        // Bounds check (when len > 0): cmp + b.hs before the store.
        // FLS §6.9: Out-of-bounds store must panic at runtime.
        //
        // Cache-line note: bounds check adds 2 instructions (8 bytes) when present.
        // add + str = two 4-byte instructions = 8 bytes baseline.
        Instr::StoreIndexed { src, base_slot, index_reg, scratch, len } => {
            let base_offset = *base_slot as u32 * 8;
            if *len > 0 {
                writeln!(
                    out,
                    "    cmp     x{index_reg}, #{len:<14} // FLS §6.9: bounds check index < {len}"
                )?;
                writeln!(
                    out,
                    "    b.hs    _galvanic_panic            // FLS §6.9: panic if index >= {len}"
                )?;
            }
            writeln!(
                out,
                "    add     x{scratch}, sp, #{base_offset:<15} // FLS §6.9: address of arr[0]"
            )?;
            writeln!(
                out,
                "    str     x{src}, [x{scratch}, x{index_reg}, lsl #3] // FLS §6.5.10: store arr[index]"
            )?;
        }

        // FLS §6.9 + §4.5 + §4.2: Load an f64 element from a `[f64; N]` array.
        //
        // Two-instruction sequence:
        //   add x9, sp, #{base_slot*8}            — base address of arr[0]
        //   ldr d{dst}, [x9, x{index_reg}, lsl #3] — load arr[index] into d-register
        //
        // The `lsl #3` scales the index by 8 (bytes per 64-bit slot), matching
        // the element layout of float arrays (each f64 occupies one 8-byte slot).
        //
        // FLS §4.2: f64 values are in d-registers (IEEE 754 double-precision).
        // FLS §6.1.2:37–45: Instructions emitted at runtime.
        //
        // Cache-line note: add + ldr = two 4-byte instructions = 8 bytes,
        // identical footprint to integer `LoadIndexed`.
        // FLS §6.9 + §4.5 + §4.2: Load an f64 element from a `[f64; N]` array.
        // Bounds check (when len > 0): cmp + b.hs before the load.
        Instr::LoadIndexedF64 { dst, base_slot, index_reg, len } => {
            let base_offset = *base_slot as u32 * 8;
            if *len > 0 {
                writeln!(
                    out,
                    "    cmp     x{index_reg}, #{len:<14} // FLS §6.9: bounds check index < {len}"
                )?;
                writeln!(
                    out,
                    "    b.hs    _galvanic_panic            // FLS §6.9: panic if index >= {len}"
                )?;
            }
            writeln!(
                out,
                "    add     x9, sp, #{base_offset:<15} // FLS §6.9: address of f64 arr[0]"
            )?;
            writeln!(
                out,
                "    ldr     d{dst}, [x9, x{index_reg}, lsl #3] // FLS §6.9: load f64 arr[index]"
            )?;
        }

        // FLS §6.9 + §4.5 + §4.2: Load an f32 element from a `[f32; N]` array.
        //
        // Three-instruction sequence:
        //   add x9, sp, #{base_slot*8}            — base address of arr[0]
        //   add x9, x9, x{index_reg}, lsl #3      — advance by index*8 (stride=8 bytes)
        //   ldr s{dst}, [x9]                       — load f32 at computed address
        //
        // f32 elements occupy one 8-byte slot each on the stack (same as all other types),
        // so the stride is 8 (lsl #3). ARM64 `ldr s` only allows lsl #0 or lsl #2 in the
        // scaled-register addressing mode (because s-registers are 4 bytes, 2^2=4). Using
        // `add` with lsl #3 is valid in the shifted-register form of `add`, so we
        // pre-compute the byte address with `add` and then use an unscaled `ldr`.
        //
        // FLS §4.2: f32 values are in s-registers (IEEE 754 single-precision).
        // Cache-line note: add + add + ldr = three 4-byte instructions = 12 bytes.
        // FLS §6.9 + §4.5 + §4.2: Load an f32 element from a `[f32; N]` array.
        // Bounds check (when len > 0): cmp + b.hs before the load.
        Instr::LoadIndexedF32 { dst, base_slot, index_reg, len } => {
            let base_offset = *base_slot as u32 * 8;
            if *len > 0 {
                writeln!(
                    out,
                    "    cmp     x{index_reg}, #{len:<14} // FLS §6.9: bounds check index < {len}"
                )?;
                writeln!(
                    out,
                    "    b.hs    _galvanic_panic            // FLS §6.9: panic if index >= {len}"
                )?;
            }
            writeln!(
                out,
                "    add     x9, sp, #{base_offset:<15} // FLS §6.9: address of f32 arr[0]"
            )?;
            writeln!(
                out,
                "    add     x9, x9, x{index_reg}, lsl #3 // FLS §6.9: advance by index*8 (stride)"
            )?;
            writeln!(
                out,
                "    ldr     s{dst}, [x9]               // FLS §6.9: load f32 arr[index]"
            )?;
        }

        // FLS §4.13: Call a function returning a `&dyn Trait` fat pointer.
        //
        // After `bl {name}`, x0 = data pointer and x1 = vtable pointer.
        // Store both to consecutive stack slots so the caller can use the
        // result as a `&dyn Trait` local (registered in `local_dyn_types`).
        //
        // FLS §4.13 AMBIGUOUS: The fat pointer return ABI is not specified.
        // Galvanic uses (x0=data_ptr, x1=vtable_ptr), symmetric with the
        // two-register parameter convention for `&dyn Trait` parameters.
        //
        // Cache-line note: N arg moves + bl + 2 stores = N+3 instructions.
        Instr::CallRetFatPtr { name, args, dst_data_slot } => {
            // Move arguments to x0, x1, ... (fat ptr args occupy two regs each).
            for (i, &src_reg) in args.iter().enumerate() {
                if src_reg != i as u8 {
                    writeln!(
                        out,
                        "    mov     x{i}, x{src_reg:<19} // FLS §4.13: arg {i}"
                    )?;
                }
            }
            writeln!(out, "    bl      {name:<24} // FLS §4.13: call &dyn Trait returning fn")?;
            // Store returned fat pointer: x0 = data_ptr, x1 = vtable_ptr.
            let data_offset = *dst_data_slot as u32 * 8;
            let vtable_offset = (*dst_data_slot as u32 + 1) * 8;
            writeln!(
                out,
                "    str     x0, [sp, #{data_offset:<15}] // FLS §4.13: store returned data ptr"
            )?;
            writeln!(
                out,
                "    str     x1, [sp, #{vtable_offset:<15}] // FLS §4.13: store returned vtable ptr"
            )?;
        }

        // FLS §6.12.2 + §10.1: Call a `&mut self` method and write modified fields back.
        //
        // After the `bl`, x0..x{N-1} hold the method's modified field values
        // (returned via `RetFields`). Store them immediately back to the caller's
        // struct slots so subsequent field reads see the updated values.
        //
        // Cache-line note: arg moves + bl + N stores. For a 2-field struct the
        // write-back is 2 × 4-byte `str` = 8 bytes, fitting in one cache line
        // alongside the `bl`.
        Instr::CallMut { name, args, write_back_slot, n_fields } => {
            // Move arguments to x0, x1, ... (struct fields first, then extra args).
            for (i, &src_reg) in args.iter().enumerate() {
                if src_reg != i as u8 {
                    writeln!(
                        out,
                        "    mov     x{i}, x{src_reg:<19} // FLS §6.12.2: arg {i}"
                    )?;
                }
            }
            writeln!(out, "    bl      {name:<24} // FLS §6.12.2: call &mut self {name}")?;
            // Write back modified fields from x0..x{N-1} to the struct's stack slots.
            for i in 0..*n_fields {
                let slot_offset = (*write_back_slot as usize + i as usize) * 8;
                writeln!(
                    out,
                    "    str     x{i}, [sp, #{slot_offset:<16}] // FLS §10.1: write-back field {i}"
                )?;
            }
        }

        // FLS §10.1: Return modified struct fields AND a scalar value from a
        // `&mut self` method.
        //
        // Convention: fields in x0..x{N-1} (write-back for caller), scalar in x{N}.
        //
        // Emits:
        //   ldr x{i}, [sp, #{(base_slot+i)*8}]  for i in 0..n_fields  — field loads
        //   mov x{n_fields}, x{val_reg}                                — return value
        //   standard epilogue + ret
        //
        // FLS §10.1 AMBIGUOUS: The spec does not define the calling convention for
        // &mut self with a non-unit return type. This extends RetFields by placing
        // the scalar return value in x{n_fields}.
        //
        // Cache-line note: (N+1) × 4-byte loads/moves before epilogue.
        Instr::RetFieldsAndValue { base_slot, n_fields, val_reg } => {
            // Load each field from its stack slot into the corresponding write-back register.
            for i in 0..*n_fields {
                let slot_offset = (*base_slot as usize + i as usize) * 8;
                writeln!(
                    out,
                    "    ldr     x{i}, [sp, #{slot_offset:<16}] // FLS §10.1: write-back field {i}"
                )?;
            }
            // Place scalar return value in x{n_fields}.
            let ret_reg = *n_fields;
            if *val_reg != ret_reg {
                writeln!(
                    out,
                    "    mov     x{ret_reg}, x{val_reg:<18} // FLS §10.1: scalar return value"
                )?;
            }
            // Standard epilogue (mirrors Instr::RetFields handling).
            if frame_size > 0 {
                writeln!(
                    out,
                    "    add     sp, sp, #{frame_size:<14} // FLS §8.1: restore stack frame"
                )?;
            }
            if saves_lr {
                writeln!(
                    out,
                    "    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr"
                )?;
            }
            writeln!(out, "    ret")?;
        }

        // FLS §10.1: Call a `&mut self` method returning a scalar, write back fields,
        // and capture the scalar return value.
        //
        // After `bl`, x0..x{N-1} hold modified field values and x{N} holds the
        // scalar return. Write x0..x{N-1} back to the receiver's stack slots, then
        // move x{N} into `dst`.
        //
        // FLS §6.12.2: Method call expressions — dispatched to mangled name.
        // FLS §10.1: Write-back convention extended for scalar returns.
        //
        // Cache-line note: arg moves + bl + N write-back stores + 1 capture move.
        Instr::CallMutReturn { name, args, write_back_slot, n_fields, dst } => {
            // Move arguments to x0, x1, ... (struct fields first, then extra args).
            for (i, &src_reg) in args.iter().enumerate() {
                if src_reg != i as u8 {
                    writeln!(
                        out,
                        "    mov     x{i}, x{src_reg:<19} // FLS §6.12.2: arg {i}"
                    )?;
                }
            }
            writeln!(out, "    bl      {name:<24} // FLS §6.12.2: call &mut self {name}")?;
            // Write back modified fields from x0..x{N-1} to the struct's stack slots.
            for i in 0..*n_fields {
                let slot_offset = (*write_back_slot as usize + i as usize) * 8;
                writeln!(
                    out,
                    "    str     x{i}, [sp, #{slot_offset:<16}] // FLS §10.1: write-back field {i}"
                )?;
            }
            // Capture scalar return value from x{n_fields} into dst.
            let ret_reg = *n_fields;
            if *dst != ret_reg {
                writeln!(
                    out,
                    "    mov     x{dst}, x{ret_reg:<18} // FLS §10.1: capture scalar return"
                )?;
            }
        }
        // FLS §6.5.1: Borrow expression `&place`.
        //
        // Computes the address of a stack slot and places it in `dst`.
        // ARM64: `add x{dst}, sp, #{slot * 8}` — adds the byte offset to sp.
        //
        // Cache-line note: one 4-byte instruction. The resulting pointer value
        // occupies one 8-byte register slot — same footprint as any i32 value.
        Instr::AddrOf { dst, slot } => {
            let offset = *slot as u32 * 8;
            writeln!(
                out,
                "    add     x{dst}, sp, #{offset:<19} // FLS §6.5.1: address of stack slot {slot}"
            )?;
        }

        // FLS §6.5.1, §6.9: Address of an array element.
        //
        // Computes `sp + base_slot*8 + index_reg*8` — the address of arr[index].
        // Two-instruction sequence: base address then scaled index add.
        //
        // Used by `for x in &mut arr` to bind each element's address to the
        // loop variable, enabling `*x = val` to mutate elements in-place.
        //
        // Cache-line note: two 4-byte instructions (8 bytes). Pairs with
        // `LoadIndexed` (also 2 instructions) — same footprint, different result.
        Instr::AddrOfIndexed { dst, base_slot, index_reg, scratch } => {
            let base_offset = *base_slot as u32 * 8;
            writeln!(
                out,
                "    add     x{scratch}, sp, #{base_offset:<15} // FLS §6.5.1: address of arr[0]"
            )?;
            writeln!(
                out,
                "    add     x{dst}, x{scratch}, x{index_reg}, lsl #3  // FLS §6.9: address of arr[index]"
            )?;
        }

        // FLS §6.5.2: Dereference expression `*expr`.
        //
        // Loads the value at the memory address held in register `src`.
        // ARM64: `ldr x{dst}, [x{src}]` — register-indirect load.
        //
        // Cache-line note: one 4-byte instruction. The load targets the cache
        // line containing the referent value (8-byte aligned on the stack).
        Instr::LoadPtr { dst, src } => {
            writeln!(
                out,
                "    ldr     x{dst}, [x{src}]           // FLS §6.5.2: deref pointer in x{src}"
            )?;
        }

        // FLS §6.5.10: Assignment through a mutable reference `*ref_var = value`.
        //
        // Stores the value in register `src` to the memory address in register `addr`.
        // ARM64: `str x{src}, [x{addr}]` — register-indirect store.
        //
        // Cache-line note: one 4-byte instruction. The store targets the same
        // cache line as the referent value (8-byte aligned). Symmetric with
        // `LoadPtr` — same instruction count, same cache footprint.
        Instr::StorePtr { src, addr } => {
            writeln!(
                out,
                "    str     x{src}, [x{addr}]           // FLS §6.5.10: store through pointer in x{addr}"
            )?;
        }

        // FLS §2.4.4.2: Load a 64-bit float constant from the per-function
        // .rodata pool into float register d{dst}.
        //
        // ARM64 sequence:
        //   ADRP x17, label   — load page-aligned PC-relative base into x17
        //   ADD  x17, x17, :lo12:label — apply low-12-bit page offset
        //   LDR  d{dst}, [x17] — load 8-byte float into d{dst}
        //
        // x17 (ip1) is the ARM64 intra-procedure-call scratch register,
        // reserved for linker veneers and callee-saved scratch. Using it avoids
        // consuming a general virtual register for the address computation.
        //
        // FLS §6.1.2:37–45: Even a float literal emits runtime loads.
        //
        // Cache-line note: 3 × 4-byte instructions = 12 bytes. The constant
        // (.quad in .rodata) is one 8-byte slot — same as a static item.
        Instr::LoadF64Const { dst, idx } => {
            let label = format!("{fn_name}__fc{idx}");
            writeln!(out, "    adrp    x17, {label}              // FLS §2.4.4.2: f64 const addr (page)")?;
            writeln!(out, "    add     x17, x17, :lo12:{label}  // FLS §2.4.4.2: f64 const addr (offset)")?;
            writeln!(out, "    ldr     d{dst}, [x17]             // FLS §2.4.4.2: load f64 into d{dst}")?;
        }

        // FLS §8.1: Store a float register to a stack slot.
        //
        // `str d{src}, [sp, #{slot*8}]` — stores 8 bytes from `d{src}` to the
        // stack frame at byte offset `slot * 8`.
        //
        // ARM64: float `str` uses the same addressing mode as integer `str`,
        // but targets the SIMD/FP register bank. 8-byte alignment is satisfied
        // because all stack slots are 8 bytes and the frame base is 16-byte aligned.
        //
        // Cache-line note: one 4-byte instruction — same footprint as integer Store.
        Instr::StoreF64 { src, slot } => {
            let off = (*slot as u32) * 8;
            writeln!(
                out,
                "    str     d{src}, [sp, #{off:<14}] // FLS §8.1: store f64 slot {slot}"
            )?;
        }

        // FLS §8.1: Load a float register from a stack slot.
        //
        // `ldr d{dst}, [sp, #{slot*8}]` — loads 8 bytes from the stack frame
        // into `d{dst}`.
        //
        // Cache-line note: one 4-byte instruction — same footprint as integer Load.
        Instr::LoadF64Slot { dst, slot } => {
            let off = (*slot as u32) * 8;
            writeln!(
                out,
                "    ldr     d{dst}, [sp, #{off:<14}] // FLS §8.1: load f64 slot {slot}"
            )?;
        }

        // FLS §6.5.9: Convert a 64-bit float register to a signed 32-bit
        // integer, truncating toward zero.
        //
        // `fcvtzs w{dst}, d{src}` — FCVTZS (Fixed-point Convert to Signed,
        // rounding toward Zero). Writes a 32-bit result into `w{dst}` (which
        // is the low 32 bits of `x{dst}`; the upper 32 bits are zero-extended).
        //
        // FLS §6.5.9: `f64 as i32` truncates (rounds toward zero).
        //   3.9  as i32 → 3
        //   -3.9 as i32 → -3
        //
        // Out-of-range: ARM64 FCVTZS saturates to INT32_MIN/INT32_MAX.
        // FLS §6.5.9 AMBIGUOUS: Rust requires wrapping behaviour in release
        // and a panic in debug when out-of-range; saturation differs from both.
        // Galvanic emits saturation at this milestone — documented limitation.
        //
        // Cache-line note: one 4-byte FCVTZS instruction.
        Instr::F64ToI32 { dst, src } => {
            writeln!(
                out,
                "    fcvtzs  w{dst}, d{src}              // FLS §6.5.9: f64→i32 truncate"
            )?;
        }

        // FLS §6.5.5: Float arithmetic — fadd/fsub/fmul/fdiv on d-registers.
        //
        // ARM64 FP/SIMD instruction set: all four operations have the form
        //   f<op>  d{dst}, d{lhs}, d{rhs}
        // and follow IEEE 754 double-precision semantics with round-to-nearest-even.
        //
        // FLS §6.5.5 AMBIGUOUS: The spec references IEEE 754 but does not
        // mandate a rounding mode. ARM64 hardware default is round-to-nearest-even.
        //
        // Cache-line note: one 4-byte instruction per f64 binary operation.
        Instr::F64BinOp { op, dst, lhs, rhs } => {
            let mnemonic = match op {
                crate::ir::F64BinOp::Add => "fadd",
                crate::ir::F64BinOp::Sub => "fsub",
                crate::ir::F64BinOp::Mul => "fmul",
                crate::ir::F64BinOp::Div => "fdiv",
            };
            writeln!(
                out,
                "    {mnemonic}    d{dst}, d{lhs}, d{rhs}           // FLS §6.5.5: f64 {mnemonic}"
            )?;
        }

        // FLS §2.4.4.2: Load a 32-bit float constant from .rodata into s{dst}.
        //
        // Same ADRP + ADD + LDR sequence as LoadF64Const but uses `ldr s{dst}`
        // (4-byte load) instead of `ldr d{dst}` (8-byte load).
        //
        // Cache-line note: 3 × 4-byte instructions = 12 bytes. The constant
        // (.word in .rodata) is one 4-byte slot — half the footprint of f64.
        Instr::LoadF32Const { dst, idx } => {
            let label = format!("{fn_name}__f32c{idx}");
            writeln!(out, "    adrp    x17, {label}              // FLS §2.4.4.2: f32 const addr (page)")?;
            writeln!(out, "    add     x17, x17, :lo12:{label}  // FLS §2.4.4.2: f32 const addr (offset)")?;
            writeln!(out, "    ldr     s{dst}, [x17]             // FLS §2.4.4.2: load f32 into s{dst}")?;
        }

        // FLS §8.1: Store a single-precision float register to a stack slot.
        //
        // `str s{src}, [sp, #{slot*8}]` — stores 4 bytes from `s{src}`.
        // The slot is 8 bytes wide; only the lower 4 bytes are written.
        //
        // Cache-line note: one 4-byte instruction.
        Instr::StoreF32 { src, slot } => {
            let off = (*slot as u32) * 8;
            writeln!(
                out,
                "    str     s{src}, [sp, #{off:<14}] // FLS §8.1: store f32 slot {slot}"
            )?;
        }

        // FLS §8.1: Load a single-precision float register from a stack slot.
        //
        // `ldr s{dst}, [sp, #{slot*8}]` — loads 4 bytes into `s{dst}`.
        //
        // Cache-line note: one 4-byte instruction.
        Instr::LoadF32Slot { dst, slot } => {
            let off = (*slot as u32) * 8;
            writeln!(
                out,
                "    ldr     s{dst}, [sp, #{off:<14}] // FLS §8.1: load f32 slot {slot}"
            )?;
        }

        // FLS §6.5.9: Convert a single-precision float register to a signed
        // 32-bit integer, truncating toward zero.
        //
        // `fcvtzs w{dst}, s{src}` — same semantics as F64ToI32 but uses `s`
        // (single-precision) instead of `d` (double-precision) source.
        //
        // FLS §6.5.9 AMBIGUOUS: ARM64 FCVTZS saturates out-of-range values;
        // Rust requires wrapping (release) or panic (debug). Same as F64ToI32.
        //
        // Cache-line note: one 4-byte instruction.
        Instr::F32ToI32 { dst, src } => {
            writeln!(
                out,
                "    fcvtzs  w{dst}, s{src}              // FLS §6.5.9: f32→i32 truncate"
            )?;
        }

        // `scvtf d{dst}, w{src}` — SCVTF (Signed integer Convert to Floating-point).
        //
        // FLS §6.5.9: `i32 as f64`. Converts a signed 32-bit integer to IEEE 754
        // double-precision. All i32 values are exactly representable in f64.
        //
        // Cache-line note: one 4-byte instruction.
        Instr::I32ToF64 { dst, src } => {
            writeln!(
                out,
                "    scvtf   d{dst}, w{src}              // FLS §6.5.9: i32→f64 convert"
            )?;
        }

        // `scvtf s{dst}, w{src}` — SCVTF single-precision variant.
        //
        // FLS §6.5.9: `i32 as f32`. Values that cannot be exactly represented
        // are rounded to nearest-even.
        //
        // Cache-line note: one 4-byte instruction.
        Instr::I32ToF32 { dst, src } => {
            writeln!(
                out,
                "    scvtf   s{dst}, w{src}              // FLS §6.5.9: i32→f32 convert"
            )?;
        }

        // `fcvt d{dst}, s{src}` — FCVT (Floating-point ConVerT), single-to-double.
        //
        // FLS §6.5.9: `f32 as f64`. The conversion is exact: every finite f32
        // value is representable as f64. NaN payloads are preserved.
        //
        // Cache-line note: one 4-byte instruction.
        Instr::F32ToF64 { dst, src } => {
            writeln!(
                out,
                "    fcvt    d{dst}, s{src}              // FLS §6.5.9: f32→f64 widen"
            )?;
        }

        // `fcvt s{dst}, d{src}` — FCVT (Floating-point ConVerT), double-to-single.
        //
        // FLS §6.5.9: `f64 as f32`. Values that cannot be exactly represented
        // are rounded to nearest-even (IEEE 754 default rounding mode).
        //
        // Cache-line note: one 4-byte instruction.
        Instr::F64ToF32 { dst, src } => {
            writeln!(
                out,
                "    fcvt    s{dst}, d{src}              // FLS §6.5.9: f64→f32 narrow"
            )?;
        }

        // FLS §6.5.5: Float arithmetic — fadd/fsub/fmul/fdiv on s-registers.
        //
        // Same mnemonics as F64BinOp but operands use `s{N}` (single-precision).
        //
        // Cache-line note: one 4-byte instruction per f32 binary operation.
        Instr::F32BinOp { op, dst, lhs, rhs } => {
            let mnemonic = match op {
                crate::ir::F32BinOp::Add => "fadd",
                crate::ir::F32BinOp::Sub => "fsub",
                crate::ir::F32BinOp::Mul => "fmul",
                crate::ir::F32BinOp::Div => "fdiv",
            };
            writeln!(
                out,
                "    {mnemonic}    s{dst}, s{lhs}, s{rhs}           // FLS §6.5.5: f32 {mnemonic}"
            )?;
        }

        // FLS §6.5.3: Float comparison — `fcmp d{lhs}, d{rhs}` sets
        // floating-point condition flags; `cset x{dst}, <cond>` materialises
        // the boolean result.
        //
        // ARM64 note: FCMP uses the same condition code names as CMP.
        // NaN inputs set Z=0, C=1, V=1, N=0 — `lt`/`le`/`gt`/`ge` produce 0,
        // `eq` produces 0, `ne` produces 1 (consistent with IEEE 754 §5.11).
        //
        // Cache-line note: 2 × 4-byte instructions = 8 bytes.
        Instr::FCmpF64 { op, dst, lhs, rhs } => {
            let cond = match op {
                crate::ir::FCmpOp::Lt => "lt",
                crate::ir::FCmpOp::Le => "le",
                crate::ir::FCmpOp::Gt => "gt",
                crate::ir::FCmpOp::Ge => "ge",
                crate::ir::FCmpOp::Eq => "eq",
                crate::ir::FCmpOp::Ne => "ne",
            };
            writeln!(out, "    fcmp    d{lhs}, d{rhs}               // FLS §6.5.3: f64 compare")?;
            writeln!(out, "    cset    x{dst}, {cond}                    // FLS §6.5.3: x{dst} = (d{lhs} {cond} d{rhs})")?;
        }

        // FLS §6.5.3: Single-precision float comparison.
        //
        // Same two-instruction pattern as FCmpF64 but uses `s`-registers.
        // Cache-line note: 2 × 4-byte instructions = 8 bytes.
        Instr::FCmpF32 { op, dst, lhs, rhs } => {
            let cond = match op {
                crate::ir::FCmpOp::Lt => "lt",
                crate::ir::FCmpOp::Le => "le",
                crate::ir::FCmpOp::Gt => "gt",
                crate::ir::FCmpOp::Ge => "ge",
                crate::ir::FCmpOp::Eq => "eq",
                crate::ir::FCmpOp::Ne => "ne",
            };
            writeln!(out, "    fcmp    s{lhs}, s{rhs}               // FLS §6.5.3: f32 compare")?;
            writeln!(out, "    cset    x{dst}, {cond}                    // FLS §6.5.3: x{dst} = (s{lhs} {cond} s{rhs})")?;
        }

        // FLS §4.13: Vtable dispatch for `dyn Trait` method calls.
        //
        // Fat pointer layout: data_slot holds the data pointer, vtable_slot holds
        // the vtable pointer (= data_slot + 1 in spill order).
        //
        // Sequence (FLS §4.13 AMBIGUOUS — layout is implementation-defined):
        //   ldr x9,  [sp, #{vtable_slot*8}]    // load vtable pointer
        //   ldr x10, [x9, #{method_idx*8}]      // load method fn-ptr from vtable
        //   ldr x0,  [sp, #{data_slot*8}]       // load data pointer into arg 0
        //   blr x10                              // indirect call via vtable entry
        //   mov x{dst}, x0                       // capture return value (if dst != 0)
        //
        // Scratch registers x9/x10 are intra-procedure temporaries per ARM64 ABI.
        // Cache-line note: 4–5 instructions = 16–20 bytes.
        Instr::CallVtable { dst, data_slot, vtable_slot, method_idx } => {
            let vtable_offset = *vtable_slot as usize * 8;
            let method_offset = method_idx * 8;
            let data_offset   = *data_slot as usize * 8;
            writeln!(out, "    ldr     x9,  [sp, #{vtable_offset:<14}] // FLS §4.13: load vtable ptr from slot {vtable_slot}")?;
            writeln!(out, "    ldr     x10, [x9,  #{method_offset:<14}] // FLS §4.13: load method[{method_idx}] fn-ptr from vtable")?;
            writeln!(out, "    ldr     x0,  [sp, #{data_offset:<14}] // FLS §4.13: load data ptr into x0")?;
            writeln!(out, "    blr     x10                          // FLS §4.13: indirect call via vtable")?;
            if *dst != 0 {
                writeln!(out, "    mov     x{dst}, x0              // FLS §4.13: capture return value → x{dst}")?;
            }
        }
    }
    Ok(())
}

/// Emit ARM64 instructions to load a 32-bit immediate into register `x{reg}`.
///
/// ARM64 encoding:
/// - Values in [-65536, 65535]: single `mov` (GAS selects MOVZ or MOVN).
/// - Other 32-bit values: MOVZ+MOVK to load the 32-bit bit pattern, then `sxtw`
///   to sign-extend from 32 bits to 64 bits.
///
/// The `sxtw` is required because galvanic uses 64-bit `x` registers for i32
/// values. `cmp x{reg}, x{other}` is a signed 64-bit comparison; if `x{reg}`
/// holds a zero-extended negative i32 (e.g., 0x00000000FFFE7960 for -100000),
/// the comparison is wrong. `sxtw` produces the correct 64-bit value
/// (e.g., 0xFFFFFFFFFFFE7960), matching values loaded via the `neg` instruction.
///
/// For positive large values (bit 31 of the 32-bit pattern = 0), `sxtw` is a
/// no-op — it leaves the value unchanged.
///
/// FLS §2.4.4.1: Integer literal materialization.
/// FLS §5.2: Negative literal patterns also materialise via this path.
/// Cache-line note: small immediates = 1 instruction (4 bytes);
///   large = 2–3 instructions (8–12 bytes, fits within one cache line).
fn emit_imm32(out: &mut String, reg: usize, n: i32) -> Result<(), CodegenError> {
    if (-65536..=65535).contains(&n) {
        // Single instruction: GAS selects MOVZ (positive) or MOVN (negative).
        // MOVN already sign-extends, so no sxtw needed.
        writeln!(out, "    mov     x{reg}, #{n:<19} // FLS §2.4.4.1: load imm {n}")?;
    } else {
        // MOVZ+MOVK loads the 32-bit two's-complement bit pattern into the low 32
        // bits of the register (upper 32 bits zero). Follow with sxtw to
        // sign-extend to 64 bits so that 64-bit comparisons are correct.
        let bits = n as u32;
        let lo16 = bits & 0xFFFF;
        let hi16 = (bits >> 16) & 0xFFFF;
        if lo16 == 0 {
            // Low 16 bits are zero: single MOVZ with shift suffices.
            writeln!(out, "    movz    x{reg}, #0x{hi16:04x}, lsl #16  // FLS §2.4.4.1: load imm {n} (hi16 only)")?;
        } else {
            writeln!(out, "    movz    x{reg}, #0x{lo16:04x}           // FLS §2.4.4.1: load imm {n} (lo16)")?;
            writeln!(out, "    movk    x{reg}, #0x{hi16:04x}, lsl #16  // FLS §2.4.4.1: load imm {n} (hi16)")?;
        }
        // Sign-extend the 32-bit pattern in w{reg} to 64 bits in x{reg}.
        // No-op for positive values; corrects sign for negative i32 values.
        writeln!(out, "    sxtw    x{reg}, w{reg}               // sign-extend i32 to 64-bit (FLS §2.4.4.1)")?;
    }
    Ok(())
}

/// Emit instructions that place `value` into `x0` for return.
///
/// FLS §2.4.4.1: Integer literals.
/// FLS §6.19: Return expressions — result in x0.
/// Cache-line note: each `mov` is 4 bytes — one slot in a 16-instruction
/// cache line. When the result is already in x0 (Reg(0)), no move is needed.
fn emit_load_x0(out: &mut String, value: &IrValue) -> Result<(), CodegenError> {
    match value {
        IrValue::I32(n) => {
            emit_imm32(out, 0, *n)?;
        }
        IrValue::Unit => {
            // FLS §4.4: unit return. Convention: exit code 0 for main.
            writeln!(out, "    mov     x0, #0              // FLS §4.4: unit return")?;
        }
        IrValue::Reg(r) => {
            if *r == 0 {
                // Result already in x0 — no move needed.
                // Cache-line note: omitting the redundant mov saves 4 bytes
                // and keeps the return sequence as tight as possible.
            } else {
                // Move result from x{r} to x0 for the ARM64 return convention.
                // FLS §6.19: return value is placed in x0.
                writeln!(
                    out,
                    "    mov     x0, x{r}              // FLS §6.19: return reg {r} → x0"
                )?;
            }
        }
        // FLS §4.2: f64 return value — place in d0 per the ARM64 float ABI.
        IrValue::FReg(r) => {
            if *r != 0 {
                writeln!(
                    out,
                    "    fmov    d0, d{r}              // FLS §4.2: f64 return reg {r} → d0"
                )?;
            }
            // If r == 0, result is already in d0.
        }
        // FLS §4.2: f32 return value — place in s0 per the ARM64 float ABI.
        IrValue::F32Reg(r) => {
            if *r != 0 {
                writeln!(
                    out,
                    "    fmov    s0, s{r}              // FLS §4.2: f32 return reg {r} → s0"
                )?;
            }
            // If r == 0, result is already in s0.
        }
    }
    Ok(())
}

// ── Trampoline emission ───────────────────────────────────────────────────────

/// Emit a trampoline function for passing a capturing closure as `impl Fn`.
///
/// FLS §6.22, §4.13: The trampoline has the signature expected by the `impl Fn`
/// caller (only explicit arguments in x0..x{n_explicit-1}). It reads captured
/// values from ARM64 callee-saved registers x27/x26/… (which the original
/// caller loaded before `bl apply`), shifts the explicit arguments to make room,
/// and tail-calls the actual closure via `b` (preserving x30 for the closure's `ret`).
///
/// Assembly pattern for n_caps=1, n_explicit=1:
/// ```asm
/// __trampoline:
///     mov  x1, x0      // shift explicit arg[0] to position 1
///     mov  x0, x27     // cap[0] → position 0
///     b    __closure   // tail call (x30 unchanged → closure ret goes to apply)
/// ```
///
/// Cache-line note: 3–6 instructions (12–24 bytes) per trampoline, well under
/// one 64-byte instruction cache line.
fn emit_trampoline(
    out: &mut String,
    t: &crate::ir::ClosureTrampoline,
) -> Result<(), CodegenError> {
    writeln!(out, "    // trampoline for {} — FLS §6.22, §4.13", t.closure_name)?;
    writeln!(out, "    .global {}", t.name)?;
    writeln!(out, "{}:", t.name)?;

    // Shift explicit args up by n_caps positions (in reverse order to avoid
    // clobbering). Explicit arg i arrives in x{i} and must move to x{n_caps+i}.
    //
    // Cache-line note: one `mov` per explicit arg = 4 bytes each.
    for i in (0..t.n_explicit).rev() {
        let src = i;
        let dst = t.n_caps + i;
        if src != dst {
            writeln!(
                out,
                "    mov     x{dst}, x{src:<24} // shift explicit arg {i} to position {dst}"
            )?;
        }
    }

    // Move captures from callee-saved registers into their final positions.
    // cap[j] lives in x{27-j} (x27, x26, x25, …) and must go to x{j}.
    //
    // Cache-line note: one `mov` per capture = 4 bytes each.
    for j in 0..t.n_caps {
        let src_phys = 27usize.saturating_sub(j);
        let dst = j;
        writeln!(
            out,
            "    mov     x{dst}, x{src_phys:<24} // cap[{j}] from x{src_phys}"
        )?;
    }

    // Tail-call the actual closure. Using `b` (not `bl`) preserves x30 so the
    // closure's `ret` returns directly to the `impl Fn` caller's call site.
    writeln!(out, "    b       {}              // tail call closure", t.closure_name)?;
    Ok(())
}

// ── Entry point stub ──────────────────────────────────────────────────────────

/// Emit the `_start` ELF entry point.
///
/// `_start` calls `main` and passes its return value to `sys_exit`.
///
/// FLS §18.1: The `main` function is the program entry point. On Linux ELF
/// the actual entry symbol is `_start`; calling `main` from there and
/// exiting is the standard bare-metal bootstrap pattern.
///
/// ARM64 Linux syscall ABI:
/// - syscall number in `x8`
/// - first arg in `x0`
/// - `svc #0` to invoke
/// - `__NR_exit` = 93
///
/// Cache-line note: `_start` is 3 instructions (12 bytes), fits in the
/// first quarter of a 64-byte cache line.
fn emit_start(out: &mut String) -> Result<(), CodegenError> {
    writeln!(out, "    // ELF entry point — FLS §18.1")?;
    writeln!(out, "    .global _start")?;
    writeln!(out, "_start:")?;
    writeln!(out, "    bl      main            // call fn main()")?;
    writeln!(out, "    // x0 = main()'s return value")?;
    writeln!(out, "    mov     x8, #93         // __NR_exit (ARM64 Linux)")?;
    writeln!(out, "    svc     #0              // exit(x0)")?;
    Ok(())
}

/// Emit the `_galvanic_panic` runtime panic primitive.
///
/// FLS §6.23: Arithmetic operations that violate runtime invariants (e.g.
/// division by zero) must panic. Galvanic implements panic as an immediate
/// `exit(101)` syscall — no unwinding, no stack trace.
///
/// Exit code 101 is galvanic's internal panic sentinel. It is distinct from
/// normal exit (0 or main's return value) and SIGABRT-from-libc (128+6 = 134).
/// The choice is implementation-defined; FLS §6.23 requires a panic but does
/// not specify the exit code.
///
/// FLS §6.23: AMBIGUOUS — the spec requires a panic but does not specify
/// whether the panic is unwinding, abort, or exit. Galvanic chooses exit(101).
///
/// ARM64 Linux syscall: x8 = __NR_exit (93), x0 = exit code.
fn emit_galvanic_panic(out: &mut String) -> Result<(), CodegenError> {
    writeln!(out, "    // FLS §6.23: runtime panic primitive — exit(101)")?;
    writeln!(out, "    .global _galvanic_panic")?;
    writeln!(out, "_galvanic_panic:")?;
    writeln!(out, "    mov     x0, #101        // panic exit code (galvanic sentinel)")?;
    writeln!(out, "    mov     x8, #93         // __NR_exit (ARM64 Linux)")?;
    writeln!(out, "    svc     #0              // exit(101) — FLS §6.23: panic")?;
    Ok(())
}
