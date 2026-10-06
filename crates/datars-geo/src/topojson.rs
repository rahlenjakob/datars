//! TopoJSON: geometry as shared arcs.
//!
//! A `Topology` keeps the arcs (dequantized, delta-decoded, in real coordinates) and the objects
//! that reference them, so a border between two regions exists exactly once. That is what makes
//! **topology-preserving simplification** possible: each arc is simplified once and every polygon
//! that uses it sees the same vertices, so neighbours never gap or overlap at any zoom band.

use crate::geojson::{position, GeoJsonOptions};
use crate::geometry::{json_to_id, Feature, FeatureCollection, Geometry, Line, Polygon};
use crate::simplify::{filter_by_weight, vertex_weights, SimplifyMethod};
use crate::GeoError;
use datars_math::Vec2;
use serde_json::Value;
use std::collections::BTreeMap;

/// An arc reference: `i` is arc `i` forwards, `!i` (i.e. `-i - 1`) is arc `i` reversed.
pub type ArcRef = i32;

/// The quantization transform a topology was encoded with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TopoTransform {
    pub scale: Vec2,
    pub translate: Vec2,
}

impl TopoTransform {
    pub fn apply(&self, q: Vec2) -> Vec2 {
        match self.decimal() {
            Some(div) => Vec2::new(q.x / div, q.y / div),
            None => Vec2::new(q.x * self.scale.x + self.translate.x, q.y * self.scale.y + self.translate.y),
        }
    }

    /// A decimal quantization (scale 10^-d on both axes, no translation) decodes by dividing by
    /// 10^d: `3129 / 100` is exactly the double `31.29` parses to, `3129 * 0.01` is not. Topologies
    /// written by [`crate::topo_encode`] rely on it to be bit-identical to their GeoJSON.
    pub fn decimal(&self) -> Option<f64> {
        if self.translate != Vec2::ZERO || self.scale.x != self.scale.y {
            return None;
        }
        let mut p = 1.0;
        for _ in 0..=12 {
            if 1.0 / p == self.scale.x {
                return Some(p);
            }
            p *= 10.0;
        }
        None
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TopoShape {
    Null,
    Point(Vec2),
    MultiPoint(Vec<Vec2>),
    LineString(Vec<ArcRef>),
    MultiLineString(Vec<Vec<ArcRef>>),
    Polygon(Vec<Vec<ArcRef>>),
    MultiPolygon(Vec<Vec<Vec<ArcRef>>>),
    Collection(Vec<TopoGeometry>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct TopoGeometry {
    pub id: Option<String>,
    pub properties: BTreeMap<String, Value>,
    pub shape: TopoShape,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Topology {
    /// The quantization the file used (arcs and points here are already dequantized).
    pub transform: Option<TopoTransform>,
    /// Arcs in real coordinates.
    pub arcs: Vec<Vec<Vec2>>,
    /// Named objects, usually `GeometryCollection`s ("countries", "states", …).
    pub objects: BTreeMap<String, TopoGeometry>,
}

/// Which arcs `Topology::mesh` returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshFilter {
    /// Every arc used by the object.
    All,
    /// Arcs shared by two different geometries: internal borders, drawn once.
    Interior,
    /// Arcs used by a single geometry: the outline (coasts, outer borders).
    Exterior,
}

/// Vertex weights for every arc plus the minimum vertex count each arc must keep so every ring
/// that uses it stays a valid ring. Computed once, filtered per tolerance / zoom band.
#[derive(Clone, Debug, PartialEq)]
pub struct ArcWeights {
    pub weights: Vec<Vec<f64>>,
    pub min_keep: Vec<usize>,
}

/// A zoom band and the tolerance its geometry is simplified to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZoomBand {
    pub min_zoom: u8,
    pub max_zoom: u8,
    /// In coordinate units (degrees for lon/lat topologies).
    pub tolerance: f64,
}

impl ZoomBand {
    /// A band whose tolerance is `pixels` at its finest zoom (Web-Mercator degrees per pixel), so
    /// no zoom in the band shows simplification error larger than that.
    pub fn for_pixels(min_zoom: u8, max_zoom: u8, pixels: f64) -> ZoomBand {
        ZoomBand { min_zoom, max_zoom, tolerance: crate::simplify::zoom_tolerance(max_zoom as f64, pixels) }
    }
}

/// Parse a TopoJSON `Topology`. `opts.id_property` works as for GeoJSON.
pub fn parse_topojson(bytes: &[u8], opts: &GeoJsonOptions) -> Result<Topology, GeoError> {
    let root: Value = serde_json::from_slice(bytes).map_err(|e| GeoError::Json(e.to_string()))?;
    if root.get("type").and_then(Value::as_str) != Some("Topology") {
        return Err(GeoError::format("not a TopoJSON Topology"));
    }
    let transform = match root.get("transform") {
        None | Some(Value::Null) => None,
        Some(t) => {
            let s = t.get("scale").map(position).transpose()?;
            let tr = t.get("translate").map(position).transpose()?;
            match (s, tr) {
                (Some(scale), Some(translate)) => Some(TopoTransform { scale, translate }),
                _ => return Err(GeoError::format("transform needs `scale` and `translate`")),
            }
        }
    };
    let raw_arcs = root.get("arcs").and_then(Value::as_array).ok_or_else(|| GeoError::format("Topology without `arcs`"))?;
    let mut arcs = Vec::with_capacity(raw_arcs.len());
    for a in raw_arcs {
        let pts = a.as_array().ok_or_else(|| GeoError::format("an arc must be an array of positions"))?;
        let mut out = Vec::with_capacity(pts.len());
        let mut acc = Vec2::ZERO;
        let div = transform.as_ref().and_then(TopoTransform::decimal);
        for p in pts {
            let q = position(p)?;
            out.push(match transform {
                Some(t) => {
                    acc += q; // delta-encoded
                    match div {
                        Some(d) => Vec2::new(acc.x / d, acc.y / d),
                        None => t.apply(acc),
                    }
                }
                None => q,
            });
        }
        arcs.push(out);
    }
    let n_arcs = arcs.len();
    let mut objects = BTreeMap::new();
    let objs = root.get("objects").and_then(Value::as_object).ok_or_else(|| GeoError::format("Topology without `objects`"))?;
    for (name, o) in objs {
        let g = topo_geometry(o, transform.as_ref(), n_arcs, opts).map_err(|e| match e {
            GeoError::Format(m) => GeoError::Format(format!("objects.{name}: {m}")),
            e => e,
        })?;
        objects.insert(name.clone(), g);
    }
    Ok(Topology { transform, arcs, objects })
}

fn topo_geometry(o: &Value, t: Option<&TopoTransform>, n_arcs: usize, opts: &GeoJsonOptions) -> Result<TopoGeometry, GeoError> {
    let kind = o.get("type").and_then(Value::as_str).unwrap_or("null");
    let properties: BTreeMap<String, Value> = match o.get("properties") {
        Some(Value::Object(m)) => m.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        _ => BTreeMap::new(),
    };
    let id = opts
        .id_property
        .as_deref()
        .and_then(|k| properties.get(k))
        .and_then(json_to_id)
        .or_else(|| o.get("id").and_then(json_to_id));
    let point = |v: &Value| -> Result<Vec2, GeoError> {
        let q = position(v)?;
        Ok(t.map_or(q, |t| t.apply(q))) // points are quantized but not delta-encoded
    };
    let shape = match kind {
        "null" => TopoShape::Null,
        "Point" => TopoShape::Point(point(o.get("coordinates").ok_or_else(|| GeoError::format("Point without coordinates"))?)?),
        "MultiPoint" => TopoShape::MultiPoint(
            o.get("coordinates")
                .and_then(Value::as_array)
                .ok_or_else(|| GeoError::format("MultiPoint without coordinates"))?
                .iter()
                .map(point)
                .collect::<Result<_, _>>()?,
        ),
        "LineString" => TopoShape::LineString(arc_refs(arcs_field(o, kind)?, n_arcs)?),
        "MultiLineString" => TopoShape::MultiLineString(arc_ref_lists(arcs_field(o, kind)?, n_arcs)?),
        "Polygon" => TopoShape::Polygon(arc_ref_lists(arcs_field(o, kind)?, n_arcs)?),
        "MultiPolygon" => TopoShape::MultiPolygon(
            as_array(arcs_field(o, kind)?)?.iter().map(|p| arc_ref_lists(p, n_arcs)).collect::<Result<_, _>>()?,
        ),
        "GeometryCollection" => TopoShape::Collection(
            o.get("geometries")
                .and_then(Value::as_array)
                .ok_or_else(|| GeoError::format("GeometryCollection without geometries"))?
                .iter()
                .map(|g| topo_geometry(g, t, n_arcs, opts))
                .collect::<Result<_, _>>()?,
        ),
        other => return Err(GeoError::format(format!("unknown geometry type `{other}`"))),
    };
    Ok(TopoGeometry { id, properties, shape })
}

fn arcs_field<'a>(o: &'a Value, kind: &str) -> Result<&'a Value, GeoError> {
    o.get("arcs").ok_or_else(|| GeoError::format(format!("{kind} without `arcs`")))
}

fn as_array(v: &Value) -> Result<&Vec<Value>, GeoError> {
    v.as_array().ok_or_else(|| GeoError::format("arc indices must be arrays"))
}

/// A list of arc references, validated against the arc count.
fn arc_refs(v: &Value, n_arcs: usize) -> Result<Vec<ArcRef>, GeoError> {
    as_array(v)?
        .iter()
        .map(|x| {
            let i = x.as_i64().ok_or_else(|| GeoError::format("arc index must be an integer"))?;
            let idx = if i < 0 { !i } else { i };
            if idx as usize >= n_arcs || i32::try_from(i).is_err() {
                return Err(GeoError::format(format!("arc index {i} out of range")));
            }
            Ok(i as ArcRef)
        })
        .collect()
}

fn arc_ref_lists(v: &Value, n_arcs: usize) -> Result<Vec<Vec<ArcRef>>, GeoError> {
    as_array(v)?.iter().map(|l| arc_refs(l, n_arcs)).collect()
}

fn arc_index(r: ArcRef) -> usize {
    (if r < 0 { !r } else { r }) as usize
}

impl Topology {
    pub fn object_names(&self) -> impl Iterator<Item = &str> {
        self.objects.keys().map(String::as_str)
    }

    /// An object as features: a `GeometryCollection` gives one feature per member, anything else
    /// a single feature.
    pub fn features(&self, object: &str) -> Option<FeatureCollection> {
        let o = self.objects.get(object)?;
        let feature = |g: &TopoGeometry| Feature { id: g.id.clone(), properties: g.properties.clone(), geometry: self.geometry(g) };
        Some(FeatureCollection {
            features: match &o.shape {
                TopoShape::Collection(gs) => gs.iter().map(feature).collect(),
                _ => vec![feature(o)],
            },
        })
    }

    /// Resolve a topology geometry into coordinates.
    pub fn geometry(&self, g: &TopoGeometry) -> Geometry {
        match &g.shape {
            TopoShape::Null => Geometry::default(),
            TopoShape::Point(p) => Geometry::Point(*p),
            TopoShape::MultiPoint(ps) => Geometry::MultiPoint(ps.clone()),
            TopoShape::LineString(a) => Geometry::LineString(self.line(a)),
            TopoShape::MultiLineString(ls) => Geometry::MultiLineString(ls.iter().map(|a| self.line(a)).collect()),
            TopoShape::Polygon(rs) => Geometry::Polygon(self.polygon(rs)),
            TopoShape::MultiPolygon(ps) => Geometry::MultiPolygon(ps.iter().map(|rs| self.polygon(rs)).collect()),
            TopoShape::Collection(gs) => Geometry::Collection(gs.iter().map(|g| self.geometry(g)).collect()),
        }
    }

    /// The points of one arc reference, reversed for negative references.
    pub fn arc_points(&self, r: ArcRef) -> Vec<Vec2> {
        let mut pts = self.arcs.get(arc_index(r)).cloned().unwrap_or_default();
        if r < 0 {
            pts.reverse();
        }
        pts
    }

    /// Stitch arcs end to end (each arc after the first drops its duplicated start point).
    fn line(&self, refs: &[ArcRef]) -> Line {
        let mut pts: Vec<Vec2> = Vec::new();
        for &r in refs {
            let a = self.arc_points(r);
            let skip = usize::from(!pts.is_empty() && !a.is_empty());
            pts.extend_from_slice(&a[skip..]);
        }
        if pts.len() == 1 {
            pts.push(pts[0]);
        }
        pts
    }

    fn polygon(&self, rings: &[Vec<ArcRef>]) -> Polygon {
        rings
            .iter()
            .map(|r| {
                let mut ring = self.line(r);
                if let (Some(&a), Some(&b)) = (ring.first(), ring.last()) {
                    if a != b {
                        ring.push(a);
                    }
                }
                while !ring.is_empty() && ring.len() < 4 {
                    ring.push(ring[0]);
                }
                ring
            })
            .collect()
    }

    /// The object's arcs as lines, filtered by how many of its geometries use them (topojson's
    /// `mesh`): `Interior` gives every internal border exactly once.
    pub fn mesh(&self, object: &str, filter: MeshFilter) -> Option<Geometry> {
        let o = self.objects.get(object)?;
        // arc → the geometries (by index among the object's leaves) using it
        let mut users: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        let mut leaf = 0usize;
        fn walk(g: &TopoGeometry, leaf: &mut usize, users: &mut BTreeMap<usize, Vec<usize>>) {
            let mut add = |refs: &[ArcRef], leaf: usize| {
                for &r in refs {
                    let u = users.entry(arc_index(r)).or_default();
                    if u.last() != Some(&leaf) {
                        u.push(leaf);
                    }
                }
            };
            match &g.shape {
                TopoShape::Collection(gs) => {
                    for g in gs {
                        walk(g, leaf, users);
                    }
                    return;
                }
                TopoShape::LineString(a) => add(a, *leaf),
                TopoShape::MultiLineString(ls) | TopoShape::Polygon(ls) => ls.iter().for_each(|a| add(a, *leaf)),
                TopoShape::MultiPolygon(ps) => ps.iter().flatten().for_each(|a| add(a, *leaf)),
                _ => {}
            }
            *leaf += 1;
        }
        walk(o, &mut leaf, &mut users);
        let lines = users
            .iter()
            .filter(|(_, u)| match filter {
                MeshFilter::All => true,
                MeshFilter::Interior => u.len() > 1,
                MeshFilter::Exterior => u.len() == 1,
            })
            .map(|(&a, _)| self.arcs[a].clone())
            .collect();
        Some(Geometry::MultiLineString(lines))
    }

    /// Weigh every arc once (see `simplify`), and work out how many vertices each arc must keep
    /// so the rings built from it stay valid: a ring made of one arc needs 4 points, of two arcs
    /// 3 each, otherwise every arc keeps at least its endpoints.
    pub fn presimplify(&self, method: SimplifyMethod) -> ArcWeights {
        let weights = self.arcs.iter().map(|a| vertex_weights(a, method)).collect();
        let mut min_keep = vec![2usize; self.arcs.len()];
        fn walk(g: &TopoGeometry, min_keep: &mut [usize]) {
            let mut ring = |refs: &[ArcRef]| {
                let need = match refs.len() {
                    1 => 4,
                    2 => 3,
                    _ => 2,
                };
                for &r in refs {
                    let k = &mut min_keep[arc_index(r)];
                    *k = (*k).max(need);
                }
            };
            match &g.shape {
                TopoShape::Polygon(rs) => rs.iter().for_each(|r| ring(r)),
                TopoShape::MultiPolygon(ps) => ps.iter().flatten().for_each(|r| ring(r)),
                TopoShape::Collection(gs) => gs.iter().for_each(|g| walk(g, min_keep)),
                _ => {}
            }
        }
        for o in self.objects.values() {
            walk(o, &mut min_keep);
        }
        ArcWeights { weights, min_keep }
    }

    /// Topology-preserving simplification: every shared arc is simplified once, so neighbours
    /// keep identical borders. Arc endpoints (the junctions between regions) never move.
    pub fn simplify(&self, tolerance: f64, method: SimplifyMethod) -> Topology {
        self.simplify_with(&self.presimplify(method), tolerance)
    }

    /// `simplify` with precomputed weights (cheap per call — one pass over the vertices).
    pub fn simplify_with(&self, w: &ArcWeights, tolerance: f64) -> Topology {
        let arcs = self
            .arcs
            .iter()
            .enumerate()
            .map(|(i, a)| match (w.weights.get(i), w.min_keep.get(i)) {
                (Some(wi), Some(&k)) => filter_by_weight(a, wi, tolerance, k),
                _ => a.clone(),
            })
            .collect();
        Topology { transform: self.transform, arcs, objects: self.objects.clone() }
    }

    /// One simplified topology per zoom band, all from a single weighting pass.
    pub fn simplify_bands(&self, bands: &[ZoomBand], method: SimplifyMethod) -> Vec<(ZoomBand, Topology)> {
        let w = self.presimplify(method);
        bands.iter().map(|b| (*b, self.simplify_with(&w, b.tolerance))).collect()
    }

    /// Total vertex count over all arcs (a size measure for LOD budgets).
    pub fn vertex_count(&self) -> usize {
        self.arcs.iter().map(Vec::len).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two unit squares side by side sharing the edge x = 1 (arc 0), quantized.
    const TWO: &str = r#"{"type":"Topology",
      "transform":{"scale":[0.5,0.5],"translate":[10,20]},
      "objects":{"regions":{"type":"GeometryCollection","geometries":[
        {"type":"Polygon","id":"A","properties":{"name":"West"},"arcs":[[0,1]]},
        {"type":"Polygon","id":"B","properties":{"name":"East"},"arcs":[[-1,2]]},
        {"type":"Point","coordinates":[1,1]}
      ]}},
      "arcs":[
        [[2,0],[0,2]],
        [[2,2],[-2,0],[0,-2],[2,0]],
        [[2,0],[2,0],[0,2],[-2,0]]
      ]}"#;

    #[test]
    fn decodes_deltas_transform_and_shared_arcs() {
        let t = parse_topojson(TWO.as_bytes(), &GeoJsonOptions::default()).unwrap();
        assert_eq!(t.arcs[0], vec![Vec2::new(11.0, 20.0), Vec2::new(11.0, 21.0)]);
        let fc = t.features("regions").unwrap();
        assert_eq!(fc.len(), 3);
        let Geometry::Polygon(a) = &fc.features[0].geometry else { panic!() };
        let Geometry::Polygon(b) = &fc.features[1].geometry else { panic!() };
        assert_eq!(a[0].first(), a[0].last());
        assert_eq!(a[0].len(), 5);
        assert_eq!(b[0].len(), 5);
        assert!(a[0].contains(&Vec2::new(10.0, 21.0)) && b[0].contains(&Vec2::new(12.0, 20.0)));
        assert_eq!(fc.features[1].id.as_deref(), Some("B"));
        assert_eq!(fc.features[2].geometry, Geometry::Point(Vec2::new(10.5, 20.5)), "points are not delta-coded");
        let by_name = parse_topojson(TWO.as_bytes(), &GeoJsonOptions::id_from("name")).unwrap();
        assert_eq!(by_name.features("regions").unwrap().features[0].id.as_deref(), Some("West"));
    }

    #[test]
    fn mesh_separates_internal_borders() {
        let t = parse_topojson(TWO.as_bytes(), &GeoJsonOptions::default()).unwrap();
        assert_eq!(t.mesh("regions", MeshFilter::Interior).unwrap().lines().len(), 1);
        assert_eq!(t.mesh("regions", MeshFilter::Exterior).unwrap().lines().len(), 2);
        assert_eq!(t.mesh("regions", MeshFilter::All).unwrap().lines().len(), 3);
        assert!(t.mesh("nope", MeshFilter::All).is_none());
    }

    #[test]
    fn rejects_bad_topologies() {
        let o = GeoJsonOptions::default();
        assert!(parse_topojson(br#"{"type":"FeatureCollection"}"#, &o).is_err());
        assert!(parse_topojson(br#"{"type":"Topology","arcs":[],"objects":{"x":{"type":"LineString","arcs":[3]}}}"#, &o).is_err());
        assert!(parse_topojson(br#"{"type":"Topology","arcs":[[[0,0],["a",1]]],"objects":{}}"#, &o).is_err());
    }
}
