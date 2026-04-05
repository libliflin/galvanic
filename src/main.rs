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

    let tokens = match galvanic::lexer::tokenize(&source) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    };

    let source_file = match galvanic::parser::parse(&tokens, &source) {
        Ok(sf) => sf,
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    };

    println!("parsed {} item(s)", source_file.items.len());
}
