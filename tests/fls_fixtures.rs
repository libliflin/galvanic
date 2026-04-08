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

#[test]
fn fls_5_patterns() {
    assert_galvanic_accepts("fls_5_patterns.rs");
}

#[test]
fn fls_13_traits() {
    assert_galvanic_accepts("fls_13_traits.rs");
}

#[test]
fn fls_7_1_consts() {
    assert_galvanic_accepts("fls_7_1_consts.rs");
}

#[test]
fn fls_7_2_statics() {
    assert_galvanic_accepts("fls_7_2_statics.rs");
}

#[test]
fn fls_4_10_type_aliases() {
    assert_galvanic_accepts("fls_4_10_type_aliases.rs");
}

#[test]
fn fls_10_3_assoc_consts() {
    assert_galvanic_accepts("fls_10_3_assoc_consts.rs");
}

#[test]
fn fls_10_2_assoc_types() {
    assert_galvanic_accepts("fls_10_2_assoc_types.rs");
}

#[test]
fn fls_6_4_2_const_block() {
    assert_galvanic_accepts("fls_6_4_2_const_block.rs");
}

#[test]
fn fls_6_4_4_unsafe_block() {
    assert_galvanic_accepts("fls_6_4_4_unsafe_block.rs");
}

#[test]
fn fls_12_1_generic_fns() {
    assert_galvanic_accepts("fls_12_1_generic_fns.rs");
}

#[test]
fn fls_12_1_generic_methods() {
    assert_galvanic_accepts("fls_12_1_generic_methods.rs");
}

#[test]
fn fls_12_1_generic_structs() {
    assert_galvanic_accepts("fls_12_1_generic_structs.rs");
}

#[test]
fn fls_12_1_generic_impl() {
    assert_galvanic_accepts("fls_12_1_generic_impl.rs");
}
