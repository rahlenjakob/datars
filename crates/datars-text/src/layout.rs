//! Text layout: paragraphs → font fallback items → UAX #14 segments → shaped pieces → greedy
//! lines → UAX #9 visual order → aligned, baseline-shifted glyph runs.
//!
//! Whitespace follows CSS `white-space: pre-wrap`: spaces are kept, lines wrap at break
//! opportunities, and trailing spaces at a line end hang (they neither count toward the width nor
//! get drawn). Words longer than `max_width` overflow rather than breaking mid-word.

use crate::fontdb::{lock, FontDb};
use crate::format;
use crate::shape::{ShapeCx, Shaped};
use datars_math::Rect;
use datars_scene::text::{Align, Baseline, TextStyle};
use datars_scene::{GlyphPos, TextNode, TextRun};
use skrifa::charmap::Charmap;
use skrifa::MetadataProvider;
use std::borrow::Cow;
use std::ops::Range;
use std::sync::Arc;
use unicode_bidi::ParagraphBidiInfo;
use unicode_linebreak::{linebreaks, BreakOpportunity};

/// Slack when comparing a line's width with `max_width`, so text measured with [`measure`] and
/// laid out again at exactly that width never wraps because of float summation order.
const FIT_EPSILON: f64 = 1e-6;

/// Shape and lay out `node.text` with `node.style`, filling `node.runs` (glyph positions in px
/// relative to `node.origin`, y down, each line's glyphs on its baseline) and `node.bounds`
/// (the line boxes — ascent to descent of the primary face — relative to `origin`).
///
/// If `node.number` is set, `node.text` is first regenerated from it with [`format::number`], so
/// an animated number only needs its value updated before re-laying out.
///
/// - `align`: `Start`/`End` put the line's start/end edge at the origin (start is the left edge
///   for left-to-right paragraphs and the right edge for right-to-left ones); `Middle` centres
///   each line on the origin.
/// - `baseline`: `Alphabetic` puts the first line's baseline at the origin; `Top`, `Middle` and
///   `Bottom` place the top, centre or bottom of the whole block (first line's ascent to last
///   line's descent) at the origin.
/// - Lines are `size × line_height` apart; `max_width` wraps at UAX #14 opportunities; `\n`
///   (also `\r\n`, `\r`, U+2028, U+2029) always breaks; each paragraph's bidi runs are ordered
///   visually per UAX #9 with the base direction taken from its first strong character.
pub fn layout(db: &FontDb, node: &mut TextNode) {
    if let Some(n) = &node.number {
        node.text = format::number(n.value, &n.format, &n.locale);
    }
    let (runs, bounds) = lay_out(db, &node.text, &node.style, true);
    node.runs = runs;
    node.bounds = bounds;
}

/// The bounds [`layout`] would produce for `text` in `style` (relative to the origin).
pub fn measure(db: &FontDb, text: &str, style: &TextStyle) -> Rect {
    lay_out(db, text, style, false).1
}

/// One line of laid-out text, as [`layout`] places it: what a host needs to lay real text (which
/// readers can select, copy and find) exactly over the drawn glyphs. Positions are px relative to
/// the text's origin, y down, before any scale or rotation.
#[derive(Clone, Debug, PartialEq)]
pub struct LineBox {
    /// The line's characters in logical order; the spaces that hang at a wrap are left out.
    pub text: String,
    /// The line's left edge.
    pub x: f64,
    /// The line's baseline.
    pub baseline: f64,
    /// The drawn advance (the width [`layout`] aligned the line by).
    pub width: f64,
    /// The primary face's ascent and descent: the line box spans `baseline - ascent` to
    /// `baseline + descent` (what [`layout`]'s bounds are made of).
    pub ascent: f64,
    pub descent: f64,
    /// A line break in the text follows this line (not a wrap).
    pub hard_break: bool,
    /// Its paragraph reads right to left (its first strong character is): the base direction
    /// its runs were ordered in, which a later line of it may not show by itself.
    pub rtl: bool,
}

/// The lines [`layout`] draws `text` in: the same breaks, alignment and baselines, with the
/// characters each line holds.
pub fn lines(db: &FontDb, text: &str, style: &TextStyle) -> Vec<LineBox> {
    let Some(b) = build_lines(db, text, style) else {
        return Vec::new();
    };
    let paras: Vec<Cow<'_, str>> = paragraphs(text).into_iter().map(clean).collect();
    let dy = block_shift(&b, style);
    let n = b.lines.len();
    b.lines
        .iter()
        .enumerate()
        .map(|(li, l)| LineBox {
            text: paras.get(l.para).and_then(|p| p.get(l.text.clone())).unwrap_or_default().to_string(),
            x: line_x(style.align, l.width),
            baseline: li as f64 * b.line_step + dy,
            width: l.width,
            ascent: b.ascent,
            descent: b.descent,
            hard_break: li + 1 < n && b.lines[li + 1].para != l.para,
            rtl: l.rtl,
        })
        .collect()
}

/// A shaped piece placed in a line (visual order within the line).
#[derive(Clone, Debug)]
struct Piece {
    /// Byte offset in the (cleaned) paragraph; used for bidi levels and tests.
    start: usize,
    face: usize,
    level: u8,
    shaped: Shaped,
    width: f64,
}

#[derive(Clone, Debug)]
struct Line {
    pieces: Vec<Piece>,
    width: f64,
    /// The paragraph it's in, and its characters there (bytes of the cleaned paragraph).
    para: usize,
    text: Range<usize>,
    /// The paragraph's base direction is right to left.
    rtl: bool,
}

/// A run of characters sharing one face and one bidi level.
#[derive(Clone, Copy, Debug)]
struct Item {
    start: usize,
    end: usize,
    face: usize,
    level: u8,
}

/// A UAX #14 segment (text between two break opportunities), split into its content and its
/// trailing spaces so the spaces can hang at a line end.
struct Segment {
    start: usize,
    end: usize,
    content: Range<usize>,
    spaces: Range<usize>,
    /// Where the trailing spaces start (bytes).
    content_end: usize,
    content_w: f64,
    spaces_w: f64,
    hard_break: bool,
}

struct Env<'a> {
    stack: Vec<usize>,
    charmaps: Vec<Charmap<'a>>,
    size: f64,
    max_width: Option<f64>,
}

struct Built {
    lines: Vec<Line>,
    ascent: f64,
    descent: f64,
    line_step: f64,
}

pub(crate) fn lay_out(db: &FontDb, text: &str, style: &TextStyle, want_glyphs: bool) -> (Vec<TextRun>, Rect) {
    let Some(built) = build_lines(db, text, style) else {
        return (Vec::new(), Rect::default());
    };
    position(db, &built, style, want_glyphs)
}

fn build_lines(db: &FontDb, text: &str, style: &TextStyle) -> Option<Built> {
    let size = style.size;
    if text.is_empty() || !(size.is_finite() && size > 0.0) || db.is_empty() {
        return None;
    }
    let stack = db.resolve_stack(&style.family, style.weight, false);
    let primary = db.face(*stack.first()?);
    let k = primary.scale(size);
    let fonts: Vec<_> = stack.iter().map(|&i| db.face(i).font()).collect();
    let env = Env {
        charmaps: fonts.iter().map(|f| f.charmap()).collect(),
        stack,
        size,
        max_width: style.max_width.filter(|w| !w.is_nan()),
    };
    let line_height = if style.line_height.is_finite() && style.line_height > 0.0 { style.line_height } else { 1.2 };
    let mut cache = lock(&db.shape_cache);
    let mut cx = ShapeCx::new(db);
    let mut lines = Vec::new();
    for (i, para) in paragraphs(text).into_iter().enumerate() {
        lay_paragraph(db, &clean(para), i, &env, &mut cx, &mut cache, &mut lines);
    }
    Some(Built { lines, ascent: primary.metrics.ascent * k, descent: primary.metrics.descent * k, line_step: size * line_height })
}

#[allow(clippy::too_many_arguments)]
fn lay_paragraph(db: &FontDb, para: &str, index: usize, env: &Env, cx: &mut ShapeCx, cache: &mut crate::shape::ShapeCache, lines: &mut Vec<Line>) {
    if para.is_empty() {
        lines.push(Line { pieces: Vec::new(), width: 0.0, para: index, text: 0..0, rtl: false });
        return;
    }
    note_missing(db, para, env);
    // Everything below U+0590 is left-to-right or neutral, so most labels skip bidi analysis.
    let bidi = if para.chars().any(|c| c as u32 >= 0x0590) { Some(ParagraphBidiInfo::new(para, None)) } else { None };
    let bidi = bidi.filter(|b| !b.is_pure_ltr || b.paragraph_level.is_rtl());
    let rtl = bidi.as_ref().is_some_and(|b| b.paragraph_level.is_rtl());
    let items = itemize(para, bidi.as_ref().map(|b| b.levels.as_slice()), env);

    // Segments and their shaped pieces, in logical order.
    let mut pieces: Vec<Piece> = Vec::new();
    let mut segs: Vec<Segment> = Vec::new();
    let mut prev = 0;
    for (pos, opp) in linebreaks(para) {
        if pos <= prev {
            continue;
        }
        let split = prev + trailing_space_start(&para[prev..pos]);
        let c0 = pieces.len();
        let content_w = push_pieces(db, para, prev..split, &items, env, cx, cache, &mut pieces);
        let c1 = pieces.len();
        let spaces_w = push_pieces(db, para, split..pos, &items, env, cx, cache, &mut pieces);
        segs.push(Segment {
            start: prev,
            end: pos,
            content: c0..c1,
            spaces: c1..pieces.len(),
            content_end: split,
            content_w,
            spaces_w,
            hard_break: opp == BreakOpportunity::Mandatory && pos < para.len(),
        });
        prev = pos;
    }

    for range in break_lines(&segs, env.max_width) {
        let last = range.end - 1;
        let mut line: Vec<Piece> = Vec::new();
        for (si, s) in segs[range.clone()].iter().enumerate() {
            line.extend(pieces[s.content.clone()].iter().cloned());
            if range.start + si != last {
                line.extend(pieces[s.spaces.clone()].iter().cloned());
            }
        }
        if let Some(b) = &bidi {
            // L1 (trailing whitespace etc.) is applied per line before reordering (L2).
            let levels = b.reordered_levels(segs[range.start].start..segs[last].end);
            for p in &mut line {
                p.level = levels[p.start].number();
            }
            let order = visual_order(&line.iter().map(|p| p.level).collect::<Vec<_>>());
            line = order.into_iter().map(|i| line[i].clone()).collect();
        }
        let width = line.iter().map(|p| p.width).sum();
        lines.push(Line { pieces: line, width, para: index, text: segs[range.start].start..segs[last].content_end, rtl });
    }
}

/// Shape the parts of `range` covered by each item; returns the total width in px.
#[allow(clippy::too_many_arguments)]
fn push_pieces(
    db: &FontDb,
    para: &str,
    range: Range<usize>,
    items: &[Item],
    env: &Env,
    cx: &mut ShapeCx,
    cache: &mut crate::shape::ShapeCache,
    out: &mut Vec<Piece>,
) -> f64 {
    let mut width = 0.0;
    if range.is_empty() {
        return width;
    }
    let first = items.partition_point(|it| it.end <= range.start);
    for it in &items[first..] {
        if it.start >= range.end {
            break;
        }
        let (s, e) = (it.start.max(range.start), it.end.min(range.end));
        let shaped = cx.shape(cache, it.face, it.level % 2 == 1, &para[s..e]);
        let w = shaped.advance as f64 * db.face(it.face).scale(env.size);
        width += w;
        out.push(Piece { start: s, face: it.face, level: it.level, shaped, width: w });
    }
    width
}

/// Greedy line filling over segments. A line always takes at least one segment with content,
/// so an over-long word overflows instead of producing empty lines.
fn break_lines(segs: &[Segment], max_width: Option<f64>) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let (mut start, mut w, mut has_content) = (0usize, 0.0f64, false);
    for (i, s) in segs.iter().enumerate() {
        if let Some(mw) = max_width {
            if i > start && has_content && !s.content.is_empty() && w + s.content_w > mw + FIT_EPSILON {
                out.push(start..i);
                (start, w, has_content) = (i, 0.0, false);
            }
        }
        w += s.content_w + s.spaces_w;
        has_content |= !s.content.is_empty();
        if s.hard_break {
            out.push(start..i + 1);
            (start, w, has_content) = (i + 1, 0.0, false);
        }
    }
    if start < segs.len() || out.is_empty() {
        out.push(start..segs.len().max(start));
    }
    out.retain(|r| !r.is_empty());
    if out.is_empty() {
        out.push(0..segs.len());
    }
    out
}

/// UAX #9 rule L2 over pieces: from the highest level down to the lowest odd level, reverse
/// every maximal run at that level or higher. Each piece is already shaped in its own direction,
/// so its glyphs keep their (visual) order.
fn visual_order(levels: &[u8]) -> Vec<usize> {
    let n = levels.len();
    let mut order: Vec<usize> = (0..n).collect();
    let (Some(&max), Some(&min)) = (levels.iter().max(), levels.iter().min()) else {
        return order;
    };
    let lowest_odd = min | 1;
    let mut lvl = max;
    while lvl >= lowest_odd {
        let mut i = 0;
        while i < n {
            if levels[order[i]] >= lvl {
                let mut j = i;
                while j < n && levels[order[j]] >= lvl {
                    j += 1;
                }
                order[i..j].reverse();
                i = j;
            } else {
                i += 1;
            }
        }
        lvl -= 1;
    }
    order
}

/// Assign each character a face (first face in the stack that has it; marks stay with their
/// base) and a bidi level, and merge neighbours that share both.
fn itemize(para: &str, levels: Option<&[unicode_bidi::Level]>, env: &Env) -> Vec<Item> {
    if env.stack.len() == 1 && levels.is_none() {
        return vec![Item { start: 0, end: para.len(), face: env.stack[0], level: 0 }];
    }
    let mut items: Vec<Item> = Vec::new();
    let mut prev_face: Option<usize> = None;
    for (i, c) in para.char_indices() {
        let level = levels.map_or(0, |l| l[i].number());
        let face = pick_face(c, prev_face, env);
        prev_face = Some(face);
        let end = i + c.len_utf8();
        match items.last_mut() {
            Some(it) if it.face == face && it.level == level => it.end = end,
            _ => items.push(Item { start: i, end, face, level }),
        }
    }
    items
}

/// Record the characters no face of the stack has (they draw as .notdef), so a runtime can fetch
/// the lazily loaded script subsets a bundle offers for them. Whitespace, controls, marks and
/// default-ignorable characters are not drawn from a face of their own, so they don't count.
fn note_missing(db: &FontDb, para: &str, env: &Env) {
    for c in para.chars() {
        if c.is_whitespace() || c.is_control() || is_cluster_extender(c) || is_default_ignorable(c) {
            continue;
        }
        if !env.charmaps.iter().any(|cm| cm.map(c).is_some_and(|g| g.to_u32() != 0)) {
            db.note_missing(c);
        }
    }
}

/// Default-ignorable code points shapers hide rather than draw (UAX #44 subset).
fn is_default_ignorable(c: char) -> bool {
    matches!(c as u32, 0x00AD | 0x034F | 0x061C | 0x115F..=0x1160 | 0x17B4..=0x17B5 | 0x180B..=0x180F | 0x200B..=0x200F | 0x202A..=0x202E | 0x2060..=0x206F | 0x3164 | 0xFE00..=0xFE0F | 0xFEFF | 0xFFA0 | 0x1BCA0..=0x1BCA3 | 0x1D173..=0x1D17A | 0xE0000..=0xE0FFF)
}

fn pick_face(c: char, prev: Option<usize>, env: &Env) -> usize {
    if env.stack.len() == 1 {
        return env.stack[0];
    }
    if let Some(p) = prev {
        if is_cluster_extender(c) {
            return p;
        }
    }
    for (k, cm) in env.charmaps.iter().enumerate() {
        if cm.map(c).is_some_and(|g| g.to_u32() != 0) {
            return env.stack[k];
        }
    }
    // No face has it: keep whitespace with its neighbour, otherwise .notdef of the primary face.
    match prev {
        Some(p) if c.is_whitespace() => p,
        _ => env.stack[0],
    }
}

/// Characters that extend the previous grapheme (combining marks, joiners, variation selectors,
/// emoji modifiers and tags) and must be shaped in the same face as their base.
fn is_cluster_extender(c: char) -> bool {
    matches!(c as u32,
        0x0300..=0x036F | 0x0483..=0x0489 | 0x0591..=0x05BD | 0x05BF | 0x05C1..=0x05C2 | 0x05C4..=0x05C5
        | 0x05C7 | 0x0610..=0x061A | 0x064B..=0x065F | 0x0670 | 0x06D6..=0x06DC | 0x06DF..=0x06E4
        | 0x06E7..=0x06E8 | 0x06EA..=0x06ED | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x200C | 0x200D
        | 0x20D0..=0x20FF | 0xFE00..=0xFE0F | 0xFE20..=0xFE2F | 0x1F3FB..=0x1F3FF | 0xE0020..=0xE007F
        | 0xE0100..=0xE01EF)
}

/// Spaces that may hang at a line end (UAX #14 class SP and the breaking typographic spaces).
fn is_break_space(c: char) -> bool {
    matches!(c, ' ' | '\u{1680}' | '\u{2000}'..='\u{2006}' | '\u{2008}'..='\u{200A}' | '\u{205F}' | '\u{3000}')
}

fn trailing_space_start(s: &str) -> usize {
    let mut end = s.len();
    for (i, c) in s.char_indices().rev() {
        if !is_break_space(c) {
            break;
        }
        end = i;
    }
    end
}

/// Split at hard line breaks: LF, CR LF, CR, VT, FF, NEL, LS, PS. `n` breaks give `n + 1`
/// paragraphs (a trailing newline yields a final empty line).
fn paragraphs(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut it = text.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        if matches!(c, '\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}') {
            out.push(&text[start..i]);
            let mut end = i + c.len_utf8();
            if c == '\r' && matches!(it.peek(), Some(&(_, '\n'))) {
                it.next();
                end += 1;
            }
            start = end;
        }
    }
    out.push(&text[start..]);
    out
}

/// Remaining control characters (tabs included) render as spaces rather than .notdef boxes.
fn clean(para: &str) -> Cow<'_, str> {
    if para.chars().any(|c| c.is_control()) {
        Cow::Owned(para.chars().map(|c| if c.is_control() { ' ' } else { c }).collect())
    } else {
        Cow::Borrowed(para)
    }
}

/// Place lines: horizontal alignment per line, vertical baseline shift for the block, and group
/// glyphs into one run per face (in order of first appearance).
/// The block's top and bottom (first line's ascent, last line's descent) around a first baseline
/// at 0.
fn block_extent(b: &Built) -> (f64, f64) {
    let n = b.lines.len().max(1);
    (-b.ascent, (n - 1) as f64 * b.line_step + b.descent)
}

/// How far `style.baseline` moves the block from a first baseline at the origin.
fn block_shift(b: &Built, style: &TextStyle) -> f64 {
    let (top, bottom) = block_extent(b);
    match style.baseline {
        Baseline::Alphabetic => 0.0,
        Baseline::Top => -top,
        Baseline::Bottom => -bottom,
        Baseline::Middle => -(top + bottom) / 2.0,
    }
}

/// A line's left edge for its width. Physical, whatever the direction: charts place labels by
/// side (an axis label ends at its tick), so right-to-left text aligned `end` still ends at the
/// anchor on the right.
fn line_x(align: Align, w: f64) -> f64 {
    let x = match align {
        Align::Start => 0.0,
        Align::Middle => -w / 2.0,
        Align::End => -w,
    };
    x + 0.0 // normalise -0.0
}

fn position(db: &FontDb, b: &Built, style: &TextStyle, want_glyphs: bool) -> (Vec<TextRun>, Rect) {
    let (top, bottom) = block_extent(b);
    let dy = block_shift(b, style);
    let mut runs: Vec<(usize, Vec<GlyphPos>)> = Vec::new();
    let (mut x0, mut x1) = (f64::INFINITY, f64::NEG_INFINITY);
    for (li, line) in b.lines.iter().enumerate() {
        let w = line.width;
        let lx = line_x(style.align, w);
        x0 = x0.min(lx);
        x1 = x1.max(lx + w);
        if !want_glyphs {
            continue;
        }
        let baseline = li as f64 * b.line_step + dy;
        let mut pen = 0.0;
        for p in &line.pieces {
            let k = db.face(p.face).scale(style.size);
            let ri = match runs.iter().position(|(f, _)| *f == p.face) {
                Some(ri) => ri,
                None => {
                    runs.push((p.face, Vec::new()));
                    runs.len() - 1
                }
            };
            let glyphs = &mut runs[ri].1;
            // Integer font units within a piece, so a glyph's x doesn't drift with its index.
            let mut units: i64 = 0;
            for g in p.shaped.glyphs.iter() {
                let x = lx + pen + (units + g.dx as i64) as f64 * k;
                let y = baseline - g.dy as f64 * k;
                glyphs.push(GlyphPos { id: g.id, x: x as f32, y: y as f32 });
                units += g.adv as i64;
            }
            pen += p.width;
        }
    }
    let runs = runs
        .into_iter()
        .map(|(face, glyphs)| TextRun { font: db.face(face).id.clone(), size: style.size, ink: style.ink.clone(), glyphs: Arc::from(glyphs) })
        .collect();
    let bounds = Rect::new(x0, top + dy, x1 - x0, bottom - top);
    (runs, bounds)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style(size: f64) -> TextStyle {
        TextStyle { size, ..TextStyle::default() }
    }

    /// Byte offsets of each line's pieces, in visual order.
    fn piece_starts(text: &str) -> Vec<Vec<usize>> {
        let db = FontDb::with_bundled();
        let b = build_lines(&db, text, &style(12.0)).unwrap();
        b.lines.iter().map(|l| l.pieces.iter().map(|p| p.start).collect()).collect()
    }

    #[test]
    fn l2_reorders_runs() {
        assert_eq!(visual_order(&[0, 0, 0]), vec![0, 1, 2]);
        assert_eq!(visual_order(&[1, 1, 1]), vec![2, 1, 0]);
        assert_eq!(visual_order(&[0, 1, 1, 0]), vec![0, 2, 1, 3]);
        // RTL paragraph with an embedded LTR run (level 2): the LTR run stays in logical order
        // internally but moves as a unit.
        assert_eq!(visual_order(&[1, 2, 2, 1]), vec![3, 1, 2, 0]);
        assert_eq!(visual_order(&[]), Vec::<usize>::new());
    }

    #[test]
    fn hebrew_words_reverse_inside_ltr_paragraph() {
        // "abc " (0..4), "אבג " (4..11), "דהו " (11..18), "def" (18..21). The two Hebrew words and
        // the space between them are one RTL run, displayed right-to-left.
        let text = "abc אבג דהו def";
        let starts = piece_starts(text);
        assert_eq!(starts.len(), 1);
        let hebrew1 = text.find('א').unwrap();
        let hebrew2 = text.find('ד').unwrap();
        let pos = |b: usize| starts[0].iter().position(|&s| s == b).unwrap();
        assert!(pos(hebrew2) < pos(hebrew1), "second Hebrew word is drawn left of the first: {starts:?}");
        assert_eq!(starts[0].first(), Some(&0), "Latin start stays leftmost");
        assert_eq!(starts[0].last(), Some(&text.find('d').unwrap()), "Latin end stays rightmost");
    }

    #[test]
    fn rtl_paragraph_puts_first_word_rightmost() {
        let text = "אבג abc";
        let starts = piece_starts(text);
        assert_eq!(starts[0].last(), Some(&0), "logical first (Hebrew) is visually last: {starts:?}");
        assert_eq!(starts[0].first(), Some(&text.find('a').unwrap()));
    }

    #[test]
    fn pure_ltr_keeps_logical_order() {
        let starts = piece_starts("one two three");
        let mut sorted = starts[0].clone();
        sorted.sort();
        assert_eq!(starts[0], sorted);
    }

    #[test]
    fn paragraph_splitting() {
        assert_eq!(paragraphs("a\nb"), vec!["a", "b"]);
        assert_eq!(paragraphs("a\r\nb\rc"), vec!["a", "b", "c"]);
        assert_eq!(paragraphs("a\n"), vec!["a", ""]);
        assert_eq!(paragraphs("a\u{2028}b"), vec!["a", "b"]);
        assert_eq!(paragraphs(""), vec![""]);
    }

    /// `lines` is `layout` line by line: the same boxes (their union is the layout's bounds), the
    /// same baselines as the drawn glyphs, and each line's own characters.
    #[test]
    fn lines_match_the_layout_they_describe() {
        let db = FontDb::with_bundled();
        let st = TextStyle { size: 14.0, max_width: Some(90.0), align: Align::Middle, baseline: Baseline::Top, ..TextStyle::default() };
        let text = "Stockholm is the capital\nOslo";
        let ls = lines(&db, text, &st);
        assert!(ls.len() >= 3, "wrapped, then a hard break: {ls:?}");
        let joined: Vec<&str> = ls.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(joined.last(), Some(&"Oslo"));
        assert_eq!(joined[..joined.len() - 1].join(" "), "Stockholm is the capital", "wraps drop only the spaces they hang: {joined:?}");
        assert!(ls.iter().all(|l| !l.text.ends_with(' ')));
        let breaks: Vec<bool> = ls.iter().map(|l| l.hard_break).collect();
        assert_eq!(breaks.iter().filter(|b| **b).count(), 1, "{breaks:?}");
        assert!(breaks[ls.len() - 2], "the newline follows the wrapped paragraph's last line");
        let mut node = TextNode::new(text, datars_math::Vec2::ZERO, st.clone());
        layout(&db, &mut node);
        // The union of the line boxes is the layout's bounds.
        let (x0, x1) = ls.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), l| (a.min(l.x), b.max(l.x + l.width)));
        let top = ls[0].baseline - ls[0].ascent;
        let last = ls.last().unwrap();
        let bottom = last.baseline + last.descent;
        for (got, want) in [(x0, node.bounds.x), (x1, node.bounds.x1()), (top, node.bounds.y), (bottom, node.bounds.y1())] {
            assert!((got - want).abs() < 1e-9, "{got} vs {want}: {ls:?} {:?}", node.bounds);
        }
        // Every drawn glyph sits on one of the lines' baselines, inside its line's span.
        for g in node.runs.iter().flat_map(|r| r.glyphs.iter()) {
            let l = ls.iter().find(|l| (l.baseline - g.y as f64).abs() < 1e-3).expect("a glyph off every baseline");
            assert!(g.x as f64 >= l.x - 1e-3 && (g.x as f64) < l.x + l.width, "{g:?} outside {l:?}");
        }
        assert!(lines(&db, "", &st).is_empty());
        assert!(ls.iter().all(|l| !l.rtl));
        // A right-to-left paragraph says so on every line, whatever a line starts with.
        let rtl = lines(&db, "אבג 2024 abc def ghi jkl", &TextStyle { max_width: Some(40.0), ..st });
        assert!(rtl.len() >= 2 && rtl.iter().all(|l| l.rtl), "{rtl:?}");
    }

    #[test]
    fn trailing_spaces_and_controls() {
        assert_eq!(trailing_space_start("abc  "), 3);
        assert_eq!(trailing_space_start("abc"), 3);
        assert_eq!(trailing_space_start("   "), 0);
        assert_eq!(clean("a\tb"), "a b");
        assert!(matches!(clean("ab"), Cow::Borrowed(_)));
    }
}
