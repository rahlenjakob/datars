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
    // Words that point at a mark only by its colour or its place (WCAG 1.3.3): the title, the
    // description, and each step's narration.
    let states = doc.program.as_ref().map(|p| p.states.clone()).unwrap_or_default();
    let mut said: Vec<(String, String)> = vec![("title".into(), doc.title.clone()), ("description".into(), doc.description.clone())];
    for st in &states {
        if let Some(n) = &st.narration {
            said.push((format!("step {}", st.name), format!("{}. {}", n.title, n.text)));
        }
    }
    for (where_, text) in &said {
        for phrase in sensory_phrases(text) {
            out.push(f("a11y/sensory", "info", format!("{where_}: \"{phrase}\" points at a mark by its colour or place only"), "name it too (\"the red line, Sweden,\"): a reader who can't see the colour, or hears the chart, can't tell which it is"));
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
    /// The same without the line spacing above and below the glyphs (what the engine's declutter
    /// keeps apart): turned names side by side a line apart touch by their line boxes, not their
    /// letters.
    pub core: [datars_math::Vec2; 4],
    pub opacity: f64,
    pub clip: Option<Rect>,
    /// Inside a view with a camera (a map): what its clip cuts is panned out of sight, on purpose.
    pub in_camera: bool,
    /// Drawn over a halo, or in an ink chosen against what's under it (`on(…)`): legible over data.
    pub guarded: bool,
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
    fn walk(n: &Node, parent: Affine, opacity: f64, clip: Option<Rect>, cam: bool, out: &mut Vec<DrawnLabel>) {
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
                let (rect, quad, core) = if t.screen_size {
                    // Glyphs at screen size around the transformed origin, turned by `rotate`.
                    (datars_engine::text_rect(t, &xf), datars_engine::text_quad(t, &xf), datars_engine::text_core(t, &xf))
                } else {
                    let local = Rect::new(t.origin.x + t.bounds.x, t.origin.y + t.bounds.y, t.bounds.w, t.bounds.h);
                    let turn = if t.rotate != 0.0 { Affine::translate(t.origin.x, t.origin.y).mul(Affine::rotate(t.rotate)).mul(Affine::translate(-t.origin.x, -t.origin.y)) } else { Affine::IDENTITY };
                    let m = xf.mul(turn);
                    let q = |r: Rect| [(r.x, r.y), (r.x1(), r.y), (r.x1(), r.y1()), (r.x, r.y1())].map(|(x, y)| m.apply(datars_math::Vec2::new(x, y)));
                    let lead = (local.h * 0.12).max(0.0);
                    (corners(m, local), q(local), q(Rect::new(local.x, local.y + lead, local.w, (local.h - 2.0 * lead).max(0.0))))
                };
                let guarded = t.halo.as_ref().is_some_and(|(_, w)| *w >= 1.5) || matches!(t.style.ink, datars_theme::Ink::On { .. });
                out.push(DrawnLabel { text: t.text.clone(), rect, quad, core, opacity, clip, in_camera: cam, guarded });
            }
            NodeKind::View { viewport, camera, clip: clips, children } => {
                let inner = xf.mul(camera.map(|c| c.transform(*viewport)).unwrap_or(Affine::translate(viewport.x, viewport.y)));
                let clip = if *clips { narrow(clip, corners(xf, *viewport)) } else { clip };
                let cam = cam || camera.is_some();
                children.iter().for_each(|c| walk(c, inner, opacity, clip, cam, out));
            }
            _ => n.children().iter().for_each(|c| walk(c, xf, opacity, clip, cam, out)),
        }
    }
    let mut out = Vec::new();
    walk(&scene.root, Affine::IDENTITY, 1.0, None, false, &mut out);
    out
}

/// Labels a reader can't read: ones that cross the canvas edge (cut off), and pairs drawn on top
/// of each other. Only labels drawn at a readable opacity count; a label clipped on purpose (inside
/// a map view) is judged by the part that shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LabelIssues {
    pub cut: Vec<String>,
    pub overlapping: Vec<(String, String)>,
    /// Labels a box clips part of (a column too narrow for its names): the reader sees a piece.
    /// Not counted under a map's camera, whose edge cuts labels as the reader pans.
    pub clipped: Vec<String>,
    /// Labels on the data: a line of a mark through their letters, or a filled mark under most of
    /// them — without a halo, or an ink chosen for that fill, to keep them legible.
    pub on_data: Vec<String>,
}

impl LabelIssues {
    /// Everything a reader can't read.
    pub fn count(&self) -> usize {
        self.cut.len() + self.overlapping.len() + self.clipped.len() + self.on_data.len()
    }
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
    let data = datars_engine::obstacles::Obstacles::collect(std::slice::from_ref(&scene.root));
    for (_, l) in labels.iter().filter(|(_, l)| !l.guarded && !l.in_camera) {
        if !data.is_empty() && data.cover_quad(&l.core).hurts() {
            out.on_data.push(l.text.clone());
        }
    }
    for (r, l) in &labels {
        let inside = r.intersects(&canvas);
        let crosses = r.x < -1.5 || r.y < -1.5 || r.x1() > scene.width + 1.5 || r.y1() > scene.height + 1.5;
        if inside && crosses && l.clip.is_none() {
            out.cut.push(l.text.clone());
        }
        // Clipped where its letters are (a descender, a halo past the edge is no loss; the end of
        // a word — "Competito" — is).
        if let (Some(c), false) = (l.clip, l.in_camera) {
            let letters = l.core.iter().fold(datars_math::Rect::empty(), |acc, p| acc.include(*p));
            let kept = c.intersect(&letters).unwrap_or(datars_math::Rect::new(letters.x, letters.y, 0.0, 0.0));
            if letters.w - kept.w > 2.0 || letters.h - kept.h > 2.0 {
                out.clipped.push(l.text.clone());
            }
        }
    }
    // Text boxes are line boxes (ascent to descent): a sliver of overlap between stacked lines is
    // spacing, not a collision — a quarter of the smaller label is.
    let turned = |l: &DrawnLabel| (l.quad[0].y - l.quad[1].y).abs() > 1e-6;
    for i in 0..labels.len() {
        for j in i + 1..labels.len() {
            let (a, b) = (labels[i].0, labels[j].0);
            // A turned label: by its slanted outline without the line spacing (more than a px into
            // the other's letters).
            if turned(labels[i].1) || turned(labels[j].1) {
                if a.intersects(&b) && datars_engine::quads_overlap(&labels[i].1.core, &labels[j].1.core, -1.0) {
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

/// One way a reader gets a chart: the box it lays out in and their text size.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Reading {
    pub width: f64,
    pub height: f64,
    pub text_scale: f64,
}

impl Reading {
    /// The readings a chart should survive: a phone, a tablet and a wide column (at the height it
    /// was written for, as a page's `height` keeps it), each at the theme's text size and at 1.5
    /// and 2 × — the large sizes readers choose on phones and in browsers.
    pub fn standard(height: f64) -> Vec<Reading> {
        let mut out = Vec::new();
        for width in [390.0, 768.0, 1200.0] {
            for text_scale in [1.0, 1.5, 2.0] {
                out.push(Reading { width, height, text_scale });
            }
        }
        out
    }

    pub fn name(&self) -> String {
        format!("{}×{} text {}×", self.width, self.height, self.text_scale)
    }
}

/// What a reading of one state shows that can't be read.
#[derive(Clone, Debug, PartialEq)]
pub struct LabelAudit {
    pub reading: Reading,
    pub state: String,
    pub issues: LabelIssues,
}

/// Every state of a chart at every reading: labels cut off, drawn over one another, clipped by
/// their box, or on the data — past the theme's text size at the height the web runtime grows the
/// chart to (`Engine::content_height`). The engine is left at the last reading.
pub fn audit_labels(engine: &mut Engine, readings: &[Reading]) -> Vec<LabelAudit> {
    let names = engine.state_names();
    let mut out = Vec::new();
    for r in readings {
        engine.resize(r.width, r.height, 1.0);
        engine.set_text_scale(r.text_scale);
        for (i, state) in names.iter().enumerate() {
            // Past the theme's text size the web runtime grows a chart whose text would run off
            // its bottom (`Engine::content_height`): read it as that reader gets it.
            let mut scene = engine.scene_for_state(i);
            if r.text_scale > 1.0 {
                engine.goto(i);
                if let Some(h) = engine.content_height() {
                    engine.resize(r.width, h, 1.0);
                    scene = engine.scene_for_state(i);
                    engine.resize(r.width, r.height, 1.0);
                }
            }
            let issues = label_issues(&scene);
            out.push(LabelAudit { reading: *r, state: state.clone(), issues });
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
    for t in &labels.on_data {
        out.push(f("legibility/on-data", "info", format!("{state}: label \"{t}\" sits on the data (a line through it, or a filled mark under it)"), "give it `alternatives` (other places to try), a halo, or an ink chosen for the fill (`on(…)`)"));
    }
    for t in &labels.clipped {
        out.push(f("legibility/clipped", "warning", format!("{state}: label \"{t}\" is partly hidden by its box's clip"), "give the box room for its labels, wrap them (maxWidth), or thin them"));
    }
    for t in &labels.cut {
        out.push(f("legibility/offscreen", "warning", format!("{state}: label \"{t}\" is cut off at the canvas edge"), "check positions, alignment near edges, and layout room"));
    }
    if datum_without_sem > 0 {
        out.push(f("a11y/semantics", "warning", format!("{state}: {datum_without_sem} interactive elements have no accessible label"), "add semantics: { role: \"datum\", label: … }"));
    }
    if let Some((n, unnamed)) = colour_only(scene) {
        out.push(f("encoding/colour-only", "warning", format!("{state}: {n} colours tell the data apart, and nothing else names them ({})", unnamed.join(", ")), "add a legend (plot({ legend: true })), label the marks (line({ labels: true }), bar({ labels: true })), or give each series a dash or symbol too (line({ dashes: true }))"));
    }
    if categorical_keys.len() > 10 {
        out.push(f("encoding/too-many-colours", "warning", format!("{state}: {} categorical colours — hard to tell apart", categorical_keys.len()), "group small categories into Other (op.top) or label directly"));
    }
    let _ = datum_total;
    out
}

/// Phrases that point at a mark by its colour ("the red line", "blå staplarna") or by where it is
/// ("on the left", "till höger"), in English and Swedish: what a reader who can't see the colour,
/// or hears the chart, can't follow — unless the mark is named too (WCAG 1.3.3, 1.4.1).
pub fn sensory_phrases(text: &str) -> Vec<String> {
    const COLOURS: &[&str] = &[
        "red", "green", "blue", "orange", "yellow", "purple", "pink", "grey", "gray", "teal", "brown", "violet", "cyan", "magenta",
        "röd", "röda", "rött", "grön", "gröna", "grönt", "blå", "blått", "gul", "gula", "gult", "lila", "rosa", "grå", "gråa",
    ];
    const MARKS: &[&str] = &[
        "line", "lines", "bar", "bars", "dot", "dots", "point", "points", "area", "areas", "slice", "slices", "segment", "segments", "series", "one", "ones", "curve", "curves", "circle", "circles", "column", "columns", "band", "bands",
        "linje", "linjen", "linjerna", "stapel", "stapeln", "staplar", "staplarna", "punkt", "punkten", "punkterna", "yta", "ytan", "kurva", "kurvan", "fältet", "fälten",
    ];
    const PLACES: &[&[&str]] = &[
        &["on", "the", "left"], &["on", "the", "right"], &["to", "the", "left"], &["to", "the", "right"], &["at", "the", "top"], &["at", "the", "bottom"],
        &["top", "left"], &["top", "right"], &["bottom", "left"], &["bottom", "right"], &["left-hand"], &["right-hand"],
        &["till", "vänster"], &["till", "höger"], &["överst"], &["nederst"],
    ];
    let words: Vec<String> = text.split(|c: char| !(c.is_alphanumeric() || c == '-')).filter(|w| !w.is_empty()).map(|w| w.to_lowercase()).collect();
    // A place points at a mark when a mark is what's there ("the bars on the left"), within a few
    // words — not a layout said in passing ("men to the left, women to the right").
    let near_mark = |i: usize, len: usize| (i.saturating_sub(4)..(i + len + 4).min(words.len())).any(|j| MARKS.contains(&words[j].as_str()));
    let mut out = Vec::new();
    for i in 0..words.len() {
        if COLOURS.contains(&words[i].as_str()) {
            if let Some(j) = (i + 1..(i + 3).min(words.len())).find(|&j| MARKS.contains(&words[j].as_str())) {
                out.push(words[i..=j].join(" "));
            }
        }
        for p in PLACES {
            if words.len() >= i + p.len() && words[i..i + p.len()].iter().zip(p.iter()).all(|(a, b)| a == b) && near_mark(i, p.len()) {
                out.push(p.join(" "));
            }
        }
    }
    out
}

/// Data marks told apart by colour alone (WCAG 1.4.1): two or more of the categorical palette's
/// colours on data marks, no legend, and fewer than all but one of the coloured marks' names
/// drawn anywhere as text (direct labels, an axis naming each bar). Returns how many colours, and
/// the names nothing draws.
pub fn colour_only(scene: &Scene) -> Option<(usize, Vec<String>)> {
    let mut colours: BTreeMap<u32, std::collections::BTreeSet<String>> = BTreeMap::new();
    let mut legend = false;
    fn name_of(label: &str) -> String {
        label.split(": ").next().unwrap_or(label).split(" · ").next().unwrap_or(label).trim().to_string()
    }
    fn walk(n: &Node, named: Option<&str>, data: bool, colours: &mut BTreeMap<u32, std::collections::BTreeSet<String>>, legend: &mut bool) {
        if !n.common.visible || n.common.opacity < 0.05 {
            return;
        }
        let sem = n.semantics.as_ref();
        if sem.is_some_and(|s| s.role == Role::Legend || s.role == Role::LegendItem) {
            *legend = true;
            return;
        }
        let is_data = data || sem.is_some_and(|s| matches!(s.role, Role::Datum | Role::Series | Role::Region));
        let named = sem.filter(|s| !s.label.is_empty() && matches!(s.role, Role::Datum | Role::Series | Role::Region)).map(|s| s.label.as_str()).or(named);
        let palette = |i: &datars_theme::Ink| match i {
            datars_theme::Ink::Palette { name, index, .. } if name.as_ref() == "categorical" => Some(*index),
            _ => None,
        };
        match &n.kind {
            NodeKind::Shape { fill, stroke, .. } if is_data => {
                let ink = fill.as_ref().and_then(|p| if let datars_scene::Paint::Solid(i) = p { palette(i) } else { None }).or_else(|| stroke.as_ref().and_then(|s| if let datars_scene::Paint::Solid(i) = &s.paint { palette(i) } else { None }));
                if let Some(ix) = ink {
                    colours.entry(ix).or_default().extend(named.map(name_of));
                }
            }
            NodeKind::Instances(i) if is_data => {
                for k in 0..i.len() {
                    if let Some(ix) = palette(&i.fill_at(k)) {
                        let label = i.labels.as_ref().and_then(|l| l.get(k)).map(|l| name_of(l)).or(named.map(name_of));
                        colours.entry(ix).or_default().extend(label);
                    }
                }
            }
            _ => {}
        }
        for c in n.children() {
            walk(c, named, is_data, colours, legend);
        }
    }
    walk(&scene.root, None, false, &mut colours, &mut legend);
    if colours.len() < 2 || legend {
        return None;
    }
    let texts: Vec<String> = drawn_labels(scene).into_iter().filter(|l| l.opacity > 0.05).map(|l| l.text.to_lowercase()).collect();
    let mut unnamed = Vec::new();
    for names in colours.values() {
        // A colour is named when one of its marks' names — or a part of one: "Institute / Ada",
        // "2021, Centre", "Ada – Bruno" — is drawn somewhere.
        let parts = |nm: &str| -> Vec<String> {
            let mut v = vec![nm.to_lowercase()];
            for sep in [" / ", " – ", " · ", ", "] {
                v.extend(nm.split(sep).map(|p| p.trim().to_lowercase()).filter(|p| !p.is_empty()));
            }
            v
        };
        if !names.iter().any(|nm| !nm.is_empty() && parts(nm).iter().any(|p| texts.iter().any(|t| t.contains(p.as_str())))) {
            unnamed.push(names.iter().next().cloned().unwrap_or_default());
        }
    }
    (unnamed.len() > 1).then(|| (colours.len(), unnamed))
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

    /// A label a box's clip cuts in half is unreadable; one a map's camera pans past the edge isn't
    /// counted (that's panning).
    #[test]
    fn a_label_its_box_clips_is_reported() {
        let doc = serde_json::json!({ "datars": 1, "size": { "width": 300, "height": 100 }, "scene": { "kind": "group", "key": "root", "children": [
            { "kind": "group", "key": "col", "size": { "w": 40 }, "clip": "box", "children": [
                { "kind": "text", "key": "t", "text": "A long category name", "at": [0, 20], "style": { "size": 11 } }
            ] }
        ] } });
        let mut e = Engine::new();
        e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
        let issues = label_issues(&e.scene());
        assert_eq!(issues.clipped, vec!["A long category name".to_string()], "{issues:?}");
        assert_eq!(issues.count(), 1);
    }

    /// The audit reads every state at every reading, and finds what only a bigger text size
    /// breaks: two labels a fixed distance apart run into each other at 2 ×.
    #[test]
    fn the_audit_finds_what_only_big_text_breaks() {
        let doc = serde_json::json!({ "datars": 1, "size": { "width": 400, "height": 100 }, "scene": { "kind": "group", "key": "root", "children": [
            { "kind": "text", "key": "a", "text": "Revenue", "at": [10, 20], "style": { "size": "$size.label" } },
            { "kind": "text", "key": "b", "text": "Costs", "at": [70, 20], "style": { "size": "$size.label" } }
        ] } });
        let mut e = Engine::new();
        e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
        let readings = [Reading { width: 400.0, height: 100.0, text_scale: 1.0 }, Reading { width: 400.0, height: 100.0, text_scale: 2.0 }];
        let audit = audit_labels(&mut e, &readings);
        assert_eq!(audit.len(), 2);
        assert_eq!(audit[0].issues.count(), 0, "{:?}", audit[0]);
        assert_eq!(audit[1].issues.overlapping, vec![("Revenue".to_string(), "Costs".to_string())], "{:?}", audit[1]);
        assert_eq!(Reading::standard(300.0).len(), 9);
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
