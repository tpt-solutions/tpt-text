//! OS font discovery through `fontdb`. Enabled by the `fontdb` feature.
//!
//! [`SystemFonts`] wraps a font database and a [`FontRegistry`]. Each face is
//! read from the database once per requested family and cached, so later
//! lookups return the same `Arc<FontFace>`.

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use spin::RwLock;
use unicode_script::Script;

use crate::font::{FontFace, FontProvider, FontQuery, FontRegistry};

/// System fonts discovered through `fontdb`, cached in a [`FontRegistry`].
pub struct SystemFonts {
    db: fontdb::Database,
    registry: FontRegistry,
    /// Faces already read from the database, keyed by database id and the
    /// family name they were requested under.
    cache: RwLock<Vec<(fontdb::ID, String, Arc<FontFace>)>>,
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
            cache: RwLock::new(Vec::new()),
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

        if let Some((_, _, face)) =
            self.cache.read().iter().find(|(cached, family, _)| {
                *cached == id && family.eq_ignore_ascii_case(&query.family)
            })
        {
            return Some(Arc::clone(face));
        }

        let info = self.db.face(id)?;
        let face_weight = info.weight.0;
        let face_italic = matches!(info.style, fontdb::Style::Italic | fontdb::Style::Oblique);
        // Register under the requested family so the registry can match it.
        let face = self
            .db
            .with_face_data(id, |data, index| {
                self.registry
                    .load_from_memory(
                        data.to_vec(),
                        index,
                        query.family.as_str(),
                        face_weight,
                        face_italic,
                    )
                    .ok()
            })
            .flatten()?;
        self.cache
            .write()
            .push((id, query.family.clone(), Arc::clone(&face)));
        Some(face)
    }
}

impl FontProvider for SystemFonts {
    fn load(&self, query: &FontQuery) -> Option<Arc<FontFace>> {
        // A cached face only counts if its weight and slant match the request;
        // otherwise ask the database for the closest real face.
        if let Some(face) = self.registry.load(query) {
            if face.weight() == query.weight && face.is_italic() == query.italic {
                return Some(face);
            }
        }
        self.resolve(query).or_else(|| self.registry.load(query))
    }

    fn fallback_chain(&self, script: Script, base_family: &str) -> Vec<Arc<FontFace>> {
        self.registry
            .fallback_families(script)
            .iter()
            .filter(|name| !name.eq_ignore_ascii_case(base_family))
            .filter_map(|name| self.load(&FontQuery::new(name.as_str())))
            .collect()
    }
}
