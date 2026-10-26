use std::{
    hint::black_box,
    io::{self, BufRead},
    time::Instant,
};

use ruff_python_parser::Mode;
use serde_json::{Value, json};
use spam::{Checker, Limits};

const LIMIT: u16 = 200;

fn minimum_passing_limit(mut rejects: impl FnMut(u16) -> bool) -> Option<u16> {
    // Cap probes so the recursive AST visitor cannot exhaust the process stack.
    if rejects(512) {
        return None;
    }
    let (mut low, mut high) = (0, 512);
    while low < high {
        let mid = low + (high - low) / 2;
        if rejects(mid) {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    Some(low)
}

fn elapsed_ns(mut operation: impl FnMut(), iterations: u32) -> u128 {
    let start = Instant::now();
    for _ in 0..iterations {
        operation();
    }
    start.elapsed().as_nanos() / u128::from(iterations)
}

fn main() {
    let mut parser = Checker::new(Limits {
        max_nesting_depth: Some(usize::from(LIMIT)),
        max_annotation_depth: Some(8),
        ..Limits::default()
    });
    let mut typing = Checker::new(Limits {
        max_nesting_depth: Some(usize::from(LIMIT)),
        max_annotation_depth: Some(8),
        max_ast_depth: Some(usize::from(LIMIT)),
        ..Limits::default()
    });
    let mut measurement = Checker::new(Limits::default());
    let mut adjusted_typing = Checker::new(Limits {
        max_nesting_depth: Some(usize::from(LIMIT)),
        max_annotation_depth: Some(8),
        max_ast_depth: Some(usize::from(LIMIT - 1)),
        ..Limits::default()
    });

    for line in io::stdin().lock().lines() {
        let input: Value = serde_json::from_str(&line.unwrap()).unwrap();
        let source = input["source"].as_str().unwrap();
        let threshold = input["threshold"].as_u64().unwrap_or(4096) as usize;
        let iterations = input["iterations"].as_u64().unwrap_or(5) as u32;
        let iterations = iterations.max(1);
        let scan_rejects = |limit| source_nesting::nesting_bound_exceeded(source, Mode::Module, limit).is_some();
        let parser_depth = minimum_passing_limit(scan_rejects);
        let parser_ok = source.len() <= threshold || !scan_rejects(LIMIT);
        // Never parse a source the existing forced scan rejects, even if the
        // production length shortcut would skip that scan.
        let safe_to_parse = !scan_rejects(LIMIT);
        let spam_parser = if source.len() <= threshold {
            Ok(())
        } else {
            parser.check(source)
        };
        let spam_type = typing.check(source);
        let adjusted_type = adjusted_typing.check(source);
        eprintln!(
            "spam: {}",
            json!({"parser_ok": spam_parser.is_ok(), "type_ok": spam_type.is_ok(),
            "type_error": spam_type.as_ref().err().map(|e| format!("{:?}", e.reason))})
        );
        eprintln!("phase: production AST guard at limit 200");
        let type_ok = if !parser_ok {
            Some(false)
        } else if safe_to_parse {
            Some(source_nesting::ast_depth_exceeded(source, LIMIT).is_none())
        } else {
            None
        };
        eprintln!("phase: score probes up to 512");
        let ast_depth = if safe_to_parse {
            minimum_passing_limit(|limit| source_nesting::ast_depth_exceeded(source, limit).is_some())
        } else {
            None
        };
        eprintln!("phase: unlimited Tokspam measurement");
        let report = measurement.analyze(source);
        eprintln!("phase: timing");
        let old_parser_ns = elapsed_ns(
            || {
                black_box(source_nesting::source_within_nesting_bound(
                    black_box(source),
                    threshold,
                ));
            },
            iterations,
        );
        let spam_parser_ns = elapsed_ns(
            || {
                if source.len() > threshold {
                    black_box(parser.check(black_box(source))).ok();
                }
            },
            iterations,
        );
        let old_type_ns = type_ok.map(|_| {
            elapsed_ns(
                || {
                    if source_nesting::source_within_nesting_bound(source, threshold) {
                        black_box(source_nesting::ast_depth_exceeded(black_box(source), LIMIT));
                    }
                },
                iterations,
            )
        });
        let spam_type_ns = elapsed_ns(
            || {
                black_box(typing.check(black_box(source))).ok();
            },
            iterations,
        );

        println!(
            "{}",
            json!({
                "name": input["name"], "bytes": source.len(), "threshold": threshold,
                "monty_parser_depth": parser_depth, "monty_ast_depth": ast_depth,
                "spam_parser_depth": report.scores.parser_depth, "spam_ast_depth": report.scores.ast_depth,
                "spam_stopped": report.stopped.map(|r| format!("{:?}", r.reason)),
                "monty_parser_ok": parser_ok, "monty_type_ok": type_ok,
                "spam_parser_ok": spam_parser.is_ok(), "spam_type_ok": spam_type.is_ok(),
                "spam_adjusted_type_ok": adjusted_type.is_ok(),
                "spam_parser_error": spam_parser.err().map(|e| format!("{:?}", e.reason)),
                "spam_type_error": spam_type.err().map(|e| format!("{:?}", e.reason)),
                "monty_parser_ns": old_parser_ns, "spam_parser_ns": spam_parser_ns,
                "monty_type_ns": old_type_ns, "spam_type_ns": spam_type_ns,
            })
        );
    }
}
