//! Polygons out of an ESRI shapefile (`.shp`), only where they're wanted.
//!
//! The osmdata land polygons (coastline-derived, the right answer to "where is the sea" at street
//! zoom) come as one 1.3 GB shapefile for the planet. A camera-aware extract needs a few small
//! regions of it, so records are read one at a time and skipped by their bounding box before their
//! points are decoded: the planet file streams through in seconds and only the land near the
//! cameras is ever held in memory. No shapefile crate: polygons are the one record type we need,
//! and the format is a fixed header plus length-prefixed records.
//!
//! Lon/lat files (EPSG:4326, e.g. `land-polygons-split-4326`) and Web Mercator ones (EPSG:3857,
//! `simplified-land-polygons-complete-3857`, reprojected to lon/lat as they're read) are
//! supported; attributes (`.dbf`) are not read.

use datars_geo::tile::{lonlat_to_world, world_to_lonlat};
use datars_geo::{Feature, FeatureCollection, GeoBbox, Geometry, Polygon};
use datars_math::Vec2;
use std::io::Read;

/// The coordinates a shapefile is written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Crs {
    /// Degrees (EPSG:4326).
    LonLat,
    /// Web Mercator metres (EPSG:3857).
    WebMercator,
}

/// Half the Web Mercator world in metres (2πR / 2 with R = 6 378 137 m).
const MERCATOR_HALF: f64 = 20_037_508.342_789_244;

impl Crs {
    fn to_lonlat(self, p: Vec2) -> Vec2 {
        match self {
            Crs::LonLat => p,
            Crs::WebMercator => world_to_lonlat(Vec2::new(0.5 + p.x / (2.0 * MERCATOR_HALF), 0.5 - p.y / (2.0 * MERCATOR_HALF))),
        }
    }

    /// A lon/lat box in file coordinates (for skipping records by their bounding box).
    fn box_of_lonlat(self, b: &GeoBbox) -> GeoBbox {
        match self {
            Crs::LonLat => *b,
            Crs::WebMercator => {
                let m = |lon: f64, lat: f64| {
                    let w = lonlat_to_world(Vec2::new(lon, lat));
                    Vec2::new((w.x - 0.5) * 2.0 * MERCATOR_HALF, (0.5 - w.y) * 2.0 * MERCATOR_HALF)
                };
                let (sw, ne) = (m(b.west, b.south), m(b.east, b.north));
                GeoBbox::new(sw.x, sw.y, ne.x, ne.y)
            }
        }
    }
}

const FILE_CODE: i32 = 9994;
const HEADER_LEN: u64 = 100;
const POLYGON: i32 = 5;

fn be_i32(b: &[u8]) -> i32 {
    i32::from_be_bytes([b[0], b[1], b[2], b[3]])
}
fn le_i32(b: &[u8]) -> i32 {
    i32::from_le_bytes([b[0], b[1], b[2], b[3]])
}
fn le_f64(b: &[u8]) -> f64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[..8]);
    f64::from_le_bytes(a)
}

fn overlaps(a: &GeoBbox, b: &GeoBbox) -> bool {
    !(a.east < b.west || a.west > b.east || a.north < b.south || a.south > b.north)
}

/// Twice the signed area of a ring in lon/lat (y up): negative for clockwise.
fn area2(r: &[Vec2]) -> f64 {
    let n = r.len();
    (0..n).map(|i| {
        let (a, b) = (r[i], r[(i + 1) % n]);
        a.x * b.y - b.x * a.y
    }).sum()
}

/// A record's rings as polygons. Shapefiles wind outer rings clockwise and holes counter-clockwise;
/// a hole belongs to the outer ring before it (osmdata writes each polygon's holes right after it).
fn group_rings(rings: Vec<Vec<Vec2>>) -> Vec<Polygon> {
    let mut out: Vec<Polygon> = Vec::new();
    for r in rings {
        if r.len() < 4 {
            continue;
        }
        let hole = area2(&r) > 0.0;
        match out.last_mut() {
            Some(p) if hole => p.push(r),
            _ => out.push(vec![r]),
        }
    }
    out
}

/// Every polygon record whose bounding box overlaps one of `regions` (all of them when `regions`
/// is empty), one feature per record, without properties. Records are read in order through one
/// reused buffer (pass a buffered reader): no seeks, so the planet file streams at disk speed.
pub fn read_polygons<R: Read>(r: R, regions: &[GeoBbox]) -> Result<FeatureCollection, String> {
    read_polygons_in(r, regions, Crs::LonLat)
}

/// [`read_polygons`] for a file in `crs` (regions are lon/lat; features come out in lon/lat).
pub fn read_polygons_in<R: Read>(mut r: R, regions: &[GeoBbox], crs: Crs) -> Result<FeatureCollection, String> {
    let regions: Vec<GeoBbox> = regions.iter().map(|b| crs.box_of_lonlat(b)).collect();
    let io = |e: std::io::Error| format!("shapefile: {e}");
    let mut header = [0u8; HEADER_LEN as usize];
    r.read_exact(&mut header).map_err(io)?;
    if be_i32(&header[0..4]) != FILE_CODE {
        return Err("not a shapefile (bad file code)".into());
    }
    let file_len = be_i32(&header[24..28]) as u64 * 2;
    let shape_type = le_i32(&header[32..36]);
    if shape_type != POLYGON {
        return Err(format!("shapefile holds shape type {shape_type}; only polygons (5) are supported"));
    }
    let mut features = Vec::new();
    let mut pos = HEADER_LEN;
    let mut rec = [0u8; 8];
    let mut content: Vec<u8> = Vec::new();
    while pos + 8 <= file_len {
        r.read_exact(&mut rec).map_err(io)?;
        let len = be_i32(&rec[4..8]).max(0) as usize * 2;
        pos += 8 + len as u64;
        content.resize(len, 0);
        r.read_exact(&mut content).map_err(io)?;
        if len < 44 || le_i32(&content[0..4]) != POLYGON {
            continue; // null shapes (and anything else) carry nothing we draw
        }
        let head = &content[..44];
        let bb = GeoBbox::new(le_f64(&head[4..]), le_f64(&head[12..]), le_f64(&head[20..]), le_f64(&head[28..]));
        if !regions.is_empty() && !regions.iter().any(|g| overlaps(&bb, g)) {
            continue;
        }
        let (parts, points) = (le_i32(&head[36..40]).max(0) as usize, le_i32(&head[40..44]).max(0) as usize);
        let body = &content[44..];
        if body.len() < parts * 4 + points * 16 {
            return Err(format!("shapefile: truncated polygon record at byte {}", pos - len as u64 - 8));
        }
        let starts: Vec<usize> = (0..parts).map(|i| le_i32(&body[i * 4..]).max(0) as usize).collect();
        let pts: Vec<Vec2> = (0..points).map(|i| {
            let o = parts * 4 + i * 16;
            crs.to_lonlat(Vec2::new(le_f64(&body[o..]), le_f64(&body[o + 8..])))
        }).collect();
        let rings: Vec<Vec<Vec2>> = (0..parts).map(|i| {
            let end = starts.get(i + 1).copied().unwrap_or(points).min(points);
            pts[starts[i].min(end)..end].to_vec()
        }).collect();
        let polys = group_rings(rings);
        let geometry = match polys.len() {
            0 => continue,
            1 => Geometry::Polygon(polys.into_iter().next().unwrap_or_default()),
            _ => Geometry::MultiPolygon(polys),
        };
        features.push(Feature::new(geometry));
    }
    Ok(FeatureCollection::new(features))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A polygon shapefile in memory: one record per entry, each a list of rings.
    pub(crate) fn shapefile(records: &[Vec<Vec<(f64, f64)>>]) -> Vec<u8> {
        let mut body = Vec::new();
        for (n, rings) in records.iter().enumerate() {
            let pts: Vec<(f64, f64)> = rings.iter().flatten().copied().collect();
            let (x0, y0) = pts.iter().fold((f64::MAX, f64::MAX), |a, p| (a.0.min(p.0), a.1.min(p.1)));
            let (x1, y1) = pts.iter().fold((f64::MIN, f64::MIN), |a, p| (a.0.max(p.0), a.1.max(p.1)));
            let mut c = Vec::new();
            c.extend(POLYGON.to_le_bytes());
            for v in [x0, y0, x1, y1] {
                c.extend(v.to_le_bytes());
            }
            c.extend((rings.len() as i32).to_le_bytes());
            c.extend((pts.len() as i32).to_le_bytes());
            let mut start = 0i32;
            for r in rings {
                c.extend(start.to_le_bytes());
                start += r.len() as i32;
            }
            for (x, y) in &pts {
                c.extend(x.to_le_bytes());
                c.extend(y.to_le_bytes());
            }
            body.extend((n as i32 + 1).to_be_bytes());
            body.extend((c.len() as i32 / 2).to_be_bytes());
            body.extend(c);
        }
        let mut h = vec![0u8; HEADER_LEN as usize];
        h[0..4].copy_from_slice(&FILE_CODE.to_be_bytes());
        h[24..28].copy_from_slice(&(((HEADER_LEN as usize + body.len()) / 2) as i32).to_be_bytes());
        h[28..32].copy_from_slice(&1000i32.to_le_bytes());
        h[32..36].copy_from_slice(&POLYGON.to_le_bytes());
        h.extend(body);
        h
    }

    /// A clockwise square (a shapefile outer ring) from (x, y) of side s.
    pub(crate) fn cw(x: f64, y: f64, s: f64) -> Vec<(f64, f64)> {
        vec![(x, y), (x, y + s), (x + s, y + s), (x + s, y), (x, y)]
    }

    #[test]
    fn reads_polygons_with_holes_and_skips_by_bbox() {
        let mut hole = cw(11.0, 51.0, 1.0);
        hole.reverse();
        let bytes = shapefile(&[vec![cw(10.0, 50.0, 3.0), hole], vec![cw(100.0, 0.0, 1.0)], vec![cw(20.0, 50.0, 1.0), cw(22.0, 50.0, 1.0)]]);
        let all = read_polygons(std::io::Cursor::new(&bytes), &[]).unwrap();
        assert_eq!(all.len(), 3);
        match &all.features[0].geometry {
            Geometry::Polygon(p) => assert_eq!(p.len(), 2, "outer ring + its hole"),
            g => panic!("{g:?}"),
        }
        assert!(matches!(&all.features[2].geometry, Geometry::MultiPolygon(pp) if pp.len() == 2), "two outer rings");
        // Only records near Europe: the Pacific one is skipped unread.
        let some = read_polygons(std::io::Cursor::new(&bytes), &[GeoBbox::new(0.0, 40.0, 30.0, 60.0)]).unwrap();
        assert_eq!(some.len(), 2);
        assert!(read_polygons(std::io::Cursor::new(&bytes[..50]), &[]).is_err());
    }

    #[test]
    fn web_mercator_files_come_out_in_lon_lat() {
        // A square around Rio in metres: x = lon·πR/180, y = R·ln(tan(π/4 + lat/2)).
        let (x0, x1, y0, y1) = (-4_815_000.0, -4_805_000.0, -2_630_000.0, -2_620_000.0);
        let ring = vec![(x0, y0), (x0, y1), (x1, y1), (x1, y0), (x0, y0)];
        let bytes = shapefile(&[vec![ring], vec![cw(0.0, 0.0, 1000.0)]]);
        let rio = GeoBbox::new(-43.5, -23.2, -43.0, -22.7);
        let fc = read_polygons_in(std::io::Cursor::new(&bytes), &[rio], Crs::WebMercator).unwrap();
        assert_eq!(fc.len(), 1, "the null-island square is skipped by its box");
        let b = datars_geo::measure::bbox(&fc.features[0].geometry).unwrap();
        assert!((b.west + 43.254).abs() < 0.01 && (b.east + 43.164).abs() < 0.01, "{b:?}");
        assert!((b.south + 23.0).abs() < 0.1 && b.north < b.south + 0.1 && b.north > b.south, "{b:?}");
    }
}
