//! Sankey diagrams: columns by depth, node heights by flow, iterative relaxation (after d3-sankey).

use crate::util::mag;
use datars_math::{m, total_cmp, Rect};
use serde::{Deserialize, Serialize};

/// Which column a node goes in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SankeyAlign {
    /// Column = depth (longest path from a source).
    Left,
    /// Column = last − height (longest path to a sink).
    Right,
    /// Like `Left`, but sources with no inflow move right to just before their first target.
    Center,
    /// Like `Left`, but sinks (no outflow) go to the last column.
    #[default]
    Justify,
}

/// Options for [`sankey`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SankeyOptions {
    /// Width of each node rect.
    pub node_width: f64,
    /// Vertical gap between nodes in a column (shrunk if the tallest column can't fit it).
    pub node_padding: f64,
    /// Relaxation passes (each is a right-to-left and a left-to-right sweep). 6 is d3's default.
    pub iterations: usize,
    pub align: SankeyAlign,
}

impl Default for SankeyOptions {
    fn default() -> Self {
        SankeyOptions { node_width: 24.0, node_padding: 8.0, iterations: 6, align: SankeyAlign::Justify }
    }
}

/// A link's geometry: a ribbon of `width` from the source's right edge `x0` (centre line at `y0`)
/// to the target's left edge `x1` (centre line at `y1`). Draw it as a horizontal-tangent cubic.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LinkGeom {
    pub x0: f64,
    pub x1: f64,
    pub y0: f64,
    pub y1: f64,
    pub width: f64,
}

/// The result of [`sankey`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SankeyLayout {
    /// One rect per node.
    pub nodes: Vec<Rect>,
    /// One geometry per input link (zeroed for links in `cyclic_links` / `invalid_links`).
    pub links: Vec<LinkGeom>,
    /// Each node's value: max(total inflow, total outflow).
    pub node_values: Vec<f64>,
    /// Each node's column (0-based).
    pub columns: Vec<usize>,
    /// Links that closed a cycle (including self-links). They're left out of the layout — a Sankey
    /// is a DAG — and reported so the caller can warn or draw them differently.
    pub cyclic_links: Vec<usize>,
    /// Links whose source or target index is out of range (left out).
    pub invalid_links: Vec<usize>,
}

/// Lays out a Sankey diagram of `n` nodes and `links` `(source, target, value)` inside `rect`.
///
/// Steps (d3-sankey's): node value = max(inflow, outflow); depth = longest path from a source;
/// column per `opts.align`; node heights share one scale so the fullest column fits `rect.h`;
/// then `iterations` relaxation passes pull each node toward the weighted centre of its neighbours
/// and resolve collisions within columns; finally links are stacked at each node ordered by the
/// other end's position, so ribbons don't cross needlessly at the nodes.
///
/// Totality: links with non-finite or negative values count as 0 (zero-width ribbons). Links
/// that reference a missing node are reported in `invalid_links`; links that close a cycle (found
/// by a DFS in index order — so which link of a cycle gets cut is deterministic) are reported in
/// `cyclic_links`; both get zeroed geometry and don't affect the rest. Isolated nodes get zero
/// height. Deterministic: stable sorts with index tie-breaks; no hashing.
pub fn sankey(n: usize, links: &[(usize, usize, f64)], rect: Rect, opts: &SankeyOptions) -> SankeyLayout {
    let fin = |v: f64| if v.is_finite() { v } else { 0.0 };
    let (x0, y0) = (fin(rect.x), fin(rect.y));
    let (x1, y1) = (x0 + mag(rect.w), y0 + mag(rect.h));
    let dx = mag(opts.node_width).min(x1 - x0);
    let mut out = SankeyLayout {
        nodes: vec![Rect::new(x0, y0, 0.0, 0.0); n],
        links: vec![LinkGeom::default(); links.len()],
        node_values: vec![0.0; n],
        columns: vec![0; n],
        ..Default::default()
    };
    let value: Vec<f64> = links.iter().map(|l| mag(l.2)).collect();
    let src: Vec<usize> = links.iter().map(|l| l.0).collect();
    let dst: Vec<usize> = links.iter().map(|l| l.1).collect();

    // Validate, then cut cycles with an iterative DFS (back edges = cyclic links).
    let mut out_edges: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut active = vec![true; links.len()];
    for (i, &(s, t, _)) in links.iter().enumerate() {
        if s >= n || t >= n {
            out.invalid_links.push(i);
            active[i] = false;
        } else {
            out_edges[s].push(i);
        }
    }
    {
        const WHITE: u8 = 0;
        const GREY: u8 = 1;
        const BLACK: u8 = 2;
        let mut color = vec![WHITE; n];
        for start in 0..n {
            if color[start] != WHITE {
                continue;
            }
            let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
            color[start] = GREY;
            while let Some(&mut (v, ref mut k)) = stack.last_mut() {
                if *k < out_edges[v].len() {
                    let e = out_edges[v][*k];
                    *k += 1;
                    let t = dst[e];
                    match color[t] {
                        WHITE => {
                            color[t] = GREY;
                            stack.push((t, 0));
                        }
                        GREY => {
                            out.cyclic_links.push(e);
                            active[e] = false;
                        }
                        _ => {}
                    }
                } else {
                    color[v] = BLACK;
                    stack.pop();
                }
            }
        }
        out.cyclic_links.sort_unstable();
    }
    let mut source_links: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut target_links: Vec<Vec<usize>> = vec![Vec::new(); n];
    for i in (0..links.len()).filter(|&i| active[i]) {
        source_links[src[i]].push(i);
        target_links[dst[i]].push(i);
    }
    if n == 0 {
        return out;
    }

    // Node values.
    let node_value: Vec<f64> = (0..n)
        .map(|v| {
            let inflow: f64 = target_links[v].iter().map(|&l| value[l]).sum();
            let outflow: f64 = source_links[v].iter().map(|&l| value[l]).sum();
            inflow.max(outflow)
        })
        .collect();
    out.node_values = node_value.clone();

    // Depths (longest path from a source) and heights (longest path to a sink) in topological order.
    let topo = {
        let mut indeg: Vec<usize> = (0..n).map(|v| target_links[v].len()).collect();
        let mut order = Vec::with_capacity(n);
        let mut ready: std::collections::BTreeSet<usize> = (0..n).filter(|&v| indeg[v] == 0).collect();
        while let Some(v) = ready.pop_first() {
            order.push(v);
            for &l in &source_links[v] {
                let t = dst[l];
                indeg[t] -= 1;
                if indeg[t] == 0 {
                    ready.insert(t);
                }
            }
        }
        order
    };
    let mut depth = vec![0usize; n];
    for &v in &topo {
        for &l in &source_links[v] {
            depth[dst[l]] = depth[dst[l]].max(depth[v] + 1);
        }
    }
    let mut height = vec![0usize; n];
    for &v in topo.iter().rev() {
        for &l in &source_links[v] {
            height[v] = height[v].max(height[dst[l]] + 1);
        }
    }
    let cols = depth.iter().copied().max().unwrap_or(0) + 1;
    let column: Vec<usize> = (0..n)
        .map(|v| {
            let c = match opts.align {
                SankeyAlign::Left => depth[v],
                SankeyAlign::Right => cols - 1 - height[v].min(cols - 1),
                SankeyAlign::Justify => {
                    if source_links[v].is_empty() {
                        cols - 1
                    } else {
                        depth[v]
                    }
                }
                SankeyAlign::Center => {
                    if !target_links[v].is_empty() {
                        depth[v]
                    } else if !source_links[v].is_empty() {
                        source_links[v].iter().map(|&l| depth[dst[l]]).min().unwrap_or(1).saturating_sub(1)
                    } else {
                        0
                    }
                }
            };
            c.min(cols - 1)
        })
        .collect();
    out.columns = column.clone();
    let kx = if cols > 1 { (x1 - x0 - dx) / (cols - 1) as f64 } else { 0.0 };
    let mut columns: Vec<Vec<usize>> = vec![Vec::new(); cols];
    for v in 0..n {
        columns[column[v]].push(v);
    }
    let max_len = columns.iter().map(|c| c.len()).max().unwrap_or(1);
    let py = if max_len > 1 { mag(opts.node_padding).min((y1 - y0) / (max_len - 1) as f64) } else { mag(opts.node_padding) };

    // Initial breadths: one scale for all columns, each column stacked then spread.
    let ky = columns
        .iter()
        .filter_map(|c| {
            let total: f64 = c.iter().map(|&v| node_value[v]).sum();
            (total > 0.0).then(|| (y1 - y0 - (c.len() as f64 - 1.0) * py) / total)
        })
        .fold(f64::INFINITY, f64::min);
    let ky = if ky.is_finite() { ky.max(0.0) } else { 0.0 };
    let width: Vec<f64> = value.iter().map(|&v| v * ky).collect();
    let mut ny0 = vec![y0; n];
    let mut ny1 = vec![y0; n];
    for c in &columns {
        let mut y = y0;
        for &v in c {
            ny0[v] = y;
            ny1[v] = y + node_value[v] * ky;
            y = ny1[v] + py;
        }
        let spread = (y1 - y + py) / (c.len() as f64 + 1.0);
        for (i, &v) in c.iter().enumerate() {
            ny0[v] += spread * (i + 1) as f64;
            ny1[v] += spread * (i + 1) as f64;
        }
    }
    let mut st = State { src: &src, dst: &dst, width: &width, source_links, target_links, y0: ny0, y1: ny1, py };
    for v in 0..n {
        st.sort_links_of(v);
    }

    let iterations = opts.iterations;
    for i in 0..iterations {
        let alpha = m::pow(0.99, i as f64);
        let beta = (1.0 - alpha).max((i + 1) as f64 / iterations as f64);
        // Right to left: each node toward the ideal positions implied by its outflows.
        for ci in (0..cols.saturating_sub(1)).rev() {
            for k in 0..columns[ci].len() {
                let s = columns[ci][k];
                let (mut y, mut w) = (0.0, 0.0);
                for &l in &st.source_links[s] {
                    let t = dst[l];
                    let v = value[l] * (column[t] as f64 - column[s] as f64);
                    y += st.source_top(s, t) * v;
                    w += v;
                }
                if w > 0.0 {
                    let d = (y / w - st.y0[s]) * alpha;
                    st.y0[s] += d;
                    st.y1[s] += d;
                    st.reorder_node_links(s);
                }
            }
            st.settle_column(&mut columns[ci], beta, y0, y1);
        }
        // Left to right: each node toward the ideal positions implied by its inflows.
        for ci in 1..cols {
            for k in 0..columns[ci].len() {
                let t = columns[ci][k];
                let (mut y, mut w) = (0.0, 0.0);
                for &l in &st.target_links[t] {
                    let s = src[l];
                    let v = value[l] * (column[t] as f64 - column[s] as f64);
                    y += st.target_top(s, t) * v;
                    w += v;
                }
                if w > 0.0 {
                    let d = (y / w - st.y0[t]) * alpha;
                    st.y0[t] += d;
                    st.y1[t] += d;
                    st.reorder_node_links(t);
                }
            }
            st.settle_column(&mut columns[ci], beta, y0, y1);
        }
    }

    // Link breadths: stack each node's links in their sorted order.
    for v in 0..n {
        let mut ys = st.y0[v];
        for &l in &st.source_links[v] {
            out.links[l].y0 = ys + width[l] / 2.0;
            ys += width[l];
        }
        let mut yt = st.y0[v];
        for &l in &st.target_links[v] {
            out.links[l].y1 = yt + width[l] / 2.0;
            yt += width[l];
        }
    }
    for v in 0..n {
        let nx = x0 + column[v] as f64 * kx;
        out.nodes[v] = Rect::new(nx, st.y0[v], dx, st.y1[v] - st.y0[v]);
    }
    for l in (0..links.len()).filter(|&l| active[l]) {
        out.links[l].x0 = out.nodes[src[l]].x1();
        out.links[l].x1 = out.nodes[dst[l]].x;
        out.links[l].width = width[l];
    }
    out
}

/// Mutable layout state shared by the relaxation helpers.
struct State<'a> {
    src: &'a [usize],
    dst: &'a [usize],
    width: &'a [f64],
    source_links: Vec<Vec<usize>>,
    target_links: Vec<Vec<usize>>,
    y0: Vec<f64>,
    y1: Vec<f64>,
    py: f64,
}

impl State<'_> {
    fn sort_by_target(&mut self, v: usize) {
        let (y0, dst) = (&self.y0, self.dst);
        self.source_links[v].sort_by(|&a, &b| total_cmp(y0[dst[a]], y0[dst[b]]).then(a.cmp(&b)));
    }
    fn sort_by_source(&mut self, v: usize) {
        let (y0, src) = (&self.y0, self.src);
        self.target_links[v].sort_by(|&a, &b| total_cmp(y0[src[a]], y0[src[b]]).then(a.cmp(&b)));
    }
    fn sort_links_of(&mut self, v: usize) {
        self.sort_by_target(v);
        self.sort_by_source(v);
    }
    /// After node `v` moved: re-sort the link stacks at its neighbours.
    fn reorder_node_links(&mut self, v: usize) {
        for k in 0..self.target_links[v].len() {
            let s = self.src[self.target_links[v][k]];
            self.sort_by_target(s);
        }
        for k in 0..self.source_links[v].len() {
            let t = self.dst[self.source_links[v][k]];
            self.sort_by_source(t);
        }
    }
    /// The `target.y0` that would make the link from `s` to `t` perfectly horizontal.
    fn target_top(&self, s: usize, t: usize) -> f64 {
        let mut y = self.y0[s] - (self.source_links[s].len() as f64 - 1.0) * self.py / 2.0;
        for &l in &self.source_links[s] {
            if self.dst[l] == t {
                break;
            }
            y += self.width[l] + self.py;
        }
        for &l in &self.target_links[t] {
            if self.src[l] == s {
                break;
            }
            y -= self.width[l];
        }
        y
    }
    /// The `source.y0` that would make the link from `s` to `t` perfectly horizontal.
    fn source_top(&self, s: usize, t: usize) -> f64 {
        let mut y = self.y0[t] - (self.target_links[t].len() as f64 - 1.0) * self.py / 2.0;
        for &l in &self.target_links[t] {
            if self.src[l] == s {
                break;
            }
            y += self.width[l] + self.py;
        }
        for &l in &self.source_links[s] {
            if self.dst[l] == t {
                break;
            }
            y -= self.width[l];
        }
        y
    }
    /// Re-sorts a column by position and pushes overlapping nodes apart (from the middle node out,
    /// then from both ends back in), by a fraction `beta` of the overlap.
    fn settle_column(&mut self, col: &mut [usize], beta: f64, top: f64, bottom: f64) {
        if col.is_empty() {
            return;
        }
        let y0 = &self.y0;
        col.sort_by(|&a, &b| total_cmp(y0[a], y0[b]).then(a.cmp(&b)));
        let i = col.len() / 2;
        let subject = col[i];
        let (sy0, sy1) = (self.y0[subject], self.y1[subject]);
        self.push_up(col, sy0 - self.py, i as isize - 1, beta);
        self.push_down(col, sy1 + self.py, i + 1, beta);
        self.push_up(col, bottom, col.len() as isize - 1, beta);
        self.push_down(col, top, 0, beta);
    }
    fn push_down(&mut self, col: &[usize], mut y: f64, from: usize, beta: f64) {
        for &v in col.iter().skip(from) {
            let d = (y - self.y0[v]) * beta;
            if d > 1e-6 {
                self.y0[v] += d;
                self.y1[v] += d;
            }
            y = self.y1[v] + self.py;
        }
    }
    fn push_up(&mut self, col: &[usize], mut y: f64, from: isize, beta: f64) {
        let mut i = from;
        while i >= 0 {
            let v = col[i as usize];
            let d = (self.y1[v] - y) * beta;
            if d > 1e-6 {
                self.y0[v] -= d;
                self.y1[v] -= d;
            }
            y = self.y0[v] - self.py;
            i -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-6;

    fn energy() -> (usize, Vec<(usize, usize, f64)>) {
        // 0 Coal, 1 Gas, 2 Power, 3 Homes, 4 Losses, 5 Industry
        (6, vec![(0, 2, 30.0), (1, 2, 20.0), (2, 3, 35.0), (2, 4, 15.0), (1, 3, 10.0), (1, 5, 5.0)])
    }

    fn check(n: usize, links: &[(usize, usize, f64)], l: &SankeyLayout, rect: Rect) {
        let skip = |i: usize| l.cyclic_links.contains(&i) || l.invalid_links.contains(&i);
        let ky = {
            // Recover the scale from any positive link.
            let (i, _) = links.iter().enumerate().find(|(i, x)| x.2 > 0.0 && !skip(*i)).unwrap();
            l.links[i].width / links[i].2
        };
        for v in 0..n {
            let r = l.nodes[v];
            assert!(r.y >= rect.y - EPS && r.y1() <= rect.y1() + EPS, "node {v} inside {r:?}");
            assert!((r.h - l.node_values[v] * ky).abs() < EPS, "node {v} height ∝ value");
            let flow = |end: fn(&(usize, usize, f64)) -> usize| -> f64 {
                links.iter().enumerate().filter(|(i, x)| end(x) == v && !skip(*i)).map(|(_, x)| mag(x.2)).sum()
            };
            let (inflow, outflow) = (flow(|x| x.1), flow(|x| x.0));
            assert!((l.node_values[v] - inflow.max(outflow)).abs() < EPS, "value conserved at {v}");
            // The links stacked on each side fill no more than the node.
            let side = |end: fn(&(usize, usize, f64)) -> usize| -> f64 {
                links.iter().enumerate().filter(|(i, x)| end(x) == v && !skip(*i)).map(|(i, _)| l.links[i].width).sum()
            };
            assert!(side(|x| x.1) <= r.h + EPS && side(|x| x.0) <= r.h + EPS);
        }
        for (i, &(s, t, w)) in links.iter().enumerate() {
            if skip(i) {
                continue;
            }
            let g = l.links[i];
            assert!((g.width - mag(w) * ky).abs() < EPS);
            let (a, b) = (l.nodes[s], l.nodes[t]);
            assert!(g.y0 - g.width / 2.0 >= a.y - EPS && g.y0 + g.width / 2.0 <= a.y1() + EPS, "link {i} within source span");
            assert!(g.y1 - g.width / 2.0 >= b.y - EPS && g.y1 + g.width / 2.0 <= b.y1() + EPS, "link {i} within target span");
            assert_eq!(g.x0, a.x1());
            assert_eq!(g.x1, b.x);
            assert!(g.x1 > g.x0, "links go left to right");
        }
        // Nodes in a column don't overlap.
        for v in 0..n {
            for u in v + 1..n {
                if l.columns[u] == l.columns[v] {
                    let (a, b) = (l.nodes[u], l.nodes[v]);
                    assert!(a.y1() <= b.y + EPS || b.y1() <= a.y + EPS, "{u} and {v} overlap");
                }
            }
        }
    }

    #[test]
    fn energy_flow_conserves_and_stays_in_spans() {
        let (n, links) = energy();
        let rect = Rect::new(0.0, 0.0, 800.0, 400.0);
        let l = sankey(n, &links, rect, &SankeyOptions::default());
        check(n, &links, &l, rect);
        assert_eq!(l.columns, vec![0, 0, 1, 2, 2, 2]);
        assert!(l.cyclic_links.is_empty() && l.invalid_links.is_empty());
        // Power's outflows sum to its inflows: its node is exactly as tall as either side.
        let power_in: f64 = [0usize, 1].iter().map(|&i| l.links[i].width).sum();
        assert!((power_in - l.nodes[2].h).abs() < EPS);
        assert_eq!(l.nodes[5].x1(), 800.0, "justify: sinks in the last column");
        assert_eq!(sankey(n, &links, rect, &SankeyOptions::default()), l, "deterministic");
    }

    #[test]
    fn alignments() {
        let (n, links) = energy();
        let rect = Rect::new(0.0, 0.0, 600.0, 300.0);
        for align in [SankeyAlign::Left, SankeyAlign::Right, SankeyAlign::Center, SankeyAlign::Justify] {
            let l = sankey(n, &links, rect, &SankeyOptions { align, ..Default::default() });
            check(n, &links, &l, rect);
        }
        let left = sankey(n, &links, rect, &SankeyOptions { align: SankeyAlign::Left, ..Default::default() });
        assert_eq!(left.columns, vec![0, 0, 1, 2, 2, 1]);
    }

    #[test]
    fn cycles_and_bad_links_are_reported() {
        let links = vec![(0, 1, 5.0), (1, 2, 5.0), (2, 0, 1.0), (1, 1, 2.0), (0, 9, 1.0), (2, 3, f64::NAN)];
        let rect = Rect::new(0.0, 0.0, 300.0, 200.0);
        let l = sankey(4, &links, rect, &SankeyOptions::default());
        assert_eq!(l.cyclic_links, vec![2, 3]);
        assert_eq!(l.invalid_links, vec![4]);
        assert_eq!(l.links[2], LinkGeom::default());
        assert_eq!(l.links[5].width, 0.0, "NaN value is a zero-width link");
        check(4, &links, &l, rect);
    }

    #[test]
    fn degenerate() {
        let e = sankey(0, &[], Rect::new(0.0, 0.0, 10.0, 10.0), &SankeyOptions::default());
        assert!(e.nodes.is_empty());
        let iso = sankey(3, &[], Rect::new(0.0, 0.0, 10.0, 10.0), &SankeyOptions::default());
        assert!(iso.nodes.iter().all(|r| r.h == 0.0 && r.y.is_finite()));
        let zero = sankey(2, &[(0, 1, 0.0)], Rect::new(0.0, 0.0, 10.0, 10.0), &SankeyOptions::default());
        assert!(zero.nodes.iter().all(|r| r.y.is_finite()));
    }
}
