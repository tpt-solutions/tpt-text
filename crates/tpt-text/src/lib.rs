//! tpt-text: deterministic text shaping and layout.
//!
//! The crate orchestrates pure-Rust primitives (rustybuzz, tpt-text-linebreak,
//! unicode-script) behind a fixed-point, integer-only pipeline so the same
//! input produces byte-identical output on every platform.
//!
//! The core is `no_std` and needs only `alloc`. The `std` feature is reserved
//! for OS font discovery.
//!
//! # Status
//!
//! Phases 1 to 3 are implemented: fixed-point metrics, font registry,
//! Bidi-aware itemization and shaping, caller-configured fallback, UAX #14
//! breaking with hyphenation hooks, tab stops, alignment, justification,
//! truncation, and hit-testing. Phase 4 (ecosystem integration) was dropped.
//! Correctness fixes from Phase 5 are in progress; see `TODO.md`.
//!
//! # Quick start
//!
//! ```no_run
//! use tpt_text::{layout_paragraph, FontRegistry, Fx, TextStyle};
//!
//! let font = std::fs::read("NotoSans-Regular.ttf").expect("font file");
//! let registry = FontRegistry::new();
//! registry
//!     .load_from_memory(font, 0, "Noto Sans", 400, false)
//!     .expect("valid font");
//!
//! let style = TextStyle::new("Noto Sans", Fx::from_int(12));
//! let layout = layout_paragraph(&registry, "Hello, world", &style, &[], Fx::from_int(200))
//!     .expect("font is registered");
//! for line in &layout.lines {
//!     println!("line at y={} holds {} runs", line.top, line.runs.len());
//! }
//! ```
//!
//! Invalid font bytes are rejected rather than panicking:
//!
//! ```
//! use tpt_text::{FontError, FontRegistry};
//!
//! let registry = FontRegistry::new();
//! let err = registry
//!     .load_from_memory(vec![0; 16], 0, "Broken", 400, false)
//!     .unwrap_err();
//! assert_eq!(err, FontError::InvalidFontData);
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

extern crate alloc;

pub mod fallback;
pub mod font;
pub mod fx;
pub mod hit_test;
pub mod hyphenate;
pub mod layout;
pub mod outline;
pub mod shape;
pub mod style;
pub mod svg;
#[cfg(feature = "fontdb")]
pub mod system;

mod itemize;

pub use fallback::FallbackResolver;
pub use font::{FontError, FontFace, FontProvider, FontQuery, FontRegistry};
pub use fx::Fx;
pub use hyphenate::Hyphenator;
pub use layout::{
    explain_line_breaks, layout_paragraph, layout_paragraph_with, layout_text, Align, Document,
    DocumentLayout, LayoutError, LayoutLine, LayoutOptions, LineBreakReason, MissingGlyph,
    ParagraphLayout, PlacedParagraph,
};
pub use outline::PathSegment;
pub use shape::{Direction, ShapeCache, ShapedGlyph, ShapedRun};
pub use style::{ColorId, FontFeature, FontVariation, StyleSpan, TextStyle};
pub use unicode_bidi::Level;
pub use unicode_script::Script;
