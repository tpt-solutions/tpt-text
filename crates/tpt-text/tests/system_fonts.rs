//! System font discovery. Runs only with `--features fontdb`.

#![cfg(feature = "fontdb")]
#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use tpt_text::font::{FontProvider, FontQuery};
use tpt_text::system::SystemFonts;

#[test]
fn unknown_family_is_not_found() {
    let fonts = SystemFonts::load();
    assert!(fonts
        .resolve(&FontQuery::new("No Such Family 4f1c9e"))
        .is_none());
}

#[test]
fn resolved_face_is_cached_in_registry() {
    let fonts = SystemFonts::load();
    // Pick a family the machine actually has, so the test runs on any OS.
    let Some(family) = fonts.families().into_iter().next() else {
        println!("no system fonts found; skipping");
        return;
    };

    let query = FontQuery::new(family);
    let a = fonts.load(&query).expect("first lookup resolves");
    let b = fonts.load(&query).expect("second lookup resolves");
    assert!(
        Arc::ptr_eq(&a, &b),
        "second lookup should hit the registry cache"
    );
}
