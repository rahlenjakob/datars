//! Graph communities (Louvain modularity) and orders that keep them together — for colouring a
//! network by its clusters and ordering the rows of an adjacency matrix or the nodes of an arc
//! diagram so the clusters show as blocks.

use crate::util::mag;

/// A graph's weighted adjacency with duplicate links merged: `adj[i]` lists `(j, weight)` sorted
/// by `j`, `self_loops[i]` the weight of links from `i` to itself.
struct Graph {
    adj: Vec<Vec<(usize, f64)>>,
    self_loops: Vec<f64>,
}

impl Graph {
    fn new(n: usize, links: &[(usize, usize, f64)]) -> Graph {
        let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
        let mut self_loops = vec![0.0; n];
        for &(a, b, w) in links {
            let w = if w.is_finite() { mag(w) } else { 1.0 };
            if a >= n || b >= n || w == 0.0 {
                continue;
            }
            if a == b {
                self_loops[a] += w;
            } else {
                adj[a].push((b, w));
                adj[b].push((a, w));
            }
        }
        for list in &mut adj {
            list.sort_by_key(|x| x.0);
            let mut merged: Vec<(usize, f64)> = Vec::with_capacity(list.len());
            for &(j, w) in list.iter() {
                match merged.last_mut() {
                    Some(last) if last.0 == j => last.1 += w,
                    _ => merged.push((j, w)),
                }
            }
            *list = merged;
        }
        Graph { adj, self_loops }
    }

    /// Weighted degree: links to others plus twice the self-loops (a loop has two ends).
    fn degree(&self, i: usize) -> f64 {
        self.adj[i].iter().map(|x| x.1).sum::<f64>() + 2.0 * self.self_loops[i]
    }
}

/// Weighted degree of each node: the sum of the weights of its links (a link to itself counts
/// twice; links with an endpoint out of range are ignored; a non-finite weight counts as 1, a
/// negative one as 0).
pub fn degrees(n: usize, links: &[(usize, usize, f64)]) -> Vec<f64> {
    let g = Graph::new(n, links);
    (0..n).map(|i| g.degree(i)).collect()
}

/// Communities of an undirected weighted graph by modularity (the Louvain method: move each node to
/// the neighbouring community that gains the most, until no move gains; merge each community into
/// one node; repeat on the merged graph). `resolution` above 1 favours smaller communities, below 1
/// larger ones (1 is classic modularity).
///
/// Returns each node's community, numbered from 0 by size (largest first; ties by the lowest node
/// index in them). Nodes without links are communities of their own.
///
/// Deterministic: nodes are visited in index order, neighbouring communities compared in index
/// order, and a node moves only for a strictly greater gain (ties keep it where it is, else go to
/// the lowest community index). Links as in [`degrees`].
pub fn communities(n: usize, links: &[(usize, usize, f64)], resolution: f64) -> Vec<usize> {
    let res = if resolution.is_finite() && resolution > 0.0 { resolution } else { 1.0 };
    let mut g = Graph::new(n, links);
    // member[i]: the node of the current (merged) graph that original node i belongs to.
    let mut member: Vec<usize> = (0..n).collect();
    let m2: f64 = (0..n).map(|i| g.degree(i)).sum();
    if m2 > 0.0 {
        for _level in 0..32 {
            let size = g.adj.len();
            let k: Vec<f64> = (0..size).map(|i| g.degree(i)).collect();
            let mut comm: Vec<usize> = (0..size).collect();
            let mut tot = k.clone();
            let mut moved_any = false;
            let mut weights: Vec<f64> = vec![0.0; size];
            let mut touched: Vec<usize> = Vec::new();
            for _pass in 0..64 {
                let mut moved = false;
                for i in 0..size {
                    let ci = comm[i];
                    tot[ci] -= k[i];
                    for &(j, w) in &g.adj[i] {
                        let c = comm[j];
                        if weights[c] == 0.0 {
                            touched.push(c);
                        }
                        weights[c] += w;
                    }
                    touched.sort_unstable();
                    touched.dedup();
                    let gain = |c: usize, w: f64| w - res * tot[c] * k[i] / m2;
                    let mut best = ci;
                    let mut best_gain = gain(ci, weights[ci]);
                    for &c in &touched {
                        let gc = gain(c, weights[c]);
                        if gc > best_gain + 1e-12 {
                            best = c;
                            best_gain = gc;
                        }
                    }
                    for &c in &touched {
                        weights[c] = 0.0;
                    }
                    touched.clear();
                    tot[best] += k[i];
                    if best != ci {
                        comm[i] = best;
                        moved = true;
                        moved_any = true;
                    }
                }
                if !moved {
                    break;
                }
            }
            if !moved_any {
                break;
            }
            // Renumber communities by first appearance and merge the graph.
            let mut id = vec![usize::MAX; size];
            let mut next = 0;
            for i in 0..size {
                if id[comm[i]] == usize::MAX {
                    id[comm[i]] = next;
                    next += 1;
                }
            }
            let mut merged_links: Vec<(usize, usize, f64)> = Vec::new();
            for i in 0..size {
                let a = id[comm[i]];
                if g.self_loops[i] > 0.0 {
                    merged_links.push((a, a, g.self_loops[i]));
                }
                for &(j, w) in &g.adj[i] {
                    if j > i {
                        merged_links.push((a, id[comm[j]], w));
                    }
                }
            }
            for x in member.iter_mut() {
                *x = id[comm[*x]];
            }
            g = Graph::new(next, &merged_links);
        }
    }
    // Number by size, largest first; ties by lowest member.
    let groups = g.adj.len();
    let mut count = vec![0usize; groups];
    let mut first = vec![usize::MAX; groups];
    for (i, &c) in member.iter().enumerate() {
        count[c] += 1;
        first[c] = first[c].min(i);
    }
    let mut order: Vec<usize> = (0..groups).filter(|&c| count[c] > 0).collect();
    order.sort_by(|&a, &b| count[b].cmp(&count[a]).then(first[a].cmp(&first[b])));
    let mut rank = vec![0usize; groups];
    for (r, &c) in order.iter().enumerate() {
        rank[c] = r;
    }
    member.iter().map(|&c| rank[c]).collect()
}

/// A position for each node that keeps communities together: by community number (as returned by
/// [`communities`], largest first), then by weighted degree (highest first), then by index.
/// Returns `position[i]` for node `i` (a permutation of `0..n`).
pub fn community_order(community: &[usize], degree: &[f64]) -> Vec<usize> {
    let n = community.len();
    let deg = |i: usize| degree.get(i).copied().filter(|d| d.is_finite()).unwrap_or(0.0);
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&a, &b| community[a].cmp(&community[b]).then(datars_math::total_cmp(deg(b), deg(a))).then(a.cmp(&b)));
    let mut pos = vec![0usize; n];
    for (p, &i) in idx.iter().enumerate() {
        pos[i] = p;
    }
    pos
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two 5-cliques joined by one bridge, plus an isolated node.
    fn two_cliques() -> (usize, Vec<(usize, usize, f64)>) {
        let mut links = Vec::new();
        for base in [0, 5] {
            for a in 0..5 {
                for b in a + 1..5 {
                    links.push((base + a, base + b, 1.0));
                }
            }
        }
        links.push((4, 5, 1.0));
        (11, links)
    }

    #[test]
    fn finds_the_two_cliques() {
        let (n, links) = two_cliques();
        let c = communities(n, &links, 1.0);
        assert!(c[..5].iter().all(|&x| x == c[0]), "{c:?}");
        assert!(c[5..10].iter().all(|&x| x == c[5]), "{c:?}");
        assert_ne!(c[0], c[5]);
        assert_eq!((c[0], c[5]), (0, 1), "equal sizes: numbered by lowest member");
        assert_eq!(c[10], 2, "the isolated node alone, last (smallest)");
        assert_eq!(communities(n, &links, 1.0), c, "deterministic");
        // Input order of the links doesn't matter.
        let mut rev = links.clone();
        rev.reverse();
        assert_eq!(communities(n, &rev, 1.0), c);
    }

    #[test]
    fn degrees_and_order() {
        let (n, links) = two_cliques();
        let d = degrees(n, &links);
        assert_eq!(d[0], 4.0);
        assert_eq!(d[4], 5.0, "the bridge end");
        assert_eq!(d[10], 0.0);
        assert_eq!(degrees(2, &[(0, 0, 2.0), (0, 1, f64::NAN), (1, 7, 1.0)]), vec![5.0, 1.0]);
        let c = communities(n, &links, 1.0);
        let pos = community_order(&c, &d);
        assert_eq!(pos[4], 0, "the best-connected member of the first community first");
        assert_eq!(pos[10], 10, "the isolated node last");
        let mut sorted = pos.clone();
        sorted.sort();
        assert_eq!(sorted, (0..n).collect::<Vec<_>>(), "a permutation");
        // Community members are contiguous.
        for i in 0..n {
            for j in 0..n {
                if c[i] < c[j] {
                    assert!(pos[i] < pos[j]);
                }
            }
        }
    }

    #[test]
    fn degenerate() {
        assert!(communities(0, &[], 1.0).is_empty());
        assert_eq!(communities(3, &[], 1.0), vec![0, 1, 2], "no links: each alone");
        assert_eq!(communities(2, &[(0, 1, 0.0), (0, 9, 1.0)], f64::NAN), vec![0, 1]);
        let c = communities(4, &[(0, 1, 1.0), (1, 0, 1.0), (2, 3, 5.0), (2, 2, 1.0)], 1.0);
        assert_eq!(c[0], c[1]);
        assert_eq!(c[2], c[3]);
        assert_ne!(c[0], c[2]);
        assert!(community_order(&[], &[]).is_empty());
    }
}
