//! `datars-geo` — geographic coordinates, geometry and tile formats (docs/09-geo.md).
//!
//! Everything a map needs at runtime, sans-IO and deterministic (all transcendental math through
//! `datars_math::m`, no hash-map iteration, bytes in and values out):
//!
//! | Module | What |
//! |---|---|
//! | [`geometry`] | `Geometry` (GeoJSON's types), `Feature`, `FeatureCollection`, `GeoBbox` |
//! | [`geojson`], [`topojson`] | parsing (and GeoJSON writing); TopoJSON keeps shared arcs in a `Topology` |
//! | [`projection`] | `Projection` (+ compiled `Projector`): equirectangular, (web) Mercator, Equal Earth, Natural Earth, Albers (+ USA), Lambert conformal conic, orthographic, transverse Mercator (UTM, SWEREF 99 TM), planar; `fit_extent` |
//! | [`mod@project`] | projected geometry: adaptive resampling, antimeridian cutting, horizon clipping, `to_path`, the `sphere` outline |
//! | [`clip`] | rectangle clipping of polygons and lines (robust rejoin), `geo` boolean ops behind `build` |
//! | [`simplify`] | Douglas–Peucker, Visvalingam, ring-safe; topology-preserving via `Topology::simplify` |
//! | [`earcut`] | triangulation with holes |
//! | [`measure`] | spherical area, centroid, bounds (antimeridian-aware), containment, distance, geodesics |
//! | [`tile`], [`mvt`], [`pmtiles`] | tile math, Mapbox Vector Tiles, PMTiles v3 (sans-IO reader, writer) |
//!
//! Coordinates are `datars_math::Vec2`: `x` = longitude, `y` = latitude in degrees (or planar
//! units with `Projection::planar`). Projected space is y-down, like the rest of the engine.

#![forbid(unsafe_code)]

pub mod clip;
pub mod earcut;
pub mod error;
pub mod geojson;
pub mod geometry;
pub mod measure;
pub mod mvt;
pub mod pmtiles;
pub mod project;
pub mod projection;
pub mod simplify;
mod sphere;
pub mod tile;
pub mod topojson;
pub mod topo_encode;

pub use clip::{clip_geometry, clip_line, clip_polygon};
pub use earcut::triangulate;
pub use error::GeoError;
pub use geojson::{parse_geojson, GeoJsonOptions};
pub use geometry::{Feature, FeatureCollection, GeoBbox, Geometry, Line, Polygon, Ring};
pub use measure::geodesic;
pub use project::{project, project_geometry, sphere, to_path, ProjectedGeometry};
pub use projection::{fit_extent, Fit, Projection, ProjectionKind, Projector, TransverseMercator};
pub use simplify::SimplifyMethod;
pub use tile::{tiles_covering, TileId};
pub use topojson::{parse_topojson, Topology};
