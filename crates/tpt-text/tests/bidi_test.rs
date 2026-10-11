//! UAX #9 conformance against `BidiTest.txt`.
//!
//! Each input is a list of Bidi classes. Every class is replaced by a
//! representative character, and the resolved levels and visual order are
//! compared with the file. Lines containing paragraph separators (class `B`)
//! are skipped, because the file's per-line model does not map onto the
//! paragraph splitting done by `BidiInfo`.

#![allow(clippy::unwrap_used)]

mod support;

use support::{check_against_known, data};
use unicode_bidi::{BidiInfo, Level};

/// Representative character for each Bidi class in `BidiTest.txt`.
fn representative(class: &str) -> Option<char> {
    Some(match class {
        "L" => '\u{0061}',
        "R" => '\u{05D0}',
        "AL" => '\u{0627}',
        "EN" => '\u{0030}',
        "ES" => '\u{002B}',
        "ET" => '\u{0023}',
        "AN" => '\u{0660}',
        "CS" => '\u{002C}',
        "NSM" => '\u{0300}',
        "BN" => '\u{00AD}',
        "S" => '\u{0009}',
        "WS" => '\u{0020}',
        "ON" => '\u{0021}',
        "LRE" => '\u{202A}',
        "LRO" => '\u{202D}',
        "RLE" => '\u{202B}',
        "RLO" => '\u{202E}',
        "PDF" => '\u{202C}',
        "LRI" => '\u{2066}',
        "RLI" => '\u{2067}',
        "FSI" => '\u{2068}',
        "PDI" => '\u{2069}',
        _ => return None,
    })
}

fn parse_levels(field: &str) -> Vec<Option<u8>> {
    field
        .split_whitespace()
        .map(|v| {
            if v == "x" {
                None
            } else {
                Some(v.parse().unwrap())
            }
        })
        .collect()
}

#[test]
fn uax9_bidi_test_cases() {
    let file = data("BidiTest.txt");
    let mut expected_levels: Vec<Option<u8>> = Vec::new();
    let mut expected_reorder: Vec<usize> = Vec::new();
    let mut total = 0;
    let mut skipped_paragraph_separators = 0;
    let mut level_failures = Vec::new();
    let mut reorder_failures = Vec::new();

    for (index, line) in file.lines().enumerate() {
        let line_no = index + 1;
        if let Some(rest) = line.strip_prefix("@Levels:") {
            expected_levels = parse_levels(rest);
            continue;
        }
        if let Some(rest) = line.strip_prefix("@Reorder:") {
            expected_reorder = rest
                .split_whitespace()
                .map(|v| v.parse().unwrap())
                .collect();
            continue;
        }
        let body = line.split('#').next().unwrap_or("").trim();
        if body.is_empty() {
            continue;
        }
        let Some((classes, bitset)) = body.split_once(';') else {
            continue;
        };
        total += 1;

        let classes: Vec<&str> = classes.split_whitespace().collect();
        if classes.contains(&"B") {
            skipped_paragraph_separators += 1;
            continue;
        }
        let text: String = classes
            .iter()
            .map(|c| representative(c).unwrap_or_else(|| panic!("unknown class {c}")))
            .collect();
        // Map each input element to its byte offset in the text.
        let offsets: Vec<usize> = text.char_indices().map(|(b, _)| b).collect();
        let bitset: u8 = u8::from_str_radix(bitset.trim(), 16).unwrap();

        // Bit 0: auto, bit 1: LTR, bit 2: RTL.
        for (bit, para) in [
            (1u8, None),
            (2, Some(Level::ltr())),
            (4, Some(Level::rtl())),
        ] {
            if bitset & bit == 0 {
                continue;
            }
            let bidi = BidiInfo::new(&text, para);
            let resolved = tpt_text::layout::resolved_levels(&bidi);
            let levels_for_input: Vec<u8> = offsets.iter().map(|&b| resolved[b].number()).collect();

            // Compare only the elements the file does not mark as removed.
            let mismatch = expected_levels
                .iter()
                .enumerate()
                .find_map(|(i, e)| match e {
                    Some(e) if levels_for_input.get(i) != Some(e) => {
                        Some(format!("element {i}: {:?} != {e}", levels_for_input.get(i)))
                    }
                    _ => None,
                });
            if let Some(m) = mismatch {
                level_failures.push((line_no, format!("{body} bit {bit}: {m}")));
            }

            // Visual order of the kept elements, from the expected levels.
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
            if visual != expected_reorder {
                reorder_failures.push((
                    line_no,
                    format!("{body}: expected {expected_reorder:?} got {visual:?}"),
                ));
            }
        }
    }

    println!("BidiTest: {total} lines, {skipped_paragraph_separators} skipped (contain B)");
    check_against_known("bidi-test-levels.txt", total, &level_failures);
    check_against_known("bidi-test-reorder.txt", total, &reorder_failures);
}
