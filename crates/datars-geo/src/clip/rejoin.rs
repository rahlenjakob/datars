//! The generalized polygon clipper shared by every clip edge (antimeridian, small circle,
//! rectangle), ported from d3-geo's `clip/rejoin.js`.
//!
//! A clipped polygon arrives as its visible *segments* (pieces of rings between crossings of the
//! clip edge). Each segment endpoint lies on the clip edge; sorting those endpoints along the edge
//! and walking segment → edge → segment rebuilds closed rings — a Weiler–Atherton walk. It handles
//! concave rings that cross the edge many times (each piece becomes its own ring) and holes, as
//! long as exteriors and holes have opposite winding.

use std::cmp::Ordering;

/// A point in clip space with d3's flag (`m`): nonzero marks points created at intersections
/// that must not be treated as closing a ring on their own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SPoint {
    pub x: f64,
    pub y: f64,
    pub m: u8,
}

impl SPoint {
    #[inline]
    pub fn new(x: f64, y: f64) -> SPoint {
        SPoint { x, y, m: 0 }
    }
    #[inline]
    pub fn flagged(x: f64, y: f64, m: u8) -> SPoint {
        SPoint { x, y, m }
    }
}

#[inline]
pub(crate) fn point_equal(a: SPoint, b: SPoint, eps: f64) -> bool {
    (a.x - b.x).abs() < eps && (a.y - b.y).abs() < eps
}

/// Emits the clip-edge path between two edge points (or the whole edge for `None, None`).
pub(crate) type Interpolate<'a> = dyn Fn(Option<SPoint>, Option<SPoint>, i32, &mut Vec<SPoint>) + 'a;

struct Node {
    x: SPoint,
    seg: Option<usize>,
    other: usize,
    entry: bool,
    visited: bool,
    next: usize,
    prev: usize,
}

fn link(nodes: &mut [Node], order: &[usize]) {
    let n = order.len();
    for i in 0..n {
        let (a, b) = (order[i], order[(i + 1) % n]);
        nodes[a].next = b;
        nodes[b].prev = a;
    }
}

/// Rejoin `segments` into rings appended to `out`. `compare` orders points along the clip edge;
/// `interpolate(from, to, direction, out)` emits the edge path between two edge points (or the
/// whole edge when both are `None`); `start_inside` says whether the start of the edge's
/// parameterization lies inside the polygon.
pub(crate) fn rejoin(
    mut segments: Vec<Vec<SPoint>>,
    compare: &dyn Fn(&SPoint, &SPoint) -> Ordering,
    mut start_inside: bool,
    interpolate: &Interpolate<'_>,
    eps: f64,
    out: &mut Vec<Vec<SPoint>>,
) {
    let mut nodes: Vec<Node> = Vec::new();
    let mut subject: Vec<usize> = Vec::new();
    let mut clip: Vec<usize> = Vec::new();
    let new_node = |nodes: &mut Vec<Node>, x: SPoint, seg: Option<usize>, entry: bool| {
        nodes.push(Node { x, seg, other: 0, entry, visited: false, next: 0, prev: 0 });
        nodes.len() - 1
    };
    for (si, seg) in segments.iter_mut().enumerate() {
        if seg.len() < 2 {
            continue;
        }
        let n = seg.len() - 1;
        let p0 = seg[0];
        if point_equal(p0, seg[n], eps) {
            if p0.m == 0 && seg[n].m == 0 {
                out.push(seg[..n].to_vec()); // already a closed ring
                continue;
            }
            seg[n].x += 2.0 * eps; // degenerate: nudge so the two ends sort apart
        }
        let p1 = seg[n];
        let a = new_node(&mut nodes, p0, Some(si), true);
        let ao = new_node(&mut nodes, p0, None, false);
        nodes[a].other = ao;
        nodes[ao].other = a;
        let b = new_node(&mut nodes, p1, Some(si), false);
        let bo = new_node(&mut nodes, p1, None, true);
        nodes[b].other = bo;
        nodes[bo].other = b;
        subject.extend([a, b]);
        clip.extend([ao, bo]);
    }
    if subject.is_empty() {
        return;
    }
    clip.sort_by(|&a, &b| compare(&nodes[a].x, &nodes[b].x)); // stable: deterministic ties
    link(&mut nodes, &subject);
    link(&mut nodes, &clip);
    for &c in &clip {
        start_inside = !start_inside;
        nodes[c].entry = start_inside;
    }

    let start = subject[0];
    loop {
        let mut current = start;
        let mut is_subject = true;
        while nodes[current].visited {
            current = nodes[current].next;
            if current == start {
                return;
            }
        }
        let mut points = nodes[current].seg;
        let mut ring: Vec<SPoint> = Vec::new();
        // Each step visits two nodes, so the walk ends within `nodes.len()` steps even on
        // inconsistent input.
        for _ in 0..=nodes.len() {
            nodes[current].visited = true;
            let o = nodes[current].other;
            nodes[o].visited = true;
            if nodes[current].entry {
                if is_subject {
                    if let Some(s) = points {
                        ring.extend_from_slice(&segments[s]);
                    }
                } else {
                    let to = nodes[nodes[current].next].x;
                    interpolate(Some(nodes[current].x), Some(to), 1, &mut ring);
                }
                current = nodes[current].next;
            } else {
                if is_subject {
                    points = nodes[nodes[current].prev].seg;
                    if let Some(s) = points {
                        ring.extend(segments[s].iter().rev());
                    }
                } else {
                    let to = nodes[nodes[current].prev].x;
                    interpolate(Some(nodes[current].x), Some(to), -1, &mut ring);
                }
                current = nodes[current].prev;
            }
            current = nodes[current].other;
            points = nodes[current].seg;
            is_subject = !is_subject;
            if nodes[current].visited {
                break;
            }
        }
        out.push(ring);
    }
}
