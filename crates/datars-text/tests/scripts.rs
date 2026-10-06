//! Other scripts through fallback fonts: a document brings a font, every label can use it.

use datars_math::Vec2;
use datars_scene::text::TextStyle;
use datars_scene::TextNode;
use datars_text::{layout, FontDb};

const HEBREW: &[u8] = include_bytes!("../../../assets/fonts/NotoSansHebrew-Regular.ttf");

fn lay(db: &FontDb, text: &str) -> TextNode {
    let mut n = TextNode::new(text, Vec2::ZERO, TextStyle { size: 16.0, ..TextStyle::default() });
    layout(db, &mut n);
    n
}

#[test]
fn a_fallback_font_supplies_the_glyphs_inter_lacks() {
    let label = "Tel Aviv — תל אביב";
    let bundled = FontDb::with_bundled();
    let before = lay(&bundled, label);
    let missing = before.runs.iter().flat_map(|r| r.glyphs.iter()).filter(|g| g.id == 0).count();
    assert_eq!(missing, 6, "Inter has no Hebrew: its six letters are .notdef");
    // The database remembers which characters no face had (a runtime fetches subsets for them);
    // spaces and marks don't count.
    assert_eq!(bundled.missing_chars(), vec!['א', 'ב', 'ל', 'ת', 'י'].into_iter().collect::<std::collections::BTreeSet<_>>().into_iter().collect::<Vec<_>>());

    let mut db = FontDb::with_bundled();
    db.add_font(HEBREW.to_vec()).unwrap();
    assert_eq!(FontDb::families_in(HEBREW), vec!["Noto Sans Hebrew".to_string()]);
    assert!(db.add_fallback("Noto Sans Hebrew"));
    assert!(!db.add_fallback("No Such Family"));
    let after = lay(&db, label);
    assert!(after.runs.iter().flat_map(|r| r.glyphs.iter()).all(|g| g.id != 0), "every glyph real");
    let fonts: Vec<&str> = after.runs.iter().map(|r| &*r.font).collect();
    assert!(fonts.contains(&"Inter-400") && fonts.iter().any(|f| f.starts_with("Noto Sans Hebrew")), "Latin from Inter, Hebrew from Noto: {fonts:?}");
    // The Hebrew run reads right to left: its glyphs are laid out in visual order (x increasing),
    // so the logical first letter (tav) sits rightmost.
    let heb = after.runs.iter().find(|r| r.font.starts_with("Noto Sans Hebrew")).unwrap();
    let tav = db.glyph_for(&heb.font, 'ת').unwrap();
    let rightmost = heb.glyphs.iter().max_by(|a, b| a.x.total_cmp(&b.x)).unwrap();
    assert_eq!(rightmost.id, tav, "{:?}", heb.glyphs);
}
