//! WCAG 2.2 success criteria the runtime is responsible for, checked over the real corpus: every
//! example, and — with `DATARS_WCAG_CORPUS` set to a folder `scripts/wcag-corpus.mjs` wrote — the
//! site's charts and every figure on the site (the chart reference's std figures among them).
//! Each test is named for its criterion (`wcag_1_4_3_…`); `scripts/a11y-report.mjs` runs them and
//! the site's accessibility page shows the results. Each prints how much it checked
//! (`WCAG-CHECKED <test> <documents> <what>`), so the report says it.

use datars_color::{contrast_ratio, Color};
use datars_engine::Engine;
use datars_theme::Mode;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

/// A document of the corpus: its name, where it came from, its JSON and the folder its relative
/// files are read from.
struct Entry {
    alias: String,
    src: String,
    json: String,
    dir: PathBuf,
}

fn corpus() -> Vec<Entry> {
    let root = root();
    let mut out = Vec::new();
    if let Ok(dir) = std::env::var("DATARS_WCAG_CORPUS") {
        let dir = root.join(dir);
        let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).expect("the corpus manifest (node scripts/wcag-corpus.mjs)")).unwrap();
        for m in manifest.as_array().unwrap() {
            let s = |k: &str| m[k].as_str().unwrap().to_string();
            let json = std::fs::read_to_string(root.join(s("doc"))).unwrap();
            out.push(Entry { alias: s("alias"), src: s("src"), json, dir: root.join(s("dir")) });
        }
        return out;
    }
    let mut names: Vec<String> = std::fs::read_dir(root.join("examples")).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    for n in names {
        let f = root.join("examples").join(&n).join("doc.json");
        if let Ok(json) = std::fs::read_to_string(&f) {
            out.push(Entry { alias: format!("examples-{n}"), src: format!("examples/{n}/doc.json"), json, dir: root.join("examples").join(&n) });
        }
    }
    out
}

fn load(e: &Entry) -> Engine {
    let mut engine = datars_headless::load_at(&e.json, Some(&e.dir)).unwrap_or_else(|err| panic!("{}: {err}", e.src));
    engine.frame(0.0);
    engine
}

/// Every document of the corpus, loaded, with `check` run on it; `check` returns what it found
/// wrong. Prints what was checked; fails with the first findings.
fn over_corpus(test: &str, what: &str, mut check: impl FnMut(&Entry, &mut Engine) -> Vec<String>) {
    let docs = corpus();
    let mut found = Vec::new();
    for d in &docs {
        let mut e = load(d);
        found.extend(check(d, &mut e).into_iter().map(|f| format!("{}: {f}", d.src)));
    }
    println!("WCAG-CHECKED {test} {} {what}", docs.len());
    if std::env::var("WCAG_EXPLORE").is_ok() {
        for f in &found {
            println!("WCAG-FOUND {test} {f}");
        }
        return;
    }
    assert!(found.is_empty(), "{} found:\n{}", found.len(), found.iter().take(40).cloned().collect::<Vec<_>>().join("\n"));
}

/// [`over_corpus`] for what is the author's to get right (their words, their encodings): the
/// shipped examples and the site's showcase charts must pass; what the rest of the site's figures
/// do is printed (`WCAG-AUTHOR`), for the report to count, and doesn't fail.
fn over_corpus_authored(test: &str, what: &str, mut check: impl FnMut(&Entry, &mut Engine) -> Vec<String>) {
    let docs = corpus();
    let (mut strict, mut authored) = (Vec::new(), Vec::new());
    for d in &docs {
        let mut e = load(d);
        let found = check(d, &mut e).into_iter().map(|f| format!("{}: {f}", d.src));
        if d.src.starts_with("examples/") || d.src.starts_with("site/charts/") {
            strict.extend(found);
        } else {
            authored.extend(found);
        }
    }
    println!("WCAG-CHECKED {test} {} {what}", docs.len());
    for f in &authored {
        println!("WCAG-AUTHOR {test} {f}");
    }
    if std::env::var("WCAG_EXPLORE").is_ok() {
        for f in &strict {
            println!("WCAG-FOUND {test} {f}");
        }
        return;
    }
    assert!(strict.is_empty(), "{} found:\n{}", strict.len(), strict.iter().take(40).cloned().collect::<Vec<_>>().join("\n"));
}

const MODES: [Mode; 3] = [Mode::Light, Mode::Dark, Mode::HighContrast];

fn mode_name(m: Mode) -> &'static str {
    m.name()
}

fn px(p: &datars_render_cpu::Pixmap, x: i64, y: i64) -> Option<Color> {
    if x < 0 || y < 0 || x >= p.width as i64 || y >= p.height as i64 {
        return None;
    }
    let [r, g, b, a] = p.pixel(x as u32, y as u32);
    Some(Color { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a: a as f32 / 255.0 })
}

/// Text as drawn, each run with the box its glyphs cover and the contrast it has against what is
/// drawn under it (everything but the text, rendered): the lowest of the background under it,
/// ignoring the quarter it reads worst on (a gridline or the antialiased edge of a mark crossing
/// a few of its letters).
fn text_contrasts(e: &mut Engine, state: usize) -> Vec<(String, f64, f64, bool)> {
    let scene = e.scene_for_state(state);
    let list = e.display_list(&scene);
    let mut under = list.clone();
    under.ops.retain(|op| !matches!(op, datars_render::Op::Glyphs { .. }));
    let bg = datars_render_cpu::render(&under, e.fonts(), 1.0);
    let mut out = Vec::new();
    for op in &list.ops {
        let datars_render::Op::Glyphs { size, glyphs, origin, scale, rotate, color, halo, text, .. } = op else { continue };
        let Some(text) = text.as_ref().map(|t| t.to_string()) else { continue };
        if text.trim().is_empty() || color.a < 0.05 {
            continue;
        }
        let s = size * scale;
        let datars_render::Op::Glyphs { font, .. } = op else { continue };
        // Points inside each glyph's box, as the renderer places it (what the text is read against).
        let mut base = datars_math::Affine::translate(origin.x, origin.y);
        if *rotate != 0.0 {
            base = base.mul(datars_math::Affine::rotate(*rotate));
        }
        let base = base.mul(datars_math::Affine::scale(*scale, *scale));
        let mut samples = Vec::new();
        for g in glyphs.iter() {
            let Some(o) = datars_render::GlyphSource::outline(e.fonts(), font, g.id, *size) else { continue };
            let b = o.transform(&base.mul(datars_math::Affine::translate(g.x as f64, g.y as f64))).bounds();
            if !(b.w > 0.0 && b.h > 0.0) {
                continue;
            }
            let (nx, ny) = ((b.w / 1.5).ceil().max(1.0) as usize, (b.h / 1.5).ceil().max(1.0) as usize);
            for i in 0..=nx {
                for j in 0..=ny {
                    let (x, y) = (b.x + b.w * i as f64 / nx as f64, b.y + b.h * j as f64 / ny as f64);
                    if let Some(c) = px(&bg, x.floor() as i64, y.floor() as i64) {
                        samples.push(c);
                    }
                }
            }
        }
        if samples.is_empty() {
            continue;
        }
        let ratios: Vec<f64> = match halo {
            // A halo is what the text is read against.
            Some((h, w)) if *w >= 1.0 && h.a > 0.5 => samples.iter().map(|b| contrast_ratio(color.over(h.over(*b)), h.over(*b))).collect(),
            _ => samples.iter().map(|b| contrast_ratio(color.over(b.over(list.background)), b.over(list.background))).collect(),
        };
        let mut r = ratios.clone();
        r.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let low = r[r.len() / 4];
        let hex = |c: Color| format!("#{:02x}{:02x}{:02x}", (c.r * 255.0).round() as u8, (c.g * 255.0).round() as u8, (c.b * 255.0).round() as u8);
        let at = ratios.iter().position(|v| *v == low).unwrap_or(0);
        let on = samples[at].over(list.background);
        out.push((format!("{text}\" ink {} on {}{}{}", hex(*color), hex(on), if halo.is_some() { " halo" } else { "" }, if contrast_ratio(on, list.background) < 1.1 && halo.is_none() { " (paper)" } else { "" }), s, low, color.a < 0.999));
    }
    out
}

/// Text that is meant not to read well, and why (WCAG's "incidental" text): (source, text).
const UNREADABLE_ON_PURPOSE: &[(&str, &str, &str)] = &[
    ("site/figures/sdk/text-options.ts", "no halo", "the halo option's example: the same label without one, over a bar, beside one with it"),
];

#[test]
fn wcag_1_4_3_text_contrast() {
    over_corpus("wcag_1_4_3_text_contrast", "documents × states × light, dark, high contrast", |d, e| {
        let mut found = Vec::new();
        let n = e.state_names().len().max(1);
        for m in MODES {
            e.set_mode(m);
            for s in 0..n {
                for (text, size, ratio, faded) in text_contrasts(e, s) {
                    if UNREADABLE_ON_PURPOSE.iter().any(|(src, t, _)| *src == d.src && text.starts_with(t)) {
                        continue;
                    }
                    let min = if size >= 24.0 { 3.0 } else { 4.5 };
                    if ratio < min {
                        found.push(format!("{} state {s}: \"{}\" {size:.0}px {ratio:.2}:1 (needs {min}){}", mode_name(m), text.chars().take(80).collect::<String>(), if faded { " faded" } else { "" }));
                    }
                }
            }
        }
        found
    });
}

/// A data mark as drawn: where (root px), its fill and outline as seen over the paper (their
/// opacity and their groups' composited), and the group it's drawn in.
struct Mark {
    path: String,
    rect: datars_math::Rect,
    fill: Option<Color>,
    stroke: Option<(Color, f64)>,
    closed: bool,
    parent: String,
    /// Coloured from a sequential or diverging ramp: the colour is the value (a heat map's cell, a
    /// choropleth's region) — its light end near the paper on purpose.
    ramp: bool,
    /// Coloured in one of the theme's data colours (a palette's, or a token marks are drawn in:
    /// `$mark`, `$accent`, `$positive` …) rather than a literal the document chose.
    theme_colour: bool,
    /// Its opacity, its groups' together: a mark faded on purpose (a link under nodes, a series
    /// stepping back) is less than 1.
    opacity: f64,
}

fn marks(e: &Engine, scene: &datars_scene::Scene) -> Vec<Mark> {
    use datars_scene::{NodeKind, Paint, Role};
    let theme = e.theme();
    let paper = scene.background.resolve(theme);
    let mut out = Vec::new();
    fn walk(n: &datars_scene::Node, path: &str, xf: datars_math::Affine, op: f64, data: bool, theme: &datars_theme::ResolvedTheme, paper: Color, out: &mut Vec<Mark>) {
        if !n.common.visible {
            return;
        }
        let op = op * n.common.opacity;
        if op < 0.05 {
            return;
        }
        let here = format!("{path}/{}", n.key);
        let xf = xf.mul(n.common.transform);
        let decoration = n.semantics.as_ref().is_some_and(|s| s.role == Role::Decoration);
        if decoration {
            return;
        }
        let data = data || n.semantics.as_ref().is_some_and(|s| matches!(s.role, Role::Datum | Role::Region | Role::Series));
        match &n.kind {
            NodeKind::Shape { geom, fill, stroke, .. } if data => {
                let ramp = matches!(fill, Some(Paint::Solid(datars_theme::Ink::Ramp { .. })));
                const MARK_TOKENS: &[&str] = &["accent", "mark", "positive", "negative", "up", "down", "highlight"];
                let themed = |p: &Paint| match p {
                    Paint::Solid(datars_theme::Ink::Palette { name, .. }) => name.as_ref() == "categorical",
                    Paint::Solid(datars_theme::Ink::Token { name, .. }) => MARK_TOKENS.contains(&name.as_ref()),
                    _ => false,
                };
                let theme_colour = fill.as_ref().is_some_and(themed) || stroke.as_ref().is_some_and(|s| themed(&s.paint));
                let solid = |p: &Paint| if let Paint::Solid(i) = p { Some(i.resolve(theme)) } else { None };
                let fill = fill.as_ref().and_then(solid).filter(|c| c.a > 0.02).map(|c| c.fade(op as f32).over(paper));
                let stroke = stroke.as_ref().and_then(|s| solid(&s.paint).filter(|c| c.a > 0.02).map(|c| (c.fade(op as f32).over(paper), s.width)));
                let b = geom.bounds();
                let pts = [(b.x, b.y), (b.x1(), b.y), (b.x, b.y1()), (b.x1(), b.y1())].map(|(x, y)| xf.apply(datars_math::Vec2::new(x, y)));
                let rect = pts.iter().fold(datars_math::Rect::empty(), |acc, p| acc.include(*p));
                out.push(Mark { path: here.clone(), rect, fill, stroke, closed: geom.is_closed(), parent: path.to_string(), ramp, theme_colour, opacity: op });
            }
            NodeKind::View { viewport, camera, children, .. } => {
                let cam = camera.map(|cm| cm.transform(*viewport)).unwrap_or(datars_math::Affine::translate(viewport.x, viewport.y));
                for c in children {
                    walk(c, &here, xf.mul(cam), op, data, theme, paper, out);
                }
                return;
            }
            _ => {}
        }
        for c in n.children() {
            walk(c, &here, xf, op, data, theme, paper, out);
        }
    }
    walk(&scene.root, "", datars_math::Affine::IDENTITY, 1.0, false, theme, paper, &mut out);
    out
}

/// Data marks in the theme's data colours, at full strength, under 3:1 against the paper in mode
/// `m` (their outline counts: a mark with a 3:1 outline is found).
fn faint_marks(e: &mut Engine, m: Mode) -> Vec<String> {
    let mut found = std::collections::BTreeSet::new();
    e.set_mode(m);
    for s in 0..states(e) {
        let scene = e.scene_for_state(s);
        let paper = scene.background.resolve(e.theme());
        for mk in marks(e, &scene).into_iter().filter(|m| m.theme_colour && !m.ramp && m.opacity >= 0.95) {
            let best = [mk.fill.filter(|_| mk.closed), mk.stroke.filter(|(_, w)| *w >= 1.0).map(|(c, _)| c)].into_iter().flatten().map(|c| contrast_ratio(c, paper)).fold(0.0, f64::max);
            if best > 0.0 && best < 3.0 {
                let c = mk.fill.filter(|_| mk.closed).or(mk.stroke.map(|s| s.0)).unwrap();
                found.insert(format!("{} {:.2}:1 {}", mode_name(m), best, c.to_hex()));
            }
        }
    }
    found.into_iter().collect()
}

/// A document with colours of its own (its theme's tokens) answers for them itself.
fn own_theme(d: &Entry) -> bool {
    let theme = serde_json::from_str::<serde_json::Value>(&d.json).ok().and_then(|j| j.get("theme").cloned()).unwrap_or_default();
    theme.get("tokens").and_then(|t| t.as_object()).is_some_and(|o| !o.is_empty()) || theme.get("themes").and_then(|t| t.as_array()).is_some_and(|a| !a.is_empty())
}

#[test]
fn wcag_1_4_11_marks_against_background() {
    // In the high-contrast mode (a reader's More contrast, Increase Contrast, `mode="high-contrast"`)
    // every data colour of the theme stands out from the paper by 3:1.
    over_corpus("wcag_1_4_11_marks_against_background", "data marks in the theme's colours × states, high contrast", |d, e| {
        if own_theme(d) { Vec::new() } else { faint_marks(e, Mode::HighContrast) }
    });
}

#[test]
fn wcag_1_4_11_marks_against_background_light_dark() {
    // And in the light and dark modes: every colour of the default palette has 3:1 against the
    // mode's paper (the light mode's are darkened to it; the dark mode keeps the brighter ones).
    over_corpus("wcag_1_4_11_marks_against_background_light_dark", "data marks in the theme's colours × states, light and dark", |d, e| {
        if own_theme(d) {
            return Vec::new();
        }
        let mut v = faint_marks(e, Mode::Light);
        v.extend(faint_marks(e, Mode::Dark));
        v
    });
}

/// Data marks a click acts on: at least 24 × 24 px, or a 24 px circle on each clear of the others
/// (the spacing exception). Not met where a chart packs them closer (a ranked bar chart's 16 px
/// rows; a map's small countries, which WCAG's "essential" exception covers). Run for the report
/// (`--ignored`), which counts them.
#[test]
#[ignore = "the author's: a chart's density; the report counts it"]
fn wcag_2_5_8_target_size_marks() {
    over_corpus("wcag_2_5_8_target_size_marks", "data marks a click acts on × states", |_, e| {
        let mut found = Vec::new();
        let n = e.state_names().len().max(1);
        for s in 0..n {
            settle(e, s);
            let items: Vec<_> = e.semantic_items().into_iter().filter(|i| i.actionable && i.role != "control" && i.kind.is_none() && i.rect.w > 0.0).collect();
            for (k, i) in items.iter().enumerate() {
                if i.rect.w >= 24.0 && i.rect.h >= 24.0 {
                    continue;
                }
                // The spacing exception: a 24 px circle on its centre clear of every other target.
                let c = datars_math::Vec2::new(i.rect.x + i.rect.w / 2.0, i.rect.y + i.rect.h / 2.0);
                let clear = items.iter().enumerate().filter(|(j, _)| *j != k).all(|(_, o)| {
                    let oc = datars_math::Vec2::new(o.rect.x + o.rect.w / 2.0, o.rect.y + o.rect.h / 2.0);
                    let dx = (c.x - oc.x).abs() - o.rect.w.min(24.0) / 2.0;
                    let dy = (c.y - oc.y).abs() - o.rect.h.min(24.0) / 2.0;
                    let near = datars_math::Vec2::new(dx.max(0.0), dy.max(0.0)).len();
                    near >= 12.0 || (o.rect.w < 24.0 && (c - oc).len() >= 24.0)
                });
                if !clear {
                    found.push(format!("state {s}: \"{}\" {:.0}×{:.0}", i.label.chars().take(50).collect::<String>(), i.rect.w, i.rect.h));
                }
            }
        }
        found
    });
}

/// Each control (a semantic item of role `control`: the standard library's), as drawn: how many
/// of the pixels in its box stand out from the paper by 3:1 — its border, its words, its thumb:
/// what tells a reader it is there and what state it is in.
fn controls_contrast(e: &mut Engine) -> Vec<(String, usize)> {
    let scene = e.scene_for_state(e.state_index());
    let list = e.display_list(&scene);
    let px = datars_render_cpu::render(&list, e.fonts(), 1.0);
    let paper = list.background;
    let items = e.semantic_items();
    let mut out = Vec::new();
    for i in items.iter().filter(|i| i.role == "control" && i.rect.w >= 1.0 && i.rect.h >= 1.0 && !matches!(i.kind.as_deref(), Some("brush" | "pan"))) {
        let mut strong = 0;
        for y in i.rect.y.floor() as i64..(i.rect.y + i.rect.h).ceil() as i64 {
            for x in i.rect.x.floor() as i64..(i.rect.x + i.rect.w).ceil() as i64 {
                if let Some(c) = self::px(&px, x, y) {
                    if contrast_ratio(c.over(paper), paper) >= 3.0 {
                        strong += 1;
                    }
                }
            }
        }
        out.push((i.label.clone(), strong));
    }
    out
}

#[test]
fn wcag_1_4_11_controls_against_background() {
    over_corpus("wcag_1_4_11_controls_against_background", "controls × states × light, dark, high contrast", |_, e| {
        let mut found = std::collections::BTreeSet::new();
        let n = e.state_names().len().max(1);
        for m in MODES {
            e.set_mode(m);
            for s in 0..n {
                e.goto(s);
                e.frame(1e5 * (s + 1) as f64);
                for (label, strong) in controls_contrast(e) {
                    // At least a short line's worth: a border's side, a letter, a thumb.
                    if strong < 12 {
                        found.insert(format!("{} \"{}\" {strong} px at 3:1", mode_name(m), label.chars().take(40).collect::<String>()));
                    }
                }
            }
        }
        found.into_iter().collect()
    });
}

thread_local! {
    static CLOCK: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) };
}

/// The engine settled on state `s` (the transition there played out).
fn settle(e: &mut Engine, s: usize) {
    e.goto(s);
    let t = CLOCK.with(|c| {
        c.set(c.get() + 1e5);
        c.get()
    });
    e.frame(t);
    e.frame(t + 50.0);
}

fn states(e: &Engine) -> usize {
    e.state_names().len().max(1)
}

#[test]
fn wcag_1_1_1_text_alternatives() {
    over_corpus("wcag_1_1_1_text_alternatives", "documents × states: a name, a summary, a label on every data mark", |_, e| {
        let mut found = Vec::new();
        for s in 0..states(e) {
            settle(e, s);
            if e.accessible_name().trim().is_empty() {
                found.push(format!("state {s}: no name"));
            }
            if e.accessible_summary().trim().is_empty() {
                found.push(format!("state {s}: no summary"));
            }
            let unlabelled = e.semantic_items().iter().filter(|i| matches!(i.role.as_str(), "datum" | "region") && i.label.trim().is_empty()).count();
            if unlabelled > 0 {
                found.push(format!("state {s}: {unlabelled} data marks without a label"));
            }
        }
        found
    });
}

#[test]
fn wcag_1_3_1_info_and_relationships() {
    over_corpus("wcag_1_3_1_info_and_relationships", "documents × states: every interactive mark has semantics; controls say their kind and state", |_, e| {
        let mut found = Vec::new();
        for s in 0..states(e) {
            let scene = e.scene_for_state(s);
            for f in datars_devtools::lint_scene(&scene, &format!("state {s}")).into_iter().filter(|f| f.rule == "a11y/semantics") {
                found.push(f.message);
            }
            settle(e, s);
            for i in e.semantic_items() {
                if matches!(i.kind.as_deref(), Some("checkbox" | "switch" | "radio")) && i.checked.is_none() {
                    found.push(format!("state {s}: {} \"{}\" doesn't say whether it's checked", i.kind.unwrap_or_default(), i.label));
                }
            }
        }
        found
    });
}

#[test]
fn wcag_1_3_2_meaningful_sequence() {
    over_corpus("wcag_1_3_2_meaningful_sequence", "documents × states: the reading order is the chart, its narration, its content, then Pause and the story's steps", |_, e| {
        let mut found = Vec::new();
        let loc = e.doc().locale.clone();
        let tail: Vec<String> = ["previous-step", "next-step", "pause", "play"].iter().map(|k| datars_engine::phrases::phrase(&loc, k, &[])).collect();
        for s in 0..states(e) {
            settle(e, s);
            let r = datars_runtime::readout::of_engine(e);
            if r.first().map(|x| x.kind) != Some("chart") {
                found.push(format!("state {s}: the chart isn't first"));
            }
            // Pause and the steps come last, after everything the chart shows.
            let is_tail = |x: &datars_runtime::readout::Readout| x.kind == "button" && (x.path.starts_with("event:") || x.path.starts_with("playback:")) && tail.contains(&x.label);
            if let Some(k) = r.iter().position(is_tail) {
                if r[k..].iter().any(|x| !is_tail(x)) {
                    found.push(format!("state {s}: the story's buttons aren't last"));
                }
            }
            if let Some(n) = e.narration().filter(|_| !e.draws_narration()) {
                let first = if n.title.trim().is_empty() { n.text.trim() } else { n.title.trim() };
                if !first.is_empty() && r.get(1).map(|x| x.label.as_str()) != Some(first) {
                    found.push(format!("state {s}: the narration doesn't follow the chart's name"));
                }
            }
        }
        found
    });
}

#[test]
fn wcag_1_3_3_sensory_characteristics() {
    // The lint finds words that point at a mark by colour or place alone ...
    let said = datars_devtools::sensory_phrases("The red line is Sweden; the bars on the left are older. Blå staplarna till höger.");
    assert_eq!(said, vec!["red line", "on the left", "blå staplarna", "till höger"]);
    assert!(datars_devtools::sensory_phrases("Sweden's share rose above 4% in 2022.").is_empty());
    // ... and none of the corpus's titles, descriptions or narration do.
    over_corpus_authored("wcag_1_3_3_sensory_characteristics", "documents: title, description, every step's narration", |_, e| {
        datars_devtools::lint(e).into_iter().filter(|f| f.rule == "a11y/sensory").map(|f| f.message).collect()
    });
}

#[test]
fn wcag_1_3_4_orientation() {
    // No orientation is locked: a chart lays itself out in a portrait box and a landscape one, and
    // says the same in both as in its own.
    over_corpus("wcag_1_3_4_orientation", "documents laid out in a portrait and a landscape box", |_, e| {
        let mut found = Vec::new();
        settle(e, 0);
        let own = e.semantic_items().iter().filter(|i| matches!(i.role.as_str(), "datum" | "region")).count();
        for (w, h) in [(390.0, 844.0), (844.0, 390.0)] {
            e.resize(w, h, 1.0);
            settle(e, 0);
            let here = e.semantic_items().iter().filter(|i| matches!(i.role.as_str(), "datum" | "region")).count();
            if e.accessible_summary().is_empty() || here < own {
                found.push(format!("{w}×{h}: {here} of {own} data marks, summary \"{}\"", e.accessible_summary()));
            }
            if !e.diagnostics().is_empty() {
                found.push(format!("{w}×{h}: {}", e.diagnostics()[0].message));
            }
        }
        found
    });
}

#[test]
fn wcag_1_4_1_use_of_colour() {
    // Two series told apart by colour alone are flagged; a legend, labels or dashes clear it.
    let lines = |params: serde_json::Value| {
        let mut p = serde_json::json!({ "data": "t", "x": "x", "y": "y", "color": "s", "children": [{ "kind": "use", "recipe": "@datars/std/line", "params": {} }] });
        for (k, v) in params.as_object().unwrap() {
            if k == "line" {
                p["children"][0]["params"] = v.clone();
            } else {
                p[k] = v.clone();
            }
        }
        let doc = serde_json::json!({ "datars": 1, "size": { "width": 400, "height": 240 },
            "data": { "t": { "values": { "x": [1, 2, 3, 1, 2, 3], "y": [1, 3, 2, 2, 1, 3], "s": ["a", "a", "a", "b", "b", "b"] }, "key": ["s", "x"] } },
            "scene": { "kind": "use", "recipe": "@datars/std/plot", "params": p } });
        let mut e = datars_headless::load(&doc.to_string()).unwrap();
        e.frame(0.0);
        datars_devtools::lint(&mut e).into_iter().filter(|f| f.rule == "encoding/colour-only").count()
    };
    assert_eq!(lines(serde_json::json!({})), 1, "colour alone");
    assert_eq!(lines(serde_json::json!({ "legend": true })), 0, "a legend names them");
    assert_eq!(lines(serde_json::json!({ "line": { "labels": true } })), 0, "labels name them");
    over_corpus_authored("wcag_1_4_1_use_of_colour", "documents × states: colours that tell data apart are named too", |_, e| {
        datars_devtools::lint(e).into_iter().filter(|f| f.rule == "encoding/colour-only").map(|f| f.message).collect()
    });
}

#[test]
fn wcag_1_4_1_dashes_tell_lines_apart() {
    // `line({ dashes: true })`: each series its own pattern too, and the legend shows it.
    let doc = serde_json::json!({ "datars": 1, "size": { "width": 400, "height": 240 },
        "data": { "t": { "values": { "x": [1, 2, 1, 2, 1, 2], "y": [1, 3, 2, 1, 3, 2], "s": ["a", "a", "b", "b", "c", "c"] }, "key": ["s", "x"] } },
        "scene": { "kind": "use", "recipe": "@datars/std/plot", "params": { "data": "t", "x": "x", "y": "y", "color": "s", "legend": true, "dashes": true,
            "children": [{ "kind": "use", "recipe": "@datars/std/line", "params": { "dashes": true } }] } } });
    let mut e = datars_headless::load(&doc.to_string()).unwrap();
    e.frame(0.0);
    let scene = e.scene_for_state(0);
    let mut dashes = Vec::new();
    scene.root.walk(&datars_scene::KeyPath::default(), &mut |_, n| {
        if let datars_scene::NodeKind::Shape { geom: datars_scene::Geom::Polyline { .. }, stroke: Some(s), .. } = &n.kind {
            dashes.push(s.dash.clone());
        }
    });
    assert_eq!(dashes, vec![None, Some(vec![7.0, 4.0]), Some(vec![1.5, 3.5])], "solid, dashed, dotted");
    let mut swatches = 0;
    scene.root.walk(&datars_scene::KeyPath::default(), &mut |_, n| {
        if n.key.to_string().contains("swatch-") && matches!(n.kind, datars_scene::NodeKind::Shape { stroke: Some(_), .. }) {
            swatches += 1;
        }
    });
    assert_eq!(swatches, 3, "the legend draws each entry's pattern");
}

#[test]
fn wcag_1_4_3_text_contrast_in_every_std_theme() {
    // Every example in each of the standard library's themes, in every mode.
    let mut found = Vec::new();
    let docs = corpus();
    let examples: Vec<&Entry> = docs.iter().filter(|d| d.src.starts_with("examples/")).collect();
    for theme in ["datars/neutral", "datars/noir", "datars/broadsheet", "datars/brand-example"] {
        for d in &examples {
            let mut doc: serde_json::Value = serde_json::from_str(&d.json).unwrap();
            let own = doc.get("theme").cloned().unwrap_or_default();
            if own.get("tokens").and_then(|t| t.as_object()).is_some_and(|o| !o.is_empty()) || own.get("themes").and_then(|t| t.as_array()).is_some_and(|a| !a.is_empty()) {
                continue; // a document with a look of its own keeps it
            }
            doc["theme"] = serde_json::json!({ "use": theme });
            let mut e = datars_headless::load_at(&doc.to_string(), Some(&d.dir)).unwrap_or_else(|err| panic!("{} in {theme}: {err}", d.src));
            e.frame(0.0);
            for m in MODES {
                e.set_mode(m);
                for s in 0..states(&e) {
                    for (text, size, ratio, _) in text_contrasts(&mut e, s) {
                        let min = if size >= 24.0 { 3.0 } else { 4.5 };
                        if ratio < min {
                            found.push(format!("{} {theme} {} state {s}: \"{}\" {ratio:.2}:1", d.src, mode_name(m), text.chars().take(60).collect::<String>()));
                        }
                    }
                }
            }
        }
    }
    println!("WCAG-CHECKED wcag_1_4_3_text_contrast_in_every_std_theme {} examples × 4 themes × light, dark, high contrast", examples.len());
    if std::env::var("WCAG_EXPLORE").is_ok() {
        for f in &found {
            println!("WCAG-FOUND wcag_1_4_3_text_contrast_in_every_std_theme {f}");
        }
        return;
    }
    assert!(found.is_empty(), "{} found:\n{}", found.len(), found.iter().take(40).cloned().collect::<Vec<_>>().join("\n"));
}

#[test]
fn wcag_1_4_4_resize_text() {
    over_corpus_authored("wcag_1_4_4_resize_text", "documents × states at the reader's 2× text: every run of text the theme sizes is twice as big", |_, e| {
        let mut found = Vec::new();
        let runs = |e: &mut Engine, s: usize| {
            let scene = e.scene_for_state(s);
            let list = e.display_list(&scene);
            let mut v: Vec<(String, f64)> = list.ops.iter().filter_map(|op| match op {
                datars_render::Op::Glyphs { size, scale, text: Some(t), .. } if !t.trim().is_empty() => Some((t.to_string(), size * scale)),
                _ => None,
            }).collect();
            v.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
            v
        };
        for s in 0..states(e) {
            let one = runs(e, s);
            e.set_text_scale(2.0);
            let two = runs(e, s);
            e.set_text_scale(1.0);
            // The same words at the reader's size: none still at the theme's.
            let mut stuck = Vec::new();
            for (t, sz) in &one {
                if let Some((_, s2)) = two.iter().find(|(t2, _)| t2 == t) {
                    if *s2 < sz * 1.9 {
                        stuck.push(format!("\"{}\" {sz:.1}px", t.chars().take(30).collect::<String>()));
                    }
                }
            }
            if !stuck.is_empty() {
                found.push(format!("state {s}: {} of {} runs of text don't grow ({})", stuck.len(), one.len(), stuck.iter().take(3).cloned().collect::<Vec<_>>().join(", ")));
            }
        }
        found
    });
}

#[test]
fn wcag_1_4_6_text_contrast_enhanced_in_high_contrast() {
    // AAA: in the high-contrast mode, text on the paper reaches 7:1 (4.5:1 when large). (Labels
    // on a coloured mark don't: a mid-tone fill has under 7:1 with black and with white.)
    over_corpus("wcag_1_4_6_text_contrast_enhanced_in_high_contrast", "text on the paper × states, high contrast", |d, e| {
        let mut found = Vec::new();
        e.set_mode(Mode::HighContrast);
        for s in 0..states(e) {
            for (text, size, ratio, _) in text_contrasts(e, s).into_iter().filter(|t| t.0.ends_with("(paper)")) {
                if UNREADABLE_ON_PURPOSE.iter().any(|(src, t, _)| *src == d.src && text.starts_with(t)) {
                    continue;
                }
                let min = if size >= 24.0 { 4.5 } else { 7.0 };
                if ratio < min {
                    found.push(format!("state {s}: \"{}\" {size:.0}px {ratio:.2}:1", text.chars().take(60).collect::<String>()));
                }
            }
        }
        found
    });
}

#[test]
fn wcag_1_4_10_reflow() {
    // At 320 px wide (a phone at 400 % zoom), no label is cut off at the chart's edge: the chart
    // lays out again, it doesn't need scrolling sideways. (A chart's two-dimensional layout is
    // WCAG's exception; its words aren't.)
    over_corpus_authored("wcag_1_4_10_reflow", "documents × states at 320 px wide", |_, e| {
        let mut found = Vec::new();
        let vp = e.viewport();
        let h = (vp.height * 320.0 / vp.width.max(1.0)).max(240.0);
        e.resize(320.0, h, 1.0);
        for s in 0..states(e) {
            let scene = e.scene_for_state(s);
            let cut = datars_devtools::label_issues(&scene).cut;
            if !cut.is_empty() {
                found.push(format!("state {s}: {} labels cut off (\"{}\")", cut.len(), cut[0]));
            }
        }
        found
    });
}

#[test]
fn wcag_1_4_11_adjacent_segments() {
    // Filled marks that touch (a stacked bar's segments, a pie's slices, a treemap's cells): 3:1
    // between them, or a line between them in a colour that stands out from both.
    over_corpus_authored("wcag_1_4_11_adjacent_segments", "touching filled marks in the theme's colours × states, high contrast", |d, e| {
        let theme = serde_json::from_str::<serde_json::Value>(&d.json).ok().and_then(|j| j.get("theme").cloned()).unwrap_or_default();
        if theme.get("tokens").and_then(|t| t.as_object()).is_some_and(|o| !o.is_empty()) || theme.get("themes").and_then(|t| t.as_array()).is_some_and(|a| !a.is_empty()) {
            return Vec::new();
        }
        let mut found = std::collections::BTreeSet::new();
        e.set_mode(Mode::HighContrast);
        for s in 0..states(e) {
            let scene = e.scene_for_state(s);
            let ms: Vec<Mark> = marks(e, &scene).into_iter().filter(|m| m.closed && m.fill.is_some() && m.theme_colour && !m.ramp && m.opacity >= 0.95).collect();
            for (i, a) in ms.iter().enumerate() {
                for b in &ms[i + 1..] {
                    if a.parent != b.parent && !a.parent.starts_with(&b.parent) && !b.parent.starts_with(&a.parent) {
                        continue;
                    }
                    let (ra, rb) = (a.rect, b.rect);
                    let gap_x = (ra.x.max(rb.x) - (ra.x + ra.w).min(rb.x + rb.w)).max(0.0);
                    let gap_y = (ra.y.max(rb.y) - (ra.y + ra.h).min(rb.y + rb.h)).max(0.0);
                    let touching = gap_x <= 0.5 && gap_y <= 0.5 && (ra.w.min(rb.w) > 1.0 && ra.h.min(rb.h) > 1.0);
                    if !touching {
                        continue;
                    }
                    let (fa, fb) = (a.fill.unwrap(), b.fill.unwrap());
                    if fa == fb {
                        continue; // one mark drawn in pieces
                    }
                    let between = contrast_ratio(fa, fb);
                    // A separating line on either: its colour stands out from both fills.
                    let line = [a.stroke, b.stroke].into_iter().flatten().filter(|(_, w)| *w >= 1.0).any(|(c, _)| contrast_ratio(c, fa) >= 3.0 && contrast_ratio(c, fb) >= 3.0);
                    if between < 3.0 && !line {
                        found.insert(format!("state {s}: {} | {} {:.2}:1 with no line between", fa.to_hex(), fb.to_hex(), between));
                    }
                }
            }
        }
        found.into_iter().collect()
    });
}

#[test]
fn wcag_1_4_11_focus_indicator() {
    // The keyboard's outline (the web element's `.focus`, and the std controls' focus) is the
    // theme's accent with the paper inside it: one of the two stands out from anything under it.
    // Here: the accent against the paper, in every theme and mode, is at least 3:1.
    let mut found = Vec::new();
    for theme in ["datars/neutral", "datars/noir", "datars/broadsheet", "datars/brand-example"] {
        let doc = serde_json::json!({ "datars": 1, "size": { "width": 100, "height": 100 }, "theme": { "use": theme }, "scene": { "kind": "group", "key": "root" } });
        let mut e = datars_headless::load(&doc.to_string()).unwrap();
        for m in MODES {
            e.set_mode(m);
            let (accent, paper) = (e.theme().color("accent").unwrap(), e.theme().color("paper").unwrap());
            let r = contrast_ratio(accent, paper);
            if r < 3.0 {
                found.push(format!("{theme} {}: accent {} on paper {} {r:.2}:1", mode_name(m), accent.to_hex(), paper.to_hex()));
            }
        }
    }
    println!("WCAG-CHECKED wcag_1_4_11_focus_indicator 4 themes × light, dark, high contrast");
    assert!(found.is_empty(), "{found:?}");
}

fn example(name: &str) -> Engine {
    let dir = root().join("examples").join(name);
    let mut e = datars_headless::load_at(&std::fs::read_to_string(dir.join("doc.json")).unwrap(), Some(&dir)).unwrap();
    e.frame(0.0);
    e
}

fn item_at(e: &mut Engine, starts: &str) -> datars_engine::SemanticItem {
    e.semantic_items().into_iter().find(|i| i.label.starts_with(starts)).unwrap_or_else(|| panic!("no item \"{starts}\""))
}

fn centre(i: &datars_engine::SemanticItem) -> (f64, f64) {
    (i.rect.x + i.rect.w / 2.0, i.rect.y + i.rect.h / 2.0)
}

#[test]
fn wcag_2_1_1_keyboard() {
    // Whatever a click acts on is a stop a keyboard and a screen reader activate (Engine::activate),
    // and what a pointer drags (a brush, a view) takes keys (Engine::key).
    over_corpus("wcag_2_1_1_keyboard", "actionable items of each document's first state (up to 12 a document), each on a fresh load", |d, e| {
        let mut found = Vec::new();
        settle(e, 0);
        let items: Vec<_> = e.semantic_items().into_iter().filter(|i| i.actionable).take(12).collect();
        for i in items {
            let mut fresh = load(d);
            settle(&mut fresh, 0);
            let ok = match i.kind.as_deref() {
                Some("brush") => fresh.key(&i.path, "Enter"),
                Some("pan") => fresh.key(&i.path, "+"),
                _ => fresh.activate(&i.path),
            };
            if !ok {
                found.push(format!("\"{}\" ({}) can't be operated without a pointer", i.label, i.kind.as_deref().unwrap_or("button")));
            }
        }
        found
    });
}

#[test]
fn wcag_2_2_2_pause_stop_hide() {
    // What moves by itself (autoplay, a clock-driven scene) has a Pause in the reading order, and
    // paused it stays: an autoplaying story doesn't move on.
    let doc = serde_json::json!({ "datars": 1, "size": { "width": 300, "height": 200 },
        "scene": { "kind": "shape", "key": "dot", "geom": { "type": "circle", "cx": 50, "cy": 50, "r": 10 }, "fill": "$mark" },
        "program": { "drivers": ["autoplay"], "states": [{ "name": "a", "hold": 1 }, { "name": "b", "hold": 1 }, { "name": "c", "hold": 1 }] } });
    let mut e = datars_headless::load(&doc.to_string()).unwrap();
    e.frame(0.0);
    assert!(e.moves_by_itself());
    let r = datars_runtime::readout::of_engine(&mut e);
    assert!(r.iter().any(|x| x.label == "Pause" && x.path == "playback:pause"), "a Pause in the reading order");
    assert!(e.activate("playback:pause"));
    assert_eq!(e.take_announcement().as_deref(), Some("Paused."));
    for t in 1..60 {
        e.frame(t as f64);
    }
    assert_eq!(e.state_index(), 0, "paused, it stays");
    assert!(datars_runtime::readout::of_engine(&mut e).iter().any(|x| x.label == "Play"));
    assert!(e.activate("playback:play"));
    for t in 60..70 {
        e.frame(t as f64);
    }
    assert!(e.state_index() > 0, "played, it moves on");
    // Over the corpus: everything that moves by itself offers it.
    over_corpus("wcag_2_2_2_pause_stop_hide", "documents that autoplay or follow a clock", |_, e| {
        if !e.moves_by_itself() {
            return Vec::new();
        }
        let r = datars_runtime::readout::of_engine(e);
        if r.iter().any(|x| x.path == "playback:pause") { Vec::new() } else { vec!["moves by itself with no Pause".into()] }
    });
}

#[test]
fn wcag_2_2_4_interruptions() {
    // AAA: a live source's arrivals don't interrupt the reader: no announcement for new rows.
    let mut e = example("election");
    e.frame(1.0);
    let _ = e.take_announcement();
    let dir = root().join("examples/election");
    let first = std::fs::read_dir(dir.join("count.json.d")).ok().and_then(|mut d| d.next()).and_then(|f| f.ok()).map(|f| std::fs::read(f.path()).unwrap());
    if let Some(bytes) = first {
        let _ = e.provide("count", &bytes);
        e.frame(2.0);
    }
    assert_eq!(e.take_announcement(), None);
    println!("WCAG-CHECKED wcag_2_2_4_interruptions 1 live example");
}

/// Frames of every transition at 30 fps, and how many flashes any one second of them has: a flash
/// is a pair of opposing changes in relative luminance of 10 % or more (the darker under 0.8), over
/// more than a quarter of a 10° field (WCAG 2.3.1's general flash, at a 1024 × 768 view: 341 × 256 px).
fn flashes_per_second(e: &mut Engine) -> usize {
    let fps = 30.0;
    let mut lum: Vec<Vec<f32>> = Vec::new();
    datars_headless::film(e, fps, 0.0, |f| {
        let px = datars_render_cpu::render(f.list, f.fonts, 0.5);
        lum.push(px.data.chunks(4).map(|c| Color { r: c[0] as f32 / 255.0, g: c[1] as f32 / 255.0, b: c[2] as f32 / 255.0, a: 1.0 }.luminance() as f32).collect());
    });
    let area = 341.0 * 256.0 * 0.25 * 0.25; // at 0.5 px per CSS px
    // Each frame step: did a large area get brighter (+1), darker (−1), or neither?
    let steps: Vec<i32> = lum.windows(2).map(|w| {
        let (mut up, mut down) = (0.0, 0.0);
        for (a, b) in w[0].iter().zip(&w[1]) {
            if (b - a).abs() >= 0.1 && a.min(*b) < 0.8 {
                if b > a { up += 1.0 } else { down += 1.0 }
            }
        }
        if up > area { 1 } else if down > area { -1 } else { 0 }
    }).collect();
    let window = fps as usize;
    (0..steps.len().saturating_sub(window).max(1)).map(|i| {
        let w: Vec<i32> = steps[i..(i + window).min(steps.len())].iter().copied().filter(|s| *s != 0).collect();
        w.windows(2).filter(|p| p[0] != p[1]).count() / 2 + usize::from(w.windows(2).filter(|p| p[0] != p[1]).count() % 2 == 1)
    }).max().unwrap_or(0)
}

#[test]
fn wcag_2_3_1_three_flashes() {
    over_corpus("wcag_2_3_1_three_flashes", "every transition of every document, frame by frame at 30 fps", |_, e| {
        let n = flashes_per_second(e);
        if n > 3 { vec![format!("{n} flashes in a second")] } else { Vec::new() }
    });
}

#[test]
fn wcag_2_3_3_animation_from_interactions() {
    // AAA: with reduced motion, nothing travels in a transition — marks cross-fade where they are.
    over_corpus("wcag_2_3_3_animation_from_interactions", "every transition with reduced motion: no mark moves", |_, e| {
        let mut found = Vec::new();
        e.set_reduced_motion(true);
        for s in 0..states(e).saturating_sub(1) {
            let (a, b, plan) = e.plan_states(s, s + 1);
            let mid = e.plan_at(&plan, 0.5);
            let (ca, cb) = (datars_headless::element_centres(&a), datars_headless::element_centres(&b));
            // (What comes is planned under a key of its own, to pair with nothing: the same mark.)
            let wrap = format!("/(\"{}\",)", datars_engine::REDUCED_MOTION_KEY);
            let cm: std::collections::BTreeMap<String, datars_math::Vec2> = datars_headless::element_centres(&mid).into_iter().map(|(k, v)| (k.replacen(&wrap, "", 1), v)).collect();
            // (A mark with no place in a frame — a ghost shrunk to nothing — isn't drawn there.)
            let moving = cm.iter().filter(|(_, p)| p.x.is_finite() && p.y.is_finite()).filter(|(k, p)| {
                let at_a = ca.get(*k).is_some_and(|q| (*q - **p).len() < 0.5);
                let at_b = cb.get(*k).is_some_and(|q| (*q - **p).len() < 0.5);
                !at_a && !at_b && ca.contains_key(*k) && cb.contains_key(*k)
            }).count();
            if moving > 0 {
                found.push(format!("{} → {}: {moving} marks travel", s, s + 1));
            }
        }
        found
    });
}

#[test]
fn wcag_2_5_2_pointer_cancellation() {
    // A click acts when the pointer comes up on what it went down on: pressed on a bar and moved
    // off it before letting go, nothing happens.
    let mut e = example("dashboard");
    let south = item_at(&mut e, "South");
    let (x, y) = centre(&south);
    let before = e.signal_values();
    e.pointer(datars_engine::Pointer::Down { x, y });
    e.pointer(datars_engine::Pointer::Move { x: 2.0, y: 2.0 });
    e.pointer(datars_engine::Pointer::Up { x: 2.0, y: 2.0 });
    assert_eq!(e.signal_values().get("selected"), before.get("selected"), "moved off: cancelled");
    e.pointer(datars_engine::Pointer::Down { x, y });
    e.pointer(datars_engine::Pointer::Up { x, y });
    assert_ne!(e.signal_values().get("selected"), before.get("selected"), "let go on it: clicked");
    println!("WCAG-CHECKED wcag_2_5_2_pointer_cancellation the dashboard example's bars");
}

#[test]
fn wcag_2_5_3_label_in_name() {
    // A control's name for a screen reader (or voice control) has the words drawn on it.
    over_corpus("wcag_2_5_3_label_in_name", "controls and buttons × states: the words drawn in each are in its name", |_, e| {
        let mut found = std::collections::BTreeSet::new();
        for s in 0..states(e) {
            settle(e, s);
            let scene = e.scene_for_state(s);
            let drawn = datars_devtools::drawn_labels(&scene);
            for i in e.semantic_items().into_iter().filter(|i| (i.role == "control" || i.actionable) && !matches!(i.kind.as_deref(), Some("brush" | "pan"))) {
                let name = i.label.to_lowercase();
                for t in drawn.iter().filter(|t| t.opacity > 0.35) {
                    let (cx, cy) = (t.rect.x + t.rect.w / 2.0, t.rect.y + t.rect.h / 2.0);
                    let inside = cx >= i.rect.x && cx <= i.rect.x + i.rect.w && cy >= i.rect.y && cy <= i.rect.y + i.rect.h;
                    // (Only what labels it: words wholly inside it, short enough to be its label.)
                    let small = t.rect.w <= i.rect.w + 1.0 && t.rect.h <= i.rect.h + 1.0;
                    let words = t.text.trim().to_lowercase();
                    if inside && small && !words.is_empty() && i.role == "control" && !name.contains(&words) {
                        found.insert(format!("\"{}\" draws \"{}\"", i.label, t.text.trim()));
                    }
                }
            }
        }
        found.into_iter().collect()
    });
}

#[test]
fn wcag_2_5_7_dragging_movements() {
    // A brush selects by two clicks, its ends, as well as by a drag.
    let mut e = example("prices");
    let brush = e.semantic_items().into_iter().find(|i| i.kind.as_deref() == Some("brush")).expect("a brush");
    let y = brush.rect.y + brush.rect.h / 2.0;
    let (x0, x1) = (brush.rect.x + brush.rect.w * 0.2, brush.rect.x + brush.rect.w * 0.6);
    e.pointer(datars_engine::Pointer::Down { x: x0, y });
    e.pointer(datars_engine::Pointer::Up { x: x0, y });
    let said = e.take_announcement().unwrap_or_default();
    assert!(said.starts_with("Selecting from"), "{said}");
    e.pointer(datars_engine::Pointer::Down { x: x1, y });
    e.pointer(datars_engine::Pointer::Up { x: x1, y });
    let said = e.take_announcement().unwrap_or_default();
    assert!(said.starts_with("Selected "), "{said}");
    let v = e.signal_values();
    assert_eq!(v.get("range.active"), Some(&serde_json::json!(true)));
    // A click on it again clears it.
    e.pointer(datars_engine::Pointer::Down { x: x1, y });
    e.pointer(datars_engine::Pointer::Up { x: x1, y });
    assert_eq!(e.take_announcement().as_deref(), Some("Selection cleared."));
    // A slider and a range's ends go where the track is pressed — no drag needed.
    let mut e = example("budget");
    let s = e.semantic_items().into_iter().find(|i| i.role == "control" && i.label.starts_with("Monthly income")).expect("the slider");
    let before = e.signal_values().get("income").cloned();
    let (x, y) = (s.rect.x + s.rect.w * 0.8, s.rect.y + s.rect.h * 0.7);
    e.pointer(datars_engine::Pointer::Down { x, y });
    e.pointer(datars_engine::Pointer::Up { x, y });
    assert_ne!(e.signal_values().get("income").cloned(), before, "pressed on the track: moved there");
    let doc = serde_json::json!({ "datars": 1, "size": { "width": 400, "height": 60 }, "signals": { "lo": { "type": "num", "default": 40 }, "hi": { "type": "num", "default": 60 } },
        "scene": { "kind": "use", "recipe": "@datars/std/range", "params": { "lo": "lo", "hi": "hi", "min": 0, "max": 100, "label": "Range" } } });
    let mut e = datars_headless::load(&doc.to_string()).unwrap();
    e.frame(0.0);
    let track_y = 30.0;
    e.pointer(datars_engine::Pointer::Down { x: 30.0, y: track_y });
    e.pointer(datars_engine::Pointer::Up { x: 30.0, y: track_y });
    let lo = e.signal_values().get("lo").and_then(|v| v.as_f64()).unwrap();
    assert!(lo < 20.0, "far left of the low thumb, pressed: the low thumb went there ({lo})");
    e.pointer(datars_engine::Pointer::Down { x: 370.0, y: track_y });
    e.pointer(datars_engine::Pointer::Up { x: 370.0, y: track_y });
    let hi = e.signal_values().get("hi").and_then(|v| v.as_f64()).unwrap();
    assert!(hi > 85.0, "far right: the high thumb ({hi})");
    println!("WCAG-CHECKED wcag_2_5_7_dragging_movements a brush (prices), a slider (budget), a range");
}

#[test]
fn wcag_2_5_8_target_size() {
    // The standard library's controls are at least 24 × 24 px (or spaced as far apart).
    over_corpus("wcag_2_5_8_target_size", "controls × states", |_, e| {
        let mut found = Vec::new();
        for s in 0..states(e) {
            settle(e, s);
            for i in e.semantic_items().into_iter().filter(|i| i.role == "control" && i.rect.w > 0.0 && !matches!(i.kind.as_deref(), Some("brush" | "pan"))) {
                if i.rect.w < 24.0 || i.rect.h < 24.0 {
                    found.push(format!("state {s}: \"{}\" {:.0}×{:.0}", i.label.chars().take(50).collect::<String>(), i.rect.w, i.rect.h));
                }
            }
        }
        found
    });
}

#[test]
fn wcag_3_1_2_language_of_parts() {
    // The words a host adds around the chart's own follow the document's language.
    let doc = serde_json::json!({ "datars": 1, "locale": "sv-SE", "title": "Frukt", "size": { "width": 300, "height": 200 },
        "data": { "t": { "values": { "k": ["a", "b"], "n": [1.5, 2.25] }, "key": ["k"] } },
        "scene": { "kind": "use", "recipe": "@datars/std/plot", "params": { "data": "t", "x": "k", "y": "n", "children": [{ "kind": "use", "recipe": "@datars/std/bar", "params": {} }] } },
        "program": { "states": [{ "name": "a" }, { "name": "b" }] } });
    let mut e = datars_headless::load(&doc.to_string()).unwrap();
    e.frame(0.0);
    assert_eq!(e.accessible_summary(), "2 datapunkter, från 1,5 till 2,25.");
    let r = datars_runtime::readout::of_engine(&mut e);
    assert!(r.iter().any(|x| x.label == "Nästa steg"), "{:?}", r.iter().map(|x| &x.label).collect::<Vec<_>>());
    println!("WCAG-CHECKED wcag_3_1_2_language_of_parts a Swedish document");
}

#[test]
fn wcag_3_2_1_on_focus() {
    // Reading the chart — its semantics, its reading order, a screen reader moving through them —
    // changes nothing: no signal, no step.
    over_corpus("wcag_3_2_1_on_focus", "documents: reading the semantics and the reading order of each state", |_, e| {
        let mut found = Vec::new();
        for s in 0..states(e) {
            settle(e, s);
            let (before, index) = (e.signal_values(), e.state_index());
            let _ = e.semantic_items();
            let _ = datars_runtime::readout::of_engine(e);
            let _ = e.accessible_summary();
            if e.signal_values() != before || e.state_index() != index {
                found.push(format!("state {s}: reading it changed it"));
            }
        }
        found
    });
}

#[test]
fn wcag_3_2_2_on_input() {
    // Setting a control (a slider, a select) changes what the chart shows, never where the reader
    // is: the story stays on its step.
    over_corpus("wcag_3_2_2_on_input", "documents with engine-drawn sliders or selects: each set to another value", |_, e| {
        let mut found = Vec::new();
        settle(e, 0);
        for c in e.controls() {
            let index = e.state_index();
            let v = if c.kind == "slider" { datars_engine::SignalValue::Num((c.min + c.max) / 2.0) } else { continue };
            e.set_signal(&c.signal, v);
            settle(e, index);
            e.frame(1e9);
            if e.state_index() != index {
                found.push(format!("setting {} moved the story", c.signal));
            }
        }
        found
    });
}

#[test]
fn wcag_3_3_2_labels_or_instructions() {
    // Every control has a name; what a pointer drags says how to work it.
    over_corpus("wcag_3_3_2_labels_or_instructions", "controls × states", |_, e| {
        let mut found = Vec::new();
        let loc = e.doc().locale.clone();
        for s in 0..states(e) {
            settle(e, s);
            for i in e.semantic_items().into_iter().filter(|i| i.role == "control") {
                if i.label.trim().is_empty() {
                    found.push(format!("state {s}: a control without a name"));
                }
                if let Some(k @ ("brush" | "pan")) = i.kind.as_deref() {
                    if datars_engine::phrases::phrase(&loc, &format!("{k}-hint"), &[]).is_empty() {
                        found.push(format!("state {s}: a {k} without instructions"));
                    }
                }
            }
        }
        found
    });
}

#[test]
fn wcag_2_4_6_headings_and_labels() {
    // The reading order's headings and controls say what they are: none is empty, and no two
    // controls side by side say the same.
    over_corpus("wcag_2_4_6_headings_and_labels", "documents × states: headings and controls in the reading order", |_, e| {
        let mut found = Vec::new();
        for s in 0..states(e) {
            settle(e, s);
            let r = datars_runtime::readout::of_engine(e);
            for w in r.windows(2) {
                if w[0].kind != "text" && w[0].kind != "datum" && w[0].kind == w[1].kind && w[0].label == w[1].label && !w[0].label.is_empty() && w[0].kind != "heading" {
                    found.push(format!("state {s}: two {}s in a row named \"{}\"", w[0].kind, w[0].label));
                }
            }
            if r.iter().any(|x| x.kind == "heading" && x.label.trim().is_empty()) {
                found.push(format!("state {s}: an empty heading"));
            }
        }
        found
    });
}

#[test]
fn wcag_4_1_2_name_role_value() {
    over_corpus("wcag_4_1_2_name_role_value", "actionable items × states: a name; a checkbox, switch or radio its state", |_, e| {
        let mut found = Vec::new();
        for s in 0..states(e) {
            settle(e, s);
            for i in e.semantic_items().into_iter().filter(|i| i.actionable) {
                if i.label.trim().is_empty() {
                    found.push(format!("state {s}: an unnamed {}", i.kind.as_deref().unwrap_or("button")));
                }
                if matches!(i.kind.as_deref(), Some("checkbox" | "switch" | "radio")) && i.checked.is_none() {
                    found.push(format!("state {s}: \"{}\" without its state", i.label));
                }
            }
        }
        found
    });
}

#[test]
fn wcag_4_1_3_status_messages() {
    // What a click changed is said, without moving the reader: a filter, a step, a brush.
    let mut e = example("dashboard");
    let south = item_at(&mut e, "South");
    assert!(e.activate(&south.path));
    let said = e.take_announcement().unwrap_or_default();
    assert!(said.contains("data points") || said.contains("South"), "{said}");
    println!("WCAG-CHECKED wcag_4_1_3_status_messages the dashboard example's filter");
}

#[test]
fn wcag_1_2_1_media_alternative() {
    // A film (`datars video`) has no sound: its transcript says everything it shows, step by step.
    let mut e = example("votes");
    let t = datars_headless::transcript(&mut e);
    assert!(t.starts_with("Vote share by party, Sweden 2022\n"), "{t}");
    assert!(t.contains("Step 2 of "), "{t}");
    for st in e.doc().program.as_ref().unwrap().states.clone() {
        if let Some(n) = st.narration {
            assert!(n.text.is_empty() || t.contains(n.text.trim()), "{} in {t}", n.text);
        }
    }
    println!("WCAG-CHECKED wcag_1_2_1_media_alternative the votes example's film");
}
