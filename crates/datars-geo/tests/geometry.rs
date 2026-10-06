//! Geometry end to end: GeoJSON/TopoJSON fixtures, topology-preserving simplification,
//! triangulation, measures, rectangle clipping, and a GeoJSON → MVT → PMTiles → back pipeline.

use datars_geo::clip::{clip_polygon, polygon_contains_planar};
use datars_geo::earcut::{triangulate, triangulate_polygons};
use datars_geo::geojson::{parse_geojson, to_geojson_string, GeoJsonOptions};
use datars_geo::measure;
use datars_geo::mvt::{self, Layer, VectorTile};
use datars_geo::pmtiles::{Reader, Step, Writer, WriterOptions};
use datars_geo::project::project;
use datars_geo::simplify::{simplify_geometry, SimplifyMethod};
use datars_geo::topojson::{parse_topojson, MeshFilter, ZoomBand};
use datars_geo::{tiles_covering, FeatureCollection, GeoBbox, Geometry, Polygon, Projection, TileId};
use datars_math::path::signed_area;
use datars_math::{m, Rect, Vec2};

const NE: &[u8] = include_bytes!("fixtures/ne_countries_subset.geojson");
const SQUARE: &[u8] = include_bytes!("fixtures/square.geojson");
const HOLE: &[u8] = include_bytes!("fixtures/polygon_with_hole.geojson");
const ANTI: &[u8] = include_bytes!("fixtures/antimeridian.geojson");
const TOPO: &[u8] = include_bytes!("fixtures/neighbours.topojson");

fn v(x: f64, y: f64) -> Vec2 {
    Vec2::new(x, y)
}

fn ne() -> FeatureCollection {
    parse_geojson(NE, &GeoJsonOptions::id_from("iso_a3")).unwrap()
}

fn country(id: &str) -> Geometry {
    ne().get(id).unwrap_or_else(|| panic!("{id}")).geometry.clone()
}

fn planar_area(p: &Polygon) -> f64 {
    let mut rings = p.iter().map(|r| signed_area(r).abs());
    rings.next().unwrap_or(0.0) - rings.sum::<f64>()
}

// ---- parsing ---------------------------------------------------------------------------------

#[test]
fn geojson_fixtures() {
    let sq = parse_geojson(SQUARE, &GeoJsonOptions::default()).unwrap();
    assert_eq!(sq.features[0].id.as_deref(), Some("sq"));
    let holes = parse_geojson(HOLE, &GeoJsonOptions::id_from("code")).unwrap();
    assert_eq!(holes.get("1").unwrap().property_str("name").as_deref(), Some("Donut"));
    let Geometry::Polygon(donut) = &holes.get("1").unwrap().geometry else { panic!() };
    assert_eq!(donut.len(), 2);
    let Geometry::MultiPolygon(arch) = &holes.get("2").unwrap().geometry else { panic!() };
    assert_eq!(arch.len(), 2);
    assert_eq!(arch[1].len(), 2, "second part has a hole");
    let world = ne();
    assert_eq!(world.len(), 10);
    let Geometry::MultiPolygon(zaf) = &world.get("ZAF").unwrap().geometry else { panic!() };
    assert!(zaf.iter().any(|p| p.len() > 1), "South Africa keeps its Lesotho hole");
    assert_eq!(world.get("SWE").unwrap().property_str("name").as_deref(), Some("Sweden"));
    // Writing and reading back is lossless.
    let text = to_geojson_string(&world, None);
    assert_eq!(parse_geojson(text.as_bytes(), &GeoJsonOptions::id_from("iso_a3")).unwrap(), world);
}

#[test]
fn topojson_shares_arcs_between_neighbours() {
    let t = parse_topojson(TOPO, &GeoJsonOptions::default()).unwrap();
    assert_eq!(t.object_names().collect::<Vec<_>>(), vec!["border", "capital", "regions"]);
    let regions = t.features("regions").unwrap();
    assert_eq!(regions.len(), 3);
    assert_eq!(regions.features[2].property_str("code").as_deref(), Some("03"));
    let (w, e) = (&regions.get("W").unwrap().geometry, &regions.get("E").unwrap().geometry);
    // The shared border appears forwards in one and backwards in the other.
    let border = t.arc_points(0);
    assert!(contains_run(ring_of(w), &border));
    let rev: Vec<Vec2> = border.iter().rev().copied().collect();
    assert!(contains_run(ring_of(e), &rev));
    let capital = t.features("capital").unwrap();
    assert!((capital.features[0].geometry.points()[0] - v(5.0, 5.0)).len() < 1e-9);
    assert_eq!(t.features("border").unwrap().features[0].geometry.lines()[0], &border);
    let inner = t.mesh("regions", MeshFilter::Interior).unwrap();
    assert_eq!(inner.lines(), vec![&border]);
}

fn ring_of(g: &Geometry) -> &[Vec2] {
    match g {
        Geometry::Polygon(p) => &p[0],
        _ => panic!("{g:?}"),
    }
}

/// Whether `run` appears as a contiguous subsequence of the closed ring (cyclically).
fn contains_run(ring: &[Vec2], run: &[Vec2]) -> bool {
    let open = &ring[..ring.len() - 1];
    let n = open.len();
    (0..n).any(|s| run.iter().enumerate().all(|(k, p)| open[(s + k) % n] == *p))
}

#[test]
fn topology_preserving_simplification_keeps_borders_identical() {
    let t = parse_topojson(TOPO, &GeoJsonOptions::default()).unwrap();
    let full = t.vertex_count();
    for method in [SimplifyMethod::DouglasPeucker, SimplifyMethod::Visvalingam] {
        let mut last = usize::MAX;
        for tol in [0.001, 0.02, 0.1, 0.4, 5.0] {
            let s = t.simplify(tol, method);
            let n = s.vertex_count();
            assert!(n <= last && n <= full, "{method:?} {tol}: monotone");
            last = n;
            let fc = s.features("regions").unwrap();
            let (w, e) = (ring_of(&fc.get("W").unwrap().geometry), ring_of(&fc.get("E").unwrap().geometry));
            let border = s.arc_points(0);
            let rev: Vec<Vec2> = border.iter().rev().copied().collect();
            assert!(contains_run(w, &border) && contains_run(e, &rev), "{method:?} {tol}: shared border identical");
            // Junctions never move.
            assert_eq!(border.first(), t.arc_points(0).first());
            assert_eq!(border.last(), t.arc_points(0).last());
            // Every ring stays valid, the one-arc island included.
            for f in &fc.features {
                for p in f.geometry.polygons() {
                    for r in p {
                        assert!(r.len() >= 4 && r.first() == r.last(), "{method:?} {tol}: {:?} ring of {}", f.id, r.len());
                        assert!(signed_area(r).abs() > 0.0);
                    }
                }
            }
        }
        assert!(last < full / 4, "{method:?}: big tolerances simplify a lot ({last} of {full})");
    }
    // Independent per-feature simplification does NOT keep them identical (why topology matters).
    let fc = t.features("regions").unwrap();
    let w = simplify_geometry(&fc.get("W").unwrap().geometry, 0.1, SimplifyMethod::DouglasPeucker);
    let e = simplify_geometry(&fc.get("E").unwrap().geometry, 0.1, SimplifyMethod::DouglasPeucker);
    let on_border = |g: &Geometry| ring_of(g).iter().filter(|p| p.x.abs() <= 1.0 && p.y > 0.0 && p.y < 10.0).count();
    let sb = t.simplify(0.1, SimplifyMethod::DouglasPeucker).arc_points(0).len() - 2;
    assert!(on_border(&w) != sb || on_border(&e) != sb, "independent simplification diverges");
    // Zoom bands: one weighting, finer bands keep more.
    let bands = [ZoomBand::for_pixels(0, 2, 1.0), ZoomBand::for_pixels(3, 5, 1.0), ZoomBand::for_pixels(6, 8, 1.0)];
    assert!(bands[0].tolerance > bands[1].tolerance && bands[1].tolerance > bands[2].tolerance);
    let out = t.simplify_bands(&bands, SimplifyMethod::Visvalingam);
    let counts: Vec<usize> = out.iter().map(|(_, s)| s.vertex_count()).collect();
    assert!(counts[0] <= counts[1] && counts[1] <= counts[2], "{counts:?}");
}

// ---- triangulation ---------------------------------------------------------------------------

fn tri_area(verts: &[Vec2], idx: &[u32]) -> f64 {
    idx.chunks_exact(3)
        .map(|t| {
            let (a, b, c) = (verts[t[0] as usize], verts[t[1] as usize], verts[t[2] as usize]);
            ((b - a).cross(c - a) / 2.0).abs()
        })
        .sum()
}

#[test]
fn earcut_triangle_areas_sum_to_polygon_area() {
    let holes = parse_geojson(HOLE, &GeoJsonOptions::default()).unwrap();
    let Geometry::Polygon(donut) = &holes.features[0].geometry else { panic!() };
    let (vs, idx) = triangulate(donut);
    assert!((tri_area(&vs, &idx) - 84.0).abs() < 1e-9);
    // Real, concave coastlines with holes (South Africa around Lesotho).
    for id in ["SWE", "NOR", "ZAF", "USA", "NZL", "FJI", "ATA"] {
        let g = country(id);
        for p in g.polygons() {
            let (vs, idx) = triangulate(p);
            let want = planar_area(p);
            let got = tri_area(&vs, &idx);
            assert!((got - want).abs() <= 1e-9 * want.max(1.0), "{id}: {got} vs {want}");
        }
    }
    // Projected polygons (screen space) triangulate the same way.
    let usa = project(&country("USA"), &Projection::albers_usa(), 0.5);
    let polys = usa.polygons();
    let (vs, idx) = triangulate_polygons(&polys);
    let want: f64 = polys.iter().map(planar_area).sum();
    assert!((tri_area(&vs, &idx) - want).abs() < 1e-6 * want);
    assert!((usa.area() - want).abs() < 1e-6 * want, "grouping keeps holes as holes");
}

// ---- measures --------------------------------------------------------------------------------

#[test]
fn spherical_area() {
    let sq = parse_geojson(SQUARE, &GeoJsonOptions::default()).unwrap().features[0].geometry.clone();
    assert!((measure::area(&sq) - 0.030_382_156_674_6).abs() < 1e-9);
    // Winding doesn't matter.
    let rev = sq.map_coords(&|p| p);
    let Geometry::Polygon(mut p) = rev else { panic!() };
    p[0].reverse();
    assert!((measure::area(&Geometry::Polygon(p)) - measure::area(&sq)).abs() < 1e-15);
    // A polar cap at 80°N: 2π(1 − sin 80°).
    let cap: Vec<Vec2> = (0..=360).map(|i| v(-180.0 + i as f64, 80.0)).collect();
    let want = m::TAU * (1.0 - m::sin(80f64.to_radians()));
    assert!((measure::area(&Geometry::Polygon(vec![cap])) - want).abs() < 1e-4 * 4.0 * m::PI);
    // Sweden ≈ 450 000 km² (Natural Earth 1:50m, simplified).
    let swe = measure::area_m2(&country("SWE")) / 1e6;
    assert!((swe - 450_000.0).abs() < 25_000.0, "{swe} km²");
    // Holes subtract: South Africa + Lesotho = South Africa's outline.
    let zaf = country("ZAF");
    let outline = Geometry::MultiPolygon(zaf.polygons().iter().map(|p| vec![p[0].clone()]).collect());
    let (a, b, c) = (measure::area(&zaf), measure::area(&country("LSO")), measure::area(&outline));
    assert!(((a + b) - c).abs() < 1e-3 * c, "{a} + {b} vs {c}");
    // Antarctica (a ring around the pole) is not the rest of the world.
    let ata = measure::area_m2(&country("ATA")) / 1e6;
    assert!(ata > 1.0e7 && ata < 1.5e7, "{ata} km²");
    assert_eq!(measure::area(&Geometry::LineString(vec![v(0.0, 0.0), v(1.0, 1.0)])), 0.0);
}

#[test]
fn centroid_and_containment() {
    let sq = parse_geojson(SQUARE, &GeoJsonOptions::default()).unwrap().features[0].geometry.clone();
    let c = measure::centroid(&sq).unwrap();
    assert!((c.x - 5.0).abs() < 1e-9 && c.y > 4.9 && c.y < 5.2, "{c:?}");
    let swe = country("SWE");
    let cs = measure::centroid(&swe).unwrap();
    assert!(measure::contains(&swe, cs), "Sweden's centroid is in Sweden: {cs:?}");
    assert!((cs.x - 16.7).abs() < 1.5 && (cs.y - 62.5).abs() < 1.5, "{cs:?}");
    // (Stockholm itself sits in the archipelago's water at 1:50m; take inland Jönköping.)
    assert!(measure::contains(&swe, v(14.16, 57.78)), "Jönköping");
    assert!(!measure::contains(&swe, v(10.75, 59.91)), "Oslo is not in Sweden");
    assert!(measure::contains(&country("NOR"), v(10.75, 59.91)));
    assert!(!measure::contains(&country("ZAF"), v(27.48, -29.31)), "Maseru is in the Lesotho hole");
    assert!(measure::contains(&country("LSO"), v(27.48, -29.31)));
    let holes = parse_geojson(HOLE, &GeoJsonOptions::default()).unwrap();
    assert!(!measure::contains(&holes.features[0].geometry, v(25.0, 45.0)));
    assert!(measure::contains(&holes.features[0].geometry, v(21.0, 41.0)));
    let anti = parse_geojson(ANTI, &GeoJsonOptions::default()).unwrap();
    let boxg = &anti.get("box").unwrap().geometry;
    assert!(measure::contains(boxg, v(179.5, -15.0)) && measure::contains(boxg, v(-179.5, -15.0)));
    assert!(!measure::contains(boxg, v(0.0, -15.0)));
    // Lines: length-weighted centroid; points: mean.
    let line = Geometry::LineString(vec![v(0.0, 0.0), v(10.0, 0.0)]);
    assert!((measure::centroid(&line).unwrap() - v(5.0, 0.0)).len() < 1e-9);
    assert!(measure::centroid(&Geometry::default()).is_none());
    // Planar containment for floor plans.
    let Geometry::Polygon(donut) = &holes.features[0].geometry else { panic!() };
    assert!(polygon_contains_planar(donut, v(21.0, 41.0)) && !polygon_contains_planar(donut, v(25.0, 45.0)));
}

#[test]
fn bounds_handle_the_antimeridian_and_poles() {
    let sq = parse_geojson(SQUARE, &GeoJsonOptions::default()).unwrap().features[0].geometry.clone();
    let b = measure::bbox(&sq).unwrap();
    assert!((b.west, b.south, b.east) == (0.0, 0.0, 10.0) && b.north >= 10.0 && b.north < 10.2, "{b:?}");
    let fiji = measure::bbox(&country("FJI")).unwrap();
    assert!(fiji.crosses_antimeridian(), "{fiji:?}");
    assert!(fiji.west > 170.0 && fiji.east < -175.0 && fiji.width() < 10.0, "{fiji:?}");
    let ata = measure::bbox(&country("ATA")).unwrap();
    assert_eq!((ata.west, ata.east, ata.south), (-180.0, 180.0, -90.0));
    assert!(ata.north < -60.0);
    let anti = parse_geojson(ANTI, &GeoJsonOptions::default()).unwrap();
    let bx = measure::bbox(&anti.get("box").unwrap().geometry).unwrap();
    assert!((bx.west - 170.0).abs() < 1e-9 && (bx.east + 170.0).abs() < 1e-9, "{bx:?}");
    let pts = measure::bbox(&Geometry::MultiPoint(vec![v(179.0, 0.0), v(-179.0, 1.0)])).unwrap();
    assert_eq!((pts.west, pts.east, pts.south, pts.north), (179.0, -179.0, 0.0, 1.0));
    let route = measure::bbox(&anti.get("route").unwrap().geometry).unwrap();
    assert!(route.crosses_antimeridian() && route.north > 45.0, "the great circle bulges north: {route:?}");
    let swe = measure::bbox(&country("SWE")).unwrap();
    assert!(!swe.crosses_antimeridian() && swe.west > 10.0 && swe.east < 25.0 && swe.south > 55.0 && swe.north < 70.0);
    assert!(measure::bbox(&Geometry::default()).is_none());
    // Tiles covering a bbox across the antimeridian.
    let tiles = tiles_covering(&fiji, 3);
    assert!(tiles.iter().any(|t| t.x == 0) && tiles.iter().any(|t| t.x == 7));
}

#[test]
fn geodesic_routes_are_great_circles() {
    let (a, b) = (v(18.07, 59.33), v(-122.42, 37.77)); // Stockholm → San Francisco
    let pts = datars_geo::geodesic(a, b, 64);
    assert_eq!(pts.len(), 65);
    assert_eq!((pts[0], pts[64]), (a, b));
    let d = measure::distance(a, b);
    let sum: f64 = pts.windows(2).map(|w| measure::distance(w[0], w[1])).sum();
    assert!((sum - d).abs() < 1.0, "segments add up to the whole: {sum} vs {d}");
    assert!(pts.iter().any(|p| p.y > 70.0), "polar route");
    assert!((measure::length(&Geometry::LineString(pts)) * measure::EARTH_RADIUS_M - d).abs() < 1.0);
}

// ---- clipping --------------------------------------------------------------------------------

#[test]
fn concave_coastlines_clip_consistently() {
    // Norway's fjords cross any rectangle many times. Tiling its bounding box into a grid and
    // clipping to every cell must conserve area exactly, with every piece valid and inside.
    let nor = country("NOR");
    let total: f64 = nor.polygons().iter().map(|p| planar_area(p)).sum();
    let bb = nor.planar_bounds().unwrap();
    let (nx, ny) = (4, 5);
    let mut sum = 0.0;
    let mut pieces = 0;
    for i in 0..nx {
        for j in 0..ny {
            let cell = Rect::new(bb.x + bb.w * i as f64 / nx as f64, bb.y + bb.h * j as f64 / ny as f64, bb.w / nx as f64, bb.h / ny as f64);
            for p in nor.polygons() {
                for piece in clip_polygon(p, cell) {
                    pieces += 1;
                    assert!(signed_area(&piece[0]) > 0.0, "exterior positive");
                    for (k, r) in piece.iter().enumerate() {
                        assert!(r.len() >= 4 && r.first() == r.last());
                        assert!(r.iter().all(|q| cell.inset(-1e-9).contains(*q)));
                        if k > 0 {
                            assert!(signed_area(r) < 0.0, "holes negative");
                        }
                    }
                    sum += planar_area(&piece);
                }
            }
        }
    }
    assert!((sum - total).abs() < 1e-9 * total, "{sum} vs {total}");
    assert!(pieces > 20, "fjords make many pieces: {pieces}");
}

// ---- pipeline --------------------------------------------------------------------------------

#[test]
fn geojson_to_mvt_to_pmtiles_and_back() {
    let world = ne();
    let mut writer = Writer::new(WriterOptions { leaf_entries: Some(4), ..WriterOptions::default() });
    let mut written = Vec::new();
    for z in 0..=3u8 {
        let mut ids: Vec<TileId> = Vec::new();
        for f in &world.features {
            if let Some(b) = measure::bbox(&f.geometry) {
                ids.extend(tiles_covering(&GeoBbox::new(b.west, b.south.max(-85.0), b.east, b.north.min(85.0)), z));
            }
        }
        ids.sort();
        ids.dedup();
        for t in ids {
            let mut layer = Layer::new("countries");
            let simplified = world.features.iter().map(|f| {
                let mut f = f.clone();
                f.geometry = simplify_geometry(&f.geometry, datars_geo::simplify::zoom_tolerance(z as f64, 0.5), SimplifyMethod::Visvalingam);
                f
            });
            layer.features.extend(simplified.filter_map(|f| mvt::from_feature(&f, t, 4096, 64)));
            if !layer.features.is_empty() {
                writer.add_tile(t, VectorTile { layers: vec![layer] }.encode());
                written.push(t);
            }
        }
    }
    let archive = writer.finish().unwrap();
    // Read the z3 tile over Jönköping back through the sans-IO range protocol.
    let jkpg = v(14.16, 57.78);
    let t = TileId::from_lonlat(jkpg, 3);
    assert!(written.contains(&t));
    let mut reader = Reader::open(&archive[..archive.len().min(16384)]).unwrap();
    let bytes = loop {
        match reader.plan_get(t).unwrap() {
            Step::Needs(r) => reader.provide(r, &archive[r.offset as usize..(r.offset + r.length) as usize]).unwrap(),
            Step::Ready(b) => break b,
            Step::Absent => panic!("tile missing"),
        }
    };
    let tile = VectorTile::decode(&bytes).unwrap();
    let layer = tile.layer("countries").unwrap();
    let swe = layer.features.iter().find(|f| f.get("iso_a3").and_then(|v| v.as_str()) == Some("SWE")).expect("Sweden in its tile");
    let back = mvt::to_feature(swe, t, layer.extent);
    assert_eq!(back.property_str("name").as_deref(), Some("Sweden"));
    assert!(measure::contains(&back.geometry, jkpg), "Jönköping still inside after the round trip");
    // Every written tile resolves.
    for t in written {
        assert!(datars_geo::pmtiles::read_tile(&archive, t).unwrap().is_some());
    }
}
