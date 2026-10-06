use crate::template::Template;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

fn default_format() -> u32 {
    crate::FORMAT_VERSION
}

/// A datars document.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Doc {
    #[serde(default = "default_format")]
    pub datars: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// The authored size; hosts may resize (the `viewport` signal).
    #[serde(default)]
    pub size: Size,
    #[serde(default)]
    pub theme: ThemeRef,
    #[serde(default = "default_locale")]
    pub locale: String,
    /// Code packages (recipes, kernels) by name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packages: Vec<PackageRef>,
    /// Data sources by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub data: BTreeMap<String, Source>,
    /// Derived tables by name: a source table plus a pipeline of operations.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tables: BTreeMap<String, Derived>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub signals: BTreeMap<String, SignalDecl>,
    /// Key metadata: display names and colours per key (party colours are data, not theme).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub keys: BTreeMap<String, KeyMeta>,
    pub scene: Template,
    /// Motion rules (a `datars_motion::MotionRules` value).
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub motion: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<Program>,
}

fn default_locale() -> String {
    "en".into()
}

/// The `tiles` URL that asks for an automatic basemap: the build tools (`datars render`,
/// `publish`, `dev`) cut an archive to the document's own cameras from open data they fetch and
/// cache, so an author never hand-makes one.
pub const AUTO_TILES: &str = "auto";

/// Where a tiles source's archive is read from: its URL, or for [`AUTO_TILES`] the archive the
/// build writes next to the document, `<name>.auto.pmtiles` — relative, like any URL, so every
/// host (headless, `datars serve`, a bundle's folder) finds it the same way.
pub fn tiles_file(name: &str, url: &str) -> String {
    if url == AUTO_TILES {
        format!("{name}.auto.pmtiles")
    } else {
        url.to_string()
    }
}

impl Doc {
    pub fn from_json(s: &str) -> Result<Doc, String> {
        Doc::from_json_checked(s).map(|(d, _)| d)
    }

    /// Parse a document the compiler already checked (a published bundle's): migrated, but not
    /// looked over for ignored fields — that walk costs more than the parse on a document with
    /// inline data, and its notes were for the author at publish time.
    pub fn from_json_trusted(s: &str) -> Result<Doc, String> {
        let mut v: serde_json::Value = serde_json::from_str(s).map_err(|e| e.to_string())?;
        crate::migrate::migrate(&mut v);
        let d: Doc = serde_json::from_value(v).map_err(|e| e.to_string())?;
        if d.datars > crate::FORMAT_VERSION {
            return Err(format!("document format {} is newer than this engine supports ({})", d.datars, crate::FORMAT_VERSION));
        }
        Ok(d)
    }

    /// Parse, migrating older IR shapes first; returns notes on what was migrated and which fields
    /// the IR ignored (typos, unsupported options) — show them to authors.
    pub fn from_json_checked(s: &str) -> Result<(Doc, Vec<String>), String> {
        let mut v: serde_json::Value = serde_json::from_str(s).map_err(|e| e.to_string())?;
        let mut notes: Vec<String> = crate::migrate::migrate(&mut v).into_iter().map(|n| format!("migrated {n}")).collect();
        notes.extend(crate::migrate::ignored_fields(&v).into_iter().map(|p| format!("ignored field `{p}` (not part of the IR — a typo?)")));
        let d: Doc = serde_json::from_value(v).map_err(|e| e.to_string())?;
        if d.datars > crate::FORMAT_VERSION {
            return Err(format!("document format {} is newer than this engine supports ({})", d.datars, crate::FORMAT_VERSION));
        }
        Ok((d, notes))
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

impl Default for Size {
    fn default() -> Self {
        Size { width: 800.0, height: 480.0 }
    }
}

/// Which theme to use, plus the document's own token overrides and inline themes.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThemeRef {
    #[serde(rename = "use", default = "default_theme")]
    pub use_: String,
    /// Inline theme definitions (JSON as in `datars-theme`), registered before resolving `use`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub themes: Vec<serde_json::Value>,
    /// Document-level token overrides (honouring the theme's locks).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tokens: BTreeMap<String, serde_json::Value>,
}

fn default_theme() -> String {
    "datars/neutral".into()
}

impl Default for ThemeRef {
    fn default() -> Self {
        ThemeRef { use_: default_theme(), themes: Vec::new(), tokens: BTreeMap::new() }
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PackageRef {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
    /// Content hash of the package (pins exact behaviour, P1).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub hash: String,
    /// Inline JS source (dev) — published bundles carry bytecode chunks instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// A data source. The engine never fetches (P10): `url` sources become requests the host fulfils.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Source {
    #[serde(flatten)]
    pub from: SourceKind,
    /// Key column(s): the identity of each row (P3).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub key: Vec<String>,
    /// Column type overrides: `"num" | "str" | "bool" | "date"`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub types: BTreeMap<String, String>,
    /// Live updates: how the host should refresh this source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live: Option<LiveDecl>,
    /// Where a JSON response keeps its rows: a dotted path to the array of records (`data`,
    /// `results.items`) — web APIs wrap their rows with paging and metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<String>,
    /// Geo sources: the feature property holding the id (default `id`, else the feature's own id).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Slots: rows shown until the host provides the data (design previews, tests, a published
    /// chart's static fallback), as columns or records. They also say which columns the host's
    /// data must have.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample: Option<serde_json::Value>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    /// Inline columns `{ "col": [values…] }` or records `[{…}, …]`.
    Values(serde_json::Value),
    /// Inline CSV text.
    Csv(String),
    /// A URL the host fetches (CSV or JSON by extension or `format`).
    Url(String),
    /// Provided by the host at runtime (a data slot, docs/12-delivery.md).
    Slot(String),
    /// Inline GeoJSON (a FeatureCollection): a geo source — a table of feature ids + properties,
    /// and geometry the engine projects through a `geo` coordinate system.
    Geojson(serde_json::Value),
    /// Inline TopoJSON (shared borders simplify consistently).
    Topojson(serde_json::Value),
    /// A built-in atlas (`"countries"`, `"admin1"`, …) from the runtime or an atlas package.
    Atlas(String),
    /// A vector-tile archive (PMTiles v3) by URL. Never fetched whole: the engine asks the host for
    /// byte ranges — the header and directories, then the tiles the visible views need at their
    /// zoom (`Request::Range`). Drawn by `tiles` template nodes. [`AUTO_TILES`] (`"auto"`) asks the
    /// build for an archive cut to the document's own cameras ([`tiles_file`]).
    Tiles(String),
    /// A font file (TrueType/OpenType) by URL, fetched like any source: its families join every
    /// text's fallback chain, so labels in other scripts (Hebrew, Arabic, CJK) find their glyphs.
    Font(String),
    /// Rows the engine generates: synthetic data too big to write down (simulations, load tests,
    /// procedural datasets) as a few expressions instead of millions of values.
    Generate(Generate),
}

/// A generated table: `rows` rows; each column an expression over the row, evaluated in order —
/// `d.i` is the row number and `d.<name>` any column before it — with `rand(key, stream)` and
/// `randn(key, stream)` for seeded randomness (the same rows on every platform). `keep` names
/// the columns the table ends up with (default: all); the others are working values.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Generate {
    pub rows: u64,
    pub columns: Vec<GenColumn>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keep: Vec<String>,
}

/// One generated column: its name and its expression.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GenColumn {
    #[serde(rename = "as")]
    pub name: String,
    pub expr: String,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LiveDecl {
    /// Seconds between refreshes (poll), or 0 for push.
    #[serde(default)]
    pub every: f64,
    /// `snapshot` (absent keys exit) or `upsert`/`append`.
    #[serde(default = "snapshot")]
    pub mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<usize>,
}

fn snapshot() -> String {
    "snapshot".into()
}

/// A derived table: `from` + ops, each op a small JSON object interpreted by the engine
/// (`filter`, `derive`, `aggregate`, `sort`, `top`, `bin`, `window`, `join`, `pivot`, `unpivot`,
/// `interpolate`, and layout algorithms like `stack`, `pie`, `treemap`, `beeswarm`, …).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Derived {
    pub from: String,
    #[serde(default)]
    pub ops: Vec<serde_json::Value>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SignalDecl {
    /// `num | str | bool | key | keyset | range | point | json`
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub default: serde_json::Value,
    /// A host-bound control, if any (`slider`, `toggle`, `select`) with its parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<serde_json::Value>,
    /// A clock signal: its value runs at this rate (units per second) while a settled scene reads
    /// it — a globe that turns, a slow drift — and pauses during transitions, off screen and for
    /// reduced motion. Resolving outside a running view (bake, render, goldens) sees its default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock: Option<f64>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct KeyMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// A program: a small statechart (docs/08-programs.md).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Program {
    /// Preset name for tooling (`story`, `dashboard`, `interactive`, `film`, `loop`, `static`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub preset: String,
    pub states: Vec<State>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<Edge>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub initial: String,
    /// Drivers: `steps`, `scroll` (`{"scroll": "scrub" | "trigger"}`), `autoplay`, `keys`, `timer`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drivers: Vec<serde_json::Value>,
    /// Parameterized sub-programs entered by an event with a key (chapters, drill-down).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub chapters: BTreeMap<String, Chapter>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub name: String,
    /// Signal assignments while in this state (values or `{"expr"}`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub set: BTreeMap<String, serde_json::Value>,
    /// Seconds to hold in autoplay/film.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold: Option<f64>,
    /// Narration for the host (cards, captions, video subtitles) and an anchor key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub narration: Option<Narration>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Narration {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    /// `next`, `prev`, `goto:<state>`, `activate`, `data`, `timer`, or a custom event name.
    pub on: String,
    pub to: String,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chapter {
    /// The parameter bound to the activating datum's key (`{c}` in names, a signal in expressions).
    pub param: String,
    pub program: Box<Program>,
}
