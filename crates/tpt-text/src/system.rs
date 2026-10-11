//! OS font discovery through `fontdb`. Enabled by the `fontdb` feature.
//!
//! [`SystemFonts`] wraps a font database and a [`FontRegistry`]. Each face is
//! read from the database once and cached in the registry, so later lookups
//! return the same `Arc<FontFace>`.

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use unicode_script::Script;

use crate::font::{FontFace, FontProvider, FontQuery, FontRegistry};

/// System fonts discovered through `fontdb`, cached in a [`FontRegistry`].
pub struct SystemFonts {
    db: fontdb::Database,
    registry: FontRegistry,
}

impl SystemFonts {
    /// Scans the operating system's font directories.
    pub fn load() -> SystemFonts {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        SystemFonts::from_database(db)
    }

    /// Wraps an existing database, for example one with extra font files.
    pub fn from_database(db: fontdb::Database) -> SystemFonts {
        SystemFonts {
            db,
            registry: FontRegistry::new(),
        }
    }

    /// Every family name the database knows, sorted and without duplicates.
    pub fn families(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .db
            .faces()
            .flat_map(|f| f.families.iter().map(|(name, _)| name.clone()))
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// The registry holding every face loaded so far.
    pub fn registry(&self) -> &FontRegistry {
        &self.registry
    }

    /// Finds the best system face for `query` and caches it in the registry.
    ///
    /// Returns `None` if the database has no face with that family.
    pub fn resolve(&self, query: &FontQuery) -> Option<Arc<FontFace>> {
        let families = [fontdb::Family::Name(&query.family)];
        let style = if query.italic {
            fontdb::Style::Italic
        } else {
            fontdb::Style::Normal
        };
        let id = self.db.query(&fontdb::Query {
            families: &families,
            weight: fontdb::Weight(query.weight),
            stretch: fontdb::Stretch::Normal,
            style,
        })?;
        let face_weight = self.db.face(id)?.weight.0;
        let italic = matches!(style, fontdb::Style::Italic);

        // Register under the requested family so the registry can match it.
        self.db
            .with_face_data(id, |data, index| {
                self.registry
                    .load_from_memory(
                        data.to_vec(),
                        index,
                        query.family.as_str(),
                        face_weight,
                        italic,
                    )
                    .ok()
            })
            .flatten()
    }
}

impl FontProvider for SystemFonts {
    fn load(&self, query: &FontQuery) -> Option<Arc<FontFace>> {
        self.registry.load(query).or_else(|| self.resolve(query))
    }

    fn fallback_chain(&self, script: Script, base_family: &str) -> Vec<Arc<FontFace>> {
        self.registry.fallback_chain(script, base_family)
    }
}
