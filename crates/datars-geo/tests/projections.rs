//! Projections and projected geometry: round trips, known values (d3 and geodesy references),
//! Albers USA insets, antimeridian cutting, horizon clipping, resampling, fitting.

use datars_geo::geojson::{parse_geojson, GeoJsonOptions};
use datars_geo::project::project;
use datars_geo::projection::{Fit, Projection, ProjectionKind, TransverseMercator};
use datars_geo::{tile, FeatureCollection, GeoBbox, Geometry, TileId};
use datars_math::{m, Rect, Vec2};

const NE: &[u8] = include_bytes!("fixtures/ne_countries_subset.geojson");
const ANTI: &[u8] = include_bytes!("fixtures/antimeridian.geojson");

fn ne() -> FeatureCollection {
    parse_geojson(NE, &GeoJsonOptions::id_from("iso_a3")).unwrap()
}

fn country(id: &str) -> Geometry {
    ne().get(id).unwrap_or_else(|| panic!("{id}")).geometry.clone()
}

fn v(x: f64, y: f64) -> Vec2 {
    Vec2::new(x, y)
}

fn lon_diff(a: f64, b: f64) -> f64 {
    let d = m::rem_euclid(a - b + 180.0, 360.0) - 180.0;
    d.abs()
}

fn grid(lon: (f64, f64), lat: (f64, f64), n: usize) -> Vec<Vec2> {
    let mut out = Vec::new();
    for i in 0..=n {
        for j in 0..=n {
            let t = i as f64 / n as f64;
            let s = j as f64 / n as f64;
            out.push(v(lon.0 + (lon.1 - lon.0) * t, lat.0 + (lat.1 - lat.0) * s));
        }
    }
    out
}

fn assert_round_trips(name: &str, p: &Projection, pts: &[Vec2], tol_deg: f64) -> usize {
    let mut n = 0;
    for &q in pts {
        let Some(xy) = p.forward(q) else { continue };
        let back = p.inverse(xy).unwrap_or_else(|| panic!("{name}: no inverse for {q:?} → {xy:?}"));
        assert!(
            lon_diff(back.x, q.x) < tol_deg && (back.y - q.y).abs() < tol_deg,
            "{name}: {q:?} → {xy:?} → {back:?}"
        );
        n += 1;
    }
    n
}

#[test]
fn forward_inverse_round_trips_for_every_projection() {
    let world = grid((-179.0, 179.0), (-80.0, 80.0), 24);
    let cases: Vec<(&str, Projection, f64)> = vec![
        ("equirectangular", Projection::equirectangular(), 1e-9),
        ("mercator", Projection::mercator(), 1e-9),
        ("web mercator", Projection::web_mercator(), 1e-9),
        ("web mercator px z7", Projection::web_mercator_px(7.0), 1e-9),
        ("equal earth", Projection::equal_earth(), 1e-9),
        ("natural earth", Projection::natural_earth1(), 1e-9),
        ("albers conic", Projection::albers_conic([20.0, 50.0]), 1e-9),
        ("albers (us preset)", Projection::albers(), 1e-9),
        ("albers symmetric → cylindrical", Projection::albers_conic([-30.0, 30.0]), 1e-9),
        ("lambert conformal conic", Projection::lambert_conformal_conic([30.0, 60.0]), 1e-9),
        ("lcc tangent", Projection::lambert_conformal_conic([45.0, 45.0]), 1e-9),
        ("orthographic", Projection::orthographic(15.0, 55.0), 1e-9),
        ("rotated equal earth", Projection::equal_earth().with_rotate([-150.0, 10.0, 20.0]), 1e-9),
        ("centred mercator", Projection::mercator().with_center(v(18.0, 59.0)).with_scale(4000.0), 1e-9),
    ];
    for (name, p, tol) in &cases {
        let n = assert_round_trips(name, p, &world, *tol);
        assert!(n > 200, "{name}: only {n} points projected");
    }
    // Transverse Mercator grids near their zones.
    let sweden = grid((10.0, 24.0), (55.0, 69.0), 14);
    assert!(assert_round_trips("sweref99tm", &Projection::sweref99tm(), &sweden, 1e-9) == sweden.len());
    let zone33 = grid((9.0, 21.0), (-80.0, 84.0), 12);
    assert_eq!(assert_round_trips("utm 33N", &Projection::utm(33, true), &zone33, 1e-9), zone33.len());
    assert_eq!(assert_round_trips("utm 56S", &Projection::utm(56, false), &grid((147.0, 159.0), (-60.0, 0.0), 8), 1e-9), 81);
    // Planar: identity and y-up.
    let plan = grid((0.0, 100.0), (0.0, 50.0), 5);
    for q in &plan {
        assert_eq!(Projection::planar().forward(*q), Some(*q));
        assert_eq!(Projection::planar_y_up().forward(*q), Some(v(q.x, -q.y)));
        assert_eq!(Projection::planar_y_up().inverse(v(q.x, -q.y)), Some(*q));
    }
    // Albers USA through the composite inverse.
    let us = [v(-122.4, 37.8), v(-74.0, 40.7), v(-95.99, 36.15), v(-149.9, 61.2), v(-157.86, 21.3), v(-147.7, 64.8), v(-155.5, 19.6)];
    assert_eq!(assert_round_trips("albers usa", &Projection::albers_usa(), &us, 1e-9), us.len());
}

#[test]
fn web_mercator_known_values() {
    let wm = Projection::web_mercator();
    assert!((wm.forward(v(0.0, 0.0)).unwrap() - v(0.5, 0.5)).len() < 1e-15);
    assert!((wm.forward(v(-180.0, tile::MAX_LAT)).unwrap() - v(0.0, 0.0)).len() < 1e-12);
    assert!((wm.forward(v(180.0, -tile::MAX_LAT)).unwrap() - v(1.0, 1.0)).len() < 1e-12);
    // Beyond the limit latitude clamps to the square.
    assert!((wm.forward(v(0.0, 89.9)).unwrap().y).abs() < 1e-12);
    // Agrees with the tile module everywhere.
    for q in grid((-180.0, 180.0), (-85.0, 85.0), 20) {
        let a = wm.forward(q).unwrap();
        let b = tile::lonlat_to_world(q);
        assert!((a - b).len() < 1e-12, "{q:?}: {a:?} vs {b:?}");
    }
    // Pixel space: 512·2^z across; Stockholm at z4 lies in tile (8, 4).
    let px = Projection::web_mercator_px(4.0);
    let sthlm = px.forward(v(18.0686, 59.3293)).unwrap();
    assert_eq!(((sthlm.x / 512.0).floor(), (sthlm.y / 512.0).floor()), (8.0, 4.0));
    assert_eq!(TileId::from_lonlat(v(18.0686, 59.3293), 4), TileId::new(4, 8, 4));
    assert!((Projection::web_mercator_px(1.0).forward(v(0.0, 0.0)).unwrap() - v(512.0, 512.0)).len() < 1e-9);
    // 45°N in world units: y = 0.5 − ln(tan(67.5°))/2π.
    let y45 = 0.5 - m::ln(m::tan(67.5f64.to_radians())) / m::TAU;
    assert!((wm.forward(v(0.0, 45.0)).unwrap().y - y45).abs() < 1e-15);
}

#[test]
fn albers_usa_matches_d3_and_uses_the_insets() {
    let p = Projection::albers_usa();
    // Reference values from d3-geo's test suite (geoAlbersUsa defaults: scale 1070, [480, 250]).
    let cases = [
        (v(-122.4194, 37.7749), v(107.4, 214.1)), // San Francisco
        (v(-74.0059, 40.7128), v(794.6, 176.5)),  // New York
        (v(-95.9928, 36.1540), v(488.8, 298.0)),  // Tulsa
        (v(-149.9003, 61.2181), v(171.2, 446.9)), // Anchorage
        (v(-157.8583, 21.3069), v(298.5, 451.0)), // Honolulu
    ];
    for (ll, want) in cases {
        let got = p.forward(ll).unwrap();
        assert!((got.x - want.x).abs() < 0.1 && (got.y - want.y).abs() < 0.1, "{ll:?}: {got:?} vs d3 {want:?}");
    }
    assert_eq!(p.forward(v(2.3522, 48.8566)), None, "Paris is not in the USA");
    // The inset boxes (d3's fractions of the scale around the translate).
    let (k, t) = (p.scale, p.translate);
    let boxed = |x0: f64, y0: f64, x1: f64, y1: f64| Rect::new(t.x + x0 * k, t.y + y0 * k, (x1 - x0) * k, (y1 - y0) * k);
    let lower48 = boxed(-0.455, -0.238, 0.455, 0.238);
    let alaska = boxed(-0.425, 0.120, -0.214, 0.234);
    let hawaii = boxed(-0.214, 0.166, -0.115, 0.234);
    assert!(alaska.contains(p.forward(v(-149.9, 61.2)).unwrap()));
    assert!(alaska.contains(p.forward(v(-147.7, 64.8)).unwrap()), "Fairbanks");
    assert!(hawaii.contains(p.forward(v(-155.5, 19.6)).unwrap()), "Big Island");
    assert!(hawaii.contains(p.forward(v(-157.86, 21.3)).unwrap()));
    // The whole country: every ring sits in exactly one inset, and all three are used.
    let usa = project(&country("USA"), &p, 0.5);
    let (mut in48, mut inak, mut inhi) = (0, 0, 0);
    for r in &usa.rings {
        let inside = |b: &Rect| r.iter().all(|q| b.inset(-1e-6).contains(*q));
        match (inside(&lower48), inside(&alaska), inside(&hawaii)) {
            (true, false, false) => in48 += 1,
            (_, true, false) => inak += 1,
            (_, false, true) => inhi += 1,
            other => panic!("ring outside the insets: {other:?}"),
        }
    }
    assert!(in48 > 0 && inak > 0 && inhi > 0, "{in48} {inak} {inhi}");
    assert!(usa.area() > 0.0);
}

#[test]
fn antimeridian_lines_and_polygons_are_cut() {
    let fc = parse_geojson(ANTI, &GeoJsonOptions::default()).unwrap();
    let route = &fc.get("route").unwrap().geometry;
    let boxg = &fc.get("box").unwrap().geometry;
    for p in [Projection::equirectangular(), Projection::mercator(), Projection::natural_earth1(), Projection::equal_earth()] {
        let right = p.forward(v(180.0, 0.0)).unwrap().x;
        let left = p.forward(v(-180.0, 0.0)).unwrap().x;
        let out = project(route, &p, 0.5);
        assert_eq!(out.lines.len(), 2, "{:?}: the Pacific route splits in two", p.kind);
        let a_end = *out.lines[0].last().unwrap();
        let b_start = out.lines[1][0];
        // Pseudo-cylindrical edges curve, so compare against the edge at the crossing latitude.
        let lat = p.inverse(a_end).unwrap().y;
        assert!(lat > 35.0, "the great circle goes north of both cities: {lat}");
        assert!((a_end.x - p.forward(v(180.0, lat)).unwrap().x).abs() < 1e-6);
        assert!((b_start.x - p.forward(v(-180.0, lat)).unwrap().x).abs() < 1e-6);
        assert!(out.lines.iter().flatten().all(|q| q.x >= left - 1e-6 && q.x <= right + 1e-6));
        let poly = project(boxg, &p, 0.5);
        assert_eq!(poly.rings.len(), 2, "{:?}", p.kind);
        let mid = (left + right) / 2.0;
        for r in &poly.rings {
            assert!(r.iter().all(|q| q.x > mid) || r.iter().all(|q| q.x < mid), "each piece stays on its side");
            assert_eq!(r.first(), r.last());
        }
        assert!(poly.area() > 0.0);
    }
    // In equirectangular the two halves add up to the 20°×10° box (edges are great circles, so
    // allow a little bulge).
    let p = Projection::equirectangular();
    let deg = p.scale * m::PI / 180.0;
    let a = project(boxg, &p, 0.1).area();
    assert!((a / (20.0 * 10.0 * deg * deg) - 1.0).abs() < 0.02, "{}", a / (200.0 * deg * deg));
    // Fiji, split at 180° in the data, projects without streaks across the map.
    let fiji = project(&country("FJI"), &Projection::equal_earth(), 0.5);
    let width = Projection::equal_earth().forward(v(180.0, 0.0)).unwrap().x - 480.0;
    for r in &fiji.rings {
        let b = Rect::from_points(r[0], r[0]);
        let b = r.iter().fold(b, |b, q| b.include(*q));
        assert!(b.w < width / 4.0, "no ring spans the map: {}", b.w);
    }
}

#[test]
fn orthographic_clips_the_back_hemisphere() {
    let p = Projection::orthographic(0.0, 0.0);
    let c = p.translate;
    let r = p.scale;
    assert_eq!(p.forward(v(0.0, 0.0)), Some(c));
    assert_eq!(p.forward(v(180.0, 0.0)), None);
    assert_eq!(p.forward(v(100.0, 0.0)), None);
    assert!(p.forward(v(80.0, 0.0)).is_some());
    let back = Geometry::Polygon(vec![vec![v(150.0, -10.0), v(-150.0, -10.0), v(-150.0, 10.0), v(150.0, 10.0), v(150.0, -10.0)]]);
    assert!(project(&back, &p, 0.5).is_empty(), "entirely on the far side");
    let straddle = Geometry::Polygon(vec![vec![v(60.0, -20.0), v(120.0, -20.0), v(120.0, 20.0), v(60.0, 20.0), v(60.0, -20.0)]]);
    let out = project(&straddle, &p, 0.5);
    assert_eq!(out.rings.len(), 1);
    let dists: Vec<f64> = out.rings[0].iter().map(|q| q.dist(c)).collect();
    assert!(dists.iter().all(|&d| d <= r + 1e-6), "inside the disc");
    assert!(dists.iter().filter(|&&d| (d - r).abs() < 1e-6).count() >= 3, "follows the horizon");
    assert!(out.area() > 0.0);
    // The equator all the way round is cut at both horizons, leaving the front half.
    let meridian = Geometry::LineString((0..=36).map(|i| v(-180.0 + 10.0 * i as f64, 0.0)).collect());
    let lines = project(&meridian, &p, 0.5).lines;
    assert_eq!(lines.len(), 1);
    assert!((lines[0][0].dist(c) - r).abs() < 1e-6 && (lines[0].last().unwrap().dist(c) - r).abs() < 1e-6);
    assert!(lines.iter().flatten().all(|q| q.dist(c) <= r + 1e-6));
    // The world from above Stockholm: Antarctica and New Zealand are hidden, Sweden is whole.
    let above = Projection::orthographic(18.0, 59.0);
    assert!(project(&country("ATA"), &above, 0.5).is_empty());
    assert!(project(&country("NZL"), &above, 0.5).is_empty());
    let swe = project(&country("SWE"), &above, 0.5);
    let Geometry::MultiPolygon(parts) = country("SWE") else { panic!() };
    assert_eq!(swe.rings.len(), parts.iter().map(|p| p.len()).sum::<usize>(), "nothing cut");
    // The USA seen from Stockholm is partly over the horizon: clipped, yet still inside the disc.
    let usa = project(&country("USA"), &above, 0.5);
    assert!(!usa.is_empty());
    assert!(usa.rings.iter().flatten().all(|q| q.dist(above.translate) <= above.scale + 1e-6));
}

#[test]
fn resampling_curves_long_edges() {
    // A parallel under a conic is an arc: a 2-point edge along 40°N gets subdivided; the great
    // circle between the ends bows toward the pole compared to the chord.
    let albers = Projection::albers();
    let edge = Geometry::LineString(vec![v(-120.0, 40.0), v(-75.0, 40.0)]);
    let pts = &project(&edge, &albers, 0.5).lines[0];
    assert!(pts.len() > 4, "{}", pts.len());
    // Tolerance controls density; 0 turns resampling off.
    assert_eq!(project(&edge, &albers, 0.0).lines[0].len(), 2);
    assert!(project(&edge, &albers, 0.05).lines[0].len() > pts.len());
    // Polygon rings are resampled too (a big triangle on Equal Earth).
    let tri = Geometry::Polygon(vec![vec![v(-60.0, -30.0), v(60.0, -30.0), v(0.0, 60.0), v(-60.0, -30.0)]]);
    let out = project(&tri, &Projection::equal_earth(), 0.5);
    assert!(out.rings[0].len() > 20);
    // Resampled output is deterministic.
    assert_eq!(out, project(&tri, &Projection::equal_earth(), 0.5));
}

#[test]
fn fit_extent_fills_the_target() {
    let check = |p: &Projection, g: &Geometry, target: Rect| {
        let b = project(g, p, 0.5).bounds().unwrap();
        assert!(b.x >= target.x - 1e-6 && b.y >= target.y - 1e-6 && b.x1() <= target.x1() + 1e-6 && b.y1() <= target.y1() + 1e-6, "{b:?} in {target:?}");
        let fills_w = (b.w - target.w).abs() < 0.5;
        let fills_h = (b.h - target.h).abs() < 0.5;
        assert!(fills_w || fills_h, "{b:?} fills {target:?}");
        // Centred.
        assert!((b.center() - target.center()).len() < 0.5, "{b:?} centred in {target:?}");
    };
    let usa = country("USA");
    let target = Rect::new(0.0, 0.0, 960.0, 600.0);
    let p = Projection::albers_usa().fit_extent(Fit::Geometry(&usa), target);
    check(&p, &usa, target);
    let world = ne();
    let t2 = Rect::new(10.0, 20.0, 400.0, 300.0);
    let p2 = Projection::equal_earth().fit_extent(Fit::Features(&world), t2);
    check(&p2, &world.to_geometry(), t2);
    let swe = country("SWE");
    let p3 = Projection::mercator().fit_size(Fit::Geometry(&swe), 300.0, 600.0);
    check(&p3, &swe, Rect::new(0.0, 0.0, 300.0, 600.0));
    // A bbox (densified so conic curvature counts).
    let bb = GeoBbox::new(-10.0, 35.0, 30.0, 70.0);
    let p4 = Projection::lambert_conformal_conic([40.0, 60.0]).with_rotate([-10.0, 0.0, 0.0]).fit_extent(Fit::Bbox(bb), target);
    check(&p4, &bb.to_polygon(1.0), target);
    // Planar floor plan: 100×50 into 200×200 → scale 2.
    let plan = Geometry::Polygon(vec![vec![v(0.0, 0.0), v(100.0, 0.0), v(100.0, 50.0), v(0.0, 50.0), v(0.0, 0.0)]]);
    let p5 = Projection::planar().fit_size(Fit::Geometry(&plan), 200.0, 200.0);
    assert!((p5.scale - 2.0).abs() < 1e-12);
    assert_eq!(p5.forward(v(0.0, 0.0)), Some(v(0.0, 50.0)));
    // A single point is centred at the current scale.
    let pt = Geometry::Point(v(18.0, 59.0));
    let p6 = Projection::mercator().fit_extent(Fit::Geometry(&pt), target);
    assert!((p6.forward(v(18.0, 59.0)).unwrap() - target.center()).len() < 1e-9);
    assert_eq!(p6.scale, Projection::mercator().scale);
}

/// Meridian arc length from the equator to `lat` (degrees) by Simpson integration.
fn meridian_arc(a: f64, f: f64, lat: f64) -> f64 {
    let e2 = f * (2.0 - f);
    let phi = lat.to_radians();
    let n = 2000;
    let h = phi / n as f64;
    let g = |x: f64| {
        let s = m::sin(x);
        a * (1.0 - e2) / m::pow(1.0 - e2 * s * s, 1.5)
    };
    let mut sum = g(0.0) + g(phi);
    for i in 1..n {
        sum += g(i as f64 * h) * if i % 2 == 1 { 4.0 } else { 2.0 };
    }
    sum * h / 3.0
}

/// Snyder's transverse Mercator series (USGS PP 1395, eq. 8-9/8-10): an independent reference,
/// accurate to millimetres within a few degrees of the central meridian.
fn snyder(tm: &TransverseMercator, ll: Vec2) -> Vec2 {
    let (a, f, k0) = (tm.semi_major, tm.flattening, tm.scale_factor);
    let e2 = f * (2.0 - f);
    let ep2 = e2 / (1.0 - e2);
    let (phi, dl) = (ll.y.to_radians(), (ll.x - tm.central_meridian).to_radians());
    let (e4, e6) = (e2 * e2, e2 * e2 * e2);
    let mm = a * ((1.0 - e2 / 4.0 - 3.0 * e4 / 64.0 - 5.0 * e6 / 256.0) * phi - (3.0 * e2 / 8.0 + 3.0 * e4 / 32.0 + 45.0 * e6 / 1024.0) * m::sin(2.0 * phi)
        + (15.0 * e4 / 256.0 + 45.0 * e6 / 1024.0) * m::sin(4.0 * phi)
        - (35.0 * e6 / 3072.0) * m::sin(6.0 * phi));
    let s = m::sin(phi);
    let n = a / (1.0 - e2 * s * s).sqrt();
    let t = m::tan(phi) * m::tan(phi);
    let c = ep2 * m::cos(phi) * m::cos(phi);
    let aa = m::cos(phi) * dl;
    let x = k0 * n * (aa + (1.0 - t + c) * aa * aa * aa / 6.0 + (5.0 - 18.0 * t + t * t + 72.0 * c - 58.0 * ep2) * m::pow(aa, 5.0) / 120.0);
    let y = k0
        * (mm + n * m::tan(phi) * (aa * aa / 2.0 + (5.0 - t + 9.0 * c + 4.0 * c * c) * m::pow(aa, 4.0) / 24.0 + (61.0 - 58.0 * t + t * t + 600.0 * c - 330.0 * ep2) * m::pow(aa, 6.0) / 720.0));
    Vec2::new(tm.false_easting + x, tm.false_northing + y)
}

#[test]
fn transverse_mercator_grids() {
    let sw = TransverseMercator::sweref99tm();
    // On the central meridian: easting 500 000, northing = k0 · meridian arc.
    for lat in [0.0, 30.0, 55.0, 59.3293, 69.0, 80.0] {
        let en = sw.grid_forward(v(15.0, lat));
        assert!((en.x - 500_000.0).abs() < 1e-6);
        let want = 0.9996 * meridian_arc(sw.semi_major, sw.flattening, lat);
        assert!((en.y - want).abs() < 0.001, "lat {lat}: {} vs {want}", en.y);
    }
    // Symmetric about the central meridian.
    let e1 = sw.grid_forward(v(18.0, 60.0));
    let e2 = sw.grid_forward(v(12.0, 60.0));
    assert!(((e1.x - 500_000.0) + (e2.x - 500_000.0)).abs() < 1e-6 && (e1.y - e2.y).abs() < 1e-6);
    // Agrees with Snyder's independent series across Sweden (within 3° of the meridian).
    for q in grid((12.0, 18.0), (55.0, 69.0), 6) {
        let (a, b) = (sw.grid_forward(q), snyder(&sw, q));
        assert!((a - b).len() < 0.01, "{q:?}: {a:?} vs {b:?}");
    }
    // Stockholm lands where SWEREF 99 TM puts it (≈ 674 km E, 6 580 km N).
    let sthlm = sw.grid_forward(v(18.0686, 59.3293));
    assert!((sthlm.x - 674_000.0).abs() < 2_000.0 && (sthlm.y - 6_580_000.0).abs() < 2_000.0, "{sthlm:?}");
    // UTM, both hemispheres, against Snyder.
    let z56s = TransverseMercator::utm(56, false);
    assert_eq!(z56s.central_meridian, 153.0);
    for q in grid((150.0, 156.0), (-40.0, -10.0), 4) {
        assert!((z56s.grid_forward(q) - snyder(&z56s, q)).len() < 0.01);
    }
    let z32n = TransverseMercator::utm(32, true);
    assert!((z32n.grid_forward(v(9.0, 0.0)) - v(500_000.0, 0.0)).len() < 1e-6);
    // Projection presets: projected space is (easting, −northing).
    let p = Projection::sweref99tm();
    let xy = p.forward(v(18.0686, 59.3293)).unwrap();
    assert!((xy - v(sthlm.x, -sthlm.y)).len() < 1e-6);
    assert!(matches!(p.kind, ProjectionKind::TransverseMercator(_)));
    let back = sw.grid_inverse(sthlm);
    assert!((back - v(18.0686, 59.3293)).len() < 1e-9);
}

#[test]
fn projector_matches_projection_and_hashes_are_stable() {
    use datars_math::StableHash;
    for p in [Projection::albers_usa(), Projection::orthographic(18.0, 59.0), Projection::sweref99tm(), Projection::equal_earth()] {
        let pr = p.projector();
        for q in grid((-170.0, 170.0), (-70.0, 70.0), 10).into_iter().chain([v(-149.9, 61.2), v(-157.86, 21.3), v(15.0, 60.0)]) {
            assert_eq!(pr.forward(q), p.forward(q));
            if let Some(xy) = pr.forward(q) {
                assert_eq!(pr.inverse(xy), p.inverse(xy));
            }
        }
    }
    let a = Projection::equal_earth().with_rotate([10.0, 0.0, 0.0]);
    assert_eq!(a.hash64(), a.clone().hash64());
    assert_ne!(a.hash64(), Projection::equal_earth().hash64());
}

#[test]
fn projections_serialize() {
    for p in [Projection::albers_usa(), Projection::orthographic(10.0, 20.0).with_clip_extent(Some(Rect::new(0.0, 0.0, 10.0, 10.0))), Projection::sweref99tm(), Projection::planar()] {
        let json = serde_json::to_string(&p).unwrap();
        let back: Projection = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p, "{json}");
    }
}

#[test]
fn polygons_larger_than_a_hemisphere_and_the_sphere_outline() {
    // A 240°-wide box covers most of the visible globe (read as drawn, not as its complement),
    // whichever way it winds.
    let mut ring = Vec::new();
    ring.extend((0..=24).map(|i| v(-120.0 + 10.0 * i as f64, -70.0)));
    ring.extend((0..=14).map(|i| v(120.0, -70.0 + 10.0 * i as f64)));
    ring.extend((0..=24).map(|i| v(120.0 - 10.0 * i as f64, 70.0)));
    ring.extend((0..=14).map(|i| v(-120.0, 70.0 - 10.0 * i as f64)));
    let p = Projection::orthographic(0.0, 0.0);
    let disc = m::PI * p.scale * p.scale;
    for r in [ring.clone(), ring.iter().rev().copied().collect()] {
        let out = project(&Geometry::Polygon(vec![r.clone()]), &p, 0.5);
        let ratio = out.area() / disc;
        assert!(ratio > 0.85 && ratio < 1.0, "{ratio}");
        let want = datars_geo::measure::area(&Geometry::Polygon(vec![r]));
        assert!(want > 2.0 * m::PI, "larger than a hemisphere: {want}");
    }
    // The sphere outline: the full disc on a globe, the whole map on Equal Earth.
    let s = datars_geo::sphere(&p, 0.5);
    assert_eq!(s.rings.len(), 1);
    // The horizon is stepped every 6° (within 0.5 px of the circle at this scale), as in d3.
    assert!((s.area() / disc - 1.0).abs() < 3e-3, "{}", s.area() / disc);
    let ee = Projection::equal_earth();
    let world = datars_geo::sphere(&ee, 0.5);
    assert!((world.area() / (4.0 * m::PI * ee.scale * ee.scale) - 1.0).abs() < 1e-3, "equal-area world");
    let b = world.bounds().unwrap();
    assert!((b.x - ee.forward(v(-180.0, 0.0)).unwrap().x).abs() < 1e-6 && (b.y - ee.forward(v(0.0, 90.0)).unwrap().y).abs() < 1e-6);
    assert_eq!(datars_geo::sphere(&Projection::albers_usa(), 0.5).rings.len(), 3, "one frame per inset");
    assert!(datars_geo::sphere(&Projection::planar(), 0.5).is_empty());
}

#[test]
fn poles_whole_world_and_clip_extent() {
    let world = ne();
    // Antarctica on Equal Earth: a thin band across the whole bottom, not its complement.
    let ee = Projection::equal_earth();
    let ata = project(&country("ATA"), &ee, 0.5);
    let lons: Vec<f64> = ata.rings.iter().flatten().filter_map(|q| ee.inverse(*q)).map(|p| p.x).collect();
    let (w, e) = lons.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &x| (a.min(x), b.max(x)));
    assert!(w < -179.99 && e > 179.99, "reaches both map edges: {w} {e}");
    let b = ata.bounds().unwrap();
    assert!(b.y > ee.forward(v(0.0, -55.0)).unwrap().y, "stays at the bottom: {b:?}");
    assert!((b.y1() - ee.forward(v(0.0, -90.0)).unwrap().y).abs() < 1e-6, "down to the pole line: {b:?}");
    // Equal Earth is equal-area: projected px² = steradians · scale².
    let want = datars_geo::measure::area(&country("ATA")) * ee.scale * ee.scale;
    assert!((ata.area() / want - 1.0).abs() < 0.01, "{} vs {want}", ata.area());
    for id in ["SWE", "USA", "FJI", "ZAF"] {
        let (got, want) = (project(&country(id), &ee, 0.1).area(), datars_geo::measure::area(&country(id)) * ee.scale * ee.scale);
        assert!((got / want - 1.0).abs() < 0.01, "{id}: {got} vs {want}");
    }
    // Seen from below the South Pole, Antarctica is whole and centred.
    let south = Projection::orthographic(0.0, -90.0);
    let a = project(&country("ATA"), &south, 0.5);
    assert!(a.area() > 0.0 && a.bounds().unwrap().center().dist(south.translate) < 30.0, "{:?}", a.bounds());
    // The whole subset on a globe: nothing escapes the disc.
    let globe = Projection::orthographic(-100.0, 30.0);
    let all = project(&world.to_geometry(), &globe, 0.5);
    assert!(all.rings.iter().flatten().all(|q| q.dist(globe.translate) <= globe.scale + 1e-6));
    assert!(all.area() > 0.0);
    // clip_extent trims in projected space; polygons stay closed and inside.
    let ext = Rect::new(200.0, 90.0, 150.0, 120.0); // across the US east coast and Canadian border
    let clipped = project(&world.to_geometry(), &Projection::mercator().with_clip_extent(Some(ext)), 0.5);
    assert!(!clipped.rings.is_empty());
    for r in &clipped.rings {
        assert_eq!(r.first(), r.last());
        assert!(r.iter().all(|q| ext.inset(-1e-6).contains(*q)));
    }
    let full = project(&world.to_geometry(), &Projection::mercator(), 0.5);
    assert!(clipped.area() > 0.0 && clipped.area() < full.area());
    // Mercator clamps the poles instead of running to infinity.
    let merc_ata = project(&country("ATA"), &Projection::mercator(), 0.5);
    assert!(merc_ata.bounds().unwrap().y1().is_finite());
}

/// A globe fitted to the sphere keeps its disc as it turns: the same scale and centre at every
/// rotation, the disc filling the extent's shorter side.
#[test]
fn a_sphere_fit_is_the_same_disc_at_every_rotation() {
    let extent = datars_math::Rect::new(10.0, 20.0, 400.0, 300.0);
    let fits: Vec<_> = [(15.0, 15.0), (-120.0, 15.0), (80.0, -40.0)]
        .iter()
        .map(|&(lon, lat)| datars_geo::Projection::orthographic(lon, lat).fit_extent(datars_geo::Fit::Sphere, extent))
        .collect();
    for f in &fits {
        assert!((f.scale - 150.0).abs() < 0.01, "radius = half the shorter side: {}", f.scale);
        assert!((f.translate.x - 210.0).abs() < 0.01 && (f.translate.y - 170.0).abs() < 0.01, "{:?}", f.translate);
    }
    // Without a horizon, the sphere is the world's outline.
    let ee = datars_geo::Projection::equal_earth();
    assert_eq!(ee.fit_extent(datars_geo::Fit::Sphere, extent).scale, ee.fit_extent(datars_geo::Fit::Bbox(datars_geo::GeoBbox::new(-180.0, -90.0, 180.0, 90.0)), extent).scale);
}
