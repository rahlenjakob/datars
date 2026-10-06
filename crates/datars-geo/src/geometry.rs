//! The geometry model: GeoJSON's six geometry types (plus collections), features with typed JSON
//! properties, and lon/lat bounding boxes.
//!
//! Coordinates are `Vec2`: `x` = longitude, `y` = latitude in degrees for geographic data, or plain
//! planar units (floor plans, fictional maps) when used with `Projection::planar`. Rings are
//! **closed** (first == last, so a valid ring has ≥ 4 points); polygons are `[exterior, holes…]`.
//! Winding order on input is free: everything that cares (spherical clipping, area, rectangle
//! clipping) normalizes it first.

use datars_math::{Rect, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// An open polyline.
pub type Line = Vec<Vec2>;
/// A closed ring (first == last).
pub type Ring = Vec<Vec2>;
/// `[exterior, hole, hole, …]`.
pub type Polygon = Vec<Ring>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "coordinates")]
pub enum Geometry {
    Point(Vec2),
    MultiPoint(Vec<Vec2>),
    LineString(Line),
    MultiLineString(Vec<Line>),
    Polygon(Polygon),
    MultiPolygon(Vec<Polygon>),
    /// GeoJSON `GeometryCollection`. An empty collection also stands for a `null` geometry.
    Collection(Vec<Geometry>),
}

impl Default for Geometry {
    fn default() -> Geometry {
        Geometry::Collection(Vec::new())
    }
}

impl Geometry {
    /// The GeoJSON type name.
    pub fn type_name(&self) -> &'static str {
        match self {
            Geometry::Point(_) => "Point",
            Geometry::MultiPoint(_) => "MultiPoint",
            Geometry::LineString(_) => "LineString",
            Geometry::MultiLineString(_) => "MultiLineString",
            Geometry::Polygon(_) => "Polygon",
            Geometry::MultiPolygon(_) => "MultiPolygon",
            Geometry::Collection(_) => "GeometryCollection",
        }
    }

    /// True when there are no coordinates at all.
    pub fn is_empty(&self) -> bool {
        let mut any = false;
        self.for_each_coord(|_| any = true);
        !any
    }

    /// Visit every coordinate (points, line vertices, ring vertices incl. closing duplicates).
    pub fn for_each_coord(&self, mut f: impl FnMut(Vec2)) {
        fn walk(g: &Geometry, f: &mut dyn FnMut(Vec2)) {
            match g {
                Geometry::Point(p) => f(*p),
                Geometry::MultiPoint(ps) | Geometry::LineString(ps) => ps.iter().for_each(|p| f(*p)),
                Geometry::MultiLineString(ls) | Geometry::Polygon(ls) => ls.iter().flatten().for_each(|p| f(*p)),
                Geometry::MultiPolygon(pp) => pp.iter().flatten().flatten().for_each(|p| f(*p)),
                Geometry::Collection(gs) => gs.iter().for_each(|g| walk(g, f)),
            }
        }
        walk(self, &mut f)
    }

    /// A copy with every coordinate mapped through `f` (structure unchanged).
    pub fn map_coords(&self, f: &impl Fn(Vec2) -> Vec2) -> Geometry {
        let line = |l: &Line| l.iter().map(|p| f(*p)).collect::<Line>();
        let poly = |p: &Polygon| p.iter().map(line).collect::<Polygon>();
        match self {
            Geometry::Point(p) => Geometry::Point(f(*p)),
            Geometry::MultiPoint(ps) => Geometry::MultiPoint(line(ps)),
            Geometry::LineString(l) => Geometry::LineString(line(l)),
            Geometry::MultiLineString(ls) => Geometry::MultiLineString(ls.iter().map(line).collect()),
            Geometry::Polygon(p) => Geometry::Polygon(poly(p)),
            Geometry::MultiPolygon(pp) => Geometry::MultiPolygon(pp.iter().map(poly).collect()),
            Geometry::Collection(gs) => Geometry::Collection(gs.iter().map(|g| g.map_coords(f)).collect()),
        }
    }

    /// Every point (from `Point`/`MultiPoint`, including inside collections).
    pub fn points(&self) -> Vec<Vec2> {
        let mut out = Vec::new();
        self.visit(&mut |g| match g {
            Geometry::Point(p) => out.push(*p),
            Geometry::MultiPoint(ps) => out.extend_from_slice(ps),
            _ => {}
        });
        out
    }

    /// Every polyline (from `LineString`/`MultiLineString`, including inside collections).
    pub fn lines(&self) -> Vec<&Line> {
        let mut out = Vec::new();
        self.visit_ref(&mut |g| match g {
            Geometry::LineString(l) => out.push(l),
            Geometry::MultiLineString(ls) => out.extend(ls.iter()),
            _ => {}
        });
        out
    }

    /// Every polygon (from `Polygon`/`MultiPolygon`, including inside collections).
    pub fn polygons(&self) -> Vec<&Polygon> {
        let mut out = Vec::new();
        self.visit_ref(&mut |g| match g {
            Geometry::Polygon(p) => out.push(p),
            Geometry::MultiPolygon(pp) => out.extend(pp.iter()),
            _ => {}
        });
        out
    }

    /// Planar bounds of the raw coordinates (no antimeridian handling; see `measure::bbox` for
    /// geographic bounds).
    pub fn planar_bounds(&self) -> Option<Rect> {
        let mut r = Rect::empty();
        self.for_each_coord(|p| {
            if p.is_finite() {
                r = r.include(p)
            }
        });
        (!r.is_empty()).then_some(r)
    }

    fn visit(&self, f: &mut dyn FnMut(&Geometry)) {
        match self {
            Geometry::Collection(gs) => gs.iter().for_each(|g| g.visit(f)),
            g => f(g),
        }
    }

    fn visit_ref<'a>(&'a self, f: &mut dyn FnMut(&'a Geometry)) {
        match self {
            Geometry::Collection(gs) => gs.iter().for_each(|g| g.visit_ref(f)),
            g => f(g),
        }
    }
}

/// A geographic feature: an optional id (the join key), JSON properties, and geometry.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Feature {
    pub id: Option<String>,
    pub properties: BTreeMap<String, serde_json::Value>,
    pub geometry: Geometry,
}

impl Feature {
    pub fn new(geometry: Geometry) -> Feature {
        Feature { id: None, properties: BTreeMap::new(), geometry }
    }
    pub fn with_id(mut self, id: impl Into<String>) -> Feature {
        self.id = Some(id.into());
        self
    }
    pub fn with_property(mut self, key: impl Into<String>, value: serde_json::Value) -> Feature {
        self.properties.insert(key.into(), value);
        self
    }
    pub fn property(&self, key: &str) -> Option<&serde_json::Value> {
        self.properties.get(key)
    }
    /// A property as a string (strings as-is, numbers and booleans formatted).
    pub fn property_str(&self, key: &str) -> Option<String> {
        self.properties.get(key).and_then(json_to_id)
    }
}

/// An ordered list of features.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FeatureCollection {
    pub features: Vec<Feature>,
}

impl FeatureCollection {
    pub fn new(features: Vec<Feature>) -> FeatureCollection {
        FeatureCollection { features }
    }
    pub fn len(&self) -> usize {
        self.features.len()
    }
    pub fn is_empty(&self) -> bool {
        self.features.is_empty()
    }
    /// The first feature with this id.
    pub fn get(&self, id: &str) -> Option<&Feature> {
        self.features.iter().find(|f| f.id.as_deref() == Some(id))
    }
    pub fn iter(&self) -> std::slice::Iter<'_, Feature> {
        self.features.iter()
    }
    /// All geometries as one collection (for bounds and fitting).
    pub fn to_geometry(&self) -> Geometry {
        Geometry::Collection(self.features.iter().map(|f| f.geometry.clone()).collect())
    }
}

/// A JSON scalar as an id string: strings as-is, numbers in their shortest JSON form (`840`,
/// `1.5`), booleans as `true`/`false`. Objects, arrays and null have no id.
pub(crate) fn json_to_id(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// A lon/lat bounding box in degrees. `west > east` means it crosses the antimeridian.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeoBbox {
    pub west: f64,
    pub south: f64,
    pub east: f64,
    pub north: f64,
}

impl GeoBbox {
    pub const fn new(west: f64, south: f64, east: f64, north: f64) -> GeoBbox {
        GeoBbox { west, south, east, north }
    }
    /// The whole globe.
    pub const fn world() -> GeoBbox {
        GeoBbox::new(-180.0, -90.0, 180.0, 90.0)
    }
    pub fn crosses_antimeridian(&self) -> bool {
        self.west > self.east
    }
    /// East–west extent in degrees (antimeridian-aware).
    pub fn width(&self) -> f64 {
        if self.crosses_antimeridian() {
            self.east - self.west + 360.0
        } else {
            self.east - self.west
        }
    }
    pub fn height(&self) -> f64 {
        self.north - self.south
    }
    /// The centre (longitude wrapped into [-180, 180]).
    pub fn center(&self) -> Vec2 {
        let mut lon = self.west + self.width() / 2.0;
        if lon > 180.0 {
            lon -= 360.0;
        }
        Vec2::new(lon, (self.south + self.north) / 2.0)
    }
    pub fn contains(&self, p: Vec2) -> bool {
        let in_lat = p.y >= self.south && p.y <= self.north;
        let in_lon = if self.crosses_antimeridian() {
            p.x >= self.west || p.x <= self.east
        } else {
            p.x >= self.west && p.x <= self.east
        };
        in_lat && in_lon
    }
    /// One box, or two when it crosses the antimeridian (west part first).
    pub fn split_antimeridian(&self) -> Vec<GeoBbox> {
        if self.crosses_antimeridian() {
            vec![
                GeoBbox::new(self.west, self.south, 180.0, self.north),
                GeoBbox::new(-180.0, self.south, self.east, self.north),
            ]
        } else {
            vec![*self]
        }
    }
    /// The box as a polygon whose edges follow meridians and parallels, densified every
    /// `step_deg` so projections that curve parallels (conics) keep the shape. Longitudes may run
    /// past 180 for boxes crossing the antimeridian (the projection pipeline cuts them).
    pub fn to_polygon(&self, step_deg: f64) -> Geometry {
        let (w, e) = (self.west, self.west + self.width());
        let (s, n) = (self.south, self.north);
        let step = if step_deg > 0.0 { step_deg } else { 1.0 };
        let seg = |a: Vec2, b: Vec2, out: &mut Vec<Vec2>| {
            let k = ((a.dist(b) / step).ceil() as usize).max(1);
            for i in 0..k {
                out.push(a.lerp(b, i as f64 / k as f64));
            }
        };
        let corners = [Vec2::new(w, s), Vec2::new(w, n), Vec2::new(e, n), Vec2::new(e, s)];
        let mut ring = Vec::new();
        for i in 0..4 {
            seg(corners[i], corners[(i + 1) % 4], &mut ring);
        }
        ring.push(corners[0]);
        Geometry::Polygon(vec![ring])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_and_maps_coordinates() {
        let g = Geometry::Collection(vec![
            Geometry::Point(Vec2::new(1.0, 2.0)),
            Geometry::LineString(vec![Vec2::new(0.0, 0.0), Vec2::new(3.0, 4.0)]),
        ]);
        let mut n = 0;
        g.for_each_coord(|_| n += 1);
        assert_eq!(n, 3);
        let shifted = g.map_coords(&|p| p + Vec2::new(10.0, 0.0));
        assert_eq!(shifted.points(), vec![Vec2::new(11.0, 2.0)]);
        assert_eq!(shifted.lines()[0][1], Vec2::new(13.0, 4.0));
        assert_eq!(g.planar_bounds(), Some(Rect::new(0.0, 0.0, 3.0, 4.0)));
        assert!(Geometry::default().is_empty());
    }

    #[test]
    fn bbox_across_the_antimeridian() {
        let b = GeoBbox::new(170.0, -20.0, -170.0, -10.0);
        assert!(b.crosses_antimeridian());
        assert_eq!(b.width(), 20.0);
        assert_eq!(b.center(), Vec2::new(180.0, -15.0));
        assert!(b.contains(Vec2::new(-175.0, -15.0)));
        assert!(!b.contains(Vec2::new(0.0, -15.0)));
        assert_eq!(b.split_antimeridian().len(), 2);
        let Geometry::Polygon(p) = b.to_polygon(5.0) else { panic!() };
        assert_eq!(p[0].first(), p[0].last());
        assert!(p[0].len() > 8, "densified");
    }
}
