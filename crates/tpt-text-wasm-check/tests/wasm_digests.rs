//! Native digests must equal the digests recorded from the WebAssembly build.
//!
//! Regenerate the recorded file with:
//!   cargo build -p tpt-text-wasm-check --target wasm32-unknown-unknown --release
//!   node scripts/wasm-digest.mjs target/wasm32-unknown-unknown/release/tpt_text_wasm_check.wasm crates/tpt-text-wasm-check/golden/digests.json

#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;

use tpt_text_wasm_check::{digest_case, CASES};

const LATIN: &[u8] = include_bytes!("../../tpt-text/tests/fonts/NotoSans-Regular.ttf");
const ARABIC: &[u8] = include_bytes!("../../tpt-text/tests/fonts/NotoSansArabic-Regular.ttf");

#[test]
fn native_digests_match_wasm_digests() {
    let recorded: BTreeMap<String, String> = serde_json::from_str(
        &std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/golden/digests.json"))
            .unwrap(),
    )
    .unwrap();

    let names: Vec<&str> = CASES.iter().map(|c| c.name).collect();
    let recorded_names: Vec<&str> = recorded.keys().map(String::as_str).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(
        sorted, recorded_names,
        "case names drifted from the recorded file"
    );

    for case in CASES {
        let digest = digest_case(LATIN, ARABIC, case).unwrap();
        let expected = &recorded[case.name];
        assert_eq!(
            format!("{digest:016x}"),
            *expected,
            "{}: native layout differs from WebAssembly",
            case.name
        );
    }
}
