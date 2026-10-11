//! Human-readable snapshots of [`ParagraphLayout`]s.
//!
//! [`describe`] writes one line per glyph run with positions, so a snapshot
//! diff shows exactly which glyph moved. [`check`] compares a layout with a
//! snapshot file. Set `TPT_SNAPSHOT_UPDATE=1` to write the file instead.

use std::fmt::Write;
use std::fs;
use std::path::Path;

use tpt_text::ParagraphLayout;

/// Renders `layout` as stable, line-oriented text.
pub fn describe(layout: &ParagraphLayout) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "paragraph direction={:?} width={} height={} digest={:016x}",
        layout.direction,
        layout.max_width.to_raw(),
        layout.total_height.to_raw(),
        layout.digest()
    );
    for (i, line) in layout.lines.iter().enumerate() {
        let _ = writeln!(
            out,
            "line {i} bytes={}..{} top={} baseline={} x={} height={}",
            line.text_range.start,
            line.text_range.end,
            line.top.to_raw(),
            line.baseline_y.to_raw(),
            line.x_offset.to_raw(),
            line.height.to_raw()
        );
        for run in &line.runs {
            let _ = writeln!(out, "  run {:?}", run.direction);
            for g in &run.glyphs {
                let _ = writeln!(
                    out,
                    "    glyph={} cluster={} advance={} dx={} dy={}",
                    g.glyph_id,
                    g.cluster_index,
                    g.x_advance.to_raw(),
                    g.x_offset.to_raw(),
                    g.y_offset.to_raw()
                );
            }
        }
    }
    for m in &layout.missing_glyphs {
        let _ = writeln!(out, "missing byte={} char={:?}", m.byte, m.ch);
    }
    out
}

/// Compares `layout` with the snapshot at `path`.
///
/// Panics with the first differing line when they disagree, or when the file
/// is missing and `TPT_SNAPSHOT_UPDATE` is not set.
pub fn check(path: impl AsRef<Path>, layout: &ParagraphLayout) {
    let path = path.as_ref();
    let actual = describe(layout);
    if std::env::var("TPT_SNAPSHOT_UPDATE").is_ok_and(|v| v == "1") {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).unwrap_or_else(|e| panic!("creating {}: {e}", dir.display()));
        }
        fs::write(path, &actual).unwrap_or_else(|e| panic!("writing {}: {e}", path.display()));
        return;
    }
    let expected = fs::read_to_string(path).unwrap_or_else(|_| {
        panic!(
            "missing snapshot {}; run with TPT_SNAPSHOT_UPDATE=1",
            path.display()
        )
    });
    if expected != actual {
        let first = expected
            .lines()
            .zip(actual.lines())
            .enumerate()
            .find(|(_, (e, a))| e != a)
            .map(|(i, (e, a))| format!("line {}:\n  expected: {e}\n  actual:   {a}", i + 1))
            .unwrap_or_else(|| "one snapshot is a prefix of the other".to_string());
        panic!("snapshot {} differs at {first}", path.display());
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tpt_text::{FontRegistry, Fx, TextStyle};

    #[test]
    fn describe_is_stable_for_the_same_layout() {
        let reg = FontRegistry::new();
        reg.load_from_memory(
            include_bytes!("../../tpt-text/tests/fonts/NotoSans-Regular.ttf").to_vec(),
            0,
            "Noto Sans",
            400,
            false,
        )
        .unwrap();
        let style = TextStyle::new("Noto Sans", Fx::from_int(12));
        let a = tpt_text::layout_text(&reg, "snapshot me", &style, Fx::from_int(200)).unwrap();
        let b = tpt_text::layout_text(&reg, "snapshot me", &style, Fx::from_int(200)).unwrap();
        assert_eq!(describe(&a), describe(&b));
        assert!(describe(&a).starts_with("paragraph direction=LeftToRight"));
    }
}
