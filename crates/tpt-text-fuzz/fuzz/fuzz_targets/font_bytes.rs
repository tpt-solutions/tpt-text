#![no_main]

//! Corrupt font bytes must be rejected or accepted without panicking. Faces
//! that load must also lay out text without panicking.

use libfuzzer_sys::fuzz_target;
use tpt_text::{layout_text, FontRegistry, Fx, TextStyle};

fuzz_target!(|data: &[u8]| {
    let registry = FontRegistry::new();
    if registry
        .load_from_memory(data.to_vec(), 0, "Fuzzed", 400, false)
        .is_err()
    {
        return;
    }
    let style = TextStyle::new("Fuzzed", Fx::from_int(12));
    let _ = layout_text(&registry, "Hello, \u{0628}\u{062A} 2026", &style, Fx::from_int(120));
});
