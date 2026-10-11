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

#[cfg(feature = "std")]
impl std::error::Error for FontError {}

/// Both parsers for one face, borrowing from the face's bytes.
pub(crate) struct Tables<'a> {
    /// The parser used for metrics and character maps.
    pub(crate) ttf: ttf_parser::Face<'a>,
    /// The shaper's view of the same bytes.
    pub(crate) hb: rustybuzz::Face<'a>,
}

self_cell::self_cell!(
    /// Font bytes owned together with the parsers that borrow them, so each
    /// face is parsed once rather than on every lookup.
    struct FontBytes {
        owner: Vec<u8>,

        #[covariant]
        dependent: Tables,
    }
);

/// A parsed font face together with the bytes that back it.
///
/// Shaping borrows the bytes through rustybuzz, so the face owns them and is
/// shared through `Arc<FontFace>`.
pub struct FontFace {
    bytes: FontBytes,
    index: u32,
    family: String,
    weight: u16,
    italic: bool,
    units_per_em: u16,
    ascender: i16,
    descender: i16,
    line_gap: i16,
}

impl fmt::Debug for FontFace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FontFace")
            .field("family", &self.family)
            .field("weight", &self.weight)
            .field("italic", &self.italic)
            .field("index", &self.index)
            .field("bytes", &self.data().len())
            .finish()
    }
}

impl FontFace {
    /// Parses a face from owned font bytes.
    ///
    /// `family`, `weight` and `italic` describe how the face is matched by
    /// [`FontQuery`]. They are caller-supplied because they are not reliably
    /// present in every font's name table; see [`FontFace::from_font_data`]
    /// to read them from the file.
    pub fn from_bytes(
        data: Vec<u8>,
        index: u32,
        family: impl Into<String>,
        weight: u16,
        italic: bool,
    ) -> Result<FontFace, FontError> {
        let bytes = FontBytes::try_new(data, |owner| {
            let ttf =
                ttf_parser::Face::parse(owner, index).map_err(|_| FontError::InvalidFontData)?;
            // Check that rustybuzz accepts the bytes too, so later lookups cannot fail.
            let hb = rustybuzz::Face::from_slice(owner, index).ok_or(FontError::InvalidFontData)?;
            Ok(Tables { ttf, hb })
        })?;
        let (units_per_em, ascender, descender, line_gap) = {
            let ttf = &bytes.borrow_dependent().ttf;
            if ttf.units_per_em() == 0 {
                return Err(FontError::InvalidFontData);
            }
            (
                ttf.units_per_em(),
                ttf.ascender(),
                ttf.descender(),
                ttf.line_gap(),
            )
        };
        Ok(FontFace {
            bytes,
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

    /// Parses a face and reads its family, weight and slant from the font.
    ///
    /// The family comes from the typographic family name (ID 16), falling back
    /// to the legacy family name (ID 1). Weight comes from the OS/2 table and
    /// slant from the font's style flags and italic angle.
    pub fn from_font_data(data: Vec<u8>, index: u32) -> Result<FontFace, FontError> {
        let (family, weight, italic) = {
            let face =
                ttf_parser::Face::parse(&data, index).map_err(|_| FontError::InvalidFontData)?;
            let family = read_family(&face).unwrap_or_default();
            (family, face.weight().to_number(), face.is_italic())
        };
        FontFace::from_bytes(data, index, family, weight, italic)
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

    /// Index of this face within its font file (non-zero for collections).
    pub fn index(&self) -> u32 {
        self.index
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
        self.bytes.borrow_owner()
    }

    /// Returns `true` if the face has a glyph for `c`.
    pub fn has_glyph(&self, c: char) -> bool {
        self.glyph_index(c).is_some()
    }

    /// Glyph index for `c` from the font's character map, if present.
    pub fn glyph_index(&self, c: char) -> Option<u32> {
        self.parsed().glyph_index(c).map(|g| u32::from(g.0))
    }

    /// The cached metrics and character-map parser.
    pub(crate) fn parsed(&self) -> &ttf_parser::Face<'_> {
        &self.bytes.borrow_dependent().ttf
    }

    /// The cached shaper for this face.
    pub(crate) fn hb_face(&self) -> &rustybuzz::Face<'_> {
        &self.bytes.borrow_dependent().hb
    }

    /// A fresh shaper for this face, for applying variation settings. Shaping
    /// with variations needs a mutable face, so it cannot use the cached one.
    pub(crate) fn hb_face_with_variations(
        &self,
        variations: &[rustybuzz::Variation],
    ) -> rustybuzz::Face<'_> {
        let mut face =
            rustybuzz::Face::from_slice(self.data(), self.index).expect("validated in from_bytes");
        face.set_variations(variations);
        face
    }
}

/// Reads the family name from a face's name table.
fn read_family(face: &ttf_parser::Face<'_>) -> Option<String> {
    // Name IDs: 1 is the legacy family, 16 the typographic family.
    const FAMILY: u16 = 1;
    const TYPOGRAPHIC_FAMILY: u16 = 16;
    let mut legacy = None;
    for name in face.names() {
        if name.name_id == TYPOGRAPHIC_FAMILY {
            if let Some(text) = decode_name(&name) {
                return Some(text);
            }
        } else if name.name_id == FAMILY && legacy.is_none() {
            legacy = decode_name(&name);
        }
    }
    legacy
}

/// Decodes a name record. Unicode and Windows records are UTF-16BE; the
/// Macintosh records used here are single-byte Roman, read as Latin-1.
fn decode_name(name: &ttf_parser::name::Name<'_>) -> Option<String> {
    if name.is_unicode() {
        let units = name
            .name
            .chunks_exact(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]));
        char::decode_utf16(units)
            .collect::<Result<String, _>>()
            .ok()
    } else {
        Some(name.name.iter().map(|&b| char::from(b)).collect())
    }
    .filter(|s: &String| !s.is_empty())
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
        let same = |f: &FontFace, data: &[u8]| {
            f.index == index
                && f.weight == weight
                && f.italic == italic
                && f.family.eq_ignore_ascii_case(&family)
                && f.data() == data
        };
        if let Some(existing) = self.faces.read().iter().find(|f| same(f, &data)) {
            return Ok(Arc::clone(existing));
        }

        // Parse outside the lock. Another thread may have added the same face
        // in the meantime, so check again before pushing.
        let face = FontFace::from_bytes(data, index, family, weight, italic)?;
        Ok(self.insert(face))
    }

    /// Registers a face whose family, weight and slant are read from the font.
    pub fn load_font(&self, data: Vec<u8>, index: u32) -> Result<Arc<FontFace>, FontError> {
        Ok(self.insert(FontFace::from_font_data(data, index)?))
    }

    /// Registers every face in a font collection (`.ttc`) and returns them in
    /// index order. A plain font file yields one face.
    pub fn load_collection(&self, data: Vec<u8>) -> Result<Vec<Arc<FontFace>>, FontError> {
        let count = ttf_parser::fonts_in_collection(&data).unwrap_or(1);
        (0..count)
            .map(|index| self.load_font(data.clone(), index))
            .collect()
    }

    /// Adds `face` unless an identical one is registered, and returns the
    /// registered handle.
    fn insert(&self, face: FontFace) -> Arc<FontFace> {
        let mut faces = self.faces.write();
        if let Some(existing) = faces.iter().find(|f| {
            f.index == face.index
                && f.weight == face.weight
                && f.italic == face.italic
                && f.family.eq_ignore_ascii_case(&face.family)
                && f.data() == face.data()
        }) {
            return Arc::clone(existing);
        }
        let face = Arc::new(face);
        faces.push(Arc::clone(&face));
        face
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

    /// The configured fallback family names for `script`, in order.
    pub(crate) fn fallback_families(&self, script: Script) -> Vec<String> {
        self.fallbacks
            .read()
            .iter()
            .find(|(s, _)| *s == script)
            .map(|(_, names)| names.clone())
            .unwrap_or_default()
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
        self.fallback_families(script)
            .iter()
            .filter(|name| !name.eq_ignore_ascii_case(base_family))
            .filter_map(|name| self.load(&FontQuery::new(name.as_str())))
            .collect()
    }
}

/// Serialises a face as its identity (family, index, weight, slant), not its bytes.
#[cfg(feature = "serde")]
pub(crate) fn serialize_font<S: serde::Serializer>(
    font: &Arc<FontFace>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::Serialize;
    (font.family(), font.index(), font.weight(), font.is_italic()).serialize(serializer)
}
