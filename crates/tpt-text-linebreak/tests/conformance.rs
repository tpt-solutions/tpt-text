//! UAX #14 conformance against the vendored `LineBreakTest.txt` (Unicode 18.0.0).

#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::PathBuf;

use tpt_text_linebreak::{linebreaks, UNICODE_VERSION};

#[test]
fn line_break_test_cases_all_pass() {
    assert_eq!(UNICODE_VERSION, "18.0.0");

    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/unicode/LineBreakTest.txt");
    let file = fs::read_to_string(&path).unwrap();

    let mut total = 0;
    let mut failures = Vec::new();
    for (index, line) in file.lines().enumerate() {
        let body = line.split('#').next().unwrap_or("").trim();
        if body.is_empty() {
            continue;
        }
        total += 1;

        // Tokens alternate: break marker (÷ allowed, × forbidden), then a code point.
        let mut text = String::new();
        let mut expected: Vec<usize> = Vec::new();
        for token in body.split_whitespace() {
            match token {
                "÷" => expected.push(text.len()),
                "×" => {}
                hex => text.push(char::from_u32(u32::from_str_radix(hex, 16).unwrap()).unwrap()),
            }
        }
        expected.retain(|&p| p > 0 && p < text.len());

        let actual: Vec<usize> = linebreaks(&text)
            .filter(|&(p, _)| p > 0 && p < text.len())
            .map(|(p, _)| p)
            .collect();
        if actual != expected {
            failures.push(format!(
                "line {}: {body:?} expected {expected:?} got {actual:?}",
                index + 1
            ));
        }
    }

    println!("{} of {total} cases fail", failures.len());
    for failure in failures.iter().take(20) {
        println!("  {failure}");
    }
    assert!(
        failures.is_empty(),
        "{} of {total} LineBreakTest cases fail",
        failures.len()
    );
}
