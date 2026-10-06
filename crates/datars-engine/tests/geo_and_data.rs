//! Engine behaviour that converted map documents lean on: lon/lat camera boxes, point-feature
//! centres, and diagnostics that describe the data as it is now (not as it was before a request was
//! fulfilled).

use datars_engine::Engine;
use datars_scene::{Node, NodeKind};

fn engine(json: &str) -> Engine {
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(json).expect("doc parses"));
    e
}

fn find<'a>(n: &'a Node, pred: &dyn Fn(&Node) -> bool) -> Option<&'a Node> {
    if pred(n) {
        return Some(n);
    }
    n.children().iter().find_map(|c| find(c, pred))
}

const SQUARES: &str = r#"{"type":"FeatureCollection","features":[
  {"type":"Feature","id":"A","properties":{},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1],[0,0]]]}},
  {"type":"Feature","id":"B","properties":{},"geometry":{"type":"Polygon","coordinates":[[[10,10],[11,10],[11,11],[10,11],[10,10]]]}}]}"#;

#[test]
fn a_view_with_a_geo_coord_frames_a_lon_lat_box() {
    let doc = format!(
        r#"{{"datars":1,"size":{{"width":220,"height":220}},"data":{{"g":{{"geojson":{SQUARES}}}}},
        "scene":{{"kind":"view","key":"v","coord":{{"type":"geo","projection":"equirectangular","fit":{{"source":"g"}},"padding":0}},
          "camera":{{"fit":{{"bbox":["=geo.x(0, 1)","=geo.y(0, 1)","=geo.x(1, 0)","=geo.y(1, 0)"]}},"padding":0}},
          "children":[{{"kind":"group","key":"map","coord":{{"type":"geo","projection":"equirectangular","fit":{{"source":"g"}},"padding":0}},
            "children":[{{"kind":"repeat","from":"g","template":{{"kind":"shape","geom":{{"type":"feature","source":"g","id":"=d.id"}},"fill":"$accent"}}}}]}}]}}}}"#
    );
    let mut e = engine(&doc);
    assert!(e.diagnostics().is_empty(), "{:?}", e.diagnostics());
    let scene = e.scene();
    let NodeKind::View { camera, .. } = &scene.root.kind else { panic!("root is a view") };
    let cam = camera.expect("the lon/lat box resolves to a camera");
    // The source spans 11° across 220 px (20 px/°); a 1° box fills the view: zoom ≈ 11.
    assert!((cam.zoom - 11.0).abs() < 0.2, "zoom {}", cam.zoom);
    // Centred on square A (the box's centre, 0.5°, 0.5°): 10 px from the left, 210 px down.
    assert!((cam.x - 10.0).abs() < 0.5 && (cam.y - 210.0).abs() < 0.5, "centre ({}, {})", cam.x, cam.y);
}

#[test]
fn a_point_feature_centres_on_itself() {
    let pts = r#"{"type":"FeatureCollection","features":[
      {"type":"Feature","id":"p","properties":{},"geometry":{"type":"Point","coordinates":[2,3]}},
      {"type":"Feature","id":"q","properties":{},"geometry":{"type":"Point","coordinates":[8,9]}}]}"#;
    let doc = format!(
        r#"{{"datars":1,"size":{{"width":200,"height":200}},"data":{{"pts":{{"geojson":{pts}}}}},
        "scene":{{"kind":"group","key":"map","coord":{{"type":"geo","projection":"equirectangular","fit":{{"source":"pts"}},"padding":0}},
          "children":[{{"kind":"instances","key":"dots","from":"pts","instance_key":"=d.id","x":"=geo.cx(\"pts\", d.id)","y":"=geo.cy(\"pts\", d.id)","r":3}}]}}}}"#
    );
    let mut e = engine(&doc);
    let scene = e.scene();
    let dots = find(&scene.root, &|n| matches!(n.kind, NodeKind::Instances(_))).expect("instances");
    let NodeKind::Instances(i) = &dots.kind else { unreachable!() };
    assert_eq!(i.len(), 2);
    // The two points span the fitted box corner to corner.
    assert!(i.x[0].abs() < 1e-6 && (i.y[0] - 200.0).abs() < 1e-6, "p at ({}, {})", i.x[0], i.y[0]);
    assert!((i.x[1] - 200.0).abs() < 1e-6 && i.y[1].abs() < 1e-6, "q at ({}, {})", i.x[1], i.y[1]);
}

#[test]
fn a_scale_inside_a_group_repeat_takes_its_domain_from_the_group() {
    // Small multiples: each group's own scale spans that group's rows (`@group`).
    let doc = r#"{"datars":1,"size":{"width":100,"height":100},
      "data":{"t":{"values":{"g":["a","a","b","b"],"v":[0,10,0,50]}}},
      "scene":{"kind":"group","key":"root","children":[
        {"kind":"repeat","from":{"groups":"t","by":"g"},"template":{"kind":"group",
          "scales":{"s":{"type":"linear","domain":{"data":"@group","field":"v"},"range":[0,100]}},
          "children":[{"kind":"repeat","from":"@group","template":{"kind":"shape","geom":{"type":"rect","x":"=scale.s(d.v)","y":0,"w":1,"h":1},"fill":"$accent"}}]}}]}}"#;
    let mut e = engine(doc);
    let scene = e.scene();
    assert!(e.diagnostics().is_empty(), "{:?}", e.diagnostics());
    let xs: Vec<f64> = scene.root.children().iter().flat_map(|g| g.children().iter()).map(|n| match &n.kind {
        NodeKind::Shape { geom: datars_scene::Geom::Rect { x, .. }, .. } => *x,
        _ => f64::NAN,
    }).collect();
    assert_eq!(xs, vec![0.0, 100.0, 0.0, 100.0], "each group's maximum maps to the end of its range");
}

#[test]
fn the_spread_op_pushes_positions_apart() {
    let doc = r#"{"datars":1,"size":{"width":100,"height":100},
      "data":{"t":{"values":{"k":["a","b","c"],"v":[10,10,50]}}},
      "tables":{"s":{"from":"t","ops":[{"op":"spread","position":"=d.v","gap":4,"as":"y"}]}},
      "scene":{"kind":"group","key":"root","children":[
        {"kind":"repeat","from":"s","template":{"kind":"shape","geom":{"type":"rect","x":0,"y":"=d.y","w":1,"h":1},"fill":"$accent"}}]}}"#;
    let mut e = engine(doc);
    let scene = e.scene();
    assert!(e.diagnostics().is_empty(), "{:?}", e.diagnostics());
    let ys: Vec<f64> = scene.root.children().iter().map(|n| match &n.kind {
        NodeKind::Shape { geom: datars_scene::Geom::Rect { y, .. }, .. } => *y,
        _ => f64::NAN,
    }).collect();
    assert_eq!(ys, vec![8.0, 12.0, 50.0]);
}

/// `units` names its unit-number column `as` (it was always `__unit`, `as` ignored); the rows keep
/// their unit keys (`("a", Unit(1))`), so a split or a parliament after it works as without `as`.
#[test]
fn the_units_op_names_its_column_as_asked() {
    let doc = |ops: &str, x: &str| format!(r#"{{"datars":1,"size":{{"width":100,"height":100}},
      "data":{{"t":{{"values":{{"k":["a","b"],"v":[2,1]}},"key":["k"]}}}},
      "tables":{{"u":{{"from":"t","ops":[{ops}]}}}},
      "scene":{{"kind":"group","key":"root","children":[
        {{"kind":"repeat","from":"u","template":{{"kind":"shape","geom":{{"type":"rect","x":"{x}","y":0,"w":1,"h":1}},"fill":"$accent"}}}}]}}}}"#);
    let marks = |ops: &str, x: &str| {
        let mut e = engine(&doc(ops, x));
        let scene = e.scene();
        assert!(e.diagnostics().is_empty(), "{:?}", e.diagnostics());
        scene.root.children().iter().map(|n| match &n.kind {
            NodeKind::Shape { geom: datars_scene::Geom::Rect { x, .. }, .. } => (n.key.to_string(), *x),
            _ => (n.key.to_string(), f64::NAN),
        }).collect::<Vec<_>>()
    };
    let named = marks(r#"{"op":"units","value":"v","as":"seat"}"#, "=d.seat * 10");
    let keys = |m: &[(String, f64)]| m.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>();
    assert_eq!(named.iter().map(|m| m.1).collect::<Vec<_>>(), vec![0.0, 10.0, 0.0], "{named:?}");
    let default = marks(r#"{"op":"units","value":"v"}"#, "=d.__unit * 10");
    assert_eq!(named, default, "the same rows, keys and numbers as the default `__unit`");
    assert_eq!(keys(&named), vec!["(\"a\", Unit(0))", "(\"a\", Unit(1))", "(\"b\", Unit(0))"]);
}

#[test]
fn fulfilled_requests_clear_the_diagnostics_they_caused() {
    let doc = r#"{"datars":1,"size":{"width":200,"height":100},
      "data":{"a":{"url":"a.csv"},"b":{"url":"b.csv"}},
      "scene":{"kind":"group","key":"root","children":[
        {"kind":"repeat","from":"a","template":{"kind":"shape","geom":{"type":"rect","x":"=d.v","y":0,"w":5,"h":5},"fill":"$accent"}},
        {"kind":"repeat","from":"b","template":{"kind":"shape","geom":{"type":"rect","x":"=d.v","y":10,"w":5,"h":5},"fill":"$accent"}}]}}"#;
    let mut e = engine(doc);
    e.provide("a", b"v\n1\n2\n").unwrap();
    assert!(e.diagnostics().iter().any(|d| d.message.contains("`b`")), "b is still missing: {:?}", e.diagnostics());
    e.provide("b", b"v\n3\n").unwrap();
    assert!(e.diagnostics().is_empty(), "stale diagnostics: {:?}", e.diagnostics());
    assert_eq!(datars_engine::node_bounds(&e.scene().root).w > 0.0, true);
}

/// A derived table that joins in a table filtered by a signal isn't a pure function of its own
/// inputs: it follows the signal (a map's regions joined to the year's values recolour per state).
#[test]
fn a_join_on_a_signal_filtered_table_follows_the_signal() {
    let doc = serde_json::json!({ "datars": 1, "size": { "width": 200, "height": 100 },
        "data": {
            "places": { "values": { "id": ["a", "b"] }, "key": ["id"] },
            "vals": { "values": { "id": ["a", "a", "b", "b"], "year": [2025, 2036, 2025, 2036], "v": [1, 2, 3, 4] }, "key": ["id", "year"] } },
        "tables": {
            "thisyear": { "from": "vals", "ops": [{ "op": "filter", "expr": { "expr": "d.year == year" } }] },
            "joined": { "from": "places", "ops": [{ "op": "join", "with": "thisyear", "on": ["id"], "kind": "left" }] } },
        "signals": { "year": { "type": "num", "default": 2025 } },
        "program": { "states": [ { "name": "early", "set": { "year": 2025 } }, { "name": "late", "set": { "year": 2036 } } ] },
        "scene": { "kind": "repeat", "key": "r", "from": "joined", "template": { "kind": "text", "key": "=d.id", "at": [10, 30], "text": "=`${d.id}:${d.v}`" } } });
    let mut e = datars_engine::Engine::new();
    e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    let texts = |s: datars_scene::Scene| {
        let mut out = Vec::new();
        s.root.walk(&Default::default(), &mut |_, n| {
            if let datars_scene::NodeKind::Text(t) = &n.kind {
                out.push(t.text.clone());
            }
        });
        out
    };
    assert_eq!(texts(e.scene()), vec!["a:1", "b:3"]);
    e.goto(1);
    assert_eq!(texts(e.scene()), vec!["a:2", "b:4"], "the joined table follows the year");
}

/// A map that draws only regions with data (`backdrop: false`), over a table filtered to nothing,
/// draws nothing: the inner join with an empty table keeps no region (it used to fail on the
/// empty table's guessed key type and fall back to every region, as "no data").
#[test]
fn a_map_without_backdrop_over_an_empty_table_draws_no_region() {
    let doc = |filter: &str| {
        serde_json::json!({ "datars": 1, "size": { "width": 200, "height": 200 },
            "data": { "g": { "geojson": serde_json::from_str::<serde_json::Value>(SQUARES).unwrap() },
                      "vals": { "values": { "id": ["A", "x"], "v": [1, 2] }, "key": ["id"] } },
            "tables": { "some": { "from": "vals", "ops": [{ "op": "filter", "expr": { "expr": filter } }] } },
            "scene": { "kind": "use", "recipe": "@datars/std/map", "params": { "source": "g", "data": "some", "key": "id", "value": "v", "backdrop": false } } })
        .to_string()
    };
    let regions = |json: &str| {
        let mut e = engine(json);
        let mut n = Vec::new();
        e.scene().root.walk(&Default::default(), &mut |_, node| {
            if node.semantics.as_ref().is_some_and(|s| s.role == datars_scene::Role::Region) {
                n.push(node.key.to_string());
            }
        });
        assert!(e.diagnostics().is_empty(), "{:?}", e.diagnostics());
        n
    };
    assert_eq!(regions(&doc("d.id == 'A'")).len(), 1, "one region with data");
    assert_eq!(regions(&doc("d.id == 'x'")), Vec::<String>::new(), "a row that matches no region");
    assert_eq!(regions(&doc("false")), Vec::<String>::new(), "no rows at all");
}

/// A filter or derive with no expression is reported (a misspelled field used to keep no rows,
/// silently: an empty chart and a clean `check`).
#[test]
fn a_filter_without_an_expression_is_reported() {
    let doc = |op: &str| format!(r#"{{"datars":1,"size":{{"width":100,"height":100}},
      "data":{{"t":{{"values":{{"k":["a","b"]}},"key":["k"]}}}},
      "tables":{{"u":{{"from":"t","ops":[{op}]}}}},
      "scene":{{"kind":"repeat","from":"u","template":{{"kind":"shape","geom":{{"type":"rect","x":0,"y":0,"w":1,"h":1}},"fill":"$accent"}}}}}}"#);
    let diags = |op: &str| {
        let mut e = Engine::new();
        e.load(datars_ir::Doc::from_json(&doc(op)).unwrap());
        let _ = e.scene();
        e.diagnostics().iter().map(|d| d.message.clone()).collect::<Vec<_>>()
    };
    let d = diags(r#"{"op":"filter","where":"d.k == 'a'"}"#);
    assert!(d.iter().any(|m| m.contains("filter: needs `expr`") && m.contains("`where` isn't")), "{d:?}");
    assert!(diags(r#"{"op":"derive","as":"x"}"#).iter().any(|m| m.contains("derive: needs `expr`")));
    assert!(diags(r#"{"op":"filter","expr":"=d.k == 'a'"}"#).is_empty());
}
