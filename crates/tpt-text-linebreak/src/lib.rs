//! UAX #14 (Unicode Line Breaking Algorithm) for `tpt-text`.
//!
//! The character data is generated from the Unicode Character Database by
//! `tpt-text-linebreak-gen`; `tables.rs` records the Unicode version it was built from.
//! The rules follow UAX #14 revision 57 (Unicode 18.0.0), with no tailoring.

#![no_std]
#![deny(missing_docs, missing_debug_implementations)]

extern crate alloc;

mod rules;
mod tables;

use alloc::vec::Vec;

pub use tables::UNICODE_VERSION;

/// Unicode line breaking class (UAX #14, Table 1).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum BreakClass {
    /// `BK`: mandatory break after.
    Mandatory,
    /// `CR`: carriage return; break after unless followed by LF.
    CarriageReturn,
    /// `LF`: line feed; mandatory break after.
    LineFeed,
    /// `CM`: combining mark.
    CombiningMark,
    /// `NL`: next line; mandatory break after.
    NextLine,
    /// `SG`: surrogate; does not occur in well-formed text.
    Surrogate,
    /// `WJ`: word joiner; prohibits breaks before and after.
    WordJoiner,
    /// `ZW`: zero width space; break opportunity.
    ZeroWidthSpace,
    /// `GL`: non-breaking glue.
    NonBreakingGlue,
    /// `SP`: space.
    Space,
    /// `ZWJ`: zero width joiner.
    ZeroWidthJoiner,
    /// `B2`: break opportunity before and after.
    BeforeAndAfter,
    /// `BA`: break opportunity after.
    After,
    /// `BB`: break opportunity before.
    Before,
    /// `HY`: hyphen; break opportunity after, except in numeric context.
    Hyphen,
    /// `CB`: contingent break opportunity.
    Contingent,
    /// `CL`: closing punctuation.
    ClosePunctuation,
    /// `CP`: closing parenthesis.
    CloseParenthesis,
    /// `EX`: exclamation or interrogation.
    Exclamation,
    /// `IN`: inseparable.
    Inseparable,
    /// `NS`: non-starter.
    NonStarter,
    /// `OP`: opening punctuation.
    OpenPunctuation,
    /// `QU`: quotation.
    Quotation,
    /// `IS`: infix numeric separator.
    InfixSeparator,
    /// `NU`: numeric.
    Numeric,
    /// `PO`: postfix numeric.
    Postfix,
    /// `PR`: prefix numeric.
    Prefix,
    /// `SY`: symbols allowing a break after.
    Symbol,
    /// `AI`: ambiguous (resolved to `AL` by this crate).
    Ambiguous,
    /// `AL`: alphabetic.
    Alphabetic,
    /// `CJ`: conditional Japanese starter (resolved to `NS`).
    ConditionalJapaneseStarter,
    /// `EB`: emoji base.
    EmojiBase,
    /// `EM`: emoji modifier.
    EmojiModifier,
    /// `H2`: Hangul LV syllable.
    HangulLvSyllable,
    /// `H3`: Hangul LVT syllable.
    HangulLvtSyllable,
    /// `HL`: Hebrew letter.
    HebrewLetter,
    /// `ID`: ideographic.
    Ideographic,
    /// `JL`: Hangul leading jamo.
    HangulLJamo,
    /// `JV`: Hangul vowel jamo.
    HangulVJamo,
    /// `JT`: Hangul trailing jamo.
    HangulTJamo,
    /// `RI`: regional indicator.
    RegionalIndicator,
    /// `SA`: complex context dependent (South East Asian).
    ComplexContext,
    /// `XX`: unknown or unassigned.
    Unknown,
    /// `HH`: unambiguous hyphen (Unicode 16 and later).
    UnambiguousHyphen,
    /// `AK`: aksara (Brahmic scripts, Unicode 16 and later).
    Aksara,
    /// `AP`: aksara pre-base (Brahmic scripts, Unicode 16 and later).
    AksaraPreBase,
    /// `AS`: aksara start (Brahmic scripts, Unicode 16 and later).
    AksaraStart,
    /// `VF`: virama final (Brahmic scripts, Unicode 16 and later).
    ViramaFinal,
    /// `VI`: virama (Brahmic scripts, Unicode 16 and later).
    Virama,
}

/// Returns the line break class of `code_point`.
///
/// Values above U+10FFFF are reported as [`BreakClass::Unknown`].
pub fn class_of(code_point: u32) -> BreakClass {
    if code_point > 0x10FFFF {
        return BreakClass::Unknown;
    }
    let index = tables::CLASS_STARTS.partition_point(|&(start, _)| start <= code_point);
    // The first entry starts at U+0000, so `index` is at least 1.
    tables::CLASS_STARTS[index - 1].1
}

/// Returns the property flags of `code_point` (see `rules::props`).
pub(crate) fn props_of(code_point: u32) -> u8 {
    if code_point > 0x10FFFF {
        return rules::props::UNASSIGNED;
    }
    let index = tables::PROP_STARTS.partition_point(|&(start, _)| start <= code_point);
    tables::PROP_STARTS[index - 1].1
}

/// Break opportunity type.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum BreakOpportunity {
    /// A line must break at this spot.
    Mandatory,
    /// A line is allowed to end at this spot.
    Allowed,
}

/// Returns the line break opportunities in `text`.
///
/// Each item is `(byte index, kind)`, where the index is the start of the character
/// that follows the break. The end of text is always reported as a mandatory break.
///
/// # Examples
///
/// ```
/// use tpt_text_linebreak::{linebreaks, BreakOpportunity::{Allowed, Mandatory}};
/// assert!(linebreaks("Hello world!").eq(vec![(6, Allowed), (12, Mandatory)]));
/// ```
pub fn linebreaks(text: &str) -> alloc::vec::IntoIter<(usize, BreakOpportunity)> {
    let units = rules::units(text);
    let mut breaks: Vec<(usize, BreakOpportunity)> = Vec::new();
    for k in 0..=units.len() {
        if let Some(kind) = rules::decide(&units, k) {
            let position = units.get(k).map_or(text.len(), |unit| unit.start);
            breaks.push((position, kind));
        }
    }
    breaks.into_iter()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use BreakOpportunity::{Allowed, Mandatory};

    #[test]
    fn looks_up_known_classes() {
        assert_eq!(class_of(0x0A), BreakClass::LineFeed);
        assert_eq!(class_of(0x20), BreakClass::Space);
        assert_eq!(class_of(0x41), BreakClass::Alphabetic);
        assert_eq!(class_of(0x10FFFF), BreakClass::Unknown);
        assert_eq!(class_of(0x11_0000), BreakClass::Unknown);
    }

    #[test]
    fn breaks_after_spaces_and_at_end() {
        assert!(linebreaks("a b \nc").eq(vec![(2, Allowed), (5, Mandatory), (6, Mandatory)]));
    }

    #[test]
    fn empty_text_has_only_the_end_break() {
        assert!(linebreaks("").eq(vec![(0, Mandatory)]));
    }
}
