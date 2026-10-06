//! GeoJSON (RFC 7946) in and out. Bytes in, values out — the host reads the file.
//!
//! Parsing is forgiving where real data is sloppy (unclosed rings are closed, altitude and extra
//! ordinates are ignored, rings with fewer than three positions are dropped) and strict where
//! guessing would be wrong (non-numeric coordinates are errors).

use crate::geometry::{json_to_id, Feature, FeatureCollection, Geometry, Line, Polygon};
use crate::GeoError;
use datars_math::Vec2;
use serde_json::{json, Map, Value};

/// How features get their ids.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GeoJsonOptions {
    /// Take the id from this property (e.g. `"iso_a3"`), falling back to the top-level `id`.
    /// `None`: the top-level `id` only.
    pub id_property: Option<String>,
}

impl GeoJsonOptions {
    pub fn id_from(property: impl Into<String>) -> GeoJsonOptions {
        GeoJsonOptions { id_property: Some(property.into()) }
    }
}

/// Parse a GeoJSON `FeatureCollection`, `Feature`, or bare geometry (wrapped as one feature).
pub fn parse_geojson(bytes: &[u8], opts: &GeoJsonOptions) -> Result<FeatureCollection, GeoError> {
    let root: Value = serde_json::from_slice(bytes).map_err(|e| GeoError::Json(e.to_string()))?;
    collection_from_json(&root, opts)
}

/// `parse_geojson` on text.
pub fn parse_geojson_str(text: &str, opts: &GeoJsonOptions) -> Result<FeatureCollection, GeoError> {
    parse_geojson(text.as_bytes(), opts)
}

/// A parsed JSON value → features.
pub fn collection_from_json(root: &Value, opts: &GeoJsonOptions) -> Result<FeatureCollection, GeoError> {
    match type_of(root)? {
        "FeatureCollection" => {
            let features = root
                .get("features")
                .and_then(Value::as_array)
                .ok_or_else(|| GeoError::format("FeatureCollection without a `features` array"))?;
            let features = features
                .iter()
                .enumerate()
                .map(|(i, f)| feature_from_json(f, opts).map_err(|e| context(e, &format!("features[{i}]"))))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(FeatureCollection { features })
        }
        "Feature" => Ok(FeatureCollection { features: vec![feature_from_json(root, opts)?] }),
        _ => Ok(FeatureCollection { features: vec![Feature::new(geometry_from_json(root)?)] }),
    }
}

/// One `Feature` object.
pub fn feature_from_json(f: &Value, opts: &GeoJsonOptions) -> Result<Feature, GeoError> {
    if type_of(f)? != "Feature" {
        return Err(GeoError::format("expected a Feature"));
    }
    let properties = match f.get("properties") {
        Some(Value::Object(m)) => m.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        Some(Value::Null) | None => Default::default(),
        Some(_) => return Err(GeoError::format("`properties` must be an object or null")),
    };
    let geometry = match f.get("geometry") {
        Some(Value::Null) | None => Geometry::default(),
        Some(g) => geometry_from_json(g).map_err(|e| context(e, "geometry"))?,
    };
    let mut feat = Feature { id: None, properties, geometry };
    feat.id = opts
        .id_property
        .as_deref()
        .and_then(|k| feat.properties.get(k))
        .and_then(json_to_id)
        .or_else(|| f.get("id").and_then(json_to_id));
    Ok(feat)
}

/// One geometry object.
pub fn geometry_from_json(g: &Value) -> Result<Geometry, GeoError> {
    let kind = type_of(g)?;
    if kind == "GeometryCollection" {
        let gs = g
            .get("geometries")
            .and_then(Value::as_array)
            .ok_or_else(|| GeoError::format("GeometryCollection without `geometries`"))?;
        return Ok(Geometry::Collection(gs.iter().map(geometry_from_json).collect::<Result<_, _>>()?));
    }
    let c = g.get("coordinates").ok_or_else(|| GeoError::format(format!("{kind} without `coordinates`")))?;
    let known = matches!(kind, "Point" | "MultiPoint" | "LineString" | "MultiLineString" | "Polygon" | "MultiPolygon");
    if known && c.as_array().is_some_and(Vec::is_empty) {
        return Ok(Geometry::default()); // RFC 7946 §3.1: empty coordinates may be read as null
    }
    Ok(match kind {
        "Point" => Geometry::Point(position(c)?),
        "MultiPoint" => Geometry::MultiPoint(positions(c)?),
        "LineString" => Geometry::LineString(positions(c)?),
        "MultiLineString" => Geometry::MultiLineString(array(c)?.iter().map(positions).collect::<Result<_, _>>()?),
        "Polygon" => Geometry::Polygon(polygon(c)?),
        "MultiPolygon" => Geometry::MultiPolygon(
            array(c)?.iter().map(polygon).filter(|p| !matches!(p, Ok(p) if p.is_empty())).collect::<Result<_, _>>()?,
        ),
        other => return Err(GeoError::format(format!("unknown geometry type `{other}`"))),
    })
}

fn type_of(v: &Value) -> Result<&str, GeoError> {
    v.get("type").and_then(Value::as_str).ok_or_else(|| GeoError::format("object without a `type`"))
}

fn context(e: GeoError, at: &str) -> GeoError {
    match e {
        GeoError::Format(m) => GeoError::Format(format!("{at}: {m}")),
        e => e,
    }
}

fn array(v: &Value) -> Result<&Vec<Value>, GeoError> {
    v.as_array().ok_or_else(|| GeoError::format("expected an array of coordinates"))
}

pub(crate) fn position(v: &Value) -> Result<Vec2, GeoError> {
    let a = array(v)?;
    let num = |i: usize| a.get(i).and_then(Value::as_f64).ok_or_else(|| GeoError::format("a position needs two numbers"));
    Ok(Vec2::new(num(0)?, num(1)?))
}

fn positions(v: &Value) -> Result<Line, GeoError> {
    array(v)?.iter().map(position).collect()
}

/// Rings are closed if they aren't; rings with fewer than three positions are dropped. A polygon
/// whose exterior was dropped is empty.
fn polygon(v: &Value) -> Result<Polygon, GeoError> {
    let mut rings = Vec::new();
    for (i, r) in array(v)?.iter().enumerate() {
        let mut ring = positions(r)?;
        if let (Some(&a), Some(&b)) = (ring.first(), ring.last()) {
            if a != b {
                ring.push(a);
            }
        }
        if ring.len() >= 4 {
            rings.push(ring);
        } else if i == 0 {
            return Ok(Vec::new());
        }
    }
    Ok(rings)
}

// ---- writing ---------------------------------------------------------------------------------

/// Coordinate rounding for output. `None` keeps full f64 precision (shortest round-trip form).
fn num(v: f64, decimals: Option<u32>) -> Value {
    let v = match decimals {
        Some(d) => {
            let k = datars_math::m::pow(10.0, d as f64);
            let r = (v * k).round() / k;
            if r == 0.0 { 0.0 } else { r }
        }
        None => v,
    };
    serde_json::Number::from_f64(v).map(Value::Number).unwrap_or(Value::Null)
}

fn pos_json(p: Vec2, d: Option<u32>) -> Value {
    Value::Array(vec![num(p.x, d), num(p.y, d)])
}

fn line_json(l: &[Vec2], d: Option<u32>) -> Value {
    Value::Array(l.iter().map(|p| pos_json(*p, d)).collect())
}

fn poly_json(p: &Polygon, d: Option<u32>) -> Value {
    Value::Array(p.iter().map(|r| line_json(r, d)).collect())
}

/// A geometry as a GeoJSON object, coordinates rounded to `decimals` if given.
pub fn geometry_to_json(g: &Geometry, decimals: Option<u32>) -> Value {
    let d = decimals;
    let coords = match g {
        Geometry::Point(p) => pos_json(*p, d),
        Geometry::MultiPoint(ps) | Geometry::LineString(ps) => line_json(ps, d),
        Geometry::MultiLineString(ls) => Value::Array(ls.iter().map(|l| line_json(l, d)).collect()),
        Geometry::Polygon(p) => poly_json(p, d),
        Geometry::MultiPolygon(pp) => Value::Array(pp.iter().map(|p| poly_json(p, d)).collect()),
        Geometry::Collection(gs) => {
            return json!({"type": "GeometryCollection", "geometries": gs.iter().map(|g| geometry_to_json(g, d)).collect::<Vec<_>>()})
        }
    };
    json!({"type": g.type_name(), "coordinates": coords})
}

/// A feature as a GeoJSON object. An empty geometry is written as `null`.
pub fn feature_to_json(f: &Feature, decimals: Option<u32>) -> Value {
    let mut o = Map::new();
    o.insert("type".into(), Value::String("Feature".into()));
    if let Some(id) = &f.id {
        o.insert("id".into(), Value::String(id.clone()));
    }
    o.insert("properties".into(), Value::Object(f.properties.iter().map(|(k, v)| (k.clone(), v.clone())).collect()));
    let geometry = if f.geometry.is_empty() { Value::Null } else { geometry_to_json(&f.geometry, decimals) };
    o.insert("geometry".into(), geometry);
    Value::Object(o)
}

/// A collection as a GeoJSON `FeatureCollection` object.
pub fn collection_to_json(fc: &FeatureCollection, decimals: Option<u32>) -> Value {
    json!({"type": "FeatureCollection", "features": fc.features.iter().map(|f| feature_to_json(f, decimals)).collect::<Vec<_>>()})
}

/// Compact GeoJSON text (deterministic: properties are sorted, numbers in shortest form).
pub fn to_geojson_string(fc: &FeatureCollection, decimals: Option<u32>) -> String {
    collection_to_json(fc, decimals).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FC: &str = r#"{"type":"FeatureCollection","features":[
      {"type":"Feature","id":7,"properties":{"iso":"AAA","name":"Alpha"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[10,0],[10,10],[0,10],[0,0]],[[2,2],[2,8],[8,8],[8,2]]]}},
      {"type":"Feature","properties":{"iso":"BBB"},"geometry":{"type":"MultiPolygon","coordinates":[[[[20,0],[21,0],[21,1],[20,0]]],[[[30,0],[31,0],[31,1],[30,0]]]]}},
      {"type":"Feature","properties":null,"geometry":{"type":"LineString","coordinates":[[0,1,99],[2,1],[4,1]]}},
      {"type":"Feature","properties":{},"geometry":{"type":"MultiPoint","coordinates":[[1,1],[2,2]]}},
      {"type":"Feature","properties":{},"geometry":{"type":"GeometryCollection","geometries":[{"type":"Point","coordinates":[5,5]},{"type":"MultiLineString","coordinates":[[[0,0],[1,1]],[[2,2],[3,3]]]}]}},
      {"type":"Feature","properties":{},"geometry":null}
    ]}"#;

    #[test]
    fn parses_every_type_with_holes_and_ids() {
        let fc = parse_geojson_str(FC, &GeoJsonOptions::default()).unwrap();
        assert_eq!(fc.len(), 6);
        assert_eq!(fc.features[0].id.as_deref(), Some("7"), "numeric top-level id");
        let Geometry::Polygon(p) = &fc.features[0].geometry else { panic!() };
        assert_eq!(p.len(), 2, "exterior + hole");
        assert_eq!(p[1].len(), 5, "the unclosed hole was closed");
        let Geometry::MultiPolygon(mp) = &fc.features[1].geometry else { panic!() };
        assert_eq!(mp.len(), 2);
        assert_eq!(fc.features[2].geometry, Geometry::LineString(vec![Vec2::new(0.0, 1.0), Vec2::new(2.0, 1.0), Vec2::new(4.0, 1.0)]));
        assert_eq!(fc.features[3].geometry.points().len(), 2);
        assert_eq!(fc.features[4].geometry.lines().len(), 2);
        assert!(fc.features[5].geometry.is_empty());
        let empty = parse_geojson_str(r#"{"type":"Point","coordinates":[]}"#, &GeoJsonOptions::default()).unwrap();
        assert!(empty.features[0].geometry.is_empty());

        let by_prop = parse_geojson_str(FC, &GeoJsonOptions::id_from("iso")).unwrap();
        assert_eq!(by_prop.features[0].id.as_deref(), Some("AAA"));
        assert_eq!(by_prop.features[1].id.as_deref(), Some("BBB"));
        assert_eq!(by_prop.features[2].id, None);
        assert_eq!(by_prop.get("BBB").unwrap().property_str("iso").as_deref(), Some("BBB"));
    }

    #[test]
    fn bare_geometry_and_single_feature() {
        let fc = parse_geojson_str(r#"{"type":"Point","coordinates":[1.5,2]}"#, &GeoJsonOptions::default()).unwrap();
        assert_eq!(fc.features[0].geometry, Geometry::Point(Vec2::new(1.5, 2.0)));
        let fc = parse_geojson_str(r#"{"type":"Feature","id":"x","geometry":{"type":"Point","coordinates":[0,0]}}"#, &GeoJsonOptions::default()).unwrap();
        assert_eq!(fc.features[0].id.as_deref(), Some("x"));
    }

    #[test]
    fn errors_are_reported_not_panicked() {
        let o = GeoJsonOptions::default();
        assert!(matches!(parse_geojson_str("{", &o), Err(GeoError::Json(_))));
        assert!(parse_geojson_str(r#"{"type":"Point","coordinates":["a",1]}"#, &o).is_err());
        assert!(parse_geojson_str(r#"{"type":"Blob","coordinates":[]}"#, &o).is_err());
        assert!(parse_geojson_str(r#"{"type":"FeatureCollection"}"#, &o).is_err());
        let e = parse_geojson_str(r#"{"type":"FeatureCollection","features":[{"type":"Feature","geometry":{"type":"Point"}}]}"#, &o).unwrap_err();
        assert!(e.to_string().contains("features[0]"), "{e}");
    }

    #[test]
    fn round_trips_through_text() {
        let fc = parse_geojson_str(FC, &GeoJsonOptions::default()).unwrap();
        let text = to_geojson_string(&fc, None);
        let back = parse_geojson_str(&text, &GeoJsonOptions::default()).unwrap();
        assert_eq!(back, fc);
        // Rounded output is short and still parses.
        let fc2 = FeatureCollection::new(vec![Feature::new(Geometry::Point(Vec2::new(18.068_581_2, 59.329_323_4)))]);
        let t = to_geojson_string(&fc2, Some(3));
        assert!(t.contains("[18.069,59.329]"), "{t}");
    }
}
