//! Hyphenation hook.
//!
//! The crate ships no dictionaries. Callers implement [`Hyphenator`] with
//! whatever pattern set they license, and the line breaker consults it for
//! word-internal break points.

use alloc::vec::Vec;

/// Supplies hyphenation points for a single word.
pub trait Hyphenator {
    /// Returns byte offsets within `word` where a hyphen break is allowed.
    ///
    /// Offsets must fall on UTF-8 character boundaries. Offsets at `0` or
    /// `word.len()` are ignored. The result need not be sorted.
    fn break_points(&self, word: &str) -> Vec<usize>;
}
