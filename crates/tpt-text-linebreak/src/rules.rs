//! The UAX #14 line breaking rules, applied to a sequence of resolved units.
//!
//! A unit is a base character plus any combining marks or joiners that LB9 attaches
//! to it. Each rule is checked in the order of revision 57 of UAX #14, and the first
//! rule that matches decides the boundary.

use alloc::vec::Vec;

use crate::{class_of, props_of, BreakClass as C, BreakOpportunity};

/// Property flags stored per code point. These must match `tpt-text-linebreak-gen`.
pub(crate) mod props {
    /// General category `Pi` (initial quotation mark).
    pub(crate) const PI: u8 = 1 << 0;
    /// General category `Pf` (final quotation mark).
    pub(crate) const PF: u8 = 1 << 1;
    /// General category `Mn` or `Mc` (combining mark).
    pub(crate) const MARK: u8 = 1 << 2;
    /// Unassigned code point (general category `Cn`).
    pub(crate) const UNASSIGNED: u8 = 1 << 3;
    /// East_Asian_Width is `F`, `W` or `H`.
    pub(crate) const EAST_ASIAN_WIDE: u8 = 1 << 4;
    /// Extended_Pictographic.
    pub(crate) const EXTENDED_PICTOGRAPHIC: u8 = 1 << 5;
}

/// One unit of the line breaking sequence.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Unit {
    /// Class after LB1 and LB10 resolution.
    pub(crate) class: C,
    /// Byte index where the unit's base character starts.
    pub(crate) start: usize,
    /// Code point of the base character.
    pub(crate) code_point: u32,
    /// Property flags of the base character (`props`).
    pub(crate) flags: u8,
    /// The last character attached to this unit is a ZWJ (LB8a).
    pub(crate) ends_with_zwj: bool,
    /// Number of consecutive regional indicator units ending at this unit (LB30a).
    pub(crate) regional_indicator_run: usize,
}

/// Outcome of the rules for one boundary.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Verdict {
    /// A break is required or allowed at the boundary.
    Break(BreakOpportunity, &'static str),
    /// The boundary is not a break.
    NoBreak(&'static str),
    /// No rule decided; the next rule applies.
    Continue,
}

/// Splits `text` into units, applying LB1 (resolution), LB9 (attach marks) and
/// LB10 (remaining marks become `AL`).
pub(crate) fn units(text: &str) -> Vec<Unit> {
    let mut units: Vec<Unit> = Vec::new();
    for (start, ch) in text.char_indices() {
        let code_point = ch as u32;
        let flags = props_of(code_point);
        let class = resolve(class_of(code_point), flags);
        let is_joiner_or_mark = matches!(class, C::CombiningMark | C::ZeroWidthJoiner);

        // LB9: a mark or joiner attaches to a base other than BK, CR, LF, NL, SP or ZW.
        let attaches = is_joiner_or_mark
            && units.last().is_some_and(|base| {
                !matches!(
                    base.class,
                    C::Mandatory
                        | C::CarriageReturn
                        | C::LineFeed
                        | C::NextLine
                        | C::Space
                        | C::ZeroWidthSpace
                )
            });
        if attaches {
            if let Some(base) = units.last_mut() {
                base.ends_with_zwj = class == C::ZeroWidthJoiner;
            }
            continue;
        }

        // LB10: a mark or joiner that stays unattached behaves as `AL`, like U+0041.
        let (class, flags) = if is_joiner_or_mark {
            (C::Alphabetic, 0)
        } else {
            (class, flags)
        };
        units.push(Unit {
            class,
            start,
            code_point,
            flags,
            ends_with_zwj: code_point == 0x200D,
            regional_indicator_run: 0,
        });
    }

    let mut run = 0;
    for unit in &mut units {
        run = if unit.class == C::RegionalIndicator {
            run + 1
        } else {
            0
        };
        unit.regional_indicator_run = run;
    }
    units
}

/// LB1: resolves the classes that depend on context outside the algorithm.
fn resolve(class: C, flags: u8) -> C {
    match class {
        C::Ambiguous | C::Surrogate | C::Unknown => C::Alphabetic,
        C::ComplexContext if flags & props::MARK != 0 => C::CombiningMark,
        C::ComplexContext => C::Alphabetic,
        C::ConditionalJapaneseStarter => C::NonStarter,
        other => other,
    }
}

/// Decides the boundary before `units[k]`, or after the last unit when `k == units.len()`.
///
/// Returns `None` when the position is not a break.
pub(crate) fn decide(units: &[Unit], k: usize) -> Option<(BreakOpportunity, &'static str)> {
    // LB3: always break at the end of text.
    if k == units.len() {
        return Some((BreakOpportunity::Mandatory, "LB3"));
    }
    // LB2: never break at the start of text.
    if k == 0 {
        return None;
    }
    match rules(units, k) {
        Verdict::Break(kind, rule) => Some((kind, rule)),
        Verdict::NoBreak(_) => None,
        // LB31: break everywhere else.
        Verdict::Continue => Some((BreakOpportunity::Allowed, "LB31")),
    }
}

/// Index of the last unit before `k` that is not a space, or `None` at the start of text.
fn before_spaces(units: &[Unit], k: usize) -> Option<usize> {
    (0..k).rev().find(|&j| units[j].class != C::Space)
}

/// Returns the class of `units[i]` when `i` is in range.
fn class_at(units: &[Unit], i: Option<usize>) -> Option<C> {
    i.and_then(|i| units.get(i)).map(|unit| unit.class)
}

/// True when the run of `SY` and `IS` units ending at `end` is preceded by `NU`.
/// Used for the `NU (SY | IS)*` patterns of LB25.
fn numeric_before_symbols(units: &[Unit], end: Option<usize>) -> bool {
    let mut i = match end {
        Some(i) => i as isize,
        None => return false,
    };
    while i >= 0 && matches!(units[i as usize].class, C::Symbol | C::InfixSeparator) {
        i -= 1;
    }
    i >= 0 && units[i as usize].class == C::Numeric
}

fn is_east_asian_wide(unit: &Unit) -> bool {
    unit.flags & props::EAST_ASIAN_WIDE != 0
}

/// True for a unit that can start an orthographic syllable in Brahmic scripts
/// (`AK`, the dotted circle, or `AS`), as used by LB28a.
fn is_brahmic_base(unit: &Unit) -> bool {
    matches!(unit.class, C::Aksara | C::AksaraStart) || unit.code_point == 0x25CC
}

/// Applies the rules in order to the boundary before `units[k]`, with `1 <= k < len`.
fn rules(units: &[Unit], k: usize) -> Verdict {
    use BreakOpportunity::{Allowed, Mandatory};
    use Verdict::{Break, Continue, NoBreak};

    let before = &units[k - 1];
    let after = &units[k];
    let (cb, ca) = (before.class, after.class);
    let at = |i: isize| -> Option<&Unit> {
        if i < 0 {
            None
        } else {
            units.get(i as usize)
        }
    };
    let next = at(k as isize + 1);
    let prev_non_space = before_spaces(units, k);
    let class_before_non_space = class_at(units, prev_non_space);

    // LB4, LB5: hard line breaks.
    if cb == C::Mandatory {
        return Break(Mandatory, "LB4");
    }
    if cb == C::CarriageReturn && ca == C::LineFeed {
        return NoBreak("LB4");
    }
    if matches!(cb, C::CarriageReturn | C::LineFeed | C::NextLine) {
        return Break(Mandatory, "LB4");
    }

    // LB6: do not break before hard line breaks.
    if matches!(
        ca,
        C::Mandatory | C::CarriageReturn | C::LineFeed | C::NextLine
    ) {
        return NoBreak("LB6");
    }

    // LB7: do not break before spaces or zero width space.
    if matches!(ca, C::Space | C::ZeroWidthSpace) {
        return NoBreak("LB7");
    }

    // LB8: break after zero width space, even if spaces intervene.
    if class_before_non_space == Some(C::ZeroWidthSpace) {
        return Break(Allowed, "LB8");
    }

    // LB8a: do not break after a zero width joiner.
    if before.ends_with_zwj {
        return NoBreak("LB8a");
    }

    // LB11: word joiner.
    if ca == C::WordJoiner || cb == C::WordJoiner {
        return NoBreak("LB11");
    }

    // LB12: do not break after glue.
    if cb == C::NonBreakingGlue {
        return NoBreak("LB12");
    }

    // LB12a: do not break before glue, except after spaces and hyphens.
    if ca == C::NonBreakingGlue && !matches!(cb, C::Space | C::Hyphen | C::UnambiguousHyphen) {
        return NoBreak("LB12a");
    }

    // LB13: do not break before closing punctuation, exclamation, solidus.
    if matches!(
        ca,
        C::ClosePunctuation | C::CloseParenthesis | C::Exclamation | C::Symbol
    ) {
        return NoBreak("LB13");
    }

    // LB14: do not break after opening punctuation, even after spaces.
    if class_before_non_space == Some(C::OpenPunctuation) {
        return NoBreak("LB14");
    }

    // LB15a: do not break after an initial quotation mark at the start of the line.
    if let Some(j) = prev_non_space {
        if units[j].class == C::Quotation && units[j].flags & props::PI != 0 {
            let starts_line = match j.checked_sub(1) {
                None => true,
                Some(p) => matches!(
                    units[p].class,
                    C::Mandatory
                        | C::CarriageReturn
                        | C::LineFeed
                        | C::NextLine
                        | C::OpenPunctuation
                        | C::Quotation
                        | C::NonBreakingGlue
                        | C::Space
                        | C::ZeroWidthSpace
                ),
            };
            if starts_line {
                return NoBreak("LB15a");
            }
        }
    }

    // LB15b: do not break before a final quotation mark at the end of the line.
    if ca == C::Quotation && after.flags & props::PF != 0 {
        let ends_line = match next {
            None => true,
            Some(n) => matches!(
                n.class,
                C::Space
                    | C::NonBreakingGlue
                    | C::WordJoiner
                    | C::ClosePunctuation
                    | C::Quotation
                    | C::CloseParenthesis
                    | C::Exclamation
                    | C::InfixSeparator
                    | C::Symbol
                    | C::Mandatory
                    | C::CarriageReturn
                    | C::LineFeed
                    | C::NextLine
                    | C::ZeroWidthSpace
            ),
        };
        if ends_line {
            return NoBreak("LB15b");
        }
    }

    // LB15c: break before a decimal mark that follows a space, as in "subtract .5".
    if cb == C::Space && ca == C::InfixSeparator && class_at(units, Some(k + 1)) == Some(C::Numeric)
    {
        return Break(Allowed, "LB15c");
    }

    // LB15d: otherwise, do not break before ';', ',' or '.'.
    if ca == C::InfixSeparator {
        return NoBreak("LB15d");
    }

    // LB16: do not break between closing punctuation and a non-starter, even after spaces.
    if ca == C::NonStarter
        && matches!(
            class_before_non_space,
            Some(C::ClosePunctuation | C::CloseParenthesis)
        )
    {
        return NoBreak("LB16");
    }

    // LB17: do not break within a double em dash, even after spaces.
    if ca == C::BeforeAndAfter && class_before_non_space == Some(C::BeforeAndAfter) {
        return NoBreak("LB17");
    }

    // LB18: break after spaces.
    if cb == C::Space {
        return Break(Allowed, "LB18");
    }

    // LB19: do not break before a non-initial quotation mark, or after a non-final one.
    if ca == C::Quotation && after.flags & props::PI == 0 {
        return NoBreak("LB19");
    }
    if cb == C::Quotation && before.flags & props::PF == 0 {
        return NoBreak("LB19");
    }

    // LB19a: quotation marks not surrounded by East Asian characters.
    if ca == C::Quotation && !is_east_asian_wide(before) {
        return NoBreak("LB19a");
    }
    if ca == C::Quotation && next.map_or(true, |n| !is_east_asian_wide(n)) {
        return NoBreak("LB19a");
    }
    if cb == C::Quotation && !is_east_asian_wide(after) {
        return NoBreak("LB19a");
    }
    if cb == C::Quotation
        && k.checked_sub(2)
            .map_or(true, |p| !is_east_asian_wide(&units[p]))
    {
        return NoBreak("LB19a");
    }

    // LB20: break before and after unresolved contingent break opportunities.
    if ca == C::Contingent || cb == C::Contingent {
        return Break(Allowed, "LB20");
    }

    // LB20a: do not break after a word-initial hyphen.
    if matches!(cb, C::Hyphen | C::UnambiguousHyphen)
        && matches!(ca, C::Alphabetic | C::HebrewLetter)
    {
        let word_initial = match k.checked_sub(2) {
            None => true,
            Some(p) => matches!(
                units[p].class,
                C::Mandatory
                    | C::CarriageReturn
                    | C::LineFeed
                    | C::NextLine
                    | C::Space
                    | C::ZeroWidthSpace
                    | C::Contingent
                    | C::NonBreakingGlue
            ),
        };
        if word_initial {
            return NoBreak("LB20a");
        }
    }

    // LB21: do not break before hyphens, non-starters or after 'before' classes.
    if matches!(
        ca,
        C::After | C::UnambiguousHyphen | C::Hyphen | C::NonStarter
    ) {
        return NoBreak("LB21");
    }
    if cb == C::Before {
        return NoBreak("LB21");
    }

    // LB21a: do not break after the hyphen in Hebrew + hyphen + non-Hebrew.
    if matches!(cb, C::Hyphen | C::UnambiguousHyphen)
        && k.checked_sub(2)
            .is_some_and(|p| units[p].class == C::HebrewLetter)
        && ca != C::HebrewLetter
    {
        return NoBreak("LB21a");
    }

    // LB21b: do not break between a solidus and a Hebrew letter.
    if cb == C::Symbol && ca == C::HebrewLetter {
        return NoBreak("LB21b");
    }

    // LB22: do not break before an ellipsis.
    if ca == C::Inseparable {
        return NoBreak("LB22");
    }

    // LB23: do not break between letters and numbers.
    let letter = |c: C| matches!(c, C::Alphabetic | C::HebrewLetter);
    if (letter(cb) && ca == C::Numeric) || (cb == C::Numeric && letter(ca)) {
        return NoBreak("LB23");
    }

    // LB23a: do not break between numeric prefixes and ideographs, or ideographs and postfixes.
    if cb == C::Prefix && matches!(ca, C::Ideographic | C::EmojiBase | C::EmojiModifier) {
        return NoBreak("LB23a");
    }
    if matches!(cb, C::Ideographic | C::EmojiBase | C::EmojiModifier) && ca == C::Postfix {
        return NoBreak("LB23a");
    }

    // LB24: do not break between numeric prefix/postfix and letters.
    if matches!(cb, C::Prefix | C::Postfix) && letter(ca) {
        return NoBreak("LB24");
    }
    if letter(cb) && matches!(ca, C::Prefix | C::Postfix) {
        return NoBreak("LB24");
    }

    // LB25: do not break numbers.
    if matches!(ca, C::Postfix | C::Prefix)
        && matches!(cb, C::ClosePunctuation | C::CloseParenthesis)
        && numeric_before_symbols(units, k.checked_sub(2))
    {
        return NoBreak("LB25");
    }
    if matches!(ca, C::Postfix | C::Prefix)
        && matches!(cb, C::Symbol | C::InfixSeparator | C::Numeric)
        && numeric_before_symbols(units, Some(k - 1))
    {
        return NoBreak("LB25");
    }
    if ca == C::Numeric && numeric_before_symbols(units, Some(k - 1)) {
        return NoBreak("LB25");
    }
    if cb == C::Postfix
        && ca == C::OpenPunctuation
        && (class_at(units, Some(k + 1)) == Some(C::Numeric)
            || (class_at(units, Some(k + 1)) == Some(C::InfixSeparator)
                && class_at(units, Some(k + 2)) == Some(C::Numeric)))
    {
        return NoBreak("LB25");
    }
    if cb == C::Prefix
        && ca == C::OpenPunctuation
        && (class_at(units, Some(k + 1)) == Some(C::Numeric)
            || (class_at(units, Some(k + 1)) == Some(C::InfixSeparator)
                && class_at(units, Some(k + 2)) == Some(C::Numeric)))
    {
        return NoBreak("LB25");
    }
    if ca == C::Numeric && matches!(cb, C::Postfix | C::Prefix | C::Hyphen | C::InfixSeparator) {
        return NoBreak("LB25");
    }

    // LB26: do not break a Korean syllable block.
    if cb == C::HangulLJamo
        && matches!(
            ca,
            C::HangulLJamo | C::HangulVJamo | C::HangulLvSyllable | C::HangulLvtSyllable
        )
    {
        return NoBreak("LB26");
    }
    if matches!(cb, C::HangulVJamo | C::HangulLvSyllable)
        && matches!(ca, C::HangulVJamo | C::HangulTJamo)
    {
        return NoBreak("LB26");
    }
    if matches!(cb, C::HangulTJamo | C::HangulLvtSyllable) && ca == C::HangulTJamo {
        return NoBreak("LB26");
    }

    // LB27: treat Korean syllable blocks as ID.
    let korean = |c: C| {
        matches!(
            c,
            C::HangulLJamo
                | C::HangulVJamo
                | C::HangulTJamo
                | C::HangulLvSyllable
                | C::HangulLvtSyllable
        )
    };
    if korean(cb) && ca == C::Postfix {
        return NoBreak("LB27");
    }
    if cb == C::Prefix && korean(ca) {
        return NoBreak("LB27");
    }

    // LB28: do not break between alphabetics.
    if letter(cb) && letter(ca) {
        return NoBreak("LB28");
    }

    // LB28a: do not break inside the orthographic syllables of Brahmic scripts.
    // `is_brahmic_base` covers AK, AS and the dotted circle; the last rule needs AK or the dotted circle.
    let aksara_or_circle = |u: &Unit| u.class == C::Aksara || u.code_point == 0x25CC;
    if cb == C::AksaraPreBase && is_brahmic_base(after) {
        return NoBreak("LB28a");
    }
    if is_brahmic_base(before) && matches!(ca, C::ViramaFinal | C::Virama) {
        return NoBreak("LB28a");
    }
    if cb == C::Virama
        && k.checked_sub(2).is_some_and(|p| is_brahmic_base(&units[p]))
        && aksara_or_circle(after)
    {
        return NoBreak("LB28a");
    }
    if is_brahmic_base(before)
        && is_brahmic_base(after)
        && class_at(units, Some(k + 1)) == Some(C::ViramaFinal)
    {
        return NoBreak("LB28a");
    }

    // LB29: do not break between numeric punctuation and alphabetics.
    if cb == C::InfixSeparator && letter(ca) {
        return NoBreak("LB29");
    }

    // LB30: do not break between letters, numbers or symbols and parentheses.
    if ca == C::OpenPunctuation && !is_east_asian_wide(after) && (letter(cb) || cb == C::Numeric) {
        return NoBreak("LB30");
    }
    if cb == C::CloseParenthesis && !is_east_asian_wide(before) && (letter(ca) || ca == C::Numeric)
    {
        return NoBreak("LB30");
    }

    // LB30a: pair regional indicators; a break is allowed only after an even count.
    if cb == C::RegionalIndicator
        && ca == C::RegionalIndicator
        && before.regional_indicator_run % 2 == 1
    {
        return NoBreak("LB30a");
    }

    // LB30b: do not break between an emoji base (or potential emoji) and an emoji modifier.
    if cb == C::EmojiBase && ca == C::EmojiModifier {
        return NoBreak("LB30b");
    }
    if ca == C::EmojiModifier
        && before.flags & props::EXTENDED_PICTOGRAPHIC != 0
        && before.flags & props::UNASSIGNED != 0
    {
        return NoBreak("LB30b");
    }

    Continue
}
