//! The coverage rasterizer: exact-area anti-aliasing accumulated per cell in 24.8 fixed point (the
//! FreeType "smooth" / font-rs technique), swept per scanline.
//!
//! Why this design: every step after the float → fixed conversion is integer arithmetic (adds,
//! multiplies, divisions with an explicit rounding rule), and integer addition is associative, so
//! the result depends only on the edges — not on the order they're added, the CPU, or the compiler.
//! That is what makes this rasterizer the bit-exact reference (docs/11-rendering.md).
//!
//! How it works: each edge is walked row by row and, within a row, cell by cell. A cell records
//! `cover` (the signed height the edge spans inside it, in 1/256 px) and `area` (`cover` × the sum
//! of the edge's x-fractions at entry and exit, i.e. twice the trapezoid area to the cell's left).
//! Sweeping a row left to right, the running sum of `cover` is the winding number times 256 and
//! `acc·512 − area` is the signed coverage of each pixel, to which the fill rule is applied.
//!
//! Clipping: in float, edges are clipped to the pixmap rows, edges left of the pixmap are projected
//! onto x = 0 (they still change the winding of everything to their right) and edges right of it
//! are dropped. In fixed point, the destination rectangle further restricts rows and columns;
//! coverage inside that rectangle is identical to rasterizing without it.

use datars_math::{FillRule, Vec2};

/// Fractional bits of the fixed-point format: 24.8, so 1/256 px.
pub const FRAC_BITS: i32 = 8;
const ONE: i32 = 1 << FRAC_BITS;
/// The cell buffer holds at most this many cells; taller shapes are rasterized in bands.
const CELL_BUDGET: usize = 1 << 18;

/// An integer pixel rectangle `[x0, x1) × [y0, y1)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IRect {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl IRect {
    pub const EMPTY: IRect = IRect { x0: 0, y0: 0, x1: 0, y1: 0 };

    pub const fn new(x0: i32, y0: i32, x1: i32, y1: i32) -> IRect {
        IRect { x0, y0, x1, y1 }
    }
    pub fn is_empty(&self) -> bool {
        self.x0 >= self.x1 || self.y0 >= self.y1
    }
    pub fn intersect(&self, o: &IRect) -> IRect {
        let r = IRect::new(self.x0.max(o.x0), self.y0.max(o.y0), self.x1.min(o.x1), self.y1.min(o.y1));
        if r.is_empty() {
            IRect::EMPTY
        } else {
            r
        }
    }
    pub fn union(&self, o: &IRect) -> IRect {
        if self.is_empty() {
            return *o;
        }
        if o.is_empty() {
            return *self;
        }
        IRect::new(self.x0.min(o.x0), self.y0.min(o.y0), self.x1.max(o.x1), self.y1.max(o.y1))
    }
    pub fn width(&self) -> usize {
        (self.x1 - self.x0).max(0) as usize
    }
    pub fn height(&self) -> usize {
        (self.y1 - self.y0).max(0) as usize
    }
}

/// An edge in fixed point, oriented top to bottom (`y0 < y1`); `dir` is +1 if the original edge
/// went down, −1 if it went up.
#[derive(Clone, Copy, Debug)]
struct Edge {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    dir: i32,
}

#[derive(Clone, Copy, Debug, Default)]
struct Cell {
    cover: i32,
    area: i32,
}

/// A reusable rasterizer for one pixmap size. Add closed contours, then [`Rasterizer::fill`].
pub struct Rasterizer {
    w: i32,
    h: i32,
    edges: Vec<Edge>,
    min_x: i32,
    min_y: i32,
    max_x: i32,
    max_y: i32,
    /// Some edge was dropped right of the pixmap, so coverage may extend to its right border.
    open_right: bool,
    cells: Vec<Cell>,
    row_lo: Vec<u32>,
    row_hi: Vec<u32>,
    cov: Vec<u8>,
}

/// `n / d` rounded to nearest (ties up), for `d != 0`. Integer-only, so identical everywhere.
#[inline]
fn div_round(n: i64, d: i64) -> i64 {
    let (n, d) = if d < 0 { (-n, -d) } else { (n, d) };
    (2 * n + d).div_euclid(2 * d)
}

/// The coverage (0–255) of a pixel from its accumulated signed area (1 px = 2·256² units).
#[inline]
fn coverage(val: i64, even_odd: bool) -> u8 {
    let mut c = (val.abs() + 256) >> 9; // 1/256 units, rounded
    if even_odd {
        c &= 511;
        if c > 256 {
            c = 512 - c;
        }
    } else if c > 256 {
        c = 256;
    }
    ((c * 255 + 128) >> 8) as u8
}

impl Rasterizer {
    pub fn new(width: u32, height: u32) -> Rasterizer {
        Rasterizer {
            w: width as i32,
            h: height as i32,
            edges: Vec::new(),
            min_x: i32::MAX,
            min_y: i32::MAX,
            max_x: i32::MIN,
            max_y: i32::MIN,
            open_right: false,
            cells: Vec::new(),
            row_lo: Vec::new(),
            row_hi: Vec::new(),
            cov: Vec::new(),
        }
    }

    /// Forget all edges (buffers are kept).
    pub fn reset(&mut self) {
        self.edges.clear();
        self.min_x = i32::MAX;
        self.min_y = i32::MAX;
        self.max_x = i32::MIN;
        self.max_y = i32::MIN;
        self.open_right = false;
    }

    pub fn is_empty(&self) -> bool {
        self.edges.is_empty() && !self.open_right
    }

    /// Add a closed polygon (the closing edge is implied). Device-space float coordinates.
    pub fn polygon(&mut self, pts: &[Vec2]) {
        if pts.len() < 2 {
            return;
        }
        for w in pts.windows(2) {
            self.line(w[0], w[1]);
        }
        self.line(pts[pts.len() - 1], pts[0]);
    }

    /// Add one edge. Horizontal edges contribute nothing; non-finite ones are ignored.
    pub fn line(&mut self, a: Vec2, b: Vec2) {
        if !(a.is_finite() && b.is_finite()) || a.y == b.y {
            return;
        }
        let (w, h) = (self.w as f64, self.h as f64);
        let (dir, p, q) = if a.y < b.y { (1, a, b) } else { (-1, b, a) };
        if p.y >= 0.0 && q.y <= h && p.x.min(q.x) >= 0.0 && p.x.max(q.x) <= w {
            self.push(p.x, p.y, q.x, q.y, dir); // the common case: nothing to clip
            return;
        }
        if q.y <= 0.0 || p.y >= h {
            return; // entirely above or below: no visible row changes
        }
        // Clip to the pixmap rows (both cut points from the original endpoints).
        let x_at = |y: f64| p.x + (q.x - p.x) * ((y - p.y) / (q.y - p.y));
        let top = if p.y < 0.0 { Vec2::new(x_at(0.0), 0.0) } else { p };
        let bot = if q.y > h { Vec2::new(x_at(h), h) } else { q };
        let (p, q) = (top, bot);
        if p.x.min(q.x) >= w {
            self.open_right = true;
            return;
        }
        if p.x.max(q.x) <= 0.0 {
            self.push(0.0, p.y, 0.0, q.y, dir);
            return;
        }
        // Split where the edge crosses x = 0 or x = w; pieces left of 0 are projected onto it,
        // pieces right of w are dropped.
        let mut cuts = [p; 4];
        let mut n = 1;
        let y_at = |x: f64| (p.y + (q.y - p.y) * ((x - p.x) / (q.x - p.x))).clamp(p.y, q.y);
        if (p.x < 0.0) != (q.x < 0.0) {
            cuts[n] = Vec2::new(0.0, y_at(0.0));
            n += 1;
        }
        if (p.x < w) != (q.x < w) {
            cuts[n] = Vec2::new(w, y_at(w));
            n += 1;
        }
        if n == 3 && cuts[2].y < cuts[1].y {
            cuts.swap(1, 2);
        }
        cuts[n] = q;
        n += 1;
        for k in 0..n - 1 {
            let (c0, c1) = (cuts[k], cuts[k + 1]);
            let mx = (c0.x + c1.x) * 0.5;
            if mx < 0.0 {
                self.push(0.0, c0.y, 0.0, c1.y, dir);
            } else if mx > w {
                self.open_right = true;
            } else {
                self.push(c0.x.clamp(0.0, w), c0.y, c1.x.clamp(0.0, w), c1.y, dir);
            }
        }
    }

    fn push(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, dir: i32) {
        let fx = |v: f64| (v * ONE as f64).round() as i32;
        let (x0, y0, x1, y1) = (fx(x0), fx(y0), fx(x1), fx(y1));
        if y0 >= y1 {
            return;
        }
        self.min_x = self.min_x.min(x0.min(x1));
        self.max_x = self.max_x.max(x0.max(x1));
        self.min_y = self.min_y.min(y0);
        self.max_y = self.max_y.max(y1);
        self.edges.push(Edge { x0, y0, x1, y1, dir });
    }

    /// The pixel bounds coverage can reach (before any destination clipping).
    pub fn bounds(&self) -> IRect {
        if self.edges.is_empty() {
            return IRect::EMPTY;
        }
        let x1 = if self.open_right { self.w } else { (self.max_x >> FRAC_BITS) + 1 };
        let r = IRect::new(self.min_x >> FRAC_BITS, self.min_y >> FRAC_BITS, x1, (self.max_y + ONE - 1) >> FRAC_BITS);
        r.intersect(&IRect::new(0, 0, self.w, self.h))
    }

    /// Rasterize the accumulated edges with `rule`, restricted to `clip`, calling
    /// `span(y, x0, coverage)` for each row with any coverage (zero entries may appear inside a
    /// span). Rows arrive top to bottom.
    pub fn fill(&mut self, rule: FillRule, clip: IRect, mut span: impl FnMut(i32, i32, &[u8])) {
        let reg = self.bounds().intersect(&clip);
        if reg.is_empty() {
            return;
        }
        let even_odd = rule == FillRule::EvenOdd;
        let rw = reg.width();
        let stride = rw + 1; // index 0 is a virtual column collecting cover from the left
        let band_h = (CELL_BUDGET / stride).clamp(1, reg.height());
        if self.cells.len() < stride * band_h {
            self.cells.resize(stride * band_h, Cell::default());
        }
        if self.row_lo.len() < band_h {
            self.row_lo.resize(band_h, u32::MAX);
            self.row_hi.resize(band_h, 0);
        }
        if self.cov.len() < rw {
            self.cov.resize(rw, 0);
        }
        let Rasterizer { edges, cells, row_lo, row_hi, cov, .. } = self;
        let mut acc_cx = Accum { cells, row_lo, row_hi, stride, cx0: reg.x0, cx1: reg.x1 };
        let mut by0 = reg.y0;
        while by0 < reg.y1 {
            let by1 = (by0 + band_h as i32).min(reg.y1);
            for e in edges.iter() {
                if e.y1 > by0 << FRAC_BITS && e.y0 < by1 << FRAC_BITS {
                    acc_cx.edge(e, by0, by1);
                }
            }
            for y in by0..by1 {
                let r = (y - by0) as usize;
                let base = r * stride;
                let virt = acc_cx.cells[base].cover;
                let (lo, hi) = (acc_cx.row_lo[r] as usize, acc_cx.row_hi[r] as usize);
                let touched = lo <= hi;
                if virt == 0 && !touched {
                    continue;
                }
                let start = if virt != 0 { 1 } else { lo };
                let last = if touched { hi } else { 0 };
                let mut acc = virt as i64;
                let mut i = start;
                while i <= last {
                    let c = acc_cx.cells[base + i];
                    acc += c.cover as i64;
                    cov[i - start] = coverage(acc * 512 - c.area as i64, even_odd);
                    i += 1;
                }
                let mut end = last.max(start - 1) + 1; // exclusive cell index
                if acc != 0 {
                    let c = coverage(acc * 512, even_odd);
                    cov[end - start..stride - start].fill(c);
                    end = stride;
                }
                if end > start {
                    span(y, reg.x0 + start as i32 - 1, &cov[..end - start]);
                }
                acc_cx.cells[base] = Cell::default();
                if touched {
                    acc_cx.cells[base + lo..=base + hi].fill(Cell::default());
                }
                acc_cx.row_lo[r] = u32::MAX;
                acc_cx.row_hi[r] = 0;
            }
            by0 = by1;
        }
    }
}

/// The cell accumulator for one band: disjoint borrows of the rasterizer's buffers.
struct Accum<'a> {
    cells: &'a mut [Cell],
    row_lo: &'a mut [u32],
    row_hi: &'a mut [u32],
    stride: usize,
    cx0: i32,
    cx1: i32,
}

impl Accum<'_> {
    /// Walk the rows of `e` inside the band `[by0, by1)`. Each row's entry and exit x are a pure
    /// function of the edge and the row, so banding never changes the result.
    fn edge(&mut self, e: &Edge, by0: i32, by1: i32) {
        let (dx, dy) = ((e.x1 - e.x0) as i64, (e.y1 - e.y0) as i64);
        let r0 = (e.y0 >> FRAC_BITS).max(by0);
        let r1 = ((e.y1 + ONE - 1) >> FRAC_BITS).min(by1);
        for r in r0..r1 {
            let top = r << FRAC_BITS;
            let ya = e.y0.max(top);
            let yb = e.y1.min(top + ONE);
            if ya >= yb {
                continue;
            }
            let xa = if ya == e.y0 { e.x0 } else { e.x0 + div_round(dx * (ya - e.y0) as i64, dy) as i32 };
            let xb = if yb == e.y1 { e.x1 } else { e.x0 + div_round(dx * (yb - e.y0) as i64, dy) as i32 };
            self.row_piece((r - by0) as usize, xa, ya - top, xb, yb - top, e.dir);
        }
    }

    /// Distribute a piece of an edge inside one row (`fya <= fyb`, both in `[0, 256]`) over the
    /// cells it crosses. Columns left of the region only contribute cover (to the virtual column);
    /// columns right of it are dropped.
    fn row_piece(&mut self, row: usize, xa: i32, fya: i32, xb: i32, fyb: i32, dir: i32) {
        let (ca, cb) = (xa >> FRAC_BITS, xb >> FRAC_BITS);
        if ca == cb {
            self.cell(row, ca, xa - (ca << FRAC_BITS), xb - (ca << FRAC_BITS), fyb - fya, dir);
            return;
        }
        let (sx, sy) = ((xb - xa) as i64, (fyb - fya) as i64);
        let y_at = |x: i32| fya + div_round(sy * (x - xa) as i64, sx) as i32;
        if xa < xb {
            if cb < self.cx0 {
                self.cell(row, cb, 0, 0, fyb - fya, dir);
                return;
            }
            if ca >= self.cx1 {
                return;
            }
            let (mut c, mut x, mut y) = (ca, xa, fya);
            if c < self.cx0 {
                let bx = self.cx0 << FRAC_BITS;
                let ny = y_at(bx);
                self.cell(row, c, 0, 0, ny - y, dir);
                (c, x, y) = (self.cx0, bx, ny);
            }
            while c < cb {
                if c >= self.cx1 {
                    return;
                }
                let bx = (c + 1) << FRAC_BITS;
                let ny = y_at(bx);
                self.cell(row, c, x - (c << FRAC_BITS), ONE, ny - y, dir);
                (c, x, y) = (c + 1, bx, ny);
            }
            self.cell(row, cb, x - (cb << FRAC_BITS), xb - (cb << FRAC_BITS), fyb - y, dir);
        } else {
            if ca < self.cx0 {
                self.cell(row, ca, 0, 0, fyb - fya, dir);
                return;
            }
            if cb >= self.cx1 {
                return;
            }
            let (mut c, mut x, mut y) = (ca, xa, fya);
            if c >= self.cx1 {
                let bx = self.cx1 << FRAC_BITS;
                let ny = y_at(bx);
                (c, x, y) = (self.cx1 - 1, bx, ny);
            }
            while c > cb {
                if c < self.cx0 {
                    self.cell(row, c, 0, 0, fyb - y, dir);
                    return;
                }
                let bx = c << FRAC_BITS;
                let ny = y_at(bx);
                self.cell(row, c, x - bx, 0, ny - y, dir);
                (c, x, y) = (c - 1, bx, ny);
            }
            self.cell(row, cb, x - (cb << FRAC_BITS), xb - (cb << FRAC_BITS), fyb - y, dir);
        }
    }

    #[inline]
    fn cell(&mut self, row: usize, c: i32, fx1: i32, fx2: i32, dy: i32, dir: i32) {
        if dy == 0 || c >= self.cx1 {
            return;
        }
        let base = row * self.stride;
        let cover = dir * dy;
        if c < self.cx0 {
            let v = &mut self.cells[base];
            v.cover = v.cover.wrapping_add(cover);
            return;
        }
        let i = (c - self.cx0) as usize + 1;
        let cell = &mut self.cells[base + i];
        cell.cover = cell.cover.wrapping_add(cover);
        cell.area = cell.area.wrapping_add(cover * (fx1 + fx2));
        let i = i as u32;
        if i < self.row_lo[row] {
            self.row_lo[row] = i;
        }
        if i > self.row_hi[row] {
            self.row_hi[row] = i;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(r: &mut Rasterizer, rule: FillRule, w: u32, h: u32) -> Vec<u8> {
        let mut out = vec![0u8; (w * h) as usize];
        r.fill(rule, IRect::new(0, 0, w as i32, h as i32), |y, x0, cov| {
            for (i, &c) in cov.iter().enumerate() {
                out[(y as u32 * w) as usize + x0 as usize + i] = c;
            }
        });
        out
    }

    #[test]
    fn div_round_rounds_to_nearest() {
        assert_eq!(div_round(5, 2), 3);
        assert_eq!(div_round(-5, 2), -2);
        assert_eq!(div_round(4, 3), 1);
        assert_eq!(div_round(5, -3), -2);
        assert_eq!(div_round(0, 7), 0);
    }

    #[test]
    fn integer_square_is_exact() {
        let mut r = Rasterizer::new(8, 8);
        r.polygon(&[Vec2::new(2.0, 2.0), Vec2::new(5.0, 2.0), Vec2::new(5.0, 6.0), Vec2::new(2.0, 6.0)]);
        let px = collect(&mut r, FillRule::NonZero, 8, 8);
        for y in 0..8 {
            for x in 0..8 {
                let inside = (2..5).contains(&x) && (2..6).contains(&y);
                assert_eq!(px[y * 8 + x], if inside { 255 } else { 0 }, "({x},{y})");
            }
        }
    }

    #[test]
    fn winding_direction_does_not_matter() {
        let sq = [Vec2::new(1.25, 1.5), Vec2::new(6.5, 1.5), Vec2::new(6.5, 5.75), Vec2::new(1.25, 5.75)];
        let rev: Vec<Vec2> = sq.iter().rev().copied().collect();
        let mut a = Rasterizer::new(8, 8);
        a.polygon(&sq);
        let mut b = Rasterizer::new(8, 8);
        b.polygon(&rev);
        assert_eq!(collect(&mut a, FillRule::NonZero, 8, 8), collect(&mut b, FillRule::NonZero, 8, 8));
    }

    #[test]
    fn clipping_rect_does_not_change_coverage_inside_it() {
        let tri = [Vec2::new(-3.3, 1.1), Vec2::new(17.7, 4.9), Vec2::new(5.2, 14.6)];
        let mut a = Rasterizer::new(16, 16);
        a.polygon(&tri);
        let full = collect(&mut a, FillRule::NonZero, 16, 16);
        let mut b = Rasterizer::new(16, 16);
        b.polygon(&tri);
        let clip = IRect::new(3, 2, 11, 13);
        let mut part = vec![0u8; 256];
        b.fill(FillRule::NonZero, clip, |y, x0, cov| {
            for (i, &c) in cov.iter().enumerate() {
                let x = x0 as usize + i;
                assert!((3..11).contains(&x) && (2..13).contains(&y));
                part[y as usize * 16 + x] = c;
            }
        });
        for y in 2..13 {
            for x in 3..11 {
                assert_eq!(part[y * 16 + x], full[y * 16 + x], "({x},{y})");
            }
        }
    }

    #[test]
    fn banding_does_not_change_coverage() {
        // A shape tall and wide enough to need several bands.
        let (w, h) = (1200u32, 700u32);
        let poly = [Vec2::new(3.3, 2.7), Vec2::new(1190.2, 350.5), Vec2::new(20.9, 698.1)];
        let mut a = Rasterizer::new(w, h);
        a.polygon(&poly);
        assert!(CELL_BUDGET / (a.bounds().width() + 1) < h as usize, "test needs multiple bands");
        let banded = collect(&mut a, FillRule::NonZero, w, h);
        // Rasterize strip by strip through the clip rect (each strip fits in one band).
        let mut b = Rasterizer::new(w, h);
        b.polygon(&poly);
        let mut strips = vec![0u8; (w * h) as usize];
        for y0 in (0..h as i32).step_by(7) {
            b.fill(FillRule::NonZero, IRect::new(0, y0, w as i32, y0 + 7), |y, x0, cov| {
                for (i, &c) in cov.iter().enumerate() {
                    strips[(y as u32 * w) as usize + x0 as usize + i] = c;
                }
            });
        }
        assert!(banded == strips);
    }

    #[test]
    fn shapes_off_the_left_edge_still_fill_to_their_right() {
        let mut r = Rasterizer::new(10, 4);
        r.polygon(&[Vec2::new(-100.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(-100.0, 4.0)]);
        let px = collect(&mut r, FillRule::NonZero, 10, 4);
        assert_eq!(&px[0..10], &[255, 255, 255, 255, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn shapes_off_the_right_edge_fill_to_the_border() {
        let mut r = Rasterizer::new(10, 2);
        r.polygon(&[Vec2::new(6.0, 0.0), Vec2::new(300.0, 0.0), Vec2::new(300.0, 2.0), Vec2::new(6.0, 2.0)]);
        let px = collect(&mut r, FillRule::NonZero, 10, 2);
        assert_eq!(&px[10..20], &[0, 0, 0, 0, 0, 0, 255, 255, 255, 255]);
    }
}
