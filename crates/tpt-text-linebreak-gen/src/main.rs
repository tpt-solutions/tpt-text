//! Generates `crates/tpt-text-linebreak/src/tables.rs` from Unicode Character Database files.
//!
//! Usage: `tpt-text-linebreak-gen <ucd-dir> <unicode-version> <output.rs>`
//!
//! `<ucd-dir>` must contain `LineBreak.txt`, `UnicodeData.txt`,
//! `EastAsianWidth.txt` and `emoji-data.txt`.
//!
//! The output holds two sorted tables of `(first code point, value)` pairs. Each
//! value applies from its first code point up to the next entry.

use std::{env, fs, path::Path, process};

/// Maps each UAX #14 abbreviation to its `BreakClass` variant name.
const CLASSES: &[(&str, &str)] = &[
    ("AI", "Ambiguous"),
    ("AK", "Aksara"),
    ("AL", "Alphabetic"),
    ("AP", "AksaraPreBase"),
    ("AS", "AksaraStart"),
    ("B2", "BeforeAndAfter"),
    ("BA", "After"),
    ("BB", "Before"),
    ("BK", "Mandatory"),
    ("CB", "Contingent"),
    ("CJ", "ConditionalJapaneseStarter"),
    ("CL", "ClosePunctuation"),
    ("CM", "CombiningMark"),
    ("CP", "CloseParenthesis"),
    ("CR", "CarriageReturn"),
    ("EB", "EmojiBase"),
    ("EM", "EmojiModifier"),
    ("EX", "Exclamation"),
    ("GL", "NonBreakingGlue"),
    ("H2", "HangulLvSyllable"),
    ("H3", "HangulLvtSyllable"),
    ("HH", "UnambiguousHyphen"),
    ("HL", "HebrewLetter"),
    ("HY", "Hyphen"),
    ("ID", "Ideographic"),
    ("IN", "Inseparable"),
    ("IS", "InfixSeparator"),
    ("JL", "HangulLJamo"),
    ("JT", "HangulTJamo"),
    ("JV", "HangulVJamo"),
    ("LF", "LineFeed"),
    ("NL", "NextLine"),
    ("NS", "NonStarter"),
    ("NU", "Numeric"),
    ("OP", "OpenPunctuation"),
    ("PO", "Postfix"),
    ("PR", "Prefix"),
    ("QU", "Quotation"),
    ("RI", "RegionalIndicator"),
    ("SA", "ComplexContext"),
    ("SG", "Surrogate"),
    ("SP", "Space"),
    ("SY", "Symbol"),
    ("VF", "ViramaFinal"),
    ("VI", "Virama"),
    ("WJ", "WordJoiner"),
    ("XX", "Unknown"),
    ("ZW", "ZeroWidthSpace"),
    ("ZWJ", "ZeroWidthJoiner"),
];

/// Highest code point in Unicode.
const MAX_CODE_POINT: u32 = 0x10FFFF;

/// Property flags. These must match the constants in `tpt-text-linebreak`'s `props` module.
const FLAG_PI: u8 = 1 << 0;
const FLAG_PF: u8 = 1 << 1;
const FLAG_MARK: u8 = 1 << 2;
const FLAG_UNASSIGNED: u8 = 1 << 3;
const FLAG_EAST_ASIAN_WIDE: u8 = 1 << 4;
const FLAG_EXTENDED_PICTOGRAPHIC: u8 = 1 << 5;

fn die(message: &str) -> ! {
    eprintln!("tpt-text-linebreak-gen: {message}");
    process::exit(1)
}

fn variant(abbreviation: &str) -> &'static str {
    CLASSES
        .iter()
        .find(|(abbr, _)| *abbr == abbreviation)
        .map(|(_, name)| *name)
        .unwrap_or_else(|| die(&format!("unknown line break class {abbreviation:?}")))
}

/// Parses a hexadecimal code point, rejecting values past the last Unicode code point.
fn parse_hex(hex: &str) -> u32 {
    let cp = u32::from_str_radix(hex.trim(), 16)
        .unwrap_or_else(|_| die(&format!("bad code point {hex:?}")));
    if cp > MAX_CODE_POINT {
        die(&format!("code point {hex:?} is past U+10FFFF"));
    }
    cp
}

/// Parses `XXXX` or `XXXX..YYYY`, which must be non-empty and in order.
fn parse_range(field: &str) -> (u32, u32) {
    let (first, last) = match field.trim().split_once("..") {
        Some((first, last)) => (parse_hex(first), parse_hex(last)),
        None => {
            let cp = parse_hex(field);
            (cp, cp)
        }
    };
    if first > last {
        die(&format!("range {field:?} ends before it starts"));
    }
    (first, last)
}

fn read(dir: &Path, name: &str) -> String {
    let path = dir.join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| die(&format!("reading {}: {e}", path.display())))
}

/// Fills `table` from a UCD file with `range; value` data lines and `# @missing` defaults.
/// Defaults are applied first so explicit lines override them.
fn fill_from_ucd<'a>(text: &'a str, table: &mut [&'a str]) {
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("# @missing:") {
            let (range, value) = rest
                .split_once(';')
                .unwrap_or_else(|| die("bad @missing line"));
            let (first, last) = parse_range(range);
            for cp in first..=last {
                table[cp as usize] = value.trim();
            }
        }
    }
    for line in text.lines() {
        let body = line.split('#').next().unwrap_or("").trim();
        if body.is_empty() {
            continue;
        }
        let (range, value) = body.split_once(';').unwrap_or_else(|| die("bad data line"));
        let (first, last) = parse_range(range);
        for cp in first..=last {
            table[cp as usize] = value.trim();
        }
    }
}

/// Marks assigned code points (clearing `FLAG_UNASSIGNED`) and records general
/// category flags from `UnicodeData.txt`. Ranges use the `<..., First>` / `<..., Last>` form.
fn apply_unicode_data(text: &str, flags: &mut [u8]) {
    let mut range_start: Option<u32> = None;
    for line in text.lines() {
        let fields: Vec<&str> = line.split(';').collect();
        if fields.len() < 3 {
            continue;
        }
        let cp = parse_hex(fields[0]);
        let name = fields[1];
        let first = if name.ends_with(", First>") {
            range_start = Some(cp);
            continue;
        } else if name.ends_with(", Last>") {
            range_start
                .take()
                .unwrap_or_else(|| die("range end without start"))
        } else {
            cp
        };
        if first > cp {
            die(&format!(
                "range starting at {first:04X} ends before it starts"
            ));
        }
        let category_flags = match fields[2] {
            "Pi" => FLAG_PI,
            "Pf" => FLAG_PF,
            "Mn" | "Mc" => FLAG_MARK,
            _ => 0,
        };
        for c in first..=cp {
            flags[c as usize] = (flags[c as usize] & !FLAG_UNASSIGNED) | category_flags;
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        eprintln!("usage: tpt-text-linebreak-gen <ucd-dir> <unicode-version> <output.rs>");
        process::exit(2);
    }
    let dir = Path::new(&args[1]);
    let version = &args[2];
    let size = MAX_CODE_POINT as usize + 1;

    let line_break = read(dir, "LineBreak.txt");
    let mut classes: Vec<&str> = vec!["XX"; size];
    fill_from_ucd(&line_break, &mut classes);

    let east_asian_width = read(dir, "EastAsianWidth.txt");
    let mut widths: Vec<&str> = vec!["N"; size];
    fill_from_ucd(&east_asian_width, &mut widths);

    let unicode_data = read(dir, "UnicodeData.txt");
    let mut flags = vec![FLAG_UNASSIGNED; size];
    apply_unicode_data(&unicode_data, &mut flags);
    for (cp, width) in widths.iter().enumerate() {
        if matches!(*width, "F" | "W" | "H") {
            flags[cp] |= FLAG_EAST_ASIAN_WIDE;
        }
    }

    let emoji = read(dir, "emoji-data.txt");
    for line in emoji.lines() {
        let body = line.split('#').next().unwrap_or("").trim();
        if let Some((range, property)) = body.split_once(';') {
            if property.trim() == "Extended_Pictographic" {
                let (first, last) = parse_range(range);
                for cp in first..=last {
                    flags[cp as usize] |= FLAG_EXTENDED_PICTOGRAPHIC;
                }
            }
        }
    }

    let mut out = String::new();
    out.push_str(&format!(
        "// @generated by tpt-text-linebreak-gen from the Unicode {version} UCD. Do not edit.\n\n"
    ));
    out.push_str("/// Unicode version the tables were generated from.\n");
    out.push_str(&format!(
        "pub const UNICODE_VERSION: &str = \"{version}\";\n\n"
    ));
    out.push_str("/// `(first code point, class)` pairs, sorted by code point.\n");
    out.push_str("pub(crate) static CLASS_STARTS: &[(u32, crate::BreakClass)] = &[\n");
    out.push_str(&runs(&classes, |abbr| {
        format!("crate::BreakClass::{}", variant(abbr))
    }));
    out.push_str("];\n\n");
    out.push_str("/// `(first code point, property flags)` pairs, sorted by code point.\n");
    out.push_str("pub(crate) static PROP_STARTS: &[(u32, u8)] = &[\n");
    out.push_str(&runs(&flags, |f| format!("0x{f:02X}")));
    out.push_str("];\n");

    fs::write(&args[3], out).unwrap_or_else(|e| die(&format!("writing {}: {e}", args[3])));
}

/// Renders one `(first code point, value)` line per run of equal values.
fn runs<T: PartialEq + Copy>(values: &[T], render: impl Fn(T) -> String) -> String {
    let mut out = String::new();
    let mut previous: Option<T> = None;
    for (cp, &value) in values.iter().enumerate() {
        if previous != Some(value) {
            out.push_str(&format!("    (0x{cp:04X}, {}),\n", render(value)));
            previous = Some(value);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_code_points_and_ranges() {
        assert_eq!(parse_hex("0041"), 0x41);
        assert_eq!(parse_range("0041"), (0x41, 0x41));
        assert_eq!(parse_range("0041..005A"), (0x41, 0x5A));
        assert_eq!(parse_range(" 10FFFF "), (0x10FFFF, 0x10FFFF));
    }

    #[test]
    fn maps_class_abbreviations_to_variants() {
        assert_eq!(variant("AL"), "Alphabetic");
        assert_eq!(variant("BK"), "Mandatory");
    }

    #[test]
    fn fills_defaults_and_then_explicit_lines() {
        let ucd = "# @missing: 0000..0043; XX\n0041..0042 ; AL # letters\n";
        let mut table: Vec<&str> = vec![""; 0x44];
        fill_from_ucd(ucd, &mut table);
        assert_eq!(table[0x41], "AL");
        assert_eq!(table[0x42], "AL");
        assert_eq!(table[0x43], "XX");
    }
}
