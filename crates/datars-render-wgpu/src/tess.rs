//! CPU tessellation with `lyon`: fills (non-zero / even-odd), strokes (width, caps, joins, miter
//! limit) and dashes (the path is split into dash segments first, then stroked with caps).
//!
//! Meshes are built once per resource in the path's LOCAL coordinates and reused every frame; the
//! vertex shader applies the transform. Vertices are stored relative to the path's bounds centre
//! (`Mesh::centre`, kept in f64 on the CPU) so f32 keeps its precision when a path lives far from
//! the origin — map geometry in projected world units, say — and the per-draw translation that
//! puts it back is computed in f64.
//!
//! Stroke vertices carry the lyon normal next to the on-path position, so the shader extrudes by
//! the exact half-width of each draw (`position = on_path + normal · half_width`); the mesh itself
//! is shared by every width in its bucket (see `lib.rs`).

use datars_math::{FillRule, PathData, PathEl, Vec2};
use datars_scene::{Cap, Join};
use lyon_tessellation as lt;
use lt::math::point;

/// A tessellated mesh in local coordinates relative to `centre`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    /// Interleaved vertex floats: `[x, y]` for fills, `[x, y, nx, ny]` for strokes.
    pub verts: Vec<f32>,
    pub indices: Vec<u32>,
    /// The local-space point the vertices are relative to.
    pub centre: Vec2,
}

impl Mesh {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }
    /// GPU bytes this mesh occupies (for the cache budget).
    pub fn bytes(&self) -> u64 {
        (self.verts.len() * 4 + self.indices.len() * 4) as u64
    }
}

/// Stroke parameters in the path's local units.
#[derive(Clone, Debug, PartialEq)]
pub struct StrokeSpec {
    pub width: f64,
    pub cap: Cap,
    pub join: Join,
    pub miter_limit: f64,
    pub dash: Option<Vec<f64>>,
}

/// Reusable lyon tessellators (they keep their allocations between meshes).
#[derive(Default)]
pub struct Tessellators {
    fill: lt::FillTessellator,
    stroke: lt::StrokeTessellator,
}

impl Tessellators {
    pub fn new() -> Tessellators {
        Tessellators::default()
    }

    /// Triangulate the interior of `path` under `rule`. lyon's fill output is a set of
    /// non-overlapping triangles, which is what lets clips count coverage in the stencil buffer.
    pub fn fill(&mut self, path: &PathData, rule: FillRule, tolerance: f64) -> Mesh {
        let centre = centre_of(path);
        if let Some(m) = convex_fan(path, centre) {
            return m;
        }
        let Some(lp) = to_lyon(path, centre) else { return Mesh { centre, ..Mesh::default() } };
        let mut buf: lt::VertexBuffers<[f32; 2], u32> = lt::VertexBuffers::new();
        let opts = lt::FillOptions::tolerance(tolerance.max(1e-9) as f32).with_fill_rule(match rule {
            FillRule::NonZero => lt::FillRule::NonZero,
            FillRule::EvenOdd => lt::FillRule::EvenOdd,
        });
        let ok = self
            .fill
            .tessellate_path(&lp, &opts, &mut lt::BuffersBuilder::new(&mut buf, |v: lt::FillVertex| v.position().to_array()))
            .is_ok();
        if !ok {
            return Mesh { centre, ..Mesh::default() };
        }
        Mesh { verts: buf.vertices.iter().flatten().copied().collect(), indices: buf.indices, centre }
    }

    /// Stroke `path` (dashed first when `spec.dash` is set). Vertices are `[x, y, nx, ny]`.
    pub fn stroke(&mut self, path: &PathData, spec: &StrokeSpec, tolerance: f64) -> Mesh {
        let centre = centre_of(path);
        let dashed;
        let src = match &spec.dash {
            Some(d) => match dash_path(path, d, tolerance) {
                Some(p) => {
                    dashed = p;
                    &dashed
                }
                None => path,
            },
            None => path,
        };
        let Some(lp) = to_lyon(src, centre) else { return Mesh { centre, ..Mesh::default() } };
        let cap = match spec.cap {
            Cap::Butt => lt::LineCap::Butt,
            Cap::Round => lt::LineCap::Round,
            Cap::Square => lt::LineCap::Square,
        };
        let join = match spec.join {
            Join::Miter => lt::LineJoin::Miter,
            Join::Round => lt::LineJoin::Round,
            Join::Bevel => lt::LineJoin::Bevel,
        };
        let opts = lt::StrokeOptions::tolerance(tolerance.max(1e-9) as f32)
            .with_line_width(spec.width.max(1e-9) as f32)
            .with_line_cap(cap)
            .with_line_join(join)
            .with_miter_limit(spec.miter_limit.max(1.0) as f32);
        let mut buf: lt::VertexBuffers<[f32; 4], u32> = lt::VertexBuffers::new();
        let ok = self
            .stroke
            .tessellate_path(
                &lp,
                &opts,
                &mut lt::BuffersBuilder::new(&mut buf, |v: lt::StrokeVertex| {
                    let (p, n) = (v.position_on_path(), v.normal());
                    [p.x, p.y, n.x, n.y]
                }),
            )
            .is_ok();
        if !ok {
            return Mesh { centre, ..Mesh::default() };
        }
        Mesh { verts: buf.vertices.iter().flatten().copied().collect(), indices: buf.indices, centre }
    }
}

/// The centre of the path's control-point bounds (the origin its mesh is stored relative to).
/// A single closed polygon of straight edges that turns one way all round (a morph's outline in
/// flight, a rect, a hexagon), fanned from its first vertex in O(n): lyon's sweep costs far more
/// per path, and a transition can morph thousands of them every frame. A fan of a convex polygon
/// doesn't overlap itself, like lyon's output (clips count coverage in the stencil), and either
/// fill rule fills a simple polygon the same. `None` for anything else, which lyon takes.
fn convex_fan(path: &PathData, centre: Vec2) -> Option<Mesh> {
    let last = path.els.len().checked_sub(1)?;
    let mut pts: Vec<Vec2> = Vec::with_capacity(path.els.len());
    for (i, el) in path.els.iter().enumerate() {
        match *el {
            PathEl::Move { p } if i == 0 => pts.push(p),
            PathEl::Line { p } if i > 0 => pts.push(p),
            PathEl::Close if i == last => {}
            _ => return None,
        }
    }
    if !pts.iter().all(|p| p.x.is_finite() && p.y.is_finite()) {
        return None;
    }
    pts.dedup();
    if pts.len() > 1 && pts.first() == pts.last() {
        pts.pop();
    }
    let n = pts.len();
    if n < 3 {
        return None;
    }
    // Every turn the same way (collinear points, a resampled rect's straight sides, don't count)…
    let mut turn = 0.0;
    for i in 0..n {
        let (a, b, c) = (pts[i], pts[(i + 1) % n], pts[(i + 2) % n]);
        let (e1, e2) = (b - a, c - b);
        let cross = e1.x * e2.y - e1.y * e2.x;
        let scale = (e1.x.abs() + e1.y.abs()) * (e2.x.abs() + e2.y.abs());
        if cross.abs() <= 1e-9 * scale {
            continue;
        }
        if turn == 0.0 {
            turn = cross.signum();
        } else if cross.signum() != turn {
            return None;
        }
    }
    if turn == 0.0 {
        return None; // all on one line: nothing to fill
    }
    // …and going round once: the edges' x and y directions each reverse exactly twice (a star
    // turns one way too, but winds more than once).
    let reversals = |d: &dyn Fn(Vec2) -> f64| {
        let signs: Vec<f64> = (0..n).map(|i| d(pts[(i + 1) % n] - pts[i])).filter(|v| *v != 0.0).map(f64::signum).collect();
        (0..signs.len()).filter(|&i| signs[i] != signs[(i + 1) % signs.len()]).count()
    };
    if reversals(&|v| v.x) > 2 || reversals(&|v| v.y) > 2 {
        return None;
    }
    let verts = pts.iter().flat_map(|p| [(p.x - centre.x) as f32, (p.y - centre.y) as f32]).collect();
    let indices = (1..n as u32 - 1).flat_map(|i| [0, i, i + 1]).collect();
    Some(Mesh { verts, indices, centre })
}

pub fn centre_of(path: &PathData) -> Vec2 {
    let b = path.bounds();
    if b.is_empty() {
        Vec2::ZERO
    } else {
        b.center()
    }
}

/// Convert to a lyon path relative to `c`. `None` for empty paths or non-finite coordinates
/// (lyon asserts on NaN; a bad coordinate draws nothing rather than taking the frame down).
fn to_lyon(path: &PathData, c: Vec2) -> Option<lt::path::Path> {
    let finite = path.els.iter().all(|e| match *e {
        PathEl::Move { p } | PathEl::Line { p } => p.is_finite(),
        PathEl::Quad { c, p } => c.is_finite() && p.is_finite(),
        PathEl::Cubic { c1, c2, p } => c1.is_finite() && c2.is_finite() && p.is_finite(),
        PathEl::Close => true,
    });
    if !finite || path.els.is_empty() {
        return None;
    }
    let pt = |p: Vec2| point((p.x - c.x) as f32, (p.y - c.y) as f32);
    let mut b = lt::path::Path::builder();
    let (mut open, mut any) = (false, false);
    let (mut start, mut last) = (Vec2::ZERO, Vec2::ZERO);
    let ensure_open = |b: &mut lt::path::path::Builder, open: &mut bool, last: Vec2| {
        if !*open {
            b.begin(pt(last));
            *open = true;
        }
    };
    for e in &path.els {
        match *e {
            PathEl::Move { p } => {
                if open {
                    b.end(false);
                }
                b.begin(pt(p));
                open = true;
                start = p;
                last = p;
            }
            PathEl::Line { p } => {
                ensure_open(&mut b, &mut open, last);
                b.line_to(pt(p));
                last = p;
                any = true;
            }
            PathEl::Quad { c: q, p } => {
                ensure_open(&mut b, &mut open, last);
                b.quadratic_bezier_to(pt(q), pt(p));
                last = p;
                any = true;
            }
            PathEl::Cubic { c1, c2, p } => {
                ensure_open(&mut b, &mut open, last);
                b.cubic_bezier_to(pt(c1), pt(c2), pt(p));
                last = p;
                any = true;
            }
            PathEl::Close => {
                if open {
                    b.end(true);
                    open = false;
                }
                last = start;
            }
        }
    }
    if open {
        b.end(false);
    }
    any.then(|| b.build())
}

/// Split a path into its dashes (SVG semantics: an odd-length pattern repeats twice, the pattern
/// restarts at every subpath, closed subpaths dash around their closing edge). Curves are
/// flattened at `tolerance`. `None` when the pattern can't dash (empty, negative, all zero) and
/// the path should be stroked solid.
pub fn dash_path(path: &PathData, pattern: &[f64], tolerance: f64) -> Option<PathData> {
    if pattern.is_empty() || pattern.iter().any(|v| !v.is_finite() || *v < 0.0) {
        return None;
    }
    let pat: Vec<f64> = if pattern.len() % 2 == 1 { pattern.iter().chain(pattern.iter()).copied().collect() } else { pattern.to_vec() };
    if pat.iter().sum::<f64>() <= 0.0 {
        return None;
    }
    let mut out = PathData::new();
    let emit = |out: &mut PathData, pts: &[Vec2]| {
        if pts.len() >= 2 {
            out.els.extend(PathData::polyline(pts).els);
        }
    };
    for (mut pts, closed) in path.flatten(tolerance) {
        if closed && pts.len() > 1 {
            pts.push(pts[0]);
        }
        let (mut idx, mut on) = (0usize, true);
        let mut remaining = pat[0];
        let mut cur: Vec<Vec2> = vec![pts[0]];
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            let len = a.dist(b);
            let mut pos = 0.0;
            // The current dash or gap ends inside this segment (possibly several times).
            while len - pos > remaining {
                pos += remaining;
                let p = a.lerp(b, pos / len);
                if on {
                    cur.push(p);
                    emit(&mut out, &cur);
                    cur.clear();
                } else {
                    cur.clear();
                    cur.push(p);
                }
                on = !on;
                idx = (idx + 1) % pat.len();
                remaining = pat[idx];
            }
            remaining -= len - pos;
            if on {
                cur.push(b);
            }
        }
        if on {
            emit(&mut out, &cur);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use datars_math::Rect;

    fn area(m: &Mesh, stride: usize) -> f64 {
        m.indices
            .chunks(3)
            .map(|t| {
                let p = |i: u32| Vec2::new(m.verts[i as usize * stride] as f64, m.verts[i as usize * stride + 1] as f64);
                let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
                ((b - a).cross(c - a) / 2.0).abs()
            })
            .sum()
    }

    #[test]
    fn fill_rect_is_relative_to_its_centre() {
        let mut t = Tessellators::new();
        let m = t.fill(&PathData::rect(Rect::new(1000.0, 2000.0, 10.0, 20.0)), FillRule::NonZero, 0.1);
        assert_eq!(m.centre, Vec2::new(1005.0, 2010.0));
        assert!((area(&m, 2) - 200.0).abs() < 1e-3);
        assert!(m.verts.iter().all(|v| v.abs() <= 10.0), "vertices are small numbers around the centre");
    }

    #[test]
    fn even_odd_punches_holes() {
        let mut p = PathData::rect(Rect::new(0.0, 0.0, 10.0, 10.0));
        p.extend(&PathData::rect(Rect::new(2.0, 2.0, 6.0, 6.0)));
        let mut t = Tessellators::new();
        let nz = t.fill(&p, FillRule::NonZero, 0.1);
        let eo = t.fill(&p, FillRule::EvenOdd, 0.1);
        assert!((area(&nz, 2) - 100.0).abs() < 1e-3, "same winding: non-zero fills the hole");
        assert!((area(&eo, 2) - 64.0).abs() < 1e-3, "even-odd leaves it empty");
    }

    #[test]
    fn stroke_normals_scale_to_any_width() {
        let line = PathData::polyline(&[Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0)]);
        let spec = StrokeSpec { width: 2.0, cap: Cap::Butt, join: Join::Miter, miter_limit: 4.0, dash: None };
        let m = Tessellators::new().stroke(&line, &spec, 0.1);
        assert!(!m.is_empty());
        // Extruding by half of *any* width gives a band of that width.
        for hw in [1.0f32, 3.0] {
            let ys: Vec<f32> = m.verts.chunks(4).map(|v| v[1] + v[3] * hw).collect();
            let (lo, hi) = ys.iter().fold((f32::MAX, f32::MIN), |(a, b), &y| (a.min(y), b.max(y)));
            assert!((hi - lo - 2.0 * hw).abs() < 1e-5, "{lo} {hi}");
        }
    }

    #[test]
    fn dashes_split_and_restart() {
        let line = PathData::polyline(&[Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0)]);
        let d = dash_path(&line, &[3.0, 2.0], 0.1).unwrap();
        let polys = d.flatten(0.1);
        let spans: Vec<(f64, f64)> = polys.iter().map(|(p, _)| (p[0].x, p[p.len() - 1].x)).collect();
        assert_eq!(spans, vec![(0.0, 3.0), (5.0, 8.0)]);
        // Odd patterns repeat: [1] == [1, 1].
        assert_eq!(dash_path(&line, &[1.0], 0.1).unwrap().flatten(0.1).len(), 5);
        assert!(dash_path(&line, &[0.0, 0.0], 0.1).is_none());
        assert!(dash_path(&line, &[-1.0, 2.0], 0.1).is_none());
    }

    #[test]
    fn bad_paths_draw_nothing() {
        let mut t = Tessellators::new();
        let nan = PathData::polygon(&[Vec2::new(0.0, 0.0), Vec2::new(f64::NAN, 1.0), Vec2::new(1.0, 1.0)]);
        assert!(t.fill(&nan, FillRule::NonZero, 0.1).is_empty());
        assert!(t.fill(&PathData::new(), FillRule::NonZero, 0.1).is_empty());
        let mut moves = PathData::new();
        moves.move_to(Vec2::new(1.0, 1.0));
        assert!(t.fill(&moves, FillRule::NonZero, 0.1).is_empty());
    }
}
