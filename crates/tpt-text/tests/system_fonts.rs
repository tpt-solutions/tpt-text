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

const LATIN: &[u8] = include_bytes!("fonts/NotoSans-Regular.ttf");
const ARABIC: &[u8] = include_bytes!("fonts/NotoSansArabic-Regular.ttf");

/// A database holding only the vendored test fonts, so these tests do not
/// depend on what the machine has installed.
fn vendored() -> SystemFonts {
    let mut db = fontdb::Database::new();
    db.load_font_data(LATIN.to_vec());
    db.load_font_data(ARABIC.to_vec());
    SystemFonts::from_database(db)
}

#[test]
fn vendored_lookup_is_cached_per_query() {
    let fonts = vendored();
    let query = FontQuery::new("Noto Sans");
    let a = fonts.load(&query).unwrap();
    let b = fonts.load(&query).unwrap();
    assert!(Arc::ptr_eq(&a, &b));
}

#[test]
fn missing_weight_and_slant_fall_back_to_closest_face() {
    let fonts = vendored();
    // Only upright regular faces are installed, so bold italic gets the closest one.
    let face = fonts
        .load(&FontQuery {
            family: "Noto Sans".into(),
            weight: 700,
            italic: true,
        })
        .unwrap();
    assert_eq!(face.family(), "Noto Sans");
    assert_eq!(face.weight(), 400);
    assert!(!face.is_italic());
}

#[test]
fn fallback_chain_resolves_system_families() {
    let fonts = vendored();
    fonts
        .registry()
        .set_fallback_chain(unicode_script::Script::Arabic, &["Noto Sans Arabic"]);
    let chain = fonts.fallback_chain(unicode_script::Script::Arabic, "Noto Sans");
    assert_eq!(chain.len(), 1);
    assert_eq!(chain[0].family(), "Noto Sans Arabic");
}
