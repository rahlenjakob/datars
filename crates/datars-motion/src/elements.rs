//! Flattening a scene into matchable **elements** and **structure**.
//!
//! Every `Shape`, `Text` and `Image` node is an element keyed by its key path; every instance of
//! an `Instances` node is an element keyed by (node key path, instance key) — materialized only
//! when a matcher other than by-path needs it, since by-path instances pair column-wise inside
//! their node. Groups and views are structure. Each element records its transform into the root
//! content space (the coordinate space of the root's children), its accumulated opacity below
//! the root, its bounds and centre there, and its structural parent.
//!
//! Mid-flight scenes (a plan's frame) carry elements flying between containers under a root
//! child keyed `~flight`, nested in groups mirroring their destination path. Flattening strips
//! that marker, so an in-flight element keeps its logical path and a retargeted plan continues
//! it instead of crossfading.

use datars_math::{Affine, Rect, Vec2};
use datars_scene::{Geom, Key, KeyPath, Node, NodeKind, Proto, Role, Scene};
use std::collections::BTreeMap;

/// The key of the flight layer in a plan's frames.
pub const FLIGHT_KEY: &str = "~flight";

pub(crate) fn flight_key() -> Key {
    Key::name(FLIGHT_KEY)
}

/// Where an element's data lives.
#[derive(Clone, Debug)]
pub(crate) enum Src {
    /// A leaf node (clone, without children).
    Node(Box<Node>),
    /// Instance `i` of `Flat::insts[inst]`.
    Instance { inst: usize, i: usize },
}

#[derive(Clone, Debug)]
pub(crate) struct Element {
    pub path: KeyPath,
    /// Logical parent path (the Instances node's path for instances).
    pub parent: KeyPath,
    /// The structural parent (`None` for instances, the root element, and in-flight elements
    /// whose logical parent is not a structure node).
    pub parent_struct: Option<usize>,
    pub key: Key,
    pub src: Src,
    /// Element-local → root content.
    pub xf_root: Affine,
    /// Parent content → root content.
    pub parent_xf: Affine,
    /// Product of ancestor opacities below the root (own excluded).
    pub acc_opacity: f64,
    pub role: Option<Role>,
    pub kind: &'static str,
    pub is_shape: bool,
    /// Root-content bounds and centre.
    pub bounds: Rect,
    pub center: Vec2,
    pub value: Option<f64>,
}

#[derive(Clone, Debug)]
pub(crate) enum Child {
    Struct(usize),
    Elem(usize),
    Inst(usize),
}

#[derive(Clone, Debug)]
pub(crate) struct StructInfo {
    pub path: KeyPath,
    /// The node without children.
    pub node: Node,
    /// Content space → root content (identity for the root).
    pub content_xf: Affine,
    /// Own transform (parent content → own local).
    pub children: Vec<Child>,
    pub bounds: Rect,
}

#[derive(Clone, Debug)]
pub(crate) struct InstNode {
    pub path: KeyPath,
    pub node: Node,
    /// Node-local (the columns' space) → root content.
    pub xf_root: Affine,
    pub acc_opacity: f64,
    pub role: Option<Role>,
    /// First materialized element (if materialized).
    pub elems: Option<usize>,
}

impl InstNode {
    pub fn inst(&self) -> &datars_scene::Instances {
        match &self.node.kind {
            NodeKind::Instances(i) => i,
            _ => unreachable!("InstNode holds an Instances node"),
        }
    }
    /// Instance centre in node-local coordinates.
    pub fn local_center(&self, i: usize) -> Vec2 {
        let ins = self.inst();
        match ins.proto {
            Proto::Rect => {
                let w = ins.w.as_ref().and_then(|v| v.get(i)).copied().unwrap_or(1.0);
                let h = ins.h.as_ref().and_then(|v| v.get(i)).copied().unwrap_or(1.0);
                Vec2::new(ins.x[i] + w / 2.0, ins.y[i] + h / 2.0)
            }
            Proto::Symbol { .. } => Vec2::new(ins.x[i], ins.y[i]),
        }
    }
    pub fn node_opacity(&self) -> f64 {
        if self.node.common.visible {
            self.node.common.opacity
        } else {
            0.0
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Flat {
    pub elems: Vec<Element>,
    /// `structs[0]` is the root: the scene's root group / view, or a virtual group (empty path)
    /// holding a leaf root.
    pub structs: Vec<StructInfo>,
    pub insts: Vec<InstNode>,
    pub virtual_root: bool,
    pub struct_by_path: BTreeMap<KeyPath, usize>,
    pub inst_by_path: BTreeMap<KeyPath, usize>,
}

fn transform_rect(xf: &Affine, r: Rect) -> Rect {
    if r.is_empty() {
        return r;
    }
    let pts = [Vec2::new(r.x, r.y), Vec2::new(r.x1(), r.y), Vec2::new(r.x1(), r.y1()), Vec2::new(r.x, r.y1())];
    pts.iter().fold(Rect::empty(), |acc, p| acc.include(xf.apply(*p)))
}

/// Local bounds and centre of a leaf node's content.
pub(crate) fn content_bounds(n: &Node) -> (Rect, Vec2) {
    match &n.kind {
        NodeKind::Shape { geom, .. } => {
            // A path's bounds walk every point: once, not again for its centre (an arc's centre
            // is on the arc, not its box's).
            let b = geom.bounds();
            (b, if matches!(geom, Geom::Arc { .. }) { geom.center() } else { b.center() })
        }
        NodeKind::Text(t) => {
            let b = if t.bounds.w > 0.0 || t.bounds.h > 0.0 {
                Rect::new(t.origin.x + t.offset.x + t.bounds.x, t.origin.y + t.offset.y + t.bounds.y, t.bounds.w, t.bounds.h)
            } else {
                Rect::new(t.origin.x + t.offset.x, t.origin.y + t.offset.y, 0.0, 0.0)
            };
            (b, b.center())
        }
        NodeKind::Image { rect, .. } => (*rect, rect.center()),
        _ => (Rect::new(0.0, 0.0, 0.0, 0.0), Vec2::ZERO),
    }
}

fn geom_kind(g: &Geom) -> &'static str {
    g.kind_name()
}

pub(crate) fn node_kind(n: &Node) -> (&'static str, bool) {
    match &n.kind {
        NodeKind::Shape { geom, .. } => (geom_kind(geom), true),
        NodeKind::Text(_) => ("text", false),
        NodeKind::Image { .. } => ("image", false),
        NodeKind::Instances(_) => ("instance", false),
        NodeKind::Group { .. } => ("group", false),
        NodeKind::View { .. } => ("view", false),
    }
}

fn eff_opacity(n: &Node) -> f64 {
    if n.common.visible {
        n.common.opacity
    } else {
        0.0
    }
}

fn stripped(n: &Node) -> Node {
    let mut c = Node { key: n.key.clone(), kind: n.kind.clone(), common: n.common.clone(), semantics: n.semantics.clone(), pickable: n.pickable, anchors: n.anchors.clone(), prov: n.prov };
    if let Some(ch) = c.children_mut() {
        ch.clear();
    }
    c
}

fn shallow(n: &Node) -> Node {
    // Clone a node without deep-cloning children (structure is recorded separately).
    let kind = match &n.kind {
        NodeKind::Group { .. } => NodeKind::Group { children: Vec::new() },
        NodeKind::View { viewport, camera, clip, .. } => NodeKind::View { viewport: *viewport, camera: *camera, clip: *clip, children: Vec::new() },
        k => k.clone(),
    };
    Node { key: n.key.clone(), kind, common: n.common.clone(), semantics: n.semantics.clone(), pickable: n.pickable, anchors: n.anchors.clone(), prov: n.prov }
}

/// The transform from a view's content to its parent (as `datars-render` draws it).
pub(crate) fn view_content_xf(n: &Node) -> Affine {
    match &n.kind {
        NodeKind::View { viewport, camera, .. } => camera.map(|c| c.transform(*viewport)).unwrap_or(Affine::translate(viewport.x, viewport.y)),
        _ => Affine::IDENTITY,
    }
}

pub(crate) fn is_struct(n: &Node) -> bool {
    matches!(n.kind, NodeKind::Group { .. } | NodeKind::View { .. })
}

impl Flat {
    pub fn new(scene: &Scene) -> Flat {
        let mut f = Flat::default();
        let root = &scene.root;
        let root_path = KeyPath(vec![root.key.clone()]);
        if is_struct(root) {
            f.structs.push(StructInfo { path: root_path.clone(), node: shallow(root), content_xf: Affine::IDENTITY, children: Vec::new(), bounds: Rect::empty() });
            f.struct_by_path.insert(root_path.clone(), 0);
            for c in root.children() {
                if c.key == flight_key() && is_struct(c) {
                    let xf = c.common.transform;
                    let op = eff_opacity(c);
                    for cc in c.children() {
                        f.walk_flight(cc, &root_path, xf, op);
                    }
                } else {
                    f.walk(c, &root_path, Some(0), Affine::IDENTITY, 1.0);
                }
            }
        } else {
            f.virtual_root = true;
            f.structs.push(StructInfo {
                path: KeyPath::default(),
                node: Node::group(Key::default(), Vec::new()),
                content_xf: Affine::IDENTITY,
                children: Vec::new(),
                bounds: Rect::empty(),
            });
            f.struct_by_path.insert(KeyPath::default(), 0);
            f.walk(root, &KeyPath::default(), Some(0), Affine::IDENTITY, 1.0);
        }
        // In-flight elements find their logical structural parent.
        for e in &mut f.elems {
            if e.parent_struct.is_none() {
                if let Src::Node(_) = e.src {
                    e.parent_struct = f.struct_by_path.get(&e.parent).copied();
                }
            }
        }
        f.compute_struct_bounds();
        f
    }

    fn walk(&mut self, n: &Node, parent_path: &KeyPath, parent: Option<usize>, parent_xf: Affine, acc: f64) {
        let path = parent_path.push(&n.key);
        match &n.kind {
            NodeKind::Group { children } | NodeKind::View { children, .. } => {
                let own = parent_xf.mul(n.common.transform);
                let content = own.mul(view_content_xf(n));
                let ix = self.structs.len();
                self.structs.push(StructInfo { path: path.clone(), node: shallow(n), content_xf: content, children: Vec::new(), bounds: Rect::empty() });
                // Duplicate structure paths: the last wins for lookups (matching the element rule).
                self.struct_by_path.insert(path.clone(), ix);
                if let Some(p) = parent {
                    self.structs[p].children.push(Child::Struct(ix));
                }
                let acc2 = acc * eff_opacity(n);
                for c in children {
                    self.walk(c, &path, Some(ix), content, acc2);
                }
            }
            NodeKind::Instances(_) => {
                let ix = self.insts.len();
                self.insts.push(InstNode {
                    path: path.clone(),
                    node: stripped(n),
                    xf_root: parent_xf.mul(n.common.transform),
                    acc_opacity: acc,
                    role: n.semantics.as_ref().map(|s| s.role),
                    elems: None,
                });
                self.inst_by_path.insert(path, ix);
                if let Some(p) = parent {
                    self.structs[p].children.push(Child::Inst(ix));
                }
            }
            _ => {
                if let Some(ix) = self.leaf(n, path, parent_path.clone(), parent, parent_xf, acc) {
                    if let Some(p) = parent {
                        self.structs[p].children.push(Child::Elem(ix));
                    }
                }
            }
        }
    }

    /// Walk the flight layer: mirror groups are not structure; leaves keep logical paths.
    fn walk_flight(&mut self, n: &Node, logical_parent: &KeyPath, xf: Affine, acc: f64) {
        match &n.kind {
            NodeKind::Group { children } | NodeKind::View { children, .. } => {
                let p = logical_parent.push(&n.key);
                let own = xf.mul(n.common.transform).mul(view_content_xf(n));
                let acc2 = acc * eff_opacity(n);
                for c in children {
                    self.walk_flight(c, &p, own, acc2);
                }
            }
            NodeKind::Instances(_) => {
                // Instances never fly as nodes; keep them as structure-less column sources.
                let path = logical_parent.push(&n.key);
                let ix = self.insts.len();
                self.insts.push(InstNode {
                    path: path.clone(),
                    node: stripped(n),
                    xf_root: xf.mul(n.common.transform),
                    acc_opacity: acc,
                    role: n.semantics.as_ref().map(|s| s.role),
                    elems: None,
                });
                self.inst_by_path.entry(path).or_insert(ix);
            }
            _ => {
                let path = logical_parent.push(&n.key);
                self.leaf(n, path, logical_parent.clone(), None, xf, acc);
            }
        }
    }

    fn leaf(&mut self, n: &Node, path: KeyPath, parent_path: KeyPath, parent: Option<usize>, parent_xf: Affine, acc: f64) -> Option<usize> {
        let (kind, is_shape) = node_kind(n);
        let xf_root = parent_xf.mul(n.common.transform);
        let (lb, lc) = content_bounds(n);
        let ix = self.elems.len();
        self.elems.push(Element {
            key: n.key.clone(),
            path,
            parent: parent_path,
            parent_struct: parent,
            src: Src::Node(Box::new(stripped(n))),
            xf_root,
            parent_xf,
            acc_opacity: acc,
            role: n.semantics.as_ref().map(|s| s.role),
            kind,
            is_shape,
            bounds: transform_rect(&xf_root, lb),
            center: xf_root.apply(lc),
            value: n.semantics.as_ref().and_then(|s| s.value),
        });
        Some(ix)
    }

    /// Materialize one element per instance of `insts[k]` (for matchers other than by-path).
    pub fn materialize(&mut self, k: usize) -> std::ops::Range<usize> {
        if let Some(start) = self.insts[k].elems {
            return start..start + self.insts[k].inst().len();
        }
        let start = self.elems.len();
        let n = self.insts[k].inst().len();
        self.elems.reserve(n);
        for i in 0..n {
            let inode = &self.insts[k];
            let ins = inode.inst();
            let key = ins.keys[i].clone();
            let g = ins.geom(i);
            let lb = g.bounds();
            let lc = inode.local_center(i);
            let xf = inode.xf_root;
            let e = Element {
                path: inode.path.push(&key),
                parent: inode.path.clone(),
                parent_struct: None,
                key,
                src: Src::Instance { inst: k, i },
                xf_root: xf,
                parent_xf: xf,
                acc_opacity: inode.acc_opacity * inode.node_opacity(),
                role: inode.role,
                kind: "instance",
                is_shape: false,
                bounds: transform_rect(&xf, lb),
                center: xf.apply(lc),
                value: None,
            };
            self.elems.push(e);
        }
        self.insts[k].elems = Some(start);
        start..start + n
    }

    fn compute_struct_bounds(&mut self) {
        // Children always have larger indices than their parent: fold bottom-up.
        for s in (0..self.structs.len()).rev() {
            let mut b = Rect::empty();
            for c in &self.structs[s].children {
                let cb = match c {
                    Child::Struct(i) => self.structs[*i].bounds,
                    Child::Elem(i) => self.elems[*i].bounds,
                    Child::Inst(i) => {
                        let inode = &self.insts[*i];
                        let ins = inode.inst();
                        let mut r = Rect::empty();
                        for k in 0..ins.len() {
                            r = r.union(&transform_rect(&inode.xf_root, ins.geom(k).bounds()));
                        }
                        r
                    }
                };
                b = b.union(&cb);
            }
            self.structs[s].bounds = b;
        }
    }

    /// Element lookup by path (the last occurrence wins for duplicate paths).
    pub fn elem_paths(&self, idx: impl Iterator<Item = usize>) -> BTreeMap<&KeyPath, usize> {
        let mut m = BTreeMap::new();
        for i in idx {
            m.insert(&self.elems[i].path, i);
        }
        m
    }
}
