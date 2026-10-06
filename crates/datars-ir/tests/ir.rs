use datars_ir::*;

const BARS: &str = r##"{
  "datars": 1,
  "title": "Vote share",
  "size": { "width": 640, "height": 400 },
  "data": { "votes": { "values": { "party": ["S", "SD", "M"], "share": [30.3, 20.5, 19.1] }, "key": ["party"] } },
  "scene": {
    "kind": "group", "key": "chart",
    "scales": {
      "x": { "type": "band", "domain": { "data": "votes", "field": "party" }, "range": "width", "padding": 0.2 },
      "y": { "type": "linear", "domain": { "data": "votes", "field": "share" }, "range": "-height", "zero": true, "nice": true }
    },
    "children": [
      { "kind": "repeat", "from": "votes", "template": {
          "id": "bar", "kind": "shape",
          "geom": { "type": "rect", "x": "=scale.x(d.party)", "y": "=scale.y(d.share)", "w": "=scale.x.bandwidth()", "h": "=scale.y(0) - scale.y(d.share)" },
          "fill": "$accent",
          "semantics": { "role": "datum", "label": "=`${d.party}: ${d.share}%`" },
          "on": { "activate": { "toggle": "selected", "value": "=d.party" } }
      } }
    ]
  },
  "program": { "preset": "story", "states": [ { "name": "bars" }, { "name": "pie", "set": { "shape": "pie" } } ],
               "edges": [ { "from": "bars", "on": "next", "to": "pie" } ] }
}"##;

#[test]
fn parses_round_trips_and_classifies_props() {
    let d = Doc::from_json(BARS).unwrap();
    assert_eq!(d.data["votes"].key, vec!["party".to_string()]);
    let again = Doc::from_json(&d.to_json()).unwrap();
    assert_eq!(again, d);
    let TKind::Group { children } = &d.scene.kind else { panic!() };
    let TKind::Repeat { template, .. } = &children[0].kind else { panic!() };
    let TKind::Shape { geom: TGeom::Rect { x, .. }, fill, .. } = &template.kind else { panic!() };
    assert_eq!(x.as_expr(), Some("scale.x(d.party)"));
    assert_eq!(fill.as_str(), Some("$accent"));
    assert!(matches!(template.on["activate"], Action::Toggle { .. }));
}

#[test]
fn patches_by_path_and_by_id() {
    let d = Doc::from_json(BARS).unwrap();
    let p = apply_patch(&d, &[
        PatchOp::Set { path: "/title".into(), value: serde_json::json!("New title") },
        PatchOp::Set { path: "/scene/#bar/fill".into(), value: serde_json::json!("$negative") },
    ])
    .unwrap();
    assert_eq!(p.title, "New title");
    assert!(p.to_json().contains("$negative"));
    assert!(apply_patch(&d, &[PatchOp::Remove { path: "/scene/#nope/fill".into() }]).is_err());
    let bad = apply_patch(&d, &[PatchOp::Set { path: "/scene/kind".into(), value: serde_json::json!("nonsense") }]);
    assert!(bad.is_err(), "a patch that breaks the document is refused");
}

#[test]
fn tile_sources_and_layers_parse() {
    let d = Doc::from_json(
        r#"{"data": {"base": {"tiles": "tiles/region.pmtiles"}},
            "scene": {"kind": "tiles", "key": "basemap", "source": "base",
                      "layers": [{"layer": "roads", "filter": "=d.kind == 'major'", "template": {"kind": "shape", "geom": {"type": "feature"}}}]}}"#,
    )
    .unwrap();
    assert_eq!(d.data["base"].from, SourceKind::Tiles("tiles/region.pmtiles".into()));
    let TKind::Tiles(t) = &d.scene.kind else { panic!("{:?}", d.scene.kind) };
    assert_eq!(t.source, "base");
    let l = &t.layers[0];
    assert!(l.merge && !l.labels, "layers batch by default and aren't labels");
    assert!(matches!(&l.template.kind, TKind::Shape { geom: TGeom::Feature { source, id }, .. } if source.is_empty() && id.is_null()));
    assert_eq!(Doc::from_json(&d.to_json()).unwrap(), d);
}

#[test]
fn newer_formats_are_refused() {
    let s = BARS.replace("\"datars\": 1", "\"datars\": 99");
    assert!(Doc::from_json(&s).unwrap_err().contains("newer"));
}
