//! The scene template: a tree the engine resolves into concrete scene nodes. Literal where static,
//! expressions where data-driven, `use` nodes where a recipe from a package expands.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A property value: a literal JSON value, or an expression (`{"expr": "…"}` or `"=…"`).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Prop(pub serde_json::Value);

impl Prop {
    pub fn num(v: f64) -> Prop {
        Prop(serde_json::json!(v))
    }
    pub fn str(s: &str) -> Prop {
        Prop(serde_json::Value::String(s.into()))
    }
    pub fn expr(src: &str) -> Prop {
        Prop(serde_json::json!({ "expr": src }))
    }
    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }
    /// The expression source, if this prop is an expression.
    pub fn as_expr(&self) -> Option<&str> {
        match &self.0 {
            serde_json::Value::String(s) if s.starts_with('=') => Some(&s[1..]),
            serde_json::Value::Object(o) => o.get("expr").and_then(|e| e.as_str()),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        self.0.as_f64()
    }
    pub fn as_str(&self) -> Option<&str> {
        match &self.0 {
            serde_json::Value::String(s) if !s.starts_with('=') => Some(s),
            _ => None,
        }
    }
}

impl From<f64> for Prop {
    fn from(v: f64) -> Prop {
        Prop::num(v)
    }
}
impl From<&str> for Prop {
    fn from(s: &str) -> Prop {
        Prop::str(s)
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Template {
    /// Stable id for patching and provenance (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The node's key: literal (`"plot"`, `["S", 3]`) or an expression over the current datum.
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub key: Prop,
    #[serde(flatten)]
    pub kind: TKind,
    /// Include only when this evaluates truthy.
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub when: Prop,
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub opacity: Prop,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<TTransform>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z: Option<i32>,
    /// `"box"` clips to this node's layout box; or a rect `[x, y, w, h]`.
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub clip: Prop,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trim: Option<[Prop; 2]>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub isolate: bool,
    /// Pinned: the origin follows cameras and scales above, the content keeps screen size. A
    /// text's origin is its `at` (a place name stays at its map point).
    #[serde(default, skip_serializing_if = "is_false")]
    pub pin: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics: Option<TSemantics>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub pickable: bool,
    /// Intent → action bindings (`inspect`, `activate`, `pan`, `zoom`, `brush`, `drag`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub on: BTreeMap<String, Action>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anchors: Vec<TAnchor>,
    /// How this node lays out its children in its box.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<Layout>,
    /// This node's size inside its parent's layout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<SizeSpec>,
    /// A box drawn behind a group's content, sized to it after layout (cards, legends, tooltips).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backdrop: Option<TBackdrop>,
    /// Places the group's content, once the whole scene is laid out, at the first of these anchors
    /// in its box (`top-left`, `top`, `top-right`, `left`, `center`, `right`, `bottom-left`,
    /// `bottom`, `bottom-right`; inset by the layout padding) where it covers the least data and
    /// text — a card that never hides the bars it talks about. An anchor may be an expression (a
    /// card that moves between states). A single anchor is a fixed place: the content goes there
    /// whatever it covers, and never takes a band (a card over a map, where land covers everything).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dodge: Vec<Prop>,
    /// Keeps the texts inside this group from landing on each other, once the whole scene is laid
    /// out: in order, a text that would overlap one before it (or any other text) tries the other
    /// side of its point — below instead of above, left instead of right, when it's offset from it
    /// — and is left out when there's no room on either side. Place names on a map that crowd
    /// together in a phone's narrow box; the first ones win, so list the important ones first.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub declutter: bool,
    /// Scales declared here, visible to descendants by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub scales: BTreeMap<String, ScaleDecl>,
    /// A coordinate system for descendants (`cartesian`, `polar`, `geo`, `planar`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coord: Option<serde_json::Value>,
    /// Provenance note (set by recipe expansion: `"@datars/std/bar"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prov: Option<String>,
    /// Motion defaults for this node and everything under it: a list of motion rules (as in the
    /// document's `motion.rules`) scoped to the node's key path. Recipe expansion puts a recipe's
    /// own defaults here (a line draws on, grouped bars grow). They sit under the document's
    /// rules whatever their specificity: the document always has the last word.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion: Option<serde_json::Value>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum TKind {
    Group {
        #[serde(default)]
        children: Vec<Template>,
    },
    /// A viewport with a camera. Clips to its box unless the template says `clip: false`.
    View {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        camera: Option<TCamera>,
        #[serde(default)]
        children: Vec<Template>,
    },
    Shape {
        geom: TGeom,
        #[serde(default, skip_serializing_if = "Prop::is_null")]
        fill: Prop,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke: Option<TStroke>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        markers: Option<serde_json::Value>,
    },
    Text(TText),
    /// One child per row (or tick, legend entry, group, number).
    Repeat {
        from: RepeatFrom,
        template: Box<Template>,
    },
    /// Vectorized marks: one prototype × the rows of a table.
    Instances(TInstances),
    /// A recipe from a package, expanded by the engine's sandbox.
    Use {
        recipe: String,
        #[serde(default)]
        params: serde_json::Value,
    },
    Image {
        asset: String,
        rect: [Prop; 4],
    },
    /// Features of a vector-tile source, streamed for what the enclosing view shows: the engine
    /// picks the tiles covering the view at a zoom matching the camera, and draws each tile layer's
    /// features through a template in the enclosing `geo` coordinate system. Resolved late — per
    /// frame, from the (possibly moving) camera — so a flight requests tiles along its path.
    Tiles(TTiles),
}

impl Default for TKind {
    fn default() -> Self {
        TKind::Group { children: Vec::new() }
    }
}


#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TTransform {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translate: Option<[Prop; 2]>,
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub rotate: Prop,
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub scale: Prop,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum TGeom {
    Rect {
        x: Prop,
        y: Prop,
        w: Prop,
        h: Prop,
        #[serde(default, skip_serializing_if = "Prop::is_null")]
        r: Prop,
    },
    Ellipse { cx: Prop, cy: Prop, rx: Prop, ry: Prop },
    Circle { cx: Prop, cy: Prop, r: Prop },
    Arc {
        cx: Prop,
        cy: Prop,
        #[serde(default, skip_serializing_if = "Prop::is_null")]
        r0: Prop,
        r1: Prop,
        a0: Prop,
        a1: Prop,
    },
    Segment { x1: Prop, y1: Prop, x2: Prop, y2: Prop },
    /// Points from the rows of `from` (a table name, or `"@group"` for the current group's rows).
    Polyline {
        from: String,
        x: Prop,
        y: Prop,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        curve: Option<String>,
        #[serde(default)]
        closed: bool,
    },
    Area {
        from: String,
        x: Prop,
        y0: Prop,
        y1: Prop,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        curve: Option<String>,
    },
    /// An SVG path string (literal or expression).
    Path { d: Prop },
    Symbol {
        #[serde(default, skip_serializing_if = "Prop::is_null")]
        symbol: Prop,
        x: Prop,
        y: Prop,
        size: Prop,
    },
    /// A feature of a geo source, projected through the enclosing geo coordinate system. Without
    /// `source` (inside a `tiles` layer template): the current tile feature's geometry.
    Feature {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        source: String,
        #[serde(default, skip_serializing_if = "Prop::is_null")]
        id: Prop,
    },
}

/// A group's backdrop: a (rounded) box behind whatever the group draws, sized to it once layout is
/// done — so a card grows with its text, and morphs when the text changes. Keyed `backdrop`.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TBackdrop {
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub fill: Prop,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<TStroke>,
    /// Corner radius (px or a number token).
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub radius: Prop,
    /// Space around the content: a number, or `[top, right, bottom, left]`.
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub padding: Prop,
    /// `content` (default) hugs the content; `width` spans the group's box across and hugs the
    /// content's height (a card of fixed width).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit: Option<String>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TStroke {
    pub paint: Prop,
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub width: Prop,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dash: Option<Vec<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cap: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub join: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub non_scaling: bool,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TText {
    pub text: Prop,
    pub at: [Prop; 2],
    #[serde(default)]
    pub style: TTextStyle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub halo: Option<[Prop; 2]>,
    /// A number that counts when animated: `{ "value": expr, "format": ",.1f" }`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<TNumber>,
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub rotate: Prop,
    /// Shift from `at` in screen px (labels beside points under a zooming camera).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<[Prop; 2]>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TNumber {
    pub value: Prop,
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub format: Prop,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TTextStyle {
    /// A font token (`"font.body"`) or a family name.
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub font: Prop,
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub size: Prop,
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub weight: Prop,
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub ink: Prop,
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub align: Prop,
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub baseline: Prop,
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub max_width: Prop,
    /// Keep the label inside the canvas: after layout it's nudged back in, only as far as needed
    /// (the last tick label of an axis, a callout near the edge). A boolean.
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub contain: Prop,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RepeatFrom {
    /// Rows of a table (`"votes"`, or `"@group"`).
    Table(String),
    /// One child per group of rows: `{ "groups": "points", "by": "series" }`; inside, `"@group"` is
    /// that group's rows and `d` its first row.
    Groups { groups: String, by: String },
    /// One child per tick of a scale: `{ "ticks": "y", "count": 5 }`; `d.value`, `d.label`. The
    /// count is a hint (0 or none: from the axis length), and may be an expression — so many ticks
    /// as labels of their measured width fit along the axis.
    Ticks {
        ticks: String,
        #[serde(default, skip_serializing_if = "Prop::is_null")]
        count: Prop,
    },
    /// One child per entry of a categorical/colour scale's domain: `d.value`, `d.ink`, `d.index`.
    Legend { legend: String },
    /// `n` children with `d.index`.
    Count { count: Prop },
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TInstances {
    pub from: String,
    /// `circle | square | diamond | triangle | cross | star | rect`
    #[serde(default = "circle")]
    pub proto: String,
    /// Per-instance key (an expression over the row). Separate from the node's own `key`: the
    /// instances node keeps one identity, each instance another.
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub instance_key: Prop,
    pub x: Prop,
    pub y: Prop,
    /// Radius (symbol prototypes), px; default: the theme's `point.radius`. Not `size`, which is
    /// the node's layout size.
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub r: Prop,
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub w: Prop,
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub h: Prop,
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub fill: Prop,
    /// Per-instance opacity (an expression over the row); the node's own `opacity` multiplies it.
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub instance_opacity: Prop,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<TStroke>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub screen_size: bool,
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub label: Prop,
    /// Where the pointer finds the instances: `marks` (the default), each mark and a few px around
    /// it; or `line`, anywhere along the line through them in row order, within `reach` px of it,
    /// finding the instance nearest to where it meets the line — a line's value wherever it's
    /// hovered or tapped, however far apart its points are.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hit: Option<String>,
    /// How far from the line (`hit: line`) the pointer still finds it, px; default 16.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reach: Option<f64>,
    /// Level of detail, for rows beyond what one frame can draw (millions): the rows are indexed
    /// once into a pyramid of tiles and each frame draws only the tiles the camera shows — a
    /// seeded, density-preserving sample of the rows in view, as many as `points` allows, every
    /// row once they fit. Rows sit at their `x`/`y` in the node's own coordinates (indexed once, so
    /// they read the row only); a transform or a view's camera above maps them to the screen.
    /// `from` may also name a point archive (a `tiles` source written by the publish compiler).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lod: Option<TLod>,
}

/// How an `instances` node with `lod` samples its rows.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TLod {
    /// The most rows one frame draws (default 150,000): a view holding more draws a uniform
    /// sample of them this size, a view holding fewer draws every one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub points: Option<f64>,
    /// Rows per tile of the index at average density (default 2048): how finely the rows are
    /// cut up (smaller: more, lighter tiles to fetch).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<f64>,
}

fn circle() -> String {
    "circle".into()
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TSemantics {
    pub role: String,
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub label: Prop,
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub value: Prop,
    /// A URL this element links to (a data credit's licence page, a source): hosts make it
    /// clickable and announce it as a link; vector exports keep it.
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub link: Prop,
}

/// A `tiles` node: which source, and how each tile layer's features become scene nodes.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TTiles {
    /// A data source of kind `tiles`.
    pub source: String,
    /// Drawn in order (later layers on top); label layers go above every tile.
    #[serde(default)]
    pub layers: Vec<TTileLayer>,
    /// The on-screen size (px) a tile should cover; the zoom follows from it. Default 512, the
    /// size tile data is simplified for. Smaller: more detail and more tiles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile_size: Option<f64>,
}

/// One layer of a `tiles` node. The template is resolved once per feature (and cached per tile);
/// inside it `d` is the feature — its properties plus `$type` (`point | line | polygon`), `$id`,
/// and `$x`/`$y` (its anchor point) — the signal `tile.zoom` is the zoom being drawn, and the
/// geometry `{"type": "feature"}` is the feature's own.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TTileLayer {
    /// The tile layer's name in the archive.
    pub layer: String,
    /// Names the layer's nodes (default: `layer`); unique within the `tiles` node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Features to draw (an expression over `d`); all when absent.
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub filter: Prop,
    /// Zoom range `[minzoom, maxzoom)` in which the layer draws.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minzoom: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maxzoom: Option<f64>,
    pub template: Box<Template>,
    /// Batch features whose resolved style is identical into one node per tile (default true):
    /// thousands of roads become a handful of paths. Off: one keyed node per feature.
    #[serde(default = "yes")]
    pub merge: bool,
    /// Labels: drawn above all tiles, placed in screen space in `priority` order (highest first),
    /// dropping any that would overlap one already placed.
    #[serde(default, skip_serializing_if = "is_false")]
    pub labels: bool,
    #[serde(default, skip_serializing_if = "Prop::is_null")]
    pub priority: Prop,
}

fn yes() -> bool {
    true
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TAnchor {
    pub name: String,
    pub at: [Prop; 2],
}

/// What an intent does: set or toggle a signal, fire a program event, or enter a chapter.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Action {
    /// One value for `pick` among `options` (each said by the matching `labels` entry, else by
    /// itself): what a host offers with its own picker — the platform's select on touch screens —
    /// in place of the chart's drawn list. No pointer intent runs it.
    Pick {
        pick: String,
        options: Vec<serde_json::Value>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        labels: Vec<Prop>,
    },
    Set { set: String, value: Prop },
    Toggle { toggle: String, value: Prop },
    Event { event: String },
    Chapter { chapter: String, key: Prop },
    /// Drag across the node to select a range along a scale: `axis` `x` or `y`, the scale of that
    /// name unless `scale` says otherwise. Continuous scales write `<brush>.lo` / `<brush>.hi`
    /// (data units); band scales write the keyset `<brush>` of the bands covered. Both set
    /// `<brush>.active`; a click without a drag clears the brush.
    /// Press or drag on the node to set `scrub` to the data value under the pointer along `axis`
    /// (through the scale of that name, or `scale`) — sliders, time scrubbers.
    Scrub {
        scrub: String,
        #[serde(default = "x_axis")]
        axis: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scale: Option<String>,
        /// Snap to multiples of this step (0 = continuous).
        #[serde(default, skip_serializing_if = "is_zero")]
        step: f64,
        /// Keep the value at or above / below these (numbers or expressions, read when the scene
        /// resolves): a range's low thumb stops at the high one's value.
        #[serde(default, skip_serializing_if = "Prop::is_null")]
        min: Prop,
        #[serde(default, skip_serializing_if = "Prop::is_null")]
        max: Prop,
    },
    Brush {
        brush: String,
        #[serde(default = "x_axis")]
        axis: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scale: Option<String>,
    },
}

fn x_axis() -> String {
    "x".into()
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TCamera {
    /// Fit keys (node keys under the view) or a bbox in content units, with padding px.
    Fit {
        fit: serde_json::Value,
        #[serde(default)]
        padding: f64,
        /// Free exploration (drag to pan, wheel/pinch to zoom) relative to the fit, kept in the
        /// signals `<explore>.x`, `.y` (content centre) and `.zoom` (× the fit's zoom).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        explore: Option<String>,
        /// How far exploration zooms in, × the fit's zoom (default 64).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_zoom: Option<f64>,
        /// How far it zooms out, × the fit's zoom (default 0.5).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min_zoom: Option<f64>,
    },
    Explicit { x: Prop, y: Prop, zoom: Prop },
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    /// `stack` (default: children share the box), `rows`, `columns`, `grid` (equal cells, `columns`
    /// across; a repeat fills one cell per item), or `flow` (resolved nodes left to right by their
    /// size, wrapping at the box edge; a repeat's nodes flow too).
    #[serde(rename = "type", default = "stack")]
    pub ty: String,
    #[serde(default)]
    pub gap: f64,
    /// Padding `[top, right, bottom, left]`; CSS shorthand accepted on input: `12`, `[12]`,
    /// `[vertical, horizontal]`, `[top, horizontal, bottom]`.
    #[serde(default, deserialize_with = "padding_shorthand")]
    #[cfg_attr(feature = "schema", schemars(with = "PaddingInput"))]
    pub padding: [f64; 4],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<usize>,
    /// Cross-axis alignment: `start | center | end | stretch` (default stretch).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    /// `columns` only: in a box narrower than this (px), the children stack as rows instead, each
    /// an equal share of the height (their widths dropped) — two charts side by side on a desktop,
    /// one above the other on a phone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap: Option<f64>,
}

fn stack() -> String {
    "stack".into()
}

/// A size along one axis: px, `"auto"` (measured content), `"fill"` / `{"fill": weight}`, `"30%"`,
/// or an expression over the parent box in px (`{"expr": "min(260, box.w - 24)"}`).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SizeSpec {
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub w: serde_json::Value,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub h: serde_json::Value,
}

/// A scale declaration.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScaleDecl {
    /// `linear | log | sqrt | pow | symlog | time | band | point | ordinal | quantize | quantile |
    /// threshold | sequential | diverging | categorical | piecewise`
    #[serde(rename = "type")]
    pub ty: String,
    /// `{"data": table, "field": col}`, `{"data", "expr"}`, a literal array, or `{"values": [...]}`.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub domain: serde_json::Value,
    /// `"width"`, `"height"`, `"-height"` (inverted), `[a, b]` (numbers or expressions), a palette
    /// token (`"$categorical"`), or a list of inks.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub range: serde_json::Value,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub zero: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub nice: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub padding: Option<f64>,
    /// `pow` only: values map through `sign(x)·|x|^exponent` (default 1, linear; `sqrt` is pow 0.5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exponent: Option<f64>,
    /// Further type-specific parameters (`mid`, `stops`, `thresholds`, `clamp`, …).
    #[serde(flatten)]
    pub params: BTreeMap<String, serde_json::Value>,
}

/// CSS-style padding: one number, or 1–4 numbers in top/right/bottom/left order.
fn padding_shorthand<'de, D: serde::Deserializer<'de>>(d: D) -> Result<[f64; 4], D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum P {
        One(f64),
        Many(Vec<f64>),
    }
    Ok(match P::deserialize(d)? {
        P::One(v) => [v; 4],
        P::Many(v) => match v.as_slice() {
            [a] => [*a; 4],
            [a, b] => [*a, *b, *a, *b],
            [a, b, c] => [*a, *b, *c, *b],
            [a, b, c, e, ..] => [*a, *b, *c, *e],
            [] => [0.0; 4],
        },
    })
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

/// What `padding` accepts on input (CSS shorthand): one number, or 1–4 numbers.
#[cfg(feature = "schema")]
#[derive(schemars::JsonSchema)]
#[schemars(untagged)]
#[allow(dead_code)]
enum PaddingInput {
    All(f64),
    Sides(Vec<f64>),
}
