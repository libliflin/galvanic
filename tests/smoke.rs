use std::io::Write;
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

#[test]
fn lower_error_shows_summary_line() {
    // When at least one function fails, galvanic must print a summary line of
    // the form "lowered N of M functions (K failed)" so the Lead Researcher
    // can read progress in a single run.
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/fls_6_18_match_expressions.rs");

    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(&fixture)
        .output()
        .expect("failed to run galvanic");

    let stderr = String::from_utf8_lossy(&output.stderr);
    // Summary format: "lowered N of M functions (K failed)"
    assert!(
        stderr.contains("lowered ") && stderr.contains(" functions (") && stderr.contains(" failed)"),
        "expected summary line in stderr, got: {stderr}"
    );
    // At least one success should be reported (fixture has 10+ working functions)
    assert!(
        !stderr.contains("lowered 0 of "),
        "expected some successes in summary, got: {stderr}"
    );
    assert!(!output.status.success(), "expected non-zero exit");
}

#[test]
fn lower_error_reports_all_failures() {
    // When multiple functions fail, ALL errors must be reported — not just the
    // first — so the researcher sees the full error landscape in a single run.
    let mut tmp = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    // Two functions that both use a tuple expression as a match scrutinee —
    // an unsupported construct at this milestone. The third function is trivial
    // and must succeed, so the summary shows "1 of 3" rather than "0 of 3".
    write!(
        tmp,
        r#"
fn fail_a(x: i32, y: i32) -> i32 {{
    match (x, y) {{ (0, 0) => 0, _ => 1 }}
}}
fn fail_b(x: i32, y: i32) -> i32 {{
    match (x, y) {{ (1, 1) => 1, _ => 0 }}
}}
fn succeed() -> i32 {{
    42
}}
"#
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(tmp.path())
        .output()
        .expect("failed to run galvanic");

    let stderr = String::from_utf8_lossy(&output.stderr);
    // Both failing functions must appear in the output.
    assert!(
        stderr.contains("fail_a") && stderr.contains("fail_b"),
        "expected both failing functions named in stderr, got: {stderr}"
    );
    // Summary must reflect 1 success and 2 failures.
    assert!(
        stderr.contains("lowered 1 of 3 functions (2 failed)"),
        "expected summary 'lowered 1 of 3 functions (2 failed)', got: {stderr}"
    );
    assert!(!output.status.success(), "expected non-zero exit");
}
