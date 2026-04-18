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

#[test]
fn partial_lower_emits_assembly_for_successful_fns() {
    // When some functions fail to lower and fn main succeeds, galvanic must
    // still emit a .s file for the functions that worked — the lead researcher
    // needs the artifact to inspect, even when one unsupported construct blocks
    // a minority of functions.
    // Exit code must remain non-zero (partial failure), but the file is written.
    let tmp_dir = tempfile::tempdir().unwrap();
    let src = tmp_dir.path().join("partial.rs");
    std::fs::write(
        &src,
        r#"
fn main() -> i32 {
    42
}
fn unsupported(x: i32, y: i32) -> i32 {
    match (x, y) { (0, 0) => 0, _ => 1 }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(&src)
        .output()
        .expect("failed to run galvanic");

    // Exit non-zero because unsupported() failed.
    assert!(
        !output.status.success(),
        "expected non-zero exit for partial failure, got {:?}",
        output.status
    );
    // Assembly file must exist.
    let asm_path = tmp_dir.path().join("partial.s");
    assert!(
        asm_path.exists(),
        "expected .s file to be emitted for partial success, but file not found"
    );
    // Assembly must mention main.
    let asm = std::fs::read_to_string(&asm_path).unwrap();
    assert!(
        asm.contains("main"),
        "expected main in emitted assembly, got: {asm}"
    );
    // Stdout must mention the partial output.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("partial"),
        "expected 'partial' in stdout for partial emission, got: {stdout}"
    );
}

#[test]
fn lower_error_names_expr_kind_variant() {
    // StructLit args in enum tuple variant constructors are now supported
    // (cycle 009 fix). This test was originally a regression guard for the
    // "not yet supported: StructLit expression" error message, but since the
    // feature was implemented it must instead verify that galvanic accepts
    // the construct and emits assembly without error.
    let mut tmp = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    write!(
        tmp,
        r#"
struct Foo {{ x: i32 }}
enum Maybe {{ Some(Foo) }}
fn main() -> i32 {{
    let _v = Maybe::Some(Foo {{ x: 7 }});
    0
}}
"#
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(tmp.path())
        .output()
        .expect("failed to run galvanic");

    let stderr = String::from_utf8_lossy(&output.stderr);
    // Galvanic should now compile this without error.
    assert!(
        output.status.success(),
        "expected zero exit for struct-literal enum variant arg, stderr: {stderr}"
    );
    // No "not yet supported" error should appear.
    assert!(
        !stderr.contains("not yet supported"),
        "unexpected 'not yet supported' in stderr: {stderr}"
    );
}

#[test]
fn no_main_prints_lowered_note() {
    // When lowering succeeds but no fn main is present, galvanic must print
    // a human-readable note so the compiler contributor knows the file was
    // processed correctly — not silently dropped.
    // Format: "galvanic: lowered N function(s) — no fn main, no assembly emitted"
    let mut tmp = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    write!(
        tmp,
        r#"
fn helper(x: i32) -> i32 {{
    x + 1
}}
"#
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(tmp.path())
        .output()
        .expect("failed to run galvanic");

    assert!(
        output.status.success(),
        "expected exit 0 for library-only file, got {:?}",
        output.status
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("lowered") && stdout.contains("function(s)") && stdout.contains("no fn main"),
        "expected no-main note in stdout, got: {stdout}"
    );
}

#[test]
fn lower_error_includes_fls_citation() {
    // Architecture invariant: every "not yet supported" error message must
    // include a FLS section citation so a Compiler Contributor can navigate
    // directly to the spec without reading surrounding code.
    // Expected form: "not yet supported: <construct> (FLS §X.Y)"
    //
    // This test exercises the nested-struct-field error in fls_5_patterns.rs,
    // which was the "worst moment" identified during cycle 016 customer
    // champion walk: a contributor hitting the error had no spec anchor.
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/fls_5_patterns.rs");

    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(&fixture)
        .output()
        .expect("failed to run galvanic");

    let stderr = String::from_utf8_lossy(&output.stderr);
    // At least one "not yet supported" error is expected from this fixture.
    assert!(
        stderr.contains("not yet supported"),
        "expected at least one unsupported error from patterns fixture, got: {stderr}"
    );
    // Every "not yet supported" line must carry a FLS section citation.
    for line in stderr.lines() {
        if line.contains("not yet supported") {
            assert!(
                line.contains("(FLS §"),
                "error line missing FLS citation: {line}"
            );
        }
    }
}

#[test]
fn partial_lower_no_main_emits_inspection_assembly() {
    // When fn main fails to lower but other functions succeed, galvanic must
    // emit a partial .s file annotated "inspection-only" so the Lead Researcher
    // has an artifact to inspect. Exit code must be non-zero (lower errors occurred).
    //
    // Uses fls_5_patterns.rs: 20 of 21 functions lower, main fails on a nested
    // struct pattern (§5.10.2). Before this cycle, all 20 successful lowerings
    // were silently discarded.
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/fls_5_patterns.rs");

    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(&fixture)
        .output()
        .expect("failed to run galvanic");

    assert!(
        !output.status.success(),
        "expected non-zero exit when lower errors occurred, got {:?}",
        output.status
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("inspection-only"),
        "expected 'inspection-only' annotation in stdout, got: {stdout}"
    );
    assert!(
        stdout.contains("no fn main"),
        "expected 'no fn main' in stdout, got: {stdout}"
    );
    assert!(
        stdout.contains("no entry point"),
        "expected 'no entry point' in stdout, got: {stdout}"
    );

    // The .s file must actually exist alongside the fixture.
    let asm_path = fixture.with_extension("s");
    assert!(
        asm_path.exists(),
        "expected .s file to be written at {}, but it was not",
        asm_path.display()
    );

    // The assembly content must carry the inspection-only annotation comment.
    let asm = std::fs::read_to_string(&asm_path).expect("failed to read .s file");
    assert!(
        asm.contains("inspection-only"),
        "expected 'inspection-only' comment in emitted assembly, got first 200 chars: {}",
        &asm[..asm.len().min(200)]
    );

    // Clean up the .s file so the fixture directory stays pristine.
    let _ = std::fs::remove_file(&asm_path);
}

#[test]
fn lower_source_all_unsupported_strings_cite_fls() {
    // Static invariant: every non-comment line in src/lower.rs containing
    // "not yet supported" must also contain "(FLS §" on the same line.
    // This complements the runtime smoke test by catching new uncited strings
    // before they reach an exercised code path. The Display impl format string
    // (`write!(f, "not yet supported: {msg}")`) is excluded — it is the prefix
    // machinery, not a message payload.
    let src = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/lower.rs"),
    )
    .expect("failed to read src/lower.rs");

    let mut violations = Vec::new();
    for (i, line) in src.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with('*') || trimmed.contains("write!(f,") {
            continue;
        }
        if trimmed.contains("not yet supported") && !trimmed.contains("(FLS §") {
            violations.push(format!("lower.rs:{}: {trimmed}", i + 1));
        }
    }
    assert!(
        violations.is_empty(),
        "lower.rs has 'not yet supported' strings without FLS citations:\n{}",
        violations.join("\n")
    );
}
