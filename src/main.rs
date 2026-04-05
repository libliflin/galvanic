use std::env;
use std::path::Path;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("usage: galvanic <source.rs> [-o <output>]");
        process::exit(1);
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
            process::exit(1);
        }
    };

    // ── Lex ───────────────────────────────────────────────────────────────────
    let tokens = match galvanic::lexer::tokenize(&source) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    };

    // ── Parse ─────────────────────────────────────────────────────────────────
    let source_file = match galvanic::parser::parse(&tokens, &source) {
        Ok(sf) => sf,
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    };

    println!("parsed {} item(s)", source_file.items.len());

    // ── Lower AST → IR ────────────────────────────────────────────────────────
    let module = match galvanic::lower::lower(&source_file, &source) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("note: skipping codegen ({e})");
            return;
        }
    };

    // Nothing to compile if there is no entry point.
    if !module.fns.iter().any(|f| f.name == "main") {
        return;
    }

    // ── Emit ARM64 assembly ───────────────────────────────────────────────────
    let asm = match galvanic::codegen::emit_asm(&module) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    };

    if let Some(out) = output_path {
        // If -o was given, assemble and link into a binary.
        if let Err(e) = assemble_and_link(&asm, out) {
            eprintln!("error: {e}");
            process::exit(1);
        }
        println!("galvanic: wrote {out}");
    } else {
        // Otherwise, write {input_stem}.s alongside the source file.
        let out_path = source_path.with_extension("s");
        if let Err(e) = std::fs::write(&out_path, &asm) {
            eprintln!("error: could not write {}: {e}", out_path.display());
            process::exit(1);
        }
        println!("galvanic: emitted {}", out_path.display());
    }
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
