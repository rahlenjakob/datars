//! Timings for the heavy algorithms. Ignored by default; run in release:
//!
//! ```sh
//! cargo test -p datars-algo --release --test perf -- --ignored --nocapture --test-threads=1
//! ```

use datars_algo::*;
use datars_math::{m, Rect, Rng, Vec2};
use std::time::Instant;

fn time<T>(label: &str, f: impl FnOnce() -> T) -> T {
    let t = Instant::now();
    let out = f();
    eprintln!("{label:<48} {:>9.2} ms", t.elapsed().as_secs_f64() * 1000.0);
    out
}

fn bell(rng: &mut Rng) -> f64 {
    (0..6).map(|_| rng.next_f64()).sum::<f64>() / 6.0
}

fn blob(n: usize, r: f64) -> Vec<Vec2> {
    (0..n)
        .map(|i| {
            let a = i as f64 / n as f64 * m::TAU;
            let rr = r * (1.0 + 0.25 * m::sin(5.0 * a));
            Vec2::new(500.0 + rr * m::cos(a), 500.0 + rr * m::sin(a))
        })
        .collect()
}

#[test]
#[ignore]
fn perf_beeswarm_20k() {
    let mut rng = Rng::new(1);
    let xs: Vec<f64> = (0..20_000).map(|_| bell(&mut rng) * 1000.0).collect();
    let ys = time("beeswarm 20k dots, r = 2", || beeswarm(&xs, 2.0, 0.0, SwarmSide::Both));
    assert_eq!(ys.len(), 20_000);
    let ys = time("beeswarm 20k dots, r = 4 (denser)", || beeswarm(&xs, 4.0, 0.0, SwarmSide::Both));
    assert_eq!(ys.len(), 20_000);
    let xs: Vec<f64> = (0..100_000).map(|_| bell(&mut rng) * 2000.0).collect();
    time("beeswarm 100k dots, r = 1.5", || beeswarm(&xs, 1.5, 0.0, SwarmSide::Both));
}

#[test]
#[ignore]
fn perf_delaunay_voronoi() {
    let mut rng = Rng::new(2);
    let pts: Vec<Vec2> = (0..100_000).map(|_| Vec2::new(rng.range(0.0, 1000.0), rng.range(0.0, 1000.0))).collect();
    let d = time("delaunay 100k random points", || delaunay(&pts));
    assert!(d.triangles.len() > 190_000);
    let grid: Vec<Vec2> = (0..300).flat_map(|y| (0..300).map(move |x| Vec2::new(x as f64, y as f64))).collect();
    let g = time("delaunay 300×300 grid (all cocircular)", || delaunay(&grid));
    assert_eq!(g.triangles.len(), 2 * 299 * 299);
    let cells = time("voronoi 100k random points", || voronoi(&pts, Rect::new(0.0, 0.0, 1000.0, 1000.0)));
    assert_eq!(cells.len(), pts.len());
}

#[test]
#[ignore]
fn perf_scatter_in() {
    let outer = blob(2000, 400.0);
    let hole: Vec<Vec2> = blob(500, 120.0);
    let rings = vec![outer, hole];
    let p = time("scatter_in 10k points, 2500-vertex polygon + hole", || scatter_in(&rings, 10_000, 7));
    assert_eq!(p.len(), 10_000);
    let p = time("scatter_in 100k points, same polygon", || scatter_in(&rings, 100_000, 7));
    assert_eq!(p.len(), 100_000);
    // A dot-density map: 400k dots over 200 regions of various sizes.
    let mut rng = Rng::new(3);
    let regions: Vec<(Vec<Vec<Vec2>>, usize)> = (0..200)
        .map(|k| {
            let r = rng.range(20.0, 120.0);
            let ring: Vec<Vec2> = (0..300).map(|i| {
                let a = i as f64 / 300.0 * m::TAU;
                Vec2::new(k as f64 * 300.0 + r * m::cos(a), r * m::sin(a) * rng.range(0.9, 1.1))
            }).collect();
            (vec![ring], 2000)
        })
        .collect();
    let total = time("dot density: 200 regions × 2000 dots", || regions.iter().map(|(r, n)| scatter_in(r, *n, 1).len()).sum::<usize>());
    assert_eq!(total, 400_000);
}

#[test]
#[ignore]
fn perf_force() {
    let n = 1000;
    let mut rng = Rng::new(4);
    let links: Vec<(usize, usize, f64)> = (1..n).map(|i| (i, rng.below(i as u64) as usize, 1.0)).collect();
    time("force 1000 nodes, 300 iterations (Barnes–Hut)", || force(n, &links, &ForceOptions::default()));
    time("force 1000 nodes, 300 iterations, collide", || force(n, &links, &ForceOptions { collide_radius: 4.0, ..Default::default() }));
    time("force 1000 nodes, 300 iterations (exact)", || force(n, &links, &ForceOptions { theta: 0.0, ..Default::default() }));
}

#[test]
#[ignore]
fn perf_grids() {
    let n = 500;
    let field: Vec<f64> = (0..n * n)
        .map(|k| {
            let (x, y) = ((k % n) as f64 / 50.0, (k / n) as f64 / 50.0);
            m::sin(x) * m::cos(y) + 0.3 * m::sin(3.0 * x + y)
        })
        .collect();
    let th: Vec<f64> = (0..10).map(|i| -1.0 + i as f64 * 0.2).collect();
    let c = time("contour 500×500, 10 thresholds", || contour(&field, n, n, &th));
    assert_eq!(c.len(), 10);
    let mut rng = Rng::new(5);
    let pts: Vec<Vec2> = (0..100_000).map(|_| Vec2::new(bell(&mut rng) * 1000.0, bell(&mut rng) * 1000.0)).collect();
    let g = time("density 100k points, 400×400 grid, bw = 15", || density(&pts, None, 15.0, 400, 400, Rect::new(0.0, 0.0, 1000.0, 1000.0)));
    assert_eq!(g.len(), 160_000);
    let bins = time("hexbin 100k points, r = 10", || hexbin(&pts, 10.0, Rect::new(0.0, 0.0, 1000.0, 1000.0)));
    assert!(!bins.is_empty());
}

#[test]
#[ignore]
fn perf_hierarchies_and_labels() {
    let mut rng = Rng::new(6);
    let values: Vec<f64> = (0..100_000).map(|_| rng.range(1.0, 100.0)).collect();
    time("treemap 100k values", || treemap(&values, Rect::new(0.0, 0.0, 1920.0, 1080.0), GOLDEN));
    let parents: Vec<Option<usize>> = (0..100_000).map(|i| if i == 0 { None } else { Some(rng.below(i as u64) as usize) }).collect();
    time("treemap_nested 100k nodes", || treemap_nested(&parents, &values, Rect::new(0.0, 0.0, 1920.0, 1080.0), &TreemapOptions::default()));
    time("tree (tidy) 100k nodes", || tree(&parents, Rect::new(0.0, 0.0, 1920.0, 1080.0), &TreeOptions::default()));
    time("partition 100k nodes", || partition(&parents, &values, Rect::new(0.0, 0.0, 1920.0, 1080.0), 0.0));
    let radii: Vec<f64> = (0..10_000).map(|_| rng.range(1.0, 10.0)).collect();
    time("pack 10k circles", || pack(&radii));
    let small: Vec<Option<usize>> = parents[..10_000].to_vec();
    time("pack_hierarchy 10k nodes, padding 2", || pack_hierarchy(&small, &values[..10_000], Rect::new(0.0, 0.0, 1000.0, 1000.0), 2.0));
    let links: Vec<(usize, usize, f64)> = (0..2000)
        .map(|_| {
            let a = rng.below(190) as usize;
            (a, a + 1 + rng.below(10) as usize, rng.range(1.0, 50.0))
        })
        .collect();
    time("sankey 200 nodes, 2000 links, 6 iterations", || sankey(200, &links, Rect::new(0.0, 0.0, 1200.0, 800.0), &SankeyOptions::default()));
    let boxes: Vec<LabelBox> = (0..20_000)
        .map(|i| LabelBox {
            rect: Rect::new(0.0, 0.0, 40.0, 12.0),
            priority: (i % 13) as f64,
            anchor: Vec2::new(rng.range(0.0, 1920.0), rng.range(0.0, 1080.0)),
            candidates: vec![Vec2::new(4.0, -14.0), Vec2::new(4.0, 2.0), Vec2::new(-44.0, -14.0), Vec2::new(-44.0, 2.0)],
        })
        .collect();
    let placed = time("label_layout 20k labels × 4 candidates", || label_layout(&boxes, Rect::new(0.0, 0.0, 1920.0, 1080.0), &[]));
    eprintln!("  ({} of 20000 placed)", placed.iter().filter(|p| p.is_some()).count());
    let seats: Vec<usize> = vec![120, 300, 45, 80, 200, 5];
    time("parliament 750 seats", || parliament(&seats, None, Vec2::new(500.0, 500.0), 150.0, 450.0));
}
