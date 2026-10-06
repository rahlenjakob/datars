//! `datars-motion` — transitions and authored animation, pure and seekable
//! (docs/05-time-and-motion.md).
//!
//! # Two ways things move
//!
//! - **Reactive transitions — plans.** [`plan`]`(from, to, rules, cx)` does the expensive work
//!   once; [`Plan::at`]`(t, shaper)` is a cheap pure function of normalized time returning a
//!   [`Scene`]. `at(0) == from` and `at(1) == to` exactly; no NaN at any `t`; entering elements'
//!   opacity never decreases and exiting elements' never increases; morphing outlines never turn
//!   inside out. Scroll can scrub a plan, video can render it, and a second change mid-flight
//!   [`retarget`]s from the current frame.
//! - **Authored animation — clips** ([`clip`]): pure functions of local time producing property
//!   patches, with a small algebra (`seq`, `par`, `delay`, `stretch`, `reverse`, `loop_n`,
//!   `ease`, `stagger`, `keyframes`, `hold`).
//!
//! ```
//! use datars_motion::{plan, Easing, MotionRules, NoShaper, PlanCx, Rule};
//! use datars_scene::{Geom, Key, Node, Scene};
//!
//! let bar = |h: f64| Scene::new(100.0, 100.0, Node::group(Key::name("root"), vec![
//!     Node::shape(Key::name("SE"), Geom::rect(10.0, 100.0 - h, 20.0, h)),
//! ]));
//! let (from, to) = (bar(20.0), bar(80.0));
//! let rules = MotionRules::new(vec![Rule::new().duration(0.6).easing(Easing::parse("spring(170, 26)").unwrap())]);
//! let p = plan(&from, &to, &rules, &PlanCx::default());
//! assert_eq!(p.duration(), 0.6);
//! assert_eq!(p.at(0.0, &NoShaper), from); // exact endpoints
//! assert_eq!(p.at(1.0, &NoShaper), to);
//! let _frame = p.at(0.37, &NoShaper); // any t, in any order: seekable
//! ```
//!
//! # How a plan is built
//!
//! 1. **Elements.** Each scene flattens into matchable elements: every `Shape`, `Text` and
//!    `Image` node, keyed by its key path, and every instance of an `Instances` node, keyed by
//!    (node path, instance key). Groups and views are structure. Each element records its
//!    transform into the root content space, accumulated opacity, bounds and structural parent.
//! 2. **Rules.** [`MotionRules`] cascade like CSS (specificity, then source order) to give every
//!    element a duration, easing, delay, matcher, choreography, route, morph strategy and
//!    enter / exit ghosts. Defaults: 0.9 s, cubic-in-out, match by path, together, straight,
//!    disc morph, fade in / fade out.
//! 3. **Matching** ([`Matcher`]): by path (default), by own key (cross-recipe morphs), hierarchy
//!    (one parent ↔ its unit children, the parent's geometry partitioned into slices or a grid),
//!    nearest (optimal assignment by position: an integer auction on ≤ 2,000 elements per side,
//!    greedy nearest beyond), or none (crossfade).
//! 4. **Choreography** ([`Choreography`]): each element's window [start, end] ⊂ [0, 1] —
//!    together, stagger (data / left / right / centre-out / value / seeded random order), phased
//!    (exit → update → enter shares), wave, ripple.
//! 5. **Interpolation.** Transforms decompose (translate, shortest-arc rotation, scale, skew);
//!    equal inks stay inks (tokens stay late-bound), different inks resolve against the plan's
//!    theme and mix in OKLab; same-kind geometry interpolates parameters (rect x/y/w/h/r, ellipse,
//!    arc angles and radii, segments, point-wise polylines and areas — resampled when lengths
//!    differ — symbols, same-structure paths); different kinds morph outlines with a strategy
//!    ([`MorphStrategy`]: disc — never folds — resample, or crossfade).
//!    Outlines are resampled to 32–256 points by on-screen size; multipolygons morph their
//!    largest part while the others shrink to their centroids. Text interpolates origin, opacity
//!    and ink, and counts numbers through a [`TextShaper`] each frame (otherwise crossfades).
//!    Instances interpolate column-wise. View cameras follow the van Wijk–Nuij path on long moves.
//!    Lines of different lengths merge instead of resampling: every vertex of both is kept and
//!    partnered at the same place on the other line (by x for series), so data points stay
//!    vertices. Instances whose every instance sits on a vertex of a line in the same group (a
//!    line's point markers) **ride** it: they pair their vertices by key, share its window and
//!    easing, enter from (and exit onto) the line, and appear as a draw-on trim reaches them.
//! 6. **Routes** ([`Route`]): detours that are exactly zero at both ends (arc, elbow, spiral,
//!    explode, hop, drift, drop).
//! 7. **Ghosts** ([`Ghost`]): enter / exit states relative to the element's own — fade, grow
//!    from an origin, offset, from the parent datum, from a point.
//!
//! # Frames
//!
//! A frame merges both scenes' structure. Elements whose two states share a container
//! interpolate in place (local coordinates, glued to an animating parent); elements changing
//! container fly in the **flight layer** — the root's last child, keyed `~flight`, nested in
//! groups mirroring each element's destination path. Flattening treats that layer as
//! transparent, so a frame is a valid `from` for [`retarget`]. During a crossfade the outgoing
//! copy precedes the incoming one under the same key.
//!
//! Determinism: no hash maps, no platform transcendentals (everything through
//! `datars_math::m`), ties broken by element order; the same inputs give the same frames on every
//! target.

pub mod camera;
pub mod clip;
pub mod easing;
pub mod ghost;
pub mod interp;
pub mod matching;
pub mod outline;
pub mod route;
pub mod rules;

mod build;
mod choreo;
mod columns;
mod elements;
mod lines;
mod plan;

pub use build::{plan, retarget};
pub use columns::COLUMN_MAX;
pub use easing::{spring_settle_time, Dir, Easing, Family, Keyframe, StepPosition};
pub use elements::FLIGHT_KEY;
pub use ghost::{Ghost, GhostFrom, Origin};
pub use matching::Correspondence;
pub use plan::{Plan, PlanStats};
pub use route::{Route, RouteCx};
pub use rules::{Choreography, Matcher, MorphStrategy, MotionRules, Order, Partition, Rule, Selector, When, DEFAULT_DURATION};

use datars_math::Vec2;
use datars_scene::TextNode;
use datars_theme::ResolvedTheme;

pub use datars_scene::Scene;

/// Re-shapes text whose content changed mid-transition (counting numbers, animated font sizes).
/// Implemented with `datars-text` (format `node.number` into `node.text`, then lay out runs and
/// bounds), so this crate needs no text stack.
pub trait TextShaper {
    fn shape(&self, node: &mut TextNode);
}

/// A shaper that leaves text as it is (numbers do not count). Useful for tests and for hosts
/// that shape text later.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoShaper;

impl TextShaper for NoShaper {
    fn shape(&self, _node: &mut TextNode) {}
}

/// What a plan needs to know besides the two scenes.
#[derive(Clone, Debug, Default)]
pub struct PlanCx {
    /// Different inks resolve against this theme to mix in OKLab.
    pub theme: ResolvedTheme,
    /// State names for rules' `when` globs (a story step, a chart state).
    pub from_state: Option<String>,
    pub to_state: Option<String>,
    /// Where the change was triggered (a click), in root-content coordinates: the origin of
    /// ripples and of `GhostFrom::Event`.
    pub event_point: Option<Vec2>,
}

impl PlanCx {
    pub fn new(theme: ResolvedTheme) -> PlanCx {
        PlanCx { theme, ..Default::default() }
    }
    pub fn states(mut self, from: Option<&str>, to: Option<&str>) -> PlanCx {
        self.from_state = from.map(Into::into);
        self.to_state = to.map(Into::into);
        self
    }
    pub fn event(mut self, p: Vec2) -> PlanCx {
        self.event_point = Some(p);
        self
    }
}
