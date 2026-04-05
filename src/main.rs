use std::env;
use std::path::Path;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("usage: galvanic <source.rs>");
        process::exit(1);
    }

    let source_path = Path::new(&args[1]);
    let filename = source_path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(&args[1]);

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
    // Lowering is non-fatal: programs with unsupported features (let bindings,
    // complex expressions, etc.) parse fine but cannot be lowered yet.
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

    // Write {input_stem}.s alongside the source file.
    let out_path = source_path.with_extension("s");
    if let Err(e) = std::fs::write(&out_path, &asm) {
        eprintln!("error: could not write {}: {e}", out_path.display());
        process::exit(1);
    }

    println!("galvanic: emitted {}", out_path.display());
}
