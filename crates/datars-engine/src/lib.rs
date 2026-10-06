//! `datars-engine` — the engine façade (docs/03-architecture.md, "The host contract").
//!
//! Documents → tables → scenes (per program state) → transitions (motion plans) → frames → display
//! lists. Sans-IO: sources the engine can't read itself become [`Request`]s the host fulfils. The
//! same engine runs in every host (web, iOS, Android, desktop, headless, tests).

mod bounds;
mod drivers;
pub mod session;
pub mod bundle;
mod env;
#[cfg(feature = "sandbox")]
pub mod expand;
mod geo;
mod graph_ops;
mod layout_ops;
pub mod pick;
pub mod points;
pub mod program;
mod resolve;
mod scales;
mod tables;
pub mod textlayer;
mod tiles;

pub use bounds::{node_bounds, quads_overlap, text_quad, text_rect};
pub use resolve::{BoundAction, ExprValue, Expand, Expansion, Origin, OriginRow};
pub use points::PointStats;
pub use tiles::{RangeFetch, TileStats, TileView};
/// Signal values (re-exported so hosts don't depend on datars-expr directly).
pub use datars_expr::Value as SignalValue;

use datars_expr::Value;
use datars_ir::Doc;
use datars_math::Vec2;
use datars_render::DisplayList;
use datars_scene::{KeyPath, Scene};
use datars_theme::{Mode, ResolvedTheme, ThemeSet, TokenValue};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::Arc;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Resolved scenes kept per signal signature (states, hovers, camera positions of a pan); the
/// cache starts over past this. Resolution is deterministic, so this only costs time.
const MAX_CACHED_SCENES: usize = 256;
/// Instances listed one by one in the accessible description, per scene (the rest are summarized
/// by their node's own label).
const MAX_INSTANCE_ITEMS: usize = 200;

/// Themes from the standard library, available to every document by name.
pub const STD_THEMES: &[&str] = &[
    include_str!("../../../packages/std/themes/noir.json"),
    include_str!("../../../packages/std/themes/broadsheet.json"),
    include_str!("../../../packages/std/themes/brand-example.json"),
];

#[derive(Clone, Debug, PartialEq)]
pub struct Diag {
    pub message: String,
}

/// IO the host must perform (the engine never does any, P10).
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// Fetch a data source by URL; hand the bytes to [`Engine::provide`].
    Source { name: String, url: String },
    /// A data slot the host app fills.
    Slot { name: String, slot: String },
    /// A built-in atlas (`countries`, …): the host provides its GeoJSON/TopoJSON bytes.
    Atlas { name: String, atlas: String },
    /// Bytes `offset .. offset + length` of a tiles source's archive (an HTTP `Range` request);
    /// hand them to [`Engine::provide_range`]. Fewer bytes are fine at the end of the file.
    /// Appear as views need tiles: after loading (the header), and as cameras move.
    Range { name: String, url: String, offset: u64, length: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub width: f64,
    pub height: f64,
    pub dpr: f64,
}

impl Viewport {
    pub fn size_class(&self) -> &'static str {
        if self.width < 560.0 {
            "phone"
        } else if self.width < 960.0 {
            "tablet"
        } else {
            "wide"
        }
    }
}

/// Pointer input, already in CSS px relative to the view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pointer {
    Move { x: f64, y: f64 },
    Down { x: f64, y: f64 },
    Up { x: f64, y: f64 },
    /// A touch press released where it went down (after its `Down`): a click that also inspects
    /// what it lands on — touch has no hover, so a tap is how a reader asks what a mark is — with
    /// a finger's reach around thin and small marks. A tap on nothing clears the inspection.
    Tap { x: f64, y: f64 },
    Leave,
    /// Wheel or pinch at (x, y): `delta` > 0 zooms out, < 0 zooms in (CSS `deltaY` units).
    Wheel { x: f64, y: f64, delta: f64 },
}

/// The longest a hover state takes to fade in or out.
const HOVER_SECONDS: f64 = 0.2;

/// What the pointer should look like over the chart ([`Engine::cursor`]): hosts show their
/// platform's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cursor {
    Default,
    /// Something a click does something to: a control, a mark that selects or steps.
    Pointer,
    /// A view that pans…
    Grab,
    /// …while it's being panned (or a slider dragged).
    Grabbing,
    /// A brushable area.
    Crosshair,
}

impl Cursor {
    /// The CSS name.
    pub fn css(self) -> &'static str {
        match self {
            Cursor::Default => "default",
            Cursor::Pointer => "pointer",
            Cursor::Grab => "grab",
            Cursor::Grabbing => "grabbing",
            Cursor::Crosshair => "crosshair",
        }
    }
}

/// A drag in progress (brush or pan): the action, where it started and where it was last.
#[derive(Clone, Debug)]
struct Drag {
    path: KeyPath,
    action: resolve::BoundAction,
    start: Vec2,
    last: Vec2,
    moved: bool,
}

/// An engine-drawn control (a slider): what a host needs to offer it to keyboards and assistive
/// technology as a native control.
#[derive(Clone, Debug, PartialEq)]
pub struct Control {
    /// `slider`: a number between `min` and `max` (`value`, snapped to `step`); `select`: one of
    /// `options`, `current` now.
    pub kind: &'static str,
    pub signal: String,
    pub label: String,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub value: f64,
    /// A select's choices: each value and what it says.
    pub options: Vec<(serde_json::Value, String)>,
    /// A select's value now.
    pub current: serde_json::Value,
    /// Where it is, in root coordinates (CSS px): a host puts its own picker over it.
    pub rect: [f64; 4],
}

/// One item of the accessible description.
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticItem {
    pub role: String,
    pub label: String,
    pub depth: usize,
    /// Bounds in root coordinates (CSS px).
    pub rect: datars_math::Rect,
    /// The node's key path (for `activate`), and whether it has an activate intent — hosts offer
    /// those as buttons, so pointer-only interactions work from keyboards and screen readers.
    pub path: String,
    pub actionable: bool,
}

/// Where a keyed point is on screen this frame (for host overlays).
#[derive(Clone, Debug, PartialEq)]
pub struct AnchorOut {
    pub path: String,
    pub name: String,
    pub x: f64,
    pub y: f64,
}

/// Rows of point tiles a frame prepares at most (sorted into Morton order, run through the
/// template) — a few milliseconds in wasm — before finer levels wait for the next frame.
const FRAME_BUILD_ROWS: usize = 4_000;

/// Where a resolve put its dodge groups (cards): the anchor each took, and those given a band.
#[derive(Clone, Debug, Default)]
struct Placements {
    anchors: BTreeMap<datars_scene::KeyPath, String>,
    banded: std::collections::BTreeSet<datars_scene::KeyPath>,
}

pub struct FrameOutput {
    pub scene: Scene,
    pub display: DisplayList,
    pub anchors: Vec<AnchorOut>,
    /// Is anything still moving? Hosts render on demand.
    pub animating: bool,
    /// When nothing is moving: the host time at which the next frame is due anyway (an autoplay
    /// hold ending, a live refresh). Hosts can sleep until then.
    pub wake_at: Option<f64>,
    /// Tiles this frame wanted but doesn't have yet (drawn from a coarser tile meanwhile, or not at
    /// all): render again after providing the ranges in [`Engine::requests`].
    pub pending_tiles: u32,
}

impl FrameOutput {
    /// The frame's identity (its display list's hash): computed when asked, not every frame — a
    /// display list with a few hundred thousand points costs milliseconds to hash.
    pub fn hash(&self) -> u64 {
        self.display.hash()
    }
}

/// One element explained ([`Engine::explain`]).
#[derive(Clone, Debug, serde::Serialize)]
pub struct Explanation {
    /// The node as drawn (its `inspect` line).
    pub node: String,
    /// Its bounds in root coordinates (CSS px).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds: Option<[f64; 4]>,
    /// Intents bound to it (`click`, `drag`, …).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub intents: Vec<String>,
    #[serde(flatten)]
    pub origin: resolve::Origin,
}

/// A face the theme's font tokens name: its id, names, where it comes from, and which tokens.
struct ThemeFace {
    id: String,
    family: String,
    weight: u16,
    italic: bool,
    url: Option<String>,
    tokens: Vec<String>,
}

/// A face added for this document beyond the built-in ones and its font sources: a theme token's
/// face (a bundle chunk or a fulfilled `font:` request) or a lazily loaded part of one.
struct ExtraFace {
    bytes: Vec<u8>,
    family: String,
    weight: u16,
    italic: bool,
    part: Option<String>,
    /// A document font source's family: a fallback for every text.
    fallback: bool,
}

struct Resolved {
    scene: Scene,
    actions: Vec<(KeyPath, BTreeMap<String, BoundAction>)>,
    /// `tiles` placeholders in `scene`, by key path.
    tiles: Vec<(String, Rc<tiles::Binding>)>,
    /// `lod` instances placeholders in `scene`, by key path.
    points: Vec<(String, Rc<points::Binding>)>,
    /// Recipes' motion defaults in this scene, scoped to their key paths.
    motion: Vec<datars_motion::Rule>,
    /// The elements `hover()` asked about while resolving it.
    hover_asked: Vec<KeyPath>,
}

/// A transition planned ahead ([`Engine::prepare`]): from the settled scene then on screen to a
/// neighbouring state.
struct Prepared {
    /// Its target's display list was handed to the host to warm its renderer (complete, or after
    /// a few rounds of tiles arriving).
    warmed: bool,
    warm_rounds: u8,
    /// Tile bytes the engine had been handed at the last round, when that round still waited for
    /// some (downloading): the next round waits for more to arrive.
    waiting_at: Option<u64>,
    from: Rc<Resolved>,
    from_state: String,
    to_state: String,
    target: Rc<Resolved>,
    plan: datars_motion::Plan,
}

struct Active {
    plan: datars_motion::Plan,
    start: f64,
    duration: f64,
    to: Rc<Resolved>,
    /// Shown in a frame yet. A transition starts at the first frame that draws it, not when it
    /// was planned: planning happens on input (a step, a click) and can take a while, and timed
    /// from the input the first frame would already be that far in — the chart sits still, then
    /// jumps. (A first frame after it would have ended shows it ended.)
    shown: bool,
}

pub struct Engine {
    doc: Doc,
    themes: ThemeSet,
    mode: Mode,
    host_tokens: BTreeMap<String, TokenValue>,
    theme: ResolvedTheme,
    fonts: Rc<datars_text::FontDb>,
    /// Does the font database start from the faces compiled into this build (`bundled-fonts`)?
    /// Runtimes without them get every face from bundles and requests.
    builtin_fonts: bool,
    /// Fonts the document provided (`font` sources), in order: the database is rebuilt from the
    /// built-in faces plus these, so a new document starts clean.
    doc_fonts: Vec<Vec<u8>>,
    /// Faces the theme's font tokens named, by face id (`"Newsreader-600"`): from a bundle's font
    /// chunks or a fulfilled `font:` request, registered under the token's family, weight and
    /// style. Ordered by id so the database doesn't depend on the order fonts arrived in.
    theme_faces: BTreeMap<String, ExtraFace>,
    /// Font bytes by the URL they came from: a reloaded document (the edit loop) or a theme
    /// switch that names the same file gets it at once instead of asking again.
    font_cache: BTreeMap<String, Vec<u8>>,
    #[cfg(feature = "sandbox")]
    expander: Option<expand::SandboxExpander>,
    sources: BTreeMap<String, Arc<datars_data::Table>>,
    geo: geo::GeoStore,
    /// Derived tables that only depend on the data, shared by every resolve until it changes.
    tables_cache: tables::SharedTables,
    requests: Vec<Request>,
    /// Declared signal defaults (lowest precedence).
    defaults: BTreeMap<String, Value>,
    /// Host and interaction overrides (highest precedence). Entering a state clears the overrides
    /// for the signals that state sets, so a story step always shows what it says.
    signals: BTreeMap<String, Value>,
    program: program::Runtime,
    viewport: Viewport,
    cache: BTreeMap<String, Rc<Resolved>>,
    shown: Option<Rc<Resolved>>,
    active: Option<Active>,
    prepared: Vec<Prepared>,
    /// Inside [`Engine::batch`]: fonts and data arriving mark what's stale instead of rebuilding and
    /// re-resolving each time.
    batching: u32,
    batch_fonts: bool,
    batch_stale: bool,
    /// The settled scene whose surroundings idle time has been fetching, and how many of the
    /// looks around it are done ([`Engine::prepare`]).
    buffered: Option<Rc<Resolved>>,
    buffer_looks: usize,
    rules: datars_motion::MotionRules,
    clock: f64,
    /// When the current state finished arriving (for autoplay holds).
    settled_at: Option<f64>,
    /// Autoplay runs while true (hosts pause it for interaction, reduced motion, off-screen).
    playing: bool,
    /// The reader asked for reduced motion: every transition is a short crossfade.
    reduced_motion: bool,
    /// Clock signals (name, rate per second), and the seconds they have run: time accumulates only
    /// while a settled scene reads them, so a transition never makes them jump.
    clocks: Vec<(String, f64)>,
    clock_time: f64,
    clock_last: Option<f64>,
    /// Per state and signals (clocks aside): does the scene read a clock — and if so, where its
    /// cards went when it settled, held while only the clock moves the data under them.
    clock_reads: BTreeMap<String, Option<Placements>>,
    /// Next refresh time per live source.
    live_due: BTreeMap<String, f64>,
    /// Sources whose rows came with the bundle (fetched when it was published), not from the host
    /// since: a live one among them refreshes as soon as the chart runs.
    snapshots: BTreeSet<String>,
    /// Scrub cache: the transition plan from state i to i + 1.
    scrub: Option<(usize, datars_motion::Plan)>,
    drag: Option<Drag>,
    /// The element under the pointer that `hover()` asked about (the innermost), if any.
    hovered: Option<KeyPath>,
    /// What the pointer over the chart should look like, as the last pointer input left it.
    cursor: Cursor,
    /// Session recording (inputs since `start_recording`).
    recording: Option<session::Session>,
    diags: Vec<Diag>,
    /// The diagnostics resolving produced: they describe the data they were resolved against, so
    /// new data (a fulfilled request) drops them and re-resolves.
    resolve_diags: Vec<Diag>,
    /// Playback mode (T1 bundles): scenes resolved at publish time, one per state.
    baked: Option<Vec<Scene>>,
    /// Tile archives and caches (filled from `&self` frame paths, hence the cell).
    tiles: RefCell<tiles::TileState>,
    /// The bindings of `tiles` placeholders, by key path, as last resolved.
    bindings: RefCell<BTreeMap<String, Rc<tiles::Binding>>>,
    /// The bindings of `lod` instances placeholders, by key path, as last resolved.
    point_bindings: RefCell<BTreeMap<String, Rc<points::Binding>>>,
    /// Scales every `lod` node's budget of points (1: as the document says).
    point_scale: f64,
    /// The host's share of the per-frame work budgets (`Engine::set_work_scale`).
    work: f64,
    /// Big instance sets' flattened columns, kept while they don't change ([`datars_render::FlattenCache`]).
    flatten_cache: RefCell<datars_render::FlattenCache>,
    /// Whether the document draws its own narration, once asked (`Engine::draws_narration`).
    narration_drawn: std::cell::Cell<Option<bool>>,
    range_fetch: Option<Box<RangeFetch>>,
}

impl Engine {
    /// An engine whose font database starts with the faces compiled into this build (the default
    /// family with the `bundled-fonts` feature; none without it).
    pub fn new() -> Engine {
        Engine::with_font_base(cfg!(feature = "bundled-fonts"))
    }

    /// An engine with no built-in faces, whatever this build carries: every face comes from a
    /// bundle's font chunks or from font requests the host answers — what the web runtime is.
    /// Tests use it to prove bundles carry every font they need.
    pub fn without_builtin_fonts() -> Engine {
        Engine::with_font_base(false)
    }

    fn with_font_base(builtin_fonts: bool) -> Engine {
        let mut themes = ThemeSet::with_builtins();
        for src in STD_THEMES {
            if let Ok(t) = datars_theme::Theme::from_json(src) {
                themes.add(t);
            }
        }
        let chain = themes.chain("datars/neutral").unwrap_or_default();
        let (theme, _) = datars_theme::resolve(&chain, Mode::Light, &[]);
        let fonts = Rc::new(if builtin_fonts { datars_text::FontDb::builtin() } else { datars_text::FontDb::empty() });
        Engine {
            doc: Doc::default(),
            themes,
            mode: Mode::Light,
            host_tokens: BTreeMap::new(),
            theme: theme.clone(),
            #[cfg(feature = "sandbox")]
            expander: expand::SandboxExpander::new(Rc::new(expand::EngineHost { fonts: std::cell::RefCell::new(fonts.clone()), theme: std::cell::RefCell::new(theme), locale: std::cell::RefCell::new("en".into()) })).ok(),
            fonts,
            builtin_fonts,
            doc_fonts: Vec::new(),
            theme_faces: BTreeMap::new(),
            font_cache: BTreeMap::new(),
            sources: BTreeMap::new(),
            geo: geo::GeoStore::new(),
            tables_cache: Default::default(),
            requests: Vec::new(),
            defaults: BTreeMap::new(),
            signals: BTreeMap::new(),
            program: program::Runtime::new(&datars_ir::Program::default()),
            viewport: Viewport { width: 800.0, height: 480.0, dpr: 1.0 },
            cache: BTreeMap::new(),
            shown: None,
            active: None,
            prepared: Vec::new(),
            batching: 0,
            batch_fonts: false,
            batch_stale: false,
            buffered: None,
            buffer_looks: 0,
            rules: datars_motion::MotionRules::default(),
            clock: 0.0,
            settled_at: None,
            playing: true,
            reduced_motion: false,
            clocks: Vec::new(),
            clock_time: 0.0,
            clock_last: None,
            clock_reads: BTreeMap::new(),
            live_due: BTreeMap::new(),
            snapshots: BTreeSet::new(),
            scrub: None,
            drag: None,
            hovered: None,
            cursor: Cursor::Default,
            recording: None,
            diags: Vec::new(),
            resolve_diags: Vec::new(),
            baked: None,
            tiles: RefCell::new(tiles::TileState::new(&Doc::default())),
            bindings: RefCell::new(BTreeMap::new()),
            point_bindings: RefCell::new(BTreeMap::new()),
            point_scale: 1.0,
            work: 1.0,
            flatten_cache: RefCell::new(datars_render::FlattenCache::default()),
            narration_drawn: std::cell::Cell::new(None),
            range_fetch: None,
        }
    }

    /// Load a document. Returns diagnostics (the engine still runs with what it could understand).
    pub fn load(&mut self, doc: Doc) -> Vec<Diag> {
        self.baked = None;
        self.diags.clear();
        self.resolve_diags.clear();
        self.cache.clear();
        self.shown = None;
        self.active = None;
        self.settled_at = None;
        self.live_due.clear();
        self.snapshots.clear();
        self.scrub = None;
        self.recording = None;
        if !self.doc_fonts.is_empty() || !self.theme_faces.is_empty() {
            self.doc_fonts.clear();
            self.theme_faces.clear();
            self.rebuild_fonts();
        }
        self.viewport.width = doc.size.width;
        self.viewport.height = doc.size.height;
        // Themes: inline definitions, then the chain for `use`.
        for t in &doc.theme.themes {
            match datars_theme::Theme::from_json(&t.to_string()) {
                Ok(th) => self.themes.add(th),
                Err(e) => self.diags.push(Diag { message: format!("theme: {e}") }),
            }
        }
        // Signals: declared defaults.
        self.signals.clear();
        self.defaults.clear();
        self.clocks = doc.signals.iter().filter_map(|(name, d)| d.clock.map(|r| (name.clone(), r))).collect();
        self.clock_time = 0.0;
        self.clock_last = None;
        self.clock_reads.clear();
        for (name, decl) in &doc.signals {
            let v = match decl.ty.as_str() {
                "keyset" => env::encode_keyset(&decl.default.as_array().map(|a| a.iter().map(|x| x.as_str().map(String::from).unwrap_or_else(|| x.to_string())).collect::<Vec<_>>()).unwrap_or_default()),
                _ => resolve::json_to_value(&decl.default),
            };
            self.defaults.insert(name.clone(), v);
        }
        self.program = program::Runtime::new(doc.program.as_ref().unwrap_or(&datars_ir::Program { states: vec![datars_ir::State { name: "main".into(), ..Default::default() }], ..Default::default() }));
        self.rules = if doc.motion.is_null() {
            datars_motion::MotionRules::default()
        } else {
            match serde_json::from_value(doc.motion.clone()) {
                Ok(r) => r,
                Err(e) => {
                    self.diags.push(Diag { message: format!("motion rules: {e}") });
                    datars_motion::MotionRules::default()
                }
            }
        };
        #[cfg(feature = "sandbox")]
        if let Some(ex) = &self.expander {
            for p in &doc.packages {
                if let Some(src) = &p.source {
                    ex.add_module(&p.name, src);
                }
            }
            ex.clear_cache();
            *ex.host.locale.borrow_mut() = doc.locale.clone();
        }
        let (sources, geo_store, requests, diags) = tables::load_sources(&doc);
        self.tiles = RefCell::new(tiles::TileState::new(&doc));
        self.bindings.borrow_mut().clear();
        self.point_bindings.borrow_mut().clear();
        self.sources = sources;
        self.tables_cache.borrow_mut().clear();
        self.geo = geo_store;
        self.requests = requests;
        self.diags.extend(diags);
        self.doc = doc;
        self.narration_drawn.set(None);
        self.retheme();
        self.diags.clone()
    }

    /// A new version of the document, shown by morphing from what is on screen — the edit loop
    /// (`datars dev`): change the data, a colour or a whole recipe and watch it animate there.
    /// The program stays in the state of the same name when the new version has one, and the
    /// host's viewport is kept.
    pub fn reload(&mut self, doc: Doc) -> Vec<Diag> {
        let on_screen = self.current_scene();
        let state = self.program.state_name();
        let (w, h, dpr) = (self.viewport.width, self.viewport.height, self.viewport.dpr);
        let diags = self.load(doc);
        self.viewport.width = w;
        self.viewport.height = h;
        self.viewport.dpr = dpr;
        if let Some(i) = self.state_names().iter().position(|n| *n == state) {
            self.program.goto_index(i);
        }
        if let Some(scene) = on_screen {
            self.shown = Some(Rc::new(Resolved { scene, actions: Vec::new(), tiles: Vec::new(), points: Vec::new(), motion: Vec::new(), hover_asked: Vec::new() }));
            self.transition_to_current(Some(state));
        }
        diags
    }

    pub fn doc(&self) -> &Doc {
        &self.doc
    }

    /// Add a font for this document (a `font` source's bytes, or a bundle's font chunk): its
    /// families become fallbacks for every text, and the scene re-resolves.
    pub fn add_font(&mut self, bytes: &[u8]) -> Result<(), String> {
        datars_text::FontDb::empty().add_font(bytes.to_vec()).map_err(|e| e.to_string())?;
        self.doc_fonts.push(bytes.to_vec());
        self.fonts_changed();
        Ok(())
    }

    /// Add a face a theme font token names (a bundle's font chunk, or the bytes for a `font:`
    /// request), registered as `family` / `weight` / `italic` — the token's names, whatever the
    /// file calls itself. Text whose stack names the family uses it; the scene re-resolves.
    pub fn add_face(&mut self, bytes: &[u8], family: &str, weight: u16, italic: bool) -> Result<(), String> {
        self.add_face_part(bytes, family, weight, italic, None, false)
    }

    /// Add a lazily loaded part of a face (a bundle's script subset, `part` = the script) — or,
    /// with `part: None`, the face itself. `fallback`: the family is a document font source's, so
    /// it becomes a fallback for every text.
    pub fn add_face_part(&mut self, bytes: &[u8], family: &str, weight: u16, italic: bool, part: Option<&str>, fallback: bool) -> Result<(), String> {
        let mut probe = datars_text::FontDb::empty();
        let ids = match part {
            Some(p) => probe.add_face_part(bytes.to_vec(), family, weight, italic, p),
            None => probe.add_face(bytes.to_vec(), family, weight, italic),
        }
        .map_err(|e| e.to_string())?;
        for id in ids {
            self.theme_faces.insert(id, ExtraFace { bytes: bytes.to_vec(), family: family.to_string(), weight, italic, part: part.map(String::from), fallback });
        }
        self.fonts_changed();
        Ok(())
    }

    /// Rebuild the database and re-resolve. A font arriving isn't a change to animate: the text
    /// snaps to its real glyphs (unless a transition was already running, which then continues to
    /// the re-resolved scene).
    fn fonts_changed(&mut self) {
        if self.batching > 0 {
            self.batch_fonts = true;
            self.batch_stale = true;
            return;
        }
        self.rebuild_fonts();
        let was_moving = self.active.is_some();
        self.invalidate();
        if !was_moving {
            if let Some(a) = self.active.take() {
                self.shown = Some(a.to);
            }
        }
    }

    /// Faces the resolved theme's font tokens name, with where each comes from.
    fn theme_font_faces(&self) -> Vec<ThemeFace> {
        let mut out: Vec<ThemeFace> = Vec::new();
        for (token, spec) in &self.theme.fonts {
            let Some(family) = spec.primary() else { continue };
            let id = if spec.italic { format!("{family}-{}-Italic", spec.weight) } else { format!("{family}-{}", spec.weight) };
            match out.iter_mut().find(|f| f.id == id) {
                Some(f) => {
                    f.tokens.push(token.clone());
                    if f.url.is_none() {
                        f.url = spec.source_url();
                    }
                }
                None => out.push(ThemeFace { id, family, weight: spec.weight, italic: spec.italic, url: spec.source_url(), tokens: vec![token.clone()] }),
            }
        }
        out
    }

    /// Font faces the theme names with a source that no face in the database satisfies yet: the
    /// host fetches each URL like a data source and hands the bytes to [`Engine::provide`] under
    /// the request's name (`font:<face id>`). `google:` URLs are for the build step, which
    /// downloads Google Fonts ahead of time and ships them in the bundle (never at runtime).
    fn font_requests(&self) -> Vec<Request> {
        self.theme_font_faces()
            .into_iter()
            .filter(|f| !self.fonts.has_face(&f.id))
            .filter_map(|f| Some(Request::Source { name: format!("font:{}", f.id), url: f.url? }))
            .collect()
    }

    /// Faces for the theme that earlier documents or themes already fetched (the same URL): added
    /// at once, so a reload or a theme switch doesn't flash fallback glyphs.
    fn apply_cached_fonts(&mut self) {
        let mut added = false;
        for ThemeFace { id, family, weight, italic, url, .. } in self.theme_font_faces() {
            if self.fonts.has_face(&id) {
                continue;
            }
            if let Some(bytes) = url.and_then(|u| self.font_cache.get(&u)).cloned() {
                if datars_text::FontDb::empty().add_face(bytes.clone(), &family, weight, italic).is_ok() {
                    self.theme_faces.insert(id, ExtraFace { bytes, family, weight, italic, part: None, fallback: false });
                    added = true;
                }
            }
        }
        if added {
            self.rebuild_fonts();
        }
    }

    /// Characters laid out so far that no loaded face has (drawn as .notdef): a runtime fetches a
    /// bundle's lazily loaded script subsets for them.
    pub fn missing_chars(&self) -> Vec<char> {
        self.fonts.missing_chars()
    }

    /// Diagnostics for font tokens the database can't honour: families with no source (the build
    /// step can't ship them) and sourced faces that never arrived. Text in them falls back.
    fn font_diags(&self) -> Vec<Diag> {
        let mut out = Vec::new();
        for ThemeFace { family, weight, italic, url, tokens, .. } in self.theme_font_faces() {
            if self.fonts.has_family(&family) {
                continue;
            }
            let fallback = self.fonts.face_id(&family, weight, italic).map(|id| self.fonts.faces().into_iter().find(|f| f.id == id).map(|f| f.family).unwrap_or(id));
            let fallback = fallback.map(|f| format!("falling back to {f}")).unwrap_or_else(|| "no fonts are loaded, so its text has no glyphs".into());
            let tokens = tokens.join(", ");
            out.push(Diag {
                message: match url {
                    None => format!("font '{family}' ({tokens}) has no source; {fallback}. Give the token a `src` (a font file or URL) or a `google` family so the build step can ship it"),
                    Some(u) if u.starts_with("google:") => format!("font '{family}' ({tokens}) comes from Google Fonts, which only the build step fetches (`datars bundle`, `publish`, `render`, `dev`); {fallback}"),
                    Some(u) => format!("font '{family}' ({tokens}) is not loaded ({u}); {fallback}"),
                },
            });
        }
        out
    }

    /// The built-in faces plus the document's fonts (each family a fallback for every text) and
    /// the theme's faces (by id, so arrival order never matters).
    fn rebuild_fonts(&mut self) {
        let mut db = if self.builtin_fonts { datars_text::FontDb::builtin() } else { datars_text::FontDb::empty() };
        for bytes in &self.doc_fonts {
            if db.add_font(bytes.clone()).is_ok() {
                for family in datars_text::FontDb::families_in(bytes) {
                    db.add_fallback(&family);
                }
            }
        }
        for f in self.theme_faces.values() {
            let added = match &f.part {
                Some(p) => db.add_face_part(f.bytes.clone(), &f.family, f.weight, f.italic, p),
                None => db.add_face(f.bytes.clone(), &f.family, f.weight, f.italic),
            };
            if added.is_ok() && f.fallback {
                db.add_fallback(&f.family);
            }
        }
        self.fonts = Rc::new(db);
        #[cfg(feature = "sandbox")]
        if let Some(ex) = &self.expander {
            *ex.host.fonts.borrow_mut() = self.fonts.clone();
        }
    }

    /// Describe a recipe (`@datars/std/bar`).
    #[cfg(feature = "sandbox")]
    pub fn describe_recipe(&self, id: &str) -> Result<serde_json::Value, String> {
        self.expander.as_ref().ok_or("no sandbox")?.describe(id)
    }

    /// List the recipes of a module.
    #[cfg(feature = "sandbox")]
    pub fn list_recipes(&self, module: &str) -> Result<Vec<String>, String> {
        self.expander.as_ref().ok_or("no sandbox")?.list(module)
    }

    /// Playback mode: scenes resolved at publish time (a T1 bundle). The document supplies the
    /// program, motion rules, theme and size; its template is not resolved.
    pub fn load_baked(&mut self, doc: Doc, scenes: Vec<Scene>) -> Vec<Diag> {
        let d = self.load(doc);
        self.baked = Some(scenes);
        self.cache.clear();
        d
    }

    pub fn is_baked(&self) -> bool {
        self.baked.is_some()
    }

    /// Scenes for every top-level state (publish time).
    pub fn bake(&mut self) -> Vec<(String, Scene)> {
        let names = self.program.state_names();
        (0..names.len()).map(|i| (names[i].clone(), self.scene_for_state(i))).collect()
    }

    /// The document with every recipe expanded ahead of time (a T2 bundle: no sandbox needed at
    /// runtime). Expansions use the authored size and size class.
    #[cfg(feature = "sandbox")]
    pub fn expanded_doc(&self) -> Result<Doc, String> {
        let ex = self.expander.as_ref().ok_or("no sandbox")?;
        let mut doc = self.doc.clone();
        let cx = serde_json::json!({ "size": [doc.size.width, doc.size.height], "sizeClass": self.viewport.size_class(), "locale": doc.locale });
        let mut tables = BTreeMap::new();
        doc.scene = expand::expand_all(ex, &doc.scene, &cx, &mut tables, 0)?;
        for (k, v) in tables {
            doc.tables.entry(k).or_insert(v);
        }
        doc.packages.clear();
        Ok(doc)
    }

    pub fn diagnostics(&self) -> &[Diag] {
        &self.diags
    }

    /// IO the host still has to perform: sources, slots and atlases from loading, and byte ranges of
    /// tile archives as views need them.
    pub fn requests(&self) -> Vec<Request> {
        let mut r = self.requests.clone();
        r.extend(self.tiles.borrow().requests());
        r.extend(self.font_requests());
        r
    }

    /// Hand over bytes for a [`Request::Range`] of a tiles source. Tiles waiting on them decode
    /// now; the next frame draws them.
    pub fn provide_range(&mut self, source: &str, offset: u64, bytes: &[u8]) -> Result<(), String> {
        if self.recording.is_some() {
            self.rec(session::Input::Range { source: source.to_string(), offset, bytes: session::base64(bytes) });
        }
        self.tiles.borrow_mut().provide(source, offset, bytes)
    }

    /// Answer tile ranges synchronously through `fetch` instead of [`Request::Range`]s: headless
    /// renders, tests, apps with archives on disk. The function does the IO, not the engine.
    pub fn set_range_fetch(&mut self, fetch: Box<RangeFetch>) {
        self.range_fetch = Some(fetch);
    }

    /// Tile counters: requests, bytes, tiles decoded and built, and what the last frame drew.
    /// Instances of big sets flattened afresh since creation (not kept from an earlier frame):
    /// per frame, what a transition rebuilds of its big instance sets. See `datars profile`.
    pub fn instances_converted(&self) -> u64 {
        self.flatten_cache.borrow().converted()
    }

    pub fn tile_stats(&self) -> TileStats {
        self.tiles.borrow().stats
    }

    /// Point pyramid counters (`instances` with `lod`): rows, what the last frame drew, tiles
    /// built, pyramids indexed.
    pub fn point_stats(&self) -> PointStats {
        self.tiles.borrow().points.stats
    }

    /// Draw every `lod` node with `scale` × its budget of points (1 restores it): the publish
    /// compiler draws a chart's poster — a placeholder shown until the runtime takes over — from a
    /// thinner sample than the live view.
    pub fn set_point_scale(&mut self, scale: f64) {
        self.point_scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    }

    /// The point pyramids of the document's `lod` nodes over data sources, as PMTiles archives by
    /// source name — what the publish compiler ships instead of the rows, so a reader downloads
    /// the tiles in view rather than the table (and nothing is generated on the device). Every
    /// state is resolved to find the nodes. Nodes over the same source share its archive (it keeps
    /// every column); a source two nodes index differently (other positions or budgets) isn't
    /// offered: one archive can't serve both.
    pub fn point_archives(&mut self) -> Vec<(String, Result<Vec<u8>, String>)> {
        for i in 0..self.state_names().len() {
            self.resolve_state(i);
        }
        let bindings: Vec<Rc<points::Binding>> = self.point_bindings.borrow().values().cloned().collect();
        let mut by_source: BTreeMap<String, Vec<Rc<points::Binding>>> = BTreeMap::new();
        for b in bindings {
            if matches!(b.origin, points::Origin::Table(_)) && self.doc.data.contains_key(&b.from) {
                by_source.entry(b.from.clone()).or_default().push(b);
            }
        }
        let env = tiles::BuildEnv { doc: &self.doc, theme: &self.theme, fonts: &self.fonts, geo: &self.geo, sources: &self.sources, size_class: self.viewport.size_class() };
        let mut out = Vec::new();
        for (source, bs) in by_source {
            let keys: std::collections::BTreeSet<u64> = bs.iter().map(|b| points::build_key(&b.spec, b.budget)).collect();
            if keys.len() > 1 {
                out.push((source, Err("indexed differently by two `lod` nodes".to_string())));
                continue;
            }
            let points::Origin::Table(t) = &bs[0].origin else { continue };
            let p = points::fill::pyramid_of(&env, &mut self.tiles.borrow_mut().points, &bs[0], t);
            out.push((source, points::archive(&p)));
        }
        out
    }

    /// Read a tiles source from another archive from now on (`datars dev` swaps in an automatic
    /// basemap rebuilt as its data arrives). Tiles already read are dropped; the next frame asks
    /// the new archive (a host re-renders, as after any provided range).
    pub fn set_tiles_url(&mut self, source: &str, url: &str) -> Result<(), String> {
        self.tiles.borrow_mut().reset(source, url)
    }

    /// Where the document's views look at its tile sources: every program state settled, and
    /// `samples` frames of each flight between states (consecutive ones, and along the program's
    /// edges) — the camera-aware extent of the document, which an automatic basemap is cut to
    /// (`datars-build`). Nothing is fetched or drawn: only cameras are evaluated.
    pub fn tile_views(&mut self, samples: usize) -> Vec<TileView> {
        let names = self.program.state_names();
        let mut out = Vec::new();
        for (i, name) in names.iter().enumerate() {
            let r = self.resolve_state(i);
            self.collect_views(&r.scene, name, &r.actions, &mut out);
        }
        let mut pairs: Vec<(usize, usize)> = (1..names.len()).map(|i| (i - 1, i)).collect();
        let edges = self.doc.program.as_ref().map(|p| p.edges.clone()).unwrap_or_default();
        for e in edges {
            let (Some(a), Some(b)) = (names.iter().position(|n| *n == e.from), names.iter().position(|n| *n == e.to)) else { continue };
            if a != b && !pairs.contains(&(a, b)) && !pairs.contains(&(b, a)) {
                pairs.push((a, b));
            }
        }
        for (a, b) in pairs {
            let (ra, rb) = (self.resolve_state(a), self.resolve_state(b));
            let cx = datars_motion::PlanCx { theme: self.theme.clone(), from_state: names.get(a).cloned(), to_state: names.get(b).cloned(), event_point: None };
            let plan = datars_motion::plan(&ra.scene, &rb.scene, &self.rules_between(&ra.motion, &rb.motion), &cx);
            for k in 1..=samples {
                let frame = self.planned_at(&plan, k as f64 / (samples + 1) as f64);
                self.collect_views(&frame, "", &[], &mut out);
            }
        }
        out
    }

    /// Where the views on screen now look (the host's viewport, and wherever a reader has panned
    /// an explorable map): what `datars dev` fetches data for next.
    pub fn tile_views_now(&mut self) -> Vec<TileView> {
        let Some(scene) = self.current_scene() else { return Vec::new() };
        let state = self.program.state_name();
        let actions = self.shown.as_ref().map(|r| r.actions.clone()).unwrap_or_default();
        let mut out = Vec::new();
        self.collect_views(&scene, &state, &actions, &mut out);
        out
    }

    fn collect_views(&self, scene: &Scene, state: &str, actions: &[(KeyPath, BTreeMap<String, BoundAction>)], out: &mut Vec<TileView>) {
        let explore: Vec<String> = actions.iter().filter(|(_, a)| a.values().any(|x| matches!(x, BoundAction::Explore { .. }))).map(|(p, _)| p.to_string()).collect();
        for s in tiles::demand::extents(&self.bindings.borrow(), scene) {
            let explores = explore.iter().any(|p| s.path.strip_prefix(p.as_str()).is_some_and(|rest| rest.is_empty() || rest.starts_with('/')));
            out.push(TileView::from_seen(&s, state, explores));
        }
    }

    /// A bundle's own rows for `source`, fetched when it was published: provided like any, and a
    /// live source's due at once — the reader should see today's figures, not the publisher's.
    pub fn provide_snapshot(&mut self, source: &str, bytes: &[u8]) -> Result<(), String> {
        self.provide(source, bytes)?;
        self.snapshots.insert(source.to_string());
        Ok(())
    }

    /// Hand over bytes for a source (CSV or JSON), after a [`Request`] or for a data slot. Geo
    /// sources (atlases, `.geojson`/`.topojson` URLs) accept GeoJSON or TopoJSON.
    pub fn provide(&mut self, source: &str, bytes: &[u8]) -> Result<(), String> {
        self.snapshots.remove(source);
        if self.recording.is_some() {
            self.rec(session::Input::Provide { source: source.to_string(), bytes: session::base64(bytes) });
        }
        if let Some(id) = source.strip_prefix("font:") {
            // A face a theme font token named (a `font:<face id>` request).
            let ThemeFace { family, weight, italic, url, .. } = self.theme_font_faces().into_iter().find(|f| f.id == id).ok_or_else(|| format!("no font token names the face `{id}`"))?;
            self.add_face(bytes, &family, weight, italic).map_err(|e| format!("font `{id}`: {e}"))?;
            if let Some(u) = url {
                self.font_cache.insert(u, bytes.to_vec());
            }
            return Ok(());
        }
        let decl = self.doc.data.get(source).ok_or_else(|| format!("unknown source `{source}`"))?;
        if matches!(decl.from, datars_ir::SourceKind::Font(_)) {
            self.add_font(bytes).map_err(|e| format!("font `{source}`: {e}"))?;
            self.requests.retain(|r| !matches!(r, Request::Source { name, .. } if name == source));
            return Ok(());
        }
        if matches!(decl.from, datars_ir::SourceKind::Tiles(_)) {
            // A whole archive at once (bundled in an app): ranges are then read from memory.
            return self.tiles.borrow_mut().provide_whole(source, bytes.to_vec());
        }
        let is_geo = matches!(&decl.from, datars_ir::SourceKind::Atlas(_)) || matches!(&decl.from, datars_ir::SourceKind::Url(u) if u.ends_with(".geojson") || u.ends_with(".topojson"));
        if is_geo {
            let (g, t) = geo::load(source, bytes, decl.id.as_deref())?;
            self.geo.insert(source.to_string(), Arc::new(g));
            self.sources.insert(source.to_string(), Arc::new(t));
            self.requests.retain(|r| !matches!(r, Request::Atlas { name, .. } | Request::Source { name, .. } if name == source));
            self.invalidate();
            return Ok(());
        }
        let t = if matches!(decl.from, datars_ir::SourceKind::Slot(_)) {
            // Wrong data keeps what was showing (the sample, or the last good data) and says why:
            // the columns first (a clearer message than a missing key column), then the keys.
            let raw = tables::parse_bytes(source, bytes, &datars_ir::Source { key: Vec::new(), ..decl.clone() })?;
            tables::check_slot(source, &raw, decl)?;
            tables::parse_bytes(source, bytes, decl)?
        } else {
            tables::parse_bytes(source, bytes, decl)?
        };
        self.sources.insert(source.to_string(), Arc::new(t));
        self.requests.retain(|r| !matches!(r, Request::Source { name, .. } | Request::Slot { name, .. } if name == source));
        self.invalidate();
        Ok(())
    }

    fn retheme(&mut self) {
        let before = (self.theme.numbers.clone(), self.theme.fonts.clone(), self.theme.texts.clone());
        self.retheme_inner();
        // Colours late-bind, but sizes, radii, strokes and fonts are read when a scene resolves:
        // when those change (a host's brand, a mode that resizes), resolved scenes are stale, so
        // re-resolve and move there like any other change.
        if (&self.theme.numbers, &self.theme.fonts, &self.theme.texts) != (&before.0, &before.1, &before.2) {
            self.cache.clear();
            self.scrub = None;
            if self.shown.is_some() || self.active.is_some() {
                self.transition_to_current(None);
            }
        }
    }

    fn retheme_inner(&mut self) {
        let name = if self.themes.themes.contains_key(&self.doc.theme.use_) { self.doc.theme.use_.clone() } else { "datars/neutral".into() };
        let chain = self.themes.chain(&name).unwrap_or_default();
        let doc_tokens: BTreeMap<String, TokenValue> = self.doc.theme.tokens.iter().filter_map(|(k, v)| TokenValue::parse(v).ok().map(|t| (k.clone(), t))).collect();
        let (theme, diags) = datars_theme::resolve(&chain, self.mode, &[&doc_tokens, &self.host_tokens]);
        for d in diags {
            self.diags.push(Diag { message: format!("theme token `{}`: {}", d.token, d.message) });
        }
        self.theme = theme;
        self.flatten_cache.borrow_mut().clear();
        self.apply_cached_fonts();
        // Tile nodes resolved theme numbers (sizes); inks stay late-bound.
        self.tiles.borrow_mut().clear_built();
        #[cfg(feature = "sandbox")]
        if let Some(ex) = &self.expander {
            *ex.host.theme.borrow_mut() = self.theme.clone();
        }
    }

    /// Switch light / dark / high contrast. Scenes keep their inks, so nothing re-resolves.
    pub fn set_mode(&mut self, mode: Mode) {
        self.rec(session::Input::Mode { mode: mode.name().to_string() });
        self.mode = mode;
        self.retheme();
    }

    /// A host app's token overrides (brand colours, fonts), honouring the theme's locks.
    pub fn set_host_tokens(&mut self, tokens: BTreeMap<String, serde_json::Value>) {
        self.rec(session::Input::Tokens { tokens: tokens.clone() });
        self.host_tokens = tokens.iter().filter_map(|(k, v)| TokenValue::parse(v).ok().map(|t| (k.clone(), t))).collect();
        self.retheme();
    }

    pub fn theme(&self) -> &ResolvedTheme {
        &self.theme
    }

    pub fn resize(&mut self, width: f64, height: f64, dpr: f64) {
        self.rec(session::Input::Resize { width, height, dpr });
        if (width, height) != (self.viewport.width, self.viewport.height) {
            self.viewport = Viewport { width, height, dpr };
            self.transition_to_current(None);
        } else {
            self.viewport.dpr = dpr;
        }
    }

    pub fn viewport(&self) -> Viewport {
        self.viewport
    }

    pub fn set_signal(&mut self, name: &str, v: Value) {
        self.rec(session::Input::Signal { name: name.to_string(), value: resolve::value_to_json(&v) });
        self.set_signal_inner(name, v);
    }

    /// Set a signal from JSON, as hosts and editors hold values: a number, string or boolean; an
    /// array of keys (a keyset — a selection); `{ "lo": a, "hi": b }` (a range — sets `name.lo`,
    /// `name.hi` and `name.active`); `null` clears (an empty keyset / inactive range).
    pub fn set_signal_json(&mut self, name: &str, v: &serde_json::Value) {
        use serde_json::Value as J;
        match v {
            J::Number(n) => self.set_signal(name, Value::Num(n.as_f64().unwrap_or(f64::NAN))),
            J::Bool(b) => self.set_signal(name, Value::Bool(*b)),
            J::String(s) => self.set_signal(name, Value::Str(Arc::from(s.as_str()))),
            J::Array(a) => {
                let keys: Vec<String> = a.iter().map(|k| k.as_str().map(String::from).unwrap_or_else(|| k.to_string())).collect();
                self.set_signal(name, env::encode_keyset(&keys));
            }
            J::Object(o) if o.contains_key("lo") || o.contains_key("hi") => {
                let num = |k: &str| o.get(k).and_then(|x| x.as_f64()).unwrap_or(f64::NAN);
                self.set_signals(vec![(format!("{name}.lo"), Value::Num(num("lo"))), (format!("{name}.hi"), Value::Num(num("hi"))), (format!("{name}.active"), Value::Bool(true))]);
            }
            J::Null => {
                self.set_signals(vec![(name.to_string(), env::encode_keyset(&[])), (format!("{name}.active"), Value::Bool(false))]);
            }
            other => self.set_signal(name, Value::Str(Arc::from(other.to_string().as_str()))),
        }
    }

    fn set_signal_inner(&mut self, name: &str, v: Value) {
        self.signals.insert(name.to_string(), v);
        self.transition_to_current(None);
    }

    /// Drop every cached resolution (profiling cold resolves; tests).
    pub fn clear_caches(&mut self) {
        self.prepared.clear();
        self.buffered = None;
        self.cache.clear();
        self.scrub = None;
    }

    /// Add diagnostics found outside the engine (document migration notes, ignored fields).
    pub fn add_diagnostics(&mut self, notes: impl IntoIterator<Item = String>) {
        for message in notes {
            let d = Diag { message };
            if !self.diags.contains(&d) {
                self.diags.push(d);
            }
        }
    }

    /// The effective value of a signal (defaults < program state < overrides).
    pub fn signal(&self, name: &str) -> Option<Value> {
        self.all_signals().get(name).cloned()
    }

    // ---- program ----------------------------------------------------------------------------

    pub fn state_names(&self) -> Vec<String> {
        self.program.state_names()
    }

    pub fn state(&self) -> String {
        self.program.path()
    }

    pub fn state_index(&self) -> usize {
        self.program.index()
    }

    /// `next`, `prev`, `goto:<name>`, `back`, or a custom event.
    pub fn event(&mut self, ev: &str) -> bool {
        self.rec(session::Input::Event { name: ev.to_string() });
        self.event_inner(ev)
    }

    fn event_inner(&mut self, ev: &str) -> bool {
        let from = self.program.state_name();
        let changed = self.program.event(ev);
        if changed {
            self.clear_state_overrides();
            self.settled_at = None;
            self.transition_to_current(Some(from));
        }
        changed
    }

    pub fn goto(&mut self, index: usize) -> bool {
        self.rec(session::Input::Goto { index });
        let from = self.program.state_name();
        let changed = self.program.goto_index(index);
        if changed {
            self.clear_state_overrides();
            self.settled_at = None;
            self.transition_to_current(Some(from));
        }
        changed
    }

    pub fn narration(&self) -> Option<datars_ir::Narration> {
        self.program.state().and_then(|s| s.narration.clone())
    }

    /// Whether the chart draws its narration itself (a card reading `narration.text`): hosts then
    /// don't add a caption of their own.
    pub fn draws_narration(&self) -> bool {
        // Cards read the narration signals, or carry each step's text themselves (imported stories).
        // Found once per document (the whole scene template as text: hosts ask on every step).
        if let Some(v) = self.narration_drawn.get() {
            return v;
        }
        let scene = serde_json::to_string(&self.doc.scene).unwrap_or_default();
        let v = scene.contains("narration.text") || scene.contains("narration.title") || scene.contains("@datars/std/card");
        self.narration_drawn.set(Some(v));
        v
    }

    // ---- resolving ---------------------------------------------------------------------------

    fn clear_state_overrides(&mut self) {
        for k in self.program.signals().keys() {
            self.signals.remove(k);
        }
        // Where the reader panned and zoomed an explorable view belongs to the state they left:
        // the next state's camera flies to its own fit.
        let prefixes: Vec<String> = self
            .shown
            .iter()
            .flat_map(|r| r.actions.iter())
            .flat_map(|(_, a)| a.values())
            .filter_map(|a| match a {
                BoundAction::Explore { prefix, .. } => Some(prefix.clone()),
                _ => None,
            })
            .collect();
        for p in prefixes {
            for k in ["x", "y", "zoom"] {
                self.signals.remove(&format!("{p}.{k}"));
            }
        }
    }

    fn all_signals(&self) -> BTreeMap<String, Value> {
        let mut s = self.defaults.clone();
        s.extend(self.program.signals());
        s.extend(self.signals.iter().map(|(k, v)| (k.clone(), v.clone())));
        s.insert("viewport.w".into(), Value::Num(self.viewport.width));
        s.insert("viewport.h".into(), Value::Num(self.viewport.height));
        s.insert("sizeClass".into(), Value::Str(Arc::from(self.viewport.size_class())));
        for (name, rate) in &self.clocks {
            let base = match s.get(name) {
                Some(Value::Num(v)) => *v,
                _ => 0.0,
            };
            s.insert(name.clone(), Value::Num(base + rate * self.clock_time));
        }
        s
    }

    /// Does the current state's scene read a clock? Probed once per state and signals: resolve
    /// with the clocks a second on, and compare. When it does, the placements of the at-rest
    /// resolve are kept for the frames the clock drives.
    fn reads_clock(&mut self) -> bool {
        self.clock_pins().is_some()
    }

    fn clock_pins(&mut self) -> Option<Placements> {
        if self.clocks.is_empty() || self.baked.is_some() {
            return None;
        }
        let saved = self.clock_time;
        self.clock_time = 0.0;
        let key = self.signature(&self.all_signals());
        if let Some(r) = self.clock_reads.get(&key) {
            self.clock_time = saved;
            return r.clone();
        }
        let (a, _, placements) = self.resolve_pinned(&self.all_signals(), false, None);
        self.clock_time = 1.0;
        let b = self.resolve_pinned(&self.all_signals(), false, Some(&placements)).0.scene;
        self.clock_time = saved;
        let reads = (a.scene != b).then_some(placements);
        self.clock_reads.insert(key, reads.clone());
        reads
    }

    fn signature(&self, signals: &BTreeMap<String, Value>) -> String {
        let hovered = self.hovered.as_ref().map(|p| p.to_string()).unwrap_or_default();
        format!("{}|{}x{}|{:?}|{hovered}", self.program.path(), self.viewport.width, self.viewport.height, signals)
    }

    fn resolve_now(&mut self) -> Rc<Resolved> {
        if let Some(b) = &self.baked {
            let i = self.program.index().min(b.len().saturating_sub(1));
            let scene = b.get(i).cloned().unwrap_or_else(|| Scene::new(self.viewport.width, self.viewport.height, datars_scene::Node::group(datars_scene::Key::name("empty"), vec![])));
            return Rc::new(Resolved { scene, actions: Vec::new(), tiles: Vec::new(), points: Vec::new(), motion: Vec::new(), hover_asked: Vec::new() });
        }
        let signals = self.all_signals();
        let sig = self.signature(&signals);
        if let Some(r) = self.cache.get(&sig).cloned() {
            self.register_tiles(&r);
            return r;
        }
        // A running clock gives every frame its own signature (those scenes aren't worth keeping)
        // and keeps the cards where the state first put them.
        let pins = if self.clock_time != 0.0 { self.clock_pins() } else { None };
        let (resolved, _, _) = self.resolve_pinned(&signals, false, pins.as_ref());
        let resolved = Rc::new(resolved);
        if self.clock_time == 0.0 {
            // Exploring gives every camera position its own signature too: keep the cache bounded.
            if self.cache.len() >= MAX_CACHED_SCENES {
                self.cache.clear();
            }
            self.cache.insert(sig, resolved.clone());
        }
        self.register_tiles(&resolved);
        resolved
    }

    /// Resolve the scene for `signals` (no cache), recording every node's origin if asked.
    fn resolve_fresh(&mut self, signals: &BTreeMap<String, Value>, record: bool) -> (Resolved, Vec<resolve::Origin>) {
        let (r, origins, _) = self.resolve_pinned(signals, record, None);
        (r, origins)
    }

    /// [`Engine::resolve_fresh`], with cards placed as `pins` say (else where they fit best), and
    /// where they went.
    fn resolve_pinned(&mut self, signals: &BTreeMap<String, Value>, record: bool, pins: Option<&Placements>) -> (Resolved, Vec<resolve::Origin>, Placements) {
        let size_class = self.viewport.size_class();
        #[cfg(feature = "sandbox")]
        let expander: Option<&dyn Expand> = self.expander.as_ref().map(|e| e as &dyn Expand);
        #[cfg(not(feature = "sandbox"))]
        let expander: Option<&dyn Expand> = None;
        let store = tables::TableStore::new(self.sources.clone(), self.doc.tables.clone(), self.tables_cache.clone());
        let mut r = resolve::Resolver::new(&self.doc, signals, &self.theme, &self.fonts, expander, size_class, &self.geo, store);
        r.hovered = self.hovered.clone();
        if record {
            r.record_origins();
        }
        if let Some(p) = pins {
            r.dodge_pins = p.anchors.clone();
            r.banded = p.banded.clone();
        }
        let mut root = r.resolve_root(self.viewport.width, self.viewport.height);
        // A card (a `dodge` group) that found no free spot: resolve once more with it in a band,
        // so the chart makes room instead of being covered.
        let crowded = std::mem::take(&mut *r.crowded.borrow_mut());
        if !crowded.is_empty() {
            let store = tables::TableStore::new(self.sources.clone(), self.doc.tables.clone(), self.tables_cache.clone());
            let mut again = resolve::Resolver::new(&self.doc, signals, &self.theme, &self.fonts, expander, size_class, &self.geo, store);
            again.hovered = self.hovered.clone();
            if record {
                again.record_origins();
            }
            again.banded = crowded.into_iter().collect();
            again.dodge_pins = r.dodge_pins.clone();
            root = again.resolve_root(self.viewport.width, self.viewport.height);
            r = again;
        }
        let mut scene = Scene::new(self.viewport.width, self.viewport.height, root);
        scene.background = datars_theme::Ink::token("paper");
        // Font diagnostics describe the faces loaded so far, so like data diagnostics they're
        // dropped when a font arrives (invalidate) and recomputed on the next resolve.
        for d in r.diags.take().into_iter().chain(self.font_diags()) {
            if !self.diags.contains(&d) {
                self.resolve_diags.push(d.clone());
                self.diags.push(d);
            }
        }
        let origins = r.origins.take().map(|o| o.into_inner()).unwrap_or_default();
        let placements = Placements { anchors: r.dodge_chosen.take(), banded: r.banded.clone() };
        (Resolved { scene, actions: r.actions.take(), tiles: r.tiles.take(), points: r.points.take(), motion: r.motion.take(), hover_asked: r.hover_asked.take() }, origins, placements)
    }

    /// Why an element exists and looks the way it does (`datars explain`): the recipes that
    /// expanded into it, the template, the data row, every expression's value, the node as drawn
    /// and the intents bound to it — for the current state. `query` is a key path as `inspect` and
    /// semantics spell it (`("chart",)/("body",)/("SE",)`), or its end (`("SE",)`); every match is
    /// returned, in scene order. Baked (T1) scenes have no templates to explain.
    pub fn explain(&mut self, query: &str) -> Vec<Explanation> {
        let signals = self.all_signals();
        let (resolved, origins) = self.resolve_fresh(&signals, true);
        let q = query.trim().trim_matches('/');
        let hit = |p: &str| p == q || p.ends_with(&format!("/{q}"));
        // Layout measures children by resolving them before the final pass: the last origin
        // recorded for a path is the drawn node's, and measuring-only paths aren't in the scene.
        let mut last: BTreeMap<String, usize> = BTreeMap::new();
        for (i, o) in origins.iter().enumerate() {
            if hit(&o.path) {
                last.insert(o.path.clone(), i);
            }
        }
        let keep: std::collections::BTreeSet<usize> = last.into_values().collect();
        origins
            .into_iter()
            .enumerate()
            .filter(|(i, _)| keep.contains(i))
            .filter_map(|(_, origin)| {
                let node = pick::node_at_str(&resolved.scene, &origin.path)?;
                Some((origin, node))
            })
            .map(|(origin, node)| {
                let node = Some(node);
                let intents = resolved.actions.iter().find(|(p, _)| p.to_string() == origin.path).map(|(_, a)| a.keys().cloned().collect()).unwrap_or_default();
                Explanation {
                    node: node.map(|(n, _)| n.snapshot_line()).unwrap_or_default(),
                    // `node_at` includes the node's own transform, which its bounds apply again.
                    bounds: node.and_then(|(n, xf)| n.common.transform.inverse().map(|own| bounds::drawn_bounds(n, xf.mul(own)))).filter(|r| !r.is_empty()).map(|r| [r.x, r.y, r.w, r.h]),
                    intents,
                    origin,
                }
            })
            .collect()
    }

    /// The placeholders of the scene about to be filled are this resolution's.
    fn register_tiles(&self, r: &Resolved) {
        if !r.tiles.is_empty() {
            let mut b = self.bindings.borrow_mut();
            for (path, binding) in &r.tiles {
                b.insert(path.clone(), binding.clone());
            }
        }
        if !r.points.is_empty() {
            let mut b = self.point_bindings.borrow_mut();
            for (path, binding) in &r.points {
                b.insert(path.clone(), binding.clone());
            }
        }
    }

    /// Fill `scene`'s `tiles` placeholders from its cameras (see `tiles`). `draw: false` only looks
    /// the tiles up, requesting what's missing (prefetch).
    fn fill_tiles(&self, scene: &mut Scene, draw: bool) -> tiles::FillReport {
        self.fill_tiles_with(scene, draw, tiles::LabelMode::Draw, None)
    }

    /// A frame's fill: point tiles are prepared within [`FRAME_BUILD_ROWS`], so a zoom step or a
    /// flight into new detail spreads its work over frames instead of stalling one.
    fn fill_frame(&self, scene: &mut Scene) -> tiles::FillReport {
        self.fill_tiles_with(scene, true, tiles::LabelMode::Draw, Some(self.frame_rows()))
    }

    /// Fill the tiles of a frame `t` (0…1) into `plan` (`duration` seconds long). Map labels fade:
    /// placement is sampled at a few moments of the last 0.3 s *of the same plan*, and a label's
    /// opacity is the share of them (and now) in which it shows. Near the end every label settles
    /// to what the final scene shows, so the last frame meets the settled one without a pop.
    /// `budget`: live frames prepare point tiles a budget at a time ([`FRAME_BUILD_ROWS`]);
    /// offline frames (tests, film, video) build everything, so they're a function of `t` alone.
    fn fill_tiles_in_flight(&self, plan: &datars_motion::Plan, t: f64, duration: f64, scene: &mut Scene, budget: Option<usize>) -> tiles::FillReport {
        const SAMPLES: u32 = 6;
        const WINDOW_S: f64 = 0.3;
        if self.bindings.borrow().is_empty() || duration <= 0.0 || t <= 0.0 || t >= 1.0 {
            return self.fill_tiles_with(scene, true, tiles::LabelMode::Draw, budget);
        }
        let dt = (WINDOW_S / duration).min(0.5);
        let mut history: BTreeMap<tiles::LabelId, u32> = BTreeMap::new();
        for k in 1..=SAMPLES {
            let tk = (t - dt * k as f64 / SAMPLES as f64).max(0.0);
            let mut past = plan.at(tk, &TextShaper(&self.fonts));
            let placed = RefCell::new(std::collections::BTreeSet::new());
            self.fill_tiles_with(&mut past, true, tiles::LabelMode::Collect(&placed), budget);
            for id in placed.into_inner() {
                *history.entry(id).or_insert(0) += 1;
            }
        }
        let settle = ((t - (1.0 - dt)) / dt).clamp(0.0, 1.0);
        self.fill_tiles_with(scene, true, tiles::LabelMode::Fade { history: &history, samples: SAMPLES, settle }, budget)
    }

    fn fill_tiles_with(&self, scene: &mut Scene, draw: bool, labels: tiles::LabelMode, build_budget: Option<usize>) -> tiles::FillReport {
        self.fill_tiles_look(scene, draw, labels, build_budget, None)
    }

    fn fill_tiles_look(&self, scene: &mut Scene, draw: bool, labels: tiles::LabelMode, build_budget: Option<usize>, look: Option<(f64, Vec2)>) -> tiles::FillReport {
        let bindings = self.bindings.borrow();
        let points = self.point_bindings.borrow();
        if bindings.is_empty() && points.is_empty() {
            return tiles::FillReport::default();
        }
        let cx = tiles::FillCx {
            env: tiles::BuildEnv { doc: &self.doc, theme: &self.theme, fonts: &self.fonts, geo: &self.geo, sources: &self.sources, size_class: self.viewport.size_class() },
            state: &self.tiles,
            bindings: &bindings,
            points: &points,
            point_scale: self.point_scale,
            work: self.work,
            fetch: self.range_fetch.as_deref(),
            now: build_budget.map(|_| self.clock),
            labels,
            build_budget,
            look,
        };
        tiles::fill_scene(&cx, scene, draw)
    }

    /// Tile diagnostics (from `&self` frame paths) into the engine's list.
    fn absorb_tile_diags(&mut self) {
        let msgs = std::mem::take(&mut self.tiles.borrow_mut().diags);
        for m in msgs {
            let d = Diag { message: m };
            if !self.diags.contains(&d) {
                self.diags.push(d);
            }
        }
    }

    /// Request the tiles a transition will pass through, before it gets there (hosts fetch while
    /// the camera flies). Hosts with a synchronous range reader fetch per frame instead.
    fn prefetch(&self, plan: &datars_motion::Plan) {
        if self.range_fetch.is_some() || (self.bindings.borrow().is_empty() && self.point_bindings.borrow().is_empty()) {
            return;
        }
        for i in 1..=8 {
            let mut s = plan.at(i as f64 / 8.0, &TextShaper(&self.fonts));
            self.fill_tiles(&mut s, false);
        }
    }

    /// Run `f` with the engine taking in several things at once (a bundle opening: its document,
    /// data and fonts): the fonts are rebuilt and the scene resolved once at the end — or at the
    /// first frame, if nothing was showing yet — instead of after each font and each source.
    pub fn batch<T>(&mut self, f: impl FnOnce(&mut Engine) -> T) -> T {
        self.batching += 1;
        let r = f(self);
        self.batching -= 1;
        if self.batching == 0 {
            if std::mem::take(&mut self.batch_fonts) {
                self.rebuild_fonts();
            }
            if std::mem::take(&mut self.batch_stale) {
                self.cache.clear();
                // Showing something already: re-resolve now, without animating what only arrived.
                if self.shown.is_some() || self.active.is_some() {
                    let was_moving = self.active.is_some();
                    self.invalidate();
                    if !was_moving {
                        if let Some(a) = self.active.take() {
                            self.shown = Some(a.to);
                        }
                    }
                }
            }
        }
        r
    }

    fn invalidate(&mut self) {
        if self.batching > 0 {
            self.cache.clear();
            self.batch_stale = true;
            return;
        }
        self.cache.clear();
        self.scrub = None;
        let stale = std::mem::take(&mut self.resolve_diags);
        self.diags.retain(|d| !stale.contains(d));
        self.transition_to_current(None);
    }

    // ---- session recording and replay ---------------------------------------------------------

    fn rec(&mut self, i: session::Input) {
        if let Some(s) = &mut self.recording {
            s.inputs.push(i);
        }
    }

    /// Start recording host inputs (call right after `load`, before providing data, for a complete
    /// session). Signals already overridden and the current state are recorded as inputs.
    pub fn start_recording(&mut self) {
        let mut s = session::Session { datars_session: 1, doc: serde_json::to_value(&self.doc).unwrap_or_default(), inputs: Vec::new() };
        s.inputs.push(session::Input::Resize { width: self.viewport.width, height: self.viewport.height, dpr: self.viewport.dpr });
        for (k, v) in &self.signals {
            s.inputs.push(session::Input::Signal { name: k.clone(), value: resolve::value_to_json(v) });
        }
        if self.program.index() != 0 {
            s.inputs.push(session::Input::Goto { index: self.program.index() });
        }
        self.recording = Some(s);
    }

    /// Stop recording and return the session.
    pub fn take_recording(&mut self) -> Option<session::Session> {
        self.recording.take()
    }

    /// Replay a session on a fresh engine; `each` sees every input and, for frames, the frame.
    pub fn replay(s: &session::Session, mut each: impl FnMut(&session::Input, Option<&FrameOutput>)) -> Result<Engine, String> {
        let doc: Doc = serde_json::from_value(s.doc.clone()).map_err(|e| e.to_string())?;
        let mut e = Engine::new();
        e.load(doc);
        for i in &s.inputs {
            let out = e.apply(i)?;
            each(i, out.as_ref());
        }
        Ok(e)
    }

    /// Apply one recorded input.
    pub fn apply(&mut self, i: &session::Input) -> Result<Option<FrameOutput>, String> {
        use session::Input as I;
        match i {
            I::Frame { t } => return Ok(Some(self.frame(*t))),
            I::Clock { t } => self.set_clock(*t),
            I::Pointer { kind, x, y } => {
                let p = match kind.as_str() {
                    "move" => Pointer::Move { x: *x, y: *y },
                    "down" => Pointer::Down { x: *x, y: *y },
                    "up" => Pointer::Up { x: *x, y: *y },
                    "tap" => Pointer::Tap { x: *x, y: *y },
                    _ => Pointer::Leave,
                };
                self.pointer(p);
            }
            I::Wheel { x, y, delta } => {
                self.wheel(Vec2::new(*x, *y), *delta);
            }
            I::Event { name } => {
                self.event(name);
            }
            I::Goto { index } => {
                self.goto(*index);
            }
            I::Signal { name, value } => self.set_signal(name, resolve::json_to_value(value)),
            I::Provide { source, bytes } => self.provide(source, &session::unbase64(bytes)?)?,
            I::Range { source, offset, bytes } => self.provide_range(source, *offset, &session::unbase64(bytes)?)?,
            I::Resize { width, height, dpr } => self.resize(*width, *height, *dpr),
            I::Seek { pos } => {
                self.seek(*pos);
            }
            I::Playing { on } => self.set_playing(*on),
            I::ReducedMotion { on } => self.set_reduced_motion(*on),
            I::Work { scale } => self.set_work_scale(*scale),
            I::Activate { path } => {
                self.activate(path);
            }
            I::Mode { mode } => self.set_mode(match mode.as_str() {
                "dark" => Mode::Dark,
                "high-contrast" => Mode::HighContrast,
                _ => Mode::Light,
            }),
            I::Tokens { tokens } => self.set_host_tokens(tokens.clone()),
        }
        Ok(None)
    }

    // ---- drivers (autoplay, scroll scrub, live) -----------------------------------------------

    /// Run or pause autoplay (hosts pause it while the reader interacts, for reduced motion, or
    /// when the view is off-screen).
    pub fn set_playing(&mut self, on: bool) {
        self.rec(session::Input::Playing { on });
        self.playing = on;
        if on {
            self.settled_at = None; // the hold restarts
        }
    }

    pub fn playing(&self) -> bool {
        self.playing
    }

    /// Reduced motion (the platform setting): transitions become short crossfades — nothing
    /// travels, morphs or staggers. The document's motion rules are set aside, not merged: a more
    /// specific rule must not bring motion back.
    /// How much of the per-frame work budgets live frames may spend (0.1–1, default 1): points
    /// built, tile bytes decoded, map features styled. The budgets are sized for a desktop; a
    /// phone's slower CPU spends three or four times as long on the same work, so a host that sees
    /// its frames run long hands the engine a smaller share — detail arrives over more frames
    /// instead of dropping them (at least one tile a frame still gets through). An input like any
    /// other: sessions record it, replays repeat it.
    pub fn set_work_scale(&mut self, scale: f64) {
        self.rec(session::Input::Work { scale });
        self.work = if scale.is_finite() { scale.clamp(0.1, 1.0) } else { 1.0 };
    }

    /// Rows of point tiles a live frame prepares: [`FRAME_BUILD_ROWS`] times the host's share.
    fn frame_rows(&self) -> usize {
        ((FRAME_BUILD_ROWS as f64 * self.work) as usize).max(250)
    }

    pub fn set_reduced_motion(&mut self, on: bool) {
        self.rec(session::Input::ReducedMotion { on });
        self.reduced_motion = on;
        self.scrub = None;
    }

    /// The rules for a transition between two scenes: the document's, over the motion defaults
    /// of the recipes in either scene (an entering line draws on, an exiting one draws off).
    /// Reduced motion ignores both.
    fn rules_between(&self, a: &[datars_motion::Rule], b: &[datars_motion::Rule]) -> datars_motion::MotionRules {
        let base = self.motion_rules().into_owned();
        if self.reduced_motion || (a.is_empty() && b.is_empty()) {
            return base;
        }
        let mut defaults = a.to_vec();
        for r in b {
            if !defaults.contains(r) {
                defaults.push(r.clone());
            }
        }
        base.with_defaults(defaults)
    }

    fn motion_rules(&self) -> std::borrow::Cow<'_, datars_motion::MotionRules> {
        if !self.reduced_motion {
            return std::borrow::Cow::Borrowed(&self.rules);
        }
        let calm = datars_motion::Rule::new().duration(0.25).matcher(datars_motion::Matcher::None).easing(datars_motion::Easing::Linear);
        std::borrow::Cow::Owned(datars_motion::MotionRules::new(vec![calm]))
    }

    /// Scroll scrub: show the program at `pos` — the state `floor(pos)`, a fraction of the way into
    /// its transition to the next state. Exact and clock-free, so scrubbing both ways replays the
    /// same frames. Returns the state index now current.
    pub fn seek(&mut self, pos: f64) -> usize {
        self.rec(session::Input::Seek { pos });
        let n = self.program.state_names().len();
        let (i, frac) = drivers::split(pos, n);
        if self.program.index() != i {
            self.program.goto_index(i);
            self.clear_state_overrides();
        }
        self.active = None;
        self.settled_at = None;
        if frac <= 0.0 {
            self.shown = Some(self.resolve_now());
            return i;
        }
        if self.scrub.as_ref().map(|(k, _)| *k) != Some(i) {
            let (_, _, plan) = self.plan_states(i, i + 1);
            self.scrub = Some((i, plan));
        }
        if let Some((_, plan)) = &self.scrub {
            let scene = plan.at(frac, &TextShaper(&self.fonts));
            self.shown = Some(Rc::new(Resolved { scene, actions: self.resolve_now().actions.clone(), tiles: Vec::new(), points: Vec::new(), motion: Vec::new(), hover_asked: Vec::new() }));
        }
        i
    }

    /// Autoplay and live refreshes due at `now`. Returns the next wake-up time, if any.
    fn run_drivers(&mut self, now: f64, animating: &mut bool) -> Option<f64> {
        let mut wake: Option<f64> = None;
        let soonest = |t: f64, w: &mut Option<f64>| *w = Some(w.map_or(t, |x: f64| x.min(t)));
        if !*animating && self.settled_at.is_none() {
            self.settled_at = Some(now);
        }
        if !*animating && self.playing && self.baked.is_none() && drivers::has(self.doc.program.as_ref(), "autoplay") {
            let due = self.settled_at.unwrap_or(now) + self.program.hold(self.program.index());
            if now >= due {
                if self.event_inner("next") {
                    *animating = self.active.is_some();
                }
            } else {
                soonest(due, &mut wake);
            }
        }
        let live: Vec<(String, f64, Option<Request>)> = self
            .doc
            .data
            .iter()
            .filter_map(|(name, s)| {
                let every = s.live.as_ref().map(|l| l.every).filter(|e| *e > 0.0)?;
                let req = match &s.from {
                    datars_ir::SourceKind::Url(url) => Some(Request::Source { name: name.clone(), url: url.clone() }),
                    datars_ir::SourceKind::Slot(slot) => Some(Request::Slot { name: name.clone(), slot: slot.clone() }),
                    _ => None,
                };
                Some((name.clone(), every, req))
            })
            .collect();
        for (name, every, req) in live {
            // Rows that came with the bundle (a snapshot taken when it was published): refreshed at
            // once, then every `every` s. Rows the host fetched are fresh.
            let shipped = self.snapshots.contains(&name);
            let due = *self.live_due.entry(name.clone()).or_insert(if shipped { now } else { now + every });
            let next = if now >= due {
                if let Some(r) = req {
                    if !self.requests.contains(&r) {
                        self.requests.push(r);
                    }
                }
                now + every
            } else {
                due
            };
            self.live_due.insert(name, next);
            soonest(next, &mut wake);
        }
        wake
    }

    /// Idle-time work, for hosts to call when the page is idle (`requestIdleCallback`): while the
    /// reader is on a step, resolve the next and previous states and plan the transitions to them
    /// from what's on screen, so that stepping starts moving at once instead of resolving and
    /// planning first. One piece of work per call; returns whether there is more. Anything that
    /// changes the scene on screen (a signal, exploring, a resize) makes the prepared plans stale,
    /// and they're simply not used.
    pub fn prepare(&mut self) -> bool {
        if self.active.is_some() || self.baked.is_some() || self.reduced_motion {
            return false;
        }
        let Some(shown) = self.shown.clone() else { return false };
        self.prepared.retain(|p| Rc::ptr_eq(&p.from, &shown));
        let names = self.program.state_names();
        let cur = self.program.index();
        for to in [cur + 1, cur.wrapping_sub(1)] {
            if to >= names.len() || self.prepared.iter().any(|p| p.to_state == names[to]) {
                continue;
            }
            let target = self.resolve_state(to);
            if target.scene == shown.scene {
                continue;
            }
            let cx = datars_motion::PlanCx { theme: self.theme.clone(), from_state: Some(names[cur].clone()), to_state: Some(names[to].clone()), event_point: None };
            let plan = datars_motion::plan(&shown.scene, &target.scene, &self.rules_between(&shown.motion, &target.motion), &cx);
            // Its map data too: the tiles along the flight and where it lands download while the
            // reader reads (decoded when drawn).
            self.prefetch(&plan);
            self.prepared.push(Prepared { warmed: false, warm_rounds: 0, waiting_at: None, from: shown.clone(), from_state: names[cur].clone(), to_state: names[to].clone(), target, plan });
            return true;
        }
        self.buffer_around(&shown)
    }

    /// Idle time, once the neighbouring steps are prepared: the map and point tiles around what's
    /// on screen — just past each edge, a level finer in the middle, a level coarser — where a
    /// reader exploring goes next. Bytes only (decoded when drawn); one look per call.
    fn buffer_around(&mut self, shown: &Rc<Resolved>) -> bool {
        const LOOKS: [(f64, f64, f64); 6] = [(1.0, 0.5, 0.0), (1.0, -0.5, 0.0), (1.0, 0.0, 0.5), (1.0, 0.0, -0.5), (2.0, 0.0, 0.0), (0.5, 0.0, 0.0)];
        if self.range_fetch.is_some() || (self.bindings.borrow().is_empty() && self.point_bindings.borrow().is_empty()) {
            return false;
        }
        if !self.buffered.as_ref().is_some_and(|b| Rc::ptr_eq(b, shown)) {
            self.buffered = Some(shown.clone());
            self.buffer_looks = 0;
        }
        let Some(&(k, px, py)) = LOOKS.get(self.buffer_looks) else { return false };
        self.buffer_looks += 1;
        let mut s = shown.scene.clone();
        self.fill_tiles_look(&mut s, false, tiles::LabelMode::Draw, None, Some((k, Vec2::new(px, py))));
        true
    }

    /// After [`Engine::prepare`]: the display list of a prepared step's destination that hasn't
    /// been handed out yet, for the host to warm its renderer with (tessellate what the step
    /// will draw before it's taken). One per call; `None` when there are no more.
    pub fn prepared_to_warm(&mut self) -> Option<DisplayList> {
        // A step whose tiles are still downloading waits until more bytes are in (then it's worth
        // another round), so idle time doesn't spin on it.
        let arrived = self.tiles.borrow().stats.bytes;
        let k = self.prepared.iter().position(|p| !p.warmed && p.waiting_at.is_none_or(|b| arrived > b))?;
        // With its map tiles, as far as they've arrived (the prefetch asked for them), decoded a
        // frame's budget at a time — idle slices stay short; the next call carries on.
        let mut scene = self.prepared[k].target.scene.clone();
        let report = self.fill_frame(&mut scene);
        let p = &mut self.prepared[k];
        p.warm_rounds += 1;
        // Done when its tiles are all in, decoded and styled — the step lands on meshes already
        // built. Before, a round with tiles still downloading counted as done: the idle warm ran
        // before the prefetch's bytes came and the landing frame tessellated the destination.
        p.waiting_at = (report.pending > 0).then_some(arrived);
        p.warmed = (report.building == 0 && report.pending == 0) || p.warm_rounds >= 32;
        Some(self.display_list(&scene))
    }

    /// A prepared plan for this step, if one fits exactly: from the settled scene on screen, to
    /// this target, between these states.
    fn take_prepared(&mut self, target: &Rc<Resolved>, from_state: &Option<String>) -> Option<datars_motion::Plan> {
        if self.active.is_some() {
            return None;
        }
        let shown = self.shown.as_ref()?;
        let to_state = self.program.state_name();
        let k = self.prepared.iter().position(|p| Rc::ptr_eq(&p.from, shown) && Rc::ptr_eq(&p.target, target) && Some(&p.from_state) == from_state.as_ref() && p.to_state == to_state)?;
        Some(self.prepared.swap_remove(k).plan)
    }

    /// Plan a transition from what's on screen to the current state's scene.
    fn transition_to_current(&mut self, from_state: Option<String>) {
        let target = self.resolve_now();
        let current = self.current_scene();
        match current {
            Some(cur) if cur != target.scene => {
                let prepared = self.take_prepared(&target, &from_state);
                self.prepared.clear();
                let cx = datars_motion::PlanCx { theme: self.theme.clone(), from_state, to_state: Some(self.program.state_name()), event_point: None };
                // What's on screen came from the transition in flight, or the settled scene.
                let from = self.active.as_ref().map(|a| a.to.motion.clone()).or_else(|| self.shown.as_ref().map(|s| s.motion.clone())).unwrap_or_default();
                let plan = match prepared {
                    Some(p) => p,
                    None => datars_motion::plan(&cur, &target.scene, &self.rules_between(&from, &target.motion), &cx),
                };
                self.prefetch(&plan);
                let duration = plan.duration().max(0.0);
                self.active = Some(Active { plan, start: self.clock, duration, to: target, shown: false });
            }
            _ => {
                self.shown = Some(target);
                self.active = None;
            }
        }
    }

    /// Every element drawn under a point of the current frame (CSS px), topmost first — pickable
    /// or not: what an editor selects when you click the canvas. `explain` says where it came from.
    pub fn hit_test(&self, x: f64, y: f64) -> Vec<pick::AnyHit> {
        self.current_scene().map(|s| pick::pick_all(&s, Vec2::new(x, y))).unwrap_or_default()
    }

    /// [`Engine::hit_test`] at many points of the same frame (the scene is made once, not per
    /// point: a host placing a card samples dozens of points over a scene of thousands of marks).
    pub fn hit_test_many(&self, pts: &[Vec2]) -> Vec<Vec<pick::AnyHit>> {
        match self.current_scene() {
            Some(s) => pts.iter().map(|p| pick::pick_all(&s, *p)).collect(),
            None => vec![Vec::new(); pts.len()],
        }
    }

    /// Elements that link somewhere (a data credit's licence page, a source), where they are in
    /// the current frame: hosts put real links over them.
    pub fn links(&self) -> Vec<pick::LinkOut> {
        self.current_scene().map(|s| pick::links(&s)).unwrap_or_default()
    }

    fn current_scene(&self) -> Option<Scene> {
        if let Some(a) = &self.active {
            let t = if a.duration > 0.0 { ((self.clock - a.start) / a.duration).clamp(0.0, 1.0) } else { 1.0 };
            return Some(a.plan.at(t, &TextShaper(&self.fonts)));
        }
        self.shown.as_ref().map(|r| r.scene.clone())
    }

    /// The settled scene of the current state (headless use, tests), tiles filled.
    pub fn scene(&mut self) -> Scene {
        let mut s = self.resolve_now().scene.clone();
        self.fill_tiles(&mut s, true);
        self.absorb_tile_diags();
        s
    }

    /// The resolved (unfilled) scene of state `index`, without changing the running program.
    fn resolve_state(&mut self, index: usize) -> Rc<Resolved> {
        let saved = (self.program.clone(), self.signals.clone());
        self.program.goto_index(index);
        self.clear_state_overrides();
        let r = self.resolve_now();
        (self.program, self.signals) = saved;
        r
    }

    /// The settled scene of state `index` (without changing the running program), tiles filled.
    pub fn scene_for_state(&mut self, index: usize) -> Scene {
        let mut s = self.resolve_state(index).scene.clone();
        self.fill_tiles(&mut s, true);
        self.absorb_tile_diags();
        s
    }

    /// A motion plan between two states' scenes (film, tests, `datars film`). Plans are made
    /// between resolved scenes (tile placeholders empty); the scenes returned, like every frame
    /// from [`Engine::plan_at`], have their tiles filled.
    pub fn plan_states(&mut self, a: usize, b: usize) -> (Scene, Scene, datars_motion::Plan) {
        let names = self.program.state_names();
        let ra = self.resolve_state(a);
        let mut sa = ra.scene.clone();
        self.fill_tiles(&mut sa, true);
        let rb = self.resolve_state(b);
        let mut sb = rb.scene.clone();
        self.fill_tiles(&mut sb, true);
        self.absorb_tile_diags();
        let cx = datars_motion::PlanCx { theme: self.theme.clone(), from_state: names.get(a).cloned(), to_state: names.get(b).cloned(), event_point: None };
        let plan = datars_motion::plan(&ra.scene, &rb.scene, &self.rules_between(&ra.motion, &rb.motion), &cx);
        (sa, sb, plan)
    }

    /// Evaluate a plan at t with this engine's text shaper; tiles filled for that frame's cameras.
    pub fn plan_at(&self, plan: &datars_motion::Plan, t: f64) -> Scene {
        let mut s = self.planned_at(plan, t);
        self.fill_tiles_in_flight(plan, t, plan.duration(), &mut s, None);
        s
    }

    /// The planned frame at t, tile placeholders left empty: the motion itself, for motion checks
    /// and trails. Tile content is a function of each frame's camera, not planned motion — labels
    /// come and go as they collide, tiles swap as the zoom crosses a level.
    pub fn planned_at(&self, plan: &datars_motion::Plan, t: f64) -> Scene {
        plan.at(t, &TextShaper(&self.fonts))
    }

    pub fn display_list(&self, scene: &Scene) -> DisplayList {
        datars_render::flatten_cached(scene, &self.theme, &mut self.flatten_cache.borrow_mut())
    }

    pub fn fonts(&self) -> &datars_text::FontDb {
        &self.fonts
    }

    // ---- frames and input ----------------------------------------------------------------------

    /// The transition in flight, if any: what its plan is made of and how far along it is (0–1),
    /// for a host's diagnostics panel.
    pub fn transition_stats(&self) -> Option<(datars_motion::PlanStats, f64)> {
        self.active.as_ref().map(|a| (a.plan.stats().clone(), if a.duration > 0.0 { ((self.clock - a.start) / a.duration).clamp(0.0, 1.0) } else { 1.0 }))
    }

    /// Tell the engine the time (seconds, host clock) without rendering. Transitions start at the
    /// engine's clock, which frames advance; a host that renders on demand has an old clock after
    /// idling, so it calls this before an event, a signal or a `goto` — otherwise the transition
    /// would count as long started and jump to its end (a 3 s camera flight would not fly).
    pub fn set_clock(&mut self, now: f64) {
        self.rec(session::Input::Clock { t: now });
        if now.is_finite() && now > self.clock {
            self.clock = now;
        }
    }

    /// Advance to `now` (seconds, host clock) and produce a frame.
    pub fn frame(&mut self, now: f64) -> FrameOutput {
        self.rec(session::Input::Frame { t: now });
        self.clock = now;
        // Clocks run on frame time, a step at most 0.1 s: a host that slept doesn't make them jump.
        let dt = self.clock_last.map(|l| (now - l).clamp(0.0, 0.1)).unwrap_or(0.0);
        self.clock_last = Some(now);
        if self.shown.is_none() && self.active.is_none() {
            self.transition_to_current(None);
        }
        let mut animating = false;
        if let Some(a) = &mut self.active {
            // Anchored at the first frame that draws it — unless that frame comes after the
            // transition would have ended anyway (a snapshot for a later time, a host that
            // doesn't keep the clock): then it shows the finished state, as it always did.
            if !a.shown {
                a.shown = true;
                if now - a.start < a.duration {
                    a.start = a.start.max(now);
                }
            }
        }
        if let Some(a) = &self.active {
            if a.duration <= 0.0 || now - a.start >= a.duration {
                self.shown = Some(a.to.clone());
                self.active = None;
                self.settled_at = Some(now);
            } else {
                animating = true;
            }
        }
        if self.active.is_none() && self.playing && !self.reduced_motion && self.reads_clock() {
            self.clock_time += dt;
            self.shown = Some(self.resolve_now());
            animating = true;
        }
        let wake_at = self.run_drivers(now, &mut animating);
        // A transition a driver started just now (autoplay) is timed from now and drawn by this
        // frame: it has been shown.
        if let Some(a) = &mut self.active {
            a.shown = true;
        }
        let mut scene = self.current_scene().unwrap_or_else(|| Scene::new(self.viewport.width, self.viewport.height, datars_scene::Node::group(datars_scene::Key::name("empty"), vec![])));
        let tiles = match &self.active {
            Some(a) => {
                let t = if a.duration > 0.0 { ((self.clock - a.start) / a.duration).clamp(0.0, 1.0) } else { 1.0 };
                self.tiles.borrow_mut().flight = Some(t);
                let r = self.fill_tiles_in_flight(&a.plan, t, a.duration, &mut scene, Some(self.frame_rows()));
                self.tiles.borrow_mut().flight = None;
                r
            }
            None => self.fill_frame(&mut scene),
        };
        // Point levels still being prepared: keep frames coming until they're in.
        if tiles.building > 0 {
            animating = true;
        }
        self.absorb_tile_diags();
        let display = self.display_list(&scene);
        let anchors = pick::anchors(&scene);
        FrameOutput { scene, display, anchors, animating, wake_at: if animating { None } else { wake_at }, pending_tiles: tiles.pending }
    }

    /// Pointer input: hit-test the current frame and run the intents bound there.
    /// Returns the label of the inspected element (for tooltips), if any.
    pub fn pointer(&mut self, p: Pointer) -> Option<String> {
        let input = match p {
            Pointer::Move { x, y } => session::Input::Pointer { kind: "move".into(), x, y },
            Pointer::Down { x, y } => session::Input::Pointer { kind: "down".into(), x, y },
            Pointer::Up { x, y } => session::Input::Pointer { kind: "up".into(), x, y },
            Pointer::Tap { x, y } => session::Input::Pointer { kind: "tap".into(), x, y },
            Pointer::Leave => session::Input::Pointer { kind: "leave".into(), x: 0.0, y: 0.0 },
            Pointer::Wheel { x, y, delta } => session::Input::Wheel { x, y, delta },
        };
        self.rec(input);
        let (x, y) = match p {
            Pointer::Move { x, y } | Pointer::Down { x, y } | Pointer::Up { x, y } | Pointer::Tap { x, y } => (x, y),
            Pointer::Wheel { x, y, delta } => {
                self.wheel_inner(Vec2::new(x, y), delta);
                return None;
            }
            Pointer::Leave => {
                self.signals.remove("inspected");
                self.drag = None;
                self.cursor = Cursor::Default;
                self.set_hovered(None);
                return None;
            }
        };
        let mut scene = self.current_scene()?;
        // Rows drawn from point pyramids are only in the filled frame: hit-test what's on screen.
        if !self.point_bindings.borrow().is_empty() {
            self.fill_frame(&mut scene);
        }
        let at = Vec2::new(x, y);
        // Drags (brush, pan) take the pointer while they last.
        match p {
            Pointer::Down { .. } => {
                if let Some((path, action)) = self.drag_target(&scene, at) {
                    let scrub = matches!(action, resolve::BoundAction::Scrub { .. });
                    self.drag = Some(Drag { path, action, start: at, last: at, moved: false });
                    if scrub {
                        // A slider jumps to where it's pressed.
                        self.drag_to(&scene, at);
                        return None;
                    }
                }
            }
            Pointer::Move { .. } if self.drag.is_some() => {
                self.drag_to(&scene, at);
                if let Some(d) = &self.drag {
                    self.cursor = match d.action {
                        resolve::BoundAction::Brush { .. } => Cursor::Crosshair,
                        _ if d.moved => Cursor::Grabbing,
                        _ => self.cursor,
                    };
                }
                return None;
            }
            Pointer::Up { .. } | Pointer::Tap { .. } => {
                if let Some(d) = self.drag.take() {
                    if d.moved {
                        // The release point counts (a host may not send a move there first).
                        if at != d.last {
                            self.drag = Some(d.clone());
                            self.drag_to(&scene, at);
                            self.drag = None;
                        }
                        self.cursor = match d.action {
                            resolve::BoundAction::Explore { .. } => Cursor::Grab,
                            resolve::BoundAction::Brush { .. } => Cursor::Crosshair,
                            _ => Cursor::Pointer,
                        };
                        return None;
                    }
                    if let resolve::BoundAction::Brush { signal, .. } = &d.action {
                        // A click without a drag clears the brush.
                        self.set_signals(vec![(format!("{signal}.active"), Value::Bool(false)), (format!("{signal}.lo"), Value::Num(f64::NAN)), (format!("{signal}.hi"), Value::Num(f64::NAN)), (signal.clone(), env::encode_keyset(&[]))]);
                    }
                }
            }
            _ => {}
        }
        let tap = matches!(p, Pointer::Tap { .. });
        let hit = if tap { pick::pick_within(&scene, at, pick::TOUCH_REACH) } else { pick::pick(&scene, at) };
        let resolved = self.shown.clone()?;
        if matches!(p, Pointer::Move { .. } | Pointer::Up { .. }) {
            self.cursor = self.cursor_at(&scene, &resolved, at, hit.as_ref());
            // Touch has no hover: a tap never leaves an element looking hovered.
            let hovered = hit.as_ref().and_then(|h| resolved.hover_asked.iter().filter(|p| h.path.0.starts_with(&p.0)).max_by_key(|p| p.0.len()).cloned());
            self.set_hovered(hovered);
        }
        match (&p, &hit) {
            (Pointer::Move { .. } | Pointer::Tap { .. }, Some(h)) => {
                let key = h.key_string();
                if self.signals.get("inspected") != Some(&Value::Str(Arc::from(key.as_str()))) {
                    self.signals.insert("inspected".into(), Value::Str(Arc::from(key.as_str())));
                }
                if let Some(acts) = resolved.actions.iter().find(|(path, _)| h.path.0.starts_with(&path.0)).map(|(_, a)| a.clone()) {
                    if let Some(a) = acts.get("inspect") {
                        self.run_action(a);
                    }
                }
                if tap {
                    self.activate_at(&resolved, &h.path);
                }
                // A control shows its own value: no tooltip over it.
                if h.control {
                    return None;
                }
                self.lod_label(&h.path).or_else(|| h.label.clone())
            }
            (Pointer::Up { .. }, Some(h)) => {
                self.activate_at(&resolved, &h.path);
                if h.control {
                    return None;
                }
                self.lod_label(&h.path).or_else(|| h.label.clone())
            }
            (Pointer::Tap { .. }, None) => {
                self.signals.remove("inspected");
                None
            }
            _ => None,
        }
    }

    /// What the pointer looks like after the last pointer input: [`Cursor::Pointer`] over what a
    /// click acts on, grab over a view that pans, a crosshair over a brushable area.
    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    fn cursor_at(&self, scene: &Scene, resolved: &Resolved, at: Vec2, hit: Option<&pick::Hit>) -> Cursor {
        // A click target a reader is told about (a label): not the veil a dropdown lays over the
        // chart to close itself on a click anywhere.
        let nearest = hit.filter(|h| h.label.is_some()).and_then(|h| resolved.actions.iter().filter(|(p, _)| h.path.0.starts_with(&p.0)).max_by_key(|(p, _)| p.0.len()));
        if nearest.is_some_and(|(_, acts)| acts.contains_key("activate") || acts.contains_key("pick")) {
            return Cursor::Pointer;
        }
        match self.drag_target(scene, at).map(|(_, a)| a) {
            Some(resolve::BoundAction::Brush { .. }) => Cursor::Crosshair,
            Some(resolve::BoundAction::Explore { .. }) => Cursor::Grab,
            Some(resolve::BoundAction::Scrub { .. }) => Cursor::Pointer,
            _ => Cursor::Default,
        }
    }

    /// Settle the element under the pointer that `hover()` asks about: a change re-resolves and
    /// fades (hover states never blink) at the pace of a UI, whatever the document's transitions
    /// take — unless it lands in a transition already playing, which keeps its own pace.
    fn set_hovered(&mut self, hovered: Option<KeyPath>) {
        if self.hovered != hovered {
            self.hovered = hovered;
            let moving = self.active.is_some();
            self.transition_to_current(None);
            if let (false, Some(a)) = (moving, &mut self.active) {
                a.duration = a.duration.min(HOVER_SECONDS);
            }
        }
    }

    /// Run the `activate` intent bound nearest above the element at `path` (a click, a tap).
    fn activate_at(&mut self, resolved: &Resolved, path: &KeyPath) {
        if let Some(acts) = resolved.actions.iter().filter(|(p, _)| path.0.starts_with(&p.0)).max_by_key(|(p, _)| p.0.len()).map(|(_, a)| a.clone()) {
            if let Some(a) = acts.get("activate") {
                self.run_action(a);
            }
        }
    }

    /// The label of a row drawn from a point pyramid, by its hit path: placeholder / tile (or cell)
    /// / row. Pyramids label lazily (only the hovered row is ever formatted).
    fn lod_label(&self, path: &KeyPath) -> Option<String> {
        let n = path.0.len();
        if n < 3 {
            return None;
        }
        let holder = KeyPath(path.0[..n - 2].to_vec()).to_string();
        let b = self.point_bindings.borrow().get(&holder).cloned()?;
        let ints: Vec<i64> = path.0[n - 2].parts().iter().filter_map(|p| if let datars_scene::KeyPart::Int(i) = p { Some(*i) } else { None }).collect();
        let (&[z, x, y, ..], Some(datars_scene::KeyPart::Int(row))) = (ints.as_slice(), path.0[n - 1].parts().first()) else { return None };
        let env = tiles::BuildEnv { doc: &self.doc, theme: &self.theme, fonts: &self.fonts, geo: &self.geo, sources: &self.sources, size_class: self.viewport.size_class() };
        points::fill::label(&env, &mut self.tiles.borrow_mut(), &b, datars_geo::TileId::new(z as u8, x as u32, y as u32), *row as u64)
    }

    fn run_action(&mut self, a: &BoundAction) {
        match a {
            BoundAction::Set { signal, value } => self.set_signal_inner(signal, value.clone()),
            BoundAction::Toggle { signal, value } => {
                let mut set = self.all_signals().get(signal).map(env::decode_keyset).unwrap_or_default();
                let v = env::str_of(value);
                if let Some(i) = set.iter().position(|k| *k == v) {
                    set.remove(i);
                } else {
                    set.push(v);
                }
                self.set_signal_inner(signal, env::encode_keyset(&set));
            }
            BoundAction::Event { event } => {
                self.event_inner(event);
            }
            BoundAction::Chapter { chapter, key } => {
                let from = self.program.state_name();
                if self.program.enter_chapter(chapter, key) {
                    self.transition_to_current(Some(from));
                }
            }
            // A pick is the host's to offer (its own picker); a drag's run by `drag_to`.
            BoundAction::Brush { .. } | BoundAction::Explore { .. } | BoundAction::Scrub { .. } | BoundAction::Pick { .. } => {}
        }
    }

    /// Set several signals as one change (one transition, not one per signal).
    fn set_signals(&mut self, kv: Vec<(String, Value)>) {
        for (k, v) in kv {
            self.signals.insert(k, v);
        }
        self.transition_to_current(None);
    }

    /// Direct manipulation (a brush or pan drag): the scene follows the pointer — resolve and
    /// show, no transition to plan.
    fn set_signals_now(&mut self, kv: Vec<(String, Value)>) {
        for (k, v) in kv {
            self.signals.insert(k, v);
        }
        self.active = None;
        self.shown = Some(self.resolve_now());
    }

    /// The innermost brush or explore binding whose node contains `at` (root coordinates) and
    /// can be seen (a faded-out map doesn't take the drag from the one showing under it).
    fn drag_target(&self, scene: &Scene, at: Vec2) -> Option<(KeyPath, resolve::BoundAction)> {
        let mut best: Option<(KeyPath, resolve::BoundAction)> = None;
        for (path, act, inv, area) in self.drag_areas(scene) {
            if area.contains(inv.apply(at)) && best.as_ref().is_none_or(|(p, _)| path.0.len() > p.0.len()) {
                best = Some((path, act));
            }
        }
        best
    }

    /// Every brush, explore or scrub binding that can be seen, with where it takes the pointer:
    /// the inverse of its node's transform (root → local) and its area in local units.
    fn drag_areas(&self, scene: &Scene) -> Vec<(KeyPath, resolve::BoundAction, datars_math::Affine, datars_math::Rect)> {
        let Some(resolved) = self.shown.as_ref() else { return Vec::new() };
        let mut out = Vec::new();
        for (path, acts) in &resolved.actions {
            let act = acts.get("brush").or_else(|| acts.get("explore")).or_else(|| acts.values().find(|a| matches!(a, resolve::BoundAction::Brush { .. } | resolve::BoundAction::Explore { .. } | resolve::BoundAction::Scrub { .. })));
            let Some(act) = act else { continue };
            if !pick::shown_at(scene, path) {
                continue;
            }
            let Some((node, xf)) = pick::node_at(scene, path) else { continue };
            let Some(inv) = xf.inverse() else { continue };
            let area = match &node.kind {
                datars_scene::NodeKind::View { viewport, .. } => *viewport,
                _ => bounds::node_bounds(&datars_scene::Node { common: datars_scene::Common::default(), ..node.clone() }),
            };
            out.push((path.clone(), act.clone(), inv, area));
        }
        out
    }

    /// The texts a frame (`FrameOutput::scene`) draws, where it draws them — for hosts that lay
    /// selectable, findable text over the canvas. In tree order (a copy keeps it); each marked
    /// where a press would start the chart's own drag (a pan, a brush) and whether it is a map's
    /// place name.
    pub fn text_layer(&self, scene: &Scene) -> Vec<textlayer::TextOut> {
        let mut out = textlayer::texts(scene, &self.fonts);
        if out.is_empty() {
            return out;
        }
        // Text that is part of what a drag moves (a place name on a pannable map, a label in a
        // brushable plot) — not text merely drawn over it, like a title over a full-bleed map.
        let areas: Vec<(String, datars_math::Affine, datars_math::Rect)> = self.drag_areas(scene).into_iter().map(|(p, _, inv, area)| (format!("{p}/"), inv, area)).collect();
        // Text on something clickable — a checkbox's label, a button's — belongs to that control:
        // a click on it is the control's, not the start of a selection.
        let clickable: Vec<String> = self.shown.as_ref().map(|r| r.actions.iter().filter(|(_, a)| a.contains_key("activate")).map(|(p, _)| format!("{p}/")).collect()).unwrap_or_default();
        // Place names are what tiles placed: under a tiles placeholder's `labels` group.
        let places: Vec<String> = self.bindings.borrow().keys().map(|k| format!("{k}/{}/", datars_scene::Key::name("labels"))).collect();
        for t in &mut out {
            let [x, y, w, h] = t.bounds;
            let c = Vec2::new(x + w / 2.0, y + h / 2.0);
            t.drag = areas.iter().any(|(p, inv, area)| t.path.starts_with(p.as_str()) && area.contains(inv.apply(c))) || clickable.iter().any(|p| t.path.starts_with(p.as_str()));
            t.place = places.iter().any(|p| t.path.starts_with(p.as_str()));
        }
        out
    }

    fn drag_to(&mut self, scene: &Scene, at: Vec2) {
        let Some(mut d) = self.drag.take() else { return };
        let Some((node, xf)) = pick::node_at(scene, &d.path) else { return };
        let Some(inv) = xf.inverse() else { return };
        if (at.x - d.start.x).abs() + (at.y - d.start.y).abs() > 3.0 {
            d.moved = true;
        }
        match &d.action {
            resolve::BoundAction::Brush { signal, axis, map } if d.moved => {
                let (a, b) = (inv.apply(d.start), inv.apply(at));
                let (pa, pb) = if *axis == 1 { (a.y, b.y) } else { (a.x, b.x) };
                let (lo_px, hi_px) = (pa.min(pb), pa.max(pb));
                let mut kv = vec![(format!("{signal}.active"), Value::Bool(true))];
                match map {
                    resolve::BrushMap::Continuous { .. } => {
                        let (va, vb) = (map.value(lo_px), map.value(hi_px));
                        kv.push((format!("{signal}.lo"), Value::Num(va.min(vb))));
                        kv.push((format!("{signal}.hi"), Value::Num(va.max(vb))));
                    }
                    resolve::BrushMap::Bands { centers } => {
                        let keys: Vec<String> = centers.iter().filter(|(c, _)| *c >= lo_px && *c <= hi_px).map(|(_, k)| k.clone()).collect();
                        kv.push((signal.clone(), env::encode_keyset(&keys)));
                    }
                }
                self.set_signals_now(kv);
            }
            resolve::BoundAction::Scrub { signal, axis, map, step, bounds } => {
                let p = inv.apply(at);
                let mut v = map.value(if *axis == 1 { p.y } else { p.x });
                if *step > 0.0 && v.is_finite() {
                    v = (v / step).round() * step;
                }
                if v.is_finite() && bounds.0 <= bounds.1 {
                    v = v.clamp(bounds.0, bounds.1);
                }
                if self.all_signals().get(signal) != Some(&Value::Num(v)) {
                    self.set_signals_now(vec![(signal.clone(), Value::Num(v))]);
                }
            }
            resolve::BoundAction::Explore { prefix, .. } => {
                if let datars_scene::NodeKind::View { camera: Some(cam), .. } = &node.kind {
                    // Pan: the content under the pointer follows it.
                    let (a, b) = (inv.apply(d.last), inv.apply(at));
                    let (dx, dy) = ((b.x - a.x) / cam.zoom, (b.y - a.y) / cam.zoom);
                    self.set_signals_now(vec![(format!("{prefix}.x"), Value::Num(cam.x - dx)), (format!("{prefix}.y"), Value::Num(cam.y - dy))]);
                }
            }
            _ => {}
        }
        d.last = at;
        self.drag = Some(d);
    }

    /// Zoom an explorable view around the pointer. Returns whether a view took the wheel (hosts
    /// let the page scroll otherwise).
    pub fn wheel(&mut self, at: Vec2, delta: f64) -> bool {
        self.rec(session::Input::Wheel { x: at.x, y: at.y, delta });
        self.wheel_inner(at, delta)
    }

    fn wheel_inner(&mut self, at: Vec2, delta: f64) -> bool {
        let Some(scene) = self.current_scene() else { return false };
        let Some((path, resolve::BoundAction::Explore { prefix, fit, zoom })) = self.drag_target(&scene, at) else { return false };
        let Some((node, xf)) = pick::node_at(&scene, &path) else { return false };
        let datars_scene::NodeKind::View { viewport, camera: Some(cam), .. } = &node.kind else { return false };
        let Some(inv) = xf.inverse() else { return false };
        let local = inv.apply(at);
        let Some(to_content) = cam.transform(*viewport).inverse() else { return false };
        let c = to_content.apply(local);
        let factor = (cam.zoom / fit.zoom * datars_math::m::exp(-delta * 0.0015)).clamp(zoom.0, zoom.1);
        let z = fit.zoom * factor;
        let (vx, vy) = (viewport.x + viewport.w / 2.0, viewport.y + viewport.h / 2.0);
        self.set_signals_now(vec![
            (format!("{prefix}.zoom"), Value::Num(factor)),
            (format!("{prefix}.x"), Value::Num(c.x - (local.x - vx) / z)),
            (format!("{prefix}.y"), Value::Num(c.y - (local.y - vy) / z)),
        ]);
        true
    }


    /// The accessible description of the current scene: (role, label, depth) in reading order.
    pub fn semantics(&mut self) -> Vec<(String, String, usize)> {
        self.semantic_items().into_iter().map(|s| (s.role, s.label, s.depth)).collect()
    }

    /// The engine-drawn controls in the current scene (scrub bindings), for native accessible
    /// counterparts: `<input type=range>` on the web, adjustable elements on iOS.
    pub fn controls(&mut self) -> Vec<Control> {
        let Some(resolved) = self.shown.clone() else { return Vec::new() };
        // Most charts have none: no scene to make.
        let offered = |acts: &BTreeMap<String, resolve::BoundAction>| acts.values().any(|a| matches!(a, resolve::BoundAction::Scrub { map: resolve::BrushMap::Continuous { .. }, .. } | resolve::BoundAction::Pick { .. }));
        if !resolved.actions.iter().any(|(_, acts)| offered(acts)) {
            return Vec::new();
        }
        // The resolved scene is enough: controls are authored nodes, never map tiles or point
        // levels — filling those decoded and built every tile in view (a galaxy's stars, tens of
        // ms) each time a host asked, on every step.
        let current = self.resolve_now();
        let scene = &current.scene;
        let signals = self.all_signals();
        let mut out = Vec::new();
        for (path, acts) in &resolved.actions {
            // A control faded out isn't offered either.
            if !pick::shown_at(scene, path) {
                continue;
            }
            let rect = pick::node_at(scene, path).map(|(n, xf)| bounds::transform_rect(bounds::node_bounds(n), &xf)).map_or([0.0; 4], |r| [r.x, r.y, r.w, r.h]);
            let label_of = |signal: &str| {
                pick::node_at(scene, path)
                    .and_then(|(n, _)| {
                        fn first(n: &datars_scene::Node) -> Option<String> {
                            n.semantics.as_ref().map(|s| s.label.clone()).filter(|l| !l.is_empty()).or_else(|| n.children().iter().find_map(first))
                        }
                        first(n)
                    })
                    .unwrap_or_else(|| signal.to_string())
            };
            for a in acts.values() {
                if let resolve::BoundAction::Pick { signal, options } = a {
                    let current = match signals.get(signal) {
                        Some(Value::Num(n)) => serde_json::json!(n),
                        Some(Value::Bool(b)) => serde_json::json!(b),
                        Some(Value::Null) | None => serde_json::Value::Null,
                        Some(v) => serde_json::Value::String(env::str_of(v)),
                    };
                    out.push(Control { kind: "select", signal: signal.clone(), label: label_of(signal), min: f64::NAN, max: f64::NAN, step: 0.0, value: f64::NAN, options: options.clone(), current, rect });
                    continue;
                }
                let resolve::BoundAction::Scrub { signal, map: resolve::BrushMap::Continuous { v, .. }, step, bounds, .. } = a else { continue };
                let (lo, hi) = (v.first().copied().unwrap_or(0.0), v.last().copied().unwrap_or(1.0));
                // A bounded thumb (a range's) offers only where it can go.
                let (lo, hi) = (lo.min(hi).max(bounds.0), lo.max(hi).min(bounds.1));
                // The label: the control's own semantics, else the first labelled node inside it.
                let label = label_of(signal);
                let value = match signals.get(signal) {
                    Some(Value::Num(n)) => *n,
                    _ => f64::NAN,
                };
                out.push(Control { kind: "slider", signal: signal.clone(), label, min: lo.min(hi), max: lo.max(hi), step: *step, value, options: Vec::new(), current: serde_json::Value::Null, rect });
            }
        }
        out
    }

    /// The accessible description with each item's bounds in root coordinates (CSS px): what
    /// native accessibility bridges and touch exploration need.
    pub fn semantic_items(&mut self) -> Vec<SemanticItem> {
        let scene = self.scene();
        let actionable: std::collections::BTreeSet<KeyPath> = self.shown.as_ref().map(|r| r.actions.iter().filter(|(_, a)| a.contains_key("activate")).map(|(p, _)| p.clone()).collect()).unwrap_or_default();
        let mut out = Vec::new();
        #[allow(clippy::too_many_arguments)]
        fn walk(n: &datars_scene::Node, path: &KeyPath, depth: usize, parent: datars_math::Affine, acc: f64, act: &std::collections::BTreeSet<KeyPath>, out: &mut Vec<SemanticItem>, budget: &mut usize) {
            // What can't be seen isn't read out (a layer faded out over another).
            let acc = pick::seen(n, acc);
            if acc <= pick::INVISIBLE {
                return;
            }
            let here = path.push(&n.key);
            let xf = parent.mul(n.common.transform);
            let mut d = depth;
            if let Some(s) = &n.semantics {
                if s.role == datars_scene::Role::Decoration {
                    return;
                }
                // Pinned content (a map callout) where it's drawn, at screen size.
                let rect = if n.common.pin { bounds::drawn_bounds(n, parent) } else { bounds::transform_rect(bounds::node_bounds(n), &parent) };
                out.push(SemanticItem { role: serde_json::to_string(&s.role).unwrap_or_default().trim_matches('"').to_string(), label: s.label.clone(), depth, rect, path: format!("{here}"), actionable: act.contains(&here) });
                d += 1;
            }
            let inner = match &n.kind {
                datars_scene::NodeKind::View { viewport, camera, .. } => xf.mul(camera.map(|c| c.transform(*viewport)).unwrap_or(datars_math::Affine::translate(viewport.x, viewport.y))),
                _ => xf,
            };
            for c in n.children() {
                walk(c, &here, d, inner, acc, act, out, budget);
            }
            if let datars_scene::NodeKind::Instances(i) = &n.kind {
                if let Some(ls) = &i.labels {
                    for (k, l) in ls.iter().enumerate().filter(|(k, _)| acc * i.opacity_at(*k) > pick::INVISIBLE) {
                        if *budget == 0 {
                            break;
                        }
                        *budget -= 1;
                        out.push(SemanticItem { role: "datum".into(), label: l.clone(), depth: d, rect: bounds::transform_rect(i.geom(k).bounds(), &xf), path: String::new(), actionable: false });
                    }
                }
            }
        }
        let mut budget = MAX_INSTANCE_ITEMS;
        walk(&scene.root, &KeyPath::default(), 0, datars_math::Affine::IDENTITY, 1.0, &actionable, &mut out, &mut budget);
        out
    }

    /// Run the `activate` intent of the node at `path` (as `SemanticItem::path` spells it) — the
    /// keyboard and screen-reader route to click interactions. Returns whether one ran.
    pub fn activate(&mut self, path: &str) -> bool {
        self.rec(session::Input::Activate { path: path.to_string() });
        let Some(resolved) = self.shown.clone() else { return false };
        let Some(a) = resolved.actions.iter().find(|(p, a)| format!("{p}") == path && a.contains_key("activate")).and_then(|(_, a)| a.get("activate").cloned()) else { return false };
        self.run_action(&a);
        true
    }
}

impl Default for Engine {
    fn default() -> Self {
        Engine::new()
    }
}

/// Re-shapes animated number labels each frame (motion's text hook).
struct TextShaper<'a>(&'a datars_text::FontDb);

impl datars_motion::TextShaper for TextShaper<'_> {
    fn shape(&self, node: &mut datars_scene::TextNode) {
        if let Some(n) = &node.number {
            node.text = datars_text::format::number(n.value, &n.format, &n.locale);
        }
        datars_text::layout(self.0, node);
    }
}

#[cfg(test)]
mod shared_table_tests {
    use super::*;

    const DOC: &str = r#"{"datars": 1,
      "data": { "counts": { "values": { "id": ["a", "b"], "n": [1, 2] }, "key": ["id"] }, "extra": { "url": "extra.json" } },
      "tables": {
        "doubled": { "from": "counts", "ops": [{ "op": "derive", "as": "m", "expr": { "expr": "floor(d.n * 2)" } }] },
        "scaled": { "from": "counts", "ops": [{ "op": "derive", "as": "m", "expr": { "expr": "d.n * k" } }] }
      },
      "signals": { "k": { "type": "num", "default": 1 } },
      "scene": { "kind": "group", "key": "root", "children": [
        { "kind": "repeat", "from": "doubled", "template": { "kind": "shape", "key": "=d.id", "geom": { "type": "rect", "x": "=d.m * 10", "y": 0, "w": 5, "h": 5 } } },
        { "kind": "repeat", "from": "scaled", "template": { "kind": "shape", "key": "=d.id", "geom": { "type": "rect", "x": "=d.m * 10", "y": 10, "w": 5, "h": 5 } } }
      ] },
      "program": { "states": [{ "name": "one", "set": { "k": 1 } }, { "name": "two", "set": { "k": 3 } }] }}"#;

    fn kept(e: &Engine, name: &str) -> Option<Arc<datars_data::Table>> {
        e.tables_cache.borrow().get(name).map(|s| s.table().clone())
    }

    #[test]
    fn pure_derived_tables_are_computed_once_per_data() {
        let mut e = Engine::new();
        e.load(Doc::from_json(DOC).unwrap());
        e.scene();
        let first = kept(&e, "doubled").expect("a table of row fields and built-ins is kept");
        assert!(kept(&e, "scaled").is_none(), "one that reads a signal is computed per resolve");
        e.goto(1);
        e.scene();
        assert!(Arc::ptr_eq(&first, &kept(&e, "doubled").unwrap()), "the next state reuses it");
        // Data it doesn't read arrives: still the same table.
        e.provide("extra", br#"[{"x": 1}]"#).unwrap();
        e.scene();
        assert!(Arc::ptr_eq(&first, &kept(&e, "doubled").unwrap()));
        // Its own source again (a live refresh): computed afresh.
        e.provide("counts", br#"[{"id": "a", "n": 5}]"#).unwrap_or_else(|_| panic!("counts is inline, but a host may replace it"));
        e.scene();
        let again = kept(&e, "doubled").unwrap();
        assert!(!Arc::ptr_eq(&first, &again));
        assert_eq!(again.len(), 1);
    }

    /// A table computed through the plot's scales (rows placed in px) is kept per layout: a hover
    /// or a state that doesn't move the plot doesn't recompute it, one that does gets its own.
    #[test]
    fn tables_through_the_scales_are_kept_per_layout() {
        let doc = r#"{"datars": 1, "size": { "width": 200, "height": 100 },
          "data": { "pts": { "values": { "id": ["a", "b"], "v": [1, 2] }, "key": ["id"] } },
          "tables": {
            "placed": { "from": "pts", "ops": [{ "op": "derive", "as": "px", "expr": { "expr": "scale.x(d.v)" } }] },
            "tinted": { "from": "pts", "ops": [{ "op": "derive", "as": "px", "expr": { "expr": "scale.x(d.v) * k" } }] }
          },
          "signals": { "k": { "type": "num", "default": 1 } },
          "scene": { "kind": "group", "key": "plot", "scales": { "x": { "type": "linear", "domain": [0, 2], "range": "width" } }, "children": [
            { "kind": "repeat", "from": "placed", "template": { "kind": "group", "key": "=d.id", "on": { "activate": { "set": "k", "value": 1 } }, "children": [
              { "kind": "shape", "key": "dot", "pickable": true, "geom": { "type": "rect", "x": "=d.px", "y": 0, "w": 20, "h": 20 }, "fill": "$ink", "opacity": "=hover() ? 1 : 0.5" } ] } },
            { "kind": "repeat", "from": "tinted", "template": { "kind": "shape", "key": "=d.id", "geom": { "type": "rect", "x": "=d.px", "y": 50, "w": 5, "h": 5 } } }
          ] }}"#;
        let placed = |e: &Engine| -> Vec<Arc<datars_data::Table>> { e.tables_cache.borrow().iter().filter(|(k, _)| k.starts_with("placed|")).map(|(_, s)| s.table().clone()).collect() };
        let mut e = Engine::new();
        e.load(Doc::from_json(doc).unwrap());
        e.frame(0.0);
        let first = placed(&e);
        assert_eq!(first.len(), 1, "kept for this layout");
        assert!(!e.tables_cache.borrow().keys().any(|k| k.starts_with("tinted")), "one that also reads a signal isn't");
        // Hovering a dot re-resolves (it asks `hover()`); the table is the one already made.
        e.pointer(Pointer::Move { x: 105.0, y: 10.0 });
        e.frame(1.0);
        let now = placed(&e);
        assert_eq!(now.len(), 1);
        assert!(Arc::ptr_eq(&first[0], &now[0]), "reused across the hover");
        // A resize moves the scale: a layout of its own.
        e.resize(300.0, 100.0, 1.0);
        e.frame(2.0);
        assert_eq!(placed(&e).len(), 2);
    }
}

#[cfg(test)]
mod clock_tests {
    use super::*;

    #[test]
    fn transitions_start_at_the_hosts_clock_after_idling() {
        let doc = r#"{"datars": 1, "signals": {"w": {"type": "num", "default": 10}},
          "scene": {"kind": "shape", "key": "bar", "geom": {"type": "rect", "x": 0, "y": 0, "w": "=w", "h": 20}, "fill": "$accent"},
          "program": {"states": [{"name": "a", "set": {"w": 10}}, {"name": "b", "set": {"w": 90}}]}}"#;
        let mut e = Engine::new();
        e.load(Doc::from_json(doc).unwrap());
        e.frame(0.0);
        // Idle for a minute (no frames), then the reader steps on: the move plays from now.
        e.set_clock(60.0);
        e.event("next");
        assert!(e.frame(60.1).animating, "still moving 0.1 s in");
        assert!(!e.frame(70.0).animating);
        // Without the clock update the step is over by the first frame (long after it).
        e.event("prev");
        assert!(!e.frame(200.0).animating);
    }

    #[test]
    fn a_slow_step_starts_moving_from_its_first_frame() {
        // The step is planned on input; the first frame comes 0.3 s later (planning took that
        // long). It shows the start of the move, not 0.3 s into it.
        let doc = r#"{"datars": 1, "signals": {"w": {"type": "num", "default": 10}},
          "motion": {"rules": [{"duration": 1, "easing": "linear"}]},
          "scene": {"kind": "shape", "key": "bar", "geom": {"type": "rect", "x": 0, "y": 0, "w": "=w", "h": 20}, "fill": "$accent"},
          "program": {"states": [{"name": "a", "set": {"w": 10}}, {"name": "b", "set": {"w": 90}}]}}"#;
        let mut e = Engine::new();
        e.load(Doc::from_json(doc).unwrap());
        e.frame(0.0);
        e.set_clock(5.0);
        e.event("next");
        let w = |f: FrameOutput| f.scene.snapshot().lines().find(|l| l.contains("(\"bar\",)")).map(String::from).unwrap_or_default();
        let first = w(e.frame(5.3));
        assert!(first.contains("w=10"), "the first frame is the start: {first}");
        let half = w(e.frame(5.8));
        assert!(half.contains("w=50"), "half a second after the first frame: {half}");
    }
}
