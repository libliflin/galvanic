//! Throughput benchmarks for galvanic's lexer, parser, and full pipeline.
//!
//! Run with: cargo bench
//! Results land in target/criterion/ with HTML reports.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use galvanic::codegen;
use galvanic::lexer;
use galvanic::lower;
use galvanic::parser;

/// FLS §2.4.4.1 — integer literals in all bases with suffixes
const FIXTURE_LITERALS: &str = include_str!("../tests/fixtures/fls_2_4_literals.rs");

/// FLS §9 — function definitions with expressions
const FIXTURE_FUNCTIONS: &str = include_str!("../tests/fixtures/fls_9_functions.rs");

/// FLS §6 — expressions: arithmetic, comparison, if-else, blocks
const FIXTURE_EXPRESSIONS: &str = include_str!("../tests/fixtures/fls_6_expressions.rs");

/// Synthetic stress: many let bindings in a function body
fn stress_let_bindings(n: usize) -> String {
    let mut s = String::from("fn main() {\n");
    for i in 0..n {
        s.push_str(&format!("    let x{i}: u32 = {i};\n"));
    }
    s.push_str("}\n");
    s
}

fn bench_lexer(c: &mut Criterion) {
    let mut group = c.benchmark_group("lexer");

    for (name, src) in [
        ("fls_literals", FIXTURE_LITERALS),
        ("fls_functions", FIXTURE_FUNCTIONS),
        ("fls_expressions", FIXTURE_EXPRESSIONS),
    ] {
        group.throughput(Throughput::Bytes(src.len() as u64));
        group.bench_with_input(BenchmarkId::new("tokenize", name), src, |b, src| {
            b.iter(|| lexer::tokenize(black_box(src)).unwrap());
        });
    }

    // Stress test: scaling behavior
    for n in [100, 1_000, 10_000] {
        let src = stress_let_bindings(n);
        group.throughput(Throughput::Bytes(src.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("tokenize_stress", n),
            &src,
            |b, src| {
                b.iter(|| lexer::tokenize(black_box(src)).unwrap());
            },
        );
    }

    group.finish();
}

fn bench_parser(c: &mut Criterion) {
    let mut group = c.benchmark_group("parser");

    for (name, src) in [
        ("fls_functions", FIXTURE_FUNCTIONS),
        ("fls_expressions", FIXTURE_EXPRESSIONS),
    ] {
        let tokens = lexer::tokenize(src).unwrap();
        group.throughput(Throughput::Bytes(src.len() as u64));
        group.bench_with_input(BenchmarkId::new("parse", name), &tokens, |b, tokens| {
            b.iter(|| parser::parse(black_box(tokens), black_box(src)).unwrap());
        });
    }

    // Stress test: scaling behavior
    for n in [100, 1_000, 10_000] {
        let src = stress_let_bindings(n);
        let tokens = lexer::tokenize(&src).unwrap();
        group.throughput(Throughput::Bytes(src.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("parse_stress", n),
            &tokens,
            |b, tokens| {
                b.iter(|| parser::parse(black_box(tokens), black_box(&src)).unwrap());
            },
        );
    }

    group.finish();
}

fn bench_end_to_end(c: &mut Criterion) {
    let mut group = c.benchmark_group("end_to_end");

    for (name, src) in [
        ("fls_functions", FIXTURE_FUNCTIONS),
        ("fls_expressions", FIXTURE_EXPRESSIONS),
    ] {
        group.throughput(Throughput::Bytes(src.len() as u64));
        group.bench_with_input(BenchmarkId::new("lex_and_parse", name), src, |b, src| {
            b.iter(|| {
                let tokens = lexer::tokenize(black_box(src)).unwrap();
                parser::parse(black_box(&tokens), black_box(src)).unwrap()
            });
        });

        // full_pipeline: lex → parse → lower → emit_asm
        // This is the benchmark that closes the claim→code→test→benchmark chain
        // for Instr (80 bytes, cache-line-spanning) — the codegen stage is now measured.
        group.bench_with_input(BenchmarkId::new("full_pipeline", name), src, |b, src| {
            b.iter(|| {
                let tokens = lexer::tokenize(black_box(src)).unwrap();
                let ast = parser::parse(black_box(&tokens), black_box(src)).unwrap();
                let module = lower::lower(black_box(&ast), black_box(src)).unwrap();
                codegen::emit_asm(black_box(&module)).unwrap()
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_lexer, bench_parser, bench_end_to_end);
criterion_main!(benches);
