//! Integration tests for datars-text's public API: fonts, shaping, layout, outlines, formatting.

use datars_math::{PathEl, Rect, Vec2};
use datars_render::GlyphSource;
use datars_scene::text::{Align, Baseline, TextStyle};
use datars_scene::{NumberText, TextNode};
use datars_text::{format, layout, measure, FontDb};
use datars_theme::Ink;
use std::sync::{Arc, OnceLock};

fn db() -> &'static FontDb {
    static DB: OnceLock<FontDb> = OnceLock::new();
    DB.get_or_init(FontDb::with_bundled)
}

fn style(size: f64) -> TextStyle {
    TextStyle { size, ..TextStyle::default() }
}

fn laid(text: &str, style: TextStyle) -> TextNode {
    let mut n = TextNode::new(text, Vec2::ZERO, style);
    layout(db(), &mut n);
    n
}

fn all_glyphs(n: &TextNode) -> Vec<datars_scene::GlyphPos> {
    n.runs.iter().flat_map(|r| r.glyphs.iter().copied()).collect()
}

/// Distinct baselines (glyph y values), sorted.
fn baselines(n: &TextNode) -> Vec<f32> {
    let mut ys: Vec<f32> = all_glyphs(n).iter().map(|g| g.y).collect();
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ys.dedup();
    ys
}

#[test]
fn hello_world_glyphs_advance_left_to_right() {
    let n = laid("Hello, world", style(16.0));
    assert_eq!(n.runs.len(), 1);
    let r = &n.runs[0];
    assert_eq!(&*r.font, "Inter-400");
    assert_eq!(r.size, 16.0);
    let g = &r.glyphs;
    assert_eq!(g.len(), "Hello, world".chars().count());
    assert!(g.iter().all(|g| g.id != 0), "no .notdef");
    for w in g.windows(2) {
        assert!(w[1].x > w[0].x, "x increases: {:?}", g);
    }
    assert!(g.iter().all(|g| g.y == 0.0), "alphabetic baseline at origin");
    assert_eq!(g[0].x, 0.0, "start-aligned at origin");
}

#[test]
fn swedish_letters_have_real_glyphs() {
    let n = laid("Åre ÖÄ åäö", style(12.0));
    let g = all_glyphs(&n);
    assert_eq!(g.len(), "Åre ÖÄ åäö".chars().count());
    assert!(g.iter().all(|g| g.id != 0), "{g:?}");
    for ch in ['Å', 'Ä', 'Ö', 'å', 'ä', 'ö', 'é', 'ü', 'ß', 'ø', 'æ'] {
        assert!(db().glyph_for("Inter-400", ch).is_some(), "{ch}");
    }
}

#[test]
fn bold_is_wider_than_regular() {
    let text = "Revenue by quarter";
    let regular = measure(db(), text, &style(14.0));
    let semibold = measure(db(), text, &TextStyle { weight: 600, ..style(14.0) });
    let bold = measure(db(), text, &TextStyle { weight: 700, ..style(14.0) });
    assert!(semibold.w > regular.w, "{semibold:?} vs {regular:?}");
    assert!(bold.w > semibold.w, "{bold:?} vs {semibold:?}");
    let n = laid(text, TextStyle { weight: 700, ..style(14.0) });
    assert_eq!(&*n.runs[0].font, "Inter-700");
}

#[test]
fn max_width_wraps_and_height_grows() {
    let text = "The quick brown fox jumps over the lazy dog";
    let one = laid(text, style(12.0));
    let wrapped = laid(text, TextStyle { max_width: Some(80.0), ..style(12.0) });
    assert_eq!(baselines(&one).len(), 1);
    let lines = baselines(&wrapped);
    assert!(lines.len() >= 3, "{lines:?}");
    assert!(wrapped.bounds.h > one.bounds.h * 2.0, "{:?} vs {:?}", wrapped.bounds, one.bounds);
    assert!(wrapped.bounds.w <= 80.0 + 1e-6, "{:?}", wrapped.bounds);
    // Lines are size × line_height apart.
    for w in lines.windows(2) {
        assert!(((w[1] - w[0]) as f64 - 12.0 * 1.2).abs() < 1e-3, "{lines:?}");
    }
    // Wrapping never drops glyphs except the hanging spaces at line ends.
    let spaces = text.matches(' ').count();
    assert_eq!(all_glyphs(&wrapped).len(), text.chars().count() - (lines.len() - 1));
    assert_eq!(all_glyphs(&one).len(), text.chars().count());
    assert!(spaces >= lines.len() - 1);
}

#[test]
fn measured_width_does_not_wrap_when_used_as_max_width() {
    let text = "Wide enough exactly";
    let w = measure(db(), text, &style(13.0)).w;
    let n = laid(text, TextStyle { max_width: Some(w), ..style(13.0) });
    assert_eq!(baselines(&n).len(), 1);
    let n = laid(text, TextStyle { max_width: Some(w - 1.0), ..style(13.0) });
    assert_eq!(baselines(&n).len(), 2);
}

#[test]
fn overlong_word_overflows_instead_of_breaking() {
    let n = laid("Supercalifragilistic", TextStyle { max_width: Some(10.0), ..style(12.0) });
    assert_eq!(baselines(&n).len(), 1);
    assert!(n.bounds.w > 10.0);
}

#[test]
fn explicit_newlines_break_lines() {
    let n = laid("first\nsecond\r\nthird", style(10.0));
    let lines = baselines(&n);
    assert_eq!(lines, vec![0.0, 12.0, 24.0]);
    let n2 = laid("a\n\nb", style(10.0));
    assert_eq!(baselines(&n2), vec![0.0, 24.0], "an empty paragraph still takes a line");
    let m = db().metrics("Inter-400", 10.0).unwrap();
    assert!((n2.bounds.h - (24.0 + m.ascent + m.descent)).abs() < 1e-9);
}

#[test]
fn align_middle_centres_each_line_on_origin() {
    let st = TextStyle { align: Align::Middle, max_width: Some(90.0), ..style(12.0) };
    let n = laid("Short line and a much longer second line here", st.clone());
    assert!((n.bounds.x + n.bounds.w / 2.0).abs() < 1e-9, "{:?}", n.bounds);
    // Every line is individually centred: its first glyph starts at −width/2.
    let single = laid("Centred", TextStyle { align: Align::Middle, ..style(12.0) });
    let w = measure(db(), "Centred", &style(12.0)).w;
    assert!((single.runs[0].glyphs[0].x as f64 + w / 2.0).abs() < 1e-4);
    assert!((single.bounds.x + w / 2.0).abs() < 1e-9 && (single.bounds.w - w).abs() < 1e-9);
    for y in baselines(&n) {
        let line: Vec<_> = all_glyphs(&n).into_iter().filter(|g| g.y == y).collect();
        let left = line.iter().map(|g| g.x).fold(f32::INFINITY, f32::min) as f64;
        assert!(left < 0.0, "line at {y} starts left of origin");
    }
}

#[test]
fn align_end_puts_right_edge_at_origin() {
    let n = laid("Right", TextStyle { align: Align::End, ..style(12.0) });
    assert!((n.bounds.x + n.bounds.w).abs() < 1e-9, "{:?}", n.bounds);
    assert!(n.runs[0].glyphs[0].x < 0.0);
}

#[test]
fn baselines_shift_the_block() {
    let m = db().metrics("Inter-400", 20.0).unwrap();
    let at = |b: Baseline| laid("Hxg", TextStyle { baseline: b, ..style(20.0) });
    let alpha = at(Baseline::Alphabetic);
    assert!((alpha.bounds.y + m.ascent).abs() < 1e-9);
    assert!((alpha.bounds.h - (m.ascent + m.descent)).abs() < 1e-9);
    let top = at(Baseline::Top);
    assert!(top.bounds.y.abs() < 1e-9, "{:?}", top.bounds);
    assert!((top.runs[0].glyphs[0].y as f64 - m.ascent).abs() < 1e-4);
    let bottom = at(Baseline::Bottom);
    assert!((bottom.bounds.y + bottom.bounds.h).abs() < 1e-9, "{:?}", bottom.bounds);
    let middle = at(Baseline::Middle);
    assert!((middle.bounds.y + middle.bounds.h / 2.0).abs() < 1e-9, "{:?}", middle.bounds);
    // Middle of a two-line block is between the lines.
    let two = laid("a\nb", TextStyle { baseline: Baseline::Middle, ..style(20.0) });
    assert!((two.bounds.y + two.bounds.h / 2.0).abs() < 1e-9);
    let ys = baselines(&two);
    assert!(ys[0] < 0.0 && ys[1] > 0.0, "{ys:?}");
}

#[test]
fn runs_carry_ink_and_face() {
    let ink = Ink::palette("category", 2);
    let n = laid("Label", TextStyle { ink: ink.clone(), weight: 600, ..style(11.0) });
    assert_eq!(n.runs.len(), 1);
    assert_eq!(n.runs[0].ink, ink);
    assert_eq!(&*n.runs[0].font, "Inter-600");
    assert_eq!(n.runs[0].size, 11.0);
}

#[test]
fn family_stack_falls_back_to_bundled_faces() {
    let n = laid("Fallback", TextStyle { family: Arc::from("\"Nonexistent Sans\", Inter, sans-serif"), ..style(12.0) });
    assert_eq!(&*n.runs[0].font, "Inter-400");
    assert!(n.runs[0].glyphs.iter().all(|g| g.id != 0));
}

static INTER_REGULAR: &[u8] = include_bytes!("../../../fonts/Inter-Regular.ttf");

fn be16(b: &[u8], o: usize) -> usize {
    u16::from_be_bytes([b[o], b[o + 1]]) as usize
}
fn be32(b: &[u8], o: usize) -> usize {
    u32::from_be_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) as usize
}
fn table(b: &[u8], tag: &[u8; 4]) -> (usize, usize) {
    (0..be16(b, 4)).map(|i| 12 + 16 * i).find(|&r| &b[r..r + 4] == tag).map(|r| (be32(b, r + 8), be32(b, r + 12))).expect("table")
}
fn replace_all(hay: &mut [u8], from: &[u8], to: &[u8]) {
    let mut i = 0;
    while i + from.len() <= hay.len() {
        if &hay[i..i + from.len()] == from {
            hay[i..i + to.len()].copy_from_slice(to);
            i += from.len();
        } else {
            i += 1;
        }
    }
}

/// A test font: Inter Regular renamed to family "Intex", with U+00C5 (Å) unmapped in every cmap
/// subtable (its range truncated, shifted, or pointed at glyph 0), so "Intex" lacks Å while Inter
/// has it. Checksums aren't verified by the parser, so byte patches suffice.
fn intex_without_ring_a() -> Vec<u8> {
    let mut b = INTER_REGULAR.to_vec();
    let (no, nl) = table(&b, b"name");
    let utf16 = |s: &str| s.encode_utf16().flat_map(|u| u.to_be_bytes()).collect::<Vec<u8>>();
    replace_all(&mut b[no..no + nl], &utf16("Inter"), &utf16("Intex"));
    replace_all(&mut b[no..no + nl], b"Inter", b"Intex");
    let (co, _) = table(&b, b"cmap");
    let mut patched = 0;
    for i in 0..be16(&b, co + 2) {
        let sub = co + be32(&b, co + 4 + 8 * i + 4);
        match be16(&b, sub) {
            4 => {
                let seg_x2 = be16(&b, sub + 6);
                let ends = sub + 14;
                let starts = ends + seg_x2 + 2;
                let deltas = starts + seg_x2;
                let range_offsets = deltas + seg_x2;
                for sgi in 0..seg_x2 / 2 {
                    let (st, e) = (be16(&b, starts + 2 * sgi), be16(&b, ends + 2 * sgi));
                    if !(st <= 0xC5 && 0xC5 <= e) {
                        continue;
                    }
                    let ro_at = range_offsets + 2 * sgi;
                    let ro = be16(&b, ro_at);
                    let put16 = |b: &mut Vec<u8>, at: usize, v: u16| b[at..at + 2].copy_from_slice(&v.to_be_bytes());
                    if ro != 0 {
                        // glyphIdArray entry for U+00C5 → glyph 0.
                        put16(&mut b, ro_at + ro + 2 * (0xC5 - st), 0);
                    } else if st < 0xC5 {
                        put16(&mut b, ends + 2 * sgi, 0xC4);
                    } else if e > 0xC5 {
                        put16(&mut b, starts + 2 * sgi, 0xC6);
                    } else {
                        // c + idDelta ≡ 0 (mod 65536).
                        put16(&mut b, deltas + 2 * sgi, (0x1_0000 - 0xC5) as u16);
                    }
                    patched += 1;
                }
            }
            12 => {
                for g in 0..be32(&b, sub + 12) {
                    let r = sub + 16 + 12 * g;
                    let (st, e, gid) = (be32(&b, r), be32(&b, r + 4), be32(&b, r + 8));
                    if !(st <= 0xC5 && 0xC5 <= e) {
                        continue;
                    }
                    let put32 = |b: &mut Vec<u8>, at: usize, v: usize| b[at..at + 4].copy_from_slice(&(v as u32).to_be_bytes());
                    if st < 0xC5 {
                        put32(&mut b, r + 4, 0xC4);
                    } else if e > 0xC5 {
                        put32(&mut b, r, 0xC6);
                        put32(&mut b, r + 8, gid + 1);
                    } else {
                        put32(&mut b, r + 8, 0);
                    }
                    patched += 1;
                }
            }
            _ => {}
        }
    }
    assert!(patched > 0);
    b
}

#[test]
fn family_stack_falls_back_per_glyph() {
    let mut db = FontDb::with_bundled();
    assert_eq!(db.add_font(intex_without_ring_a()).unwrap(), vec!["Intex-400"]);
    assert!(db.glyph_for("Intex-400", 'Å').is_none());
    assert!(db.glyph_for("Intex-400", 'A').is_some());
    assert_eq!(db.face_id("Intex", 700, false).unwrap(), "Intex-400", "nearest weight within the family");

    for family in ["Intex, Inter", "Intex"] {
        let mut n = TextNode::new("AÅA", Vec2::ZERO, TextStyle { family: Arc::from(family), ..style(20.0) });
        layout(&db, &mut n);
        let fonts: Vec<&str> = n.runs.iter().map(|r| &*r.font).collect();
        assert_eq!(fonts, vec!["Intex-400", "Inter-400"], "{family}");
        let (primary, fallback) = (&n.runs[0].glyphs, &n.runs[1].glyphs);
        assert_eq!(primary.len(), 2);
        assert_eq!(fallback.len(), 1);
        assert_ne!(fallback[0].id, 0, "Å comes from the fallback face, not .notdef");
        assert!(primary[0].x < fallback[0].x && fallback[0].x < primary[1].x, "visual order kept across faces");
    }

    // A combining mark stays in its base character's face (Intex has U+0301, but Å is from Inter).
    let mut n = TextNode::new("Å\u{301}", Vec2::ZERO, TextStyle { family: Arc::from("Intex, Inter"), ..style(20.0) });
    layout(&db, &mut n);
    let fonts: Vec<&str> = n.runs.iter().map(|r| &*r.font).collect();
    assert_eq!(fonts, vec!["Inter-400"]);
}

#[test]
fn fontdb_is_send_and_sync() {
    fn check<T: Send + Sync>() {}
    check::<FontDb>();
}

#[test]
fn missing_glyphs_are_notdef_not_dropped() {
    // Inter has no Hebrew: glyphs come out as .notdef (id 0) but still occupy space.
    let n = laid("שלום", style(12.0));
    let g = all_glyphs(&n);
    assert_eq!(g.len(), 4);
    assert!(g.iter().all(|g| g.id == 0));
    assert!(n.bounds.w > 0.0);
}

#[test]
fn bidi_rtl_paragraph_orders_runs_visually() {
    // RTL base direction (first strong char is Hebrew): the Latin word is drawn LEFT of the
    // Hebrew. Alignment stays physical: "start" puts the left edge at the origin.
    let n = laid("שלום abc", style(12.0));
    let g = all_glyphs(&n);
    let abc: Vec<u16> = ['a', 'b', 'c'].iter().map(|&ch| db().glyph_for("Inter-400", ch).unwrap()).collect();
    let latin: Vec<_> = g.iter().filter(|g| abc.contains(&g.id)).collect();
    let hebrew: Vec<_> = g.iter().filter(|g| g.id == 0).collect();
    assert_eq!(latin.len(), 3);
    assert_eq!(hebrew.len(), 4);
    let latin_max = latin.iter().map(|g| g.x).fold(f32::NEG_INFINITY, f32::max);
    let hebrew_min = hebrew.iter().map(|g| g.x).fold(f32::INFINITY, f32::min);
    assert!(latin_max < hebrew_min, "Latin left of Hebrew: {g:?}");
    // Within the LTR embedding, Latin keeps left-to-right order.
    let a = db().glyph_for("Inter-400", 'a').unwrap();
    let c = db().glyph_for("Inter-400", 'c').unwrap();
    let xa = g.iter().find(|g| g.id == a).unwrap().x;
    let xc = g.iter().find(|g| g.id == c).unwrap().x;
    assert!(xa < xc);
    assert!(n.bounds.x.abs() < 1e-9, "start = left edge at the origin, in any direction: {:?}", n.bounds);
    let mut end = TextNode::new("שלום abc", Vec2::ZERO, TextStyle { align: Align::End, ..style(12.0) });
    layout(db(), &mut end);
    assert!((end.bounds.x + end.bounds.w).abs() < 1e-9, "end = right edge at the origin: {:?}", end.bounds);
}

#[test]
fn bidi_ltr_paragraph_keeps_latin_around_hebrew() {
    // LTR base: "abc" then Hebrew then "xyz" — the Hebrew sits between the two Latin words.
    let n = laid("abc שלום xyz", style(12.0));
    let g = all_glyphs(&n);
    let id = |ch| db().glyph_for("Inter-400", ch).unwrap();
    let x_of = |ch| g.iter().find(|g| g.id == id(ch)).unwrap().x;
    let hebrew: Vec<f32> = g.iter().filter(|g| g.id == 0).map(|g| g.x).collect();
    assert_eq!(hebrew.len(), 4);
    assert!(hebrew.iter().all(|&x| x > x_of('c') && x < x_of('x')));
    assert_eq!(n.bounds.x, 0.0);
}

#[test]
fn kerning_is_applied() {
    let st = style(40.0);
    let av = measure(db(), "AV", &st).w;
    let a = measure(db(), "A", &st).w;
    let v = measure(db(), "V", &st).w;
    assert!(av < a + v - 0.5, "AV {av} vs A+V {}", a + v);
}

#[test]
fn outline_of_h_is_closed_with_cap_height() {
    let size = 100.0;
    let gid = db().glyph_for("Inter-400", 'H').unwrap();
    let path = db().outline("Inter-400", gid, size).expect("outline");
    assert!(!path.is_empty());
    // Every subpath ends with Close.
    let mut open = false;
    for e in &path.els {
        match e {
            PathEl::Move { .. } => {
                assert!(!open, "previous subpath closed before the next Move");
                open = true;
            }
            PathEl::Close => open = false,
            _ => assert!(open, "drawing inside a subpath"),
        }
    }
    assert!(!open, "last subpath closed");
    let b = path.bounds();
    let cap = db().metrics("Inter-400", size).unwrap().cap_height;
    assert!((b.h - cap).abs() < 0.5, "height {} vs cap height {cap}", b.h);
    // y down, origin on the baseline: the H sits above the baseline (negative y) from y=0.
    assert!(b.y1().abs() < 0.5 && b.y < 0.0, "{b:?}");
    assert!(b.x >= 0.0 && b.w > 0.0, "{b:?}");
    // Scales linearly.
    let half = db().outline("Inter-400", gid, size / 2.0).unwrap().bounds();
    assert!((half.h * 2.0 - b.h).abs() < 1e-9);
}

#[test]
fn glyph_positions_line_up_with_outlines() {
    // Each glyph drawn at its position gives visually ordered, non-overlapping letters for "HI".
    let n = laid("HI", style(50.0));
    let g = &n.runs[0].glyphs;
    let h = db().outline("Inter-400", g[0].id, 50.0).unwrap().bounds();
    let i = db().outline("Inter-400", g[1].id, 50.0).unwrap().bounds();
    assert!(g[0].x as f64 + h.x1() <= g[1].x as f64 + i.x + 1e-6);
}

#[test]
fn measure_matches_layout_bounds() {
    for (text, st) in [
        ("Hello", style(12.0)),
        ("Wrapped text that goes on and on", TextStyle { max_width: Some(60.0), align: Align::Middle, ..style(10.0) }),
        ("Top\nBottom", TextStyle { baseline: Baseline::Bottom, align: Align::End, ..style(18.0) }),
    ] {
        let n = laid(text, st.clone());
        assert_eq!(measure(db(), text, &st), n.bounds);
    }
}

#[test]
fn empty_and_degenerate_inputs() {
    let n = laid("", style(12.0));
    assert!(n.runs.is_empty());
    assert_eq!(n.bounds, Rect::default());
    assert!(laid("x", style(0.0)).runs.is_empty());
    assert!(laid("x", style(f64::NAN)).runs.is_empty());
    let mut node = TextNode::new("abc", Vec2::ZERO, style(12.0));
    layout(&FontDb::empty(), &mut node);
    assert!(node.runs.is_empty());
    // Tabs and stray control characters render as spaces, not boxes.
    let t = laid("a\tb\u{7}c", style(12.0));
    assert!(all_glyphs(&t).iter().all(|g| g.id != 0));
}

#[test]
fn number_nodes_are_formatted_before_layout() {
    let mut n = TextNode::new("stale", Vec2::ZERO, style(12.0));
    n.number = Some(NumberText { value: 1234.5, format: Arc::from("$,.2f"), locale: Arc::from("sv") });
    layout(db(), &mut n);
    assert_eq!(n.text, "1\u{a0}234,50\u{a0}kr");
    assert_eq!(n.runs[0].glyphs.len(), n.text.chars().count());
}

#[test]
fn layout_is_deterministic() {
    let st = TextStyle { max_width: Some(120.0), align: Align::Middle, baseline: Baseline::Middle, ..style(13.0) };
    let text = "Déterminisme: the same input gives the same glyphs, abc שלום 123, every time.";
    let a = laid(text, st.clone());
    let b = laid(text, st.clone());
    assert_eq!(a.runs, b.runs);
    assert_eq!(a.bounds, b.bounds);
    // A fresh database (cold caches) gives bit-identical output too.
    let fresh = FontDb::with_bundled();
    let mut c = TextNode::new(text, Vec2::ZERO, st);
    layout(&fresh, &mut c);
    assert_eq!(a.runs, c.runs);
    assert_eq!(a.bounds.x.to_bits(), c.bounds.x.to_bits());
    assert_eq!(a.bounds.w.to_bits(), c.bounds.w.to_bits());
    let bits = |n: &TextNode| n.runs.iter().flat_map(|r| r.glyphs.iter().map(|g| (g.id, g.x.to_bits(), g.y.to_bits()))).collect::<Vec<_>>();
    assert_eq!(bits(&a), bits(&c));
}

#[test]
fn format_via_public_api() {
    assert_eq!(format::number(1234567.891, ",.2f", "en"), "1,234,567.89");
    assert_eq!(format::number(1234567.891, ",.2f", "sv"), "1\u{a0}234\u{a0}567,89");
    assert_eq!(format::number(1234567.891, ",.2f", "de"), "1.234.567,89");
    assert_eq!(format::number(0.256, ".0%", "en"), "26%");
    assert_eq!(format::number(1_234_567.0, ".2~s", "en"), "1.2M");
    assert_eq!(format::number(1_200.0, "~s", "en"), "1.2k");
    assert_eq!(format::number(1234.5, "$,.0f", "en"), "$1,235");
    assert_eq!(format::number(f64::NAN, ",.0f", "en"), "\u{2013}");
    let day = format::days_from_civil(2025, 7, 14);
    assert_eq!(format::date(day, "%b %Y", "en"), "Jul 2025");
    assert_eq!(format::date(day, "%-d %b", "sv"), "14 jul");
    assert_eq!(format::date(day, "%A", "de"), "Montag");
    assert_eq!(format::date(day, "Q%q %Y", "en"), "Q3 2025");
    assert_eq!(format::date(day, "%d.%m.%y", "nb"), "14.07.25");
}

/// Shaping 1,000 short labels (cold, then warm caches). Run with
/// `cargo test -p datars-text --release -- --ignored --nocapture` for timings.
#[test]
#[ignore]
fn perf_thousand_labels() {
    let labels: Vec<String> = (0..1000).map(|i| format::number(i as f64 * 137.31, ",.1f", "en") + " units").collect();
    let st = style(11.0);
    let db = FontDb::with_bundled();
    let run = || {
        let t = std::time::Instant::now();
        for l in &labels {
            let mut n = TextNode::new(l.clone(), Vec2::ZERO, st.clone());
            layout(&db, &mut n);
        }
        t.elapsed()
    };
    let cold = run();
    let warm = run();
    println!("1000 labels: cold caches {cold:?}, warm caches {warm:?}");
    assert!(cold.as_millis() < 1000);

    // Worst case for the piece cache: every word unique, wrapped into several lines.
    let unique: Vec<String> = (0..1000).map(|i| format!("Region{i} grew by {} percent in period{i}", i * 7)).collect();
    let wrap = TextStyle { max_width: Some(90.0), ..style(11.0) };
    let db = FontDb::with_bundled();
    let t = std::time::Instant::now();
    for l in &unique {
        let mut n = TextNode::new(l.clone(), Vec2::ZERO, wrap.clone());
        layout(&db, &mut n);
    }
    println!("1000 unique wrapped sentences: {:?}", t.elapsed());
}
