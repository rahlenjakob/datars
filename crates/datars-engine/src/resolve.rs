//! Template → scene. Walks the document's scene template with a context (layout box, scales in
//! scope, current datum), evaluating properties, expanding recipes, laying out boxes, shaping text.

use crate::bounds::node_bounds;
use crate::env::{str_of, Fields, ResolveEnv, Row};
use crate::scales::Scope;
use crate::Diag;
use datars_data::{Column, Table};
use datars_expr::{Compiled, Value};
use datars_ir::{Action, Doc, Prop, RepeatFrom, TCamera, TGeom, TInstances, TKind, TStroke, TText, Template};
use datars_math::{Affine, PathData, Rect, Vec2};
use datars_scene::text::{Align, Baseline, TextStyle};
use datars_scene::{
    Anchor, Camera, Cap, Clip, Common, Curve, Geom, Instances, Join, Key, KeyPart, KeyPath, Node, NodeKind, NumberText, Paint, Proto, Role, Semantics, Stroke, SymbolKind, TextNode,
};
use datars_theme::{Ink, ResolvedTheme};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::Arc;

/// Expands `use` nodes (recipes from packages).
pub trait Expand {
    fn expand(&self, recipe: &str, params: &serde_json::Value, cx: &serde_json::Value) -> Result<Expansion, String>;
}

#[derive(Clone, Debug, Default)]
pub struct Expansion {
    pub template: Template,
    pub tables: BTreeMap<String, datars_ir::Derived>,
    /// Fields of the recipe's output the IR ignored (a recipe bug worth reporting).
    pub notes: Vec<String>,
}

/// An intent binding with its values evaluated for one node.
#[derive(Clone, Debug, PartialEq)]
pub enum BoundAction {
    Set { signal: String, value: Value },
    Toggle { signal: String, value: Value },
    /// A choice a host may offer with its own picker: the signal, and each option's value and what
    /// it says.
    Pick { signal: String, options: Vec<(serde_json::Value, String)> },
    Event { event: String },
    Chapter { chapter: String, key: String },
    /// Range selection along `axis` (0 = x, 1 = y) of the node's local space.
    Brush { signal: String, axis: usize, map: BrushMap },
    /// A value under the pointer along `axis` (sliders), snapped to `step` when > 0 and kept
    /// within `bounds` (the action's `min`/`max`, else unbounded).
    Scrub { signal: String, axis: usize, map: BrushMap, step: f64, bounds: (f64, f64) },
    /// Pan/zoom a view's camera relative to its fit camera, within `zoom` (min, max) × its zoom.
    Explore { prefix: String, fit: Camera, zoom: (f64, f64) },
}

/// How brushed pixels (node-local) become data.
#[derive(Clone, Debug, PartialEq)]
pub enum BrushMap {
    /// A sampled inverse of a continuous scale: exact for linear, close for log/time.
    Continuous { px: Vec<f64>, v: Vec<f64> },
    /// Band centres and their keys.
    Bands { centers: Vec<(f64, String)> },
}

impl BrushMap {
    pub fn from_scale(s: &crate::scales::EngineScale) -> BrushMap {
        let (r0, r1) = (s.range_min(), s.range_max());
        if s.bandwidth() > 0.0 {
            let centers = s.domain_values().iter().map(|d| (crate::resolve::value_num(&s.map_value(d)) + s.bandwidth() / 2.0, crate::env::str_of(d))).collect();
            return BrushMap::Bands { centers };
        }
        let n = 256;
        let px: Vec<f64> = (0..=n).map(|i| r0 + (r1 - r0) * i as f64 / n as f64).collect();
        let v = px.iter().map(|p| s.invert(*p)).collect();
        BrushMap::Continuous { px, v }
    }

    /// The data value at local pixel `p` (continuous scales).
    pub fn value(&self, p: f64) -> f64 {
        let BrushMap::Continuous { px, v } = self else { return f64::NAN };
        if px.len() < 2 {
            return f64::NAN;
        }
        let p = p.clamp(px[0], px[px.len() - 1]);
        let i = px.partition_point(|x| *x <= p).clamp(1, px.len() - 1);
        let t = if px[i] > px[i - 1] { (p - px[i - 1]) / (px[i] - px[i - 1]) } else { 0.0 };
        v[i - 1] + (v[i] - v[i - 1]) * t
    }
}

/// Where a scene node came from: recorded only when asked ([`crate::Engine::explain`]), so
/// ordinary resolution pays nothing for it.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Origin {
    /// The node's key path (as `SemanticItem::path` and `inspect` spell it).
    pub path: String,
    /// Recipes whose expansion produced it, outermost first (empty: the document wrote it).
    pub recipes: Vec<String>,
    /// The data row it was drawn for (a repeat's row, group, tick or legend entry).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row: Option<OriginRow>,
    /// The template that produced it, children omitted: what the author or the recipe wrote.
    pub template: serde_json::Value,
    /// Every expression in that template and its value for this node.
    pub values: Vec<ExprValue>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct OriginRow {
    pub table: String,
    pub index: usize,
    pub fields: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ExprValue {
    /// Where in the template (`fill`, `geom.x`, `semantics.label`).
    pub field: String,
    pub expr: String,
    pub value: serde_json::Value,
}

#[derive(Clone)]
pub(crate) struct Cx {
    pub box_w: f64,
    pub box_h: f64,
    pub scope: Rc<Scope>,
    pub row: Option<Rc<Row>>,
    pub fields: Option<Rc<Fields>>,
    pub group: Option<(Arc<Table>, Rc<Vec<usize>>)>,
    pub path: KeyPath,
    pub depth: usize,
    /// The projection of the nearest enclosing `geo` coordinate system.
    pub proj: Option<Rc<datars_geo::Projection>>,
    /// The key a keyless template gets here (a repeat's row, group, tick or legend key). It
    /// applies to the repeated node only, and must be known before the node's path is used:
    /// intents, anchors and provenance are recorded under that path.
    pub default_key: Option<Key>,
    /// Inside a `tiles` layer template: the tile layer (for the feature's geometry).
    pub tile: Option<Rc<crate::tiles::LayerCx>>,
    /// The recipe expansions around this point, outermost first — tracked only while recording
    /// origins.
    pub recipes: Option<Rc<Vec<String>>>,
    /// The nearest node at or above this point the pointer can find (with intents, or
    /// `pickable`): what `hover()` asks about.
    pub interactive: Option<Rc<KeyPath>>,
}

/// Compiled expressions by source. Compiling depends on the source alone, so the resolvers made
/// per map tile and per point cell share one: made fresh, each re-parsed and re-compiled the whole
/// template — most of a galaxy zoom's engine time, frames of 20 ms on a phone. Hashed (a seedless
/// hasher; nothing iterates it): styling a tile looks an expression up for every feature.
pub(crate) type Exprs = indexmap::IndexMap<String, Option<Rc<Expr>>, std::hash::BuildHasherDefault<rustc_hash::FxHasher>>;

/// A compiled expression, and the tables it names (`table.max('t', …)`): found once when it's
/// compiled rather than by scanning its source at every evaluation (every feature of every tile).
pub(crate) struct Expr {
    code: Compiled,
    pub tables: Vec<String>,
}

impl std::ops::Deref for Expr {
    type Target = Compiled;
    fn deref(&self) -> &Compiled {
        &self.code
    }
}
pub(crate) type SharedExprs = Rc<RefCell<Exprs>>;
/// Expressions a resolver has met, by address, and the values of those that read only signals
/// (see `Resolver::fixed`).
type Fixed = BTreeMap<usize, (Rc<Expr>, Option<Value>)>;

pub struct Resolver<'a> {
    pub doc: &'a Doc,
    pub signals: &'a BTreeMap<String, Value>,
    pub theme: &'a ResolvedTheme,
    pub fonts: &'a datars_text::FontDb,
    pub expander: Option<&'a dyn Expand>,
    pub size_class: &'a str,
    pub geo: &'a crate::geo::GeoStore,
    pub(crate) tables: RefCell<crate::tables::TableStore>,
    exprs: SharedExprs,
    /// Expressions met, by address (each kept alive here, so no other takes its address), and the
    /// value of those that read only signals — fixed for a resolver's life: a tile layer's line
    /// width by zoom evaluates once, not once per feature.
    fixed: RefCell<Fixed>,
    /// Inks parsed, by their text (the first [`MAX_INKS`]): a layer's features share a few colours.
    inks: RefCell<BTreeMap<String, Option<Ink>>>,
    pub diags: RefCell<Vec<Diag>>,
    pub actions: RefCell<Vec<(KeyPath, BTreeMap<String, BoundAction>)>>,
    /// `tiles` nodes resolved (placeholders the frame pass fills), by key path.
    pub(crate) tiles: RefCell<Vec<(String, Rc<crate::tiles::Binding>)>>,
    /// `instances` nodes with `lod` resolved (placeholders the frame pass fills), by key path.
    pub(crate) points: RefCell<Vec<(String, Rc<crate::points::Binding>)>>,
    /// Where each node came from, when recording ([`Resolver::record_origins`]).
    pub origins: Option<RefCell<Vec<Origin>>>,
    /// Groups that place their content after layout (`dodge`), placed by `resolve_root`.
    dodges: RefCell<Vec<Dodge>>,
    /// Groups whose texts keep off each other (`declutter`), sorted out by `resolve_root`.
    declutters: RefCell<BTreeSet<KeyPath>>,
    /// Recipes' motion defaults met while resolving (a template's `motion`), each rule scoped to
    /// the recipe's key path: they sit under the document's rules when transitions are planned.
    pub(crate) motion: RefCell<Vec<datars_motion::Rule>>,
    /// `dodge` groups (by path) that get a band of their parent's box this pass …
    pub(crate) banded: BTreeSet<KeyPath>,
    /// Anchors fixed from an earlier resolve of the same state (a running clock moves the data
    /// under a card; the card keeps its place): dodge groups take these instead of choosing.
    pub(crate) dodge_pins: BTreeMap<KeyPath, String>,
    /// The anchor each dodge group took this resolve.
    pub(crate) dodge_chosen: RefCell<BTreeMap<KeyPath, String>>,
    /// … and those that found no free spot this pass (the engine resolves again, banding them).
    pub(crate) crowded: RefCell<Vec<KeyPath>>,
    /// The interactive element under the pointer, as the engine last settled it.
    pub(crate) hovered: Option<KeyPath>,
    /// The elements `hover()` was asked about this resolve: only moving onto or off one of these
    /// needs a new scene. A list, not a set: a thousand lines each asking three times would cost
    /// tens of thousands of key-path comparisons; repeats from one element are skipped (the last
    /// one asked is held, so no other element's path can take its address meanwhile).
    pub(crate) hover_asked: RefCell<Vec<KeyPath>>,
    pub(crate) hover_last: RefCell<Option<Rc<KeyPath>>>,
}

/// A `dodge` group: its path, its box and inset, and the anchors to try in order.
struct Dodge {
    path: KeyPath,
    size: (f64, f64),
    pad: [f64; 4],
    anchors: Vec<String>,
}

const MAX_EXPANSION_DEPTH: usize = 24;
/// How far from a line of instances (`hit: line`) the pointer still finds it, px, unless the
/// template says.
const LINE_REACH: f64 = 16.0;
/// Inks a resolver keeps parsed ([`Resolver::ink`]); colours computed per row (a choropleth's
/// scale) are many and parse as they come.
const MAX_INKS: usize = 256;

impl<'a> Resolver<'a> {
    pub(crate) fn new(
        doc: &'a Doc,
        signals: &'a BTreeMap<String, Value>,
        theme: &'a ResolvedTheme,
        fonts: &'a datars_text::FontDb,
        expander: Option<&'a dyn Expand>,
        size_class: &'a str,
        geo: &'a crate::geo::GeoStore,
        tables: crate::tables::TableStore,
    ) -> Resolver<'a> {
        Resolver { doc, signals, theme, fonts, expander, size_class, geo, tables: RefCell::new(tables), exprs: SharedExprs::default(), fixed: RefCell::new(BTreeMap::new()), inks: RefCell::new(BTreeMap::new()), diags: RefCell::new(Vec::new()), actions: RefCell::new(Vec::new()), tiles: RefCell::new(Vec::new()), points: RefCell::new(Vec::new()), origins: None, dodges: RefCell::new(Vec::new()), declutters: RefCell::new(BTreeSet::new()), motion: RefCell::new(Vec::new()), banded: BTreeSet::new(), crowded: RefCell::new(Vec::new()), dodge_pins: BTreeMap::new(), dodge_chosen: RefCell::new(BTreeMap::new()), hovered: None, hover_asked: RefCell::new(Vec::new()), hover_last: RefCell::new(None) }
    }

    /// Record every node's [`Origin`] while resolving (for `explain`; costs a template clone and
    /// expression evaluations per node).
    pub(crate) fn record_origins(&mut self) {
        self.origins = Some(RefCell::new(Vec::new()));
    }

    fn record(&self, t: &Template, cx: &Cx, path: &KeyPath) {
        let Some(origins) = &self.origins else { return };
        let mut template = serde_json::to_value(t).unwrap_or_default();
        if let serde_json::Value::Object(o) = &mut template {
            for k in ["children", "template"] {
                if let Some(c) = o.get_mut(k) {
                    *c = serde_json::Value::String("…".into());
                }
            }
        }
        let mut values = Vec::new();
        fn walk(r: &Resolver, v: &serde_json::Value, field: &str, cx: &Cx, out: &mut Vec<ExprValue>) {
            let p = Prop(v.clone());
            if let Some(src) = p.as_expr() {
                out.push(ExprValue { field: field.to_string(), expr: src.to_string(), value: value_to_json(&r.eval(&p, cx)) });
                return;
            }
            match v {
                serde_json::Value::Object(o) => {
                    for (k, c) in o {
                        // Scale definitions and child templates evaluate in their own contexts.
                        if field.is_empty() && matches!(k.as_str(), "scales" | "children" | "template" | "tables") {
                            continue;
                        }
                        walk(r, c, &if field.is_empty() { k.clone() } else { format!("{field}.{k}") }, cx, out);
                    }
                }
                serde_json::Value::Array(a) => {
                    for (i, c) in a.iter().enumerate() {
                        walk(r, c, &format!("{field}[{i}]"), cx, out);
                    }
                }
                _ => {}
            }
        }
        walk(self, &serde_json::to_value(t).unwrap_or_default(), "", cx, &mut values);
        let row = cx.row.as_ref().map(|r| OriginRow {
            table: r.table.name.clone(),
            index: r.row,
            fields: r.table.columns.iter().map(|(name, c)| (name.clone(), serde_json::to_value(c.get(r.row)).unwrap_or_default())).collect(),
        });
        origins.borrow_mut().push(Origin { path: path.to_string(), recipes: cx.recipes.as_deref().cloned().unwrap_or_default(), row, template, values });
    }

    pub(crate) fn diag(&self, msg: impl Into<String>) {
        let m = msg.into();
        let mut d = self.diags.borrow_mut();
        if !d.iter().any(|x| x.message == m) {
            d.push(Diag { message: m });
        }
    }

    /// Resolve the whole scene template into a root node for a viewport.
    pub(crate) fn resolve_root(&self, width: f64, height: f64) -> Node {
        // A root that yields several nodes (a top-level repeat) is wrapped in a `root` group; its
        // nodes resolve under that path from the start, so intents and anchors find them.
        let wrapped = matches!(self.doc.scene.kind, TKind::Repeat { .. });
        let path = if wrapped { KeyPath::default().push(&Key::name("root")) } else { KeyPath::default() };
        let cx = Cx { box_w: width, box_h: height, scope: Rc::new(Scope::default()), row: None, fields: None, group: None, path, depth: 0, proj: None, default_key: None, tile: None, recipes: None, interactive: None };
        let mut nodes = self.resolve(&self.doc.scene, &cx, 0);
        let dodges = std::mem::take(&mut *self.dodges.borrow_mut());
        for d in dodges.iter().filter(|d| !self.banded.contains(&d.path)) {
            // (A pin the group no longer offers — its anchor is an expression that changed — is
            // no pin.)
            let pin = self.dodge_pins.get(&d.path).map(|a| a.as_str()).filter(|p| d.anchors.iter().any(|a| a == p));
            let (covered, hidden, worst, anchor) = dodge(&mut nodes, &KeyPath::default(), d, pin);
            // More than a sliver covered wherever it goes: next pass, it takes a band instead —
            // unless it has one place only (fixed: it stays there, over whatever is under it). A
            // sliver is 4 % of the card, at most 2 % of what there is to read (in a phone's narrow
            // box a card is big next to the chart, and 4 % of it is a whole slice), and no label
            // more than a fifth hidden.
            if pin.is_none() && (covered > 0.04 || hidden > 0.02 || worst > 0.2) && d.anchors.len() > 1 {
                self.crowded.borrow_mut().push(d.path.clone());
            }
            if let Some(a) = anchor {
                self.dodge_chosen.borrow_mut().insert(d.path.clone(), a);
            }
        }
        let mut texts = Vec::new();
        nodes.iter().for_each(|n| text_quads(n, Affine::IDENTITY, &mut texts, false));
        let canvas = Rect::new(0.0, 0.0, width, height);
        for n in &mut nodes {
            contain(n, Affine::IDENTITY, canvas, false, &texts);
        }
        for p in std::mem::take(&mut *self.declutters.borrow_mut()) {
            let mut texts = Vec::new();
            nodes.iter().for_each(|n| text_quads(n, Affine::IDENTITY, &mut texts, true));
            declutter(&mut nodes, &p, &texts, canvas);
        }
        if wrapped {
            return Node::group(Key::name("root"), nodes);
        }
        match nodes.len() {
            1 => nodes.pop().unwrap(),
            _ => Node::group(Key::name("root"), nodes),
        }
    }

    // ---- expressions ----------------------------------------------------------------------

    /// Compile into (and read from) `exprs`, shared with other resolvers (see [`Exprs`]).
    pub(crate) fn with_exprs(mut self, exprs: &SharedExprs) -> Self {
        self.exprs = exprs.clone();
        self
    }

    pub(crate) fn compiled(&self, src: &str) -> Option<Rc<Expr>> {
        if let Some(c) = self.exprs.borrow().get(src) {
            return c.clone();
        }
        let c = match datars_expr::parse(src) {
            Ok(e) => match datars_expr::compile(&e) {
                Ok(c) => Some(Rc::new(Expr { code: c, tables: table_refs(src) })),
                Err(err) => {
                    self.diag(format!("expression `{src}`: {err:?}"));
                    None
                }
            },
            Err(err) => {
                self.diag(format!("expression `{src}`: {err:?}"));
                None
            }
        };
        self.exprs.borrow_mut().insert(src.to_string(), c.clone());
        c
    }

    /// The computed tables an expression can read, after computing the ones it names: aggregate
    /// functions (`table.max('t', 'v')`) read tables by name, and a derived table nobody has
    /// asked for yet would otherwise read as missing — results would depend on resolution order.
    pub(crate) fn snapshot_for(&self, tables: &[String], cx: &Cx) -> Rc<BTreeMap<String, Arc<Table>>> {
        for name in tables {
            if !self.tables.borrow().snapshot_has(name) {
                let _ = crate::tables::get(self, name, cx);
            }
        }
        self.tables.borrow().snapshot()
    }

    pub(crate) fn env<'e>(&'e self, cx: &'e Cx, tables: &'e BTreeMap<String, Arc<Table>>) -> ResolveEnv<'e> {
        ResolveEnv {
            whole: None,
            row: cx.row.as_deref(),
            fields: cx.fields.as_deref(),
            group: cx.group.as_ref().map(|(t, r)| (t.as_ref(), r.as_slice())),
            signals: self.signals,
            scopes: &cx.scope,
            box_w: cx.box_w,
            box_h: cx.box_h,
            theme: self.theme,
            keys: &self.doc.keys,
            locale: &self.doc.locale,
            tables,
            scratch: RefCell::new(Vec::new()),
            proj: cx.proj.as_deref(),
            geo: Some(self.geo),
            fonts: Some(self.fonts),
            hover: cx.interactive.as_ref().map(|at| crate::env::Hover { at, hovered: self.hovered.as_ref(), asked: &self.hover_asked, last: &self.hover_last }),
        }
    }

    pub(crate) fn eval(&self, p: &Prop, cx: &Cx) -> Value {
        if let Some(src) = p.as_expr() {
            let Some(c) = self.compiled(src) else { return Value::Null };
            let id = Rc::as_ptr(&c) as usize;
            let known = self.fixed.borrow().get(&id).map(|(_, v)| v.clone());
            if let Some(Some(v)) = known {
                return v;
            }
            let snapshot = self.snapshot_for(&c.tables, cx);
            let env = self.env(cx, &snapshot);
            let v = c.eval_scalar(&env);
            if known.is_none() {
                let free = c.row_free(&|name| self.signals.contains_key(name));
                self.fixed.borrow_mut().insert(id, (c.clone(), free.then(|| v.clone())));
            }
            return v;
        }
        json_to_value(&p.0)
    }

    pub(crate) fn num(&self, p: &Prop, cx: &Cx, default: f64) -> f64 {
        if p.is_null() {
            return default;
        }
        if let Some(s) = p.as_str() {
            if let Some(tok) = s.strip_prefix('$') {
                return self.theme.number(tok).unwrap_or(default);
            }
        }
        match self.eval(p, cx) {
            Value::Num(n) => n,
            Value::Bool(b) => b as i32 as f64,
            Value::Str(s) => s.parse().unwrap_or(default),
            _ => default,
        }
    }

    pub(crate) fn string(&self, p: &Prop, cx: &Cx) -> String {
        if p.is_null() {
            return String::new();
        }
        str_of(&self.eval(p, cx))
    }

    pub(crate) fn truthy(&self, p: &Prop, cx: &Cx) -> bool {
        match self.eval(p, cx) {
            Value::Null => false,
            Value::Bool(b) => b,
            Value::Num(n) => n != 0.0 && !n.is_nan(),
            Value::Str(s) => !s.is_empty(),
        }
    }

    pub(crate) fn ink(&self, p: &Prop, cx: &Cx) -> Option<Ink> {
        if p.is_null() {
            return None;
        }
        // A literal is its own text (no value to build); an expression's value is the text.
        let owned;
        let s = match p.as_str() {
            Some(s) => s,
            None => {
                owned = self.string(p, cx);
                owned.as_str()
            }
        };
        if let Some(ink) = self.inks.borrow().get(s) {
            return ink.clone();
        }
        let ink = if s.is_empty() || s == "none" {
            None
        } else {
            Ink::parse(s).or_else(|| {
                self.diag(format!("not a colour: `{s}`"));
                None
            })
        };
        let mut inks = self.inks.borrow_mut();
        if inks.len() < MAX_INKS {
            inks.insert(s.to_string(), ink.clone());
        }
        ink
    }

    fn key_of(&self, p: &Prop, cx: &Cx, fallback: Key) -> Key {
        if p.is_null() {
            return fallback;
        }
        if p.as_expr().is_none() {
            if let serde_json::Value::Array(a) = &p.0 {
                // A composite key: literal parts, or expressions per part (`[=d.series, =d.month]`)
                // — so a segment's key has its series as parent and can merge into that series' bar.
                return Key::new(
                    a.iter()
                        .map(|x| {
                            let part = Prop(x.clone());
                            if part.as_expr().is_some() {
                                value_key_part(&self.eval(&part, cx))
                            } else {
                                json_key_part(x)
                            }
                        })
                        .collect(),
                );
            }
        }
        match self.eval(p, cx) {
            Value::Null => fallback,
            v => Key::one(value_key_part(&v)),
        }
    }

    // ---- templates --------------------------------------------------------------------------

    pub(crate) fn resolve(&self, t: &Template, cx: &Cx, index: usize) -> Vec<Node> {
        if !t.when.is_null() && !self.truthy(&t.when, cx) {
            return Vec::new();
        }
        let default_key = cx.default_key.clone().unwrap_or_else(|| Key::one(format!("#{index}")));
        match &t.kind {
            TKind::Use { recipe, params } => self.resolve_use(t, recipe, params, cx, index),
            TKind::Repeat { from, template } => self.resolve_repeat(from, template, &Cx { default_key: None, ..cx.clone() }),
            _ => {
                let key = self.key_of(&t.key, cx, default_key);
                let path = cx.path.push(&key);
                if let Some(m) = &t.motion {
                    self.motion_defaults(m, &path);
                }
                // Descendants key themselves; below a node the pointer finds, `hover()` asks about it.
                let interactive = if !t.on.is_empty() || t.pickable { Some(Rc::new(path.clone())) } else { cx.interactive.clone() };
                let own_cx = Cx { default_key: None, interactive, ..cx.clone() };
                let cx = &own_cx;
                // A coordinate system for this subtree (a map's projection fits this node's box).
                let with_coord;
                let cx = match t.coord.as_ref().and_then(|c| crate::geo::projection_for(self, c, cx)) {
                    Some(p) => {
                        with_coord = Cx { proj: Some(Rc::new(p)), ..cx.clone() };
                        &with_coord
                    }
                    None => cx,
                };
                let kind = match self.resolve_kind(t, cx, &path) {
                    Some(k) => k,
                    None => return Vec::new(),
                };
                let mut n = Node::new(key, kind);
                self.apply_common(&mut n, t, cx, &path);
                pin_text_at_its_point(&mut n);
                self.record(t, cx, &path);
                vec![n]
            }
        }
    }

    /// A template's motion defaults (a list of rules, or `{ rules: [...] }`), each scoped to the
    /// template's subtree: a recipe's "lines draw on" applies to the lines it makes, nowhere else.
    fn motion_defaults(&self, m: &serde_json::Value, path: &KeyPath) {
        let list = m.get("rules").unwrap_or(m);
        match serde_json::from_value::<Vec<datars_motion::Rule>>(list.clone()) {
            Ok(rules) => {
                let mut out = self.motion.borrow_mut();
                for mut r in rules {
                    r.select.get_or_insert_with(Default::default).key_prefix = Some(path.clone());
                    if !out.contains(&r) {
                        out.push(r);
                    }
                }
            }
            Err(e) => self.diag(format!("motion defaults at {path}: {e}")),
        }
    }

    fn resolve_use(&self, t: &Template, recipe: &str, params: &serde_json::Value, cx: &Cx, index: usize) -> Vec<Node> {
        if cx.depth > MAX_EXPANSION_DEPTH {
            self.diag(format!("recipe `{recipe}` expands too deeply (recursive?)"));
            return Vec::new();
        }
        let Some(ex) = self.expander else {
            self.diag(format!("recipe `{recipe}` needs the sandbox (not available in this runtime)"));
            return Vec::new();
        };
        let ecx = serde_json::json!({
            "size": [cx.box_w, cx.box_h],
            "sizeClass": self.size_class,
            "locale": self.doc.locale,
        });
        match ex.expand(recipe, params, &ecx) {
            Ok(exp) => {
                for n in &exp.notes {
                    self.diag(format!("recipe `{recipe}`: {n}"));
                }
                if !exp.tables.is_empty() {
                    self.tables.borrow_mut().add_derived(exp.tables);
                }
                let mut inner = exp.template;
                // The use node's own common props wrap the expansion.
                if !t.key.is_null() {
                    inner.key = t.key.clone();
                }
                if inner.prov.is_none() {
                    inner.prov = Some(recipe.to_string());
                }
                if t.size.is_some() {
                    inner.size = t.size.clone();
                }
                if t.id.is_some() && inner.id.is_none() {
                    inner.id = t.id.clone();
                }
                if !t.when.is_null() {
                    inner.when = Prop::default(); // already checked
                }
                let recipes = self.origins.is_some().then(|| {
                    let mut chain = cx.recipes.as_deref().cloned().unwrap_or_default();
                    chain.push(recipe.to_string());
                    Rc::new(chain)
                });
                let ncx = Cx { depth: cx.depth + 1, recipes: recipes.or_else(|| cx.recipes.clone()), ..cx.clone() };
                self.resolve(&inner, &ncx, index)
            }
            Err(e) => {
                self.diag(format!("recipe `{recipe}`: {e}"));
                Vec::new()
            }
        }
    }

    fn resolve_repeat(&self, from: &RepeatFrom, template: &Template, cx: &Cx) -> Vec<Node> {
        let mut out = Vec::new();
        match from {
            RepeatFrom::Table(name) => {
                let Some(table) = self.table_for(name, cx) else { return out };
                for i in 0..table.len() {
                    let row = Rc::new(Row { table: table.clone(), row: i });
                    let key = crate::tables::row_key(&table, i);
                    let ncx = Cx { row: Some(row), fields: None, default_key: Some(key), ..cx.clone() };
                    out.extend(self.resolve(template, &ncx, i));
                }
            }
            RepeatFrom::Groups { groups, by } => {
                let Some(table) = self.table_for(groups, cx) else { return out };
                let mut order: Vec<String> = Vec::new();
                let mut rows: BTreeMap<String, Vec<usize>> = BTreeMap::new();
                for i in 0..table.len() {
                    let g = crate::tables::cell_string(&table, by, i);
                    if !rows.contains_key(&g) {
                        order.push(g.clone());
                    }
                    rows.entry(g).or_default().push(i);
                }
                for (gi, g) in order.iter().enumerate() {
                    let rs = Rc::new(rows.remove(g).unwrap_or_default());
                    let first = rs.first().copied().unwrap_or(0);
                    let row = Rc::new(Row { table: table.clone(), row: first });
                    let ncx = Cx { row: Some(row), fields: None, group: Some((table.clone(), rs)), default_key: Some(Key::one(g.as_str())), ..cx.clone() };
                    out.extend(self.resolve(template, &ncx, gi));
                }
            }
            RepeatFrom::Ticks { ticks, count } => {
                let Some(sc) = cx.scope.get(ticks) else {
                    self.diag(format!("repeat over ticks of unknown scale `{ticks}`"));
                    return out;
                };
                let all = sc.ticks(self.tick_count(count, cx), &self.doc.locale);
                let n = all.len();
                for (i, t) in all.into_iter().enumerate() {
                    let v = t.value;
                    let mut f = Fields::new();
                    f.insert("value".into(), v.clone());
                    f.insert("label".into(), Value::Str(Arc::from(t.label.as_str())));
                    f.insert("index".into(), Value::Num(i as f64));
                    // How many ticks the scale made: with the axis length, how far apart they are.
                    f.insert("count".into(), Value::Num(n as f64));
                    f.insert("pos".into(), sc.map_value(&v));
                    f.insert("kind".into(), Value::Str(Arc::from(t.kind)));
                    f.insert("major".into(), Value::Bool(t.major));
                    let ncx = Cx { row: Some(Self::datum_row(&f)), fields: None, default_key: Some(Key::one(value_key_part(&v))), ..cx.clone() };
                    out.extend(self.resolve(template, &ncx, i));
                }
            }
            RepeatFrom::Legend { legend } => {
                let Some(sc) = cx.scope.get(legend) else {
                    self.diag(format!("legend of unknown scale `{legend}`"));
                    return out;
                };
                for (i, v) in sc.domain_values().into_iter().enumerate() {
                    let mut f = Fields::new();
                    f.insert("value".into(), v.clone());
                    f.insert("label".into(), Value::Str(Arc::from(str_of(&v).as_str())));
                    f.insert("ink".into(), Value::Str(Arc::from(sc.ink(&v).to_string().as_str())));
                    f.insert("index".into(), Value::Num(i as f64));
                    let ncx = Cx { row: Some(Self::datum_row(&f)), fields: None, default_key: Some(Key::one(value_key_part(&v))), ..cx.clone() };
                    out.extend(self.resolve(template, &ncx, i));
                }
            }
            RepeatFrom::Count { count } => {
                let n = self.num(count, cx, 0.0).max(0.0) as usize;
                for i in 0..n.min(100_000) {
                    let mut f = Fields::new();
                    f.insert("index".into(), Value::Num(i as f64));
                    let ncx = Cx { row: Some(Self::datum_row(&f)), fields: None, default_key: Some(Key::one(i as i64)), ..cx.clone() };
                    out.extend(self.resolve(template, &ncx, i));
                }
            }
        }
        out
    }

    /// A synthetic datum (tick, legend entry, counter) as a one-row table, so expressions read it
    /// exactly like a data row (`d.value`, `d.label`, …).
    fn datum_row(fields: &Fields) -> Rc<Row> {
        let cols: Vec<(String, Column)> = fields
            .iter()
            .map(|(k, v)| {
                let c = match v {
                    Value::Num(n) => Column::Num(vec![*n]),
                    Value::Bool(b) => Column::Bool(vec![*b]),
                    Value::Str(s) => Column::Str(vec![Some(s.clone())]),
                    Value::Null => Column::Num(vec![f64::NAN]),
                };
                (k.clone(), c)
            })
            .collect();
        let t = Table::from_columns("datum", cols).unwrap_or_default();
        Rc::new(Row { table: Arc::new(t), row: 0 })
    }

    pub(crate) fn table_for(&self, name: &str, cx: &Cx) -> Option<Arc<Table>> {
        if name == "@group" {
            let (t, rows) = cx.group.as_ref()?;
            return Some(Arc::new(crate::tables::take_rows(t, rows)));
        }
        let t = crate::tables::get(self, name, cx);
        if t.is_none() {
            self.diag(format!("unknown table `{name}`"));
        }
        t
    }

    fn resolve_kind(&self, t: &Template, cx: &Cx, path: &KeyPath) -> Option<NodeKind> {
        Some(match &t.kind {
            TKind::Group { children } => {
                if let Some(l) = t.layout.as_ref().filter(|l| l.ty == "grid" && children.iter().any(|c| matches!(c.kind, TKind::Repeat { .. }))) {
                    let (scope, _) = self.scope_and_layout(t, children, cx, Some(path));
                    return Some(NodeKind::Group { children: self.grid_of_repeats(l, children, cx, &scope, path) });
                }
                let (scope, boxes) = self.scope_and_layout(t, children, cx, Some(path));
                let mut kids = self.resolve_children(children, &boxes, cx, &scope, path);
                if let Some(l) = t.layout.as_ref().filter(|l| l.ty == "flow") {
                    flow(&mut kids, Rect::new(l.padding[3], l.padding[0], cx.box_w - l.padding[1] - l.padding[3], cx.box_h - l.padding[0] - l.padding[2]), l.gap);
                }
                if let Some(bd) = &t.backdrop {
                    if let Some(n) = self.backdrop(bd, &kids, cx) {
                        kids.insert(0, n);
                    }
                }
                if !t.dodge.is_empty() {
                    let pad = t.layout.as_ref().map_or([0.0; 4], |l| l.padding);
                    // Anchors may be expressions; each is tried once, in the order first named.
                    let mut anchors: Vec<String> = Vec::new();
                    for a in t.dodge.iter().map(|a| self.string(a, cx)) {
                        if !a.is_empty() && !anchors.contains(&a) {
                            anchors.push(a);
                        }
                    }
                    self.dodges.borrow_mut().push(Dodge { path: path.clone(), size: (cx.box_w, cx.box_h), pad, anchors });
                }
                if t.declutter {
                    self.declutters.borrow_mut().insert(path.clone());
                }
                NodeKind::Group { children: kids }
            }
            TKind::View { camera, children } => {
                let (scope, boxes) = self.scope_and_layout(t, children, cx, Some(path));
                let kids = self.resolve_children(children, &boxes, cx, &scope, path);
                let viewport = Rect::new(0.0, 0.0, cx.box_w, cx.box_h);
                let cam = camera.as_ref().and_then(|c| self.camera(c, &kids, viewport, cx));
                if let (Some(TCamera::Fit { explore: Some(prefix), max_zoom, min_zoom, .. }), Some(fit)) = (camera.as_ref(), cam) {
                    let mut bound = BTreeMap::new();
                    let zoom = (min_zoom.unwrap_or(0.5).max(1e-6), max_zoom.unwrap_or(64.0).max(1e-6));
                    bound.insert("explore".to_string(), BoundAction::Explore { prefix: prefix.clone(), fit, zoom: (zoom.0.min(zoom.1), zoom.0.max(zoom.1)) });
                    self.actions.borrow_mut().push((path.clone(), bound));
                }
                let cam = cam.map(|c| self.explored(c, camera.as_ref()));
                let clip = !matches!(t.clip.0, serde_json::Value::Bool(false));
                NodeKind::View { viewport, camera: cam, clip, children: kids }
            }
            TKind::Shape { geom, fill, stroke, markers } => NodeKind::Shape {
                geom: self.geom(geom, cx)?,
                fill: self.ink(fill, cx).map(Paint::Solid),
                stroke: stroke.as_ref().and_then(|s| self.stroke(s, cx)),
                markers: markers.as_ref().and_then(|m| serde_json::from_value(m.clone()).ok()),
            },
            TKind::Text(tt) => NodeKind::Text(self.text(tt, cx)),
            TKind::Instances(ti) if ti.lod.is_some() => crate::points::resolve(self, ti, cx, path)?,
            TKind::Instances(ti) => NodeKind::Instances(Arc::new(self.instances(ti, cx)?)),
            TKind::Image { asset, rect } => NodeKind::Image {
                asset: Arc::from(asset.as_str()),
                rect: Rect::new(self.num(&rect[0], cx, 0.0), self.num(&rect[1], cx, 0.0), self.num(&rect[2], cx, 0.0), self.num(&rect[3], cx, 0.0)),
            },
            TKind::Tiles(tt) => crate::tiles::resolve(self, tt, cx, path)?,
            TKind::Use { .. } | TKind::Repeat { .. } => unreachable!("handled in resolve"),
        })
    }

    /// Scales declared on `t` (in a scope over the parent's) and the layout boxes of its children.
    fn scope_and_layout(&self, t: &Template, children: &[Template], cx: &Cx, path: Option<&KeyPath>) -> (Rc<Scope>, Vec<Rect>) {
        let own = Rect::new(0.0, 0.0, cx.box_w, cx.box_h);
        if t.scales.is_empty() {
            return (cx.scope.clone(), self.layout_children(t, children, cx, own, path));
        }
        // Pass 1: provisional scales with ranges over our own box.
        let provisional = Rc::new(crate::scales::build_scope(self, &t.scales, cx, None));
        let pcx = Cx { scope: provisional.clone(), ..cx.clone() };
        let boxes = self.layout_children(t, children, &pcx, own, path);
        // Which named boxes do ranges refer to?
        let wanted: Vec<String> = t.scales.values().filter_map(|d| d.range.get("box").and_then(|b| b.as_str()).map(String::from)).collect();
        if wanted.is_empty() {
            return (provisional, boxes);
        }
        let mut found: BTreeMap<String, Rect> = BTreeMap::new();
        for id in &wanted {
            if let Some(r) = self.find_box(children, &boxes, &pcx, id) {
                found.insert(id.clone(), r);
            }
        }
        // Pass 2: final scales; layout again (axis sizes may change with the final ranges).
        let scope = Rc::new(crate::scales::build_scope(self, &t.scales, cx, Some(&found)));
        let fcx = Cx { scope: scope.clone(), ..cx.clone() };
        let boxes = self.layout_children(t, children, &fcx, own, path);
        (scope, boxes)
    }

    /// Find the box of the descendant template with `id`, relative to the current node.
    fn find_box(&self, children: &[Template], boxes: &[Rect], cx: &Cx, id: &str) -> Option<Rect> {
        for (c, b) in children.iter().zip(boxes) {
            if c.id.as_deref() == Some(id) {
                return Some(*b);
            }
            let kids = match &c.kind {
                TKind::Group { children } | TKind::View { children, .. } => children,
                _ => continue,
            };
            let ccx = Cx { box_w: b.w, box_h: b.h, ..cx.clone() };
            let inner = self.layout_children(c, kids, &ccx, Rect::new(0.0, 0.0, b.w, b.h), None);
            if let Some(r) = self.find_box(kids, &inner, &ccx, id) {
                return Some(Rect::new(r.x + b.x, r.y + b.y, r.w, r.h));
            }
        }
        None
    }

    fn layout_children(&self, t: &Template, children: &[Template], cx: &Cx, own: Rect, path: Option<&KeyPath>) -> Vec<Rect> {
        // A `dodge` child that found no free spot on the last pass takes a band at the bottom of
        // this box; the other children lay out in what's left (a card under a crowded chart).
        if let Some(p) = path.filter(|_| !self.banded.is_empty()) {
            // The child is the dodge group, or wraps it (a recipe's root inside its use node).
            // Only a `use` node wraps its recipe's root one level down; a plain group that merely
            // contains the card (a chart area with a card over it) is not the card.
            let is_banded = |i: usize, c: &Template| {
                let cp = p.push(&self.key_of(&c.key, cx, Key::one(format!("#{i}"))));
                let wraps = matches!(c.kind, TKind::Use { .. });
                self.banded.iter().any(|b| b.0 == cp.0 || (wraps && b.0.starts_with(&cp.0) && b.0.len() == cp.0.len() + 1))
            };
            // (Of several children at that path — one card per scene — the one showing.)
            let shown = |c: &Template| c.when.is_null() || self.truthy(&c.when, cx);
            if let Some(b) = children.iter().enumerate().position(|(i, c)| is_banded(i, c) && shown(c)) {
                let pad = t.layout.as_ref().map_or([0.0; 4], |l| l.padding);
                let w = (own.w - pad[1] - pad[3]).max(0.0);
                let mcx = Cx { box_w: w, box_h: own.h, ..cx.clone() };
                let used = self.resolve(&children[b], &mcx, b).iter().fold(Rect::empty(), |acc, n| acc.union(&node_bounds(n)));
                let hb = if used.is_empty() { 0.0 } else { used.y1().max(0.0) }.min(own.h * 0.5);
                let rest = Rect::new(own.x, own.y, own.w, (own.h - hb).max(0.0));
                let mut rects = self.layout_children_in(t, children, cx, rest);
                rects[b] = Rect::new(own.x + pad[3], own.y + own.h - hb - pad[2], w, hb);
                return rects;
            }
        }
        self.layout_children_in(t, children, cx, own)
    }

    fn layout_children_in(&self, t: &Template, children: &[Template], cx: &Cx, own: Rect) -> Vec<Rect> {
        // `flow` places the resolved nodes afterwards (a repeat is one child template but many
        // nodes); they resolve in the whole box.
        let Some(l) = t.layout.as_ref().filter(|l| l.ty != "flow") else {
            // No layout: children share the box, unless they ask for a size (placed top-left).
            return children
                .iter()
                .map(|c| match &self.child_size(c, cx, 0) {
                    Some(s) => {
                        let w = self.dim(&s.w, cx).map(|d| dim_px(d, own.w)).unwrap_or(own.w);
                        let h = self.dim(&s.h, cx).map(|d| dim_px(d, own.h)).unwrap_or(own.h);
                        Rect::new(0.0, 0.0, w, h)
                    }
                    None => own,
                })
                .collect();
        };
        // Columns that `wrap` below a width: rows in a narrower box, every child an equal share.
        let stacked = l.ty == "columns" && l.wrap.is_some_and(|w| own.w < w);
        let spec = datars_layout::Spec::parse(if stacked { "rows" } else { &l.ty }, l.gap, l.padding, l.columns, l.align.as_deref());
        let all: Vec<usize> = (0..children.len()).collect();
        let rects = self.layout_some(children, &all, &spec, cx, own, stacked);
        if spec.kind == datars_layout::Kind::Stack {
            return rects;
        }
        // A child whose `when` is false isn't there: along rows, columns and grids it takes no
        // share of the space and leaves no gap. `when` is read with the box the child would get,
        // as `resolve` reads it; hidden children keep that box (so they stay hidden), the others
        // are laid out again without them.
        let shown: Vec<usize> = all.iter().copied().filter(|&i| {
            let c = &children[i];
            c.when.is_null() || self.truthy(&c.when, &Cx { box_w: rects[i].w, box_h: rects[i].h, ..cx.clone() })
        }).collect();
        if shown.len() == children.len() {
            return rects;
        }
        let mut out = rects;
        for (i, r) in shown.iter().zip(self.layout_some(children, &shown, &spec, cx, own, stacked)) {
            out[*i] = r;
        }
        out
    }

    /// A child's size in its parent's layout: its own, or — for a recipe whose use doesn't say —
    /// the size its expansion's root asks for (a title as tall as its lines), as a pre-expanded
    /// document has it. Expansions are cached, so asking is cheap.
    fn child_size(&self, c: &Template, cx: &Cx, depth: usize) -> Option<datars_ir::SizeSpec> {
        if c.size.is_some() || depth > MAX_EXPANSION_DEPTH {
            return c.size.clone();
        }
        let TKind::Use { recipe, params } = &c.kind else { return None };
        let ecx = serde_json::json!({ "size": [cx.box_w, cx.box_h], "sizeClass": self.size_class, "locale": self.doc.locale });
        let exp = self.expander?.expand(recipe, params, &ecx).ok()?;
        self.child_size(&exp.template, cx, depth + 1)
    }

    /// Lay out the children at `which` (indices into `children`), one rect each.
    fn layout_some(&self, children: &[Template], which: &[usize], spec: &datars_layout::Spec, cx: &Cx, own: Rect, equal: bool) -> Vec<Rect> {
        let kids: Vec<datars_layout::Child> = which
            .iter()
            .map(|&i| match equal {
                // (Wrapped columns: sizes meant for side by side don't apply one above the other.)
                true => datars_layout::Child::default(),
                false => {
                    let size = self.child_size(&children[i], cx, 0);
                    datars_layout::Child { w: size.as_ref().and_then(|s| self.dim(&s.w, cx)), h: size.as_ref().and_then(|s| self.dim(&s.h, cx)) }
                }
            })
            .collect();
        datars_layout::layout(own, spec, &kids, &mut |k, aw, ah| {
            let i = which[k];
            let w = if aw.is_finite() { aw } else { own.w };
            let h = if ah.is_finite() { ah } else { own.h };
            let mcx = Cx { box_w: w, box_h: h, ..cx.clone() };
            let nodes = self.resolve(&children[i], &mcx, i);
            let b = nodes.iter().fold(Rect::empty(), |acc, n| acc.union(&node_bounds(n)));
            if b.is_empty() {
                (0.0, 0.0)
            } else {
                // Content can extend left/up of the origin (right-aligned axis labels): count it.
                (b.x1().max(0.0) - b.x.min(0.0), b.y1().max(0.0) - b.y.min(0.0))
            }
        })
    }

    /// A grid over repeated children (small multiples): a repeat is one template but many nodes, so
    /// the cells are sized by how many items the repeats will make, every item resolves in a cell-
    /// sized box, and the nodes are placed cell by cell in order.
    fn grid_of_repeats(&self, l: &datars_ir::Layout, children: &[Template], cx: &Cx, scope: &Rc<Scope>, path: &KeyPath) -> Vec<Node> {
        let pad = l.padding;
        let inner = Rect::new(pad[3], pad[0], (cx.box_w - pad[1] - pad[3]).max(0.0), (cx.box_h - pad[0] - pad[2]).max(0.0));
        let scx = Cx { scope: scope.clone(), ..cx.clone() };
        // Hidden children (`when` false) take no cell.
        let n: usize = children
            .iter()
            .filter(|c| c.when.is_null() || self.truthy(&c.when, &scx))
            .map(|c| match &c.kind {
                TKind::Repeat { from, .. } => self.repeat_len(from, &scx),
                _ => 1,
            })
            .sum();
        let cols = l.columns.unwrap_or(2).max(1);
        let rows = n.div_ceil(cols).max(1);
        let cw = ((inner.w - l.gap * (cols - 1) as f64) / cols as f64).max(0.0);
        let rh = ((inner.h - l.gap * (rows - 1) as f64) / rows as f64).max(0.0);
        let cells = vec![Rect::new(0.0, 0.0, cw, rh); children.len()];
        let mut kids = self.resolve_children(children, &cells, cx, scope, path);
        for (i, k) in kids.iter_mut().enumerate() {
            let (r, c) = (i / cols, i % cols);
            k.common.transform = Affine::translate(inner.x + c as f64 * (cw + l.gap), inner.y + r as f64 * (rh + l.gap)).mul(k.common.transform);
        }
        kids
    }

    /// A size along one axis: px, `auto`, `fill`, a percentage — or an expression over the parent's
    /// box, in px (`min(260, box.w - 24)`: a card as wide as asked, never wider than a phone).
    fn dim(&self, v: &serde_json::Value, cx: &Cx) -> Option<datars_layout::Dim> {
        let p = Prop(v.clone());
        if p.as_expr().is_some() {
            let px = self.num(&p, cx, f64::NAN);
            return px.is_finite().then(|| datars_layout::Dim::Px(px.max(0.0)));
        }
        datars_layout::Dim::from_json(v)
    }

    /// A repeat's tick-count hint (a number or an expression; 0 lets the scale pick by length).
    fn tick_count(&self, count: &Prop, cx: &Cx) -> usize {
        if count.is_null() {
            return 0;
        }
        let n = self.num(count, cx, 0.0);
        if n.is_finite() { n.round().clamp(0.0, 50.0) as usize } else { 0 }
    }

    /// How many items a repeat makes (without resolving them).
    fn repeat_len(&self, from: &RepeatFrom, cx: &Cx) -> usize {
        match from {
            RepeatFrom::Table(name) => self.table_for(name, cx).map_or(0, |t| t.len()),
            RepeatFrom::Groups { groups, by } => self.table_for(groups, cx).map_or(0, |t| (0..t.len()).map(|i| crate::tables::cell_string(&t, by, i)).collect::<BTreeSet<_>>().len()),
            RepeatFrom::Ticks { ticks, count } => cx.scope.get(ticks).map_or(0, |sc| sc.ticks(self.tick_count(count, cx), &self.doc.locale).len()),
            RepeatFrom::Legend { legend } => cx.scope.get(legend).map_or(0, |sc| sc.domain_values().len()),
            RepeatFrom::Count { count } => self.num(count, cx, 0.0).clamp(0.0, 100_000.0) as usize,
        }
    }

    fn resolve_children(&self, children: &[Template], boxes: &[Rect], cx: &Cx, scope: &Rc<Scope>, path: &KeyPath) -> Vec<Node> {
        let mut out = Vec::new();
        for (i, (c, b)) in children.iter().zip(boxes).enumerate() {
            let ccx = Cx { box_w: b.w, box_h: b.h, scope: scope.clone(), path: path.clone(), ..cx.clone() };
            for mut n in self.resolve(c, &ccx, i) {
                if b.x != 0.0 || b.y != 0.0 {
                    // Content measured with a negative origin (right-aligned labels) keeps its box.
                    n.common.transform = Affine::translate(b.x + self.auto_offset_x(c, &n), b.y + self.auto_offset_y(c, &n)).mul(n.common.transform);
                } else {
                    let (ox, oy) = (self.auto_offset_x(c, &n), self.auto_offset_y(c, &n));
                    if ox != 0.0 || oy != 0.0 {
                        n.common.transform = Affine::translate(ox, oy).mul(n.common.transform);
                    }
                }
                out.push(n);
            }
        }
        out
    }

    /// The box behind a group's resolved content (nothing when the content is empty — an empty
    /// caption hides its card).
    fn backdrop(&self, bd: &datars_ir::TBackdrop, kids: &[Node], cx: &Cx) -> Option<Node> {
        let b = kids.iter().fold(Rect::empty(), |acc, k| acc.union(&node_bounds(k)));
        if b.is_empty() || (b.w <= 0.0 && b.h <= 0.0) {
            return None;
        }
        let [pt, pr, pb, pl] = match &bd.padding.0 {
            serde_json::Value::Array(a) => {
                let v: Vec<f64> = a.iter().map(|x| self.num(&Prop(x.clone()), cx, 0.0)).collect();
                match v.len() {
                    1 => [v[0]; 4],
                    2 => [v[0], v[1], v[0], v[1]],
                    3 => [v[0], v[1], v[2], v[1]],
                    4 => [v[0], v[1], v[2], v[3]],
                    _ => [0.0; 4],
                }
            }
            _ => [self.num(&bd.padding, cx, 0.0); 4],
        };
        let (x, w) = if bd.fit.as_deref() == Some("width") { (0.0, cx.box_w) } else { (b.x - pl, b.w + pl + pr) };
        let r = self.num(&bd.radius, cx, 0.0).max(0.0);
        let mut n = Node::shape(Key::name("backdrop"), Geom::Rect { x, y: b.y - pt, w, h: b.h + pt + pb, r: [r; 4] });
        if let NodeKind::Shape { fill, stroke, .. } = &mut n.kind {
            *fill = self.ink(&bd.fill, cx).map(Paint::Solid);
            *stroke = bd.stroke.as_ref().and_then(|s| self.stroke(s, cx));
        }
        n.semantics = Some(datars_scene::Semantics::new(datars_scene::Role::Decoration, ""));
        Some(n)
    }

    /// Auto-sized content that extends left of its origin (e.g. right-aligned labels) is shifted
    /// so it starts at the box's left edge.
    fn auto_offset_x(&self, t: &Template, n: &Node) -> f64 {
        match t.size.as_ref().and_then(|s| datars_layout::Dim::from_json(&s.w)) {
            Some(datars_layout::Dim::Auto) => (-node_bounds(n).x).max(0.0),
            _ => 0.0,
        }
    }
    fn auto_offset_y(&self, t: &Template, n: &Node) -> f64 {
        match t.size.as_ref().and_then(|s| datars_layout::Dim::from_json(&s.h)) {
            Some(datars_layout::Dim::Auto) => (-node_bounds(n).y).max(0.0),
            _ => 0.0,
        }
    }

    fn camera(&self, c: &TCamera, kids: &[Node], viewport: Rect, cx: &Cx) -> Option<Camera> {
        match c {
            TCamera::Explicit { x, y, zoom } => Some(Camera { x: self.num(x, cx, viewport.w / 2.0), y: self.num(y, cx, viewport.h / 2.0), zoom: self.num(zoom, cx, 1.0), rotation: 0.0 }),
            TCamera::Fit { fit, padding, .. } => {
                let content = if let Some(b) = fit.get("bbox").and_then(|b| b.as_array()) {
                    // Two corners, as numbers or expressions evaluated here: a view that declares a
                    // geo `coord` can frame a lon/lat box with `geo.x(lon, lat)` / `geo.y(lon, lat)`.
                    let v: Vec<f64> = b.iter().map(|x| self.num(&Prop(x.clone()), cx, f64::NAN)).collect();
                    (v.len() == 4 && v.iter().all(|x| x.is_finite())).then(|| Rect::new(v[0].min(v[2]), v[1].min(v[3]), (v[2] - v[0]).abs(), (v[3] - v[1]).abs()))?
                } else if let Some(g) = fit.get("geo") {
                    // A lon/lat box through the view's own `geo` coordinate system.
                    crate::geo::bbox_content(self, g, cx)?
                } else {
                    let keys: Vec<String> = match fit.get("keys") {
                        Some(serde_json::Value::Array(a)) => a.iter().map(|k| k.as_str().map(String::from).unwrap_or_else(|| k.to_string())).collect(),
                        Some(serde_json::Value::String(s)) if s.starts_with('=') => crate::env::decode_keyset(&self.eval(&Prop(serde_json::Value::String(s.clone())), cx)),
                        _ => Vec::new(),
                    };
                    let mut r = Rect::empty();
                    fn walk(n: &Node, parent: Affine, keys: &[String], r: &mut Rect) {
                        let xf = parent.mul(n.common.transform);
                        let k = n.key.parts().first().map(|p| match p {
                            KeyPart::Str(s) => s.to_string(),
                            KeyPart::Int(i) => i.to_string(),
                            KeyPart::Unit { unit } => unit.to_string(),
                        });
                        let wanted = keys.is_empty() || k.as_ref().is_some_and(|k| keys.contains(k));
                        if n.common.pin {
                            // Screen-size content (a callout): only its origin is in content units —
                            // and it counts only when it's one of the keys framed (a label pinned at
                            // every town mustn't frame them all).
                            if wanted {
                                let o = xf.apply(Vec2::ZERO);
                                *r = r.union(&Rect::new(o.x, o.y, 0.0, 0.0));
                            }
                            return;
                        }
                        if wanted && !matches!(n.kind, NodeKind::Group { .. } | NodeKind::View { .. }) {
                                // fit_bounds includes the node's own transform.
                                *r = r.union(&crate::bounds::transform_rect(crate::bounds::fit_bounds(n), &parent));
                                return;
                            }
                        for c in n.children() {
                            walk(c, xf, keys, r);
                        }
                    }
                    for k in kids {
                        walk(k, Affine::IDENTITY, &keys, &mut r);
                    }
                    if r.is_empty() {
                        return None;
                    }
                    r
                };
                Some(Camera::fit(content, viewport, *padding))
            }
        }
    }

    /// Apply exploration signals (`<prefix>.x`, `.y`, `.zoom`) to a fit camera.
    fn explored(&self, cam: Camera, t: Option<&TCamera>) -> Camera {
        let Some(TCamera::Fit { explore: Some(prefix), .. }) = t else { return cam };
        let num = |k: &str| match self.signals.get(&format!("{prefix}.{k}")) {
            Some(Value::Num(n)) if n.is_finite() => Some(*n),
            _ => None,
        };
        Camera { x: num("x").unwrap_or(cam.x), y: num("y").unwrap_or(cam.y), zoom: cam.zoom * num("zoom").unwrap_or(1.0), rotation: cam.rotation }
    }

    fn geom(&self, g: &TGeom, cx: &Cx) -> Option<Geom> {
        let n = |p: &Prop| self.num(p, cx, 0.0);
        Some(match g {
            TGeom::Rect { x, y, w, h, r } => {
                let rv = self.num(r, cx, 0.0);
                Geom::Rect { x: n(x), y: n(y), w: n(w), h: n(h), r: [rv; 4] }
            }
            TGeom::Ellipse { cx: x, cy: y, rx, ry } => Geom::Ellipse { cx: n(x), cy: n(y), rx: n(rx), ry: n(ry) },
            TGeom::Circle { cx: x, cy: y, r } => Geom::circle(n(x), n(y), n(r)),
            TGeom::Arc { cx: x, cy: y, r0, r1, a0, a1 } => Geom::Arc { cx: n(x), cy: n(y), r0: n(r0), r1: n(r1), a0: n(a0), a1: n(a1) },
            TGeom::Segment { x1, y1, x2, y2 } => Geom::Segment { x1: n(x1), y1: n(y1), x2: n(x2), y2: n(y2) },
            TGeom::Polyline { from, x, y, curve, closed } => {
                let pts = self.points(from, x, y, cx)?;
                Geom::Polyline { pts: pts.into(), closed: *closed, curve: curve_of(curve.as_deref()) }
            }
            TGeom::Area { from, x, y0, y1, curve } => {
                let top = self.points(from, x, y1, cx)?;
                let base = self.points(from, x, y0, cx)?;
                Geom::Area { top: top.into(), base: base.into(), curve: curve_of(curve.as_deref()) }
            }
            TGeom::Path { d } => Geom::path(parse_svg_path(&self.string(d, cx))),
            TGeom::Symbol { symbol, x, y, size } => Geom::Symbol { kind: symbol_of(&self.string(symbol, cx)), x: n(x), y: n(y), size: n(size) },
            TGeom::Feature { source, .. } if source.is_empty() => crate::tiles::feature_geom(self, cx)?,
            TGeom::Feature { source, id } => {
                let id = self.string(id, cx);
                crate::geo::feature_geom(self, source, &id, cx)?
            }
        })
    }

    /// Points from the rows of a table (vectorized).
    fn points(&self, from: &str, x: &Prop, y: &Prop, cx: &Cx) -> Option<Vec<Vec2>> {
        let table = self.table_for(from, cx)?;
        let xs = self.column_values(x, &table, cx);
        let ys = self.column_values(y, &table, cx);
        Some(xs.iter().zip(&ys).map(|(a, b)| Vec2::new(*a, *b)).filter(|p| p.is_finite()).collect())
    }

    /// Evaluate a prop for every row of `table` as numbers.
    pub(crate) fn column_values(&self, p: &Prop, table: &Arc<Table>, cx: &Cx) -> Vec<f64> {
        let n = table.len();
        if let Some(src) = p.as_expr() {
            let Some(c) = self.compiled(src) else { return vec![f64::NAN; n] };
            let snapshot = self.snapshot_for(&c.tables, cx);
            let mut env = self.env(cx, &snapshot);
            env.whole = Some(table);
            env.row = None;
            return c.eval_rows(n, &env).iter().map(value_num).collect();
        }
        if let Some(s) = p.as_str() {
            if let Some(Column::Num(v)) = table.column(s) {
                return v.clone();
            }
        }
        vec![self.num(p, cx, f64::NAN); n]
    }

    /// Like [`Resolver::column_values`] through the VM's numeric path (no `Value` per row): for
    /// columns of millions of rows, such as the positions a point pyramid indexes.
    pub(crate) fn column_nums(&self, p: &Prop, table: &Arc<Table>, cx: &Cx) -> Vec<f64> {
        match p.as_expr().and_then(|src| self.compiled(src)) {
            Some(c) => {
                let snapshot = self.snapshot_for(&c.tables, cx);
                let mut env = self.env(cx, &snapshot);
                env.whole = Some(table);
                env.row = None;
                c.eval_rows_num(table.len(), &env)
            }
            None => self.column_values(p, table, cx),
        }
    }

    /// An ink per row: the expression's strings parsed as inks, each distinct string once (rows
    /// mostly share a handful of colours), with no string built per row.
    pub(crate) fn column_inks(&self, p: &Prop, table: &Arc<Table>, cx: &Cx) -> Vec<Ink> {
        let n = table.len();
        let Some(src) = p.as_expr() else { return vec![self.ink(p, cx).unwrap_or_else(|| Ink::token("mark")); n] };
        let Some(c) = self.compiled(src) else { return vec![Ink::token("mark"); n] };
        let snapshot = self.snapshot_for(&c.tables, cx);
        let mut env = self.env(cx, &snapshot);
        env.whole = Some(table);
        env.row = None;
        let mut seen: Vec<(Arc<str>, Ink)> = Vec::new();
        c.eval_rows(n, &env)
            .iter()
            .map(|v| match v {
                Value::Str(s) => match seen.iter().find(|(k, _)| Arc::ptr_eq(k, s) || **k == **s) {
                    Some((_, ink)) => ink.clone(),
                    None => {
                        let ink = Ink::parse(s).unwrap_or_else(|| Ink::token("mark"));
                        if seen.len() < 64 {
                            seen.push((s.clone(), ink.clone()));
                        }
                        ink
                    }
                },
                other => Ink::parse(&str_of(other)).unwrap_or_else(|| Ink::token("mark")),
            })
            .collect()
    }

    pub(crate) fn column_strings(&self, p: &Prop, table: &Arc<Table>, cx: &Cx) -> Vec<String> {
        let n = table.len();
        if let Some(src) = p.as_expr() {
            let Some(c) = self.compiled(src) else { return vec![String::new(); n] };
            let snapshot = self.snapshot_for(&c.tables, cx);
            let mut env = self.env(cx, &snapshot);
            env.whole = Some(table);
            env.row = None;
            return c.eval_rows(n, &env).iter().map(str_of).collect();
        }
        vec![self.string(p, cx); n]
    }

    fn stroke(&self, s: &TStroke, cx: &Cx) -> Option<Stroke> {
        let ink = self.ink(&s.paint, cx)?;
        Some(Stroke {
            paint: Paint::Solid(ink),
            width: self.num(&s.width, cx, 1.0),
            dash: s.dash.clone(),
            cap: match s.cap.as_deref() {
                Some("round") => Cap::Round,
                Some("square") => Cap::Square,
                _ => Cap::Butt,
            },
            join: match s.join.as_deref() {
                Some("round") => Join::Round,
                Some("bevel") => Join::Bevel,
                _ => Join::Miter,
            },
            non_scaling: s.non_scaling,
        })
    }

    fn text(&self, t: &TText, cx: &Cx) -> TextNode {
        let st = &t.style;
        let (family, mut weight) = self.font(&st.font);
        if !st.weight.is_null() {
            weight = self.num(&st.weight, cx, weight as f64) as u16;
        }
        let size = if st.size.is_null() { self.theme.number("size.label").unwrap_or(11.0) } else { self.num(&st.size, cx, 11.0) };
        let ink = self.ink(&st.ink, cx).unwrap_or_else(|| Ink::token("ink"));
        let align = match self.string(&st.align, cx).as_str() {
            "middle" | "center" => Align::Middle,
            "end" | "right" => Align::End,
            _ => Align::Start,
        };
        let baseline = match self.string(&st.baseline, cx).as_str() {
            "middle" => Baseline::Middle,
            "top" | "hanging" => Baseline::Top,
            "bottom" => Baseline::Bottom,
            _ => Baseline::Alphabetic,
        };
        let max_width = (!st.max_width.is_null()).then(|| self.num(&st.max_width, cx, f64::INFINITY));
        let style = TextStyle { family: Arc::from(family.as_str()), weight, size, ink, align, baseline, max_width, line_height: 1.25 };
        let origin = Vec2::new(self.num(&t.at[0], cx, 0.0), self.num(&t.at[1], cx, 0.0));
        let (content, number) = match &t.number {
            Some(num) => {
                let v = self.num(&num.value, cx, f64::NAN);
                let spec = if num.format.is_null() { ",.2~f".to_string() } else { self.string(&num.format, cx) };
                (datars_text::format::number(v, &spec, &self.doc.locale), Some(NumberText { value: v, format: Arc::from(spec.as_str()), locale: Arc::from(self.doc.locale.as_str()) }))
            }
            None => (self.string(&t.text, cx), None),
        };
        let mut node = TextNode::new(content, origin, style);
        node.number = number;
        node.rotate = self.num(&t.rotate, cx, 0.0).to_radians();
        node.contain = !st.contain.is_null() && self.truthy(&st.contain, cx);
        if let Some([dx, dy]) = &t.offset {
            node.offset = Vec2::new(self.num(dx, cx, 0.0), self.num(dy, cx, 0.0));
        }
        if let Some(h) = &t.halo {
            if let Some(i) = self.ink(&h[0], cx) {
                node.halo = Some((i, self.num(&h[1], cx, 2.0)));
            }
        }
        datars_text::layout(self.fonts, &mut node);
        node
    }

    /// A font token (`"font.body"`) or family name → (family stack, weight). Tokens give their
    /// whole stack (a family, then its fallbacks); a family the database has no face for is
    /// diagnosed rather than silently drawn in the fallback face.
    fn font(&self, p: &Prop) -> (String, u16) {
        let name = match p.as_str() {
            Some(s) => s.trim_start_matches('$').to_string(),
            None => "font.body".to_string(),
        };
        match self.theme.font(&name) {
            Some(f) => (f.stack().join(", "), f.weight),
            None => {
                let families = name.split(',').map(|s| s.trim().trim_matches(|c| c == '"' || c == '\'').trim()).filter(|s| !s.is_empty()).collect::<Vec<_>>();
                let generic = |f: &str| matches!(f.to_ascii_lowercase().as_str(), "sans-serif" | "serif" | "monospace" | "cursive" | "fantasy" | "system-ui" | "ui-sans-serif" | "ui-serif" | "ui-monospace" | "ui-rounded" | "emoji" | "math" | "fangsong");
                let first = families.first().copied().unwrap_or_default();
                let known = families.iter().any(|f| !generic(f) && self.fonts.has_family(f));
                if !known && !first.is_empty() && !generic(first) {
                    let message = if name.starts_with("font.") {
                        format!("unknown font token `{name}` (the theme defines font.body, font.strong, font.title, font.number and its own)")
                    } else {
                        let fallback = self.fonts.face_id(&name, 400, false).and_then(|id| self.fonts.faces().into_iter().find(|f| f.id == id)).map(|f| format!("falling back to {}", f.family)).unwrap_or_else(|| "no fonts are loaded".into());
                        format!("font '{first}' has no source; {fallback}. Name a theme font token (`$font.body`) and give it a `src` or `google` family")
                    };
                    let d = Diag { message };
                    let mut diags = self.diags.borrow_mut();
                    if !diags.contains(&d) {
                        diags.push(d);
                    }
                }
                (name, 400)
            }
        }
    }

    fn instances(&self, ti: &TInstances, cx: &Cx) -> Option<Instances> {
        let table = self.table_for(&ti.from, cx)?;
        Some(self.instances_over(ti, &table, cx, false))
    }

    /// One tile of an `lod` node (`points`): the tile's rows through the node's template. Rows
    /// sit at the tile's `x`/`y` and, without an `instance_key`, are keyed by their row number in
    /// the source (`$row`), so an instance keeps its identity whichever tile draws it.
    pub(crate) fn lod_instances(&self, ti: &TInstances, table: &Arc<Table>, cx: &Cx) -> Instances {
        self.instances_over(ti, table, cx, true)
    }

    fn instances_over(&self, ti: &TInstances, table: &Arc<Table>, cx: &Cx, lod: bool) -> Instances {
        let n = table.len();
        let pos = |p: &Prop, col: &str| if lod { table.num(col).map(<[f64]>::to_vec).unwrap_or_else(|| vec![f64::NAN; n]) } else { self.column_values(p, table, cx) };
        let x = pos(&ti.x, "x");
        let y = pos(&ti.y, "y");
        // Point pyramid tiles take the VM's numeric path (no `Value` per row): numeric channels,
        // and inks memoized per distinct string. Other instances keep the general path (a string
        // holding a number counts as that number there).
        let nums = |p: &Prop| if lod { self.column_nums(p, table, cx) } else { self.column_values(p, table, cx) };
        let size = if ti.r.is_null() { vec![self.theme.number("point.radius").unwrap_or(3.0); n] } else { nums(&ti.r) };
        let proto = match ti.proto.as_str() {
            "rect" => Proto::Rect,
            s => Proto::Symbol { symbol: symbol_of(s) },
        };
        let (w, h) = if matches!(proto, Proto::Rect) { (Some(self.column_values(&ti.w, table, cx)), Some(self.column_values(&ti.h, table, cx))) } else { (None, None) };
        let fill: Vec<Ink> = if ti.fill.is_null() {
            vec![Ink::token("mark")]
        } else if lod && ti.fill.as_expr().is_some() {
            self.column_inks(&ti.fill, table, cx)
        } else if ti.fill.as_expr().is_some() {
            // Rows mostly share a handful of inks: parse each distinct one once.
            let mut seen: BTreeMap<String, Ink> = BTreeMap::new();
            self.column_strings(&ti.fill, table, cx).into_iter().map(|s| seen.entry(s).or_insert_with_key(|s| Ink::parse(s).unwrap_or_else(|| Ink::token("mark"))).clone()).collect()
        } else {
            vec![self.ink(&ti.fill, cx).unwrap_or_else(|| Ink::token("mark"))]
        };
        let opacity = if ti.instance_opacity.is_null() { Vec::new() } else { nums(&ti.instance_opacity) };
        let keys: Vec<Key> = if !ti.instance_key.is_null() {
            self.column_strings(&ti.instance_key, table, cx).into_iter().map(|s| Key::one(s.as_str())).collect()
        } else if let (true, Some(rows)) = (lod, table.num("$row")) {
            rows.iter().map(|&r| Key::one(r as i64)).collect()
        } else {
            (0..n).map(|i| crate::tables::row_key(table, i)).collect()
        };
        // Point pyramids label lazily, one hovered row at a time (`points::label`): formatting a
        // tooltip for every row of every tile would cost more than drawing them.
        let lazy = lod && ti.instance_key.is_null() && table.num("$row").is_some();
        let labels = (!ti.label.is_null() && !lazy).then(|| self.column_strings(&ti.label, table, cx));
        let line_reach = match ti.hit.as_deref() {
            None | Some("marks") => None,
            Some("line") => Some(ti.reach.filter(|r| r.is_finite() && *r >= 0.0).unwrap_or(LINE_REACH)),
            Some(other) => {
                self.diag(format!("instances `hit: {other}`: expected `marks` or `line`"));
                None
            }
        };
        Instances { proto, keys, x, y, size, w, h, fill, opacity, stroke: ti.stroke.as_ref().and_then(|s| self.stroke(s, cx)), screen_size: ti.screen_size, labels, line_reach }
    }

    /// The label of row `k` of `table` through `ti`'s template (a point pyramid's hovered row).
    pub(crate) fn lod_label(&self, ti: &TInstances, table: &Arc<Table>, k: usize, cx: &Cx) -> Option<String> {
        if ti.label.is_null() || k >= table.len() {
            return None;
        }
        let one = Arc::new(crate::tables::take_rows(table, &[k]));
        self.column_strings(&ti.label, &one, cx).into_iter().next()
    }

    fn apply_common(&self, n: &mut Node, t: &Template, cx: &Cx, path: &KeyPath) {
        let mut c = Common::default();
        if !t.opacity.is_null() {
            c.opacity = self.num(&t.opacity, cx, 1.0).clamp(0.0, 1.0);
        }
        if let Some(tr) = &t.transform {
            let mut xf = Affine::IDENTITY;
            if !tr.scale.is_null() {
                let s = self.num(&tr.scale, cx, 1.0);
                xf = xf.then(Affine::scale(s, s));
            }
            if !tr.rotate.is_null() {
                xf = xf.then(Affine::rotate(self.num(&tr.rotate, cx, 0.0).to_radians()));
            }
            if let Some([x, y]) = &tr.translate {
                xf = xf.then(Affine::translate(self.num(x, cx, 0.0), self.num(y, cx, 0.0)));
            }
            c.transform = xf;
        }
        c.z = t.z.unwrap_or(0);
        if !t.clip.is_null() {
            if t.clip.as_str() == Some("box") {
                c.clip = Some(Clip::Rect { rect: Rect::new(0.0, 0.0, cx.box_w, cx.box_h) });
            } else if let serde_json::Value::Array(a) = &t.clip.0 {
                let v: Vec<f64> = a.iter().map(|x| self.num(&Prop(x.clone()), cx, 0.0)).collect();
                if v.len() == 4 {
                    c.clip = Some(Clip::Rect { rect: Rect::new(v[0], v[1], v[2], v[3]) });
                }
            }
        }
        if let Some([a, b]) = &t.trim {
            c.trim = Some([self.num(a, cx, 0.0), self.num(b, cx, 1.0)]);
        }
        c.isolate = t.isolate;
        c.pin = t.pin;
        n.common = c;
        if let Some(s) = &t.semantics {
            n.semantics = Some(Semantics {
                role: role_of(&s.role),
                label: self.string(&s.label, cx),
                datum: cx.row.as_ref().map(|r| Arc::from(format!("{}#{}", r.table.name, r.row).as_str())),
                order: None,
                value: (!s.value.is_null()).then(|| self.num(&s.value, cx, f64::NAN)),
                link: (!s.link.is_null()).then(|| Arc::from(self.string(&s.link, cx).as_str())).filter(|l: &Arc<str>| !l.is_empty()),
            });
        }
        n.pickable = t.pickable || !t.on.is_empty();
        for a in &t.anchors {
            n.anchors.push(Anchor { name: Arc::from(a.name.as_str()), at: Vec2::new(self.num(&a.at[0], cx, 0.0), self.num(&a.at[1], cx, 0.0)) });
        }
        if !t.on.is_empty() {
            let mut bound = BTreeMap::new();
            for (intent, act) in &t.on {
                let b = match act {
                    Action::Pick { pick, options, labels } => BoundAction::Pick {
                        signal: pick.clone(),
                        options: options
                            .iter()
                            .enumerate()
                            .map(|(i, v)| {
                                let said = labels.get(i).map(|l| self.string(l, cx)).unwrap_or_else(|| v.as_str().map(String::from).unwrap_or_else(|| v.to_string()));
                                (v.clone(), said)
                            })
                            .collect(),
                    },
                    Action::Set { set, value } => BoundAction::Set { signal: set.clone(), value: self.eval(value, cx) },
                    Action::Toggle { toggle, value } => BoundAction::Toggle { signal: toggle.clone(), value: self.eval(value, cx) },
                    Action::Event { event } => BoundAction::Event { event: event.clone() },
                    Action::Chapter { chapter, key } => BoundAction::Chapter { chapter: chapter.clone(), key: self.string(key, cx) },
                    Action::Scrub { scrub, axis, scale, step, min, max } => {
                        let name = scale.clone().unwrap_or_else(|| axis.clone());
                        let own = (!t.scales.is_empty() && cx.scope.get(&name).is_none()).then(|| crate::scales::build_scope(self, &t.scales, cx, None));
                        let bound = |p: &Prop, none: f64| if p.is_null() { none } else { Some(self.num(p, cx, none)).filter(|v| !v.is_nan()).unwrap_or(none) };
                        let bounds = (bound(min, f64::NEG_INFINITY), bound(max, f64::INFINITY));
                        match own.as_ref().and_then(|s| s.get(&name)).or_else(|| cx.scope.get(&name)) {
                            Some(s) => BoundAction::Scrub { signal: scrub.clone(), axis: usize::from(axis == "y"), map: BrushMap::from_scale(s), step: *step, bounds },
                            None => {
                                self.diag(format!("scrub `{scrub}`: no scale `{name}` here"));
                                continue;
                            }
                        }
                    }
                    Action::Brush { brush, axis, scale } => {
                        let name = scale.clone().unwrap_or_else(|| axis.clone());
                        // The brushed node may declare the scale itself (its own scope isn't the
                        // context it's decorated in).
                        let own = (!t.scales.is_empty() && cx.scope.get(&name).is_none()).then(|| crate::scales::build_scope(self, &t.scales, cx, None));
                        match own.as_ref().and_then(|s| s.get(&name)).or_else(|| cx.scope.get(&name)) {
                            Some(s) => BoundAction::Brush { signal: brush.clone(), axis: usize::from(axis == "y"), map: BrushMap::from_scale(s) },
                            None => {
                                self.diag(format!("brush `{brush}`: no scale `{name}` here"));
                                continue;
                            }
                        }
                    }
                };
                bound.insert(intent.clone(), b);
            }
            self.actions.borrow_mut().push((path.clone(), bound));
        }
    }
}

/// A pinned node is anchored at its origin: that point follows the cameras above, the content is
/// drawn at screen size around it. A pinned text is anchored at its `at` — a place name at a map
/// point stays at the point — so its origin moves into its transform (otherwise the text would
/// be drawn `at` screen px from its parent's origin, far off screen under a zoomed camera).
fn pin_text_at_its_point(n: &mut Node) {
    if !n.common.pin {
        return;
    }
    if let NodeKind::Text(t) = &mut n.kind {
        n.common.transform = n.common.transform.mul(Affine::translate(t.origin.x, t.origin.y));
        t.origin = Vec2::ZERO;
    }
}

/// The `flow` layout: nodes left to right by their measured bounds, `gap` apart, wrapping to a
/// new line when the next one would cross the box's right edge (legends, chips, tag lists).
/// Move a `dodge` group's content to the anchor where it covers the least of everything else a
/// reader needs: data marks and text (earlier anchors win ties, so the first is the preference).
/// Place a dodge group at its least-covered anchor (or at `pin`): the share of it still covering
/// something, the share of everything to read (data and text) it hides, the most of any one text
/// it covers, and the anchor taken.
fn dodge(roots: &mut [Node], base: &KeyPath, d: &Dodge, pin: Option<&str>) -> (f64, f64, f64, Option<String>) {
    // Where the group is, and what it would cover (everything outside it), in canvas px.
    /// Something a placed group shouldn't cover: a text's box (weighted: a hidden label is worse
    /// than a hidden sliver of a bar), a mark's outline (exact, so a pie's empty corners are free),
    /// or a stroked mark (a line: its length under the group, as wide as its stroke — and weighted
    /// like text, since a line hidden for a few px is a gap in the data).
    enum Obstacle {
        Text(Rect),
        Mark(Rect, Vec<Vec<Vec2>>),
        Stroke(Rect, Vec<Vec<Vec2>>, f64),
    }
    fn obstacles(n: &Node, at: &KeyPath, skip: &KeyPath, parent: Affine, out: &mut Vec<Obstacle>) {
        let here = at.push(&n.key);
        if &here == skip || !n.common.visible || n.common.opacity <= 0.05 {
            return;
        }
        let xf = n.common.placed(parent);
        // Data marks, and the marks that draw a whole series (a line, an area).
        let data = n.semantics.as_ref().is_some_and(|s| matches!(s.role, Role::Datum | Role::Region | Role::Series));
        match &n.kind {
            NodeKind::Text(t) if !t.text.trim().is_empty() => out.push(Obstacle::Text(crate::bounds::text_rect(t, &xf))),
            NodeKind::Shape { geom, fill: None, stroke: Some(s), .. } if data => {
                let lines: Vec<Vec<Vec2>> = geom.to_path().transform(&xf).flatten(0.5).into_iter().map(|(r, _)| r).collect();
                out.push(Obstacle::Stroke(crate::bounds::transform_rect(node_bounds(n), &parent), lines, s.width.max(2.0)));
            }
            NodeKind::Shape { geom, .. } if data => {
                let rings: Vec<Vec<Vec2>> = geom.to_path().transform(&xf).flatten(0.5).into_iter().map(|(r, _)| r).collect();
                out.push(Obstacle::Mark(crate::bounds::transform_rect(node_bounds(n), &parent), rings));
            }
            // Instanced marks (points, seats): their boxes (node_bounds includes the own transform).
            NodeKind::Instances(_) => out.push(Obstacle::Mark(crate::bounds::transform_rect(node_bounds(n), &parent), Vec::new())),
            NodeKind::View { viewport, camera, children, .. } => {
                let inner = xf.mul(camera.map(|c| c.transform(*viewport)).unwrap_or(Affine::translate(viewport.x, viewport.y)));
                children.iter().for_each(|c| obstacles(c, &here, skip, inner, out));
            }
            _ => n.children().iter().for_each(|c| obstacles(c, &here, skip, xf, out)),
        }
    }
    let mut obs = Vec::new();
    roots.iter().for_each(|n| obstacles(n, base, &d.path, Affine::IDENTITY, &mut obs));
    let Some((node, xf)) = find_node(roots, &d.path, base, Affine::IDENTITY) else { return (0.0, 0.0, 0.0, None) };
    let Some(kids) = node.children_mut() else { return (0.0, 0.0, 0.0, None) };
    let content = kids.iter().fold(Rect::empty(), |acc, k| acc.union(&node_bounds(k)));
    if content.is_empty() {
        return (0.0, 0.0, 0.0, None);
    }
    let [pt, pr, pb, pl] = d.pad;
    let (w, h) = d.size;
    let spot = |a: &str| -> Vec2 {
        let x = if a.ends_with("left") { pl } else if a.ends_with("right") { w - pr - content.w } else { pl + (w - pl - pr - content.w) / 2.0 };
        let y = if a.starts_with("top") { pt } else if a.starts_with("bottom") { h - pb - content.h } else { pt + (h - pt - pb - content.h) / 2.0 };
        Vec2::new(x, y)
    };
    let covered = |p: Vec2| -> f64 {
        let r = crate::bounds::transform_rect(Rect::new(p.x, p.y, content.w, content.h), &xf);
        obs.iter()
            .map(|o| match o {
                Obstacle::Text(b) => b.intersect(&r).map_or(0.0, |i| 3.0 * i.w * i.h),
                Obstacle::Mark(b, rings) if rings.is_empty() => b.intersect(&r).map_or(0.0, |i| i.w * i.h),
                Obstacle::Mark(b, rings) => {
                    if !b.intersects(&r) {
                        return 0.0;
                    }
                    rings.iter().map(|ring| datars_math::path::signed_area(&clip_to(ring, r)).abs()).sum()
                }
                Obstacle::Stroke(b, lines, width) => {
                    if !b.inset(-width).intersects(&r) {
                        return 0.0;
                    }
                    3.0 * width * lines.iter().map(|l| l.windows(2).map(|s| length_in(s[0], s[1], r)).sum::<f64>()).sum::<f64>()
                }
            })
            .sum()
    };
    let mut best: Option<(f64, Vec2, &String)> = None;
    for a in d.anchors.iter().filter(|a| pin.is_none_or(|p| p == a.as_str())) {
        let p = spot(a);
        let c = covered(p);
        if best.is_none_or(|(bc, _, _)| c < bc - 1e-6) {
            best = Some((c, p, a));
        }
    }
    let Some((c, p, a)) = best else { return (0.0, 0.0, 0.0, None) };
    let (dx, dy) = (p.x - content.x, p.y - content.y);
    for k in kids.iter_mut() {
        k.common.transform = Affine::translate(dx, dy).mul(k.common.transform);
    }
    // Everything there is to read, weighted as covering it counts.
    let total: f64 = obs
        .iter()
        .map(|o| match o {
            Obstacle::Text(b) => 3.0 * b.w * b.h,
            Obstacle::Mark(b, rings) if rings.is_empty() => b.w * b.h,
            Obstacle::Mark(_, rings) => rings.iter().map(|r| datars_math::path::signed_area(r).abs()).sum(),
            Obstacle::Stroke(_, lines, width) => 3.0 * width * lines.iter().map(|l| l.windows(2).map(|s| (s[1] - s[0]).len()).sum::<f64>()).sum::<f64>(),
        })
        .sum();
    // And the text it covers most: a label half hidden is a label lost.
    let r = crate::bounds::transform_rect(Rect::new(p.x, p.y, content.w, content.h), &xf);
    let worst = obs.iter().filter_map(|o| match o {
        Obstacle::Text(b) if b.w * b.h > 0.0 => b.intersect(&r).map(|i| i.w * i.h / (b.w * b.h)),
        _ => None,
    }).fold(0.0, f64::max);
    (c / (content.w * content.h).max(1.0), c / total.max(1.0), worst, Some(a.clone()))
}

/// How much of the segment a–b lies inside `r` (Liang–Barsky).
fn length_in(a: Vec2, b: Vec2, r: Rect) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, a.x - r.x), (dx, r.x1() - a.x), (-dy, a.y - r.y), (dy, r.y1() - a.y)] {
        if p == 0.0 {
            if q < 0.0 {
                return 0.0;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
        }
    }
    if t1 > t0 {
        (t1 - t0) * (dx * dx + dy * dy).sqrt()
    } else {
        0.0
    }
}

/// A polygon clipped to an axis-aligned rectangle (Sutherland–Hodgman).
fn clip_to(poly: &[Vec2], r: Rect) -> Vec<Vec2> {
    let mut out = poly.to_vec();
    for side in 0..4 {
        if out.is_empty() {
            break;
        }
        let inside = |p: Vec2| match side {
            0 => p.x >= r.x,
            1 => p.x <= r.x1(),
            2 => p.y >= r.y,
            _ => p.y <= r.y1(),
        };
        let cross = |p: Vec2, q: Vec2| {
            let t = match side {
                0 => (r.x - p.x) / (q.x - p.x),
                1 => (r.x1() - p.x) / (q.x - p.x),
                2 => (r.y - p.y) / (q.y - p.y),
                _ => (r.y1() - p.y) / (q.y - p.y),
            };
            p.lerp(q, if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 })
        };
        let input = std::mem::take(&mut out);
        for i in 0..input.len() {
            let (p, q) = (input[i], input[(i + 1) % input.len()]);
            match (inside(p), inside(q)) {
                (true, true) => out.push(q),
                (true, false) => out.push(cross(p, q)),
                (false, true) => {
                    out.push(cross(p, q));
                    out.push(q);
                }
                (false, false) => {}
            }
        }
    }
    out
}

/// Where every shown text is drawn, as its four corners in canvas px (through views and cameras).
fn text_quads(n: &Node, parent: Affine, out: &mut Vec<[Vec2; 4]>, core: bool) {
    if !n.common.visible || n.common.opacity <= 0.05 {
        return;
    }
    let xf = n.common.placed(parent);
    match &n.kind {
        NodeKind::Text(t) if !t.text.trim().is_empty() && t.bounds.w > 0.0 => out.push(if core { crate::bounds::text_core(t, &xf) } else { crate::bounds::text_quad(t, &xf) }),
        NodeKind::View { viewport, camera, children, .. } => {
            let inner = xf.mul(camera.map(|c| c.transform(*viewport)).unwrap_or(Affine::translate(viewport.x, viewport.y)));
            children.iter().for_each(|c| text_quads(c, inner, out, core));
        }
        _ => n.children().iter().for_each(|c| text_quads(c, xf, out, core)),
    }
}

/// A rect's corners, in the order [`crate::bounds::text_quad`] gives a text's.
fn quad_of(r: &Rect) -> [Vec2; 4] {
    [Vec2::new(r.x, r.y), Vec2::new(r.x1(), r.y), Vec2::new(r.x1(), r.y1()), Vec2::new(r.x, r.y1())]
}

fn shifted(q: &[Vec2; 4], d: Vec2) -> [Vec2; 4] {
    q.map(|p| p + d)
}

/// Move a text (drawn through `xf`) by `d` canvas px: its origin moves, in its own units.
fn move_text(t: &mut datars_scene::TextNode, xf: &Affine, d: Vec2) {
    let (sx, sy) = (xf.apply_vec(Vec2::new(1.0, 0.0)).x, xf.apply_vec(Vec2::new(0.0, 1.0)).y);
    if (d.x != 0.0 || d.y != 0.0) && sx.abs() > 1e-9 && sy.abs() > 1e-9 {
        t.origin = Vec2::new(t.origin.x + d.x / sx, t.origin.y + d.y / sy);
    }
}

/// Texts marked `contain` that cross the edge of their frame — the canvas, or the map view they're
/// drawn in — move back in, once every transform above them is known. A text set beside its point
/// (a sideways `offset`: a place name right of its dot) first tries the point's other side; what
/// still crosses is nudged in by the overhang. Under a camera only labels whose point is in view
/// are kept in: the label of a place panned away goes with it.
fn contain(n: &mut Node, parent: Affine, frame: Rect, in_view: bool, texts: &[[Vec2; 4]]) {
    const MARGIN: f64 = 2.0;
    let xf = n.common.placed(parent);
    match &mut n.kind {
        NodeKind::Text(t) if t.contain && t.bounds.w > 0.0 => {
            let anchor = xf.apply(t.origin);
            if in_view && !frame.contains(anchor) {
                // Its place is out of view: rather than a label cut at the edge, none.
                if crate::bounds::text_rect(t, &xf).intersects(&frame) {
                    n.common.visible = false;
                }
                return;
            }
            let own = crate::bounds::text_quad(t, &xf);
            let mut r = crate::bounds::text_rect(t, &xf);
            let crosses = |r: &Rect| r.x < frame.x + MARGIN || r.x1() > frame.x1() - MARGIN;
            let mut dx = 0.0;
            // (Not onto another label: a name the author set left of its dot to clear a neighbour
            // stays on that side, nudged in.)
            let clear = |m: &Rect| !texts.iter().any(|o| *o != own && crate::bounds::quads_overlap(o, &quad_of(m), -1.0));
            if t.offset.x != 0.0 && t.rotate == 0.0 && crosses(&r) {
                let mirrored = Rect::new(2.0 * anchor.x - r.x1(), r.y, r.w, r.h);
                if !crosses(&mirrored) && clear(&mirrored) {
                    dx = mirrored.x - r.x;
                    r = mirrored;
                }
            }
            let shift = |lo: f64, hi: f64, min: f64, max: f64| {
                if hi - lo > max - min - 2.0 * MARGIN || lo < min + MARGIN {
                    min + MARGIN - lo
                } else if hi > max - MARGIN {
                    max - MARGIN - hi
                } else {
                    0.0
                }
            };
            dx += shift(r.x, r.x1(), frame.x, frame.x1());
            let dy = shift(r.y, r.y1(), frame.y, frame.y1());
            move_text(t, &xf, Vec2::new(dx, dy));
        }
        NodeKind::View { viewport, camera, children, .. } => {
            let inner = xf.mul(camera.map(|c| c.transform(*viewport)).unwrap_or(Affine::translate(viewport.x, viewport.y)));
            let Some(shown) = crate::bounds::transform_rect(*viewport, &xf).intersect(&frame) else { return };
            children.iter_mut().for_each(|c| contain(c, inner, shown, true, texts));
        }
        _ => {
            if let Some(kids) = n.children_mut() {
                kids.iter_mut().for_each(|c| contain(c, xf, frame, in_view, texts));
            }
        }
    }
}

/// The node at `path` among `nodes` (under `at`), and its transform to the canvas (its own
/// included; through views and their cameras).
fn find_node<'n>(nodes: &'n mut [Node], path: &KeyPath, at: &KeyPath, parent: Affine) -> Option<(&'n mut Node, Affine)> {
    for n in nodes {
        let here = at.push(&n.key);
        let xf = n.common.placed(parent);
        if &here == path {
            return Some((n, xf));
        }
        if path.0.starts_with(&here.0) {
            let inner = match &n.kind {
                NodeKind::View { viewport, camera, .. } => xf.mul(camera.map(|c| c.transform(*viewport)).unwrap_or(Affine::translate(viewport.x, viewport.y))),
                _ => xf,
            };
            if let Some(kids) = n.children_mut() {
                return find_node(kids, path, &here, inner);
            }
        }
    }
    None
}

/// A `declutter` group's texts, in order, kept off each other and off every other text: one that
/// would land on an earlier one tries the other side of its point (below instead of above, left
/// instead of right, when it's offset from the point that way), else it's left out. Overlap is
/// judged on the texts as drawn — turned labels by their slanted outlines.
fn declutter(roots: &mut [Node], path: &KeyPath, all: &[[Vec2; 4]], canvas: Rect) {
    fn children_xf(n: &Node, xf: Affine) -> Affine {
        match &n.kind {
            NodeKind::View { viewport, camera, .. } => xf.mul(camera.map(|c| c.transform(*viewport)).unwrap_or(Affine::translate(viewport.x, viewport.y))),
            _ => xf,
        }
    }
    fn mine(n: &Node, xf: Affine, out: &mut Vec<[Vec2; 4]>) {
        if !n.common.visible || n.common.opacity <= 0.05 {
            return;
        }
        match &n.kind {
            NodeKind::Text(t) if !t.text.trim().is_empty() && t.bounds.w > 0.0 => out.push(crate::bounds::text_core(t, &xf)),
            _ => n.children().iter().for_each(|c| mine(c, c.common.placed(children_xf(n, xf)), out)),
        }
    }
    fn place(n: &mut Node, xf: Affine, fixed: &[[Vec2; 4]], taken: &mut Vec<[Vec2; 4]>, canvas: Rect) {
        if !n.common.visible || n.common.opacity <= 0.05 {
            return;
        }
        let inner = children_xf(n, xf);
        let mut hide = false;
        match &mut n.kind {
            NodeKind::Text(t) if !t.text.trim().is_empty() && t.bounds.w > 0.0 => {
                let q = crate::bounds::text_core(t, &xf);
                let r = crate::bounds::text_rect(t, &xf);
                let a = xf.apply(t.origin);
                // (Clear of the group's placed texts by a px of air; of the rest, not over them.)
                let clear = |m: &[Vec2; 4]| !taken.iter().any(|o| crate::bounds::quads_overlap(o, m, 1.0)) && !fixed.iter().any(|o| crate::bounds::quads_overlap(o, m, -1.0));
                let inside = |m: &[Vec2; 4]| m.iter().all(|p| canvas.inset(-0.5).contains(*p));
                // Where it is (as laid out, even if it crosses the edge), else the other side of its
                // point — only where that side is on the canvas.
                let mut tries = vec![Vec2::ZERO];
                if t.rotate == 0.0 && t.offset.y != 0.0 {
                    tries.push(Vec2::new(0.0, 2.0 * a.y - r.y1() - r.y));
                }
                if t.rotate == 0.0 && t.offset.x != 0.0 {
                    tries.push(Vec2::new(2.0 * a.x - r.x1() - r.x, 0.0));
                }
                match tries.iter().enumerate().find(|(i, d)| clear(&shifted(&q, **d)) && (*i == 0 || inside(&shifted(&q, **d)))).map(|(_, d)| *d) {
                    Some(d) => {
                        move_text(t, &xf, d);
                        taken.push(shifted(&q, d));
                    }
                    None => hide = true,
                }
            }
            _ => {
                if let Some(kids) = n.children_mut() {
                    for c in kids.iter_mut() {
                        let cxf = c.common.placed(inner);
                        place(c, cxf, fixed, taken, canvas);
                    }
                }
            }
        }
        if hide {
            n.common.visible = false;
        }
    }
    let Some((node, xf)) = find_node(roots, path, &KeyPath::default(), Affine::IDENTITY) else { return };
    let mut own = Vec::new();
    mine(node, xf, &mut own);
    // Everything else stays where it is: the group's texts keep off it.
    let fixed: Vec<[Vec2; 4]> = all.iter().filter(|q| !own.contains(q)).copied().collect();
    place(node, xf, &fixed, &mut Vec::new(), canvas);
}

/// Place `nodes` in lines across `inner` (the group's box less its padding), wrapping at its right
/// edge; the first line starts at its top-left corner, as a rows or grid layout's first child does.
fn flow(nodes: &mut [Node], inner: Rect, gap: f64) {
    let (mut x, mut y, mut line_h) = (inner.x, inner.y, 0.0f64);
    for n in nodes {
        let b = node_bounds(n);
        if b.is_empty() {
            continue;
        }
        if x > inner.x && x + b.w > inner.x1() {
            x = inner.x;
            y += line_h + gap;
            line_h = 0.0;
        }
        // Along the line by the measured extent; down by whole lines from the padded top (each
        // node keeps its own vertical origin within its line, so baselines on a line agree).
        n.common.transform = Affine::translate(x - b.x, y).mul(n.common.transform);
        x += b.w + gap;
        line_h = line_h.max(b.h);
    }
}

/// Table names an expression passes to aggregate functions as string literals
/// (`sum('votes', 'share')`, `table.count("t")`).
fn table_refs(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    if !src.contains(['\'', '"']) {
        return out; // no string literal, no table name (the common case, per node and prop)
    }
    // Every `table.*` aggregate (env.rs): one missing here reads a derived table nobody computed.
    for f in ["sum(", "max(", "min(", "mean(", "count(", "first(", "last("] {
        let mut rest = src;
        while let Some(i) = rest.find(f) {
            let after = rest[i + f.len()..].trim_start();
            if let Some(q) = after.chars().next().filter(|c| *c == '\'' || *c == '"') {
                if let Some(end) = after[1..].find(q) {
                    let name = &after[1..1 + end];
                    if !out.iter().any(|n: &String| n == name) {
                        out.push(name.to_string());
                    }
                }
            }
            rest = &rest[i + f.len()..];
        }
    }
    out
}

fn dim_px(d: datars_layout::Dim, avail: f64) -> f64 {
    match d {
        datars_layout::Dim::Px(v) => v,
        datars_layout::Dim::Percent(p) => p * avail,
        _ => avail,
    }
}

pub(crate) fn value_num(v: &Value) -> f64 {
    match v {
        Value::Num(n) => *n,
        Value::Bool(b) => *b as i32 as f64,
        Value::Str(s) => s.parse().unwrap_or(f64::NAN),
        _ => f64::NAN,
    }
}

/// The inverse of [`json_to_value`] (NaN and ±∞ become null).
pub(crate) fn value_to_json(v: &Value) -> serde_json::Value {
    match v {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Num(n) => serde_json::Number::from_f64(*n).map(serde_json::Value::Number).unwrap_or(serde_json::Value::Null),
        Value::Str(s) => serde_json::Value::String(s.to_string()),
    }
}

pub(crate) fn json_to_value(v: &serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => Value::Num(n.as_f64().unwrap_or(f64::NAN)),
        serde_json::Value::String(s) => Value::Str(Arc::from(s.as_str())),
        // Lists are keysets (the one list type signals have): `set: { focus: ["SWE", "NOR"] }`.
        serde_json::Value::Array(a) => crate::env::encode_keyset(&a.iter().map(|x| x.as_str().map(String::from).unwrap_or_else(|| x.to_string())).collect::<Vec<_>>()),
        other => Value::Str(Arc::from(other.to_string().as_str())),
    }
}

pub(crate) fn value_key_part(v: &Value) -> KeyPart {
    match v {
        Value::Num(n) if n.fract() == 0.0 && n.abs() < 9e15 => KeyPart::Int(*n as i64),
        other => KeyPart::str(&str_of(other)),
    }
}

fn json_key_part(v: &serde_json::Value) -> KeyPart {
    match v {
        serde_json::Value::Number(n) if n.as_i64().is_some() => KeyPart::Int(n.as_i64().unwrap_or(0)),
        serde_json::Value::String(s) => KeyPart::str(s),
        serde_json::Value::Object(o) if o.contains_key("unit") => KeyPart::Unit { unit: o["unit"].as_u64().unwrap_or(0) as u32 },
        other => KeyPart::str(&other.to_string()),
    }
}

fn curve_of(s: Option<&str>) -> Curve {
    match s {
        Some("monotone-x") | Some("monotone") => Curve::MonotoneX,
        Some("catmull-rom") | Some("smooth") => Curve::CatmullRom,
        Some("step") => Curve::Step,
        Some("step-before") => Curve::StepBefore,
        Some("step-after") => Curve::StepAfter,
        _ => Curve::Linear,
    }
}

pub(crate) fn symbol_of(s: &str) -> SymbolKind {
    match s {
        "square" => SymbolKind::Square,
        "diamond" => SymbolKind::Diamond,
        "triangle" => SymbolKind::Triangle,
        "cross" => SymbolKind::Cross,
        "star" => SymbolKind::Star,
        _ => SymbolKind::Circle,
    }
}

pub(crate) fn role_of(s: &str) -> Role {
    match s {
        "datum" => Role::Datum,
        "series" => Role::Series,
        "region" => Role::Region,
        "axis" => Role::Axis,
        "tick" => Role::Tick,
        "grid" => Role::Grid,
        "legend" => Role::Legend,
        "legend-item" => Role::LegendItem,
        "annotation" => Role::Annotation,
        "title" => Role::Title,
        "label" => Role::Label,
        "tooltip" => Role::Tooltip,
        "control" => Role::Control,
        "decoration" => Role::Decoration,
        _ => Role::Group,
    }
}

/// A small SVG path parser: M L H V C S Q T A Z, absolute and relative.
pub fn parse_svg_path(d: &str) -> PathData {
    let mut p = PathData::new();
    let bytes = d.as_bytes();
    let mut i = 0;
    let mut cmd = b'M';
    let (mut cur, mut start, mut last_c) = (Vec2::ZERO, Vec2::ZERO, Vec2::ZERO);
    let num = |i: &mut usize| -> Option<f64> {
        while *i < bytes.len() && (bytes[*i].is_ascii_whitespace() || bytes[*i] == b',') {
            *i += 1;
        }
        let st = *i;
        if *i < bytes.len() && (bytes[*i] == b'-' || bytes[*i] == b'+') {
            *i += 1;
        }
        let mut dot = false;
        while *i < bytes.len() && (bytes[*i].is_ascii_digit() || (bytes[*i] == b'.' && !dot)) {
            dot |= bytes[*i] == b'.';
            *i += 1;
        }
        if *i < bytes.len() && (bytes[*i] == b'e' || bytes[*i] == b'E') {
            *i += 1;
            if *i < bytes.len() && (bytes[*i] == b'-' || bytes[*i] == b'+') {
                *i += 1;
            }
            while *i < bytes.len() && bytes[*i].is_ascii_digit() {
                *i += 1;
            }
        }
        std::str::from_utf8(&bytes[st..*i]).ok()?.parse().ok()
    };
    loop {
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b',') {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        if bytes[i].is_ascii_alphabetic() {
            cmd = bytes[i];
            i += 1;
            if cmd == b'Z' || cmd == b'z' {
                p.close();
                cur = start;
                continue;
            }
        }
        let rel = cmd.is_ascii_lowercase();
        let base = if rel { cur } else { Vec2::ZERO };
        let pt = |i: &mut usize| -> Option<Vec2> { Some(Vec2::new(num(i)?, num(i)?)) };
        match cmd.to_ascii_uppercase() {
            b'M' => {
                let Some(q) = pt(&mut i) else { break };
                cur = base + q;
                start = cur;
                p.move_to(cur);
                cmd = if rel { b'l' } else { b'L' };
            }
            b'L' => {
                let Some(q) = pt(&mut i) else { break };
                cur = base + q;
                p.line_to(cur);
            }
            b'H' => {
                let Some(x) = num(&mut i) else { break };
                cur = Vec2::new(if rel { cur.x + x } else { x }, cur.y);
                p.line_to(cur);
            }
            b'V' => {
                let Some(y) = num(&mut i) else { break };
                cur = Vec2::new(cur.x, if rel { cur.y + y } else { y });
                p.line_to(cur);
            }
            b'C' => {
                let (Some(a), Some(b), Some(c)) = (pt(&mut i), pt(&mut i), pt(&mut i)) else { break };
                let (a, b, c) = (base + a, base + b, base + c);
                p.cubic_to(a, b, c);
                last_c = b;
                cur = c;
            }
            b'S' => {
                let (Some(b), Some(c)) = (pt(&mut i), pt(&mut i)) else { break };
                let a = cur * 2.0 - last_c;
                let (b, c) = (base + b, base + c);
                p.cubic_to(a, b, c);
                last_c = b;
                cur = c;
            }
            b'Q' => {
                let (Some(a), Some(c)) = (pt(&mut i), pt(&mut i)) else { break };
                let (a, c) = (base + a, base + c);
                p.quad_to(a, c);
                last_c = a;
                cur = c;
            }
            b'T' => {
                let Some(c) = pt(&mut i) else { break };
                let a = cur * 2.0 - last_c;
                let c = base + c;
                p.quad_to(a, c);
                last_c = a;
                cur = c;
            }
            b'A' => {
                // rx ry rotation large-arc sweep x y. The flags are single digits, which SVG lets
                // run into what follows (`a10 10 0 1017 20`).
                let flag = |i: &mut usize| -> Option<bool> {
                    while *i < bytes.len() && (bytes[*i].is_ascii_whitespace() || bytes[*i] == b',') {
                        *i += 1;
                    }
                    let f = match bytes.get(*i)? {
                        b'0' => false,
                        b'1' => true,
                        _ => return None,
                    };
                    *i += 1;
                    Some(f)
                };
                let (Some(rx), Some(ry), Some(rot), Some(large), Some(sweep), Some(q)) = (num(&mut i), num(&mut i), num(&mut i), flag(&mut i), flag(&mut i), pt(&mut i)) else { break };
                let to = base + q;
                arc_to(&mut p, cur, rx, ry, rot, large, sweep, to);
                cur = to;
            }
            _ => break,
        }
    }
    p
}

/// An SVG elliptical arc from `from` to `to` (SVG 1.1 F.6.5: endpoint to centre form, radii grown
/// when too small to reach), as cubic Béziers of at most a quarter turn each.
#[allow(clippy::too_many_arguments)]
fn arc_to(p: &mut PathData, from: Vec2, rx: f64, ry: f64, rot_deg: f64, large: bool, sweep: bool, to: Vec2) {
    use datars_math::m;
    if from == to {
        return;
    }
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx == 0.0 || ry == 0.0 || !rx.is_finite() || !ry.is_finite() {
        p.line_to(to);
        return;
    }
    let (sin_p, cos_p) = m::sin_cos(rot_deg.to_radians());
    let (dx, dy) = ((from.x - to.x) / 2.0, (from.y - to.y) / 2.0);
    let (x1, y1) = (cos_p * dx + sin_p * dy, -sin_p * dx + cos_p * dy);
    let lambda = x1 * x1 / (rx * rx) + y1 * y1 / (ry * ry);
    if lambda > 1.0 {
        rx *= lambda.sqrt();
        ry *= lambda.sqrt();
    }
    let (rx2, ry2) = (rx * rx, ry * ry);
    let den = rx2 * y1 * y1 + ry2 * x1 * x1;
    let coef = if den > 0.0 { ((rx2 * ry2 - den) / den).max(0.0).sqrt() } else { 0.0 } * if large == sweep { -1.0 } else { 1.0 };
    let (cxp, cyp) = (coef * rx * y1 / ry, -coef * ry * x1 / rx);
    let c = Vec2::new(cos_p * cxp - sin_p * cyp + (from.x + to.x) / 2.0, sin_p * cxp + cos_p * cyp + (from.y + to.y) / 2.0);
    let angle = |ux: f64, uy: f64, vx: f64, vy: f64| m::atan2(ux * vy - uy * vx, ux * vx + uy * vy);
    let (ux, uy) = ((x1 - cxp) / rx, (y1 - cyp) / ry);
    let (vx, vy) = ((-x1 - cxp) / rx, (-y1 - cyp) / ry);
    let start = angle(1.0, 0.0, ux, uy);
    let mut span = angle(ux, uy, vx, vy);
    if !sweep && span > 0.0 {
        span -= m::TAU;
    } else if sweep && span < 0.0 {
        span += m::TAU;
    }
    let at = |a: f64| {
        let (s, co) = m::sin_cos(a);
        (Vec2::new(c.x + rx * cos_p * co - ry * sin_p * s, c.y + rx * sin_p * co + ry * cos_p * s), Vec2::new(-rx * cos_p * s - ry * sin_p * co, -rx * sin_p * s + ry * cos_p * co))
    };
    let n = (span.abs() / (m::PI / 2.0)).ceil().max(1.0) as usize;
    let step = span / n as f64;
    let k = 4.0 / 3.0 * m::tan(step / 4.0);
    for j in 0..n {
        let (a0, a1) = (start + step * j as f64, start + step * (j + 1) as f64);
        let ((p0, d0), (p1, d1)) = (at(a0), at(a1));
        let end = if j + 1 == n { to } else { p1 };
        p.cubic_to(p0 + d0 * k, p1 - d1 * k, end);
    }
}

/// SVG path data for a path (for expressions that return geometry, e.g. `geo.geodesic`).
pub fn svg_path_string(p: &PathData) -> String {
    use datars_math::PathEl;
    let mut out = String::new();
    let f = |v: f64| format!("{:.2}", v);
    for e in &p.els {
        match e {
            PathEl::Move { p } => out += &format!("M{} {}", f(p.x), f(p.y)),
            PathEl::Line { p } => out += &format!("L{} {}", f(p.x), f(p.y)),
            PathEl::Quad { c, p } => out += &format!("Q{} {} {} {}", f(c.x), f(c.y), f(p.x), f(p.y)),
            PathEl::Cubic { c1, c2, p } => out += &format!("C{} {} {} {} {} {}", f(c1.x), f(c1.y), f(c2.x), f(c2.y), f(p.x), f(p.y)),
            PathEl::Close => out += "Z",
        }
    }
    out
}

#[cfg(test)]
mod svg_arc_tests {
    use datars_math::{PathEl, Vec2};

    fn ends(d: &str) -> Vec<Vec2> {
        super::parse_svg_path(d).els.iter().filter_map(|e| match e { PathEl::Move { p } | PathEl::Line { p } | PathEl::Quad { p, .. } | PathEl::Cubic { p, .. } => Some(*p), PathEl::Close => None }).collect()
    }

    #[test]
    fn an_arc_is_a_curve_through_the_circle_not_a_chord() {
        // A half circle of radius 50 from (0, 50) to (100, 50), sweeping through the top (y = 0).
        let p = super::parse_svg_path("M0 50 A50 50 0 0 1 100 50");
        let cubics: Vec<_> = p.els.iter().filter(|e| matches!(e, PathEl::Cubic { .. })).collect();
        assert_eq!(cubics.len(), 2, "two quarter turns: {:?}", p.els);
        let pts = ends("M0 50 A50 50 0 0 1 100 50");
        assert!((pts[1] - Vec2::new(50.0, 0.0)).len() < 1e-9, "through the top: {:?}", pts);
        assert!((pts[2] - Vec2::new(100.0, 50.0)).len() < 1e-12, "ends exactly where it says");
        // The other sweep goes through the bottom.
        assert!((ends("M0 50 A50 50 0 0 0 100 50")[1] - Vec2::new(50.0, 100.0)).len() < 1e-9);
    }

    #[test]
    fn flags_may_run_into_the_numbers_after_them_and_radii_grow_to_reach() {
        // Radius 1 can't span 100 px: it grows to 50 (a half circle), as SVG says.
        let a = ends("M0 50 A1 1 0 0 1 100 50");
        assert!((a[1] - Vec2::new(50.0, 0.0)).len() < 1e-9, "{a:?}");
        // `a50 50 0 01100 0` is rx 50, ry 50, rotation 0, large 0, sweep 1, then 100 0.
        let b = ends("M0 50 a50 50 0 01100 0");
        assert!((b.last().unwrap().x - 100.0).abs() < 1e-9 && (b[1].y - 0.0).abs() < 1e-9, "{b:?}");
    }
}

#[cfg(test)]
mod table_ref_tests {
    #[test]
    fn every_table_aggregate_names_its_table() {
        // A derived table read only through `table.last` must still be computed first.
        let refs = super::table_refs("'on ' + formatDate(table.last('days', 'record_date')) + format(table.first(\"recent\", 't'))");
        assert_eq!(refs, vec!["recent".to_string(), "days".to_string()]);
        assert!(super::table_refs("table.max('a', 'v') + table.count('b')").contains(&"b".to_string()));
    }
}
