//! Point pyramids in the build (docs/12-delivery.md, "Big data stays interactive"): an `lod`
//! instances node over a data source — rows written in the document, read from a file, or
//! generated — ships as a point archive instead of the rows. The engine indexes the source exactly
//! as every frame would ([`datars_engine::Engine::point_archives`]); `publish` and `bundle` write
//! the archive beside the chart under a content-hashed name, and the shipped document reads it by
//! range ([`rewrite`]: the source becomes `{"tiles": url}`). A reader downloads the tiles in view,
//! never the table, and nothing is generated on the device — the archive draws exactly what the
//! rows draw.

use std::collections::BTreeMap;
use std::path::Path;

/// One source's archive.
#[derive(Clone, Debug)]
pub struct Archive {
    pub source: String,
    pub bytes: Vec<u8>,
}

/// The point archives of a document's `lod` nodes over sources (its data files read from `dir`).
/// Sources that can't ship as one archive are reported in the second list, with why.
pub fn archives(doc_json: &str, dir: Option<&Path>) -> Result<(Vec<Archive>, Vec<String>), String> {
    let doc = datars_ir::Doc::from_json(doc_json)?;
    if !has_lod(&serde_json::to_value(&doc.scene).unwrap_or_default()) && doc.packages.is_empty() && !uses_recipes(&doc) {
        return Ok((Vec::new(), Vec::new()));
    }
    let mut e = datars_headless::load_at(doc_json, dir)?;
    let mut out = Vec::new();
    let mut skipped = Vec::new();
    for (source, bytes) in e.point_archives() {
        match bytes {
            Ok(bytes) => out.push(Archive { source, bytes }),
            Err(why) => skipped.push(format!("points `{source}`: not shipped as an archive ({why}); the rows ship instead")),
        }
    }
    Ok((out, skipped))
}

/// Does a template (JSON) hold an `instances` node with `lod`?
fn has_lod(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Object(o) => (o.get("kind").and_then(|k| k.as_str()) == Some("instances") && o.contains_key("lod")) || o.values().any(has_lod),
        serde_json::Value::Array(a) => a.iter().any(has_lod),
        _ => false,
    }
}

/// Does the scene expand recipes (which may emit `lod` nodes)?
fn uses_recipes(doc: &datars_ir::Doc) -> bool {
    serde_json::to_string(&doc.scene).unwrap_or_default().contains("\"kind\":\"use\"")
}

/// The document with each source in `urls` read from its shipped archive: the source becomes
/// `{"tiles": url}` (its rows, file or generator are no longer needed).
pub fn rewrite(doc_json: &str, urls: &BTreeMap<String, String>) -> Result<String, String> {
    let mut v: serde_json::Value = serde_json::from_str(doc_json).map_err(|e| e.to_string())?;
    for (source, url) in urls {
        if let Some(s) = v.get_mut("data").and_then(|d| d.get_mut(source)) {
            *s = serde_json::json!({ "tiles": url });
        }
    }
    serde_json::to_string(&v).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = r##"{"datars": 1, "size": {"width": 300, "height": 300},
        "data": {"pts": {"generate": {"rows": 40000, "columns": [
            {"as": "x", "expr": "randn(d.i, 1) * 10"}, {"as": "y", "expr": "randn(d.i, 2) * 10"}, {"as": "v", "expr": "floor(rand(d.i, 3) * 100)"}]}},
          "other": {"values": {"a": [1, 2]}}},
        "scene": {"kind": "view", "key": "sky", "camera": {"fit": {"bbox": [-40, -40, 40, 40]}, "explore": "cam", "max_zoom": 500},
          "children": [{"kind": "instances", "key": "dots", "from": "pts", "x": "=d.x", "y": "=d.y", "r": 1, "screen_size": true,
            "fill": "=d.v > 50 ? \"#ff0000\" : \"#0000ff\"", "label": "=`v ${d.v}`", "lod": {"budget": 512}}]},
        "program": {"states": [{"name": "all"}, {"name": "near", "set": {"cam.zoom": 30}}]}}"##;

    #[test]
    fn an_lod_source_ships_as_an_archive_that_draws_what_the_rows_draw() {
        let (archives, skipped) = archives(DOC, None).unwrap();
        assert!(skipped.is_empty(), "{skipped:?}");
        assert_eq!(archives.len(), 1);
        assert_eq!(archives[0].source, "pts");
        let shipped = rewrite(DOC, &BTreeMap::from([("pts".to_string(), "pts.pmtiles".to_string())])).unwrap();
        let doc = datars_ir::Doc::from_json(&shipped).unwrap();
        assert_eq!(doc.data["pts"].from, datars_ir::SourceKind::Tiles("pts.pmtiles".into()));
        assert!(matches!(doc.data["other"].from, datars_ir::SourceKind::Values(_)));
        // Every state of the shipped document, read from the archive, is the source's.
        let mut rows = datars_headless::load_at(DOC, None).unwrap();
        let mut from_archive = datars_headless::load_at(&shipped, None).unwrap();
        from_archive.provide("pts", &archives[0].bytes).unwrap();
        for i in 0..2 {
            assert_eq!(rows.scene_for_state(i).hash(), from_archive.scene_for_state(i).hash(), "state {i}");
        }
        // The shipped document is small: no generator, no rows.
        assert!(!shipped.contains("generate"));
    }

    #[test]
    fn documents_without_lod_have_none() {
        let doc = r#"{"datars": 1, "data": {"t": {"values": {"a": [1]}}}, "scene": {"kind": "group"}}"#;
        assert!(archives(doc, None).unwrap().0.is_empty());
    }
}
