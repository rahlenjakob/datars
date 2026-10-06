//! `datars-devtools` — lint, descriptions and scene queries (docs/14-devtools-and-agents.md).
//! Lint is about data-graphics correctness, not syntax: identity, encoding, legibility,
//! accessibility, motion, performance.

pub mod profile;
pub mod specimen;
pub mod templates;

use datars_engine::Engine;
use datars_math::{Affine, Rect};
use datars_scene::{KeyPath, Node, NodeKind, Role, Scene};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Finding {
    pub rule: String,
    pub severity: String,
    pub message: String,
    pub fix: String,
}

fn f(rule: &str, severity: &str, message: String, fix: &str) -> Finding {
    Finding { rule: rule.into(), severity: severity.into(), message, fix: fix.into() }
}

/// Lint a loaded document: theme checks, then every state's scene.
pub fn lint(engine: &mut Engine) -> Vec<Finding> {
    let mut out = Vec::new();
    for t in datars_theme::validate(engine.theme()) {
        out.push(f("theme", if t.severity == datars_theme::Severity::Error { "error" } else { "warning" }, format!("{}: {}", t.check, t.message), "adjust the theme tokens or pick a theme that passes"));
    }
    for d in engine.diagnostics() {
        out.push(f("diagnostic", "error", d.message.clone(), "see the message"));
    }
    // Bundles embed their fonts: a vendor that forbids embedding says so in OS/2 fsType.
    for face in engine.fonts().faces() {
        if face.fs_type & datars_text::EMBED_RESTRICTED != 0 {
            out.push(f(
                "fonts/embedding",
                "warning",
                format!("font '{}' {}{} is marked restricted-licence embedding (OS/2 fsType {:#06x}); bundles embed the fonts they draw with", face.family, face.weight, if face.italic { " italic" } else { "" }, face.fs_type),
                "use a font whose licence allows embedding (OFL and Apache fonts do), or confirm your licence covers embedding in published charts",
            ));
        }
    }
    let doc = engine.doc().clone();
    for (name, src) in &doc.data {
        let geo = matches!(src.from, datars_ir::SourceKind::Atlas(_) | datars_ir::SourceKind::Geojson(_) | datars_ir::SourceKind::Topojson(_)) || src.id.is_some();
        // Fonts and tile archives have no rows to identify.
        let rowless = matches!(src.from, datars_ir::SourceKind::Font(_) | datars_ir::SourceKind::Tiles(_));
        if src.key.is_empty() && !geo && !rowless {
            out.push(f("identity/row-keys", "warning", format!("source `{name}` has no key: rows are identified by position, so transitions pair the wrong rows when data changes"), "declare a key column: data.values(…, { key: \"id\" })"));
        }
    }
    let names = engine.state_names();
    let tiles = doc.data.values().any(|s| matches!(s.from, datars_ir::SourceKind::Tiles(_)));
    let mut credited = false;
    for i in 0..names.len() {
        let scene = engine.scene_for_state(i);
        credited |= drawn_labels(&scene).iter().any(|l| l.text.contains("OpenStreetMap"));
        out.extend(lint_scene(&scene, &names[i]));
    }
    // Our tile archives are OpenStreetMap data: its licence (ODbL) requires the credit, visible
    // on the map (std/attribution draws it, linked to the copyright page).
    if tiles && !credited {
        out.push(f("maps/attribution", "warning", "the chart draws vector tiles (OpenStreetMap data) but never shows its credit".into(), "add attribution() from @datars/std beside the map: \"© OpenStreetMap contributors\", linked to openstreetmap.org/copyright"));
    }
    // Most elements entering/exiting between states suggests keys don't match. Ask the motion
    // planner itself — it pairs by path, then data elements by their own key — so the lint and
    // what readers see agree.
    // Only data elements count: axes and titles are supposed to cross-fade when a chart changes form.
    for i in 0..names.len().saturating_sub(1) {
        let (sa, sb, plan) = engine.plan_states(i, i + 1);
        let (da, db) = (data_paths(&sa), data_paths(&sb));
        let c = plan.correspondence();
        // Marks whose own key appears on both sides are the same data; if they don't pair, identity
        // is broken. Disjoint keys (a different dataset) are supposed to cross-fade.
        let (ia, ib) = (identities(&da), identities(&db));
        let kb: std::collections::BTreeSet<&String> = ib.values().collect();
        let common: Vec<&KeyPath> = da.iter().filter(|p| ia.get(*p).is_some_and(|k| kb.contains(k))).collect();
        let kept = common.iter().filter(|p| c.pairs.iter().any(|(a, _)| a == **p) || c.splits.iter().any(|(a, _)| a == **p)).count();
        let _ = keys;
        if common.len() >= 3 && kept * 2 < common.len() {
            out.push(f("identity/mismatch", "warning", format!("{} → {}: {} data marks exist in both states but only {kept} keep their identity; the rest cross-fade", names[i], names[i + 1], common.len()), "key each datum's mark by the row key in both states (each datum's shape directly under the repeat), or add a motion rule"));
        }
    }
    out.sort_by(|a, b| (a.severity != "error", &a.rule).cmp(&(b.severity != "error", &b.rule)));
    out.dedup();
    out
}

fn keys(scene: &Scene) -> Vec<String> {
    let mut v = Vec::new();
    scene.root.walk(&KeyPath::default(), &mut |p, n| {
        if matches!(n.kind, NodeKind::Shape { .. }) {
            v.push(p.last().map(|k| k.to_string()).unwrap_or_default());
        }
    });
    v
}

/// Scene-level rules.
/// A label as drawn: its text, canvas-px box, effective opacity, and the clip it's drawn under.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawnLabel {
    pub text: String,
    /// The box it covers on the canvas (a turned label: the box around its slant).
    pub rect: Rect,
    /// Its four corners as drawn, turned with it: two slanted labels side by side collide only if
    /// these do, however much their boxes overlap.
    pub quad: [datars_math::Vec2; 4],
    pub opacity: f64,
    pub clip: Option<Rect>,
}

/// Every text the scene draws, where it lands on the canvas: through transforms, pins, views and
/// cameras (screen-sized labels keep their size), with the opacity it's drawn at and the clip over it.
pub fn drawn_labels(scene: &Scene) -> Vec<DrawnLabel> {
    fn corners(xf: Affine, r: Rect) -> Rect {
        let pts = [(r.x, r.y), (r.x1(), r.y), (r.x, r.y1()), (r.x1(), r.y1())].map(|(x, y)| xf.apply(datars_math::Vec2::new(x, y)));
        pts.iter().fold(Rect::empty(), |acc, p| acc.include(*p))
    }
    fn narrow(clip: Option<Rect>, r: Rect) -> Option<Rect> {
        Some(match clip {
            Some(c) => c.intersect(&r).unwrap_or(Rect::new(r.x, r.y, 0.0, 0.0)),
            None => r,
        })
    }
    fn walk(n: &Node, parent: Affine, opacity: f64, clip: Option<Rect>, out: &mut Vec<DrawnLabel>) {
        let opacity = opacity * n.common.opacity;
        if !n.common.visible || opacity <= 0.01 {
            return;
        }
        let xf = n.common.placed(parent);
        let clip = match &n.common.clip {
            Some(datars_scene::Clip::Rect { rect }) => narrow(clip, corners(xf, *rect)),
            Some(datars_scene::Clip::Path { path }) => narrow(clip, corners(xf, path.bounds())),
            None => clip,
        };
        match &n.kind {
            NodeKind::Text(t) if !t.text.trim().is_empty() && t.bounds.w > 0.0 => {
                let (rect, quad) = if t.screen_size {
                    // Glyphs at screen size around the transformed origin, turned by `rotate`.
                    (datars_engine::text_rect(t, &xf), datars_engine::text_quad(t, &xf))
                } else {
                    let local = Rect::new(t.origin.x + t.bounds.x, t.origin.y + t.bounds.y, t.bounds.w, t.bounds.h);
                    let turn = if t.rotate != 0.0 { Affine::translate(t.origin.x, t.origin.y).mul(Affine::rotate(t.rotate)).mul(Affine::translate(-t.origin.x, -t.origin.y)) } else { Affine::IDENTITY };
                    let m = xf.mul(turn);
                    let quad = [(local.x, local.y), (local.x1(), local.y), (local.x1(), local.y1()), (local.x, local.y1())].map(|(x, y)| m.apply(datars_math::Vec2::new(x, y)));
                    (corners(m, local), quad)
                };
                out.push(DrawnLabel { text: t.text.clone(), rect, quad, opacity, clip });
            }
            NodeKind::View { viewport, camera, clip: clips, children } => {
                let inner = xf.mul(camera.map(|c| c.transform(*viewport)).unwrap_or(Affine::translate(viewport.x, viewport.y)));
                let clip = if *clips { narrow(clip, corners(xf, *viewport)) } else { clip };
                children.iter().for_each(|c| walk(c, inner, opacity, clip, out));
            }
            _ => n.children().iter().for_each(|c| walk(c, xf, opacity, clip, out)),
        }
    }
    let mut out = Vec::new();
    walk(&scene.root, Affine::IDENTITY, 1.0, None, &mut out);
    out
}

/// Labels a reader can't read: ones that cross the canvas edge (cut off), and pairs drawn on top
/// of each other. Only labels drawn at a readable opacity count; a label clipped on purpose (inside
/// a map view) is judged by the part that shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LabelIssues {
    pub cut: Vec<String>,
    pub overlapping: Vec<(String, String)>,
}

pub fn label_issues(scene: &Scene) -> LabelIssues {
    let canvas = Rect::new(0.0, 0.0, scene.width, scene.height);
    let drawn = drawn_labels(scene);
    let labels: Vec<(Rect, &DrawnLabel)> = drawn
        .iter()
        .filter(|l| l.opacity >= 0.35)
        .filter_map(|l| {
            let shown = match l.clip {
                Some(c) => c.intersect(&l.rect)?,
                None => l.rect,
            };
            (shown.w > 0.5 && shown.h > 0.5).then_some((shown, l))
        })
        .collect();
    let mut out = LabelIssues::default();
    for (r, l) in &labels {
        let inside = r.intersects(&canvas);
        let crosses = r.x < -1.5 || r.y < -1.5 || r.x1() > scene.width + 1.5 || r.y1() > scene.height + 1.5;
        if inside && crosses && l.clip.is_none() {
            out.cut.push(l.text.clone());
        }
    }
    // Text boxes are line boxes (ascent to descent): a sliver of overlap between stacked lines is
    // spacing, not a collision — a quarter of the smaller label is.
    let turned = |l: &DrawnLabel| (l.quad[0].y - l.quad[1].y).abs() > 1e-6;
    for i in 0..labels.len() {
        for j in i + 1..labels.len() {
            let (a, b) = (labels[i].0, labels[j].0);
            // A turned label: by its slanted outline (more than a px into the other's).
            if turned(labels[i].1) || turned(labels[j].1) {
                if a.intersects(&b) && datars_engine::quads_overlap(&labels[i].1.quad, &labels[j].1.quad, -1.0) {
                    out.overlapping.push((labels[i].1.text.clone(), labels[j].1.text.clone()));
                }
                continue;
            }
            if let Some(x) = a.intersect(&b) {
                let small = (a.w * a.h).min(b.w * b.h);
                if x.w * x.h > (0.25 * small).max(6.0) {
                    out.overlapping.push((labels[i].1.text.clone(), labels[j].1.text.clone()));
                }
            }
        }
    }
    out
}

pub fn lint_scene(scene: &Scene, state: &str) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut datum_without_sem = 0usize;
    let mut datum_total = 0usize;
    let mut categorical_keys = std::collections::BTreeSet::new();
    // `in_datum`: under a datum or any labelled node — a control's hit area takes the control's
    // label, as a screen reader announces it.
    fn walk(n: &Node, dws: &mut usize, dt: &mut usize, cats: &mut std::collections::BTreeSet<String>, in_datum: bool) {
        let is_datum = n.semantics.as_ref().is_some_and(|s| s.role == Role::Datum || !s.label.is_empty()) || in_datum;
        match &n.kind {
            NodeKind::Shape { fill: Some(datars_scene::Paint::Solid(datars_theme::Ink::Palette { name, index, .. })), .. } if name.as_ref() == "categorical" => {
                cats.insert(index.to_string());
                *dt += 1;
                if !is_datum && n.pickable {
                    *dws += 1;
                }
            }
            NodeKind::Shape { .. } if n.pickable => {
                *dt += 1;
                if n.semantics.is_none() && !in_datum {
                    *dws += 1;
                }
            }
            _ => {}
        }
        for c in n.children() {
            walk(c, dws, dt, cats, is_datum);
        }
    }
    walk(&scene.root, &mut datum_without_sem, &mut datum_total, &mut categorical_keys, false);
    let labels = label_issues(scene);
    if let Some((a, b)) = labels.overlapping.first() {
        let n = labels.overlapping.len();
        out.push(f("legibility/label-collisions", if n > 5 { "warning" } else { "info" }, format!("{state}: {n} overlapping labels (\"{a}\" overlaps \"{b}\")"), "give labels priorities, thin them, or use a legend"));
    }
    for t in &labels.cut {
        out.push(f("legibility/offscreen", "warning", format!("{state}: label \"{t}\" is cut off at the canvas edge"), "check positions, alignment near edges, and layout room"));
    }
    if datum_without_sem > 0 {
        out.push(f("a11y/semantics", "warning", format!("{state}: {datum_without_sem} interactive elements have no accessible label"), "add semantics: { role: \"datum\", label: … }"));
    }
    if categorical_keys.len() > 10 {
        out.push(f("encoding/too-many-colours", "warning", format!("{state}: {} categorical colours — hard to tell apart", categorical_keys.len()), "group small categories into Other (op.top) or label directly"));
    }
    let _ = datum_total;
    out
}

/// Describe every recipe of the standard library (for `datars describe` and generated docs).
pub fn describe_std(engine: &Engine) -> BTreeMap<String, serde_json::Value> {
    let mut out = BTreeMap::new();
    if let Ok(list) = engine.list_recipes("@datars/std") {
        for id in list {
            if let Ok(d) = engine.describe_recipe(&id) {
                out.insert(id, d);
            }
        }
    }
    out
}

/// Key paths of the data marks (role datum or region) in a scene.
fn data_paths(scene: &Scene) -> std::collections::BTreeSet<KeyPath> {
    fn go(n: &Node, path: &KeyPath, out: &mut std::collections::BTreeSet<KeyPath>) {
        let here = path.push(&n.key);
        if n.semantics.as_ref().is_some_and(|s| matches!(s.role, Role::Datum | Role::Region)) {
            out.insert(here.clone());
        }
        for c in n.children() {
            go(c, &here, out);
        }
    }
    let mut out = std::collections::BTreeSet::new();
    go(&scene.root, &KeyPath::default(), &mut out);
    out
}

/// The data identity of each mark: its own key when that's unique among the data marks, else its
/// parent's (a structural name like `"bar"` under every datum group is not an identity) — or,
/// when the parent is shared too (a day's candle and its volume bar, keyed alike under `bodies`
/// and `volume`), the two together.
fn identities(marks: &std::collections::BTreeSet<KeyPath>) -> BTreeMap<KeyPath, String> {
    let own = |p: &KeyPath| p.0.last().map(|k| k.to_string()).unwrap_or_default();
    let parent = |p: &KeyPath| p.0.iter().rev().nth(1).map(|k| k.to_string());
    let count = |ids: &mut dyn Iterator<Item = String>| {
        let mut n: BTreeMap<String, usize> = BTreeMap::new();
        for k in ids {
            *n.entry(k).or_default() += 1;
        }
        n
    };
    let n = count(&mut marks.iter().map(own));
    let first: Vec<(KeyPath, String)> = marks.iter().map(|p| (p.clone(), if n[&own(p)] == 1 { own(p) } else { parent(p).unwrap_or_else(|| own(p)) })).collect();
    let m = count(&mut first.iter().map(|(_, id)| id.clone()));
    first
        .into_iter()
        .map(|(p, id)| {
            let id = if n[&own(&p)] > 1 && m[&id] > 1 { format!("{id}/{}", own(&p)) } else { id };
            (p, id)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Turned labels are judged as drawn: a slanted name that ends at its tick inside the canvas
    /// isn't cut off, and slanted names a line apart don't collide (their boxes overlap, they don't).
    #[test]
    fn turned_labels_are_judged_by_their_slant() {
        let names = ["Social Democrats", "Sweden Democrats", "Moderates"];
        let kids: Vec<serde_json::Value> = names
            .iter()
            .enumerate()
            .map(|(i, n)| serde_json::json!({ "kind": "text", "key": n, "text": n, "at": [80.0 + 22.0 * i as f64, 20], "rotate": -45, "style": { "size": 11, "align": "end", "baseline": "middle" } }))
            .collect();
        let doc = serde_json::json!({ "datars": 1, "size": { "width": 200, "height": 120 }, "scene": { "kind": "group", "key": "root", "children": kids } });
        let mut e = Engine::new();
        e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
        let issues = label_issues(&e.scene());
        assert_eq!(issues, LabelIssues::default(), "{issues:?}");
        // Laid flat at the same points they'd run into each other and off the left edge.
        let flat = doc.to_string().replace("\"rotate\":-45", "\"rotate\":0");
        e.load(datars_ir::Doc::from_json(&flat).unwrap());
        let issues = label_issues(&e.scene());
        assert!(!issues.cut.is_empty() && !issues.overlapping.is_empty(), "{issues:?}");
    }

    #[test]
    fn only_row_sources_need_keys() {
        let doc = r#"{"datars":1,"size":{"width":100,"height":50},
            "data":{"t":{"values":{"v":[1,2]}},"font":{"font":"x.ttf"},"base":{"tiles":"x.pmtiles"}},
            "scene":{"kind":"group","key":"root","children":[]}}"#;
        let mut e = Engine::new();
        e.load(datars_ir::Doc::from_json(doc).unwrap());
        let keys: Vec<String> = lint(&mut e).into_iter().filter(|f| f.rule == "identity/row-keys").map(|f| f.message).collect();
        assert_eq!(keys.len(), 1, "{keys:?}");
        assert!(keys[0].contains("`t`"));
    }
}
