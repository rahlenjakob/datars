//! Font subsetting: a subset covers exactly the characters asked for, keeps glyph ids, shapes and
//! draws its text exactly like the whole font (so every tier renders the same pixels), and the
//! same inputs give the same bytes.

use datars_build::fonts::subset::{subset, NotSubset};
use datars_math::Vec2;
use datars_render::GlyphSource;
use datars_scene::text::TextStyle;
use datars_scene::TextNode;
use datars_text::FontDb;
use std::collections::BTreeSet;
use std::sync::Arc;

const NEWSREADER: &[u8] = include_bytes!("../../../assets/fonts/Newsreader-Regular.ttf");
const HEBREW: &[u8] = include_bytes!("../../../assets/fonts/NotoSansHebrew-Regular.ttf");

fn inter() -> &'static [u8] {
    datars_text::bundled_file("Inter-Regular.ttf").unwrap()
}

fn chars(s: &str) -> BTreeSet<char> {
    s.chars().collect()
}

fn db(bytes: &[u8]) -> FontDb {
    let mut db = FontDb::empty();
    db.add_face(bytes.to_vec(), "T", 400, false).unwrap();
    db
}

fn lay(db: &FontDb, text: &str) -> TextNode {
    let mut n = TextNode::new(text, Vec2::ZERO, TextStyle { family: Arc::from("T"), size: 23.0, ..TextStyle::default() });
    datars_text::layout(db, &mut n);
    n
}

/// Shaping with the subset gives the same glyphs, positions and bounds as the whole font, and
/// every glyph draws the same outline.
fn assert_same_text(full: &[u8], text: &str) {
    let sub = subset(full, 0, &chars(text)).unwrap();
    assert!(!sub.gpos_verbatim, "GPOS was rewritten");
    let (a, b) = (db(full), db(&sub.bytes));
    let (na, nb) = (lay(&a, text), lay(&b, text));
    assert_eq!(na.runs, nb.runs, "{text}");
    assert_eq!(na.bounds, nb.bounds, "{text}");
    assert!(na.runs.iter().flat_map(|r| r.glyphs.iter()).all(|g| g.id != 0), "{text}: every glyph real");
    for r in &na.runs {
        for g in r.glyphs.iter() {
            assert_eq!(a.outline(&r.font, g.id, 20.0), b.outline(&r.font, g.id, 20.0), "{text}: glyph {}", g.id);
        }
    }
}

#[test]
fn subsets_shape_and_draw_like_the_whole_font() {
    // Kerning pairs (AV, To, Te, y.), figures, punctuation, the U+2212 minus.
    assert_same_text(inter(), "AVATAR To Te y. Wave 1,234.5 \u{2212}0.25 (Växjö) — «ok»");
    // A serif with ligatures (fi, ffl), old-style details and accents.
    assert_same_text(NEWSREADER, "Office waffle flights — Café, naïve, Łódź 1 234,5 %");
    // Right-to-left with marks.
    assert_same_text(HEBREW, "תל אביב יפו רִאשׁוֹן לְצִיּוֹן");
}

#[test]
fn a_subset_covers_what_was_asked_and_nothing_else() {
    let want = chars("Hello 123");
    let sub = subset(inter(), 0, &want).unwrap();
    let small = db(&sub.bytes);
    let full = db(inter());
    for c in "Helo123 ".chars() {
        assert_eq!(small.glyph_for("T-400", c), full.glyph_for("T-400", c), "{c:?} keeps its glyph id");
    }
    for c in "AZxyz9".chars() {
        assert!(full.glyph_for("T-400", c).is_some() && small.glyph_for("T-400", c).is_none(), "{c:?} dropped");
    }
    assert_eq!(sub.chars, want.len());
    // The licence notices travel with the subset (name IDs 0, 13, 14 are kept).
    assert_eq!(datars_build::fonts::licence(&sub.bytes, 0), datars_build::fonts::licence(inter(), 0));
    assert!(sub.bytes.len() * 6 < inter().len(), "{} vs {}", sub.bytes.len(), inter().len());
    // Metrics are the font's.
    assert_eq!(small.metrics("T-400", 100.0), full.metrics("T-400", 100.0));
    // Characters the font doesn't have are simply not covered.
    let heb = subset(inter(), 0, &chars("Aא")).unwrap();
    assert_eq!(heb.chars, 1);
}

#[test]
fn subsetting_is_deterministic() {
    let want = chars("Newsreader 2026 — Fig. 3");
    let a = subset(NEWSREADER, 0, &want).unwrap().bytes;
    let b = subset(NEWSREADER, 0, &want).unwrap().bytes;
    assert_eq!(a, b);
    // Pinned: the same bytes on every machine and every run (a cross-platform check lives in CI
    // by running this test everywhere). Update only with a subsetter change.
    assert_eq!(datars_bundle::chunk_hash(&a), PINNED, "subset bytes changed");
}

const PINNED: &str = "b3:e8a633c3e5b7bdedcbe56ee9a36a90d742a611e32e4dc35184b1e5fd2d975484";

/// Embedding permissions: a restricted font is shipped with a warning, a no-subsetting font
/// whole; the licence comes from the font's own name table.
#[test]
fn packing_honours_embedding_bits_and_records_licences() {
    let face = |fs_type: u16| datars_build::fonts::Face {
        id: "Newsreader-400".into(),
        family: "Newsreader".into(),
        weight: 400,
        italic: false,
        fs_type,
        bytes: NEWSREADER.to_vec(),
        index: 0,
        from: "fonts/Newsreader-Regular.ttf".into(),
        source: None,
        chars: chars("Rivers 2026"),
    };
    let mut b = datars_bundle::Builder::new();
    let p = datars_build::fonts::pack(&[face(0x0002), face(0x0100)], false, &mut b);
    assert!(p.decisions.iter().any(|d| d.starts_with("warning:") && d.contains("restricted-licence embedding")), "{:#?}", p.decisions);
    assert!(p.decisions.iter().any(|d| d.contains("shipped whole") && d.contains("forbids subsetting")), "{:#?}", p.decisions);
    assert_eq!(p.eager.len(), 2);
    assert!(p.lazy.is_empty());
    assert_eq!(p.credits[0].licence, "OFL-1.1");
    assert!(p.credits[0].copyright.as_deref().unwrap_or("").contains("Newsreader Project Authors"));
    let lic = datars_build::fonts::licence(inter(), 0);
    assert_eq!((lic.id.as_str(), lic.url.as_deref()), ("OFL-1.1", Some("http://scripts.sil.org/OFL")));
}

/// The real network: Google Fonts' css2 API answers a non-browser client with a static TrueType
/// instance. Opt in with `cargo test -p datars-build --test fonts -- --ignored`.
#[test]
#[ignore]
fn downloads_a_static_instance_from_google_fonts() {
    use datars_build::fonts::acquire::{Curl, Fonts};
    let dir = std::env::temp_dir().join(format!("datars-google-{}", std::process::id()));
    let fonts = Fonts::new(Box::new(Curl), Some(dir.clone()));
    let bytes = fonts.fetch("google:Newsreader:600").expect("download");
    let f = read_fonts::FontRef::new(&bytes).unwrap();
    use read_fonts::TableProvider;
    assert!(f.table_data(read_fonts::types::Tag::new(b"glyf")).is_some() && f.table_data(read_fonts::types::Tag::new(b"fvar")).is_none(), "a static TrueType instance");
    assert_eq!(f.os2().unwrap().us_weight_class(), 600);
    assert_eq!(datars_build::fonts::licence(&bytes, 0).id, "OFL-1.1");
    assert!(dir.join("fonts/google/newsreader/600.ttf").is_file(), "cached");
    // A face the engine can draw with, and subset.
    let mut db = FontDb::empty();
    db.add_face(bytes.clone(), "Newsreader", 600, false).unwrap();
    assert!(subset(&bytes, 0, &chars("Hello")).is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cff_fonts_are_shipped_whole() {
    // A font with CFF outlines (tables copied from a TrueType font, `glyf` swapped for `CFF `).
    let f = read_fonts::FontRef::new(inter()).unwrap();
    let mut b = write_fonts::FontBuilder::new();
    for r in f.table_directory().table_records() {
        let tag = r.tag();
        if tag != read_fonts::types::Tag::new(b"glyf") && tag != read_fonts::types::Tag::new(b"loca") {
            b.add_raw(tag, f.table_data(tag).unwrap().as_bytes().to_vec());
        }
    }
    b.add_raw(read_fonts::types::Tag::new(b"CFF "), vec![1, 0, 4, 1]);
    assert_eq!(subset(&b.build(), 0, &chars("a")).unwrap_err(), NotSubset::Cff);
}
