//! Mapbox Vector Tiles (spec 2.1): decode for the runtime, encode for the build pipeline.
//!
//! A tile holds named [`Layer`]s; each layer holds [`Feature`]s whose geometry is integer paths in
//! the tile's local space (`0..extent`, plus a buffer beyond the edges), with typed properties.
//! The codec never reprojects; `to_geometry`/`from_geometry` convert between tile-local integers
//! and lon/lat (clipping and quantizing on the way in). `decode(encode(t)) == t` is the
//! correctness anchor (see the tests).

mod convert;
mod proto;

pub use convert::{from_feature, from_geometry, local_to_lonlat, lonlat_to_local, to_feature, to_geometry, TilePaths};

use crate::GeoError;
use proto::{zigzag_decode, zigzag_encode, Reader, Writer, WIRE_FIXED32, WIRE_FIXED64, WIRE_LEN, WIRE_VARINT};
use std::collections::BTreeMap;

/// The conventional tile extent: geometry coordinates run `0..4096` across the tile.
pub const DEFAULT_EXTENT: u32 = 4096;

/// MVT geometry classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeomType {
    Unknown,
    Point,
    LineString,
    Polygon,
}

impl GeomType {
    fn from_u64(v: u64) -> GeomType {
        match v {
            1 => GeomType::Point,
            2 => GeomType::LineString,
            3 => GeomType::Polygon,
            _ => GeomType::Unknown,
        }
    }

    fn to_u64(self) -> u64 {
        match self {
            GeomType::Unknown => 0,
            GeomType::Point => 1,
            GeomType::LineString => 2,
            GeomType::Polygon => 3,
        }
    }
}

/// A typed property value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    String(String),
    Float(f32),
    Double(f64),
    Int(i64),
    Uint(u64),
    Bool(bool),
}

impl Value {
    /// The value as a string, if it is one.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    /// The value as an f64, if it's any numeric kind.
    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            Value::Float(v) => Some(v as f64),
            Value::Double(v) => Some(v),
            Value::Int(v) => Some(v as f64),
            Value::Uint(v) => Some(v as f64),
            _ => None,
        }
    }

    /// As a JSON value (feature properties elsewhere in the crate are JSON). Non-finite floats
    /// become `null`, since JSON has no representation for them.
    pub fn to_json(&self) -> serde_json::Value {
        use serde_json::Value as J;
        let num = |v: f64| serde_json::Number::from_f64(v).map_or(J::Null, J::Number);
        match self {
            Value::String(s) => J::String(s.clone()),
            Value::Float(v) => num(*v as f64),
            Value::Double(v) => num(*v),
            Value::Int(v) => J::Number((*v).into()),
            Value::Uint(v) => J::Number((*v).into()),
            Value::Bool(b) => J::Bool(*b),
        }
    }

    /// From a JSON value: strings, booleans and numbers (integers that fit `i64` → `Int`, larger
    /// unsigned → `Uint`, everything else → `Double`). `null`, arrays and objects have no MVT form.
    pub fn from_json(v: &serde_json::Value) -> Option<Value> {
        use serde_json::Value as J;
        match v {
            J::String(s) => Some(Value::String(s.clone())),
            J::Bool(b) => Some(Value::Bool(*b)),
            J::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Some(Value::Int(i))
                } else if let Some(u) = n.as_u64() {
                    Some(Value::Uint(u))
                } else {
                    n.as_f64().map(Value::Double)
                }
            }
            _ => None,
        }
    }
}

/// One feature: optional id, geometry class, integer paths in tile coordinates, properties.
///
/// `geometry` is a list of paths: for [`GeomType::Point`] each path is one point; for
/// [`GeomType::LineString`] each path is a line; for [`GeomType::Polygon`] each path is a ring
/// without a closing duplicate (exteriors wind clockwise on screen — positive area — and holes
/// counter-clockwise, per the spec; not enforced here).
#[derive(Clone, Debug, PartialEq)]
pub struct Feature {
    pub id: Option<u64>,
    pub geom_type: GeomType,
    pub geometry: Vec<Vec<(i32, i32)>>,
    pub properties: Vec<(String, Value)>,
}

impl Feature {
    /// A property by key.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.properties.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// Polygon rings grouped into polygons, per the spec: a ring with positive area (surveyor's
    /// formula in tile coordinates, y down) starts a polygon and the negative rings after it are
    /// its holes. Zero-area rings and holes before any exterior are dropped. Rings come back
    /// without a closing duplicate.
    pub fn polygons(&self) -> Vec<Vec<Vec<(i32, i32)>>> {
        let mut out: Vec<Vec<Vec<(i32, i32)>>> = Vec::new();
        for ring in &self.geometry {
            let mut ring = ring.clone();
            if ring.len() > 1 && ring.first() == ring.last() {
                ring.pop();
            }
            if ring.len() < 3 {
                continue;
            }
            let a = signed_area2(&ring);
            if a > 0 {
                out.push(vec![ring]);
            } else if a < 0 {
                if let Some(poly) = out.last_mut() {
                    poly.push(ring);
                }
            }
        }
        out
    }
}

/// Twice the signed area (surveyor's formula), exact in i128 whatever the i32 coordinates.
fn signed_area2(ring: &[(i32, i32)]) -> i128 {
    let n = ring.len();
    let mut s: i128 = 0;
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        s += a.0 as i128 * b.1 as i128 - b.0 as i128 * a.1 as i128;
    }
    s
}

/// A named set of features sharing one extent.
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub name: String,
    pub version: u32,
    pub extent: u32,
    pub features: Vec<Feature>,
}

impl Layer {
    /// An empty version-2 layer with the default 4096 extent.
    pub fn new(name: impl Into<String>) -> Layer {
        Layer { name: name.into(), version: 2, extent: DEFAULT_EXTENT, features: Vec::new() }
    }
}

/// A vector tile: an ordered list of layers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VectorTile {
    pub layers: Vec<Layer>,
}

impl VectorTile {
    /// The first layer with this name.
    pub fn layer(&self, name: &str) -> Option<&Layer> {
        self.layers.iter().find(|l| l.name == name)
    }

    /// Parse a tile from its (uncompressed) protobuf bytes.
    pub fn decode(bytes: &[u8]) -> Result<VectorTile, GeoError> {
        let mut tile = VectorTile::default();
        let mut r = Reader::new(bytes);
        while let Some((field, wire)) = r.tag()? {
            match (field, wire) {
                (3, WIRE_LEN) => tile.layers.push(decode_layer(r.bytes()?)?),
                _ => r.skip(wire)?,
            }
        }
        Ok(tile)
    }

    /// Serialize to protobuf bytes (uncompressed).
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new();
        for layer in &self.layers {
            w.bytes_field(3, &encode_layer(layer));
        }
        w.buf
    }
}

// ---- decoding ----------------------------------------------------------------------------------

fn decode_layer(data: &[u8]) -> Result<Layer, GeoError> {
    let mut r = Reader::new(data);
    let mut name = String::new();
    // Spec defaults: version 1, extent 4096.
    let mut version = 1u32;
    let mut extent = DEFAULT_EXTENT;
    let mut keys: Vec<String> = Vec::new();
    let mut values: Vec<Value> = Vec::new();
    let mut raw_features: Vec<&[u8]> = Vec::new();
    while let Some((field, wire)) = r.tag()? {
        match (field, wire) {
            (1, WIRE_LEN) => name = r.string()?,
            (2, WIRE_LEN) => raw_features.push(r.bytes()?),
            (3, WIRE_LEN) => keys.push(r.string()?),
            (4, WIRE_LEN) => values.push(decode_value(r.bytes()?)?),
            (5, WIRE_VARINT) => extent = r.varint()? as u32,
            (15, WIRE_VARINT) => version = r.varint()? as u32,
            _ => r.skip(wire)?,
        }
    }
    let features = raw_features.into_iter().map(|f| decode_feature(f, &keys, &values)).collect::<Result<_, _>>()?;
    Ok(Layer { name, version, extent, features })
}

fn decode_value(data: &[u8]) -> Result<Value, GeoError> {
    let mut r = Reader::new(data);
    let mut out = None;
    // A oneof in practice; if several are present the last wins (protobuf semantics).
    while let Some((field, wire)) = r.tag()? {
        out = Some(match (field, wire) {
            (1, WIRE_LEN) => Value::String(r.string()?),
            (2, WIRE_FIXED32) => Value::Float(f32::from_bits(r.fixed32()?)),
            (3, WIRE_FIXED64) => Value::Double(f64::from_bits(r.fixed64()?)),
            (4, WIRE_VARINT) => Value::Int(r.varint()? as i64),
            (5, WIRE_VARINT) => Value::Uint(r.varint()?),
            (6, WIRE_VARINT) => Value::Int(zigzag_decode(r.varint()?)),
            (7, WIRE_VARINT) => Value::Bool(r.varint()? != 0),
            _ => {
                r.skip(wire)?;
                continue;
            }
        });
    }
    out.ok_or_else(|| GeoError::format("MVT: empty property value"))
}

fn decode_feature(data: &[u8], keys: &[String], values: &[Value]) -> Result<Feature, GeoError> {
    let mut r = Reader::new(data);
    let mut id = None;
    let mut geom_type = GeomType::Unknown;
    let mut tags: Vec<u32> = Vec::new();
    let mut geom: Vec<u32> = Vec::new();
    while let Some((field, wire)) = r.tag()? {
        match (field, wire) {
            (1, WIRE_VARINT) => id = Some(r.varint()?),
            (2, WIRE_LEN) => r.packed_u32(&mut tags)?,
            (2, WIRE_VARINT) => tags.push(r.varint()? as u32),
            (3, WIRE_VARINT) => geom_type = GeomType::from_u64(r.varint()?),
            (4, WIRE_LEN) => r.packed_u32(&mut geom)?,
            (4, WIRE_VARINT) => geom.push(r.varint()? as u32),
            _ => r.skip(wire)?,
        }
    }
    if tags.len() & 1 == 1 {
        return Err(GeoError::format("MVT: odd number of feature tags"));
    }
    let properties = (0..tags.len() / 2)
        .map(|i| {
            let (ki, vi) = (tags[2 * i], tags[2 * i + 1]);
            let k = keys.get(ki as usize).ok_or_else(|| GeoError::format(format!("MVT: key index {ki} out of range")))?;
            let v = values.get(vi as usize).ok_or_else(|| GeoError::format(format!("MVT: value index {vi} out of range")))?;
            Ok((k.clone(), v.clone()))
        })
        .collect::<Result<_, GeoError>>()?;
    Ok(Feature { id, geom_type, geometry: decode_geometry(&geom)?, properties })
}

const CMD_MOVE_TO: u32 = 1;
const CMD_LINE_TO: u32 = 2;
const CMD_CLOSE_PATH: u32 = 7;

/// Command/parameter integers → absolute integer paths. Coordinates are cumulative zigzag deltas;
/// `MoveTo` starts a path per point, `LineTo` extends it, `ClosePath` closes the ring implicitly
/// (no point is appended — rings stay without a closing duplicate).
fn decode_geometry(g: &[u32]) -> Result<Vec<Vec<(i32, i32)>>, GeoError> {
    let mut paths: Vec<Vec<(i32, i32)>> = Vec::new();
    let mut cur: Vec<(i32, i32)> = Vec::new();
    let (mut x, mut y) = (0i32, 0i32);
    let mut i = 0;
    while i < g.len() {
        let (cmd, count) = (g[i] & 0x7, (g[i] >> 3) as usize);
        i += 1;
        match cmd {
            CMD_MOVE_TO | CMD_LINE_TO => {
                // Checked up front so a huge count can't loop past the data.
                if count > (g.len() - i) / 2 {
                    return Err(GeoError::format("MVT: geometry command runs past the end"));
                }
                for _ in 0..count {
                    if cmd == CMD_MOVE_TO && !cur.is_empty() {
                        paths.push(std::mem::take(&mut cur));
                    }
                    x = x.wrapping_add(zigzag_decode(g[i] as u64) as i32);
                    y = y.wrapping_add(zigzag_decode(g[i + 1] as u64) as i32);
                    i += 2;
                    cur.push((x, y));
                }
            }
            CMD_CLOSE_PATH => {}
            c => return Err(GeoError::format(format!("MVT: unknown geometry command {c}"))),
        }
    }
    if !cur.is_empty() {
        paths.push(cur);
    }
    Ok(paths)
}

// ---- encoding ----------------------------------------------------------------------------------

fn encode_layer(layer: &Layer) -> Vec<u8> {
    // Key and value dictionaries, deduplicated, indexed in first-use order. Values are keyed by
    // their encoded bytes, which also distinguishes 1 (Int) from 1.0 (Double) and handles NaN.
    let mut keys: Vec<&str> = Vec::new();
    let mut key_index: BTreeMap<&str, u32> = BTreeMap::new();
    let mut values: Vec<Vec<u8>> = Vec::new();
    let mut value_index: BTreeMap<Vec<u8>, u32> = BTreeMap::new();

    let mut w = Writer::new();
    w.string_field(1, &layer.name);
    for f in &layer.features {
        let mut fw = Writer::new();
        if let Some(id) = f.id {
            fw.varint_field(1, id);
        }
        let mut tags: Vec<u32> = Vec::with_capacity(f.properties.len() * 2);
        for (k, v) in &f.properties {
            let ki = *key_index.entry(k.as_str()).or_insert_with(|| {
                keys.push(k.as_str());
                (keys.len() - 1) as u32
            });
            let vb = encode_value(v);
            let vi = match value_index.get(&vb) {
                Some(&i) => i,
                None => {
                    let i = values.len() as u32;
                    value_index.insert(vb.clone(), i);
                    values.push(vb);
                    i
                }
            };
            tags.push(ki);
            tags.push(vi);
        }
        if !tags.is_empty() {
            fw.packed_u32(2, &tags);
        }
        fw.varint_field(3, f.geom_type.to_u64());
        let geom = encode_geometry(&f.geometry, f.geom_type);
        if !geom.is_empty() {
            fw.packed_u32(4, &geom);
        }
        w.bytes_field(2, &fw.buf);
    }
    for k in &keys {
        w.string_field(3, k);
    }
    for v in &values {
        w.bytes_field(4, v);
    }
    w.varint_field(5, layer.extent as u64);
    w.varint_field(15, layer.version.max(1) as u64);
    w.buf
}

fn encode_value(v: &Value) -> Vec<u8> {
    let mut w = Writer::new();
    match v {
        Value::String(s) => w.string_field(1, s),
        Value::Float(f) => w.float_field(2, *f),
        Value::Double(d) => w.double_field(3, *d),
        // Negative ints as sint (zigzag): a negative int64 varint costs 10 bytes.
        Value::Int(i) if *i < 0 => w.varint_field(6, zigzag_encode(*i)),
        Value::Int(i) => w.varint_field(4, *i as u64),
        Value::Uint(u) => w.varint_field(5, *u),
        Value::Bool(b) => w.varint_field(7, *b as u64),
    }
    w.buf
}

fn command(id: u32, count: usize) -> u32 {
    (id & 0x7) | ((count as u32) << 3)
}

/// Integer paths → command/parameter integers.
fn encode_geometry(paths: &[Vec<(i32, i32)>], geom_type: GeomType) -> Vec<u32> {
    let mut out = Vec::new();
    let (mut cx, mut cy) = (0i32, 0i32);
    let mut delta = |out: &mut Vec<u32>, (x, y): (i32, i32)| {
        out.push(zigzag_encode(x.wrapping_sub(cx) as i64) as u32);
        out.push(zigzag_encode(y.wrapping_sub(cy) as i64) as u32);
        (cx, cy) = (x, y);
    };
    if geom_type == GeomType::Point {
        // Every point in one MoveTo whose count is the number of points.
        let pts: Vec<(i32, i32)> = paths.iter().flatten().copied().collect();
        if !pts.is_empty() {
            out.push(command(CMD_MOVE_TO, pts.len()));
            for p in pts {
                delta(&mut out, p);
            }
        }
        return out;
    }
    let polygon = geom_type == GeomType::Polygon;
    for path in paths {
        // Rings close implicitly (ClosePath), so an explicit closing duplicate is dropped.
        let mut p: &[(i32, i32)] = path;
        if polygon && p.len() > 1 && p.first() == p.last() {
            p = &p[..p.len() - 1];
        }
        if p.len() < if polygon { 3 } else { 2 } {
            continue;
        }
        out.push(command(CMD_MOVE_TO, 1));
        delta(&mut out, p[0]);
        out.push(command(CMD_LINE_TO, p.len() - 1));
        for &q in &p[1..] {
            delta(&mut out, q);
        }
        if polygon {
            out.push(command(CMD_CLOSE_PATH, 1));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feature(id: Option<u64>, geom_type: GeomType, geometry: Vec<Vec<(i32, i32)>>, props: Vec<(&str, Value)>) -> Feature {
        Feature { id, geom_type, geometry, properties: props.into_iter().map(|(k, v)| (k.to_string(), v)).collect() }
    }

    #[test]
    fn geometry_round_trips() {
        let poly = vec![vec![(0, 0), (10, 0), (10, 10), (0, 10)], vec![(2, 2), (2, 8), (8, 8), (8, 2)]];
        assert_eq!(decode_geometry(&encode_geometry(&poly, GeomType::Polygon)), Ok(poly.clone()));
        let lines = vec![vec![(5, 5), (50, 20), (80, 90)], vec![(-3, 4100), (4, 4)]];
        assert_eq!(decode_geometry(&encode_geometry(&lines, GeomType::LineString)), Ok(lines));
        let pts = vec![vec![(1, 2)], vec![(3, 4)], vec![(-5, 6)]];
        assert_eq!(decode_geometry(&encode_geometry(&pts, GeomType::Point)), Ok(pts));
        // A closing duplicate is dropped on encode.
        let mut closed = poly[0].clone();
        closed.push(closed[0]);
        assert_eq!(decode_geometry(&encode_geometry(&[closed], GeomType::Polygon)), Ok(vec![poly[0].clone()]));
    }

    #[test]
    fn decodes_spec_example_geometry() {
        // From the spec: a polygon "MoveTo(3,6) LineTo(8,12) LineTo(20,34) ClosePath".
        let g = [9, 6, 12, 18, 10, 12, 24, 44, 15];
        assert_eq!(decode_geometry(&g), Ok(vec![vec![(3, 6), (8, 12), (20, 34)]]));
    }

    #[test]
    fn polygons_group_holes_by_winding() {
        let ext = vec![(0, 0), (10, 0), (10, 10), (0, 10)]; // clockwise on screen: positive
        let hole = vec![(2, 2), (2, 8), (8, 8), (8, 2)];
        let ext2 = vec![(20, 0), (30, 0), (30, 10), (20, 10), (20, 0)]; // with closing duplicate
        let f = feature(None, GeomType::Polygon, vec![ext.clone(), hole.clone(), ext2.clone()], vec![]);
        let polys = f.polygons();
        assert_eq!(polys.len(), 2);
        assert_eq!(polys[0], vec![ext, hole]);
        assert_eq!(polys[1], vec![ext2[..4].to_vec()]);
    }

    fn sample() -> VectorTile {
        let mut water = Layer::new("water");
        water.features.push(feature(
            Some(1),
            GeomType::Polygon,
            vec![vec![(0, 0), (4096, 0), (4096, 4096), (0, 4096)], vec![(100, 100), (100, 200), (200, 200), (200, 100)]],
            vec![("class", Value::String("ocean".into())), ("area", Value::Double(12345.5))],
        ));
        let mut roads = Layer::new("roads");
        roads.extent = 8192;
        roads.features.push(feature(
            Some(0),
            GeomType::LineString,
            vec![vec![(10, 10), (100, 50), (200, 300)], vec![(-64, -64), (4160, 4160)]],
            vec![("name", Value::String("E4".into())), ("lanes", Value::Int(4)), ("offset", Value::Int(-7))],
        ));
        roads.features.push(feature(
            None,
            GeomType::LineString,
            vec![vec![(1, 1), (2, 2)]],
            vec![("lanes", Value::Int(4)), ("big", Value::Uint(u64::MAX)), ("min", Value::Int(i64::MIN))],
        ));
        let mut places = Layer::new("places");
        places.version = 1;
        places.features.push(feature(
            Some(3),
            GeomType::Point,
            vec![vec![(2048, 2048)], vec![(100, 200)]],
            vec![("name", Value::String("Stockholm".into())), ("pop", Value::Float(0.5)), ("capital", Value::Bool(true))],
        ));
        VectorTile { layers: vec![water, roads, places] }
    }

    #[test]
    fn tile_round_trips() {
        let t = sample();
        let bytes = t.encode();
        let back = VectorTile::decode(&bytes).unwrap();
        assert_eq!(back, t);
        // Deterministic bytes.
        assert_eq!(back.encode(), bytes);
        let roads = back.layer("roads").unwrap();
        assert_eq!(roads.extent, 8192);
        assert_eq!(roads.features[0].get("name").and_then(Value::as_str), Some("E4"));
        assert_eq!(roads.features[0].get("offset").and_then(Value::as_f64), Some(-7.0));
        assert!(back.layer("nope").is_none());
    }

    #[test]
    fn values_dedup_in_dictionary() {
        let mut l = Layer::new("l");
        for i in 0..10 {
            l.features.push(feature(Some(i), GeomType::Point, vec![vec![(0, 0)]], vec![("k", Value::String("same".into()))]));
        }
        // Numbers that encode alike but differ in kind stay distinct.
        l.features.push(feature(None, GeomType::Point, vec![vec![(0, 0)]], vec![("n", Value::Int(1)), ("m", Value::Uint(1)), ("d", Value::Double(1.0))]));
        let bytes = VectorTile { layers: vec![l.clone()] }.encode();
        let count = |needle: &[u8]| bytes.windows(needle.len()).filter(|w| *w == needle).count();
        assert_eq!(count(b"same"), 1, "the value is stored once");
        assert_eq!(count(b"\x1a\x01k"), 1, "the key is stored once");
        assert_eq!(VectorTile::decode(&bytes).unwrap().layers[0], l);
    }

    #[test]
    fn json_conversion() {
        use serde_json::json;
        assert_eq!(Value::from_json(&json!("a")), Some(Value::String("a".into())));
        assert_eq!(Value::from_json(&json!(true)), Some(Value::Bool(true)));
        assert_eq!(Value::from_json(&json!(-3)), Some(Value::Int(-3)));
        assert_eq!(Value::from_json(&json!(u64::MAX)), Some(Value::Uint(u64::MAX)));
        assert_eq!(Value::from_json(&json!(1.5)), Some(Value::Double(1.5)));
        assert_eq!(Value::from_json(&json!(null)), None);
        assert_eq!(Value::from_json(&json!([1])), None);
        assert_eq!(Value::Int(-3).to_json(), json!(-3));
        assert_eq!(Value::Float(0.5).to_json(), json!(0.5));
        assert_eq!(Value::Double(f64::NAN).to_json(), json!(null));
        assert_eq!(Value::Uint(7).to_json(), json!(7));
    }

    #[test]
    fn malformed_input_is_an_error_not_a_panic() {
        assert_eq!(VectorTile::decode(&[]), Ok(VectorTile::default()));
        assert!(VectorTile::decode(&[0x1a, 0x05, 0x0a]).is_err(), "layer length past the end");
        assert!(VectorTile::decode(&[0x1a]).is_err(), "truncated varint");
        assert!(VectorTile::decode(&[0x1b]).is_err(), "group wire type");
        // Geometry running past the end, and an unknown command.
        assert!(decode_geometry(&[command(CMD_LINE_TO, 3), 2, 2]).is_err());
        assert!(decode_geometry(&[command(CMD_MOVE_TO, 1 << 28), 2, 2]).is_err());
        assert!(decode_geometry(&[command(4, 1), 0, 0]).is_err());
        // A tag pointing at a missing key.
        let mut l = Layer::new("x");
        l.features.push(feature(None, GeomType::Point, vec![vec![(1, 1)]], vec![("k", Value::Bool(false))]));
        let mut bytes = VectorTile { layers: vec![l] }.encode();
        let pos = bytes.windows(3).position(|w| w == [0x12, 0x02, 0x00]).expect("tags field");
        bytes[pos + 2] = 5; // key index 5 doesn't exist
        assert!(VectorTile::decode(&bytes).is_err());
    }

    #[test]
    fn fuzzed_bytes_never_panic() {
        let good = sample().encode();
        let mut rng = datars_math::Rng::new(7);
        for _ in 0..2000 {
            let mut b = good.clone();
            match rng.below(3) {
                0 => b.truncate(rng.below(b.len() as u64) as usize),
                1 => {
                    for _ in 0..1 + rng.below(4) {
                        let i = rng.below(b.len() as u64) as usize;
                        b[i] = rng.next_u64() as u8;
                    }
                }
                _ => b = (0..rng.below(64)).map(|_| rng.next_u64() as u8).collect(),
            }
            let _ = VectorTile::decode(&b);
        }
    }
}
