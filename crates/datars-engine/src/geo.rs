//! Geo in the engine (docs/09-geo.md): geo sources (GeoJSON, TopoJSON, atlases) as tables plus a
//! feature store; a `geo` coordinate system on a group fits a projection to its layout box; feature
//! geometry and lon/lat points project through the nearest one.

use crate::resolve::{Cx, Resolver};
use datars_data::{Column, Table};
use datars_geo::{Feature, FeatureCollection, Fit, GeoBbox, GeoJsonOptions, Projection};
use datars_math::{Rect, Vec2};
use datars_scene::Geom;
use std::collections::BTreeMap;
use std::sync::Arc;

pub struct GeoSource {
    pub fc: FeatureCollection,
    pub index: BTreeMap<String, usize>,
    /// Each feature's visual centre, once asked for (see [`GeoSource::centre`]).
    centres: Vec<std::sync::OnceLock<Option<Vec2>>>,
}

impl GeoSource {
    pub fn new(fc: FeatureCollection, index: BTreeMap<String, usize>) -> GeoSource {
        let centres = (0..fc.features.len()).map(|_| std::sync::OnceLock::new()).collect();
        GeoSource { fc, index, centres }
    }

    pub fn feature(&self, id: &str) -> Option<&Feature> {
        self.index.get(id).map(|&i| &self.fc.features[i])
    }

    /// A feature's visual centre ([`visual_centre`]), found once: it's in lon/lat, the same in
    /// every state and projection, and symbols on a map of counties ask for three thousand of them
    /// per state, for x and again for y (the search was most of such a state's resolve).
    pub fn centre(&self, id: &str) -> Option<Vec2> {
        let &i = self.index.get(id)?;
        *self.centres.get(i)?.get_or_init(|| visual_centre(&self.fc.features[i]))
    }
}

pub type GeoStore = BTreeMap<String, Arc<GeoSource>>;

/// Parse GeoJSON or TopoJSON bytes into a source + its table (id, name, scalar properties).
pub fn load(name: &str, bytes: &[u8], id_prop: Option<&str>) -> Result<(GeoSource, Table), String> {
    let opts = GeoJsonOptions { id_property: id_prop.map(String::from) };
    let v: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let fc = if v.get("type").and_then(|t| t.as_str()) == Some("Topology") {
        let topo = datars_geo::parse_topojson(bytes, &opts).map_err(|e| e.to_string())?;
        let obj = topo.object_names().next().map(String::from).ok_or("TopoJSON has no objects")?;
        topo.features(&obj).ok_or("TopoJSON object is empty")?
    } else {
        datars_geo::parse_geojson(bytes, &opts).map_err(|e| e.to_string())?
    };
    let mut index = BTreeMap::new();
    let mut ids = Vec::new();
    let mut names = Vec::new();
    let mut props: BTreeMap<String, Vec<serde_json::Value>> = BTreeMap::new();
    let mut keep: Vec<usize> = Vec::new();
    for (i, f) in fc.features.iter().enumerate() {
        let id = f.id.clone().or_else(|| f.property_str("id")).unwrap_or_else(|| i.to_string());
        if index.contains_key(&id) {
            continue;
        }
        index.insert(id.clone(), i);
        keep.push(i);
        names.push(f.property_str("name").or_else(|| f.property_str("NAME")).unwrap_or_else(|| id.clone()));
        ids.push(id);
        for (k, pv) in &f.properties {
            if k == "id" || k == "name" || !(pv.is_number() || pv.is_string()) {
                continue;
            }
            if props.len() < 24 || props.contains_key(k) {
                props.entry(k.clone()).or_insert_with(|| vec![serde_json::Value::Null; ids.len() - 1]);
            }
        }
        for (k, col) in props.iter_mut() {
            col.resize(ids.len() - 1, serde_json::Value::Null);
            col.push(f.properties.get(k).cloned().unwrap_or(serde_json::Value::Null));
        }
    }
    let mut columns: Vec<(String, Column)> = vec![
        ("id".into(), Column::Str(ids.iter().map(|s| Some(Arc::from(s.as_str()))).collect())),
        ("name".into(), Column::Str(names.iter().map(|s| Some(Arc::from(s.as_str()))).collect())),
    ];
    for (k, col) in props {
        let c = if col.iter().all(|v| v.is_number() || v.is_null()) {
            Column::Num(col.iter().map(|v| v.as_f64().unwrap_or(f64::NAN)).collect())
        } else {
            Column::Str(col.iter().map(|v| v.as_str().map(Arc::from).or_else(|| (!v.is_null()).then(|| Arc::from(v.to_string().as_str())))).collect())
        };
        columns.push((k, c));
    }
    let mut t = Table::from_columns(name, columns).map_err(|e| e.to_string())?.with_key(&["id"]).map_err(|e| e.to_string())?;
    t.name = name.to_string();
    Ok((GeoSource::new(fc, index), t))
}

fn base_projection(c: &serde_json::Value, center: (f64, f64)) -> Projection {
    let name = c.get("projection").and_then(|p| p.as_str()).unwrap_or("equal-earth");
    match name {
        "mercator" => Projection::mercator(),
        "web-mercator" => Projection::web_mercator(),
        "natural-earth" => Projection::natural_earth1(),
        "equirectangular" | "plate-carree" => Projection::equirectangular(),
        "albers" => Projection::albers(),
        "albers-usa" => Projection::albers_usa(),
        "lambert" | "lambert-conformal-conic" => Projection::lambert_conformal_conic([33.0, 45.0]),
        "orthographic" | "globe" => Projection::orthographic(center.0, center.1),
        "sweref99tm" => Projection::sweref99tm(),
        "planar" => Projection::planar_y_up(),
        "planar-y-down" => Projection::planar(),
        n if n.starts_with("utm-") => {
            let zone: u8 = n[4..n.len() - 1].parse().unwrap_or(33);
            Projection::utm(zone, n.ends_with('n'))
        }
        _ => Projection::equal_earth(),
    }
}

/// The projection a `coord` declaration gives for a box.
pub(crate) fn projection_for(r: &Resolver, coord: &serde_json::Value, cx: &Cx) -> Option<Projection> {
    if coord.get("type").and_then(|t| t.as_str()) != Some("geo") {
        return None;
    }
    // The centre (a globe's rotation) may be expressions: `[e("spin"), 15]` turns it with a signal.
    let center = coord.get("center").and_then(|v| v.as_array()).map(|a| {
        let at = |i: usize| a.get(i).and_then(|x| r.eval(&datars_ir::Prop(x.clone()), cx).as_num()).unwrap_or(0.0);
        (at(0), at(1))
    });
    let base = base_projection(coord, center.unwrap_or((0.0, 0.0)));
    let pad = coord.get("padding").and_then(|p| p.as_f64()).unwrap_or(8.0);
    let extent = Rect::new(pad, pad, (cx.box_w - 2.0 * pad).max(1.0), (cx.box_h - 2.0 * pad).max(1.0));
    let fit = coord.get("fit").cloned().unwrap_or(serde_json::Value::Null);
    // `{sphere: true}` (or `"sphere"`): the whole globe, whatever its rotation.
    if fit.as_str() == Some("sphere") || fit.get("sphere").and_then(|s| s.as_bool()) == Some(true) {
        return Some(base.fit_extent(Fit::Sphere, extent));
    }
    if let Some(b) = fit.get("bbox").and_then(|b| b.as_array()) {
        let v: Vec<f64> = b.iter().filter_map(|x| x.as_f64()).collect();
        if v.len() == 4 {
            return Some(base.fit_extent(Fit::Bbox(GeoBbox::new(v[0], v[1], v[2], v[3])), extent));
        }
    }
    let src = fit.get("source").and_then(|s| s.as_str()).map(String::from).or_else(|| r.geo.keys().next().cloned())?;
    let g = r.geo.get(&src)?;
    let keys: Vec<String> = match fit.get("keys") {
        Some(serde_json::Value::Array(a)) => a.iter().filter_map(|k| k.as_str().map(String::from)).collect(),
        Some(k @ serde_json::Value::String(_)) | Some(k @ serde_json::Value::Object(_)) => crate::env::decode_keyset(&r.eval(&datars_ir::Prop(k.clone()), cx)),
        _ => Vec::new(),
    };
    if keys.is_empty() {
        return Some(base.fit_extent(Fit::Features(&g.fc), extent));
    }
    let sub = FeatureCollection { features: keys.iter().filter_map(|k| g.feature(k).cloned()).collect() };
    if sub.features.is_empty() {
        return Some(base.fit_extent(Fit::Features(&g.fc), extent));
    }
    Some(base.fit_extent(Fit::Features(&sub), extent))
}

pub(crate) fn feature_geom(r: &Resolver, source: &str, id: &str, cx: &Cx) -> Option<Geom> {
    let Some(proj) = &cx.proj else {
        r.diag(format!("feature `{source}#{id}` outside a geo coordinate system"));
        return None;
    };
    let g = r.geo.get(source).or_else(|| {
        r.diag(format!("unknown geo source `{source}`"));
        None
    })?;
    let f = g.feature(id)?;
    let path = datars_geo::to_path(&f.geometry, proj, 0.35);
    if path.is_empty() {
        return None;
    }
    Some(Geom::path(path))
}

/// A lon/lat box `[west, south, east, north]` in content units of the enclosing geo coordinate
/// system: its projected outline's bounds (edges densified, so curved projections frame what the
/// box covers). For cameras: `fit: { geo: [...] }` — four numbers or expressions, or one
/// expression naming a list signal (`"=bounds"`, set per story step: `set: { bounds: [...] }`).
pub(crate) fn bbox_content(r: &Resolver, g: &serde_json::Value, cx: &Cx) -> Option<Rect> {
    let Some(proj) = &cx.proj else {
        r.diag("camera `fit.geo` needs a geo coordinate system on the view");
        return None;
    };
    let v: Vec<f64> = match g.as_array() {
        Some(a) => a.iter().map(|x| r.num(&datars_ir::Prop(x.clone()), cx, f64::NAN)).collect(),
        None => crate::env::decode_keyset(&r.eval(&datars_ir::Prop(g.clone()), cx)).iter().map(|s| s.trim().parse().unwrap_or(f64::NAN)).collect(),
    };
    if v.len() != 4 || v.iter().any(|x| !x.is_finite()) {
        r.diag(format!("camera `fit.geo` wants [west, south, east, north], got {g}"));
        return None;
    }
    let bb = GeoBbox::new(v[0], v[1], v[2], v[3]);
    let datars_geo::Geometry::Polygon(ring) = bb.to_polygon((bb.width().max(v[3] - v[1]) / 16.0).clamp(1e-4, 1.0)) else { return None };
    let p = proj.projector();
    let b = ring.iter().flatten().filter_map(|q| p.forward(*q)).fold(Rect::empty(), |b, q| b.include(q));
    (!b.is_empty()).then_some(b)
}

/// The visual centre of a feature (pole of inaccessibility of its largest polygon), in lon/lat.
/// Points and lines have no interior: their centroid (a point feature's own position).
pub fn visual_centre(f: &Feature) -> Option<Vec2> {
    let polys = f.geometry.polygons();
    let Some(largest) = polys.iter().max_by(|a, b| datars_math::total_cmp(ring_area(&a[0]), ring_area(&b[0]))) else {
        return datars_geo::measure::centroid(&f.geometry);
    };
    let (c, _) = datars_algo::polylabel(largest, 0.01);
    c.is_finite().then_some(c).or_else(|| datars_geo::measure::centroid(&f.geometry))
}

pub(crate) fn ring_area(r: &[Vec2]) -> f64 {
    datars_math::path::signed_area(r).abs()
}

/// Expression host functions for geo (`geo.x`, `geo.y`, `geo.cx`, `geo.cy`, `geo.geodesic`).
pub fn call(name: &str, args: &[datars_expr::Value], proj: Option<&Projection>, geo: &GeoStore) -> Option<datars_expr::Value> {
    use datars_expr::Value as V;
    let num = |i: usize| match args.get(i) {
        Some(V::Num(n)) => *n,
        Some(V::Str(s)) => s.parse().unwrap_or(f64::NAN),
        _ => f64::NAN,
    };
    let s = |i: usize| match args.get(i) {
        Some(V::Str(s)) => s.to_string(),
        Some(V::Num(n)) => format!("{n}"),
        _ => String::new(),
    };
    let proj = proj?;
    match name {
        "geo.x" | "geo.y" => {
            let p = proj.forward(Vec2::new(num(0), num(1)))?;
            Some(V::Num(if name == "geo.x" { p.x } else { p.y }))
        }
        "geo.cx" | "geo.cy" => {
            let p = proj.forward(geo.get(&s(0))?.centre(&s(1))?)?;
            Some(V::Num(if name == "geo.cx" { p.x } else { p.y }))
        }
        "geo.geodesic" => {
            let pts = datars_geo::geodesic(Vec2::new(num(0), num(1)), Vec2::new(num(2), num(3)), 48);
            let line = datars_geo::Geometry::LineString(pts);
            let path = datars_geo::to_path(&line, proj, 0.35);
            Some(V::Str(Arc::from(crate::resolve::svg_path_string(&path).as_str())))
        }
        _ => None,
    }
}
