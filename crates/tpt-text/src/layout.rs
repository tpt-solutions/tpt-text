//! Paragraph layout: Bidi, font fallback, UAX #14 breaking with hyphenation,
//! tab stops, alignment, justification and truncation.

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt;
use core::ops::Range;

use unicode_bidi::{BidiInfo, Level};
use unicode_linebreak::{linebreaks, BreakOpportunity};
use unicode_script::Script;

use crate::fallback::FallbackResolver;
use crate::font::{FontFace, FontProvider, FontQuery};
use crate::fx::{scale_round, Fx};
use crate::hyphenate::Hyphenator;
use crate::itemize::{itemize, style_table, Item};
use crate::shape::{shape_run, Direction, ShapedGlyph, ShapedRun};
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

/// Horizontal alignment of lines within the width limit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    /// Flush left.
    #[default]
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
#[derive(Default)]
pub struct LayoutOptions<'a> {
    /// Paragraph direction. `None` detects it from the first strong character.
    pub direction: Option<Direction>,
    /// Horizontal alignment.
    pub align: Align,
    /// Line height as a multiple of the natural height. `None` means `1`.
    pub line_spacing: Option<Fx>,
    /// Maximum number of lines. `Some(0)` is treated as `Some(1)`.
    pub max_lines: Option<usize>,
    /// When truncated by `max_lines`, end the last line with an ellipsis.
    pub ellipsis: bool,
    /// Tab-stop interval, measured from the line start. `None` means four em.
    pub tab_stop: Option<Fx>,
    /// Caller-configured fallback families.
    pub fallback: Option<&'a FallbackResolver>,
    /// Hyphenation points for word-internal breaks.
    pub hyphenator: Option<&'a dyn Hyphenator>,
}

/// One line of a laid-out paragraph.
#[derive(Clone, Debug)]
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

/// A fully laid-out paragraph.
#[derive(Clone, Debug)]
pub struct ParagraphLayout {
    /// Lines from top to bottom.
    pub lines: Vec<LayoutLine>,
    /// Sum of all line heights.
    pub total_height: Fx,
    /// The width limit used for breaking.
    pub max_width: Fx,
    /// The laid-out text, kept for hit-testing.
    pub text: String,
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

/// Lays out `text` with explicit [`LayoutOptions`].
pub fn layout_paragraph_with(
    provider: &dyn FontProvider,
    text: &str,
    base: &TextStyle,
    spans: &[StyleSpan],
    max_width: Fx,
    options: &LayoutOptions<'_>,
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
    let items = itemize(text, spans, &levels);
    let (fonts, segments) = resolve_segments(provider, options.fallback, text, &items, &styles)?;

    let runs: Vec<ShapedRun> = segments
        .iter()
        .map(|s| {
            shape_run(
                &fonts[s.font],
                text,
                s.range.clone(),
                styles[s.style_index],
                direction_of(s.level),
            )
        })
        .collect();
    let run_levels: Vec<Level> = segments.iter().map(|s| s.level).collect();

    let breaks = break_opportunities(text, options.hyphenator);
    let glyphs = flatten(&runs, &run_levels, text, &breaks);
    let mut line_segments = break_segments(&glyphs, max_width);

    let max_lines = options.max_lines.unwrap_or(usize::MAX).max(1);
    let truncated = line_segments.len() > max_lines;
    line_segments.truncate(max_lines);

    let tab_stop = options
        .tab_stop
        .unwrap_or_else(|| base.size * Fx::from_int(4));
    let line_spacing = options.line_spacing.unwrap_or(Fx::ONE);
    let base_metrics = Metrics::of(&base_face, base.size);

    let mut lines = Vec::with_capacity(line_segments.len());
    let mut top = Fx::ZERO;
    let count = line_segments.len();
    for (idx, seg) in line_segments.iter().enumerate() {
        let mut line_runs = build_line_runs(&runs, &run_levels, &glyphs, text, seg);
        apply_tabs(&mut line_runs, text, tab_stop);

        let is_last_of_paragraph = seg.range.end >= glyphs.len()
            || glyphs[seg.range.end]
                .brk
                .is_some_and(|b| b.kind == BreakKind::Mandatory);
        let is_last_line = idx + 1 == count;

        let mut width = line_width(&line_runs);
        if truncated && is_last_line && options.ellipsis {
            let (font, style) = match line_runs.last() {
                Some(run) => (Arc::clone(&run.font), run.style.clone()),
                None => (Arc::clone(&base_face), base.clone()),
            };
            let mark = shaped_mark(&font, &style, "\u{2026}", text.len());
            truncate_with_mark(&mut line_runs, max_width, mark);
            width = line_width(&line_runs);
        } else if options.align == Align::Justify && !is_last_of_paragraph && !is_last_line {
            justify(&mut line_runs, text, max_width - width);
            width = max_width;
        }

        let x_offset = match options.align {
            Align::Left | Align::Justify => Fx::ZERO,
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
            text_range: line_text_range(&glyphs, seg, text.len()),
        });
        top += height;
    }

    Ok(ParagraphLayout {
        lines,
        total_height: top,
        max_width,
        text: String::from(text),
    })
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

/// Per-byte levels after UAX #9 rule L1, which resets segment separators
/// (tabs) and trailing whitespace to the paragraph level. Each paragraph is
/// treated as one line, so whitespace at the end of a wrapped line is not
/// reset here.
/// Resolved per-byte Bidi levels for `bidi`, with rule L1 applied.
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

    for item in items {
        let style = styles[item.style_index];
        let primary = load_face(provider, style)?;
        let primary_index = intern(&mut fonts, &primary);
        let mut candidates: Option<Vec<Arc<FontFace>>> = None;

        for (rel, c) in text[item.range.clone()].char_indices() {
            let start = item.range.start + rel;
            let end = start + c.len_utf8();
            let chosen = if c.is_control() || primary.has_glyph(c) {
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

fn is_hidden(text: &str, byte: usize) -> bool {
    matches!(char_at(text, byte), Some('\n' | '\r' | '\u{AD}'))
}

fn is_tab(text: &str, byte: usize) -> bool {
    char_at(text, byte) == Some('\t')
}

/// Collects every break opportunity, sorted and unique by byte position.
fn break_opportunities(text: &str, hyphenator: Option<&dyn Hyphenator>) -> Vec<(usize, BreakKind)> {
    let mut out: Vec<(usize, BreakKind)> = linebreaks(text)
        .map(|(pos, op)| {
            let kind = match op {
                BreakOpportunity::Mandatory => BreakKind::Mandatory,
                BreakOpportunity::Allowed => {
                    // A break after a soft hyphen draws a hyphen.
                    if text[..pos].ends_with('\u{AD}') {
                        BreakKind::Hyphen
                    } else {
                        BreakKind::Allowed
                    }
                }
            };
            (pos, kind)
        })
        .collect();

    if let Some(hyphenator) = hyphenator {
        let mut word_start = 0;
        let boundaries: Vec<usize> = out.iter().map(|(p, _)| *p).collect();
        for end in boundaries {
            if end > word_start {
                let word = &text[word_start..end];
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
            let hidden = is_hidden(text, cluster);
            let advance = if hidden || is_tab(text, cluster) {
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
fn break_segments(glyphs: &[GlyphRef], max_width: Fx) -> Vec<LineSegment> {
    let n = glyphs.len();

    // prefix[i] is the total advance of glyphs[..i], in raw units. Kept as
    // i64 so long paragraphs cannot saturate the 16.16 range.
    let mut prefix: Vec<i64> = Vec::with_capacity(n + 1);
    prefix.push(0);
    for g in glyphs {
        let last = prefix[prefix.len() - 1];
        prefix.push(last + i64::from(g.advance.to_raw()));
    }

    let mut segments = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < n {
        if let Some(b) = glyphs[i].brk {
            if b.kind == BreakKind::Mandatory && i > start {
                segments.push(LineSegment {
                    range: start..i,
                    hyphen_run: None,
                });
                start = i;
            }
        }

        let max_raw = i64::from(max_width.to_raw());
        let width = prefix[i + 1] - prefix[start];
        if width > max_raw && i > start {
            // Find the last opportunity that still fits, scanning backwards.
            let fitting = (start + 1..=i).rev().find(|&b| {
                glyphs[b].brk.is_some_and(|brk| {
                    let extra = if brk.kind == BreakKind::Hyphen {
                        brk.hyphen_width
                    } else {
                        Fx::ZERO
                    };
                    prefix[b] - prefix[start] + i64::from(extra.to_raw()) <= max_raw
                })
            });
            match fitting {
                Some(b) => {
                    let hyphen_run = glyphs[b].brk.and_then(|brk| match brk.kind {
                        BreakKind::Hyphen => Some(brk.hyphen_run),
                        _ => None,
                    });
                    segments.push(LineSegment {
                        range: start..b,
                        hyphen_run,
                    });
                    start = b;
                }
                None => {
                    // No opportunity fits: break before glyph `i` anyway.
                    segments.push(LineSegment {
                        range: start..i,
                        hyphen_run: None,
                    });
                    start = i;
                }
            }
            // Re-examine glyph `i` against the new line start.
            continue;
        }
        i += 1;
    }
    if start < n || segments.is_empty() {
        segments.push(LineSegment {
            range: start..n,
            hyphen_run: None,
        });
    }
    segments
}

/// Builds the runs for one line, in visual order, with hidden glyphs removed.
fn build_line_runs(
    runs: &[ShapedRun],
    run_levels: &[Level],
    glyphs: &[GlyphRef],
    text: &str,
    seg: &LineSegment,
) -> Vec<ShapedRun> {
    // Visual index span of each run's glyphs that fall on this line.
    let mut spans: Vec<Option<(usize, usize)>> = alloc::vec![None; runs.len()];
    for g in &glyphs[seg.range.clone()] {
        if g.hidden {
            continue;
        }
        spans[g.run] = Some(match spans[g.run] {
            None => (g.visual, g.visual),
            Some((lo, hi)) => (lo.min(g.visual), hi.max(g.visual)),
        });
    }

    let present: Vec<usize> = (0..runs.len()).filter(|&r| spans[r].is_some()).collect();
    let levels: Vec<Level> = present.iter().map(|&r| run_levels[r]).collect();
    let order = if levels.is_empty() {
        Vec::new()
    } else {
        BidiInfo::reorder_visual(&levels)
    };

    let mut out = Vec::new();
    for k in order {
        let r = present[k];
        let Some((lo, hi)) = spans[r] else { continue };
        let run = &runs[r];
        let kept: Vec<ShapedGlyph> = run.glyphs[lo..=hi]
            .iter()
            .copied()
            .filter(|g| !is_hidden(text, g.cluster_index))
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

    if let Some(r) = seg.hyphen_run {
        let run = &runs[r];
        let cluster = glyphs[seg.range.end].cluster;
        let mut hyphen = shaped_mark(&run.font, &run.style, "-", cluster);
        hyphen.direction = run.direction;
        out.push(hyphen);
    }
    out
}

/// Byte range of text covered by a line.
fn line_text_range(glyphs: &[GlyphRef], seg: &LineSegment, text_len: usize) -> Range<usize> {
    let start = glyphs.get(seg.range.start).map_or(text_len, |g| g.cluster);
    let end = glyphs.get(seg.range.end).map_or(text_len, |g| g.cluster);
    start..end.max(start)
}

/// Snaps tab glyphs to the next tab stop, measured from the line start.
fn apply_tabs(runs: &mut [ShapedRun], text: &str, tab_stop: Fx) {
    let stop = i64::from(tab_stop.to_raw());
    if stop <= 0 {
        return;
    }
    let mut x = Fx::ZERO;
    for run in runs {
        for g in &mut run.glyphs {
            if is_tab(text, g.cluster_index) {
                let next = (i64::from(x.to_raw()) / stop + 1) * stop;
                g.x_advance = Fx::from_raw(next as i32) - x;
            }
            x += g.x_advance;
        }
    }
}

/// Spreads `extra` over the spaces on a line, or over the gaps between
/// glyphs if there are no spaces. Integer arithmetic keeps it deterministic:
/// the remainder goes to the first targets.
fn justify(runs: &mut [ShapedRun], text: &str, extra: Fx) {
    if extra <= Fx::ZERO {
        return;
    }
    let mut spaces: Vec<(usize, usize)> = Vec::new();
    let mut glyphs_all: Vec<(usize, usize)> = Vec::new();
    for (r, run) in runs.iter().enumerate() {
        for (g, glyph) in run.glyphs.iter().enumerate() {
            if char_at(text, glyph.cluster_index) == Some(' ') {
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

/// Drops glyphs from the end of a line until `mark` fits, then appends `mark`.
fn truncate_with_mark(runs: &mut Vec<ShapedRun>, max_width: Fx, mark: ShapedRun) {
    let mark_width = mark.width();
    while line_width(runs) + mark_width > max_width {
        let Some(last) = runs.last_mut() else { break };
        last.glyphs.pop();
        if last.glyphs.is_empty() {
            runs.pop();
        }
    }
    runs.push(mark);
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
        assert!(is_tab(text, 6));
    }
}
