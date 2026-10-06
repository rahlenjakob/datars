//! Cross-module checks through the public API: totality on hostile input, pipelines, and pinned
//! output hashes (P1: the same bits on every target — run this suite on wasm and x86-64 too).

use datars_algo::*;
use datars_math::path::signed_area;
use datars_math::{m, Hash64, Rect, Rng, Vec2};

const NASTY: [f64; 7] = [0.0, -0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e300];

fn all_finite(v: impl IntoIterator<Item = f64>) -> bool {
    v.into_iter().all(|x| x.is_finite())
}

fn rect_ok(r: &Rect) -> bool {
    all_finite([r.x, r.y, r.w, r.h]) && r.w >= 0.0 && r.h >= 0.0
}

#[test]
fn every_function_is_total_on_hostile_input() {
    let nasty: Vec<f64> = NASTY.to_vec();
    let nasty_pts: Vec<Vec2> = NASTY.iter().flat_map(|&x| NASTY.iter().map(move |&y| Vec2::new(x, y))).collect();
    let bad_rect = Rect::new(f64::NAN, f64::INFINITY, -5.0, f64::NAN);
    let good_rect = Rect::new(0.0, 0.0, 100.0, 100.0);

    for (a, b) in [(0.0, m::TAU), (f64::NAN, 1.0), (0.0, f64::INFINITY)] {
        let arcs = pie(&nasty, a, b, f64::NAN, PieSort::Desc);
        assert_eq!(arcs.len(), nasty.len());
        assert!(arcs.iter().all(|x| x.0.is_finite() && x.1.is_finite()));
    }
    let st = stack(&[nasty.clone(), nasty.clone(), vec![]], StackOffset::Expand, StackOrder::InsideOut);
    assert!(st.iter().flatten().all(|s| s.0.is_finite() && s.1.is_finite()));
    for off in [StackOffset::Zero, StackOffset::Silhouette, StackOffset::Wiggle] {
        let st = stack(&[vec![1.0, 2.0], nasty.clone()], off, StackOrder::Ascending);
        assert!(st.iter().flatten().all(|s| s.0.is_finite() && s.1.is_finite()));
    }
    assert!(dodge(&[0, 3, 1], 2, f64::NAN, f64::INFINITY, -1.0).iter().all(|d| d.0.is_finite() && d.1.is_finite()));
    assert!(all_finite(waterfall(&nasty, &[true]).into_iter().flat_map(|x| [x.0, x.1])));
    assert!(funnel(&nasty, f64::NAN, 1.0).iter().all(|s| all_finite([s.x0, s.x1, s.of_previous, s.of_first])));
    assert!(all_finite(pareto(&nasty)));

    for r in [good_rect, bad_rect] {
        assert!(treemap(&nasty, r, f64::NAN).iter().all(rect_ok));
        let parents = [None, Some(0), Some(0), Some(7), Some(2), Some(4), Some(5)];
        assert!(treemap_nested(&parents, &nasty, r, &TreemapOptions { padding_inner: f64::NAN, padding_outer: 1e9, padding_top: -3.0, ratio: 0.0 }).iter().all(rect_ok));
        assert!(partition(&parents, &nasty, r, f64::INFINITY).iter().all(rect_ok));
        assert!(pack_hierarchy(&parents, &nasty, r, f64::NAN).iter().all(|c| c.center.is_finite() && c.r.is_finite()));
        assert!(pack_values(&nasty, r, 3.0).iter().all(|c| c.center.is_finite() && c.r.is_finite()));
        assert!(tree(&parents, r, &TreeOptions { sibling_separation: f64::NAN, cousin_separation: -1.0, node_size: None }).iter().all(|p| p.is_finite()));
        assert!(cluster(&parents, r, &TreeOptions::default()).iter().all(|p| p.is_finite()));
        let links: Vec<(usize, usize, f64)> = nasty.iter().enumerate().map(|(i, &v)| (i % 3, (i + 1) % 4, v)).collect();
        let s = sankey(4, &links, r, &SankeyOptions { node_width: f64::NAN, node_padding: f64::INFINITY, iterations: 3, align: SankeyAlign::Center });
        assert!(s.nodes.iter().all(rect_ok));
        assert!(s.links.iter().all(|l| all_finite([l.x0, l.x1, l.y0, l.y1, l.width])));
        assert!(waffle(&[3, 0, 5], 4, 2, r, WaffleOrder::ColumnMajor).iter().all(|c| rect_ok(&c.1)));
        assert!(voronoi(&nasty_pts, r).iter().flatten().all(|p| p.is_finite()));
        assert!(hexbin(&nasty_pts, 5.0, r).iter().all(|h| h.center.is_finite()));
        assert!(all_finite(density(&nasty_pts, Some(&nasty), f64::NAN, 5, 5, r)));
        assert!(label_layout(&[LabelBox { rect: bad_rect, priority: f64::NAN, anchor: Vec2::ZERO, candidates: nasty_pts.clone() }], r, &[bad_rect]).len() == 1);
    }
    assert!(pack(&nasty).circles.iter().all(|c| c.center.is_finite()));
    assert!(enclose(&[Circle::new(Vec2::new(f64::NAN, 0.0), 1.0)]).r.is_finite());
    assert!(all_finite(beeswarm(&nasty, 2.0, f64::NAN, SwarmSide::Both)));
    let p = parliament(&[3, 0, 2], Some(0), Vec2::new(f64::NAN, 0.0), f64::NAN, f64::INFINITY);
    assert!(p.seats.iter().all(|s| s.1.is_finite()));
    assert_eq!(apportion(&nasty, 10).iter().sum::<usize>(), 10);
    assert_eq!(calendar(&[i32::MIN, 0, i32::MAX], f64::NAN, WeekStart::Sunday).len(), 3);
    assert!(calendar_month_outline(i32::MAX, 99, 1.0, WeekStart::Monday).iter().all(|p| p.is_finite()));
    assert!(force(10, &[(0, 1, f64::NAN), (3, 99, 1.0)], &ForceOptions { charge: f64::NAN, link_distance: f64::INFINITY, collide_radius: f64::NAN, theta: f64::NAN, velocity_decay: f64::NAN, center: Vec2::new(f64::NAN, 0.0), ..Default::default() })
        .iter()
        .all(|p| p.is_finite()));
    let d = delaunay(&nasty_pts);
    assert!(d.triangles.iter().flatten().all(|&i| nasty_pts[i].is_finite()));
    assert!(contour(&nasty, 3, 3, &nasty).iter().flat_map(|c| c.1.iter().flatten()).all(|p| p.is_finite()));
    assert!(scatter_in(std::slice::from_ref(&nasty_pts), 5, 1).iter().all(|p| p.is_finite()));
    assert!(polylabel(std::slice::from_ref(&nasty_pts), f64::NAN).0.is_finite());
    for (a, b) in [(f64::NAN, 1.0), (0.0, f64::INFINITY), (1e300, -1e300), (-1e-300, 1e-300)] {
        assert!(all_finite(nice_ticks(a, b, 10)));
        let _ = nice_domain(a, b, 10);
        assert!(tick_step(a, b, 3).is_finite());
    }
    assert!(nice_ticks(0.0, 1.0, usize::MAX).len() <= 1_000_001);
    assert!(time_ticks(i32::MIN, i32::MAX, 10).len() < 100);
    assert!(time_ticks(-5, 5, usize::MAX).len() <= 11);
}

#[test]
fn density_contours_label_pipeline() {
    // Two clusters → KDE → contours → polygons → a label point inside each blob.
    let mut rng = Rng::new(21);
    let mut pts = Vec::new();
    for c in [Vec2::new(30.0, 30.0), Vec2::new(70.0, 65.0)] {
        for _ in 0..400 {
            let (a, r) = (rng.range(0.0, m::TAU), 8.0 * rng.next_f64().sqrt());
            pts.push(c + Vec2::new(r * m::cos(a), r * m::sin(a)));
        }
    }
    let (w, h) = (100, 100);
    let grid = density(&pts, None, 3.0, w, h, Rect::new(0.0, 0.0, 100.0, 100.0));
    let peak = grid.iter().copied().fold(0.0, f64::max);
    let bands = contour(&grid, w, h, &[peak * 0.2]);
    let polys = group_rings(bands[0].1.clone());
    assert_eq!(polys.len(), 2, "two blobs");
    for poly in &polys {
        assert!(signed_area(&poly[0]) > 0.0);
        let (p, d) = polylabel(poly, 0.1);
        assert!(d > 3.0);
        let near = [Vec2::new(30.5, 30.5), Vec2::new(70.5, 65.5)].iter().map(|c| c.dist(p)).fold(f64::INFINITY, f64::min);
        assert!(near < 3.0, "label near a cluster centre: {p:?}");
    }
}

#[test]
fn unit_chart_fills_a_glyph_with_holes() {
    // An "O" (outer and inner ellipse) plus a separate bar: the units must avoid the counter.
    let ellipse = |rx: f64, ry: f64| -> Vec<Vec2> {
        (0..120).map(|i| {
            let a = i as f64 / 120.0 * m::TAU;
            Vec2::new(50.0 + rx * m::cos(a), 50.0 + ry * m::sin(a))
        }).collect()
    };
    let bar = vec![Vec2::new(100.0, 10.0), Vec2::new(115.0, 10.0), Vec2::new(115.0, 90.0), Vec2::new(100.0, 90.0)];
    let rings = vec![ellipse(40.0, 45.0), ellipse(22.0, 28.0), bar];
    let units = scatter_in(&rings, 600, 99);
    assert_eq!(units.len(), 600);
    assert!(units.iter().all(|p| polygon::contains(&rings, *p)));
    assert!(units.iter().any(|p| p.x > 100.0), "the bar gets its share");
    assert_eq!(scatter_in(&rings, 600, 99), units);
}

fn hash_f64s(h: &mut Hash64, v: impl IntoIterator<Item = f64>) {
    for x in v {
        h.f64(x);
    }
}

/// Pinned output hashes. A mismatch means results changed: intended (update the pin) or a
/// determinism bug (a platform-dependent operation crept in). Set DATARS_PRINT_HASHES=1 to print.
#[test]
fn pinned_output_hashes() {
    let mut rng = Rng::new(2024);
    let values: Vec<f64> = (0..64).map(|_| rng.range(0.0, 100.0)).collect();
    let pts: Vec<Vec2> = (0..300).map(|_| Vec2::new(rng.range(0.0, 500.0), rng.range(0.0, 500.0))).collect();
    let parents: Vec<Option<usize>> = (0..64).map(|i| if i == 0 { None } else { Some(rng.below(i as u64) as usize) }).collect();
    let r = Rect::new(0.0, 0.0, 800.0, 600.0);
    let mut out: Vec<(&str, u64)> = Vec::new();
    let mut put = |name: &'static str, f: &dyn Fn(&mut Hash64)| {
        let mut h = Hash64::new();
        f(&mut h);
        out.push((name, h.finish()));
    };
    put("pie", &|h| hash_f64s(h, pie(&values, 0.0, m::TAU, 0.01, PieSort::Desc).into_iter().flat_map(|a| [a.0, a.1])));
    put("stack", &|h| {
        let s: Vec<Vec<f64>> = values.chunks(8).map(|c| c.to_vec()).collect();
        hash_f64s(h, stack(&s, StackOffset::Wiggle, StackOrder::InsideOut).into_iter().flatten().flat_map(|a| [a.0, a.1]))
    });
    put("treemap", &|h| hash_f64s(h, treemap(&values, r, GOLDEN).into_iter().flat_map(|q| [q.x, q.y, q.w, q.h])));
    put("treemap_nested", &|h| hash_f64s(h, treemap_nested(&parents, &values, r, &TreemapOptions { padding_inner: 2.0, ..Default::default() }).into_iter().flat_map(|q| [q.x, q.y, q.w, q.h])));
    put("pack_hierarchy", &|h| hash_f64s(h, pack_hierarchy(&parents, &values, r, 2.0).into_iter().flat_map(|c| [c.center.x, c.center.y, c.r])));
    put("tree", &|h| hash_f64s(h, tree(&parents, r, &TreeOptions::default()).into_iter().flat_map(|p| [p.x, p.y])));
    put("sankey", &|h| {
        let links: Vec<(usize, usize, f64)> = (0..40).map(|i| (i % 10, 10 + i % 7, values[i])).collect();
        let s = sankey(17, &links, r, &SankeyOptions::default());
        hash_f64s(h, s.nodes.iter().flat_map(|q| [q.x, q.y, q.w, q.h]));
        hash_f64s(h, s.links.iter().flat_map(|l| [l.x0, l.x1, l.y0, l.y1, l.width]));
    });
    put("beeswarm", &|h| hash_f64s(h, beeswarm(&values, 3.0, 0.0, SwarmSide::Both)));
    put("parliament", &|h| hash_f64s(h, parliament(&[40, 60, 13, 7], None, Vec2::new(400.0, 400.0), 100.0, 300.0).seats.into_iter().flat_map(|s| [s.0 as f64, s.1.x, s.1.y])));
    put("force", &|h| hash_f64s(h, force(40, &parents[1..40].iter().enumerate().map(|(i, p)| (i + 1, p.unwrap(), 1.0)).collect::<Vec<_>>(), &ForceOptions { iterations: 100, collide_radius: 3.0, ..Default::default() }).into_iter().flat_map(|p| [p.x, p.y])));
    put("delaunay", &|h| {
        for t in delaunay(&pts).triangles {
            h.u64(t[0] as u64);
            h.u64(t[1] as u64);
            h.u64(t[2] as u64);
        }
    });
    put("voronoi", &|h| hash_f64s(h, voronoi(&pts, Rect::new(0.0, 0.0, 500.0, 500.0)).into_iter().flatten().flat_map(|p| [p.x, p.y])));
    put("density+contour", &|h| {
        let g = density(&pts, None, 20.0, 50, 50, Rect::new(0.0, 0.0, 500.0, 500.0));
        hash_f64s(h, g.iter().copied());
        let peak = g.iter().copied().fold(0.0, f64::max);
        for (_, rings) in contour(&g, 50, 50, &[peak * 0.3, peak * 0.6]) {
            hash_f64s(h, rings.into_iter().flatten().flat_map(|p| [p.x, p.y]));
        }
    });
    put("scatter_in", &|h| hash_f64s(h, scatter_in(&[pts[..40].to_vec()], 200, 5).into_iter().flat_map(|p| [p.x, p.y])));
    put("polylabel", &|h| {
        let (p, d) = polylabel(&[vec![Vec2::new(0.0, 0.0), Vec2::new(90.0, 10.0), Vec2::new(60.0, 80.0), Vec2::new(10.0, 60.0)]], 0.01);
        hash_f64s(h, [p.x, p.y, d]);
    });
    put("ticks", &|h| {
        hash_f64s(h, nice_ticks(-0.37, 12.9, 7));
        for (d, u) in time_ticks(-400, 20_000, 9) {
            h.u64(d as u64);
            h.u8(u as u8);
        }
    });
    let pinned: &[(&str, u64)] = &PINNED;
    if std::env::var("DATARS_PRINT_HASHES").is_ok() {
        for (name, v) in &out {
            println!("    (\"{name}\", 0x{v:016x}),");
        }
    }
    for (name, v) in &out {
        let want = pinned.iter().find(|p| p.0 == *name).map(|p| p.1);
        assert_eq!(want, Some(*v), "output hash of {name} changed");
    }
}

/// Produced on aarch64-apple-darwin; must match on every target.
const PINNED: [(&str, u64); 16] = [
    ("pie", 0x4295edc58e07f8ae),
    ("stack", 0x62bfee5fdacd0989),
    ("treemap", 0x2b0cc5f639fc0748),
    ("treemap_nested", 0xdd103846bbb9ae3d),
    ("pack_hierarchy", 0x47c1a6ee96e5dfcc),
    ("tree", 0xd60997db05b9dabb),
    ("sankey", 0xe339752bdcdd6448),
    ("beeswarm", 0x70fa0434acd41811),
    ("parliament", 0x2cc04af66f323a75),
    ("force", 0xee2bdae6a191264a),
    ("delaunay", 0x41a61a62527f9847),
    ("voronoi", 0x271dc30b7a56b392),
    ("density+contour", 0x6a33376a98b4f6a4),
    ("scatter_in", 0x4f5cf97be11eda5a),
    ("polylabel", 0x471558c79f91a028),
    ("ticks", 0xda2aed85acf2ffde),
];
