//! Paragraph layout: Bidi, font fallback, UAX #14 breaking with hyphenation,
//! tab stops, alignment, justification, truncation and multi-paragraph
//! documents.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt;
use core::ops::Range;

use tpt_text_linebreak::{linebreaks_explained, BreakOpportunity};
use unicode_bidi::{BidiInfo, Level};
use unicode_script::Script;

use crate::fallback::FallbackResolver;
use crate::font::{FontFace, FontProvider, FontQuery};
use crate::fx::{scale_round, Fx};
use crate::hyphenate::Hyphenator;
use crate::itemize::{itemize, style_table, Item};
use crate::shape::{shape_run, shape_run_cached, Direction, ShapeCache, ShapedGlyph, ShapedRun};
use crate::style::{StyleSpan, TextStyle};

/// Errors raised by layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayoutError {
    /// No face in the provider matches this family.
    MissingFont(String),
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LayoutError::MissingFont(family) => {
                write!(f, "no font registered for family `{family}`")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for LayoutError {}

/// Horizontal alignment of lines within the width limit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    /// The start of the line: left for left-to-right text, right for
    /// right-to-left text.
    #[default]
    Start,
    /// The end of the line: right for left-to-right text, left for
    /// right-to-left text.
    End,
    /// Flush left.
    Left,
    /// Flush right.
    Right,
    /// Centred.
    Center,
    /// Spread spaces so each non-final line fills the width.
    Justify,
}

/// Options for [`layout_paragraph_with`]. The default reproduces
/// [`layout_paragraph`].
#[derive(Clone, Default)]
pub struct LayoutOptions<'a> {
    /// Paragraph direction. `None` detects it from the first strong character.
    pub direction: Option<Direction>,
    /// Horizontal alignment.
    pub align: Align,
    /// Line height as a multiple of the natural height. `None` means `1`.
    pub line_spacing: Option<Fx>,
    /// Maximum number of lines. `Some(0)` is treated as `Some(1)`.
    pub max_lines: Option<usize>,
    /// Ends a line with an ellipsis when it is truncated by `max_lines`, or
    /// when the last line is wider than the width and cannot wrap.
    pub ellipsis: bool,
    /// Tab-stop interval, measured from the line start. `None` means four em.
    pub tab_stop: Option<Fx>,
    /// Caller-configured fallback families.
    pub fallback: Option<&'a FallbackResolver>,
    /// Hyphenation points for word-internal breaks.
    pub hyphenator: Option<&'a dyn Hyphenator>,
}

impl<'a> LayoutOptions<'a> {
    /// Sets the paragraph direction.
    pub fn direction(mut self, direction: Direction) -> Self {
        self.direction = Some(direction);
        self
    }

    /// Sets the alignment.
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// Sets the line height as a multiple of the natural height.
    pub fn line_spacing(mut self, spacing: Fx) -> Self {
        self.line_spacing = Some(spacing);
        self
    }

    /// Limits the number of lines.
    pub fn max_lines(mut self, lines: usize) -> Self {
        self.max_lines = Some(lines);
        self
    }

    /// Enables or disables the ellipsis.
    pub fn ellipsis(mut self, on: bool) -> Self {
        self.ellipsis = on;
        self
    }

    /// Sets the tab-stop interval.
    pub fn tab_stop(mut self, stop: Fx) -> Self {
        self.tab_stop = Some(stop);
        self
    }

    /// Sets the fallback resolver.
    pub fn fallback(mut self, resolver: &'a FallbackResolver) -> Self {
        self.fallback = Some(resolver);
        self
    }

    /// Sets the hyphenator.
    pub fn hyphenator(mut self, hyphenator: &'a dyn Hyphenator) -> Self {
        self.hyphenator = Some(hyphenator);
        self
    }
}

impl fmt::Debug for LayoutOptions<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LayoutOptions")
            .field("direction", &self.direction)
            .field("align", &self.align)
            .field("line_spacing", &self.line_spacing)
            .field("max_lines", &self.max_lines)
            .field("ellipsis", &self.ellipsis)
            .field("tab_stop", &self.tab_stop)
            .field("fallback", &self.fallback.is_some())
            .field("hyphenator", &self.hyphenator.is_some())
            .finish()
    }
}

/// One line of a laid-out paragraph.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct LayoutLine {
    /// Runs in visual (left-to-right) order, each sliced to this line's glyphs.
    pub runs: Vec<ShapedRun>,
    /// Top edge of the line box, from the top of the paragraph.
    pub top: Fx,
    /// Baseline position from the top of the paragraph.
    pub baseline_y: Fx,
    /// Line box height.
    pub height: Fx,
    /// Horizontal offset applied by alignment.
    pub x_offset: Fx,
    /// Byte range of the paragraph text that this line covers.
    pub text_range: Range<usize>,
}

/// A character that had no glyph in any face tried for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct MissingGlyph {
    /// Byte index of the character in the paragraph text.
    pub byte: usize,
    /// The character.
    pub ch: char,
}

/// A fully laid-out paragraph.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ParagraphLayout {
    /// Lines from top to bottom.
    pub lines: Vec<LayoutLine>,
    /// Sum of all line heights.
    pub total_height: Fx,
    /// The width limit used for breaking.
    pub max_width: Fx,
    /// The laid-out text, kept for hit-testing.
    pub text: String,
    /// Base direction of the paragraph.
    pub direction: Direction,
    /// Characters with no glyph, in text order. Each is drawn as `.notdef`.
    pub missing_glyphs: Vec<MissingGlyph>,
}

impl ParagraphLayout {
    /// A 64-bit FNV-1a digest of the layout's positions and glyph ids.
    ///
    /// Integers are hashed little-endian, so two layouts with equal digests
    /// place the same glyphs at the same positions on any platform.
    pub fn digest(&self) -> u64 {
        let mut hash = FnvHasher::new();
        hash.write_u64(self.lines.len() as u64);
        hash.write_i32(self.total_height.to_raw());
        for line in &self.lines {
            hash.write_i32(line.top.to_raw());
            hash.write_i32(line.baseline_y.to_raw());
            hash.write_i32(line.height.to_raw());
            hash.write_i32(line.x_offset.to_raw());
            hash.write_u64(line.text_range.start as u64);
            hash.write_u64(line.text_range.end as u64);
            for run in &line.runs {
                hash.write_u64(run.glyphs.len() as u64);
                for g in &run.glyphs {
                    hash.write_u64(u64::from(g.glyph_id));
                    hash.write_i32(g.x_advance.to_raw());
                    hash.write_i32(g.x_offset.to_raw());
                    hash.write_i32(g.y_offset.to_raw());
                    hash.write_u64(g.cluster_index as u64);
                }
            }
        }
        hash.finish()
    }
}

struct FnvHasher(u64);

impl FnvHasher {
    fn new() -> FnvHasher {
        FnvHasher(0xcbf2_9ce4_8422_2325)
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn write_u64(&mut self, value: u64) {
        self.write_bytes(&value.to_le_bytes());
    }

    fn write_i32(&mut self, value: i32) {
        self.write_bytes(&value.to_le_bytes());
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

/// Lays out `text` inside `max_width` with default options.
///
/// `base` applies to bytes not covered by `spans`. Faces are found through
/// `provider` by family, weight and slant. A glyph wider than `max_width` is
/// placed alone on its line so the function always terminates.
pub fn layout_paragraph(
    provider: &dyn FontProvider,
    text: &str,
    base: &TextStyle,
    spans: &[StyleSpan],
    max_width: Fx,
) -> Result<ParagraphLayout, LayoutError> {
    layout_paragraph_with(
        provider,
        text,
        base,
        spans,
        max_width,
        &LayoutOptions::default(),
    )
}

/// Lays out `text` with a single style and default options.
///
/// The shortest entry point: `layout_text(&registry, "Hello", &style, width)`.
pub fn layout_text(
    provider: &dyn FontProvider,
    text: &str,
    style: &TextStyle,
    max_width: Fx,
) -> Result<ParagraphLayout, LayoutError> {
    layout_paragraph(provider, text, style, &[], max_width)
}

/// Lays out `text` with explicit [`LayoutOptions`].
pub fn layout_paragraph_with(
    provider: &dyn FontProvider,
    text: &str,
    base: &TextStyle,
    spans: &[StyleSpan],
    max_width: Fx,
    options: &LayoutOptions<'_>,
) -> Result<ParagraphLayout, LayoutError> {
    layout_inner(provider, text, base, spans, max_width, options, None)
}

/// [`layout_paragraph_with`], reusing shaping results from `cache` when given.
fn layout_inner(
    provider: &dyn FontProvider,
    text: &str,
    base: &TextStyle,
    spans: &[StyleSpan],
    max_width: Fx,
    options: &LayoutOptions<'_>,
    mut cache: Option<&mut ShapeCache>,
) -> Result<ParagraphLayout, LayoutError> {
    // Resolve the base face up front so metrics exist even for empty text.
    let base_face = load_face(provider, base)?;

    let para_level = options.direction.map(|d| match d {
        Direction::LeftToRight => Level::ltr(),
        Direction::RightToLeft => Level::rtl(),
    });
    let bidi = BidiInfo::new(text, para_level);
    let styles = style_table(base, spans);

    let levels = resolved_levels(&bidi);
    let base_level = bidi.paragraphs.first().map_or(Level::ltr(), |p| p.level);
    let items = itemize(text, spans, &levels);
    let (fonts, segments) = resolve_segments(provider, options.fallback, text, &items, &styles)?;

    let mut runs: Vec<ShapedRun> = Vec::with_capacity(segments.len());
    for s in &segments {
        let font = &fonts[s.font];
        let style = styles[s.style_index];
        let direction = direction_of(s.level);
        runs.push(match cache.as_deref_mut() {
            Some(c) => shape_run_cached(c, font, text, s.range.clone(), style, direction),
            None => shape_run(font, text, s.range.clone(), style, direction),
        });
    }
    let run_levels: Vec<Level> = segments.iter().map(|s| s.level).collect();
    let missing_glyphs = missing_glyphs(&runs, text);
    let starts = cluster_starts(&runs, text.len());

    let tab_stop = options
        .tab_stop
        .unwrap_or_else(|| base.size * Fx::from_int(4));
    let breaks = break_opportunities(text, options.hyphenator);
    let glyphs = flatten(&runs, &run_levels, text, &starts, &breaks);
    let direction = direction_of(base_level);
    let mut line_segments = break_segments(&glyphs, text, &starts, max_width, tab_stop);

    let max_lines = options.max_lines.unwrap_or(usize::MAX).max(1);
    // A paragraph with no break opportunity is one word. With the ellipsis on,
    // it is shown as a single truncated line instead of being split mid-word.
    let single_word = glyphs.iter().skip(1).all(|g| g.brk.is_none());
    let overlong_word = options.ellipsis && single_word && line_segments.len() > 1;
    let truncated = line_segments.len() > max_lines || overlong_word;
    line_segments.truncate(max_lines.min(if overlong_word { 1 } else { usize::MAX }));
    if !truncated && ends_with_separator(text) {
        // A trailing separator leaves an empty last line: a caret target.
        line_segments.push(LineSegment {
            range: glyphs.len()..glyphs.len(),
            hyphen_run: None,
        });
    }

    let line_spacing = options.line_spacing.unwrap_or(Fx::ONE);
    let base_metrics = Metrics::of(&base_face, base.size);

    let mut lines = Vec::with_capacity(line_segments.len());
    let mut top = Fx::ZERO;
    let count = line_segments.len();
    for (idx, seg) in line_segments.iter().enumerate() {
        let is_last_line = idx + 1 == count;
        let line_para_level = paragraph_level_at(&bidi, seg, &glyphs, text.len());
        let line_rtl = line_para_level.is_rtl();
        let hanging = trailing_space_clusters(&glyphs, seg, text, &starts);

        let mut line_runs = build_line_runs(
            &runs,
            &run_levels,
            &glyphs,
            text,
            seg,
            line_para_level,
            &hanging,
            &starts,
        );
        apply_tabs(&mut line_runs, text, &starts, tab_stop, line_rtl);
        if let Some(hyphen) = hyphen_mark(&runs, seg, &glyphs) {
            // The hyphen sits at the logical end of the line.
            if line_rtl {
                line_runs.insert(0, hyphen);
            } else {
                line_runs.push(hyphen);
            }
        }

        let is_last_of_paragraph = seg.range.end >= glyphs.len()
            || glyphs[seg.range.end]
                .brk
                .is_some_and(|b| b.kind == BreakKind::Mandatory);

        // Trailing whitespace hangs past the line end, so it is excluded from
        // the width used for alignment and justification.
        let mut width = visible_width(&line_runs, &hanging);
        let mut text_range = line_text_range(&glyphs, seg, text.len());
        let overflows = width > max_width;
        if options.ellipsis && is_last_line && (truncated || overflows) {
            let (font, style) = mark_font(&line_runs, base, &base_face, '\u{2026}');
            let mark = shaped_mark(&font, &style, "\u{2026}", text.len());
            let kept = truncate_with_mark(&mut line_runs, text, &starts, max_width, mark, line_rtl);
            let end = kept.map_or(text_range.start, |last| {
                glyphs
                    .iter()
                    .map(|g| g.cluster)
                    .find(|&c| c > last)
                    .unwrap_or(text.len())
                    .min(text_range.end)
            });
            text_range = text_range.start..end.max(text_range.start);
            width = visible_width(&line_runs, &[]);
        } else if options.align == Align::Justify && !is_last_of_paragraph && !is_last_line {
            justify(&mut line_runs, text, &starts, max_width - width, &hanging);
            width = max_width;
        }

        let align = resolve_align(options.align, line_rtl);
        let x_offset = match align {
            Align::Left | Align::Justify | Align::Start | Align::End => Fx::ZERO,
            Align::Right => (max_width - width).max(Fx::ZERO),
            Align::Center => {
                let free = (max_width - width).max(Fx::ZERO);
                Fx::from_raw(free.to_raw() / 2)
            }
        };

        let metrics = line_runs
            .iter()
            .fold(None, |acc: Option<Metrics>, run| {
                let m = Metrics::of(&run.font, run.style.size);
                Some(acc.map_or(m, |a| a.max(m)))
            })
            .unwrap_or(base_metrics);
        let natural = metrics.natural();
        let height = natural * line_spacing;
        let leading = Fx::from_raw((height - natural).to_raw() / 2);

        lines.push(LayoutLine {
            runs: line_runs,
            top,
            baseline_y: top + leading + metrics.ascent,
            height,
            x_offset,
            text_range,
        });
        top += height;
    }

    Ok(ParagraphLayout {
        lines,
        total_height: top,
        max_width,
        text: String::from(text),
        direction,
        missing_glyphs,
    })
}

/// Resolves `Start` and `End` against a line's base direction.
fn resolve_align(align: Align, rtl: bool) -> Align {
    match (align, rtl) {
        (Align::Start, false) | (Align::End, true) => Align::Left,
        (Align::Start, true) | (Align::End, false) => Align::Right,
        (other, _) => other,
    }
}

/// The paragraph level of the paragraph that contains a line's first glyph.
fn paragraph_level_at(
    bidi: &BidiInfo<'_>,
    seg: &LineSegment,
    glyphs: &[GlyphRef],
    text_len: usize,
) -> Level {
    let byte = glyphs.get(seg.range.start).map_or(text_len, |g| g.cluster);
    bidi.paragraphs
        .iter()
        .find(|p| p.range.contains(&byte) || (byte == text_len && p.range.end == text_len))
        .or_else(|| bidi.paragraphs.last())
        .map_or(Level::ltr(), |p| p.level)
}

/// True if the text ends with a character that forces a line break.
fn ends_with_separator(text: &str) -> bool {
    matches!(
        text.chars().next_back(),
        Some('\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}')
    )
}

/// Characters with no glyph in the face that drew them.
fn missing_glyphs(runs: &[ShapedRun], text: &str) -> Vec<MissingGlyph> {
    let mut out: Vec<MissingGlyph> = Vec::new();
    for run in runs {
        for g in &run.glyphs {
            if g.glyph_id != 0 || out.last().is_some_and(|m| m.byte == g.cluster_index) {
                continue;
            }
            if let Some(ch) = char_at(text, g.cluster_index) {
                if !ch.is_control() && !ch.is_whitespace() && !is_hidden(text, g.cluster_index) {
                    out.push(MissingGlyph {
                        byte: g.cluster_index,
                        ch,
                    });
                }
            }
        }
    }
    out.sort_by_key(|m| m.byte);
    out.dedup_by_key(|m| m.byte);
    out
}

fn load_face(provider: &dyn FontProvider, style: &TextStyle) -> Result<Arc<FontFace>, LayoutError> {
    provider
        .load(&FontQuery {
            family: style.family.clone(),
            weight: style.weight,
            italic: style.italic,
        })
        .ok_or_else(|| LayoutError::MissingFont(style.family.clone()))
}

/// Resolved per-byte Bidi levels for `bidi`, with rule L1 applied to the
/// separators (tabs) and trailing whitespace of each paragraph. Wrapped lines
/// apply L1 again to their own trailing whitespace during layout.
pub fn resolved_levels(bidi: &BidiInfo<'_>) -> Vec<Level> {
    let mut levels = bidi.levels.clone();
    for para in &bidi.paragraphs {
        let reset = bidi.reordered_levels(para, para.range.clone());
        levels[para.range.clone()].copy_from_slice(&reset[para.range.clone()]);
    }
    levels
}

fn direction_of(level: Level) -> Direction {
    if level.is_rtl() {
        Direction::RightToLeft
    } else {
        Direction::LeftToRight
    }
}

fn line_width(runs: &[ShapedRun]) -> Fx {
    runs.iter().fold(Fx::ZERO, |acc, r| acc + r.width())
}

/// Vertical metrics of a font at a given size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Metrics {
    ascent: Fx,
    descent: Fx,
    gap: Fx,
}

impl Metrics {
    fn of(face: &FontFace, size: Fx) -> Metrics {
        let upem = i64::from(face.units_per_em());
        let size_raw = i64::from(size.to_raw());
        let scale = |units: i64| {
            Fx::from_raw(
                scale_round(units, size_raw, upem).clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32,
            )
        };
        Metrics {
            ascent: scale(i64::from(face.ascender())),
            descent: scale(-i64::from(face.descender())),
            gap: scale(i64::from(face.line_gap())),
        }
    }

    fn natural(self) -> Fx {
        self.ascent + self.descent + self.gap
    }

    /// Takes the larger ascent, descent and gap, so mixed-size lines fit.
    fn max(self, other: Metrics) -> Metrics {
        Metrics {
            ascent: self.ascent.max(other.ascent),
            descent: self.descent.max(other.descent),
            gap: self.gap.max(other.gap),
        }
    }
}

/// A run of text sharing one font, style and Bidi level.
struct Segment {
    range: Range<usize>,
    style_index: usize,
    level: Level,
    font: usize,
}

/// Splits items into font segments, falling back per character when the
/// primary face lacks a glyph. Returns the distinct fonts used and the segments.
fn resolve_segments(
    provider: &dyn FontProvider,
    fallback: Option<&FallbackResolver>,
    text: &str,
    items: &[Item],
    styles: &[&TextStyle],
) -> Result<(Vec<Arc<FontFace>>, Vec<Segment>), LayoutError> {
    let mut fonts: Vec<Arc<FontFace>> = Vec::new();
    let mut segments: Vec<Segment> = Vec::new();
    // Font of the previous character, so a cluster can stay in one font.
    let mut prev_font: Option<usize> = None;
    let mut after_joiner = false;

    for item in items {
        let style = styles[item.style_index];
        let primary = load_face(provider, style)?;
        let primary_index = intern(&mut fonts, &primary);
        let mut candidates: Option<Vec<Arc<FontFace>>> = None;

        for (rel, c) in text[item.range.clone()].char_indices() {
            let start = item.range.start + rel;
            let end = start + c.len_utf8();
            let extends = after_joiner || is_extender(c);
            after_joiner = c == '\u{200D}';
            let chosen = if let (true, Some(font)) = (extends, prev_font) {
                // Marks, selectors and joined characters follow their base.
                font
            } else if c.is_control() || primary.has_glyph(c) {
                primary_index
            } else {
                let found = candidates
                    .get_or_insert_with(|| fallback_faces(provider, fallback, style, item.script))
                    .iter()
                    .find(|f| f.has_glyph(c))
                    .cloned();
                match found {
                    Some(face) => intern(&mut fonts, &face),
                    None => primary_index,
                }
            };
            prev_font = Some(chosen);

            match segments.last_mut() {
                Some(seg)
                    if seg.range.end == start
                        && seg.font == chosen
                        && seg.style_index == item.style_index
                        && seg.level == item.level =>
                {
                    seg.range.end = end;
                }
                _ => segments.push(Segment {
                    range: start..end,
                    style_index: item.style_index,
                    level: item.level,
                    font: chosen,
                }),
            }
        }
    }
    Ok((fonts, segments))
}

/// Characters that join the preceding character into one cluster: combining
/// marks, variation selectors, emoji modifiers and tags.
fn is_extender(c: char) -> bool {
    matches!(
        c as u32,
        0x0300..=0x036F
            | 0x1AB0..=0x1AFF
            | 0x1DC0..=0x1DFF
            | 0x200C
            | 0x200D
            | 0x20D0..=0x20FF
            | 0xFE00..=0xFE0F
            | 0xFE20..=0xFE2F
            | 0x1F3FB..=0x1F3FF
            | 0xE0020..=0xE007F
            | 0xE0100..=0xE01EF
    )
}

/// Returns the index of `face` in `fonts`, adding it if absent.
fn intern(fonts: &mut Vec<Arc<FontFace>>, face: &Arc<FontFace>) -> usize {
    match fonts.iter().position(|f| Arc::ptr_eq(f, face)) {
        Some(i) => i,
        None => {
            fonts.push(Arc::clone(face));
            fonts.len() - 1
        }
    }
}

/// Loads fallback faces for `script`: resolver families first, then the
/// provider's own chain.
fn fallback_faces(
    provider: &dyn FontProvider,
    fallback: Option<&FallbackResolver>,
    style: &TextStyle,
    script: Script,
) -> Vec<Arc<FontFace>> {
    let mut faces = Vec::new();
    if let Some(resolver) = fallback {
        for family in resolver.candidates(script) {
            let query = FontQuery {
                family: family.into(),
                weight: style.weight,
                italic: style.italic,
            };
            if let Some(face) = provider.load(&query) {
                faces.push(face);
            }
        }
    }
    faces.extend(provider.fallback_chain(script, &style.family));
    faces
}

/// Chooses the font and style that draw a mark (hyphen or ellipsis): the
/// first font on the line that has the character, else the base face.
fn mark_font(
    line_runs: &[ShapedRun],
    base: &TextStyle,
    base_face: &Arc<FontFace>,
    ch: char,
) -> (Arc<FontFace>, TextStyle) {
    for run in line_runs {
        if run.font.has_glyph(ch) {
            return (Arc::clone(&run.font), run.style.clone());
        }
    }
    (Arc::clone(base_face), base.clone())
}

/// Shapes a short synthetic string (hyphen, ellipsis) and pins its clusters.
fn shaped_mark(font: &Arc<FontFace>, style: &TextStyle, text: &str, cluster: usize) -> ShapedRun {
    let mut run = shape_run(font, text, 0..text.len(), style, Direction::LeftToRight);
    for g in &mut run.glyphs {
        g.cluster_index = cluster;
    }
    run
}

/// Kind of a line-break opportunity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BreakKind {
    /// A line may end here.
    Allowed,
    /// A line must end here.
    Mandatory,
    /// A line may end here, and a hyphen is drawn at the line end.
    Hyphen,
}

/// Break opportunity attached to the glyph that starts the next line.
#[derive(Clone, Copy, Debug)]
struct Break {
    kind: BreakKind,
    /// Advance of the hyphen drawn if this break is taken.
    hyphen_width: Fx,
    /// Run whose font and style draw that hyphen.
    hyphen_run: usize,
}

/// Flat glyph record used by the line breaker, in logical order.
#[derive(Clone, Copy, Debug)]
struct GlyphRef {
    /// Index into the run list.
    run: usize,
    /// Index into that run's glyphs (visual order).
    visual: usize,
    /// Byte index of the glyph's cluster start.
    cluster: usize,
    /// Advance used for breaking. Zero for hidden glyphs and tabs.
    advance: Fx,
    /// Hidden glyphs (newline, soft hyphen) are never drawn.
    hidden: bool,
    /// Break opportunity immediately before this glyph.
    brk: Option<Break>,
}

fn char_at(text: &str, byte: usize) -> Option<char> {
    text.get(byte..).and_then(|s| s.chars().next())
}

/// Characters that never draw: line and paragraph separators, vertical tab,
/// form feed, next line, and the soft hyphen (drawn only at a break).
fn is_hidden(text: &str, byte: usize) -> bool {
    matches!(
        char_at(text, byte),
        Some('\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{AD}' | '\u{2028}' | '\u{2029}')
    )
}

/// The character at `byte` when its cluster holds nothing else: the next
/// character starts a cluster of its own, or the text ends. HarfBuzz can merge
/// a following joiner or virama into a space's cluster; such a space is not
/// whitespace on its own.
fn pure_char(text: &str, starts: &[usize], byte: usize) -> Option<char> {
    let c = char_at(text, byte)?;
    let next = byte + c.len_utf8();
    (next == text.len() || starts.binary_search(&next).is_ok()).then_some(c)
}

/// Hidden only when its cluster holds nothing else. A hidden separator merged
/// into a visible cluster keeps that cluster on the line.
fn is_hidden_at(text: &str, starts: &[usize], byte: usize) -> bool {
    matches!(
        pure_char(text, starts, byte),
        Some('\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{AD}' | '\u{2028}' | '\u{2029}')
    )
}

fn is_tab_at(text: &str, starts: &[usize], byte: usize) -> bool {
    pure_char(text, starts, byte) == Some('\t')
}

/// Space or tab: whitespace that hangs at the end of a line.
fn is_space_at(text: &str, starts: &[usize], byte: usize) -> bool {
    matches!(pure_char(text, starts, byte), Some(' ' | '\t'))
}

/// Clusters that start at each glyph, sorted and unique, plus the text length.
fn cluster_starts(runs: &[ShapedRun], text_len: usize) -> Vec<usize> {
    let mut starts: Vec<usize> = runs
        .iter()
        .flat_map(|r| r.glyphs.iter().map(|g| g.cluster_index))
        .collect();
    starts.push(text_len);
    starts.sort_unstable();
    starts.dedup();
    starts
}

/// Collects every break opportunity, sorted and unique by byte position.
fn break_opportunities(text: &str, hyphenator: Option<&dyn Hyphenator>) -> Vec<(usize, BreakKind)> {
    let mut out: Vec<(usize, BreakKind)> = linebreaks_explained(text)
        .map(|b| {
            let kind = match b.kind {
                BreakOpportunity::Mandatory => BreakKind::Mandatory,
                BreakOpportunity::Allowed => {
                    // A break after a soft hyphen draws a hyphen.
                    if text[..b.position].ends_with('\u{AD}') {
                        BreakKind::Hyphen
                    } else {
                        BreakKind::Allowed
                    }
                }
            };
            (b.position, kind)
        })
        .collect();

    if let Some(hyphenator) = hyphenator {
        let mut word_start = 0;
        let boundaries: Vec<usize> = out.iter().map(|(p, _)| *p).collect();
        for end in boundaries {
            if end > word_start {
                // Break boundaries fall after whitespace, so trim it before the
                // hyphenator sees the word. Offsets are unchanged, as the word
                // starts where the previous boundary was.
                let word = text[word_start..end].trim_end_matches(char::is_whitespace);
                for p in hyphenator.break_points(word) {
                    if p > 0 && p < word.len() && word.is_char_boundary(p) {
                        out.push((word_start + p, BreakKind::Hyphen));
                    }
                }
                word_start = end;
            }
        }
    }

    // A stable sort keeps UAX #14 entries ahead of hyphenation points at the
    // same position, so dedup keeps the UAX #14 kind.
    out.sort_by_key(|(p, _)| *p);
    out.dedup_by_key(|(p, _)| *p);
    out
}

fn flatten(
    runs: &[ShapedRun],
    run_levels: &[Level],
    text: &str,
    starts: &[usize],
    breaks: &[(usize, BreakKind)],
) -> Vec<GlyphRef> {
    let mut out: Vec<GlyphRef> = Vec::new();
    for (run_index, run) in runs.iter().enumerate() {
        let len = run.glyphs.len();
        // Logical order: right-to-left runs are stored visually.
        let order: Vec<usize> = if run_levels[run_index].is_rtl() {
            (0..len).rev().collect()
        } else {
            (0..len).collect()
        };
        for visual in order {
            let glyph = run.glyphs[visual];
            let cluster = glyph.cluster_index;
            let hidden = is_hidden_at(text, starts, cluster);
            let advance = if hidden || is_tab_at(text, starts, cluster) {
                Fx::ZERO
            } else {
                glyph.x_advance
            };

            let starts_cluster = out.last().map_or(true, |prev| prev.cluster != cluster);
            let brk = if starts_cluster {
                breaks
                    .binary_search_by_key(&cluster, |(p, _)| *p)
                    .ok()
                    .map(|i| {
                        let kind = breaks[i].1;
                        match (kind, out.last()) {
                            (BreakKind::Hyphen, Some(prev)) => {
                                let prev_run = &runs[prev.run];
                                Break {
                                    kind,
                                    hyphen_width: shape_run(
                                        &prev_run.font,
                                        "-",
                                        0..1,
                                        &prev_run.style,
                                        Direction::LeftToRight,
                                    )
                                    .width(),
                                    hyphen_run: prev.run,
                                }
                            }
                            _ => Break {
                                kind,
                                hyphen_width: Fx::ZERO,
                                hyphen_run: run_index,
                            },
                        }
                    })
            } else {
                None
            };

            out.push(GlyphRef {
                run: run_index,
                visual,
                cluster,
                advance,
                hidden,
                brk,
            });
        }
    }
    out
}

/// One line as a half-open range of flat glyph indices.
#[derive(Clone, Debug)]
struct LineSegment {
    range: Range<usize>,
    /// Run that draws a hyphen at the end of this line, if any.
    hyphen_run: Option<usize>,
}

/// Decides where lines end.
///
/// Each line is scanned forward from its start. Tabs are measured from the
/// line start, as `apply_tabs` will place them, and trailing whitespace does
/// not count towards the width. The last opportunity that still fits wins;
/// if none fits, the line breaks before the glyph that overflowed.
fn break_segments(
    glyphs: &[GlyphRef],
    text: &str,
    starts: &[usize],
    max_width: Fx,
    tab_stop: Fx,
) -> Vec<LineSegment> {
    let n = glyphs.len();
    let max_raw = i64::from(max_width.to_raw());
    let stop = i64::from(tab_stop.to_raw());

    let mut segments = Vec::new();
    let mut start = 0;
    while start < n {
        // Width of glyphs[start..j], and the same up to the last glyph that
        // is not whitespace. Only the latter counts for fitting.
        let mut x: i64 = 0;
        let mut visible: i64 = 0;
        let mut j = start;
        let mut best: Option<(usize, Option<usize>)> = None;

        let (end, hyphen_run) = 'scan: {
            while j < n {
                let g = &glyphs[j];
                if j > start {
                    if let Some(brk) = g.brk {
                        if brk.kind == BreakKind::Mandatory {
                            break 'scan (j, None);
                        }
                        let extra = match brk.kind {
                            BreakKind::Hyphen => i64::from(brk.hyphen_width.to_raw()),
                            _ => 0,
                        };
                        if visible + extra <= max_raw {
                            let run = (brk.kind == BreakKind::Hyphen).then_some(brk.hyphen_run);
                            best = Some((j, run));
                        }
                    }
                }

                x += advance_raw(g, text, starts, x, stop);
                if !g.hidden && !is_space_at(text, starts, g.cluster) {
                    visible = x;
                }
                if visible > max_raw {
                    break 'scan match best {
                        Some(found) => found,
                        // Nothing fits: break before the overflowing glyph, or
                        // after it if it is the only glyph on the line.
                        None => (if j > start { j } else { start + 1 }, None),
                    };
                }
                j += 1;
            }
            (n, None)
        };

        segments.push(LineSegment {
            range: start..end,
            hyphen_run,
        });
        start = end;
    }
    if segments.is_empty() {
        segments.push(LineSegment {
            range: 0..0,
            hyphen_run: None,
        });
    }
    segments
}

/// Advance of one glyph in raw units when the pen is at `x`.
fn advance_raw(g: &GlyphRef, text: &str, starts: &[usize], x: i64, stop: i64) -> i64 {
    if is_tab_at(text, starts, g.cluster) {
        if stop <= 0 {
            return 0;
        }
        return (x / stop + 1) * stop - x;
    }
    i64::from(g.advance.to_raw())
}

/// Clusters of the whitespace that ends one line, in logical order.
fn trailing_space_clusters(
    glyphs: &[GlyphRef],
    seg: &LineSegment,
    text: &str,
    starts: &[usize],
) -> Vec<usize> {
    glyphs[seg.range.clone()]
        .iter()
        .rev()
        .take_while(|g| g.hidden || is_space_at(text, starts, g.cluster))
        .filter(|g| !g.hidden)
        .map(|g| g.cluster)
        .collect()
}

/// Width of a line's glyphs, leaving out those on `hanging` clusters.
fn visible_width(runs: &[ShapedRun], hanging: &[usize]) -> Fx {
    runs.iter().fold(Fx::ZERO, |acc, r| {
        r.glyphs.iter().fold(acc, |acc, g| {
            if hanging.contains(&g.cluster_index) {
                acc
            } else {
                acc + g.x_advance
            }
        })
    })
}

/// Builds the runs for one line, in visual order, with hidden glyphs removed.
///
/// Applies rule L1 per line: the trailing whitespace of a wrapped line takes
/// the paragraph level, so it reorders as part of the paragraph rather than
/// with the run it was shaped in.
#[allow(clippy::too_many_arguments)]
fn build_line_runs(
    runs: &[ShapedRun],
    run_levels: &[Level],
    glyphs: &[GlyphRef],
    text: &str,
    seg: &LineSegment,
    para_level: Level,
    hanging: &[usize],
    starts: &[usize],
) -> Vec<ShapedRun> {
    // Pieces: consecutive logical glyphs from one run that share a level.
    struct Piece {
        run: usize,
        lo: usize,
        hi: usize,
        level: Level,
        hanging: bool,
    }
    let mut pieces: Vec<Piece> = Vec::new();
    for g in &glyphs[seg.range.clone()] {
        if g.hidden {
            continue;
        }
        let is_hanging = hanging.contains(&g.cluster);
        let level = if is_hanging {
            para_level
        } else {
            run_levels[g.run]
        };
        match pieces.last_mut() {
            Some(p) if p.run == g.run && p.hanging == is_hanging => {
                p.lo = p.lo.min(g.visual);
                p.hi = p.hi.max(g.visual);
            }
            _ => pieces.push(Piece {
                run: g.run,
                lo: g.visual,
                hi: g.visual,
                level,
                hanging: is_hanging,
            }),
        }
    }

    let levels: Vec<Level> = pieces.iter().map(|p| p.level).collect();
    let order = if levels.is_empty() {
        Vec::new()
    } else {
        BidiInfo::reorder_visual(&levels)
    };

    let mut out = Vec::new();
    for k in order {
        let piece = &pieces[k];
        let run = &runs[piece.run];
        let kept: Vec<ShapedGlyph> = run.glyphs[piece.lo..=piece.hi]
            .iter()
            .copied()
            .filter(|g| !is_hidden_at(text, starts, g.cluster_index))
            .collect();
        if kept.is_empty() {
            continue;
        }
        out.push(ShapedRun {
            font: Arc::clone(&run.font),
            style: run.style.clone(),
            glyphs: kept,
            direction: run.direction,
        });
    }
    out
}

/// The hyphen drawn at the end of a line that breaks at a hyphenation point.
///
/// It is added after tab stops are applied: its cluster is the byte that
/// starts the next line, which may be a tab.
fn hyphen_mark(runs: &[ShapedRun], seg: &LineSegment, glyphs: &[GlyphRef]) -> Option<ShapedRun> {
    let run = &runs[seg.hyphen_run?];
    let cluster = glyphs[seg.range.end].cluster;
    let mut hyphen = shaped_mark(&run.font, &run.style, "-", cluster);
    hyphen.direction = run.direction;
    Some(hyphen)
}

/// Byte range of text covered by a line.
fn line_text_range(glyphs: &[GlyphRef], seg: &LineSegment, text_len: usize) -> Range<usize> {
    let start = glyphs.get(seg.range.start).map_or(text_len, |g| g.cluster);
    let end = glyphs.get(seg.range.end).map_or(text_len, |g| g.cluster);
    start..end.max(start)
}

/// Snaps tab glyphs to the next tab stop, measured from the line start.
///
/// For right-to-left lines the line start is the right edge, so the stops are
/// measured from there and the glyphs are walked from right to left.
fn apply_tabs(runs: &mut [ShapedRun], text: &str, starts: &[usize], tab_stop: Fx, rtl: bool) {
    let stop = i64::from(tab_stop.to_raw());
    if stop <= 0 {
        return;
    }
    let mut x = Fx::ZERO;
    let snap = |g: &mut ShapedGlyph, x: &mut Fx| {
        if is_tab_at(text, starts, g.cluster_index) {
            let next = (i64::from(x.to_raw()) / stop + 1) * stop;
            g.x_advance = Fx::from_raw(next as i32) - *x;
        }
        *x += g.x_advance;
    };
    if rtl {
        for run in runs.iter_mut().rev() {
            for g in run.glyphs.iter_mut().rev() {
                snap(g, &mut x);
            }
        }
    } else {
        for run in runs.iter_mut() {
            for g in &mut run.glyphs {
                snap(g, &mut x);
            }
        }
    }
}

/// Spreads `extra` over the spaces on a line, or over the gaps between
/// glyphs if there are no spaces. Integer arithmetic keeps it deterministic:
/// the remainder goes to the first targets.
fn justify(runs: &mut [ShapedRun], text: &str, starts: &[usize], extra: Fx, hanging: &[usize]) {
    if extra <= Fx::ZERO {
        return;
    }
    let mut spaces: Vec<(usize, usize)> = Vec::new();
    let mut glyphs_all: Vec<(usize, usize)> = Vec::new();
    for (r, run) in runs.iter().enumerate() {
        for (g, glyph) in run.glyphs.iter().enumerate() {
            if hanging.contains(&glyph.cluster_index) {
                continue;
            }
            if pure_char(text, starts, glyph.cluster_index) == Some(' ') {
                spaces.push((r, g));
            }
            glyphs_all.push((r, g));
        }
    }
    let targets = if spaces.is_empty() {
        glyphs_all.pop(); // The final glyph has no gap after it.
        glyphs_all
    } else {
        spaces
    };
    if targets.is_empty() {
        return;
    }

    let count = targets.len() as i64;
    let raw = i64::from(extra.to_raw());
    let per = raw / count;
    let rem = raw % count;
    for (k, (r, g)) in targets.into_iter().enumerate() {
        let add = per + i64::from((k as i64) < rem);
        runs[r].glyphs[g].x_advance += Fx::from_raw(add as i32);
    }
}

/// Removes glyphs from the logical end of a line until `mark` fits, then adds
/// `mark` there. Whole clusters are removed, so no cluster is split, and
/// whitespace just before the mark is dropped. Returns the cluster of the last
/// glyph kept, or `None` if nothing is kept.
fn truncate_with_mark(
    runs: &mut Vec<ShapedRun>,
    text: &str,
    starts: &[usize],
    max_width: Fx,
    mark: ShapedRun,
    rtl: bool,
) -> Option<usize> {
    let mark_width = mark.width();
    while let Some(c) = end_cluster(runs, rtl) {
        if !is_space_at(text, starts, c) {
            break;
        }
        pop_cluster(runs, rtl);
    }
    while line_width(runs) + mark_width > max_width {
        if pop_cluster(runs, rtl).is_none() {
            break;
        }
    }
    let kept = end_cluster(runs, rtl);
    if rtl {
        runs.insert(0, mark);
    } else {
        runs.push(mark);
    }
    kept
}

/// Cluster of the glyph at the logical end of a line.
fn end_cluster(runs: &[ShapedRun], rtl: bool) -> Option<usize> {
    if rtl {
        runs.first()?.glyphs.first().map(|g| g.cluster_index)
    } else {
        runs.last()?.glyphs.last().map(|g| g.cluster_index)
    }
}

/// Removes every glyph of the cluster at the logical end of a line.
fn pop_cluster(runs: &mut Vec<ShapedRun>, rtl: bool) -> Option<usize> {
    let cluster = end_cluster(runs, rtl)?;
    loop {
        let next = end_cluster(runs, rtl);
        if next != Some(cluster) {
            break;
        }
        let run = if rtl {
            runs.first_mut()?
        } else {
            runs.last_mut()?
        };
        if rtl {
            run.glyphs.remove(0);
        } else {
            run.glyphs.pop();
        }
        if run.glyphs.is_empty() {
            if rtl {
                runs.remove(0);
            } else {
                runs.pop();
            }
        }
    }
    Some(cluster)
}

/// A paragraph placed inside a [`DocumentLayout`].
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct PlacedParagraph {
    /// Top edge of the paragraph in document coordinates.
    pub top: Fx,
    /// The paragraph's layout, positioned from its own top edge.
    pub layout: ParagraphLayout,
}

/// Several paragraphs stacked vertically.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct DocumentLayout {
    /// Paragraphs from top to bottom.
    pub paragraphs: Vec<PlacedParagraph>,
    /// Height of the stack, including spacing between paragraphs.
    pub total_height: Fx,
}

/// The options that a cached paragraph layout depends on.
#[derive(Clone, Debug, PartialEq)]
struct LayoutKey {
    width: Fx,
    base: TextStyle,
    align: Align,
    direction: Option<Direction>,
    line_spacing: Option<Fx>,
    max_lines: Option<usize>,
    ellipsis: bool,
    tab_stop: Option<Fx>,
}

struct DocParagraph {
    text: String,
    spans: Vec<StyleSpan>,
    cache: Option<ParagraphLayout>,
}

/// A list of paragraphs with incremental relayout.
///
/// Editing one paragraph with [`Document::set`] relays out only that
/// paragraph on the next [`Document::layout`]. Changing the width, base style
/// or a layout option relays out everything. Changes to the fallback resolver
/// or hyphenator are not detected; call [`Document::invalidate`] after one.
pub struct Document {
    paragraphs: Vec<DocParagraph>,
    spacing: Fx,
    key: Option<LayoutKey>,
    shapes: ShapeCache,
    last_relayout: usize,
}

impl Document {
    /// Creates an empty document with `spacing` between paragraphs.
    pub fn new(spacing: Fx) -> Document {
        Document {
            paragraphs: Vec::new(),
            spacing,
            key: None,
            shapes: ShapeCache::default(),
            last_relayout: 0,
        }
    }

    /// Number of paragraphs.
    pub fn len(&self) -> usize {
        self.paragraphs.len()
    }

    /// True if the document has no paragraphs.
    pub fn is_empty(&self) -> bool {
        self.paragraphs.is_empty()
    }

    /// Appends a paragraph and returns its index.
    pub fn push(&mut self, text: impl Into<String>, spans: Vec<StyleSpan>) -> usize {
        self.paragraphs.push(DocParagraph {
            text: text.into(),
            spans,
            cache: None,
        });
        self.paragraphs.len() - 1
    }

    /// Replaces paragraph `index`. Only that paragraph is relaid out next time.
    ///
    /// Returns `false` if `index` is out of range.
    pub fn set(&mut self, index: usize, text: impl Into<String>, spans: Vec<StyleSpan>) -> bool {
        match self.paragraphs.get_mut(index) {
            Some(p) => {
                p.text = text.into();
                p.spans = spans;
                p.cache = None;
                true
            }
            None => false,
        }
    }

    /// Discards every cached paragraph layout.
    pub fn invalidate(&mut self) {
        for p in &mut self.paragraphs {
            p.cache = None;
        }
    }

    /// Number of distinct shaping results cached for this document.
    pub fn shape_cache_len(&self) -> usize {
        self.shapes.len()
    }

    /// Number of paragraphs laid out by the most recent [`Document::layout`].
    pub fn last_relayout_count(&self) -> usize {
        self.last_relayout
    }

    /// Lays out every paragraph, reusing cached layouts where possible.
    pub fn layout(
        &mut self,
        provider: &dyn FontProvider,
        base: &TextStyle,
        max_width: Fx,
        options: &LayoutOptions<'_>,
    ) -> Result<DocumentLayout, LayoutError> {
        let key = LayoutKey {
            width: max_width,
            base: base.clone(),
            align: options.align,
            direction: options.direction,
            line_spacing: options.line_spacing,
            max_lines: options.max_lines,
            ellipsis: options.ellipsis,
            tab_stop: options.tab_stop,
        };
        if self.key.as_ref() != Some(&key) {
            self.invalidate();
            self.key = Some(key);
        }

        let mut relaid = 0;
        for p in &mut self.paragraphs {
            if p.cache.is_none() {
                p.cache = Some(layout_inner(
                    provider,
                    &p.text,
                    base,
                    &p.spans,
                    max_width,
                    options,
                    Some(&mut self.shapes),
                )?);
                relaid += 1;
            }
        }
        self.last_relayout = relaid;

        let mut top = Fx::ZERO;
        let mut paragraphs = Vec::with_capacity(self.paragraphs.len());
        for p in &self.paragraphs {
            // Every slot was filled above, so the cache is always present.
            let layout = p.cache.clone().expect("filled above");
            let height = layout.total_height;
            paragraphs.push(PlacedParagraph { top, layout });
            top += height + self.spacing;
        }
        if !paragraphs.is_empty() {
            top -= self.spacing;
        }
        Ok(DocumentLayout {
            paragraphs,
            total_height: top,
        })
    }
}

/// Why a line ended where it did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineBreakReason {
    /// Index of the line that ended.
    pub line: usize,
    /// Byte index where the next line starts.
    pub byte: usize,
    /// The UAX #14 rule that allowed or required the break, such as `"LB18"`.
    /// `"hyphenation"` marks a hyphen drawn at the break. `"overflow"` marks a
    /// break forced by a word wider than the line.
    pub rule: &'static str,
}

/// Explains each line end in `layout`, for debugging line breaks.
pub fn explain_line_breaks(layout: &ParagraphLayout) -> Vec<LineBreakReason> {
    let rules: BTreeMap<usize, &'static str> = linebreaks_explained(&layout.text)
        .map(|b| (b.position, b.rule))
        .collect();
    let count = layout.lines.len();
    let mut out = Vec::new();
    for (line, l) in layout
        .lines
        .iter()
        .enumerate()
        .take(count.saturating_sub(1))
    {
        let byte = l.text_range.end;
        let hyphen = l
            .runs
            .iter()
            .flat_map(|r| r.glyphs.iter())
            .any(|g| g.cluster_index == byte);
        let rule = if hyphen {
            "hyphenation"
        } else {
            rules.get(&byte).copied().unwrap_or("overflow")
        };
        out.push(LineBreakReason { line, byte, rule });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_natural_height() {
        let m = Metrics {
            ascent: Fx::from_int(10),
            descent: Fx::from_int(3),
            gap: Fx::from_int(1),
        };
        assert_eq!(m.natural(), Fx::from_int(14));
    }

    #[test]
    fn hidden_characters_are_recognised() {
        let text = "a\nb\u{AD}c\t";
        assert!(is_hidden(text, 1));
        assert!(is_hidden(text, 3));
        assert!(!is_hidden(text, 0));
        assert!(is_tab_at(text, &[], 6));
    }

    #[test]
    fn start_and_end_follow_direction() {
        assert_eq!(resolve_align(Align::Start, false), Align::Left);
        assert_eq!(resolve_align(Align::Start, true), Align::Right);
        assert_eq!(resolve_align(Align::End, false), Align::Right);
        assert_eq!(resolve_align(Align::End, true), Align::Left);
        assert_eq!(resolve_align(Align::Center, true), Align::Center);
    }
}
