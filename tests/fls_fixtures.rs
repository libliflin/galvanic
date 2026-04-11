//! Integration tests using FLS-derived fixture programs.
//!
//! Each fixture is a real Rust program with examples drawn from the
//! Ferrocene Language Specification. These tests verify galvanic can
//! lex and parse them. When codegen exists, they should also compile
//! and run the output.

/// Lex and parse a fixture file through the galvanic library and assert no errors.
///
/// Tests only the lexer and parser — not lowering or codegen. Some fixture files
/// contain FLS examples that galvanic can parse but not yet lower; those are still
/// valid parse-acceptance tests. Use `compile_and_run` in `e2e.rs` for full-pipeline
/// tests.
fn assert_galvanic_accepts(fixture: &str) {
    let fixture_path = format!(
        "{}/tests/fixtures/{fixture}",
        env!("CARGO_MANIFEST_DIR")
    );

    let source = std::fs::read_to_string(&fixture_path)
        .unwrap_or_else(|e| panic!("could not read {fixture}: {e}"));

    let tokens = galvanic::lexer::tokenize(&source)
        .unwrap_or_else(|e| panic!("galvanic lexer rejected {fixture}: {e}"));

    galvanic::parser::parse(&tokens, &source)
        .unwrap_or_else(|e| panic!("galvanic parser rejected {fixture}: {e}"));
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

#[test]
fn fls_12_1_generic_enums() {
    assert_galvanic_accepts("fls_12_1_generic_enums.rs");
}

#[test]
fn fls_12_1_generic_trait_impl() {
    assert_galvanic_accepts("fls_12_1_generic_trait_impl.rs");
}

#[test]
fn fls_12_1_trait_bounds() {
    assert_galvanic_accepts("fls_12_1_trait_bounds.rs");
}

#[test]
fn fls_4_14_where_clauses() {
    assert_galvanic_accepts("fls_4_14_where_clauses.rs");
}

#[test]
fn fls_4_14_where_clauses_on_types() {
    assert_galvanic_accepts("fls_4_14_where_clauses_on_types.rs");
}

#[test]
fn fls_11_impl_trait() {
    assert_galvanic_accepts("fls_11_impl_trait.rs");
}

#[test]
fn fls_4_13_dyn_trait() {
    assert_galvanic_accepts("fls_4_13_dyn_trait.rs");
}

#[test]
fn fls_10_2_assoc_type_bounds() {
    assert_galvanic_accepts("fls_10_2_assoc_type_bounds.rs");
}

#[test]
fn fls_8_1_let_else() {
    assert_galvanic_accepts("fls_8_1_let_else.rs");
}

#[test]
fn fls_4_14_supertrait_bounds() {
    assert_galvanic_accepts("fls_4_14_supertrait_bounds.rs");
}

#[test]
fn fls_10_2_where_clause_proj() {
    assert_galvanic_accepts("fls_10_2_where_clause_proj.rs");
}

#[test]
fn fls_19_unsafe_fn() {
    assert_galvanic_accepts("fls_19_unsafe_fn.rs");
}

#[test]
fn fls_19_unsafe_trait() {
    assert_galvanic_accepts("fls_19_unsafe_trait.rs");
}

#[test]
fn fls_6_23_overflow() {
    assert_galvanic_accepts("fls_6_23_overflow.rs");
}

#[test]
fn fls_4_9_slices() {
    assert_galvanic_accepts("fls_4_9_slices.rs");
}

#[test]
fn fls_6_23_div_zero() {
    assert_galvanic_accepts("fls_6_23_div_zero.rs");
}

#[test]
fn fls_6_15_loop_expressions() {
    assert_galvanic_accepts("fls_6_15_loop_expressions.rs");
}

#[test]
fn fls_6_17_if_expressions() {
    assert_galvanic_accepts("fls_6_17_if_expressions.rs");
}

#[test]
fn fls_6_18_match_expressions() {
    assert_galvanic_accepts("fls_6_18_match_expressions.rs");
}

#[test]
fn fls_6_19_return_expressions() {
    assert_galvanic_accepts("fls_6_19_return_expressions.rs");
}

#[test]
fn fls_6_5_operator_expressions() {
    assert_galvanic_accepts("fls_6_5_operator_expressions.rs");
}

#[test]
fn fls_6_16_range_expressions() {
    assert_galvanic_accepts("fls_6_16_range_expressions.rs");
}

#[test]
fn fls_6_3_path_expressions() {
    assert_galvanic_accepts("fls_6_3_path_expressions.rs");
}

#[test]
fn fls_6_11_struct_expressions() {
    assert_galvanic_accepts("fls_6_11_struct_expressions.rs");
}

#[test]
fn fls_6_14_closure_expressions() {
    assert_galvanic_accepts("fls_6_14_closure_expressions.rs");
}
