use crate::geom::{Geom, SymbolKind};
use crate::key::{Key, KeyPath, Sym};
use crate::paint::{Paint, Stroke};
use crate::text::TextNode;
use datars_math::{Affine, PathData, Rect, Vec2};
use datars_theme::Ink;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Blend {
    #[default]
    Normal,
    Multiply,
    Screen,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Clip {
    Rect { rect: Rect },
    Path { path: Arc<PathData> },
}

/// Nodes whose `z` is at least this float: they're drawn — and picked — above the whole scene,
/// whatever their place in the tree and outside their ancestors' clips (a menu's list over the
/// chart below it, a popover). Among floating nodes a higher `z` is on top, then tree order. Below
/// it, `z` orders siblings only.
pub const FLOAT_Z: i32 = 1000;

impl Common {
    /// Drawn and picked above the whole scene ([`FLOAT_Z`]).
    pub fn floats(&self) -> bool {
        self.z >= FLOAT_Z
    }
}

/// Properties every node has.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Common {
    #[serde(default, skip_serializing_if = "Affine::is_identity")]
    pub transform: Affine,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub opacity: f64,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub visible: bool,
    #[serde(default, skip_serializing_if = "is_zero_i")]
    pub z: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip: Option<Clip>,
    /// Draw only this portion [t0, t1] of the geometry's length (line reveals, tracks).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trim: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "is_normal")]
    pub blend: Blend,
    /// Composite children as one layer (exact group opacity).
    #[serde(default, skip_serializing_if = "is_false")]
    pub isolate: bool,
    /// Pinned: this node's origin follows the transforms and cameras above it, but its content
    /// is drawn at screen size, unrotated (map markers and callouts that stay legible at any zoom).
    #[serde(default, skip_serializing_if = "is_false")]
    pub pin: bool,
}

fn one() -> f64 {
    1.0
}
fn yes() -> bool {
    true
}
fn is_one(v: &f64) -> bool {
    *v == 1.0
}
fn is_true(v: &bool) -> bool {
    *v
}
fn is_false(v: &bool) -> bool {
    !*v
}
fn is_zero_i(v: &i32) -> bool {
    *v == 0
}
fn is_normal(b: &Blend) -> bool {
    *b == Blend::Normal
}

impl Default for Common {
    fn default() -> Self {
        Common { transform: Affine::IDENTITY, opacity: 1.0, visible: true, z: 0, clip: None, trim: None, blend: Blend::Normal, isolate: false, pin: false }
    }
}

impl Common {
    pub fn is_default(&self) -> bool {
        *self == Common::default()
    }

    /// The transform this node's content is drawn with under `parent`: `parent · transform`, or,
    /// pinned, only the translation to where that puts the node's origin.
    pub fn placed(&self, parent: Affine) -> Affine {
        let xf = parent.mul(self.transform);
        if self.pin {
            let o = xf.apply(datars_math::Vec2::ZERO);
            Affine::translate(o.x, o.y)
        } else {
            xf
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    #[default]
    Group,
    Datum,
    Series,
    Region,
    Axis,
    Tick,
    Grid,
    Legend,
    LegendItem,
    Annotation,
    Title,
    Label,
    Tooltip,
    Control,
    Decoration,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Semantics {
    pub role: Role,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    /// The data row this node shows: `"table#row"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub datum: Option<Sym>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    /// A URL the element links to (hosts make it clickable; vector exports keep it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<Sym>,
}

impl Semantics {
    pub fn new(role: Role, label: impl Into<String>) -> Semantics {
        Semantics { role, label: label.into(), ..Default::default() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Anchor {
    pub name: Sym,
    pub at: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Marker {
    Arrow { size: f64 },
    Dot { r: f64 },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Markers {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<Marker>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<Marker>,
}

/// What a `View` shows: the content point at the viewport's centre, zoom, rotation.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
    #[serde(default)]
    pub rotation: f64,
}

impl Camera {
    /// The camera that shows content coordinates unchanged in `viewport`.
    pub fn identity_for(viewport: Rect) -> Camera {
        Camera { x: viewport.w / 2.0, y: viewport.h / 2.0, zoom: 1.0, rotation: 0.0 }
    }
    /// Content → viewport-parent transform.
    pub fn transform(&self, viewport: Rect) -> Affine {
        Affine::translate(-self.x, -self.y)
            .then(Affine::rotate(self.rotation))
            .then(Affine::scale(self.zoom, self.zoom))
            .then(Affine::translate(viewport.x + viewport.w / 2.0, viewport.y + viewport.h / 2.0))
    }
    /// The camera that fits `content` into `viewport` with `padding` px.
    pub fn fit(content: Rect, viewport: Rect, padding: f64) -> Camera {
        let c = content.center();
        let zx = (viewport.w - 2.0 * padding).max(1.0) / content.w.max(1e-9);
        let zy = (viewport.h - 2.0 * padding).max(1.0) / content.h.max(1e-9);
        Camera { x: c.x, y: c.y, zoom: zx.min(zy), rotation: 0.0 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Proto {
    Symbol { symbol: SymbolKind },
    /// Rectangles: per-instance `w`/`h` columns; `x`/`y` are the top-left corner.
    Rect,
}

/// One prototype × N keyed instances with per-instance columns (the path to 10⁶ marks).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Instances {
    pub proto: Proto,
    pub keys: Vec<Key>,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    /// Radius for symbols; ignored for rects.
    #[serde(default)]
    pub size: Vec<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<Vec<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<Vec<f64>>,
    pub fill: Vec<Ink>,
    #[serde(default)]
    pub opacity: Vec<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Stroke>,
    /// Sizes stay in screen px under a zooming camera.
    #[serde(default)]
    pub screen_size: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
    /// Hit as the line through the instances in order, within this many px of it (finding the
    /// instance nearest to where the pointer meets the line); `None`: each mark and a few px around
    /// it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_reach: Option<f64>,
}

impl Instances {
    pub fn len(&self) -> usize {
        self.keys.len()
    }
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
    pub fn opacity_at(&self, i: usize) -> f64 {
        self.opacity.get(i).copied().unwrap_or(1.0)
    }
    pub fn size_at(&self, i: usize) -> f64 {
        self.size.get(i).copied().unwrap_or(3.0)
    }
    pub fn fill_at(&self, i: usize) -> Ink {
        self.fill.get(i).cloned().or_else(|| self.fill.first().cloned()).unwrap_or_default()
    }
    /// The instances at `idx`, in that order (per-instance columns taken; a column holding one
    /// value for all — a single fill — stays one value).
    pub fn take(&self, idx: &[usize]) -> Instances {
        fn col<T: Clone>(v: &[T], idx: &[usize]) -> Vec<T> {
            if v.len() <= 1 {
                return v.to_vec();
            }
            idx.iter().filter_map(|&i| v.get(i).cloned()).collect()
        }
        Instances {
            proto: self.proto.clone(),
            keys: idx.iter().map(|&i| self.keys[i].clone()).collect(),
            x: idx.iter().map(|&i| self.x[i]).collect(),
            y: idx.iter().map(|&i| self.y[i]).collect(),
            size: col(&self.size, idx),
            w: self.w.as_ref().map(|w| col(w, idx)),
            h: self.h.as_ref().map(|h| col(h, idx)),
            fill: col(&self.fill, idx),
            opacity: col(&self.opacity, idx),
            stroke: self.stroke.clone(),
            screen_size: self.screen_size,
            labels: self.labels.as_ref().map(|l| col(l, idx)),
            line_reach: self.line_reach,
        }
    }
    /// Several instance sets as one, in order (all with the same prototype and stroke; a column
    /// holding one value for all is spread per instance where the parts differ).
    pub fn concat(parts: &[&Instances]) -> Instances {
        let Some(first) = parts.first() else { return Instances { proto: Proto::Rect, keys: Vec::new(), x: Vec::new(), y: Vec::new(), size: Vec::new(), w: None, h: None, fill: Vec::new(), opacity: Vec::new(), stroke: None, screen_size: false, labels: None, line_reach: None } };
        // Per-instance values (a part's single shared value spread over its instances); empty
        // when every part leaves the column empty (the default applies).
        fn col<T: Clone>(parts: &[&Instances], get: impl Fn(&Instances) -> &Vec<T>, fallback: impl Fn(&Instances, usize) -> T) -> Vec<T> {
            if parts.iter().all(|p| get(p).is_empty()) {
                return Vec::new();
            }
            let mut out = Vec::with_capacity(parts.iter().map(|p| p.len()).sum());
            for p in parts {
                let v = get(p);
                out.extend((0..p.len()).map(|i| v.get(i).cloned().unwrap_or_else(|| fallback(p, i))));
            }
            out
        }
        Instances {
            proto: first.proto.clone(),
            keys: parts.iter().flat_map(|p| p.keys.iter().cloned()).collect(),
            x: parts.iter().flat_map(|p| p.x.iter().copied()).collect(),
            y: parts.iter().flat_map(|p| p.y.iter().copied()).collect(),
            size: col(parts, |p| &p.size, |p, i| p.size_at(i)),
            w: first.w.as_ref().map(|_| parts.iter().flat_map(|p| p.w.clone().unwrap_or_default()).collect()),
            h: first.h.as_ref().map(|_| parts.iter().flat_map(|p| p.h.clone().unwrap_or_default()).collect()),
            fill: col(parts, |p| &p.fill, |p, i| p.fill_at(i)),
            opacity: col(parts, |p| &p.opacity, |p, i| p.opacity_at(i)),
            stroke: first.stroke.clone(),
            screen_size: first.screen_size,
            labels: first.labels.as_ref().map(|_| parts.iter().flat_map(|p| p.labels.clone().unwrap_or_default()).collect()),
            line_reach: first.line_reach,
        }
    }
    /// The geometry of instance `i` in local units.
    pub fn geom(&self, i: usize) -> Geom {
        match &self.proto {
            Proto::Symbol { symbol } => Geom::Symbol { kind: *symbol, x: self.x[i], y: self.y[i], size: self.size_at(i) },
            Proto::Rect => Geom::rect(
                self.x[i],
                self.y[i],
                self.w.as_ref().and_then(|w| w.get(i)).copied().unwrap_or(1.0),
                self.h.as_ref().and_then(|h| h.get(i)).copied().unwrap_or(1.0),
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum NodeKind {
    Group {
        #[serde(default)]
        children: Vec<Node>,
    },
    View {
        viewport: Rect,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        camera: Option<Camera>,
        #[serde(default = "yes")]
        clip: bool,
        #[serde(default)]
        children: Vec<Node>,
    },
    Shape {
        geom: Geom,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fill: Option<Paint>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke: Option<Stroke>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        markers: Option<Markers>,
    },
    Text(TextNode),
    Image {
        asset: Sym,
        rect: Rect,
    },
    /// Shared: a frame, a hit test and a filled tile cache all hold the same columns (clones are
    /// cheap however many instances there are; edits copy on write).
    Instances(Arc<Instances>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub key: Key,
    #[serde(flatten)]
    pub kind: NodeKind,
    #[serde(default, skip_serializing_if = "Common::is_default")]
    pub common: Common,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics: Option<Semantics>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub pickable: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anchors: Vec<Anchor>,
    /// Provenance: an index into the scene's provenance table (0 = unknown).
    #[serde(default, skip_serializing_if = "is_zero_u")]
    pub prov: u32,
}

fn is_zero_u(v: &u32) -> bool {
    *v == 0
}

impl Node {
    pub fn new(key: Key, kind: NodeKind) -> Node {
        Node { key, kind, common: Common::default(), semantics: None, pickable: false, anchors: Vec::new(), prov: 0 }
    }
    pub fn group(key: Key, children: Vec<Node>) -> Node {
        Node::new(key, NodeKind::Group { children })
    }
    pub fn shape(key: Key, geom: Geom) -> Node {
        Node::new(key, NodeKind::Shape { geom, fill: None, stroke: None, markers: None })
    }
    pub fn text(key: Key, t: TextNode) -> Node {
        Node::new(key, NodeKind::Text(t))
    }
    pub fn fill(mut self, p: impl Into<Paint>) -> Node {
        if let NodeKind::Shape { fill, .. } = &mut self.kind {
            *fill = Some(p.into());
        }
        self
    }
    pub fn stroke(mut self, s: Stroke) -> Node {
        if let NodeKind::Shape { stroke, .. } = &mut self.kind {
            *stroke = Some(s);
        }
        self
    }
    pub fn opacity(mut self, o: f64) -> Node {
        self.common.opacity = o;
        self
    }
    pub fn transform(mut self, t: Affine) -> Node {
        self.common.transform = t;
        self
    }
    pub fn semantics(mut self, s: Semantics) -> Node {
        self.semantics = Some(s);
        self
    }
    pub fn pickable(mut self) -> Node {
        self.pickable = true;
        self
    }
    pub fn children(&self) -> &[Node] {
        match &self.kind {
            NodeKind::Group { children } | NodeKind::View { children, .. } => children,
            _ => &[],
        }
    }
    pub fn children_mut(&mut self) -> Option<&mut Vec<Node>> {
        match &mut self.kind {
            NodeKind::Group { children } | NodeKind::View { children, .. } => Some(children),
            _ => None,
        }
    }
    pub fn kind_name(&self) -> &'static str {
        match &self.kind {
            NodeKind::Group { .. } => "group",
            NodeKind::View { .. } => "view",
            NodeKind::Shape { geom, .. } => geom.kind_name(),
            NodeKind::Text(_) => "text",
            NodeKind::Image { .. } => "image",
            NodeKind::Instances(_) => "instances",
        }
    }
    /// Depth-first visit with key paths.
    pub fn walk<'a>(&'a self, path: &KeyPath, f: &mut dyn FnMut(&KeyPath, &'a Node)) {
        let p = path.push(&self.key);
        f(&p, self);
        for c in self.children() {
            c.walk(&p, f);
        }
    }
    pub fn walk_mut(&mut self, f: &mut dyn FnMut(&mut Node)) {
        f(self);
        if let Some(cs) = self.children_mut() {
            for c in cs {
                c.walk_mut(f);
            }
        }
    }
    pub fn count(&self) -> usize {
        1 + self.children().iter().map(|c| c.count()).sum::<usize>()
    }
}

/// A resolved scene for one state, or one frame of a transition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    pub width: f64,
    pub height: f64,
    pub background: Ink,
    pub root: Node,
}

impl Scene {
    pub fn new(width: f64, height: f64, root: Node) -> Scene {
        Scene { width, height, background: Ink::token("paper"), root }
    }
    /// Find a node by key path.
    pub fn find(&self, path: &KeyPath) -> Option<&Node> {
        let mut out = None;
        self.root.walk(&KeyPath::default(), &mut |p, n| {
            if out.is_none() && p == path {
                out = Some(n);
            }
        });
        out
    }
    pub fn node_count(&self) -> usize {
        self.root.count()
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
    pub fn from_json(s: &str) -> Result<Scene, String> {
        serde_json::from_str(s).map_err(|e| e.to_string())
    }
}
