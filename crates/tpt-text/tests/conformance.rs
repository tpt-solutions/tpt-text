//! Unicode conformance: UAX #14 (line breaks) and UAX #9 (Bidi) against the
//! official test files in `testdata/unicode`.
//!
//! The vendored files are Unicode 18.0.0. `unicode-linebreak` 0.1.5 implements
//! Unicode 15.0.0, so some UAX #14 cases fail for reasons that are outside
//! this crate. Known failures are recorded per line in
//! `testdata/unicode/known-failures/`. The test fails on any failure that is
//! not in that list, so regressions still show. Regenerate the lists with
//! `TPT_CONFORMANCE_UPDATE=1 cargo test -p tpt-text --test conformance`.

#![allow(clippy::unwrap_used)]

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use unicode_bidi::{BidiInfo, Level};
use unicode_linebreak::{linebreaks, BreakOpportunity};

fn unicode_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/unicode")
}

fn data(name: &str) -> String {
    let path = unicode_dir().join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// Compares this run's failures with the recorded known failures.
///
/// `failures` holds `(1-based line number, description)`.
fn check_against_known(name: &str, total: usize, failures: &[(usize, String)]) {
    let known_path = unicode_dir().join("known-failures").join(name);
    let update = std::env::var("TPT_CONFORMANCE_UPDATE").is_ok_and(|v| v == "1");
    if update {
        fs::create_dir_all(known_path.parent().unwrap()).unwrap();
        let mut body = String::new();
        for (line, _) in failures {
            body.push_str(&format!("{line}\n"));
        }
        fs::write(&known_path, body).unwrap();
        println!("{name}: recorded {} known failures", failures.len());
        return;
    }

    let known: BTreeSet<usize> = fs::read_to_string(&known_path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.trim().parse().ok())
        .collect();

    println!("{name}: {} of {total} cases fail", failures.len());
    let new: Vec<&(usize, String)> = failures
        .iter()
        .filter(|(line, _)| !known.contains(line))
        .collect();
    for (line, msg) in new.iter().take(20) {
        println!("  NEW line {line}: {msg}");
    }
    assert!(
        new.is_empty(),
        "{} conformance failures not in {name} known list",
        new.len()
    );
}

#[test]
fn uax14_line_break_cases() {
    let file = data("LineBreakTest.txt");
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
                hex => {
                    let cp = u32::from_str_radix(hex, 16).unwrap();
                    text.push(char::from_u32(cp).unwrap());
                }
            }
        }
        expected.retain(|&p| p > 0 && p < text.len());

        let actual: Vec<usize> = linebreaks(&text)
            .filter(|&(p, _)| p > 0 && p < text.len())
            .map(|(p, _)| p)
            .collect();

        if actual != expected {
            failures.push((
                index + 1,
                format!("{body:?} expected {expected:?} got {actual:?}"),
            ));
        }
    }

    check_against_known("uax14.txt", total, &failures);
}

#[test]
fn uax9_levels_and_reorder() {
    let file = data("BidiCharacterTest.txt");
    let mut total = 0;
    let mut level_failures = Vec::new();
    let mut reorder_failures = Vec::new();

    for (index, line) in file.lines().enumerate() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split(';').collect();
        if fields.len() < 5 {
            continue;
        }
        total += 1;
        let line_no = index + 1;

        let expected_levels: Vec<Option<u8>> = fields[3]
            .split_whitespace()
            .map(|v| {
                if v == "x" {
                    None
                } else {
                    Some(v.parse().unwrap())
                }
            })
            .collect();
        let expected_order: Vec<usize> = fields[4]
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();

        // Reorder (UAX #9 L2) on the expected levels, so this checks the
        // reordering step on its own.
        let kept: Vec<(usize, Level)> = expected_levels
            .iter()
            .enumerate()
            .filter_map(|(i, l)| l.map(|n| (i, Level::new(n).unwrap())))
            .collect();
        let levels: Vec<Level> = kept.iter().map(|(_, l)| *l).collect();
        let visual: Vec<usize> = BidiInfo::reorder_visual(&levels)
            .into_iter()
            .map(|k| kept[k].0)
            .collect();
        if visual != expected_order {
            reorder_failures.push((
                line_no,
                format!("expected {expected_order:?} got {visual:?}"),
            ));
        }

        // Resolution (UAX #9 P/X/W/N/I rules), checked against the expected
        // paragraph level and per-character levels.
        let text: String = fields[0]
            .split_whitespace()
            .map(|h| char::from_u32(u32::from_str_radix(h, 16).unwrap()).unwrap())
            .collect();
        let para_level = match fields[1].trim() {
            "0" => Some(Level::ltr()),
            "1" => Some(Level::rtl()),
            _ => None,
        };
        let expected_para: u8 = fields[2].trim().parse().unwrap();
        let bidi = BidiInfo::new(&text, para_level);
        if bidi.paragraphs.len() != 1 {
            continue;
        }
        let mut problem = None;
        if bidi.paragraphs[0].level.number() != expected_para {
            problem = Some(format!(
                "paragraph level {} != {expected_para}",
                bidi.paragraphs[0].level.number()
            ));
        }
        for (i, (byte, _)) in text.char_indices().enumerate() {
            if let Some(Some(e)) = expected_levels.get(i) {
                let got = bidi.levels[byte].number();
                if got != *e {
                    problem = Some(format!("char {i} level {got} != {e}"));
                    break;
                }
            }
        }
        if let Some(p) = problem {
            level_failures.push((line_no, p));
        }
    }

    check_against_known("uax9-levels.txt", total, &level_failures);
    check_against_known("uax9-reorder.txt", total, &reorder_failures);
}

#[test]
fn break_opportunity_kinds_are_present() {
    // Sanity check on the kinds the layout engine depends on.
    let kinds: Vec<BreakOpportunity> = linebreaks("a b\nc").map(|(_, k)| k).collect();
    assert!(kinds.contains(&BreakOpportunity::Allowed));
    assert!(kinds.contains(&BreakOpportunity::Mandatory));
}
