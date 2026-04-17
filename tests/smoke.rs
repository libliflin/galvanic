use std::process::Command;

#[test]
fn empty_file_exits_zero() {
    let empty = tempfile::NamedTempFile::with_suffix(".rs").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(empty.path())
        .output()
        .expect("failed to run galvanic");

    assert!(output.status.success(), "expected exit 0, got {:?}", output.status);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("galvanic: compiling"), "unexpected output: {stdout}");
}

#[test]
fn lower_error_names_failing_item() {
    // When lowering fails, the error message must include the item name so
    // the compiler contributor can navigate directly to the problem without
    // binary-searching through a multi-item fixture.
    // Format: "error: lower failed in '<name>': not yet supported: ..."
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/fls_9_functions.rs");

    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(&fixture)
        .output()
        .expect("failed to run galvanic");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error: lower failed in '"),
        "expected item name in error, got: {stderr}"
    );
    assert!(
        !output.status.success(),
        "expected non-zero exit for unsupported fixture"
    );
}
