//! Determinism check shared by native tests and the WebAssembly build.
//!
//! [`digest_case`] lays out a paragraph and folds every position into an
//! FNV-1a hash, always writing little-endian integers. The same inputs must
//! produce the same digest on every target. `tests/wasm_digests.rs` compares
//! native digests with values recorded from the WebAssembly build under Node
//! (see `scripts/wasm-digest.mjs`).

use std::slice;

use tpt_text::{
    layout_paragraph_with, Align, FallbackResolver, FontRegistry, Fx, LayoutOptions,
    ParagraphLayout, TextStyle,
};

/// A paragraph to lay out, with its expected name in the digest file.
pub struct Case {
    /// Name used as the key in the digest file.
    pub name: &'static str,
    /// Paragraph text.
    pub text: &'static str,
    /// Width limit in whole layout pixels.
    pub width: i32,
    /// Alignment: 0 left, 1 right, 2 center, 3 justify.
    pub align: u32,
}

/// Cases covering shaping, Bidi, fallback, justification, and long text.
pub const CASES: &[Case] = &[
    Case {
        name: "latin_wrap",
        text: "Deterministic layout means the same glyphs and positions on every platform, \
               so PDFs and CAD drawings agree byte for byte.",
        width: 240,
        align: 0,
    },
    Case {
        name: "latin_justify",
        text: "one two three four five six seven eight nine ten eleven twelve thirteen",
        width: 200,
        align: 3,
    },
    Case {
        name: "arabic_numbers_right",
        text: "\u{0628}\u{062A}\u{0628} 2026 \u{062A}\u{0628}",
        width: 400,
        align: 1,
    },
    Case {
        name: "mixed_fallback_center",
        text: "Invoice \u{0628}\u{062A}\u{0628} total 2026 is due on receipt.",
        width: 260,
        align: 2,
    },
    Case {
        name: "long_paragraph",
        text: "lorem ipsum dolor sit amet consectetur ",
        width: 480,
        align: 3,
    },
];

fn align_from(code: u32) -> Align {
    match code {
        1 => Align::Right,
        2 => Align::Center,
        3 => Align::Justify,
        _ => Align::Left,
    }
}

/// Lays out `case` with the given fonts and returns its digest.
///
/// `latin` is the primary face; `arabic` is the fallback face. Both are
/// registered in a fresh registry on every call.
pub fn digest_case(latin: &[u8], arabic: &[u8], case: &Case) -> Result<u64, String> {
    let registry = FontRegistry::new();
    registry
        .load_from_memory(latin.to_vec(), 0, "Noto Sans", 400, false)
        .map_err(|e| e.to_string())?;
    registry
        .load_from_memory(arabic.to_vec(), 0, "Noto Sans Arabic", 400, false)
        .map_err(|e| e.to_string())?;

    let resolver = FallbackResolver::new().with_defaults(&["Noto Sans Arabic"]);
    let options = LayoutOptions {
        align: align_from(case.align),
        fallback: Some(&resolver),
        ..LayoutOptions::default()
    };
    let style = TextStyle::new("Noto Sans", Fx::from_int(14));
    let layout = layout_paragraph_with(
        &registry,
        case.text,
        &style,
        &[],
        Fx::from_int(case.width),
        &options,
    )
    .map_err(|e| e.to_string())?;
    Ok(digest(&layout))
}

/// FNV-1a over every line and glyph position, little-endian.
pub fn digest(layout: &ParagraphLayout) -> u64 {
    let mut h = Fnv::new();
    h.u64(layout.lines.len() as u64);
    h.i32(layout.total_height.to_raw());
    for line in &layout.lines {
        h.i32(line.top.to_raw());
        h.i32(line.baseline_y.to_raw());
        h.i32(line.height.to_raw());
        h.i32(line.x_offset.to_raw());
        h.u64(line.text_range.start as u64);
        h.u64(line.text_range.end as u64);
        h.u64(line.runs.len() as u64);
        for run in &line.runs {
            h.u64(run.glyphs.len() as u64);
            for g in &run.glyphs {
                h.u64(u64::from(g.glyph_id));
                h.i32(g.x_advance.to_raw());
                h.i32(g.y_advance.to_raw());
                h.i32(g.x_offset.to_raw());
                h.i32(g.y_offset.to_raw());
                h.u64(g.cluster_index as u64);
            }
        }
    }
    h.finish()
}

struct Fnv(u64);

impl Fnv {
    fn new() -> Fnv {
        Fnv(0xcbf2_9ce4_8422_2325)
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }

    fn u64(&mut self, v: u64) {
        self.bytes(&v.to_le_bytes());
    }

    fn i32(&mut self, v: i32) {
        self.bytes(&v.to_le_bytes());
    }

    fn finish(self) -> u64 {
        self.0
    }
}

// ---------------------------------------------------------------------------
// C ABI for the WebAssembly build. Memory is handed to the host with
// `tpt_alloc`. The host process is short-lived, so buffers are not freed.

/// Allocates `len` bytes for the host to write into. The buffer is leaked on
/// purpose; this crate is a test harness.
#[no_mangle]
pub extern "C" fn tpt_alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::<u8>::with_capacity(len.max(1));
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// Digest for case `index` in [`CASES`], or `u64::MAX` if the index is out of
/// range or the fonts fail to load.
///
/// # Safety
/// Both font pointers must be valid for reads of their lengths.
#[no_mangle]
pub unsafe extern "C" fn tpt_digest(
    index: u32,
    latin_ptr: *const u8,
    latin_len: usize,
    arabic_ptr: *const u8,
    arabic_len: usize,
) -> u64 {
    let Some(case) = CASES.get(index as usize) else {
        return u64::MAX;
    };
    let latin = slice::from_raw_parts(latin_ptr, latin_len);
    let arabic = slice::from_raw_parts(arabic_ptr, arabic_len);
    digest_case(latin, arabic, case).unwrap_or(u64::MAX)
}

/// Number of cases in [`CASES`], so the host can check its own list.
#[no_mangle]
pub extern "C" fn tpt_case_count() -> u32 {
    CASES.len() as u32
}
