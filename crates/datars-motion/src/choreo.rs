//! Choreography: each element's window [start, end] ⊂ [0, 1] of the plan.
//!
//! A rule's `delay` and `duration` give its window in plan time; the choreography subdivides it
//! among the elements that share the rule's choreography (staggers rank them, waves and ripples
//! delay by position, phased splits exits / updates / enters).

use crate::route::{mix64, unit};
use crate::rules::{Choreography, Order};
use datars_math::{total_cmp, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Phase {
    Exit,
    Update,
    Enter,
}

pub(crate) struct Item {
    /// Which rule supplied the choreography (items sharing it are choreographed together).
    pub group: usize,
    pub phase: Phase,
    pub center: Vec2,
    pub order: usize,
    pub value: Option<f64>,
    pub hash: u64,
    /// The rule window in plan time.
    pub ws: f64,
    pub we: f64,
}

/// Normalized positions o ∈ [0, 1] of `idx` within their group.
fn positions(items: &[Item], idx: &[usize], ch: &Choreography, event: Option<Vec2>, scene_center: Vec2) -> Vec<f64> {
    let n = idx.len();
    let rank = |keys: Vec<(f64, usize, usize)>| -> Vec<f64> {
        // keys: (primary, order, position in idx)
        let mut k = keys;
        k.sort_by(|a, b| total_cmp(a.0, b.0).then(a.1.cmp(&b.1)));
        let mut o = vec![0.0; n];
        for (r, (_, _, p)) in k.iter().enumerate() {
            o[*p] = if n > 1 { r as f64 / (n - 1) as f64 } else { 0.0 };
        }
        o
    };
    let spread_by = |vals: Vec<f64>| -> Vec<f64> {
        let lo = vals.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let d = hi - lo;
        vals.into_iter().map(|v| if d > 1e-12 && v.is_finite() { ((v - lo) / d).clamp(0.0, 1.0) } else { 0.0 }).collect()
    };
    match ch {
        Choreography::Stagger { order, .. } => {
            let keys: Vec<(f64, usize, usize)> = idx
                .iter()
                .enumerate()
                .map(|(p, &i)| {
                    let it = &items[i];
                    let primary = match order {
                        Order::Data => 0.0,
                        Order::Left => it.center.x,
                        Order::Right => -it.center.x,
                        Order::CenterOut => 0.0, // filled below
                        Order::Value => it.value.map_or(f64::INFINITY, |v| -v),
                        Order::Random(seed) => unit(mix64(it.hash ^ mix64(*seed))),
                    };
                    (primary, it.order, p)
                })
                .collect();
            let keys = if *order == Order::CenterOut {
                let c = idx.iter().fold(Vec2::ZERO, |acc, &i| acc + items[i].center) / n.max(1) as f64;
                keys.into_iter().map(|(_, o, p)| (items[idx[p]].center.dist(c), o, p)).collect()
            } else {
                keys
            };
            rank(keys)
        }
        Choreography::Wave { angle, .. } => {
            let (s, c) = datars_math::m::sin_cos(*angle);
            spread_by(idx.iter().map(|&i| items[i].center.x * c + items[i].center.y * s).collect())
        }
        Choreography::Ripple { origin, .. } => {
            let o = origin.or(event).unwrap_or(scene_center);
            let d: Vec<f64> = idx.iter().map(|&i| items[i].center.dist(o)).collect();
            let hi = d.iter().copied().fold(0.0, f64::max);
            d.into_iter().map(|v| if hi > 1e-12 { v / hi } else { 0.0 }).collect()
        }
        _ => vec![0.0; n],
    }
}

/// Windows for every item; `choreo_of(group)` gives the group's choreography.
pub(crate) fn windows(items: &[Item], choreo_of: &dyn Fn(usize) -> Choreography, event: Option<Vec2>, scene_center: Vec2) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = items.iter().map(|it| (it.ws, it.we)).collect();
    let mut groups: std::collections::BTreeMap<usize, Vec<usize>> = std::collections::BTreeMap::new();
    for (i, it) in items.iter().enumerate() {
        if it.group != usize::MAX {
            groups.entry(it.group).or_default().push(i);
        }
    }
    for (g, idx) in groups {
        let ch = choreo_of(g);
        match &ch {
            Choreography::Together => {}
            Choreography::Phased { exit, update, enter } => {
                let (sx, su, se) = (exit.max(0.0), update.max(0.0), enter.max(0.0));
                let tot = sx + su + se;
                if tot <= 0.0 || !tot.is_finite() {
                    continue;
                }
                for &i in &idx {
                    let it = &items[i];
                    let l = it.we - it.ws;
                    let (a, b) = match it.phase {
                        Phase::Exit => (0.0, sx / tot),
                        Phase::Update => (sx / tot, (sx + su) / tot),
                        Phase::Enter => ((sx + su) / tot, 1.0),
                    };
                    out[i] = (it.ws + a * l, it.ws + b * l);
                }
            }
            Choreography::Stagger { spread, .. } | Choreography::Wave { spread, .. } | Choreography::Ripple { spread, .. } => {
                let sp = if spread.is_finite() { spread.clamp(0.0, 0.95) } else { 0.0 };
                let o = positions(items, &idx, &ch, event, scene_center);
                for (p, &i) in idx.iter().enumerate() {
                    let it = &items[i];
                    let l = it.we - it.ws;
                    let s = it.ws + o[p] * sp * l;
                    out[i] = (s, (s + (1.0 - sp) * l).min(it.we));
                }
            }
        }
    }
    for w in &mut out {
        let s = if w.0.is_finite() { w.0.clamp(0.0, 1.0) } else { 0.0 };
        let e = if w.1.is_finite() { w.1.clamp(s, 1.0) } else { 1.0 };
        *w = (s, e);
    }
    out
}

/// Linear progress within a window at plan time `t` (a zero-length window steps at its start).
#[inline]
pub(crate) fn progress(t: f64, w: (f64, f64)) -> f64 {
    if t <= w.0 {
        if t >= w.1 {
            1.0
        } else {
            0.0
        }
    } else if t >= w.1 {
        1.0
    } else {
        (t - w.0) / (w.1 - w.0)
    }
}
