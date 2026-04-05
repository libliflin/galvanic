//! Integration tests using FLS-derived fixture programs.
//!
//! Each fixture is a real Rust program with examples drawn from the
//! Ferrocene Language Specification. These tests verify galvanic can
//! lex and parse them. When codegen exists, they should also compile
//! and run the output.

use std::process::Command;

/// Run galvanic on a fixture file and assert it exits 0 (successful parse).
fn assert_galvanic_accepts(fixture: &str) {
    let fixture_path = format!(
        "{}/tests/fixtures/{fixture}",
        env!("CARGO_MANIFEST_DIR")
    );

    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(&fixture_path)
        .output()
        .expect("failed to run galvanic");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "galvanic rejected {fixture}:\nstdout: {stdout}\nstderr: {stderr}"
    );
}

#[test]
fn fls_2_4_literals() {
    assert_galvanic_accepts("fls_2_4_literals.rs");
}

#[test]
fn fls_6_expressions() {
    assert_galvanic_accepts("fls_6_expressions.rs");
}

#[test]
fn fls_9_functions() {
    assert_galvanic_accepts("fls_9_functions.rs");
}
