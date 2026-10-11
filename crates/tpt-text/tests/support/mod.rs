//! Helpers shared by the Unicode conformance tests.

#![allow(dead_code, clippy::unwrap_used)]

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

pub fn unicode_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/unicode")
}

pub fn data(name: &str) -> String {
    let path = unicode_dir().join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// Compares this run's failures with the recorded known failures.
///
/// `failures` holds `(1-based line number, description)`.
pub fn check_against_known(name: &str, total: usize, failures: &[(usize, String)]) {
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
    for (line, msg) in failures.iter().take(5) {
        println!("  line {line}: {msg}");
    }
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
