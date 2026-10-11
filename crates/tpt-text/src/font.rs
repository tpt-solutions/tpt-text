//! Font faces, the provider trait, and the in-memory font registry.

use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt;

use rustybuzz::ttf_parser;
use spin::RwLock;
use unicode_script::Script;

/// Errors raised while loading a font.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontError {
    /// The bytes are not a parseable font, or the face index is out of range.
    InvalidFontData,
}

impl fmt::Display for FontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FontError::InvalidFontData => f.write_str("invalid font data"),
        }
    }
}

/// A parsed font face together with the bytes that back it.
///
/// Shaping borrows the bytes through rustybuzz, so the face owns them and is
/// shared through `Arc<FontFace>`.
#[derive(Debug)]
pub struct FontFace {
    data: Vec<u8>,
    index: u32,
    family: String,
    weight: u16,
    italic: bool,
    units_per_em: u16,
    ascender: i16,
    descender: i16,
    line_gap: i16,
}

impl FontFace {
    /// Parses a face from owned font bytes.
    ///
    /// `family`, `weight` and `italic` describe how the face is matched by
    /// [`FontQuery`]. They are caller-supplied because they are not reliably
    /// present in every font's name table.
    pub fn from_bytes(
        data: Vec<u8>,
        index: u32,
        family: impl Into<String>,
        weight: u16,
        italic: bool,
    ) -> Result<FontFace, FontError> {
        let (units_per_em, ascender, descender, line_gap) = {
            let face =
                ttf_parser::Face::parse(&data, index).map_err(|_| FontError::InvalidFontData)?;
            if face.units_per_em() == 0 {
                return Err(FontError::InvalidFontData);
            }
            (
                face.units_per_em(),
                face.ascender(),
                face.descender(),
                face.line_gap(),
            )
        };
        Ok(FontFace {
            data,
            index,
            family: family.into(),
            weight,
            italic,
            units_per_em,
            ascender,
            descender,
            line_gap,
        })
    }

    /// Family name this face was registered under.
    pub fn family(&self) -> &str {
        &self.family
    }

    /// Weight this face was registered under.
    pub fn weight(&self) -> u16 {
        self.weight
    }

    /// Whether this face is italic.
    pub fn is_italic(&self) -> bool {
        self.italic
    }

    /// Font units per em.
    pub fn units_per_em(&self) -> u16 {
        self.units_per_em
    }

    /// Typographic ascender in font units.
    pub fn ascender(&self) -> i16 {
        self.ascender
    }

    /// Typographic descender in font units (usually negative).
    pub fn descender(&self) -> i16 {
        self.descender
    }

    /// Line gap in font units.
    pub fn line_gap(&self) -> i16 {
        self.line_gap
    }

    /// Raw font bytes.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Returns `true` if the face has a glyph for `c`.
    pub fn has_glyph(&self, c: char) -> bool {
        self.glyph_index(c).is_some()
    }

    /// Glyph index for `c` from the font's character map, if present.
    pub fn glyph_index(&self, c: char) -> Option<u32> {
        self.parsed().glyph_index(c).map(|g| u32::from(g.0))
    }

    /// Parses the face for a single operation. Parsing is cheap because
    /// tables are read lazily; the result borrows from `self.data`.
    pub(crate) fn parsed(&self) -> ttf_parser::Face<'_> {
        // `from_bytes` already validated these bytes, so this cannot fail.
        ttf_parser::Face::parse(&self.data, self.index).expect("validated in from_bytes")
    }

    /// Creates a rustybuzz face for shaping.
    pub(crate) fn hb_face(&self) -> rustybuzz::Face<'_> {
        rustybuzz::Face::from_slice(&self.data, self.index).expect("validated in from_bytes")
    }
}

/// A request for a font by family, weight and slant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontQuery {
    /// Family name, matched case-insensitively.
    pub family: String,
    /// Requested weight, 100 to 900.
    pub weight: u16,
    /// Whether an italic face is requested.
    pub italic: bool,
}

impl FontQuery {
    /// A regular-weight, upright query for `family`.
    pub fn new(family: impl Into<String>) -> FontQuery {
        FontQuery {
            family: family.into(),
            weight: 400,
            italic: false,
        }
    }
}

/// Source of fonts for the layout pipeline.
pub trait FontProvider {
    /// Finds the best face for `query`, or `None` if the family is unknown.
    fn load(&self, query: &FontQuery) -> Option<Arc<FontFace>>;

    /// Faces to try, in order, when the base family lacks a glyph for `script`.
    fn fallback_chain(&self, script: Script, base_family: &str) -> Vec<Arc<FontFace>>;
}

/// Thread-safe, in-memory collection of fonts.
///
/// Fonts are supplied as bytes (for example embedded in a PDF or bundled with
/// the application), so no OS font access is needed. Loading the same bytes
/// twice returns the same `Arc`.
#[derive(Debug, Default)]
pub struct FontRegistry {
    faces: RwLock<Vec<Arc<FontFace>>>,
    fallbacks: RwLock<Vec<(Script, Vec<String>)>>,
}

impl FontRegistry {
    /// Creates an empty registry.
    pub fn new() -> FontRegistry {
        FontRegistry::default()
    }

    /// Registers a face from memory and returns the shared handle.
    ///
    /// If a face with identical bytes, index, family, weight and slant is
    /// already registered, that handle is returned instead.
    pub fn load_from_memory(
        &self,
        data: Vec<u8>,
        index: u32,
        family: impl Into<String>,
        weight: u16,
        italic: bool,
    ) -> Result<Arc<FontFace>, FontError> {
        let family = family.into();
        {
            let faces = self.faces.read();
            if let Some(existing) = faces.iter().find(|f| {
                f.index == index
                    && f.weight == weight
                    && f.italic == italic
                    && f.family.eq_ignore_ascii_case(&family)
                    && f.data == data
            }) {
                return Ok(Arc::clone(existing));
            }
        }
        let face = Arc::new(FontFace::from_bytes(data, index, family, weight, italic)?);
        self.faces.write().push(Arc::clone(&face));
        Ok(face)
    }

    /// Sets the ordered fallback families used for `script`.
    ///
    /// Replaces any previous chain for that script. Families that are not
    /// registered are skipped at lookup time.
    pub fn set_fallback_chain(&self, script: Script, families: &[&str]) {
        let mut chains = self.fallbacks.write();
        let names: Vec<String> = families.iter().map(|f| f.to_string()).collect();
        match chains.iter_mut().find(|(s, _)| *s == script) {
            Some(entry) => entry.1 = names,
            None => chains.push((script, names)),
        }
    }
}

/// Scores a face against a query. Lower is better; `None` means no match.
fn score(face: &FontFace, query: &FontQuery) -> Option<u32> {
    if !face.family.eq_ignore_ascii_case(&query.family) {
        return None;
    }
    let slant_penalty = if face.italic == query.italic { 0 } else { 1000 };
    let weight_penalty = (face.weight as i32 - query.weight as i32).unsigned_abs();
    Some(slant_penalty + weight_penalty)
}

impl FontProvider for FontRegistry {
    fn load(&self, query: &FontQuery) -> Option<Arc<FontFace>> {
        let faces = self.faces.read();
        // `min_by_key` keeps the first of equal scores, so lookup is deterministic.
        faces
            .iter()
            .filter_map(|f| score(f, query).map(|s| (s, f)))
            .min_by_key(|(s, _)| *s)
            .map(|(_, f)| Arc::clone(f))
    }

    fn fallback_chain(&self, script: Script, base_family: &str) -> Vec<Arc<FontFace>> {
        let families = {
            let chains = self.fallbacks.read();
            match chains.iter().find(|(s, _)| *s == script) {
                Some((_, names)) => names.clone(),
                None => return Vec::new(),
            }
        };
        families
            .iter()
            .filter(|name| !name.eq_ignore_ascii_case(base_family))
            .filter_map(|name| self.load(&FontQuery::new(name.as_str())))
            .collect()
    }
}
