//! Splits paragraph text into runs that share a script, style and Bidi level.

use alloc::vec::Vec;
use core::ops::Range;

use unicode_bidi::Level;
use unicode_script::{Script, UnicodeScript};

use crate::style::{StyleSpan, TextStyle};

/// A contiguous piece of text with one script, one style and one Bidi level.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Item {
    /// Byte range within the paragraph text.
    pub range: Range<usize>,
    /// Resolved script. `Common` and `Inherited` characters take the script
    /// of their neighbours.
    pub script: Script,
    /// Index into the style table: `0` is the base style, `i + 1` is `spans[i]`.
    pub style_index: usize,
    /// Resolved embedding level from the Unicode Bidi algorithm.
    pub level: Level,
}

/// Returns the style table: base style first, then one entry per span.
pub(crate) fn style_table<'a>(base: &'a TextStyle, spans: &'a [StyleSpan]) -> Vec<&'a TextStyle> {
    let mut table = Vec::with_capacity(spans.len() + 1);
    table.push(base);
    table.extend(spans.iter().map(|s| &s.style));
    table
}

/// Splits `text` into items by script, style and level.
///
/// `levels` holds one Bidi level per byte of `text`, as produced by
/// `unicode_bidi::BidiInfo`.
pub(crate) fn itemize(text: &str, spans: &[StyleSpan], levels: &[Level]) -> Vec<Item> {
    // Per character: (byte start, byte end, style index, script, level).
    let mut chars: Vec<(usize, usize, usize, Script, Level)> = Vec::with_capacity(text.len());
    for (start, c) in text.char_indices() {
        let end = start + c.len_utf8();
        // Later spans override earlier ones when ranges overlap.
        let style_index = spans
            .iter()
            .rposition(|s| s.range.start <= start && start < s.range.end)
            .map_or(0, |i| i + 1);
        chars.push((start, end, style_index, c.script(), levels[start]));
    }

    resolve_neutral_scripts(&mut chars);

    let mut items: Vec<Item> = Vec::new();
    for (start, end, style_index, script, level) in chars {
        match items.last_mut() {
            Some(last)
                if last.style_index == style_index
                    && last.script == script
                    && last.level == level =>
            {
                last.range.end = end;
            }
            _ => items.push(Item {
                range: start..end,
                script,
                style_index,
                level,
            }),
        }
    }
    items
}

type Char = (usize, usize, usize, Script, Level);

/// Gives `Common` and `Inherited` characters the script of the nearest strong
/// character before them, or after them if none precedes.
fn resolve_neutral_scripts(chars: &mut [Char]) {
    let is_neutral = |s: Script| s == Script::Common || s == Script::Inherited;

    let mut previous_strong: Option<Script> = None;
    for entry in chars.iter_mut() {
        if is_neutral(entry.3) {
            if let Some(s) = previous_strong {
                entry.3 = s;
            }
        } else {
            previous_strong = Some(entry.3);
        }
    }

    // Leading neutrals that could not take a script from the left.
    let mut next_strong: Option<Script> = None;
    for entry in chars.iter_mut().rev() {
        if is_neutral(entry.3) {
            if let Some(s) = next_strong {
                entry.3 = s;
            }
        } else {
            next_strong = Some(entry.3);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::Fx;

    fn ltr(text: &str) -> Vec<Level> {
        alloc::vec![Level::ltr(); text.len()]
    }

    #[test]
    fn splits_on_script_change() {
        let text = "abc\u{0628}\u{062A}";
        let items = itemize(text, &[], &ltr(text));
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].script, Script::Latin);
        assert_eq!(items[0].range, 0..3);
        assert_eq!(items[1].script, Script::Arabic);
    }

    #[test]
    fn neutrals_take_neighbouring_script() {
        let text = "abc 123";
        let items = itemize(text, &[], &ltr(text));
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].script, Script::Latin);
    }

    #[test]
    fn splits_on_style_span() {
        let text = "abcdef";
        let spans = [StyleSpan {
            range: 2..4,
            style: TextStyle::new("Other", Fx::from_int(20)),
        }];
        let items = itemize(text, &spans, &ltr(text));
        let styles: Vec<(Range<usize>, usize)> = items
            .iter()
            .map(|i| (i.range.clone(), i.style_index))
            .collect();
        assert_eq!(styles, [(0..2, 0), (2..4, 1), (4..6, 0)]);
    }

    #[test]
    fn splits_on_bidi_level() {
        let text = "ab\u{05D0}\u{05D1}";
        let mut levels = ltr(text);
        for l in levels.iter_mut().skip(2) {
            *l = Level::rtl();
        }
        let items = itemize(text, &[], &levels);
        assert_eq!(items.len(), 2);
        assert!(items[1].level.is_rtl());
    }
}
