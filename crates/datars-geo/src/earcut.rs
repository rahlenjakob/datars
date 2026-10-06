//! Polygon triangulation: a faithful port of mapbox/earcut (v2.2.x) — linked-list ear clipping
//! with hole bridging and z-order hashing. Nodes live in an index arena (`Vec<Node>`, links are
//! `usize`, `NIL` = none): no Rc/RefCell, no unsafe, no floating-point functions beyond + − × ÷,
//! so results are identical on every target.
//!
//! The build pipeline pre-triangulates atlas meshes so the runtime rarely needs this for big
//! polygons; it's here for custom geometry and tiles.

use crate::geometry::Polygon;
use datars_math::Vec2;


const NIL: usize = usize::MAX;

#[derive(Clone, Debug)]
struct Node {
    i: u32, // vertex index (earcut.js stores the coordinate index; we store i / dim)
    x: f64,
    y: f64,
    prev: usize,
    next: usize,
    z: i32,
    prev_z: usize,
    next_z: usize,
    steiner: bool,
}

struct Arena {
    n: Vec<Node>,
}

impl Arena {
    #[inline]
    fn x(&self, p: usize) -> f64 {
        self.n[p].x
    }
    #[inline]
    fn y(&self, p: usize) -> f64 {
        self.n[p].y
    }
    #[inline]
    fn next(&self, p: usize) -> usize {
        self.n[p].next
    }
    #[inline]
    fn prev(&self, p: usize) -> usize {
        self.n[p].prev
    }

    fn new_node(&mut self, i: u32, x: f64, y: f64) -> usize {
        let id = self.n.len();
        self.n.push(Node { i, x, y, prev: id, next: id, z: 0, prev_z: NIL, next_z: NIL, steiner: false });
        id
    }

    /// Insert a node after `last` (or start a new ring when `last == NIL`).
    fn insert_node(&mut self, i: u32, x: f64, y: f64, last: usize) -> usize {
        let p = self.new_node(i, x, y);
        if last != NIL {
            let ln = self.n[last].next;
            self.n[p].next = ln;
            self.n[p].prev = last;
            self.n[ln].prev = p;
            self.n[last].next = p;
        }
        p
    }

    fn remove_node(&mut self, p: usize) {
        let (pp, pn, pz, nz) = (self.n[p].prev, self.n[p].next, self.n[p].prev_z, self.n[p].next_z);
        self.n[pn].prev = pp;
        self.n[pp].next = pn;
        if pz != NIL {
            self.n[pz].next_z = nz;
        }
        if nz != NIL {
            self.n[nz].prev_z = pz;
        }
    }

    /// Signed area of triangle (p, q, r).
    #[inline]
    fn area(&self, p: usize, q: usize, r: usize) -> f64 {
        let (p, q, r) = (&self.n[p], &self.n[q], &self.n[r]);
        (q.y - p.y) * (r.x - q.x) - (q.x - p.x) * (r.y - q.y)
    }

    #[inline]
    fn equals(&self, a: usize, b: usize) -> bool {
        self.n[a].x == self.n[b].x && self.n[a].y == self.n[b].y
    }

    /// Circular doubly linked list from a coordinate range, in the requested winding.
    fn linked_list(&mut self, data: &[f64], start: usize, end: usize, clockwise: bool) -> usize {
        let mut last = NIL;
        if start >= end {
            return NIL;
        }
        if clockwise == (signed_area(data, start, end) > 0.0) {
            let mut i = start;
            while i < end {
                last = self.insert_node((i / 2) as u32, data[i], data[i + 1], last);
                i += 2;
            }
        } else {
            let mut i = end;
            while i > start {
                i -= 2;
                last = self.insert_node((i / 2) as u32, data[i], data[i + 1], last);
            }
        }
        if last != NIL && self.equals(last, self.next(last)) {
            let nx = self.next(last);
            self.remove_node(last);
            last = nx;
        }
        last
    }

    /// Eliminate collinear or duplicate points.
    fn filter_points(&mut self, start: usize, end: usize) -> usize {
        if start == NIL {
            return start;
        }
        let mut end = if end == NIL { start } else { end };
        let mut p = start;
        loop {
            let mut again = false;
            let (pp, pn) = (self.prev(p), self.next(p));
            if !self.n[p].steiner && (self.equals(p, pn) || self.area(pp, p, pn) == 0.0) {
                self.remove_node(p);
                p = pp;
                end = pp;
                if p == self.next(p) {
                    break;
                }
                again = true;
            } else {
                p = pn;
            }
            if !(again || p != end) {
                break;
            }
        }
        end
    }

    /// Main ear-slicing loop.
    #[allow(clippy::too_many_arguments)]
    fn earcut_linked(&mut self, ear: usize, tris: &mut Vec<u32>, min_x: f64, min_y: f64, inv_size: f64, pass: u8) {
        if ear == NIL {
            return;
        }
        let mut ear = ear;
        // interlink polygon nodes in z-order
        if pass == 0 && inv_size != 0.0 {
            self.index_curve(ear, min_x, min_y, inv_size);
        }
        let mut stop = ear;

        while self.prev(ear) != self.next(ear) {
            let prev = self.prev(ear);
            let next = self.next(ear);

            let is_ear = if inv_size != 0.0 { self.is_ear_hashed(ear, min_x, min_y, inv_size) } else { self.is_ear(ear) };
            if is_ear {
                tris.push(self.n[prev].i);
                tris.push(self.n[ear].i);
                tris.push(self.n[next].i);
                self.remove_node(ear);
                // skipping the next vertex leads to fewer sliver triangles
                ear = self.next(next);
                stop = ear;
                continue;
            }

            ear = next;

            // looped through the whole remaining polygon without finding an ear
            if ear == stop {
                match pass {
                    0 => {
                        let e = self.filter_points(ear, NIL);
                        self.earcut_linked(e, tris, min_x, min_y, inv_size, 1);
                    }
                    1 => {
                        let e = self.filter_points(ear, NIL);
                        let e = self.cure_local_intersections(e, tris);
                        self.earcut_linked(e, tris, min_x, min_y, inv_size, 2);
                    }
                    _ => self.split_earcut(ear, tris, min_x, min_y, inv_size),
                }
                break;
            }
        }
    }

    /// Bounding box of triangle (a, b, c).
    #[inline]
    fn tri_bbox(&self, a: usize, b: usize, c: usize) -> (f64, f64, f64, f64) {
        let (ax, bx, cx, ay, by, cy) = (self.x(a), self.x(b), self.x(c), self.y(a), self.y(b), self.y(c));
        let x0 = if ax < bx { if ax < cx { ax } else { cx } } else if bx < cx { bx } else { cx };
        let y0 = if ay < by { if ay < cy { ay } else { cy } } else if by < cy { by } else { cy };
        let x1 = if ax > bx { if ax > cx { ax } else { cx } } else if bx > cx { bx } else { cx };
        let y1 = if ay > by { if ay > cy { ay } else { cy } } else if by > cy { by } else { cy };
        (x0, y0, x1, y1)
    }

    fn is_ear(&self, ear: usize) -> bool {
        let (a, b, c) = (self.prev(ear), ear, self.next(ear));
        if self.area(a, b, c) >= 0.0 {
            return false; // reflex, can't be an ear
        }
        let (ax, bx, cx, ay, by, cy) = (self.x(a), self.x(b), self.x(c), self.y(a), self.y(b), self.y(c));
        let (x0, y0, x1, y1) = self.tri_bbox(a, b, c);
        // make sure no other points are inside the potential ear
        let mut p = self.next(c);
        while p != a {
            let (px, py) = (self.x(p), self.y(p));
            if px >= x0
                && px <= x1
                && py >= y0
                && py <= y1
                && point_in_triangle(ax, ay, bx, by, cx, cy, px, py)
                && self.area(self.prev(p), p, self.next(p)) >= 0.0
            {
                return false;
            }
            p = self.next(p);
        }
        true
    }

    fn is_ear_hashed(&self, ear: usize, min_x: f64, min_y: f64, inv_size: f64) -> bool {
        let (a, b, c) = (self.prev(ear), ear, self.next(ear));
        if self.area(a, b, c) >= 0.0 {
            return false;
        }
        let (ax, bx, cx, ay, by, cy) = (self.x(a), self.x(b), self.x(c), self.y(a), self.y(b), self.y(c));
        let (x0, y0, x1, y1) = self.tri_bbox(a, b, c);

        // z-order range for the triangle bbox
        let min_z = z_order(x0, y0, min_x, min_y, inv_size);
        let max_z = z_order(x1, y1, min_x, min_y, inv_size);

        let blocks = |p: usize| -> bool {
            let (px, py) = (self.x(p), self.y(p));
            px >= x0
                && px <= x1
                && py >= y0
                && py <= y1
                && p != a
                && p != c
                && point_in_triangle(ax, ay, bx, by, cx, cy, px, py)
                && self.area(self.prev(p), p, self.next(p)) >= 0.0
        };

        let mut p = self.n[ear].prev_z;
        let mut n = self.n[ear].next_z;

        // look for points inside the triangle in both directions
        while p != NIL && self.n[p].z >= min_z && n != NIL && self.n[n].z <= max_z {
            if blocks(p) {
                return false;
            }
            p = self.n[p].prev_z;
            if blocks(n) {
                return false;
            }
            n = self.n[n].next_z;
        }
        // remaining points in decreasing z-order
        while p != NIL && self.n[p].z >= min_z {
            if blocks(p) {
                return false;
            }
            p = self.n[p].prev_z;
        }
        // remaining points in increasing z-order
        while n != NIL && self.n[n].z <= max_z {
            if blocks(n) {
                return false;
            }
            n = self.n[n].next_z;
        }
        true
    }

    /// Cure small local self-intersections.
    fn cure_local_intersections(&mut self, start: usize, tris: &mut Vec<u32>) -> usize {
        if start == NIL {
            return NIL;
        }
        let mut start = start;
        let mut p = start;
        loop {
            let a = self.prev(p);
            let pn = self.next(p);
            let b = self.next(pn);
            if !self.equals(a, b) && self.intersects(a, p, pn, b) && self.locally_inside(a, b) && self.locally_inside(b, a) {
                tris.push(self.n[a].i);
                tris.push(self.n[p].i);
                tris.push(self.n[b].i);
                // remove the two nodes involved
                self.remove_node(p);
                self.remove_node(pn);
                p = b;
                start = b;
            }
            p = self.next(p);
            if p == start {
                break;
            }
        }
        self.filter_points(p, NIL)
    }

    /// Try splitting the polygon along a valid diagonal and triangulate both halves.
    fn split_earcut(&mut self, start: usize, tris: &mut Vec<u32>, min_x: f64, min_y: f64, inv_size: f64) {
        let mut a = start;
        loop {
            let mut b = self.next(self.next(a));
            while b != self.prev(a) {
                if self.n[a].i != self.n[b].i && self.is_valid_diagonal(a, b) {
                    let c = self.split_polygon(a, b);
                    // filter collinear points around the cuts
                    let an = self.next(a);
                    let a2 = self.filter_points(a, an);
                    let cn = self.next(c);
                    let c2 = self.filter_points(c, cn);
                    self.earcut_linked(a2, tris, min_x, min_y, inv_size, 0);
                    self.earcut_linked(c2, tris, min_x, min_y, inv_size, 0);
                    return;
                }
                b = self.next(b);
            }
            a = self.next(a);
            if a == start {
                break;
            }
        }
    }

    /// Link every hole into the outer ring, producing a single ring without holes.
    fn eliminate_holes(&mut self, data: &[f64], hole_starts: &[usize], mut outer: usize) -> usize {
        let mut queue = Vec::with_capacity(hole_starts.len());
        for (k, &start) in hole_starts.iter().enumerate() {
            let end = hole_starts.get(k + 1).copied().unwrap_or(data.len());
            let list = self.linked_list(data, start, end, false);
            if list == NIL {
                continue;
            }
            if list == self.next(list) {
                self.n[list].steiner = true;
            }
            queue.push(self.get_leftmost(list));
        }
        queue.sort_by(|&a, &b| self.x(a).partial_cmp(&self.x(b)).unwrap_or(std::cmp::Ordering::Equal));
        // process holes left to right
        for h in queue {
            outer = self.eliminate_hole(h, outer);
        }
        outer
    }

    fn eliminate_hole(&mut self, hole: usize, outer: usize) -> usize {
        let bridge = self.find_hole_bridge(hole, outer);
        if bridge == NIL {
            return outer;
        }
        let bridge_rev = self.split_polygon(bridge, hole);
        // filter collinear points around the cuts
        let brn = self.next(bridge_rev);
        self.filter_points(bridge_rev, brn);
        let bn = self.next(bridge);
        self.filter_points(bridge, bn)
    }

    /// David Eberly's algorithm for finding a bridge between a hole and the outer polygon.
    fn find_hole_bridge(&self, hole: usize, outer: usize) -> usize {
        let mut p = outer;
        let (hx, hy) = (self.x(hole), self.y(hole));
        let mut qx = f64::NEG_INFINITY;
        let mut m = NIL;

        // find a segment intersected by a ray from the hole's leftmost point to the left;
        // the segment's endpoint with lesser x is the potential connection point
        loop {
            let pn = self.next(p);
            let (px, py, nx, ny) = (self.x(p), self.y(p), self.x(pn), self.y(pn));
            if hy <= py && hy >= ny && ny != py {
                let x = px + (hy - py) * (nx - px) / (ny - py);
                if x <= hx && x > qx {
                    qx = x;
                    m = if px < nx { p } else { pn };
                    if x == hx {
                        return m; // hole touches outer segment; pick leftmost endpoint
                    }
                }
            }
            p = pn;
            if p == outer {
                break;
            }
        }
        if m == NIL {
            return NIL;
        }

        // look for points inside the triangle of hole point, segment intersection and endpoint;
        // if none, the connection is valid; otherwise pick the point of minimum angle with the ray
        let stop = m;
        let (mx, my) = (self.x(m), self.y(m));
        let mut tan_min = f64::INFINITY;
        p = m;
        loop {
            let (px, py) = (self.x(p), self.y(p));
            if hx >= px
                && px >= mx
                && hx != px
                && point_in_triangle(if hy < my { hx } else { qx }, hy, mx, my, if hy < my { qx } else { hx }, hy, px, py)
            {
                let tan = (hy - py).abs() / (hx - px);
                if self.locally_inside(p, hole)
                    && (tan < tan_min
                        || (tan == tan_min && (px > self.x(m) || (px == self.x(m) && self.sector_contains_sector(m, p)))))
                {
                    m = p;
                    tan_min = tan;
                }
            }
            p = self.next(p);
            if p == stop {
                break;
            }
        }
        m
    }

    /// Whether the sector at vertex m contains the sector at vertex p (same coordinates).
    fn sector_contains_sector(&self, m: usize, p: usize) -> bool {
        self.area(self.prev(m), m, self.prev(p)) < 0.0 && self.area(self.next(p), m, self.next(m)) < 0.0
    }

    /// Interlink polygon nodes in z-order.
    fn index_curve(&mut self, start: usize, min_x: f64, min_y: f64, inv_size: f64) {
        let mut p = start;
        loop {
            if self.n[p].z == 0 {
                self.n[p].z = z_order(self.x(p), self.y(p), min_x, min_y, inv_size);
            }
            self.n[p].prev_z = self.n[p].prev;
            self.n[p].next_z = self.n[p].next;
            p = self.next(p);
            if p == start {
                break;
            }
        }
        let pz = self.n[p].prev_z;
        self.n[pz].next_z = NIL;
        self.n[p].prev_z = NIL;
        self.sort_linked(p);
    }

    /// Simon Tatham's linked-list merge sort over the z links.
    fn sort_linked(&mut self, mut list: usize) -> usize {
        let mut in_size = 1usize;
        loop {
            let mut p = list;
            list = NIL;
            let mut tail = NIL;
            let mut num_merges = 0;

            while p != NIL {
                num_merges += 1;
                let mut q = p;
                let mut p_size = 0;
                for _ in 0..in_size {
                    p_size += 1;
                    q = self.n[q].next_z;
                    if q == NIL {
                        break;
                    }
                }
                let mut q_size = in_size;

                while p_size > 0 || (q_size > 0 && q != NIL) {
                    let e;
                    if p_size != 0 && (q_size == 0 || q == NIL || self.n[p].z <= self.n[q].z) {
                        e = p;
                        p = self.n[p].next_z;
                        p_size -= 1;
                    } else {
                        e = q;
                        q = self.n[q].next_z;
                        q_size -= 1;
                    }
                    if tail != NIL {
                        self.n[tail].next_z = e;
                    } else {
                        list = e;
                    }
                    self.n[e].prev_z = tail;
                    tail = e;
                }
                p = q;
            }
            if tail != NIL {
                self.n[tail].next_z = NIL;
            }
            in_size *= 2;
            if num_merges <= 1 {
                break;
            }
        }
        list
    }

    fn get_leftmost(&self, start: usize) -> usize {
        let mut p = start;
        let mut left = start;
        loop {
            if self.x(p) < self.x(left) || (self.x(p) == self.x(left) && self.y(p) < self.y(left)) {
                left = p;
            }
            p = self.next(p);
            if p == start {
                break;
            }
        }
        left
    }

    /// Whether the diagonal a-b lies in the polygon interior.
    fn is_valid_diagonal(&self, a: usize, b: usize) -> bool {
        let bi = self.n[b].i;
        self.n[self.next(a)].i != bi
            && self.n[self.prev(a)].i != bi
            && !self.intersects_polygon(a, b)
            && ((self.locally_inside(a, b)
                && self.locally_inside(b, a)
                && self.middle_inside(a, b)
                // does not create opposite-facing sectors
                && (self.area(self.prev(a), a, self.prev(b)) != 0.0 || self.area(a, self.prev(b), b) != 0.0))
                // special zero-length case
                || (self.equals(a, b)
                    && self.area(self.prev(a), a, self.next(a)) > 0.0
                    && self.area(self.prev(b), b, self.next(b)) > 0.0))
    }

    /// Whether segment p1-q1 intersects p2-q2 (including collinear overlap).
    fn intersects(&self, p1: usize, q1: usize, p2: usize, q2: usize) -> bool {
        let o1 = sign(self.area(p1, q1, p2));
        let o2 = sign(self.area(p1, q1, q2));
        let o3 = sign(self.area(p2, q2, p1));
        let o4 = sign(self.area(p2, q2, q1));
        if o1 != o2 && o3 != o4 {
            return true; // general case
        }
        (o1 == 0 && self.on_segment(p1, p2, q1))
            || (o2 == 0 && self.on_segment(p1, q2, q1))
            || (o3 == 0 && self.on_segment(p2, p1, q2))
            || (o4 == 0 && self.on_segment(p2, q1, q2))
    }

    /// For collinear p, q, r: whether q lies on segment p-r.
    fn on_segment(&self, p: usize, q: usize, r: usize) -> bool {
        let (px, py, qx, qy, rx, ry) = (self.x(p), self.y(p), self.x(q), self.y(q), self.x(r), self.y(r));
        qx <= px.max(rx) && qx >= px.min(rx) && qy <= py.max(ry) && qy >= py.min(ry)
    }

    /// Whether the diagonal a-b intersects any polygon edge.
    fn intersects_polygon(&self, a: usize, b: usize) -> bool {
        let (ai, bi) = (self.n[a].i, self.n[b].i);
        let mut p = a;
        loop {
            let pn = self.next(p);
            let (pi, pni) = (self.n[p].i, self.n[pn].i);
            if pi != ai && pni != ai && pi != bi && pni != bi && self.intersects(p, pn, a, b) {
                return true;
            }
            p = pn;
            if p == a {
                break;
            }
        }
        false
    }

    /// Whether the diagonal a-b is locally inside the polygon at a.
    fn locally_inside(&self, a: usize, b: usize) -> bool {
        let (ap, an) = (self.prev(a), self.next(a));
        if self.area(ap, a, an) < 0.0 {
            self.area(a, b, an) >= 0.0 && self.area(a, ap, b) >= 0.0
        } else {
            self.area(a, b, ap) < 0.0 || self.area(a, an, b) < 0.0
        }
    }

    /// Whether the midpoint of diagonal a-b is inside the polygon.
    fn middle_inside(&self, a: usize, b: usize) -> bool {
        let mut p = a;
        let mut inside = false;
        let px = (self.x(a) + self.x(b)) / 2.0;
        let py = (self.y(a) + self.y(b)) / 2.0;
        loop {
            let pn = self.next(p);
            let (x0, y0, x1, y1) = (self.x(p), self.y(p), self.x(pn), self.y(pn));
            if (y0 > py) != (y1 > py) && y1 != y0 && px < (x1 - x0) * (py - y0) / (y1 - y0) + x0 {
                inside = !inside;
            }
            p = pn;
            if p == a {
                break;
            }
        }
        inside
    }

    /// Link a to b with a bridge. If a and b are in the same ring it splits into two;
    /// if in different rings it merges them. Returns the duplicate of b.
    fn split_polygon(&mut self, a: usize, b: usize) -> usize {
        let a2 = self.new_node(self.n[a].i, self.x(a), self.y(a));
        let b2 = self.new_node(self.n[b].i, self.x(b), self.y(b));
        let an = self.next(a);
        let bp = self.prev(b);

        self.n[a].next = b;
        self.n[b].prev = a;

        self.n[a2].next = an;
        self.n[an].prev = a2;

        self.n[b2].next = a2;
        self.n[a2].prev = b2;

        self.n[bp].next = b2;
        self.n[b2].prev = bp;

        b2
    }
}

/// Point-in-triangle test (inclusive of edges) for a counter-clockwise-in-earcut-terms triangle.
#[allow(clippy::too_many_arguments)]
#[inline]
fn point_in_triangle(ax: f64, ay: f64, bx: f64, by: f64, cx: f64, cy: f64, px: f64, py: f64) -> bool {
    (cx - px) * (ay - py) >= (ax - px) * (cy - py)
        && (ax - px) * (by - py) >= (bx - px) * (ay - py)
        && (bx - px) * (cy - py) >= (cx - px) * (by - py)
}

/// z-order of a point, coords mapped into a 15-bit integer grid.
fn z_order(x: f64, y: f64, min_x: f64, min_y: f64, inv_size: f64) -> i32 {
    // `as i32` saturates (and maps NaN to 0), where JS `| 0` wraps; values are in [0, 32767] anyway
    let mut x = ((x - min_x) * inv_size) as i32;
    let mut y = ((y - min_y) * inv_size) as i32;

    x = (x | (x << 8)) & 0x00FF_00FF;
    x = (x | (x << 4)) & 0x0F0F_0F0F;
    x = (x | (x << 2)) & 0x3333_3333;
    x = (x | (x << 1)) & 0x5555_5555;

    y = (y | (y << 8)) & 0x00FF_00FF;
    y = (y | (y << 4)) & 0x0F0F_0F0F;
    y = (y | (y << 2)) & 0x3333_3333;
    y = (y | (y << 1)) & 0x5555_5555;

    x | (y << 1)
}

#[inline]
fn sign(v: f64) -> i8 {
    if v > 0.0 {
        1
    } else if v < 0.0 {
        -1
    } else {
        0
    }
}

/// Shoelace sum over coordinate range [start, end) (earcut.js sign convention).
fn signed_area(data: &[f64], start: usize, end: usize) -> f64 {
    let mut sum = 0.0;
    if end < start + 2 {
        return sum;
    }
    let mut j = end - 2;
    let mut i = start;
    while i < end {
        sum += (data[j] - data[i]) * (data[i + 1] + data[j + 1]);
        j = i;
        i += 2;
    }
    sum
}

/// Hole starts as coordinate indices, clamped to the data and forced monotonic.
fn hole_coord_starts(len: usize, hole_indices: &[usize]) -> Vec<usize> {
    let mut out = Vec::with_capacity(hole_indices.len());
    let mut lo = 0;
    for &h in hole_indices {
        let s = h.saturating_mul(2).min(len).max(lo);
        out.push(s);
        lo = s;
    }
    out
}

/// Triangulate a polygon given as flat coordinates [x0,y0,x1,y1,...] with `hole_indices`
/// = vertex indices (not coordinate indices) where each hole ring starts. Returns triangle
/// vertex indices (3 per triangle) into the vertex list. Same contract as earcut.js.
pub fn earcut(coords: &[f64], hole_indices: &[usize]) -> Vec<u32> {
    let data = &coords[..coords.len() & !1]; // ignore a dangling odd coordinate
    let holes = hole_coord_starts(data.len(), hole_indices);
    let outer_len = holes.first().copied().unwrap_or(data.len());

    let mut tris = Vec::new();
    let mut ar = Arena { n: Vec::with_capacity(data.len() / 2 * 3 / 2 + 8) };
    let mut outer = ar.linked_list(data, 0, outer_len, true);
    if outer == NIL || ar.next(outer) == ar.prev(outer) {
        return tris;
    }
    tris.reserve(data.len() / 2 * 3);

    if !holes.is_empty() {
        outer = ar.eliminate_holes(data, &holes, outer);
    }

    // for non-trivial shapes, hash on a z-order curve; compute the outer ring's bbox
    let (mut min_x, mut min_y, mut inv_size) = (0.0, 0.0, 0.0);
    if data.len() > 80 * 2 {
        min_x = data[0];
        min_y = data[1];
        let (mut max_x, mut max_y) = (min_x, min_y);
        let mut i = 2;
        while i < outer_len {
            let (x, y) = (data[i], data[i + 1]);
            if x < min_x {
                min_x = x;
            }
            if y < min_y {
                min_y = y;
            }
            if x > max_x {
                max_x = x;
            }
            if y > max_y {
                max_y = y;
            }
            i += 2;
        }
        let size = (max_x - min_x).max(max_y - min_y);
        inv_size = if size != 0.0 && size.is_finite() { 32767.0 / size } else { 0.0 };
    }

    ar.earcut_linked(outer, &mut tris, min_x, min_y, inv_size, 0);
    tris
}

/// Triangulate one polygon: `rings[0]` is the exterior, the rest are holes (any winding; a
/// closing duplicate per ring is dropped). Returns the vertices and triangle indices into them
/// (3 per triangle).
pub fn triangulate(rings: &[Vec<Vec2>]) -> (Vec<Vec2>, Vec<u32>) {
    let mut verts: Vec<Vec2> = Vec::new();
    let mut holes = Vec::new();
    for (k, ring) in rings.iter().enumerate() {
        let mut r: &[Vec2] = ring;
        if r.len() > 1 && r[0] == r[r.len() - 1] {
            r = &r[..r.len() - 1];
        }
        if k > 0 {
            if r.is_empty() {
                continue; // an empty hole would alias the next ring's start
            }
            holes.push(verts.len());
        }
        verts.extend_from_slice(r);
    }
    let flat: Vec<f64> = verts.iter().flat_map(|p| [p.x, p.y]).collect();
    let tris = earcut(&flat, &holes);
    (verts, tris)
}

/// Triangulate many polygons into one mesh (indices offset per polygon).
pub fn triangulate_polygons(polygons: &[Polygon]) -> (Vec<Vec2>, Vec<u32>) {
    let mut verts = Vec::new();
    let mut idx = Vec::new();
    for p in polygons {
        let (v, t) = triangulate(p);
        let base = verts.len() as u32;
        verts.extend(v);
        idx.extend(t.into_iter().map(|i| i + base));
    }
    (verts, idx)
}

/// earcut.js `deviation`: relative difference between polygon area and triangle area (0 = perfect).
pub fn deviation(coords: &[f64], hole_indices: &[usize], triangles: &[u32]) -> f64 {
    let data = &coords[..coords.len() & !1];
    let holes = hole_coord_starts(data.len(), hole_indices);
    let outer_len = holes.first().copied().unwrap_or(data.len());

    let mut polygon_area = signed_area(data, 0, outer_len).abs();
    for (k, &start) in holes.iter().enumerate() {
        let end = holes.get(k + 1).copied().unwrap_or(data.len());
        polygon_area -= signed_area(data, start, end).abs();
    }

    let mut triangles_area = 0.0;
    for t in triangles.chunks_exact(3) {
        let (a, b, c) = (t[0] as usize * 2, t[1] as usize * 2, t[2] as usize * 2);
        if c + 1 >= data.len() || a + 1 >= data.len() || b + 1 >= data.len() {
            continue;
        }
        triangles_area += ((data[a] - data[c]) * (data[b + 1] - data[a + 1])
            - (data[a] - data[b]) * (data[c + 1] - data[a + 1]))
            .abs();
    }

    if polygon_area == 0.0 && triangles_area == 0.0 {
        0.0
    } else {
        ((triangles_area - polygon_area) / polygon_area).abs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use datars_math::m;

    fn flat(rings: &[Vec<Vec2>]) -> (Vec<f64>, Vec<usize>) {
        let mut c = Vec::new();
        let mut h = Vec::new();
        for (k, r) in rings.iter().enumerate() {
            if k > 0 {
                h.push(c.len() / 2);
            }
            for p in r {
                c.push(p.x);
                c.push(p.y);
            }
        }
        (c, h)
    }

    fn tri_area(v: &[Vec2], t: &[u32]) -> f64 {
        t.chunks_exact(3)
            .map(|t| {
                let (a, b, c) = (v[t[0] as usize], v[t[1] as usize], v[t[2] as usize]);
                ((b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y)).abs() / 2.0
            })
            .sum()
    }

    fn square(x0: f64, y0: f64, s: f64) -> Vec<Vec2> {
        vec![Vec2::new(x0, y0), Vec2::new(x0 + s, y0), Vec2::new(x0 + s, y0 + s), Vec2::new(x0, y0 + s)]
    }

    fn pts(v: &[(f64, f64)]) -> Vec<Vec2> {
        v.iter().map(|&(x, y)| Vec2::new(x, y)).collect()
    }

    #[test]
    fn square_two_tris() {
        let c = [0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0];
        let t = earcut(&c, &[]);
        assert_eq!(t.len(), 6);
        assert_eq!(deviation(&c, &[], &t), 0.0);
        assert!(t.iter().all(|&i| i < 4));
    }

    #[test]
    fn square_with_hole() {
        let rings = vec![square(0.0, 0.0, 10.0), square(3.0, 3.0, 4.0)];
        let (c, h) = flat(&rings);
        let t = earcut(&c, &h);
        assert_eq!(t.len() / 3, 8);
        assert!(deviation(&c, &h, &t) < 1e-12);
        let (v, t2) = triangulate(&rings);
        assert!((tri_area(&v, &t2) - 84.0).abs() < 1e-9);
    }

    #[test]
    fn concave_l() {
        let l = pts(&[(0.0, 0.0), (2.0, 0.0), (2.0, 1.0), (1.0, 1.0), (1.0, 2.0), (0.0, 2.0)]);
        let (v, t) = triangulate(std::slice::from_ref(&l));
        assert_eq!(t.len() / 3, 4);
        assert!((tri_area(&v, &t) - 3.0).abs() < 1e-12);
        // reversed winding gives the same area
        let mut r = l;
        r.reverse();
        let (v, t) = triangulate(&[r]);
        assert!((tri_area(&v, &t) - 3.0).abs() < 1e-12);
    }

    #[test]
    fn two_holes() {
        let mut h2 = square(12.0, 2.0, 5.0);
        h2.reverse(); // mixed hole windings
        let rings = vec![pts(&[(0.0, 0.0), (20.0, 0.0), (20.0, 10.0), (0.0, 10.0)]), square(2.0, 2.0, 5.0), h2];
        let (c, h) = flat(&rings);
        let t = earcut(&c, &h);
        // <= n + 2h - 2: bridges collinear with hole edges get filtered, saving triangles
        assert!(t.len() / 3 >= 10 && t.len() / 3 <= 12 + 2 * 2 - 2, "{} tris", t.len() / 3);
        assert!(deviation(&c, &h, &t) < 1e-12, "dev {}", deviation(&c, &h, &t));
        let v: Vec<Vec2> = c.chunks(2).map(|p| Vec2::new(p[0], p[1])).collect();
        assert!((tri_area(&v, &t) - (200.0 - 50.0)).abs() < 1e-9);
    }

    #[test]
    fn large_star_hashed() {
        // 2000-vertex noisy star: exercises z-order hashing (> 80 vertices)
        let n = 2000;
        let mut seed: u64 = 12345;
        let mut ring = Vec::with_capacity(n);
        for k in 0..n {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let noise = ((seed >> 33) as f64 / (1u64 << 31) as f64) * 0.2;
            let r = if k % 2 == 0 { 100.0 } else { 60.0 } + noise * 10.0;
            let a = k as f64 / n as f64 * m::TAU;
            ring.push(Vec2::new(r * m::cos(a), r * m::sin(a)));
        }
        let hole: Vec<Vec2> = (0..200)
            .map(|k| {
                let a = k as f64 / 200.0 * m::TAU;
                Vec2::new(20.0 * m::cos(a), 20.0 * m::sin(a))
            })
            .collect();
        let (c, h) = flat(&[ring.clone()]);
        let t = earcut(&c, &h);
        assert_eq!(t.len() / 3, n - 2);
        assert!(deviation(&c, &h, &t) < 1e-9, "dev {}", deviation(&c, &h, &t));

        let (c, h) = flat(&[ring, hole]);
        let t = earcut(&c, &h);
        assert_eq!(t.len() / 3, n + 200 + 2 - 2);
        assert!(deviation(&c, &h, &t) < 1e-9, "dev {}", deviation(&c, &h, &t));
    }

    #[test]
    fn degenerate_inputs() {
        assert!(earcut(&[], &[]).is_empty());
        assert!(earcut(&[0.0, 0.0, 1.0, 1.0], &[]).is_empty());
        assert!(earcut(&[0.0, 0.0, 1.0, 1.0, 2.0], &[]).is_empty()); // odd length
        let col = [0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0];
        let t = earcut(&col, &[]);
        assert!(deviation(&col, &[], &t) == 0.0);
        // all same point
        let _ = earcut(&[1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0], &[]);
        // duplicates and a zero-area spike
        let _ = earcut(&[0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 1.0, 2.0, 1.0, 1.0, 1.0, 0.0, 1.0], &[]);
        // bogus hole indices (past end, duplicated, empty holes)
        let sq = [0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0];
        assert_eq!(earcut(&sq, &[4, 4, 99]).len(), 6);
        let _ = earcut(&sq, &[0]);
        let _ = earcut(&sq, &[3]);
        // non-finite coordinates must not hang
        let _ = earcut(&[0.0, 0.0, f64::NAN, 0.0, 1.0, 1.0, 0.0, 1.0], &[]);
        let _ = earcut(&[0.0, 0.0, f64::INFINITY, 0.0, 1.0, 1.0, 0.0, 1.0], &[]);
        // self-touching bow-tie
        let _ = earcut(&[0.0, 0.0, 2.0, 2.0, 2.0, 0.0, 0.0, 2.0], &[]);
        assert!(triangulate(&[]).1.is_empty());
        assert!(triangulate(&[vec![]]).1.is_empty());
    }

    #[test]
    fn hole_touching_outer() {
        // hole shares a vertex with the outer ring
        let rings = vec![square(0.0, 0.0, 4.0), pts(&[(0.0, 2.0), (1.0, 1.0), (2.0, 2.0), (1.0, 3.0)])];
        let (c, h) = flat(&rings);
        let t = earcut(&c, &h);
        assert!(deviation(&c, &h, &t) < 1e-12);
    }

    #[test]
    fn closing_duplicate_dropped() {
        let mut outer = square(0.0, 0.0, 10.0);
        outer.push(outer[0]);
        let mut hole = square(3.0, 3.0, 4.0);
        hole.push(hole[0]);
        let (v, t) = triangulate(&[outer, hole]);
        assert_eq!(v.len(), 8);
        assert_eq!(t.len() / 3, 8);
        assert!((tri_area(&v, &t) - 84.0).abs() < 1e-9);
        assert!(t.iter().all(|&i| (i as usize) < v.len()));
    }
}
