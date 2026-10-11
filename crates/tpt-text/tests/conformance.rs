//! Unicode conformance: UAX #14 (line breaks) and UAX #9 (Bidi) against the
//! official test files in `testdata/unicode` (Unicode 18.0.0).
//!
//! UAX #14 cases run against `tpt-text-linebreak`, which must pass them all. UAX #9
//! failures are recorded per line in `testdata/unicode/known-failures/` and the
//! test fails on any failure not in that list. Regenerate the lists with
//! `TPT_CONFORMANCE_UPDATE=1 cargo test -p tpt-text --test conformance`.

#![allow(clippy::unwrap_used)]

mod support;

use support::{check_against_known, data};

use tpt_text_linebreak::{linebreaks, BreakOpportunity};
use unicode_bidi::{BidiInfo, Level};

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
        let resolved = tpt_text::layout::resolved_levels(&bidi);
        for (i, (byte, _)) in text.char_indices().enumerate() {
            if let Some(Some(e)) = expected_levels.get(i) {
                let got = resolved[byte].number();
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
