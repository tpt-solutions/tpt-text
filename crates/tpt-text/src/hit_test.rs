//! Cursor and selection queries over a [`ParagraphLayout`].
//!
//! All queries work in visual order, so mixed-direction text is handled. A
//! caret's byte index is always a character boundary. Ligatures and clusters
//! are treated as one unit, but grapheme clusters are split at character
//! boundaries.

use alloc::vec::Vec;
use core::ops::Range;

use crate::fx::Fx;
use crate::layout::{LayoutLine, ParagraphLayout};
use crate::shape::Direction;

/// A caret position in the paragraph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Caret {
    /// Byte index in the paragraph text where the caret sits.
    pub byte_index: usize,
    /// Horizontal position from the paragraph's left edge.
    pub x: Fx,
    /// Top of the line containing the caret.
    pub y: Fx,
    /// Height of the line containing the caret.
    pub height: Fx,
    /// Index of the line containing the caret.
    pub line: usize,
}

/// An axis-aligned rectangle in paragraph coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    /// Left edge.
    pub x: Fx,
    /// Top edge.
    pub y: Fx,
    /// Width.
    pub width: Fx,
    /// Height.
    pub height: Fx,
}

/// One glyph's horizontal extent on a line.
#[derive(Clone, Copy, Debug)]
struct Cell {
    cluster: usize,
    rtl: bool,
    x0: Fx,
    x1: Fx,
}

/// Glyph extents in visual order, including alignment offset.
///
/// Hyphen and ellipsis marks are left out: their cluster is the byte where the
/// line ends, which is not a position inside this line.
fn cells(line: &LayoutLine) -> Vec<Cell> {
    let mut out = Vec::new();
    let mut x = line.x_offset;
    let end = line.text_range.end;
    for run in &line.runs {
        let rtl = run.direction == Direction::RightToLeft;
        for g in &run.glyphs {
            let x1 = x + g.x_advance;
            // A mark still takes up space, so the pen moves past it.
            if g.cluster_index < end {
                out.push(Cell {
                    cluster: g.cluster_index,
                    rtl,
                    x0: x,
                    x1,
                });
            }
            x = x1;
        }
    }
    out
}

/// Sorted, unique cluster starts of every glyph, followed by the text length.
fn cluster_starts(layout: &ParagraphLayout) -> Vec<usize> {
    let mut starts: Vec<usize> = layout
        .lines
        .iter()
        .flat_map(|l| l.runs.iter())
        .flat_map(|r| r.glyphs.iter().map(|g| g.cluster_index))
        .collect();
    starts.push(layout.text.len());
    starts.sort_unstable();
    starts.dedup();
    starts
}

/// Byte index a caret at the cell's left (`left = true`) or right edge maps to.
///
/// The right edge of a cell is the end of its whole cluster, so a caret never
/// lands between a base character and the marks that belong to it.
fn edge_byte(cell: &Cell, starts: &[usize], left: bool) -> usize {
    // Logical start sits on the left edge for LTR and the right edge for RTL.
    let at_start = left != cell.rtl;
    if at_start {
        cell.cluster
    } else {
        let next = starts.partition_point(|&s| s <= cell.cluster);
        starts.get(next).copied().unwrap_or(cell.cluster)
    }
}

fn line_range(layout: &ParagraphLayout, line: usize) -> Caret {
    let l = &layout.lines[line];
    Caret {
        byte_index: l.text_range.start,
        x: l.x_offset,
        y: l.top,
        height: l.height,
        line,
    }
}

/// Finds the caret nearest to the point `(x, y)`.
///
/// Points above the paragraph map to the first line and points below it to
/// the last. Returns `None` for an empty layout.
pub fn hit_test(layout: &ParagraphLayout, x: Fx, y: Fx) -> Option<Caret> {
    if layout.lines.is_empty() {
        return None;
    }
    let line_index = layout
        .lines
        .iter()
        .position(|l| y < l.top + l.height)
        .unwrap_or(layout.lines.len() - 1);
    let line = &layout.lines[line_index];
    let starts = cluster_starts(layout);

    let mut best: Option<(Fx, Caret)> = None;
    for cell in cells(line) {
        for (left, cx) in [(true, cell.x0), (false, cell.x1)] {
            let distance = if x >= cx { x - cx } else { cx - x };
            let caret = Caret {
                byte_index: edge_byte(&cell, &starts, left),
                x: cx,
                y: line.top,
                height: line.height,
                line: line_index,
            };
            if best.map_or(true, |(d, _)| distance < d) {
                best = Some((distance, caret));
            }
        }
    }
    Some(best.map_or_else(|| line_range(layout, line_index), |(_, c)| c))
}

/// Returns the caret for a byte index.
///
/// If the byte has no glyph of its own (for example a hidden newline), the
/// caret goes after the nearest preceding glyph on its line. Returns `None`
/// if `byte` is past the end of the text.
pub fn caret_for_byte(layout: &ParagraphLayout, byte: usize) -> Option<Caret> {
    if byte > layout.text.len() || layout.lines.is_empty() {
        return None;
    }
    let line_index = layout
        .lines
        .iter()
        .position(|l| byte < l.text_range.end)
        .unwrap_or(layout.lines.len() - 1);
    let line = &layout.lines[line_index];
    let cells = cells(line);

    let x = if let Some(cell) = cells.iter().find(|c| c.cluster == byte) {
        // Leading edge of the cluster.
        if cell.rtl {
            cell.x1
        } else {
            cell.x0
        }
    } else if let Some(cell) = cells.iter().rev().find(|c| c.cluster < byte) {
        // Trailing edge of the nearest preceding glyph.
        if cell.rtl {
            cell.x0
        } else {
            cell.x1
        }
    } else {
        line.x_offset
    };

    Some(Caret {
        byte_index: byte,
        x,
        y: line.top,
        height: line.height,
        line: line_index,
    })
}

/// Rectangles covering the glyphs whose clusters lie in `range`, one per
/// contiguous visual stretch per line.
pub fn selection_rects(layout: &ParagraphLayout, range: Range<usize>) -> Vec<Rect> {
    let mut rects = Vec::new();
    if range.start >= range.end {
        return rects;
    }
    for line in &layout.lines {
        let mut current: Option<(Fx, Fx)> = None;
        for cell in cells(line) {
            let selected = cell.cluster >= range.start && cell.cluster < range.end;
            if selected {
                current = Some(match current {
                    None => (cell.x0, cell.x1),
                    Some((a, b)) => (a.min(cell.x0), b.max(cell.x1)),
                });
            } else if let Some((a, b)) = current.take() {
                rects.push(rect(a, b, line));
            }
        }
        if let Some((a, b)) = current {
            rects.push(rect(a, b, line));
        }
    }
    rects
}

fn rect(a: Fx, b: Fx, line: &LayoutLine) -> Rect {
    Rect {
        x: a,
        y: line.top,
        width: b - a,
        height: line.height,
    }
}
