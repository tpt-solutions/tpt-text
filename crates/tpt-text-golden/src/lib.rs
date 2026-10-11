//! JSON golden-file helper for tpt-text.
//!
//! [`check`] compares a serialisable value against a checked-in JSON file.
//! Running with `TPT_GOLDEN_UPDATE=1` rewrites the file instead of failing,
//! so intentional output changes are reviewed as a git diff.

use std::fs;
use std::path::Path;

use serde::Serialize;

/// Environment variable that switches [`check`] into update mode.
pub const UPDATE_ENV: &str = "TPT_GOLDEN_UPDATE";

/// Compares `value` (pretty JSON, trailing newline) against the file at `path`.
///
/// # Panics
///
/// Panics when the file is missing or differs and update mode is off, or when
/// the value cannot be serialised. Panicking is intentional: this is test
/// support code.
pub fn check<T: Serialize>(path: impl AsRef<Path>, value: &T) {
    let path = path.as_ref();
    let mut actual = serde_json::to_string_pretty(value).expect("golden value serialises");
    actual.push('\n');

    if std::env::var_os(UPDATE_ENV).is_some_and(|v| v == "1") {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create golden directory");
        }
        fs::write(path, &actual).expect("write golden file");
        return;
    }

    // Git on Windows may check golden files out with CRLF (core.autocrlf), so
    // compare with line endings normalised to LF.
    let expected = fs::read_to_string(path)
        .unwrap_or_else(|_| {
            panic!(
                "missing golden file {}; run with {UPDATE_ENV}=1 to create it",
                path.display()
            )
        })
        .replace("\r\n", "\n");
    assert!(
        expected == actual,
        "golden mismatch for {}; run with {UPDATE_ENV}=1 to accept the change",
        path.display()
    );
}
