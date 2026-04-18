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
    //
    // Uses fls_4_14_where_clauses_on_types.rs because it contains constructs
    // not yet supported (method calls on primitive types), which produce a
    // reliable lower-error. fls_9_functions.rs now lowers cleanly after the
    // §8.2 expression-statement fix in cycle 026.
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/fls_4_14_where_clauses_on_types.rs");

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
    // Uses a tuple-parameter function (still unsupported) alongside a trivial
    // success so the summary shows partial success.
    let mut tmp = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    write!(
        tmp,
        r#"
fn fail(t: (i32, i32)) -> i32 {{ t.0 }}
fn succeed() -> i32 {{ 42 }}
"#
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(tmp.path())
        .output()
        .expect("failed to run galvanic");

    let stderr = String::from_utf8_lossy(&output.stderr);
    // Summary format: "lowered N of M functions (K failed)"
    assert!(
        stderr.contains("lowered ") && stderr.contains(" functions (") && stderr.contains(" failed)"),
        "expected summary line in stderr, got: {stderr}"
    );
    // At least one success should be reported (succeed() compiles fine).
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
    // Two functions that both use a tuple parameter — still unsupported.
    // The third function is trivial and must succeed, so the summary shows
    // "1 of 3" rather than "0 of 3".
    write!(
        tmp,
        r#"
fn fail_a(t: (i32, i32)) -> i32 {{ t.0 }}
fn fail_b(t: (i32, i32)) -> i32 {{ t.1 }}
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
    // Uses a tuple-parameter function (still unsupported) as the failing case.
    let tmp_dir = tempfile::tempdir().unwrap();
    let src = tmp_dir.path().join("partial.rs");
    std::fs::write(
        &src,
        r#"
fn main() -> i32 {
    42
}
fn unsupported(t: (i32, i32)) -> i32 {
    t.0
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
fn tuple_scrutinee_match_compiles_and_emits_asm() {
    // Cycle 028 added the error message; this cycle implements tuple scrutinee
    // match. Verify the program now compiles successfully and emits assembly.
    //
    // FLS §6.18, §6.10: `match (x, y) { (0, 0) => 0, _ => 1 }` — tuple
    // expression as scrutinee with Pat::Tuple arm and wildcard default arm.
    let mut tmp = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    write!(
        tmp,
        r#"
fn match_tuple(x: i32, y: i32) -> i32 {{
    match (x, y) {{ (0, 0) => 0, _ => 1 }}
}}
fn main() {{}}
"#
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(tmp.path())
        .output()
        .expect("failed to run galvanic");

    assert!(
        output.status.success(),
        "expected zero exit for tuple scrutinee match, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn tuple_parameter_error_names_fls_sections_and_fix_site() {
    // Cycle 031 goal: when a contributor writes `fn first(t: (i32, i32)) -> i32 { t.0 }`,
    // the error must cite FLS §4.4 and §6.10 AND point to the tuple_struct_defs branch
    // in lower_fn as the fix site — mirroring the tuple scrutinee guidance from cycle 028.
    //
    // Before this cycle the error had the right citations but no fix-site hint.
    // A contributor following the error hit lower_ty (a leaf function, not the fix site)
    // and had no path to the parameter-handling dispatch in lower_fn ~750 lines away.
    let mut tmp = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    write!(
        tmp,
        r#"
fn first(t: (i32, i32)) -> i32 {{ t.0 }}
"#
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(tmp.path())
        .output()
        .expect("failed to run galvanic");

    let stderr = String::from_utf8_lossy(&output.stderr);
    // Must cite both FLS sections.
    assert!(
        stderr.contains("FLS §4.4"),
        "expected FLS §4.4 citation in tuple parameter error, got: {stderr}"
    );
    assert!(
        stderr.contains("§6.10"),
        "expected §6.10 citation in tuple parameter error, got: {stderr}"
    );
    // Must point the contributor to the right fix site.
    assert!(
        stderr.contains("tuple_struct_defs"),
        "expected fix-site hint 'tuple_struct_defs' in tuple parameter error, got: {stderr}"
    );
    assert!(!output.status.success(), "expected non-zero exit");
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

    // _start must NOT be emitted — inspection-only output has no entry point.
    assert!(
        !asm.contains("_start"),
        "inspection-only assembly must not contain _start; it would produce a binary with no entry point"
    );

    // The 20 successfully lowered function bodies must be present.
    // Count label lines (lines matching "^<ident>:") as a proxy for function count.
    let fn_labels: Vec<&str> = asm
        .lines()
        .filter(|l| {
            let t = l.trim();
            t.ends_with(':')
                && !t.starts_with('.')
                && !t.starts_with("//")
                && t != "_galvanic_panic:"
        })
        .collect();
    assert!(
        fn_labels.len() >= 20,
        "expected ≥20 function labels in inspection-only assembly (one per successfully lowered fn), found {}: {:?}",
        fn_labels.len(),
        &fn_labels[..fn_labels.len().min(5)]
    );

    // Clean up the .s file so the fixture directory stays pristine.
    let _ = std::fs::remove_file(&asm_path);
}

/// When `main` is the only function and it fails to lower, no assembly is emitted
/// and the exit code is non-zero. This is the zero-function boundary: `partial_module`
/// is `None` so the code returns early with exit 1 before reaching any emit path.
///
/// Distinct from the inspection-only path (main fails but ≥1 other function succeeded).
#[test]
fn main_only_fails_emits_no_assembly() {
    // A file with struct defs and only fn main, where main fails to lower.
    // Outer { inner: x } uses a variable for a nested struct field — not yet supported.
    let mut tmp = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    write!(
        tmp,
        r#"
struct Inner {{ a: i32 }}
struct Outer {{ inner: Inner }}
fn main() -> i32 {{
    let x = Inner {{ a: 1 }};
    let o = Outer {{ inner: x }};
    o.inner.a
}}
"#
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(tmp.path())
        .output()
        .expect("failed to run galvanic");

    // Must exit non-zero — lower errors occurred.
    assert!(
        !output.status.success(),
        "expected non-zero exit when only function fails to lower, got {:?}",
        output.status
    );

    // No .s file must be created — zero functions lowered successfully.
    let asm_path = tmp.path().with_extension("s");
    assert!(
        !asm_path.exists(),
        "expected no .s file when zero functions lowered, but found one at {}",
        asm_path.display()
    );

    // Must NOT emit the inspection-only message — that path requires ≥1 successful fn.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("inspection-only"),
        "must not emit inspection-only message when zero functions lowered, got: {stdout}"
    );
}

#[test]
fn tuple_type_error_cites_fls_sections() {
    // Cycle 030 goal: the error for a function with a tuple parameter or return
    // type must cite FLS §4.4 (Tuple types) and §6.10 (Tuple expressions).
    // Before this cycle the message was "tuple type in scalar context (use tuple
    // pattern parameter instead)" — a workaround hint with no FLS citation.
    // The static ratchet test alone doesn't pin the runtime form; this does.
    let mut tmp = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    write!(
        tmp,
        r#"
fn swap(pair: (i32, i32)) -> (i32, i32) {{ (pair.1, pair.0) }}
"#
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(tmp.path())
        .output()
        .expect("failed to run galvanic");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("FLS §4.4"),
        "expected FLS §4.4 citation in tuple type error, got: {stderr}"
    );
    assert!(
        stderr.contains("§6.10"),
        "expected §6.10 citation in tuple type error, got: {stderr}"
    );
    assert!(
        stderr.contains("tuple type in scalar context"),
        "expected 'tuple type in scalar context' construct name in error, got: {stderr}"
    );
    assert!(!output.status.success(), "expected non-zero exit");
}

#[test]
fn field_access_on_scalar_error_cites_fls_section() {
    // Cycle 030 goal: the error for field access on a non-struct (scalar) value
    // must cite FLS §6.13 (Field access expressions).
    // Before this cycle the message was "field access on scalar value (field `X`)"
    // with no FLS citation.
    let mut tmp = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    write!(
        tmp,
        r#"
fn bad_field(x: i32) -> i32 {{ x.foo }}
"#
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(tmp.path())
        .output()
        .expect("failed to run galvanic");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("FLS §6.13"),
        "expected FLS §6.13 citation in field-access-on-scalar error, got: {stderr}"
    );
    assert!(
        stderr.contains("field access on scalar value"),
        "expected 'field access on scalar value' construct name in error, got: {stderr}"
    );
    assert!(!output.status.success(), "expected non-zero exit");
}

#[test]
fn tuple_index_access_error_cites_fls_sections() {
    // Cycle 031 goal: when a named binding holds a tuple return value and the
    // caller accesses it via numeric index (`p.0`), the error must cite both
    // FLS §6.10 (Tuple Expressions) and §6.13 (Field Access Expressions) and
    // name the construct "tuple index access on non-destructured binding".
    // Before this cycle the message was the generic "field access on scalar
    // value: `0` (FLS §6.13)" — no §6.10 cite, no guidance toward destructuring.
    let mut tmp = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    write!(
        tmp,
        r#"
fn make_pair(a: i32, b: i32) -> (i32, i32) {{ (a, b) }}
fn main() -> i32 {{
    let p = make_pair(3, 7);
    p.0 + p.1 - 10
}}
"#
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(tmp.path())
        .output()
        .expect("failed to run galvanic");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("FLS §6.10"),
        "expected FLS §6.10 citation in tuple-index error, got: {stderr}"
    );
    assert!(
        stderr.contains("§6.13"),
        "expected §6.13 citation in tuple-index error, got: {stderr}"
    );
    assert!(
        stderr.contains("tuple index access on non-destructured binding"),
        "expected 'tuple index access on non-destructured binding' construct name in error, got: {stderr}"
    );
    assert!(!output.status.success(), "expected non-zero exit");
}

#[test]
fn lifetime_parameter_parse_error_cites_fls() {
    // FLS §12.1, §4.14: When the parser sees a lifetime parameter (`'a`)
    // in a generic parameter list, the error message must cite the FLS section
    // so a Compiler Contributor can navigate directly to the spec.
    //
    // Contributor journey: they write `fn longest<'a>(...)` and get an error.
    // The error must say "(FLS §12.1" so they know which section to read and
    // which file to fix (parse_fn_def() in parser.rs).
    let mut tmp = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    write!(
        tmp,
        r#"fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {{ x }}
fn main() {{}}
"#
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(tmp.path())
        .output()
        .expect("failed to run galvanic");

    assert!(!output.status.success(), "expected non-zero exit for lifetime params");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("FLS §12.1"),
        "expected FLS §12.1 citation in lifetime parameter error, got: {stderr}"
    );
    assert!(
        stderr.contains("lifetime"),
        "expected 'lifetime' in error message, got: {stderr}"
    );
}

#[test]
fn lifetime_annotated_ref_parse_error_cites_fls() {
    // FLS §4.8, §4.14: When the parser sees a lifetime-annotated reference type
    // (`&'static T`, `&'a T`) in any type position, the error must cite FLS §4.8
    // and §4.14 so a contributor knows which spec sections to read.
    let mut tmp = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    write!(
        tmp,
        "fn foo(x: &'static str) -> &'static str {{ x }}\nfn main() {{}}\n"
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(tmp.path())
        .output()
        .expect("failed to run galvanic");

    assert!(!output.status.success(), "expected non-zero exit for lifetime-annotated ref");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("FLS §4.8"),
        "expected FLS §4.8 citation in lifetime-annotated ref error, got: {stderr}"
    );
    assert!(
        stderr.contains("lifetime"),
        "expected 'lifetime' in error message, got: {stderr}"
    );
}

#[test]
fn lifetime_bound_parse_error_cites_fls() {
    // FLS §4.14, §12.1: When the parser sees a lifetime bound (`T: 'static`,
    // `T: 'a`) in a generic parameter, the error must cite FLS §4.14 so a
    // contributor knows which spec section to read.
    let mut tmp = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    write!(
        tmp,
        "fn foo<T: 'static>(x: T) -> T {{ x }}\nfn main() {{}}\n"
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(tmp.path())
        .output()
        .expect("failed to run galvanic");

    assert!(!output.status.success(), "expected non-zero exit for lifetime bound");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("FLS §4.14"),
        "expected FLS §4.14 citation in lifetime bound error, got: {stderr}"
    );
    assert!(
        stderr.contains("lifetime"),
        "expected 'lifetime' in error message, got: {stderr}"
    );
}

#[test]
fn where_clause_lifetime_bound_parses_cleanly() {
    // FLS §4.14: `where T: 'static` and `where T: 'a + Trait` are valid
    // where-clause predicates. galvanic discards all where-clause bounds —
    // this test confirms the Lifetime token in bound position no longer
    // produces "expected OpenBrace, found Lifetime".
    let mut tmp = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    write!(
        tmp,
        "fn foo<T>(x: T) -> i32 where T: 'static {{ 0 }}\nfn main() {{}}\n"
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(tmp.path())
        .output()
        .expect("failed to run galvanic");

    assert!(
        output.status.success(),
        "expected exit 0 for where T: 'static, got: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn where_clause_mixed_lifetime_and_trait_bound_parses_cleanly() {
    // FLS §4.14: `where T: 'static + Clone` mixes a lifetime bound and a trait
    // bound separated by `+`. Both tokens must be consumed correctly.
    let mut tmp = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    write!(
        tmp,
        "fn foo<T>(x: i32) -> i32 where T: 'static + Copy {{ x }}\nfn main() {{}}\n"
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(tmp.path())
        .output()
        .expect("failed to run galvanic");

    assert!(
        output.status.success(),
        "expected exit 0 for where T: 'static + Copy, got: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn where_clause_lifetime_lhs_parses_cleanly() {
    // FLS §4.14: `where 'static: 'static` — a lifetime outlives predicate with a
    // bare lifetime on the LHS. galvanic discards where-clause bounds; the Lifetime
    // token in LHS position must be consumed silently rather than producing
    // "expected OpenBrace, found Lifetime".
    let mut tmp = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    write!(
        tmp,
        "fn foo<T>(x: i32) -> i32 where 'static: 'static {{ x }}\nfn main() {{}}\n"
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(tmp.path())
        .output()
        .expect("failed to run galvanic");

    assert!(
        output.status.success(),
        "expected exit 0 for where 'static: 'static, got: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn struct_lifetime_param_parse_error_cites_fls() {
    // FLS §12.1, §4.14: `struct Foo<'a>` — lifetime parameter in struct definition.
    // Before this fix: opaque "expected type parameter name or `>`, found Lifetime".
    // After: cited error naming FLS §12.1, §4.14 and the fix site.
    let mut tmp = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    write!(tmp, "struct Foo<'a> {{ x: i32 }}\nfn main() {{}}\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(tmp.path())
        .output()
        .expect("failed to run galvanic");

    assert!(
        !output.status.success(),
        "expected non-zero exit for struct with lifetime param"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("FLS §12.1"),
        "expected FLS §12.1 citation in error, got: {stderr}"
    );
    assert!(
        stderr.contains("lifetime"),
        "expected 'lifetime' in error message, got: {stderr}"
    );
}

#[test]
fn enum_lifetime_param_parse_error_cites_fls() {
    // FLS §12.1, §4.14: `enum Foo<'a>` — lifetime parameter in enum definition.
    // Before this fix: infinite loop (parser hung on unrecognized Lifetime token
    // in the while loop, since neither the Ident nor Comma branches advanced).
    // After: cited error + clean exit.
    let mut tmp = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    write!(tmp, "enum Bar<'a> {{ X }}\nfn main() {{}}\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(tmp.path())
        .output()
        .expect("failed to run galvanic");

    assert!(
        !output.status.success(),
        "expected non-zero exit for enum with lifetime param"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("FLS §12.1"),
        "expected FLS §12.1 citation in error, got: {stderr}"
    );
    assert!(
        stderr.contains("lifetime"),
        "expected 'lifetime' in error message, got: {stderr}"
    );
}

#[test]
fn impl_lifetime_param_parse_error_cites_fls() {
    // FLS §12.1, §4.14: `impl<'a> Foo` — lifetime parameter in impl block.
    // Before this fix: opaque "expected type parameter name or `>` in impl generic
    // params, found Lifetime" with no FLS citation.
    // After: cited error naming FLS §12.1, §4.14 and the fix site.
    let mut tmp = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    write!(
        tmp,
        "struct Baz {{ x: i32 }}\nimpl<'a> Baz {{}}\nfn main() {{}}\n"
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(tmp.path())
        .output()
        .expect("failed to run galvanic");

    assert!(
        !output.status.success(),
        "expected non-zero exit for impl with lifetime param"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("FLS §12.1"),
        "expected FLS §12.1 citation in error, got: {stderr}"
    );
    assert!(
        stderr.contains("lifetime"),
        "expected 'lifetime' in error message, got: {stderr}"
    );
}

#[test]
fn lower_source_all_unsupported_strings_cite_fls() {
    // Static invariant: every LowerError::Unsupported( call site in src/lower.rs
    // must supply a message string that contains "(FLS §". This ensures every
    // "not yet supported" error emitted to users names the spec section.
    //
    // The previous version of this test scanned for the literal text
    // "not yet supported" in source lines — a string that appears only in the
    // Display impl (`write!(f, "not yet supported: {msg}")`), never in the
    // message payloads themselves. That scan matched zero call sites and the
    // test passed vacuously while hundreds of messages lacked FLS citations.
    //
    // This version scans for `LowerError::Unsupported(` and inspects a window
    // of lines around each call site for `(FLS §`. New call sites without a
    // citation will push the violation count above MAX_UNCITED_VIOLATIONS and
    // fail CI — a ratchet. As citations are added, lower MAX_UNCITED_VIOLATIONS.
    //
    // Known debt as of cycle 031: 287 call sites still lack FLS citations.
    // Every new Unsupported call must include a citation; reduce the debt
    // incrementally by adding citations when touching the surrounding code.
    const MAX_UNCITED_VIOLATIONS: usize = 287;

    let src = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/lower.rs"),
    )
    .expect("failed to read src/lower.rs");

    let lines: Vec<&str> = src.lines().collect();
    let mut violations = Vec::new();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        // Skip comments and the Display impl which holds the prefix machinery.
        if trimmed.starts_with("//")
            || trimmed.starts_with('*')
            || trimmed.contains("write!(f,")
        {
            continue;
        }
        if trimmed.contains("LowerError::Unsupported(") {
            // Inspect a 3-line window (the call line plus the next two) for
            // a FLS citation. Most call sites are single-line or two-line;
            // three lines covers all observed patterns.
            let window_end = (i + 3).min(lines.len());
            let window = lines[i..window_end].join(" ");
            if !window.contains("(FLS §") {
                violations.push(format!("lower.rs:{}: {}", i + 1, trimmed));
            }
        }
    }

    assert!(
        violations.len() <= MAX_UNCITED_VIOLATIONS,
        "lower.rs has {} LowerError::Unsupported call site(s) without FLS citations \
         (max allowed: {}). New call sites must include a citation. \
         Reduce MAX_UNCITED_VIOLATIONS as debt is paid down.\nFirst new violation(s):\n{}",
        violations.len(),
        MAX_UNCITED_VIOLATIONS,
        violations[violations.len().saturating_sub(5)..].join("\n")
    );
}
