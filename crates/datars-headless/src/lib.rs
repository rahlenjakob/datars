//! `datars-headless` — how tests, the CLI, servers and coding agents see datars output: CPU
//! reference rendering (bit-exact everywhere, P1), filmstrips and motion trails of transitions
//! (docs/13-testing.md), SVG export and scene snapshots. No GPU, no browser, no network.

use datars_engine::{Engine, Request};
use std::path::{Path, PathBuf};
use datars_math::{Affine, PathData, Rect, Vec2};
use datars_render_cpu::Pixmap;
use datars_scene::{Geom, Key, KeyPath, Node, NodeKind, Paint, Scene, Stroke};
use datars_theme::Ink;
use std::collections::BTreeMap;

/// Load a document (JSON IR) into an engine. Requests (data URLs, atlases) stay pending; use
/// [`load_at`] to fulfil them from the filesystem.
pub fn load(json: &str) -> Result<Engine, String> {
    let (doc, notes) = datars_ir::Doc::from_json_checked(json)?;
    let mut e = Engine::new();
    e.load(doc);
    e.add_diagnostics(notes);
    Ok(e)
}

/// Load a document and fulfil its requests from disk, the way a host would over the network:
/// relative data URLs resolve against `dir` (the document's directory); atlases (`countries`, …)
/// are looked up as `atlas/<name>.geojson|.topojson` under `dir`, `dir/assets`, every ancestor's
/// `assets/`, and `$DATARS_ASSETS`. Remote URLs and slots are left pending (no network here).
pub fn load_at(json: &str, dir: Option<&Path>) -> Result<Engine, String> {
    load_at_with(json, dir, &|r| fetch_from_disk(r, dir))
}

/// [`load_at`] with the host's IO in `fetch`: the CLI passes one that also acquires fonts
/// (Google Fonts, font URLs) through its cache, so previews draw what bundles ship. Tile ranges
/// are still read from disk relative to `dir`.
pub fn load_at_with(json: &str, dir: Option<&Path>, fetch: &dyn Fn(&Request) -> Option<Vec<u8>>) -> Result<Engine, String> {
    let mut e = load(json)?;
    // Tile archives are read by range as frames need them, like a host serving `Range` requests.
    let base = dir.map(Path::to_path_buf);
    e.set_range_fetch(Box::new(move |url, offset, length| read_range(&tile_file(url, base.as_deref())?, offset, length)));
    for req in e.requests() {
        let name = match &req {
            Request::Atlas { name, .. } | Request::Source { name, .. } | Request::Slot { name, .. } => name.clone(),
            Request::Range { .. } => continue, // through the range reader above
        };
        match fetch(&req) {
            Some(bytes) => e.provide(&name, &bytes).map_err(|err| format!("{name}: {err}"))?,
            None if matches!(req, Request::Slot { .. }) => {}
            // A missing font is diagnosed by the engine (`datars check`), with what it falls back to.
            None if name.starts_with("font:") => {}
            // Remote data and Google Fonts aren't on disk by design (a disk-only load: `datars
            // basemap`); commands that need them pass a fetch that acquires them.
            None if matches!(&req, Request::Source { url, .. } if url.contains("://") || url.starts_with("google:")) => {}
            None => eprintln!("warning: nothing on disk for request {req:?}"),
        }
    }
    Ok(e)
}

/// The bytes for a `datars:fonts/<file>` URL: the default fonts that ship with the datars
/// toolchain and runtimes (compiled into this build, else the source tree's `fonts/`).
pub fn datars_asset(url: &str) -> Option<Vec<u8>> {
    let rest = url.strip_prefix("datars:")?;
    let file = rest.strip_prefix("fonts/")?;
    if file.contains("..") || file.contains('/') {
        return None;
    }
    datars_text::bundled_file(file).map(|b| b.to_vec()).or_else(|| std::fs::read(PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fonts")).join(file)).ok())
}

/// A tile archive on disk: relative to `dir`, else under an asset root (`dir/assets`, every
/// ancestor's `assets/`, `$DATARS_ASSETS`).
fn tile_file(url: &str, dir: Option<&Path>) -> Option<PathBuf> {
    if url.contains("://") {
        return None;
    }
    let direct = dir.map(|d| d.join(url)).filter(|p| p.is_file());
    direct.or_else(|| asset_roots(dir).into_iter().map(|r| r.join(url)).find(|p| p.is_file()))
}

/// `length` bytes at `offset` (fewer at the end of the file).
pub fn read_range(file: &Path, offset: u64, length: u64) -> Option<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(file).ok()?;
    f.seek(SeekFrom::Start(offset)).ok()?;
    let mut buf = Vec::with_capacity(length as usize);
    f.take(length).read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// What a host would fetch for `req`, from disk: relative data URLs against `dir`; atlases as
/// `atlas/<name>.geojson|.topojson` under `dir`, `dir/assets`, every ancestor's `assets/`, and
/// `$DATARS_ASSETS`; byte ranges of tile archives; `datars:fonts/…` from the distribution's
/// default fonts. Remote URLs, `google:` fonts and slots: None (no network here).
pub fn fetch_from_disk(req: &Request, dir: Option<&Path>) -> Option<Vec<u8>> {
    if let Request::Source { url, .. } = req {
        if url.starts_with("datars:") {
            return datars_asset(url);
        }
    }
    let file = match req {
        Request::Atlas { atlas, .. } => asset_roots(dir).iter().flat_map(|r| ["geojson", "topojson"].map(|ext| r.join("atlas").join(format!("{atlas}.{ext}")))).find(|p| p.is_file()),
        Request::Source { url, .. } if !url.contains("://") => dir.map(|d| d.join(url)).filter(|p| p.is_file()),
        Request::Range { url, offset, length, .. } => return read_range(&tile_file(url, dir)?, *offset, *length),
        _ => None,
    }?;
    std::fs::read(file).ok()
}

fn asset_roots(dir: Option<&Path>) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(d) = dir {
        v.push(d.to_path_buf());
        for a in d.ancestors() {
            v.push(a.join("assets"));
        }
    }
    if let Ok(env) = std::env::var("DATARS_ASSETS") {
        v.push(PathBuf::from(env));
    }
    // The built-in atlases of the source tree this tool was built from (documents anywhere on
    // disk — `datars new` scaffolds — still find `countries`).
    v.push(PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets")));
    v
}

/// Render a scene with the CPU reference rasterizer.
pub fn render_scene(engine: &Engine, scene: &Scene, dpr: f64) -> Pixmap {
    let list = engine.display_list(scene);
    datars_render_cpu::render(&list, engine.fonts(), dpr)
}

pub fn render_state(engine: &mut Engine, state: usize, dpr: f64) -> Pixmap {
    let scene = engine.scene_for_state(state);
    render_scene(engine, &scene, dpr)
}

pub fn svg_state(engine: &mut Engine, state: usize) -> String {
    svg_state_with(engine, state, &datars_render_svg::SvgOptions::default())
}

/// A state as a one-page vector PDF (print, reports): glyph outlines plus searchable text.
pub fn pdf_state(engine: &mut Engine, state: usize) -> Vec<u8> {
    let scene = engine.scene_for_state(state);
    datars_render_pdf::to_pdf(&engine.display_list(&scene), engine.fonts())
}

/// Several states as one PDF, a page each in the order given (a story as a handout).
pub fn pdf_states(engine: &mut Engine, states: &[usize]) -> Vec<u8> {
    let lists: Vec<_> = states.iter().map(|&s| {
        let scene = engine.scene_for_state(s);
        engine.display_list(&scene)
    }).collect();
    datars_render_pdf::to_pdf_pages(&lists, engine.fonts())
}

/// [`svg_state`] with output options (posters use [`datars_render_svg::SvgOptions::poster`]).
pub fn svg_state_with(engine: &mut Engine, state: usize, opts: &datars_render_svg::SvgOptions) -> String {
    let scene = engine.scene_for_state(state);
    let svg = datars_render_svg::to_svg_with(&engine.display_list(&scene), engine.fonts(), opts);
    // Elements that link somewhere (a map's OpenStreetMap credit) stay links in the file.
    let links = datars_engine::pick::links(&scene);
    if links.is_empty() {
        return svg;
    }
    let esc = |s: &str| s.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;");
    let areas: String = links
        .iter()
        .map(|l| format!("<a href=\"{h}\" xlink:href=\"{h}\"><title>{t}</title><rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"transparent\"/></a>\n", l.bounds[0], l.bounds[1], l.bounds[2], l.bounds[3], h = esc(&l.href), t = esc(&l.label)))
        .collect();
    match svg.rfind("</svg>") {
        Some(i) => format!("{}{areas}{}", &svg[..i], &svg[i..]),
        None => svg,
    }
}

/// Copy `src` into `dst` at (x, y), with an optional downscale factor (box filter).
pub fn blit(dst: &mut Pixmap, src: &Pixmap, x: u32, y: u32, down: u32) {
    let d = down.max(1);
    for sy in 0..src.height / d {
        for sx in 0..src.width / d {
            let mut acc = [0u32; 4];
            for oy in 0..d {
                for ox in 0..d {
                    let i = (((sy * d + oy) * src.width + sx * d + ox) * 4) as usize;
                    for c in 0..4 {
                        acc[c] += src.data[i + c] as u32;
                    }
                }
            }
            let (tx, ty) = (x + sx, y + sy);
            if tx < dst.width && ty < dst.height {
                let j = ((ty * dst.width + tx) * 4) as usize;
                for c in 0..4 {
                    dst.data[j + c] = (acc[c] / (d * d)) as u8;
                }
            }
        }
    }
}

fn blank(w: u32, h: u32, rgba: [u8; 4]) -> Pixmap {
    let mut p = Pixmap { width: w, height: h, data: vec![0; (w * h * 4) as usize] };
    for px in p.data.as_chunks_mut::<4>().0 {
        px.copy_from_slice(&rgba);
    }
    p
}

/// Frames of a transition from state `a` to `b`, side by side (`n` frames including both ends).
pub fn filmstrip(engine: &mut Engine, a: usize, b: usize, n: usize, dpr: f64) -> Pixmap {
    let (_, _, plan) = engine.plan_states(a, b);
    let frames: Vec<Pixmap> = (0..n.max(2))
        .map(|i| {
            let t = i as f64 / (n.max(2) - 1) as f64;
            render_scene(engine, &engine.plan_at(&plan, t), dpr)
        })
        .collect();
    let (fw, fh) = (frames[0].width, frames[0].height);
    let gap = 8;
    let mut out = blank(fw * frames.len() as u32 + gap * (frames.len() as u32 - 1), fh, [236, 236, 236, 255]);
    for (i, f) in frames.iter().enumerate() {
        blit(&mut out, f, i as u32 * (fw + gap), 0, 1);
    }
    out
}

/// Element centres (device px) per key path in a scene — for trails and motion checks. An element
/// flying between containers in a plan's frame (under the flight layer, in groups mirroring its
/// destination path) is reported at its destination path, as motion's own flattening does, so its
/// track joins up with where it lands.
pub fn element_centres(scene: &Scene) -> BTreeMap<String, Vec2> {
    let mut out = BTreeMap::new();
    fn go(n: &Node, path: &KeyPath, xf: Affine, out: &mut BTreeMap<String, Vec2>) {
        let xf = n.common.placed(xf);
        let here = if n.key == Key::name(datars_motion::FLIGHT_KEY) { path.clone() } else { path.push(&n.key) };
        match &n.kind {
            NodeKind::Shape { geom, .. } => {
                out.insert(here.to_string(), xf.apply(geom.center()));
            }
            NodeKind::Instances(i) => {
                for k in 0..i.len().min(5000) {
                    out.insert(here.push(&i.keys[k]).to_string(), xf.apply(Vec2::new(i.x[k], i.y[k])));
                }
            }
            NodeKind::View { viewport, camera, children, .. } => {
                let cam = camera.map(|c| c.transform(*viewport)).unwrap_or(Affine::translate(viewport.x, viewport.y));
                for c in children {
                    go(c, &here, xf.mul(cam), out);
                }
            }
            _ => {
                for c in n.children() {
                    go(c, &here, xf, out);
                }
            }
        }
    }
    go(&scene.root, &KeyPath::default(), Affine::IDENTITY, &mut out);
    out
}

/// The path of every element that moves in a transition (by its key path in the target), sampled
/// `samples` times — what [`trails`] draws.
pub fn trail_paths(engine: &mut Engine, a: usize, b: usize, samples: usize) -> Vec<(String, Vec<Vec2>)> {
    let (_, _, plan) = engine.plan_states(a, b);
    paths_of(engine, &plan, samples)
}

fn paths_of(engine: &Engine, plan: &datars_motion::Plan, samples: usize) -> Vec<(String, Vec<Vec2>)> {
    let n = samples.max(3);
    // Trails follow planned motion (tile content is per-frame, not motion). At exactly 0 a plan is
    // its source scene, where an element changing container still sits at its old path; just
    // after, it's in the flight layer under its new one — so start there.
    let at = |i: usize| if i == 0 { 1e-9 } else { i as f64 / (n - 1) as f64 };
    let tracks: Vec<BTreeMap<String, Vec2>> = (0..n).map(|i| element_centres(&engine.planned_at(plan, at(i)))).collect();
    let mut out = Vec::new();
    for (k, end) in &tracks[n - 1] {
        let pts: Vec<Vec2> = tracks.iter().filter_map(|m| m.get(k).copied()).collect();
        if pts.len() >= 2 && pts[0].dist(*end) >= 1.0 {
            out.push((k.clone(), pts));
        }
    }
    out
}

/// One image of a whole transition: the end state with every element's path drawn as a trail
/// (older samples lighter) — motion an agent can inspect in a single picture.
pub fn trails(engine: &mut Engine, a: usize, b: usize, samples: usize, dpr: f64) -> Pixmap {
    let (_, sb, plan) = engine.plan_states(a, b);
    let paths = paths_of(engine, &plan, samples);
    let mut overlay = Vec::new();
    for (k, pts) in &paths {
        overlay.push(
            Node::shape(Key::one(k.as_str()), Geom::polyline(pts.clone()))
                .stroke(Stroke::new(Paint::Solid(Ink::token("accent")), 1.25))
                .opacity(0.55),
        );
        overlay.push(Node::shape(Key::one(format!("{k}#start")), Geom::circle(pts[0].x, pts[0].y, 2.5)).fill(Paint::Solid(Ink::token("muted"))));
    }
    let mut scene = sb.clone();
    let root = std::mem::replace(&mut scene.root, Node::group(Key::name("x"), vec![]));
    scene.root = Node::group(Key::name("trails-root"), vec![root.opacity(0.35), Node::group(Key::name("trails"), overlay)]);
    render_scene(engine, &scene, dpr)
}

/// A stable hash of every sampled frame of a transition (dense capture, docs/13-testing.md).
pub fn sweep_hashes(engine: &mut Engine, a: usize, b: usize, samples: usize) -> Vec<u64> {
    let (_, _, plan) = engine.plan_states(a, b);
    (0..samples.max(2)).map(|i| engine.plan_at(&plan, i as f64 / (samples.max(2) - 1) as f64).hash()).collect()
}

/// A bounding-box overview of a scene (debugging aid).
pub fn bounds(scene: &Scene) -> Rect {
    datars_engine::node_bounds(&scene.root)
}

#[allow(dead_code)]
fn unused(_: PathData) {}

/// A caption cue, in seconds.
#[derive(Clone, Debug, PartialEq)]
pub struct Cue {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// One frame of a film: the scene, its display list, the fonts to draw it with, and how many
/// consecutive frames show it (holds repeat a frame).
pub struct FilmFrame<'a> {
    pub scene: &'a Scene,
    pub list: &'a datars_render::DisplayList,
    pub fonts: &'a datars_text::FontDb,
    pub repeat: usize,
}

/// Render the document's program as a film (docs/10-platforms.md §Video): each state held for its
/// `hold` (else `default_hold`) seconds, then the transition to the next sampled frame-exactly at
/// `fps` — the same plans the live views play. `frame` receives every distinct frame in order.
/// Returns caption cues from the states' narration: a state's text shows from the start of the
/// transition into it until the transition out of it begins.
pub fn film(engine: &mut Engine, fps: f64, default_hold: f64, frame: impl FnMut(FilmFrame)) -> Vec<Cue> {
    film_with_chapters(engine, fps, default_hold, frame).0
}

/// [`film`], and its chapters: one cue per state, named by the state, for the time the film holds
/// it (`datars video` writes them as a WebVTT chapters track). Players show them as chapter marks,
/// and a page that needs a state's exact frame seeks into its hold — captions alone can't say
/// where the holds are, and a state without narration has none.
pub fn film_with_chapters(engine: &mut Engine, fps: f64, default_hold: f64, mut frame: impl FnMut(FilmFrame)) -> (Vec<Cue>, Vec<Cue>) {
    let fps = fps.max(1.0);
    let names = engine.state_names();
    let mut chapters = Vec::new();
    let states: Vec<datars_ir::State> = engine.doc().program.as_ref().map(|p| p.states.clone()).unwrap_or_default();
    let n = engine.state_names().len().max(1);
    let mut frames = 0usize; // frames emitted so far
    let secs = |f: usize| f as f64 / fps;
    let mut cues = Vec::new();
    let mut cue_start = 0.0;
    for i in 0..n {
        let st = states.get(i);
        let hold = st.and_then(|s| s.hold).unwrap_or(default_hold).max(0.0);
        let hold_frames = ((hold * fps).round() as usize).max(1);
        let scene = engine.scene_for_state(i);
        let list = engine.display_list(&scene);
        frame(FilmFrame { scene: &scene, list: &list, fonts: engine.fonts(), repeat: hold_frames });
        chapters.push(Cue { start: secs(frames), end: secs(frames + hold_frames), text: names.get(i).cloned().unwrap_or_else(|| format!("state {i}")) });
        frames += hold_frames;
        let mut transition_start = secs(frames);
        if i + 1 < n {
            let (_, _, plan) = engine.plan_states(i, i + 1);
            let tf = (plan.duration() * fps).round() as usize;
            for k in 1..tf {
                let s = engine.plan_at(&plan, k as f64 / tf as f64);
                let list = engine.display_list(&s);
                frame(FilmFrame { scene: &s, list: &list, fonts: engine.fonts(), repeat: 1 });
            }
            transition_start = secs(frames);
            frames += tf.saturating_sub(1);
        }
        if let Some(nar) = st.and_then(|s| s.narration.as_ref()) {
            let text = [nar.title.as_str(), nar.text.as_str()].iter().filter(|t| !t.is_empty()).copied().collect::<Vec<_>>().join(" — ");
            if !text.is_empty() {
                let end = if i + 1 < n { transition_start } else { secs(frames) };
                cues.push(Cue { start: cue_start, end, text });
            }
        }
        cue_start = transition_start;
    }
    (cues, chapters)
}

/// WebVTT captions for [`film`] cues.
pub fn webvtt(cues: &[Cue]) -> String {
    let ts = |t: f64| {
        let ms = (t * 1000.0).round() as u64;
        format!("{:02}:{:02}:{:02}.{:03}", ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000)
    };
    let mut s = String::from("WEBVTT\n");
    for (i, c) in cues.iter().enumerate() {
        s.push_str(&format!("\n{}\n{} --> {}\n{}\n", i + 1, ts(c.start), ts(c.end), c.text));
    }
    s
}

/// SubRip (.srt) captions for [`film`] cues — what video editors and most social sites import.
pub fn srt(cues: &[Cue]) -> String {
    let ts = |t: f64| {
        let ms = (t * 1000.0).round() as u64;
        format!("{:02}:{:02}:{:02},{:03}", ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000)
    };
    let mut s = String::new();
    for (i, c) in cues.iter().enumerate() {
        if i > 0 {
            s.push('\n');
        }
        s.push_str(&format!("{}\n{} --> {}\n{}\n", i + 1, ts(c.start), ts(c.end), c.text));
    }
    s
}

#[cfg(test)]
mod film_tests {
    use super::*;

    const DOC: &str = r#"{"datars":1,"size":{"width":100,"height":60},
      "signals":{"w":{"type":"num","default":10}},
      "scene":{"kind":"shape","key":"bar","geom":{"type":"rect","x":0,"y":0,"w":"=w","h":20},"fill":"$accent"},
      "program":{"states":[
        {"name":"a","set":{"w":10},"hold":1,"narration":{"text":"Small."}},
        {"name":"b","set":{"w":90},"hold":0.5,"narration":{"title":"Big","text":"Now wide."}}]}}"#;

    #[test]
    fn films_hold_then_transition_frame_exactly() {
        let mut e = load(DOC).unwrap();
        let (mut distinct, mut total) = (0, 0);
        let (cues, chapters) = film_with_chapters(&mut e, 10.0, 2.0, |f| {
            distinct += 1;
            total += f.repeat;
        });
        // Each state's hold, by name: 0–1 s, then (after the transition's 8 in-between frames) 1.8–2.3 s.
        assert_eq!(chapters.iter().map(|c| (c.text.as_str(), c.start, c.end)).collect::<Vec<_>>(), [("a", 0.0, 1.0), ("b", 1.8, 2.3)]);
        // 1 s hold (10 frames) + 0.9 s transition (8 in-between frames) + 0.5 s hold (5 frames).
        assert_eq!(total, 10 + 8 + 5);
        assert_eq!(distinct, 1 + 8 + 1);
        assert_eq!(cues.len(), 2);
        assert_eq!((cues[0].start, cues[0].end), (0.0, 1.0));
        assert_eq!(cues[1].text, "Big — Now wide.");
        assert!(webvtt(&cues).contains("00:00:01.000 --> 00:00:02.300"));
        let srt = srt(&cues);
        assert!(srt.starts_with("1\n00:00:00,000 --> 00:00:01,000\nSmall.\n"), "{srt}");
        assert!(srt.contains("\n2\n00:00:01,000 --> 00:00:02,300\nBig — Now wide.\n"));
    }

    /// An element that changes parent (a by-key morph from one group into another) has a trail:
    /// mid-flight it's under the flight layer, which the trail reads as its destination path (it
    /// found no match there, so the trails image showed nothing moving while the strip did).
    #[test]
    fn an_element_changing_parent_leaves_a_trail() {
        let doc = r#"{"datars":1,"size":{"width":200,"height":60},
          "signals":{"side":{"type":"num","default":0}},
          "scene":{"kind":"group","key":"root","children":[
            {"kind":"group","key":"left","children":[
              {"kind":"shape","key":"dot","when":"=side == 0","geom":{"type":"circle","cx":20,"cy":30,"r":5},"fill":"$accent"}]},
            {"kind":"group","key":"right","transform":{"translate":[100,0]},"children":[
              {"kind":"shape","key":"dot","when":"=side == 1","geom":{"type":"circle","cx":60,"cy":30,"r":5},"fill":"$accent"}]}]},
          "motion":{"rules":[{"matcher":"by-key"}]},
          "program":{"states":[{"name":"a","set":{"side":0}},{"name":"b","set":{"side":1}}]}}"#;
        let mut e = load(doc).unwrap();
        let paths = trail_paths(&mut e, 0, 1, 12);
        let (k, pts) = paths.iter().find(|(k, _)| k.ends_with("(\"dot\",)")).unwrap_or_else(|| panic!("no trail: {paths:?}"));
        assert!(k.contains("right"), "under its destination: {k}");
        assert_eq!(pts.len(), 12, "every sample, the first included: {pts:?}");
        assert!((pts[0].x - 20.0).abs() < 1e-6 && (pts[11].x - 160.0).abs() < 1e-6, "from where it was to where it lands: {pts:?}");
        assert!(pts.windows(2).all(|w| w[1].x >= w[0].x), "along the way: {pts:?}");
    }

    #[test]
    fn states_become_the_pages_of_one_pdf() {
        let mut e = load(DOC).unwrap();
        let pdf = pdf_states(&mut e, &[0, 1]);
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("/Count 2") && text.matches("/Type /Page ").count() == 2);
        assert!(text.contains("/MediaBox [0 0 75 45]"), "100×60 px pages");
    }
}

/// Options for [`render_published`].
#[derive(Clone, Debug, Default)]
pub struct ImageRequest {
    pub state: usize,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub dpr: f64,
    pub dark: bool,
    pub svg: bool,
    /// A vector PDF page (print, reports) instead of PNG/SVG.
    pub pdf: bool,
}

/// Render a published chart (its manifest, and chunks by hash through `chunk`) to PNG or SVG with
/// the CPU reference — the image server behind social cards and email embeds. Returns (content
/// type, bytes).
pub fn render_published(manifest: &[u8], chunk: &dyn Fn(&str) -> Option<Vec<u8>>, req: &ImageRequest) -> Result<(&'static str, Vec<u8>), String> {
    let manifest = datars_bundle::Manifest::from_json(manifest).map_err(|e| e.to_string())?;
    let caps = datars_engine::bundle::capabilities(true, Vec::new());
    let v = manifest.select(&caps).map_err(|e| e.to_string())?.clone();
    let entry: serde_json::Value = serde_json::from_slice(&chunk(&v.entry).ok_or("missing entry chunk")?).map_err(|e| e.to_string())?;
    let mut engine = Engine::new();
    datars_engine::bundle::open_with(&mut engine, &manifest, v.tier, &entry, chunk)?;
    let vp = engine.viewport();
    let (w, h) = (req.width.unwrap_or(vp.width).clamp(50.0, 4000.0), req.height.unwrap_or(vp.height).clamp(50.0, 4000.0));
    let dpr = if req.dpr > 0.0 { req.dpr.clamp(0.25, 4.0) } else { 1.0 };
    engine.resize(w, h, dpr);
    if req.dark {
        engine.set_mode(datars_theme::Mode::Dark);
    }
    let scene = engine.scene_for_state(req.state);
    let list = engine.display_list(&scene);
    Ok(if req.pdf {
        ("application/pdf", datars_render_pdf::to_pdf(&list, engine.fonts()))
    } else if req.svg {
        ("image/svg+xml", datars_render_svg::to_svg(&list, engine.fonts()).into_bytes())
    } else {
        ("image/png", datars_render_cpu::render(&list, engine.fonts(), dpr).to_png())
    })
}
