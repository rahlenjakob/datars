//! Shaping one piece of text (a line-break segment within one bidi level and one face) with
//! harfrust, memoized.
//!
//! Results stay in integer font units; layout converts to px. Kerning, ligatures and contextual
//! alternates are HarfBuzz defaults, so no user features are passed. Shape plans are expensive to
//! build and are cached per (face, direction, script); shaped pieces are cached per
//! (face, direction, text) — the same short strings recur constantly (repeated labels, animated numbers).

use crate::fontdb::FontDb;
use harfrust::{Direction, ShapePlan, Shaper, UnicodeBuffer};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Bound on memoized pieces; clearing is safe (pure memoization) and keeps memory flat.
const WORD_CACHE_MAX: usize = 32_768;

/// One shaped glyph in font units. `dx`/`dy` are HarfBuzz offsets (y up).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ShapedGlyph {
    pub id: u16,
    pub adv: i32,
    pub dx: i32,
    pub dy: i32,
}

/// A shaped piece: glyphs in visual (left-to-right) order and the total advance.
#[derive(Clone, Debug)]
pub(crate) struct Shaped {
    pub glyphs: Arc<[ShapedGlyph]>,
    pub advance: i64,
}

#[derive(Default)]
pub(crate) struct ShapeCache {
    plans: BTreeMap<(usize, bool, u32), ShapePlan>,
    words: BTreeMap<(usize, bool), BTreeMap<String, Shaped>>,
    word_count: usize,
}

impl ShapeCache {
    pub fn clear(&mut self) {
        self.plans.clear();
        self.words.clear();
        self.word_count = 0;
    }
}

/// Per-layout shaping state: shapers are built lazily per face and reused for every piece.
pub(crate) struct ShapeCx<'a> {
    db: &'a FontDb,
    shapers: BTreeMap<usize, Shaper<'a>>,
    buffer: Option<UnicodeBuffer>,
}

impl<'a> ShapeCx<'a> {
    pub fn new(db: &'a FontDb) -> ShapeCx<'a> {
        ShapeCx { db, shapers: BTreeMap::new(), buffer: None }
    }

    /// Shape `text` in face `face`, right-to-left if `rtl`.
    pub fn shape(&mut self, cache: &mut ShapeCache, face: usize, rtl: bool, text: &str) -> Shaped {
        if let Some(s) = cache.words.get(&(face, rtl)).and_then(|m| m.get(text)) {
            return s.clone();
        }
        let db = self.db;
        let shaper = self.shapers.entry(face).or_insert_with(|| {
            let f = db.face(face);
            f.shaper_data.shaper(&f.font()).build()
        });
        let dir = if rtl { Direction::RightToLeft } else { Direction::LeftToRight };
        let mut buf = self.buffer.take().unwrap_or_default();
        buf.push_str(text);
        buf.set_direction(dir);
        buf.guess_segment_properties();
        let script = buf.script();
        // Pin the guessed script (UNKNOWN when the piece is all digits/punctuation) so the buffer
        // and the cached plan always agree.
        buf.set_script(script);
        let tag = u32::from_be_bytes(script.tag().to_be_bytes());
        let plan = cache.plans.entry((face, rtl, tag)).or_insert_with(|| ShapePlan::new(shaper, dir, Some(script), None, &[]));
        let out = shaper.shape_with_plan(plan, buf, &[]);
        let mut advance: i64 = 0;
        let glyphs: Arc<[ShapedGlyph]> = out
            .glyph_infos()
            .iter()
            .zip(out.glyph_positions())
            .map(|(info, pos)| {
                advance += pos.x_advance as i64;
                ShapedGlyph { id: info.glyph_id.min(u16::MAX as u32) as u16, adv: pos.x_advance, dx: pos.x_offset, dy: pos.y_offset }
            })
            .collect();
        self.buffer = Some(out.clear());
        let shaped = Shaped { glyphs, advance };
        if cache.word_count >= WORD_CACHE_MAX {
            cache.words.clear();
            cache.word_count = 0;
        }
        cache.words.entry((face, rtl)).or_default().insert(text.to_string(), shaped.clone());
        cache.word_count += 1;
        shaped
    }
}
