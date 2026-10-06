//! The font database: fonts the host hands over as bytes (sans-IO, P10) — from a bundle's font
//! chunks, a document's font sources, or (with the `bundled-fonts` feature) the default family
//! compiled in.
//!
//! Faces are addressed by id `"<Family>-<Weight>[-Italic]"` (e.g. `"Inter-400"`), the string text
//! runs carry into the display list. Matching a family + weight + style to a face follows the CSS
//! font-matching rules so authored styles behave as they would on the web.

use crate::shape::ShapeCache;
use crate::TextError;
use datars_math::{PathData, PathEl, Vec2};
use harfrust::ShaperData;
use skrifa::attribute::Style;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::raw::{FileRef, TableProvider};
use skrifa::string::StringId;
use skrifa::{FontRef, GlyphId, MetadataProvider};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, MutexGuard};

/// The default family's files, as the datars distribution ships them (`fonts/`): what
/// `datars:fonts/<file>` names. Compiled in only with the `bundled-fonts` feature.
#[cfg(feature = "bundled-fonts")]
static BUNDLED: &[(&str, &[u8])] = &[
    ("Inter-Regular.ttf", include_bytes!("../../../fonts/Inter-Regular.ttf")),
    ("Inter-SemiBold.ttf", include_bytes!("../../../fonts/Inter-SemiBold.ttf")),
    ("Inter-Bold.ttf", include_bytes!("../../../fonts/Inter-Bold.ttf")),
];

/// A default font file compiled into this build (`"Inter-Regular.ttf"`), for hosts answering
/// `datars:fonts/<file>` requests. `None` without the `bundled-fonts` feature.
pub fn bundled_file(name: &str) -> Option<&'static [u8]> {
    #[cfg(feature = "bundled-fonts")]
    return BUNDLED.iter().find(|(n, _)| *n == name).map(|(_, b)| *b);
    #[cfg(not(feature = "bundled-fonts"))]
    {
        let _ = name;
        None
    }
}

/// Which compiled-in default font file these bytes are (`"Inter-Bold.ttf"`), if any.
pub fn bundled_name(bytes: &[u8]) -> Option<&'static str> {
    #[cfg(feature = "bundled-fonts")]
    return BUNDLED.iter().find(|(_, b)| *b == bytes).map(|(n, _)| *n);
    #[cfg(not(feature = "bundled-fonts"))]
    {
        let _ = bytes;
        None
    }
}

/// The family every lookup falls back to (and what CSS generic families map to) when present;
/// otherwise the first registered face is the fallback.
pub(crate) const DEFAULT_FAMILY: &str = "Inter";

/// Cached outlines are bounded so a long-running session can't grow without limit; clearing is
/// safe because the cache is pure memoization.
const OUTLINE_CACHE_MAX: usize = 16_384;

/// Unscaled outlines (font units, y up) by (face index, glyph id).
type OutlineCache = BTreeMap<(usize, u16), Arc<[PathEl]>>;

/// Font file bytes; cloning is cheap, so every face of a collection shares one allocation.
#[derive(Clone)]
enum Bytes {
    Static(&'static [u8]),
    Owned(Arc<[u8]>),
}

impl Bytes {
    fn as_slice(&self) -> &[u8] {
        match self {
            Bytes::Static(b) => b,
            Bytes::Owned(b) => b,
        }
    }
}

/// Global metrics of one face, in font units (converted to px on request).
#[derive(Clone, Copy, Debug)]
pub(crate) struct UnitMetrics {
    pub upem: f64,
    pub ascent: f64,
    /// Positive distance below the baseline.
    pub descent: f64,
    pub line_gap: f64,
    pub cap_height: f64,
    pub x_height: f64,
}

pub(crate) struct Face {
    pub id: Arc<str>,
    pub family: String,
    family_key: String,
    pub weight: u16,
    pub italic: bool,
    bytes: Bytes,
    index: u32,
    pub metrics: UnitMetrics,
    pub shaper_data: ShaperData,
    /// OS/2 `fsType`: the embedding permissions the font's vendor declares.
    fs_type: u16,
    /// Another subset of a face (`"<id>~<part>"`), tried before the face itself.
    part: bool,
}

impl Face {
    /// A font reference over this face's bytes. Parsing is validated when the face is added and
    /// the bytes are immutable, so this cannot fail afterwards.
    pub fn font(&self) -> FontRef<'_> {
        FontRef::from_index(self.bytes.as_slice(), self.index).expect("face bytes were validated when added")
    }

    /// Scale factor from font units to px at `size`.
    pub fn scale(&self, size: f64) -> f64 {
        size / self.metrics.upem
    }
}

/// Public description of a registered face.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FaceInfo {
    pub id: String,
    pub family: String,
    pub weight: u16,
    pub italic: bool,
    /// OS/2 `fsType` embedding bits (0 = installable; see [`EMBED_RESTRICTED`]).
    pub fs_type: u16,
}

/// OS/2 `fsType` bit 1: "Restricted License embedding" — the vendor forbids embedding the font
/// in documents, which a bundle is.
pub const EMBED_RESTRICTED: u16 = 0x0002;
/// OS/2 `fsType` bit 8: "No subsetting" — the font may only be embedded whole.
pub const EMBED_NO_SUBSETTING: u16 = 0x0100;

/// Global metrics of a face at a size, in px (y distances are positive magnitudes).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceMetrics {
    pub units_per_em: u16,
    pub ascent: f64,
    pub descent: f64,
    pub line_gap: f64,
    pub cap_height: f64,
    pub x_height: f64,
}

/// Font faces by family + weight + style, plus the shaping and outline caches.
///
/// `FontDb` is `Send + Sync`: caches sit behind mutexes (uncontended on wasm), and every cached
/// value is exactly what a fresh computation would return, so sharing a database between figures
/// never changes a result.
pub struct FontDb {
    faces: Vec<Face>,
    by_id: BTreeMap<Arc<str>, usize>,
    /// Families every stack falls back to after its own and the default: fonts a document brings
    /// for other scripts (Hebrew, Arabic, CJK), so any label finds its glyphs.
    fallbacks: Vec<String>,
    pub(crate) shape_cache: Mutex<ShapeCache>,
    outline_cache: Mutex<OutlineCache>,
    /// Characters layout found in no face of their stack (drawn as .notdef): a runtime can fetch
    /// a bundle's lazily loaded script subsets for them. Observation only — never read by layout.
    missing: Mutex<BTreeSet<char>>,
}

impl Default for FontDb {
    fn default() -> Self {
        FontDb::builtin()
    }
}

impl std::fmt::Debug for FontDb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontDb").field("faces", &self.faces.iter().map(|f| &*f.id).collect::<Vec<_>>()).finish()
    }
}

impl FontDb {
    /// A database with no faces: every face comes from the host (bundle font chunks, document
    /// font sources). Layout before any face arrives produces no glyphs.
    pub fn empty() -> FontDb {
        FontDb {
            faces: Vec::new(),
            by_id: BTreeMap::new(),
            fallbacks: Vec::new(),
            shape_cache: Mutex::new(ShapeCache::default()),
            outline_cache: Mutex::new(BTreeMap::new()),
            missing: Mutex::new(BTreeSet::new()),
        }
    }

    /// The compiled-in faces: Inter Regular (400), SemiBold (600) and Bold (700).
    #[cfg(feature = "bundled-fonts")]
    pub fn with_bundled() -> FontDb {
        let mut db = FontDb::empty();
        for (_, bytes) in BUNDLED {
            db.add_bytes(Bytes::Static(bytes), None).expect("bundled Inter fonts are valid");
        }
        db
    }

    /// What this build starts with: the compiled-in faces with the `bundled-fonts` feature,
    /// otherwise none.
    pub fn builtin() -> FontDb {
        #[cfg(feature = "bundled-fonts")]
        return FontDb::with_bundled();
        #[cfg(not(feature = "bundled-fonts"))]
        FontDb::empty()
    }

    /// Add a font as a named face — what a theme's font token says it is (`family`, `weight`,
    /// `italic`), whatever its name table calls it: a subset or a renamed build of a family still
    /// matches the stacks that name the token's family. Every face of a collection takes the
    /// family; weight and style are taken only for a single-face file. Not a fallback: text uses
    /// it when its stack names the family.
    pub fn add_face(&mut self, bytes: Vec<u8>, family: &str, weight: u16, italic: bool) -> Result<Vec<String>, TextError> {
        self.add_bytes(Bytes::Owned(Arc::from(bytes)), Some((family, weight, italic, None)))
    }

    /// Add another subset of a face's font (a script a bundle loads lazily), as face
    /// `"<id>~<part>"`. Text in that family, weight and style tries the face's parts before the
    /// face itself — each part covers the face's characters too, so a label shaped with a part
    /// kerns as it would with the whole font.
    pub fn add_face_part(&mut self, bytes: Vec<u8>, family: &str, weight: u16, italic: bool, part: &str) -> Result<Vec<String>, TextError> {
        self.add_bytes(Bytes::Owned(Arc::from(bytes)), Some((family, weight, italic, Some(part))))
    }

    /// Is a face with this id (`"Inter-600"`) registered?
    pub fn has_face(&self, id: &str) -> bool {
        self.by_id.contains_key(id)
    }

    /// Does any face belong to `family` (generic families: the default family)?
    pub fn has_family(&self, family: &str) -> bool {
        let key = family_key(if is_generic_family(family) { DEFAULT_FAMILY } else { family });
        self.faces.iter().any(|f| f.family_key == key)
    }

    /// The font file behind a face and the face's index in it (collections), for tools that
    /// re-package faces (the publish compiler subsets them into bundles).
    pub fn face_data(&self, id: &str) -> Option<(&[u8], u32)> {
        let f = &self.faces[*self.by_id.get(id)?];
        Some((f.bytes.as_slice(), f.index))
    }

    /// Characters laid out so far that no face of their stack had (drawn as .notdef).
    pub fn missing_chars(&self) -> Vec<char> {
        lock(&self.missing).iter().copied().collect()
    }

    pub(crate) fn note_missing(&self, c: char) {
        lock(&self.missing).insert(c);
    }

    /// Make `family` a fallback for every family stack (after the stack and the default family):
    /// glyphs no earlier face has come from it. Returns false if no face has that family.
    pub fn add_fallback(&mut self, family: &str) -> bool {
        let key = family_key(family);
        if !self.faces.iter().any(|f| f.family_key == key) {
            return false;
        }
        if !self.fallbacks.iter().any(|f| family_key(f) == key) {
            self.fallbacks.push(family.to_string());
            lock(&self.shape_cache).clear();
        }
        true
    }

    /// The family names of the faces in font `bytes` (for registering them as fallbacks).
    /// The ids of the faces in a font file (`"Family-600"`), as [`FontDb`] would address them.
    pub fn faces_in(bytes: &[u8]) -> Vec<String> {
        let Ok(file) = FileRef::new(bytes) else { return Vec::new() };
        file.fonts().flatten().filter_map(|f| describe(&f)).map(|d| face_id_string(&d.family, d.weight, d.italic)).collect()
    }

    pub fn families_in(bytes: &[u8]) -> Vec<String> {
        let Ok(file) = FileRef::new(bytes) else { return Vec::new() };
        let mut out: Vec<String> = Vec::new();
        for font in file.fonts().flatten() {
            if let Some(d) = describe(&font) {
                if !out.contains(&d.family) {
                    out.push(d.family);
                }
            }
        }
        out
    }

    /// Add a font (or every face of a font collection) and return the new face ids. A face whose
    /// id already exists replaces the earlier one.
    pub fn add_font(&mut self, bytes: Vec<u8>) -> Result<Vec<String>, TextError> {
        self.add_bytes(Bytes::Owned(Arc::from(bytes)), None)
    }

    fn add_bytes(&mut self, bytes: Bytes, alias: Option<(&str, u16, bool, Option<&str>)>) -> Result<Vec<String>, TextError> {
        // Parse everything first so a bad file leaves the database untouched.
        let mut new_faces = Vec::new();
        {
            let data = bytes.as_slice();
            let file = FileRef::new(data).map_err(|e| TextError::InvalidFont(e.to_string()))?;
            for (index, font) in file.fonts().enumerate() {
                let font = font.map_err(|e| TextError::InvalidFont(e.to_string()))?;
                if let Some(desc) = describe(&font) {
                    new_faces.push((index as u32, desc, ShaperData::new(&font)));
                }
            }
        }
        if new_faces.is_empty() {
            return Err(TextError::NoFaces);
        }
        let part = alias.and_then(|a| a.3).map(str::trim).filter(|p| !p.is_empty());
        if let Some((family, weight, italic, _)) = alias.filter(|(f, _, _, _)| !f.trim().is_empty()) {
            let single = new_faces.len() == 1;
            for (_, desc, _) in &mut new_faces {
                desc.family = family.trim().to_string();
                if single {
                    desc.weight = weight.clamp(1, 1000);
                    desc.italic = italic;
                }
            }
        }
        let mut ids = Vec::with_capacity(new_faces.len());
        for (index, desc, shaper_data) in new_faces {
            let base = face_id_string(&desc.family, desc.weight, desc.italic);
            let id: Arc<str> = Arc::from(match part {
                Some(p) => format!("{base}~{p}"),
                None => base,
            });
            let face = Face {
                part: part.is_some(),
                id: id.clone(),
                family_key: family_key(&desc.family),
                family: desc.family,
                weight: desc.weight,
                italic: desc.italic,
                bytes: bytes.clone(),
                index,
                metrics: desc.metrics,
                shaper_data,
                fs_type: desc.fs_type,
            };
            match self.by_id.get(&id) {
                Some(&i) => self.faces[i] = face,
                None => {
                    self.by_id.insert(id.clone(), self.faces.len());
                    self.faces.push(face);
                }
            }
            ids.push(id.to_string());
        }
        // Face indices key the caches; a replaced face must not serve stale shapes or outlines.
        lock(&self.shape_cache).clear();
        lock(&self.outline_cache).clear();
        Ok(ids)
    }

    /// The best face for `family` (a single family or a CSS-style stack, first match wins),
    /// `weight` and style, following CSS font matching. Unknown families fall back to Inter, then
    /// to the first registered face; `None` only for an empty database.
    pub fn face_id(&self, family: &str, weight: u16, italic: bool) -> Option<String> {
        let idx = split_families(family)
            .into_iter()
            .find_map(|f| self.match_family(&f, weight, italic))
            .or_else(|| self.default_face(weight, italic))?;
        Some(self.faces[idx].id.to_string())
    }

    /// All registered faces, in registration order.
    pub fn faces(&self) -> Vec<FaceInfo> {
        self.faces
            .iter()
            .map(|f| FaceInfo { id: f.id.to_string(), family: f.family.clone(), weight: f.weight, italic: f.italic, fs_type: f.fs_type })
            .collect()
    }

    /// Global metrics of a face at `size` px.
    pub fn metrics(&self, face_id: &str, size: f64) -> Option<FaceMetrics> {
        let f = &self.faces[*self.by_id.get(face_id)?];
        let k = f.scale(size);
        let m = f.metrics;
        Some(FaceMetrics {
            units_per_em: m.upem as u16,
            ascent: m.ascent * k,
            descent: m.descent * k,
            line_gap: m.line_gap * k,
            cap_height: m.cap_height * k,
            x_height: m.x_height * k,
        })
    }

    /// The nominal glyph for `ch` in a face (`None` if the face lacks it, i.e. it would be .notdef).
    pub fn glyph_for(&self, face_id: &str, ch: char) -> Option<u16> {
        let f = &self.faces[*self.by_id.get(face_id)?];
        f.font().charmap().map(ch).map(|g| g.to_u32() as u16).filter(|&g| g != 0)
    }

    // ---- crate-internal ------------------------------------------------------------------------

    pub(crate) fn face(&self, idx: usize) -> &Face {
        &self.faces[idx]
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.faces.is_empty()
    }

    /// Face indices for a family stack: each listed family's best match in order, then the default
    /// family, without duplicates. Glyph fallback walks this list.
    pub(crate) fn resolve_stack(&self, family: &str, weight: u16, italic: bool) -> Vec<usize> {
        let mut out: Vec<usize> = Vec::new();
        let push = |i: usize, out: &mut Vec<usize>| {
            for j in self.with_parts(i) {
                if !out.contains(&j) {
                    out.push(j);
                }
            }
        };
        for f in split_families(family) {
            if let Some(i) = self.match_family(&f, weight, italic) {
                push(i, &mut out);
            }
        }
        if let Some(i) = self.default_face(weight, italic) {
            push(i, &mut out);
        }
        for f in &self.fallbacks {
            if let Some(i) = self.match_family(f, weight, italic) {
                push(i, &mut out);
            }
        }
        out
    }

    /// A face's parts (same family, weight and style; by id, so arrival order doesn't matter),
    /// then the face.
    fn with_parts(&self, i: usize) -> Vec<usize> {
        let f = &self.faces[i];
        let mut parts: Vec<usize> = (0..self.faces.len()).filter(|&j| j != i && self.faces[j].part && self.faces[j].family_key == f.family_key && self.faces[j].weight == f.weight && self.faces[j].italic == f.italic).collect();
        parts.sort_by(|a, b| self.faces[*a].id.cmp(&self.faces[*b].id));
        parts.push(i);
        parts
    }

    fn default_face(&self, weight: u16, italic: bool) -> Option<usize> {
        self.match_family(DEFAULT_FAMILY, weight, italic).or(if self.faces.is_empty() { None } else { Some(0) })
    }

    /// CSS Fonts 4 §5.2: style first (italic → normal fallback), then weight.
    fn match_family(&self, family: &str, weight: u16, italic: bool) -> Option<usize> {
        let key = family_key(if is_generic_family(family) { DEFAULT_FAMILY } else { family });
        let in_family: Vec<usize> = (0..self.faces.len()).filter(|&i| self.faces[i].family_key == key).collect();
        if in_family.is_empty() {
            return None;
        }
        let same_style: Vec<usize> = in_family.iter().copied().filter(|&i| self.faces[i].italic == italic).collect();
        let pool = if same_style.is_empty() { in_family } else { same_style };
        pool.into_iter().min_by_key(|&i| (weight_rank(self.faces[i].weight, weight), self.faces[i].part, i))
    }

    /// Unscaled outline (font units, y up), cached.
    fn unscaled_outline(&self, face: usize, glyph: u16) -> Option<Arc<[PathEl]>> {
        if let Some(p) = lock(&self.outline_cache).get(&(face, glyph)) {
            return Some(p.clone());
        }
        let font = self.faces[face].font();
        let outlines = font.outline_glyphs();
        let g = outlines.get(GlyphId::new(glyph as u32))?;
        let mut pen = Pen { els: Vec::new() };
        g.draw(DrawSettings::unhinted(Size::unscaled(), LocationRef::default()), &mut pen).ok()?;
        let els: Arc<[PathEl]> = pen.els.into();
        let mut cache = lock(&self.outline_cache);
        if cache.len() >= OUTLINE_CACHE_MAX {
            cache.clear();
        }
        cache.insert((face, glyph), els.clone());
        Some(els)
    }
}

impl datars_render::GlyphSource for FontDb {
    /// Outline of `glyph` in face `font` at `size` px: y down, origin at the glyph origin on the
    /// baseline. Font units are converted with f64 arithmetic only (deterministic).
    fn outline(&self, font: &str, glyph: u16, size: f64) -> Option<PathData> {
        let &face = self.by_id.get(font)?;
        let els = self.unscaled_outline(face, glyph)?;
        let k = self.faces[face].scale(size);
        let t = |p: Vec2| Vec2::new(p.x * k, -p.y * k);
        Some(PathData {
            els: els
                .iter()
                .map(|e| match *e {
                    PathEl::Move { p } => PathEl::Move { p: t(p) },
                    PathEl::Line { p } => PathEl::Line { p: t(p) },
                    PathEl::Quad { c, p } => PathEl::Quad { c: t(c), p: t(p) },
                    PathEl::Cubic { c1, c2, p } => PathEl::Cubic { c1: t(c1), c2: t(c2), p: t(p) },
                    PathEl::Close => PathEl::Close,
                })
                .collect(),
        })
    }
}

/// Records skrifa's pen callbacks in font units (f32 → f64 is exact).
struct Pen {
    els: Vec<PathEl>,
}

impl OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.els.push(PathEl::Move { p: Vec2::new(x as f64, y as f64) });
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.els.push(PathEl::Line { p: Vec2::new(x as f64, y as f64) });
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.els.push(PathEl::Quad { c: Vec2::new(cx0 as f64, cy0 as f64), p: Vec2::new(x as f64, y as f64) });
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.els.push(PathEl::Cubic {
            c1: Vec2::new(cx0 as f64, cy0 as f64),
            c2: Vec2::new(cx1 as f64, cy1 as f64),
            p: Vec2::new(x as f64, y as f64),
        });
    }
    fn close(&mut self) {
        self.els.push(PathEl::Close);
    }
}

struct FaceDesc {
    family: String,
    weight: u16,
    italic: bool,
    metrics: UnitMetrics,
    fs_type: u16,
}

/// Name, weight, style and metrics of a face; `None` if it can't be used for text (no cmap or no
/// scalable outlines).
fn describe(font: &FontRef) -> Option<FaceDesc> {
    if !font.charmap().has_map() || font.outline_glyphs().format().is_none() {
        return None;
    }
    let name = |id: StringId| font.localized_strings(id).english_or_first().map(|s| s.to_string()).filter(|s| !s.trim().is_empty());
    // Prefer the typographic family (ID 16): static fonts put the weight into the legacy family
    // name ("Inter SemiBold"), which would split one family into several.
    let family = name(StringId::TYPOGRAPHIC_FAMILY_NAME).or_else(|| name(StringId::FAMILY_NAME)).unwrap_or_else(|| "Unknown".to_string());
    let attrs = font.attributes();
    let weight = attrs.weight.value().round().clamp(1.0, 1000.0) as u16;
    let italic = !matches!(attrs.style, Style::Normal);
    let m = font.metrics(Size::unscaled(), LocationRef::default());
    let upem = if m.units_per_em == 0 { 1000.0 } else { m.units_per_em as f64 };
    let ascent = m.ascent as f64;
    let descent = (m.descent as f64).abs();
    let (ascent, descent) = if ascent + descent <= 0.0 { (upem * 0.8, upem * 0.2) } else { (ascent, descent) };
    let metrics = UnitMetrics {
        upem,
        ascent,
        descent,
        line_gap: m.leading as f64,
        cap_height: m.cap_height.map(|v| v as f64).filter(|v| *v > 0.0).unwrap_or(ascent * 0.7),
        x_height: m.x_height.map(|v| v as f64).filter(|v| *v > 0.0).unwrap_or(ascent * 0.5),
    };
    let fs_type = font.os2().map(|t| t.fs_type()).unwrap_or(0);
    Some(FaceDesc { family: family.trim().to_string(), weight, italic, metrics, fs_type })
}

pub(crate) fn face_id_string(family: &str, weight: u16, italic: bool) -> String {
    if italic {
        format!("{family}-{weight}-Italic")
    } else {
        format!("{family}-{weight}")
    }
}

fn family_key(family: &str) -> String {
    family.trim().to_lowercase()
}

/// CSS generic families resolve to the bundled default.
fn is_generic_family(family: &str) -> bool {
    matches!(
        family.trim().to_ascii_lowercase().as_str(),
        "sans-serif"
            | "serif"
            | "monospace"
            | "cursive"
            | "fantasy"
            | "system-ui"
            | "ui-sans-serif"
            | "ui-serif"
            | "ui-monospace"
            | "ui-rounded"
            | "emoji"
            | "math"
            | "fangsong"
    )
}

/// Split a CSS family list (`"Noto Sans", Inter, sans-serif`) into names.
pub(crate) fn split_families(list: &str) -> Vec<String> {
    list.split(',')
        .map(|s| s.trim().trim_matches(|c| c == '"' || c == '\'').trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Rank of an available weight for a desired one (lower is better), per CSS Fonts 4 §5.2:
/// desired 400–500 tries up to 500, then down, then above 500; below 400 tries down then up;
/// above 500 tries up then down.
fn weight_rank(available: u16, desired: u16) -> (u8, u16) {
    let (a, d) = (available, desired);
    if a == d {
        return (0, 0);
    }
    if (400..=500).contains(&d) {
        if a > d && a <= 500 {
            (1, a - d)
        } else if a < d {
            (2, d - a)
        } else {
            (3, a - d)
        }
    } else if d < 400 {
        if a < d {
            (1, d - a)
        } else {
            (2, a - d)
        }
    } else if a > d {
        (1, a - d)
    } else {
        (2, d - a)
    }
}

/// Lock a cache, recovering from poisoning (a panic elsewhere can't corrupt memoized values).
pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(all(test, feature = "bundled-fonts"))]
mod tests {
    use super::*;
    use datars_render::GlyphSource;

    #[test]
    fn bundled_faces_have_expected_ids() {
        let db = FontDb::with_bundled();
        let ids: Vec<String> = db.faces().into_iter().map(|f| f.id).collect();
        assert_eq!(ids, vec!["Inter-400", "Inter-600", "Inter-700"]);
    }

    #[test]
    fn css_weight_matching() {
        let db = FontDb::with_bundled();
        let id = |w| db.face_id("Inter", w, false).unwrap();
        assert_eq!(id(400), "Inter-400");
        assert_eq!(id(600), "Inter-600");
        assert_eq!(id(700), "Inter-700");
        // 400–500: up to 500, then down → 450 has nothing in (450, 500], so 400.
        assert_eq!(id(450), "Inter-400");
        assert_eq!(id(500), "Inter-400");
        // > 500: up first.
        assert_eq!(id(550), "Inter-600");
        assert_eq!(id(650), "Inter-700");
        assert_eq!(id(900), "Inter-700");
        // < 400: down first, then up.
        assert_eq!(id(100), "Inter-400");
        assert_eq!(id(300), "Inter-400");
    }

    #[test]
    fn unknown_family_and_italic_fall_back_to_inter() {
        let db = FontDb::with_bundled();
        assert_eq!(db.face_id("Comic Sans", 700, false).unwrap(), "Inter-700");
        assert_eq!(db.face_id("sans-serif", 400, false).unwrap(), "Inter-400");
        assert_eq!(db.face_id("Nope, 'Inter'", 600, false).unwrap(), "Inter-600");
        // No italic bundled: the upright face is the CSS fallback.
        assert_eq!(db.face_id("Inter", 400, true).unwrap(), "Inter-400");
        assert!(FontDb::empty().face_id("Inter", 400, false).is_none());
    }

    #[test]
    fn add_font_returns_ids_and_replaces_duplicates() {
        let mut db = FontDb::with_bundled();
        let ids = db.add_font(bundled_file("Inter-Bold.ttf").unwrap().to_vec()).unwrap();
        assert_eq!(ids, vec!["Inter-700"]);
        assert_eq!(db.faces().len(), 3, "same id replaces, doesn't duplicate");
        assert!(matches!(db.add_font(b"not a font".to_vec()), Err(TextError::InvalidFont(_))));
        let mut empty = FontDb::empty();
        assert_eq!(empty.add_font(bundled_file("Inter-Regular.ttf").unwrap().to_vec()).unwrap(), vec!["Inter-400"]);
        assert_eq!(empty.face_id("Whatever", 700, false).unwrap(), "Inter-400");
    }

    /// A theme token's face is registered as the token says — family, weight, style — whatever
    /// its name table calls it, so a renamed or subset build still matches the token's stacks.
    #[test]
    fn faces_take_the_token_name() {
        let mut db = FontDb::empty();
        let ids = db.add_face(bundled_file("Inter-Bold.ttf").unwrap().to_vec(), "House Sans", 800, true).unwrap();
        assert_eq!(ids, vec!["House Sans-800-Italic"]);
        assert!(db.has_face("House Sans-800-Italic") && db.has_family("house sans") && !db.has_family("Inter"));
        assert_eq!(db.face_id("House Sans", 800, true).unwrap(), "House Sans-800-Italic");
        let (bytes, index) = db.face_data("House Sans-800-Italic").unwrap();
        assert_eq!((bytes.len(), index), (bundled_file("Inter-Bold.ttf").unwrap().len(), 0));
        assert_eq!(db.faces()[0].fs_type, 0, "Inter is installable");
        // Generic families mean the default family.
        assert!(!db.has_family("sans-serif") && FontDb::with_bundled().has_family("sans-serif"));
    }

    #[test]
    fn stack_resolution_dedupes_and_ends_with_default() {
        let db = FontDb::with_bundled();
        assert_eq!(db.resolve_stack("Inter, Foo, sans-serif", 400, false), vec![0]);
        assert_eq!(db.resolve_stack("Foo", 700, false), vec![2]);
        assert_eq!(split_families(" \"Noto Sans\" , 'Inter',sans-serif,"), vec!["Noto Sans", "Inter", "sans-serif"]);
    }

    #[test]
    fn metrics_are_sane() {
        let db = FontDb::with_bundled();
        let m = db.metrics("Inter-400", 100.0).unwrap();
        assert!(m.ascent > 80.0 && m.ascent < 110.0, "{m:?}");
        assert!(m.descent > 15.0 && m.descent < 35.0, "{m:?}");
        assert!(m.cap_height > 65.0 && m.cap_height < 80.0, "{m:?}");
        assert!(m.x_height > 45.0 && m.x_height < 60.0, "{m:?}");
        assert!(db.metrics("Nope-400", 10.0).is_none());
    }

    #[test]
    fn outline_unknown_face_is_none_and_space_is_empty() {
        let db = FontDb::with_bundled();
        assert!(db.outline("Nope-400", 1, 12.0).is_none());
        let space = db.glyph_for("Inter-400", ' ');
        // Space maps to a real glyph with no contours.
        let g = space.unwrap();
        assert!(db.outline("Inter-400", g, 12.0).unwrap().is_empty());
    }
}
