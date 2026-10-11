//! tpt-text: deterministic text shaping and layout.
//!
//! The crate orchestrates pure-Rust primitives (rustybuzz, unicode-linebreak,
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
//! truncation, and hit-testing. Phase 4 integrations are pending.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

extern crate alloc;

pub mod fallback;
pub mod font;
pub mod fx;
pub mod hit_test;
pub mod hyphenate;
pub mod layout;
pub mod shape;
pub mod style;
#[cfg(feature = "fontdb")]
pub mod system;

mod itemize;

pub use fallback::FallbackResolver;
pub use font::{FontError, FontFace, FontProvider, FontQuery, FontRegistry};
pub use fx::Fx;
pub use hyphenate::Hyphenator;
pub use layout::{
    layout_paragraph, layout_paragraph_with, Align, LayoutError, LayoutLine, LayoutOptions,
    ParagraphLayout,
};
pub use shape::{Direction, ShapedGlyph, ShapedRun};
pub use style::{ColorId, FontFeature, StyleSpan, TextStyle};
pub use unicode_bidi::Level;
pub use unicode_script::Script;
