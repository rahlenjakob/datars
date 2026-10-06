//! Font lint: bundles embed their fonts, so a font whose vendor forbids embedding is flagged, and
//! a family with no source is reported (by the engine's diagnostics) instead of silently drawn in
//! the fallback face.

const NEWSREADER: &[u8] = include_bytes!("../../../assets/fonts/Newsreader-Regular.ttf");

/// The font with its OS/2 `fsType` set to `bits`.
fn with_fs_type(bytes: &[u8], bits: u16) -> Vec<u8> {
    let mut b = bytes.to_vec();
    let n = u16::from_be_bytes([b[4], b[5]]) as usize;
    for i in 0..n {
        let rec = 12 + i * 16;
        if &b[rec..rec + 4] == b"OS/2" {
            let off = u32::from_be_bytes([b[rec + 8], b[rec + 9], b[rec + 10], b[rec + 11]]) as usize;
            b[off + 8..off + 10].copy_from_slice(&bits.to_be_bytes());
        }
    }
    b
}

const DOC: &str = r#"{ "datars": 1, "size": { "width": 200, "height": 60 },
  "theme": { "tokens": { "font.title": { "family": "House", "weight": 400, "src": "house.ttf" }, "font.strong": { "family": "GT America", "weight": 600 } } },
  "scene": { "kind": "text", "key": "t", "at": [10, 30], "text": "Hej", "style": { "font": "$font.title" } } }"#;

#[test]
fn restricted_fonts_and_sourceless_families_are_reported() {
    let mut e = datars_headless::load(DOC).unwrap();
    e.provide("font:House-400", &with_fs_type(NEWSREADER, 0x0002)).unwrap();
    let findings = datars_devtools::lint(&mut e);
    let embed: Vec<_> = findings.iter().filter(|f| f.rule == "fonts/embedding").collect();
    assert_eq!(embed.len(), 1, "{findings:#?}");
    assert!(embed[0].message.contains("font 'House' 400") && embed[0].severity == "warning");
    assert!(findings.iter().any(|f| f.rule == "diagnostic" && f.message.contains("font 'GT America' (font.strong) has no source")), "{findings:#?}");

    // Installable (fsType 0): nothing to say about embedding.
    let mut e = datars_headless::load(DOC).unwrap();
    e.provide("font:House-400", NEWSREADER).unwrap();
    assert!(!datars_devtools::lint(&mut e).iter().any(|f| f.rule == "fonts/embedding"));
}
