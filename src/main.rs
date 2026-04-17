use std::env;
use std::path::Path;
use std::process;

/// Stack size for the compilation thread.
///
/// The lexer, parser, lowering, and codegen passes all use recursive descent.
/// Deeply nested source files can overflow the default thread stack (typically
/// 8 MB on macOS/Linux). We run the pipeline in a thread with a larger stack
/// so that adversarial inputs produce a clean parse error (via the parser's
/// MAX_BLOCK_DEPTH limit) rather than an OS signal.
///
/// 64 MB matches rustc's own compilation-thread stack budget.
const COMPILE_STACK_SIZE: usize = 64 * 1024 * 1024;

fn main() {
    let args: Vec<String> = env::args().collect();

    // Run the entire compilation pipeline in a thread with a larger stack so
    // deeply-nested programs produce a clean error instead of a signal death.
    let child = std::thread::Builder::new()
        .stack_size(COMPILE_STACK_SIZE)
        .spawn(move || compile(args))
        .expect("failed to spawn compilation thread");

    // If the child calls process::exit the whole process exits immediately and
    // join() is unreachable. Otherwise join() propagates the exit code.
    match child.join() {
        Ok(code) => process::exit(code),
        Err(_) => process::exit(101),
    }
}

/// Run the full compilation pipeline. Returns the intended process exit code
/// so the caller can use process::exit without preventing unwinding in tests.
fn compile(args: Vec<String>) -> i32 {
    if args.len() < 2 {
        eprintln!("usage: galvanic <source.rs> [-o <output>]");
        return 1;
    }

    let source_path = Path::new(&args[1]);
    let filename = source_path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(&args[1]);

    // Parse optional -o <output> flag.
    let output_path: Option<&str> = args
        .windows(2)
        .find(|w| w[0] == "-o")
        .map(|w| w[1].as_str());

    println!("galvanic: compiling {filename}");

    let source = match std::fs::read_to_string(source_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: could not read {filename}: {e}");
            return 1;
        }
    };

    // ── Lex ───────────────────────────────────────────────────────────────────
    let tokens = match galvanic::lexer::tokenize(&source) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };

    // ── Parse ─────────────────────────────────────────────────────────────────
    let source_file = match galvanic::parser::parse(&tokens, &source) {
        Ok(sf) => sf,
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };

    println!("parsed {} item(s)", source_file.items.len());

    // ── Lower AST → IR ────────────────────────────────────────────────────────
    let (module, had_lower_errors) = match galvanic::lower::lower(&source_file, &source) {
        Ok(m) => (m, false),
        Err(errs) => {
            // Print every per-function error so the researcher sees the full
            // error landscape in a single run (not just the first failure).
            for e in &errs.errors {
                eprintln!("error: lower failed {e}");
            }
            // Summary line: how many succeeded vs how many were attempted.
            // Omitted when fn_count == 0 (e.g. file with only struct/enum defs).
            if errs.fn_count > 0 {
                eprintln!(
                    "lowered {} of {} functions ({} failed)",
                    errs.success_count,
                    errs.fn_count,
                    errs.errors.len()
                );
            }
            // If some functions did lower successfully, emit assembly for them
            // so the researcher has an artifact to inspect (the goal of partial
            // output: a partial success should not be entirely silent).
            match errs.partial_module {
                Some(partial) => (*partial, true),
                None => return 1,
            }
        }
    };

    // Nothing to compile if there is no entry point.
    if !module.fns.iter().any(|f| f.name == "main") {
        println!(
            "galvanic: lowered {} function(s) — no fn main, no assembly emitted",
            module.fns.len()
        );
        // Exit non-zero if lower errors occurred, even when there's no fn main.
        return if had_lower_errors { 1 } else { 0 };
    }

    // ── Emit ARM64 assembly ───────────────────────────────────────────────────
    let asm = match galvanic::codegen::emit_asm(&module) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };

    if let Some(out) = output_path {
        // If -o was given, assemble and link into a binary.
        if let Err(e) = assemble_and_link(&asm, out) {
            eprintln!("error: {e}");
            return 1;
        }
        println!("galvanic: wrote {out}");
    } else {
        // Otherwise, write {input_stem}.s alongside the source file.
        let out_path = source_path.with_extension("s");
        if let Err(e) = std::fs::write(&out_path, &asm) {
            eprintln!("error: could not write {}: {e}", out_path.display());
            return 1;
        }
        if had_lower_errors {
            // Partial output: some functions failed, but we emit what succeeded.
            println!("galvanic: emitted {} (partial — some functions failed)", out_path.display());
        } else {
            println!("galvanic: emitted {}", out_path.display());
        }
    }

    // Exit non-zero if any lower errors occurred, even with partial assembly.
    if had_lower_errors { 1 } else { 0 }
}

/// Write assembly text to a temp file, assemble it to an object file, and
/// link it into a standalone ARM64 Linux ELF binary.
///
/// Uses GNU binutils cross tools:
///   - `aarch64-linux-gnu-as`  (assembler)
///   - `aarch64-linux-gnu-ld`  (linker)
///
/// These are available in the `gcc-aarch64-linux-gnu` package on Debian/Ubuntu.
/// Run the resulting binary with `qemu-aarch64` on non-ARM64 hosts.
///
/// The intermediate `.s` and `.o` files are cleaned up on success.
fn assemble_and_link(asm: &str, output: &str) -> Result<(), String> {
    use std::process::Command;

    let asm_path = format!("{output}.s");
    let obj_path = format!("{output}.o");

    // Write the assembly text.
    std::fs::write(&asm_path, asm)
        .map_err(|e| format!("could not write assembly to {asm_path}: {e}"))?;

    // Assemble: .s → .o
    let as_status = Command::new("aarch64-linux-gnu-as")
        .args(["-o", &obj_path, &asm_path])
        .status()
        .map_err(|e| {
            format!(
                "could not run aarch64-linux-gnu-as: {e}\n\
                 hint: install gcc-aarch64-linux-gnu (Debian/Ubuntu)"
            )
        })?;

    if !as_status.success() {
        return Err(format!(
            "assembler failed (exit {as_status}); assembly was:\n{asm}"
        ));
    }

    // Link: .o → ELF binary (no libc, bare _start entry point)
    let ld_status = Command::new("aarch64-linux-gnu-ld")
        .args(["-o", output, &obj_path])
        .status()
        .map_err(|e| {
            format!(
                "could not run aarch64-linux-gnu-ld: {e}\n\
                 hint: install gcc-aarch64-linux-gnu (Debian/Ubuntu)"
            )
        })?;

    if !ld_status.success() {
        return Err(format!("linker failed (exit {ld_status})"));
    }

    // Clean up intermediates.
    let _ = std::fs::remove_file(&asm_path);
    let _ = std::fs::remove_file(&obj_path);

    Ok(())
}
