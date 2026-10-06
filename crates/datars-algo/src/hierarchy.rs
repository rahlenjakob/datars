//! Parent-index hierarchies: the shared input of `treemap_nested`, `pack_hierarchy`, `partition`,
//! `tree` and `cluster`.
//!
//! A hierarchy is a slice `parents: &[Option<usize>]` — node `i`'s parent, `None` for a root.
//! Several roots form a forest (layouts treat them as children of an invisible root). Invalid input
//! is repaired, never rejected: a parent index out of range makes the node a root, and a cycle is
//! broken by turning the node that closes it into a root. Children keep input index order.

use crate::util::mag;

/// A validated parent-index hierarchy with children lists, roots, depths and a pre-order.
#[derive(Clone, Debug, PartialEq)]
pub struct Hierarchy {
    /// Repaired parent of each node (`None` for roots).
    pub parent: Vec<Option<usize>>,
    /// Children of each node, in input index order.
    pub children: Vec<Vec<usize>>,
    /// Root nodes, in input index order.
    pub roots: Vec<usize>,
    /// Depth of each node (roots are 0).
    pub depth: Vec<usize>,
    /// Every node in pre-order (a parent before its children; siblings in order).
    pub preorder: Vec<usize>,
}

impl Hierarchy {
    /// Builds and repairs a hierarchy from parent indices (see the module docs). O(n).
    pub fn new(parents: &[Option<usize>]) -> Hierarchy {
        let n = parents.len();
        let mut parent: Vec<Option<usize>> = parents.iter().map(|p| p.filter(|&q| q < n)).collect();
        // Break cycles: walk up from each unvisited node; meeting the current path again closes a
        // cycle, which we break at the last node walked (the one whose parent is on the path).
        const NEW: u8 = 0;
        const ON_PATH: u8 = 1;
        const DONE: u8 = 2;
        let mut state = vec![NEW; n];
        let mut path = Vec::new();
        for i in 0..n {
            if state[i] != NEW {
                continue;
            }
            let mut v = i;
            loop {
                state[v] = ON_PATH;
                path.push(v);
                match parent[v] {
                    None => break,
                    Some(p) if state[p] == DONE => break,
                    Some(p) if state[p] == ON_PATH => {
                        parent[v] = None;
                        break;
                    }
                    Some(p) => v = p,
                }
            }
            for &u in &path {
                state[u] = DONE;
            }
            path.clear();
        }
        let mut children = vec![Vec::new(); n];
        let mut roots = Vec::new();
        for (i, p) in parent.iter().enumerate() {
            match p {
                Some(p) => children[*p].push(i),
                None => roots.push(i),
            }
        }
        let mut depth = vec![0usize; n];
        let mut preorder = Vec::with_capacity(n);
        let mut stack: Vec<usize> = roots.iter().rev().copied().collect();
        while let Some(v) = stack.pop() {
            preorder.push(v);
            for &c in children[v].iter().rev() {
                depth[c] = depth[v] + 1;
                stack.push(c);
            }
        }
        Hierarchy { parent, children, roots, depth, preorder }
    }

    /// Number of nodes.
    pub fn len(&self) -> usize {
        self.parent.len()
    }

    /// Whether there are no nodes.
    pub fn is_empty(&self) -> bool {
        self.parent.is_empty()
    }

    /// Whether node `i` has no children.
    pub fn is_leaf(&self, i: usize) -> bool {
        self.children[i].is_empty()
    }

    /// The greatest depth (0 for an empty or flat hierarchy).
    pub fn height(&self) -> usize {
        self.depth.iter().copied().max().unwrap_or(0)
    }

    /// Every node in post-order (children before their parent; siblings in order).
    pub fn postorder(&self) -> Vec<usize> {
        let n = self.len();
        let mut out = Vec::with_capacity(n);
        let mut stack: Vec<(usize, usize)> = self.roots.iter().rev().map(|&r| (r, 0)).collect();
        while let Some((v, k)) = stack.pop() {
            if k < self.children[v].len() {
                stack.push((v, k + 1));
                stack.push((self.children[v][k], 0));
            } else {
                out.push(v);
            }
        }
        out
    }

    /// Subtree sums: a leaf's own value (non-finite and negative count as 0), an internal node the
    /// sum of its children. Internal nodes' own values are ignored, so tables that carry subtotals
    /// on parent rows don't double count. `values` may be shorter than the hierarchy (0s).
    pub fn sums(&self, values: &[f64]) -> Vec<f64> {
        let mut s = vec![0.0; self.len()];
        for v in self.postorder() {
            s[v] = if self.children[v].is_empty() {
                mag(values.get(v).copied().unwrap_or(0.0))
            } else {
                self.children[v].iter().map(|&c| s[c]).sum()
            };
        }
        s
    }
}

/// Renumbers a hierarchy so every node's children (and the roots) come largest `key` first, ties
/// in index order: returns `order` (new index → old index, a pre-order of the sorted hierarchy) and
/// the parents in the new numbering. The layouts keep children in index order, so laying out the
/// renumbered hierarchy draws siblings by size (sunbursts and icicles usually want that); map the
/// results back with `order`. `key` may be shorter than `parents` (0s); non-finite keys sort last.
pub fn sort_by_key_desc(parents: &[Option<usize>], key: &[f64]) -> (Vec<usize>, Vec<Option<usize>>) {
    let h = Hierarchy::new(parents);
    let k = |i: usize| key.get(i).copied().filter(|v| v.is_finite()).unwrap_or(f64::NEG_INFINITY);
    let by_key = |list: &mut Vec<usize>| list.sort_by(|&a, &b| datars_math::total_cmp(k(b), k(a)).then(a.cmp(&b)));
    let mut roots = h.roots.clone();
    by_key(&mut roots);
    let mut children = h.children.clone();
    children.iter_mut().for_each(by_key);
    let mut order = Vec::with_capacity(h.len());
    let mut stack: Vec<usize> = roots.iter().rev().copied().collect();
    while let Some(v) = stack.pop() {
        order.push(v);
        stack.extend(children[v].iter().rev());
    }
    let mut new_index = vec![0usize; h.len()];
    for (pos, &old) in order.iter().enumerate() {
        new_index[old] = pos;
    }
    let new_parents = order.iter().map(|&old| h.parent[old].map(|p| new_index[p])).collect();
    (order, new_parents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_siblings_by_key() {
        // 0 ─┬─ 1 (small) ── 3
        //    └─ 2 (big)
        // 4 (a second root, the biggest)
        let parents = [None, Some(0), Some(0), Some(1), None];
        let (order, np) = sort_by_key_desc(&parents, &[5.0, 1.0, 4.0, 1.0, 9.0]);
        assert_eq!(order, vec![4, 0, 2, 1, 3]);
        assert_eq!(np, vec![None, None, Some(1), Some(1), Some(3)]);
        let h = Hierarchy::new(&np);
        assert_eq!(h.children[1], vec![2, 3], "big child first");
        // Ties keep index order; a short key is padded.
        let (o, _) = sort_by_key_desc(&[None, Some(0), Some(0)], &[]);
        assert_eq!(o, vec![0, 1, 2]);
        assert_eq!(sort_by_key_desc(&[], &[]), (vec![], vec![]));
    }

    #[test]
    fn builds_children_depths_orders() {
        // 0 ─┬─ 1 ─── 3
        //    └─ 2
        let h = Hierarchy::new(&[None, Some(0), Some(0), Some(1)]);
        assert_eq!(h.roots, vec![0]);
        assert_eq!(h.children[0], vec![1, 2]);
        assert_eq!(h.depth, vec![0, 1, 1, 2]);
        assert_eq!(h.preorder, vec![0, 1, 3, 2]);
        assert_eq!(h.postorder(), vec![3, 1, 2, 0]);
        assert_eq!(h.height(), 2);
        assert_eq!(h.sums(&[100.0, 7.0, 2.0, 5.0]), vec![7.0, 5.0, 2.0, 5.0]);
    }

    #[test]
    fn repairs_cycles_and_bad_indices() {
        // 0 → 1 → 2 → 0 is a cycle; 3's parent is out of range; 4 is its own parent.
        let h = Hierarchy::new(&[Some(1), Some(2), Some(0), Some(99), Some(4)]);
        assert_eq!(h.roots, vec![2, 3, 4]);
        assert_eq!(h.parent[0], Some(1));
        assert_eq!(h.parent[1], Some(2));
        assert_eq!(h.preorder.len(), 5, "every node reachable after repair");
        let empty = Hierarchy::new(&[]);
        assert!(empty.is_empty() && empty.postorder().is_empty() && empty.height() == 0);
    }
}
