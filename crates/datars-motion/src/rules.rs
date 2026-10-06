//! Motion rules — CSS for data-joined elements (docs/05-time-and-motion.md §Motion rules).
//!
//! A document or recipe declares a list of [`Rule`]s; each element of a transition gets the
//! cascade of the rules that apply to it. A rule applies when its `when` state globs match the
//! plan's from/to state names and its `select`or matches the element (role, kind, key-path
//! prefix). Rules are ordered by **specificity** (selector fields, key-prefix depth, state
//! globs), then by source order; for every field, the last rule in that order that sets it wins.
//! Unset fields fall back to the defaults: 0.9 s, cubic-in-out, no delay, match by key path,
//! together, straight routes, disc morphs, fade in, fade out.
//!
//! `MotionRules` is serde-serializable: it is part of the document IR.

use crate::easing::Easing;
use crate::ghost::Ghost;
use crate::route::Route;
use datars_math::Vec2;
use datars_scene::{KeyPath, Role};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DEFAULT_DURATION: f64 = 0.9;

/// The rules of a document or recipe, in source order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MotionRules {
    #[serde(default)]
    pub rules: Vec<Rule>,
    /// Defaults under `rules`: recipes' own motion (a line draws on, grouped bars grow), each
    /// scoped to the recipe's key path. They cascade among themselves like `rules`, and any
    /// field a rule in `rules` sets wins over them whatever the specificities — the document
    /// always has the last word. Indices in [`Resolved::applied`] count `rules` first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub defaults: Vec<Rule>,
}

impl MotionRules {
    pub fn new(rules: Vec<Rule>) -> MotionRules {
        MotionRules { rules, defaults: Vec::new() }
    }
    pub fn with_defaults(mut self, defaults: Vec<Rule>) -> MotionRules {
        self.defaults = defaults;
        self
    }
}

/// When a rule applies: glob patterns over the plan's from/to state names (`*` and `?`).
/// A missing pattern matches anything; a present pattern other than `*` needs a state name.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct When {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
}

/// Which elements a rule applies to. Every present field must match.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Selector {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
    /// An element kind: a geometry kind (`rect`, `ellipse`, `arc`, `segment`, `polyline`, `area`,
    /// `path`, `symbol`), `shape` (any geometry), `text`, `image`, `instance`, or a structural
    /// kind (`group`, `view`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// The element's key path must start with this path (root key first).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_prefix: Option<KeyPath>,
}

impl Selector {
    pub fn role(role: Role) -> Selector {
        Selector { role: Some(role), ..Default::default() }
    }
    pub fn kind(kind: &str) -> Selector {
        Selector { kind: Some(kind.into()), ..Default::default() }
    }
    pub fn prefix(p: KeyPath) -> Selector {
        Selector { key_prefix: Some(p), ..Default::default() }
    }
}

/// How a split parent divides into its N children.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Partition {
    /// Proportional slices when every piece carries a value (a bar and its months' segments), else
    /// a grid when the pieces are compact marks (dots, squares: parliament seats), else slices.
    #[default]
    Auto,
    /// N slices along the shape's long axis (angle or radius for arcs).
    Slices,
    /// A near-square grid of N cells (the last row stretches to fill the width).
    Grid,
    /// Slices along the long axis as wide as each piece's value (a bar that is the sum of its
    /// segments); equal slices where values are missing.
    Proportional,
}

/// Who becomes whom.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Matcher {
    /// Same key path → pair (the default: object constancy by data identity).
    #[default]
    ByPath,
    /// Same own key regardless of parent path — cross-recipe morphs (a map region into a bar).
    ByKey,
    /// By path first; then a parent key `("S",)` on one side splits into / merges from its child
    /// keys `("S", Unit(i))` on the other, the parent's geometry partitioned into N pieces.
    Hierarchy {
        #[serde(default)]
        partition: Partition,
    },
    /// Deterministic optimal assignment by position (auction on ≤ 2,000 elements per side,
    /// greedy nearest beyond) for unkeyed sets.
    Nearest,
    /// Nothing pairs: everything exits and enters (a crossfade).
    None,
}

impl Matcher {
    pub(crate) fn group(&self) -> u8 {
        match self {
            Matcher::ByPath => 0,
            Matcher::ByKey => 1,
            Matcher::Hierarchy { .. } => 2,
            Matcher::Nearest => 3,
            Matcher::None => 4,
        }
    }
}

/// The order a stagger walks elements in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Order {
    /// Tree (data) order.
    #[default]
    Data,
    /// Leftmost first (by centre x).
    Left,
    Right,
    /// From the centre of the set outward.
    CenterOut,
    /// Largest `semantics.value` first (elements without a value last, in data order).
    Value,
    /// Shuffled by a seeded hash of the key path: the same every time.
    Random(u64),
}

fn half() -> f64 {
    0.5
}

/// Timing: each element gets a window [start, end] ⊂ [0, 1] of the plan.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Choreography {
    /// Everyone uses the rule's whole window.
    #[default]
    Together,
    /// One after another in `order`; `spread` ∈ [0, 0.95] is the share of the window the starts
    /// are spread over.
    Stagger {
        #[serde(default)]
        order: Order,
        #[serde(default = "half")]
        spread: f64,
    },
    /// Exits, then updates, then enters, in these shares of the window.
    Phased { exit: f64, update: f64, enter: f64 },
    /// A wave rolling across in direction `angle` (radians, 0 = left → right): delay ∝ position.
    Wave {
        #[serde(default = "half")]
        spread: f64,
        #[serde(default)]
        angle: f64,
    },
    /// Delay ∝ distance from `origin` (root-content coordinates; the plan's event point or the
    /// scene centre when absent).
    Ripple {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        origin: Option<Vec2>,
        #[serde(default = "half")]
        spread: f64,
    },
}

/// Path-morph strategy for pairs whose geometry kinds differ (same kinds always interpolate
/// their parameters, unless the strategy is `Crossfade`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MorphStrategy {
    /// Travel + blend through an area-matched disc: never folds.
    #[default]
    Disc,
    /// Equal-arc-length outlines with aligned winding and start point, lerped point-wise.
    Resample,
    /// Opacity swap (also for same-kind pairs).
    Crossfade,
}

/// One motion rule. Every field is optional; see the module docs for the cascade.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<When>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub select: Option<Selector>,
    /// Seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub easing: Option<Easing>,
    /// Seconds before the element's window opens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matcher: Option<Matcher>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choreo: Option<Choreography>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<Route>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub morph: Option<MorphStrategy>,
    /// The ghost state entering elements start from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enter: Option<Ghost>,
    /// The ghost state exiting elements end at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit: Option<Ghost>,
}

impl Rule {
    pub fn new() -> Rule {
        Rule::default()
    }
    pub fn select(mut self, s: Selector) -> Rule {
        self.select = Some(s);
        self
    }
    pub fn when(mut self, from: Option<&str>, to: Option<&str>) -> Rule {
        self.when = Some(When { from: from.map(Into::into), to: to.map(Into::into) });
        self
    }
    pub fn duration(mut self, d: f64) -> Rule {
        self.duration = Some(d);
        self
    }
    pub fn easing(mut self, e: Easing) -> Rule {
        self.easing = Some(e);
        self
    }
    pub fn delay(mut self, d: f64) -> Rule {
        self.delay = Some(d);
        self
    }
    pub fn matcher(mut self, m: Matcher) -> Rule {
        self.matcher = Some(m);
        self
    }
    pub fn choreo(mut self, c: Choreography) -> Rule {
        self.choreo = Some(c);
        self
    }
    pub fn route(mut self, r: Route) -> Rule {
        self.route = Some(r);
        self
    }
    pub fn morph(mut self, m: MorphStrategy) -> Rule {
        self.morph = Some(m);
        self
    }
    pub fn enter(mut self, g: Ghost) -> Rule {
        self.enter = Some(g);
        self
    }
    pub fn exit(mut self, g: Ghost) -> Rule {
        self.exit = Some(g);
        self
    }

    /// Specificity: selector fields weigh most, then key-prefix depth, then state globs that are
    /// not `*`.
    fn specificity(&self) -> u32 {
        let mut s = 0;
        if let Some(sel) = &self.select {
            s += 1000 * (sel.role.is_some() as u32 + sel.kind.is_some() as u32 + sel.key_prefix.is_some() as u32);
            s += 10 * sel.key_prefix.as_ref().map_or(0, |p| p.0.len() as u32).min(99);
        }
        if let Some(w) = &self.when {
            s += w.from.as_deref().is_some_and(|p| p != "*") as u32 + w.to.as_deref().is_some_and(|p| p != "*") as u32;
        }
        s
    }
}

/// `*` matches any run, `?` one character.
pub fn glob(pattern: &str, s: &str) -> bool {
    fn go(p: &[char], s: &[char]) -> bool {
        match p.split_first() {
            None => s.is_empty(),
            Some(('*', rest)) => (0..=s.len()).any(|i| go(rest, &s[i..])),
            Some(('?', rest)) => !s.is_empty() && go(rest, &s[1..]),
            Some((c, rest)) => s.first() == Some(c) && go(rest, &s[1..]),
        }
    }
    let p: Vec<char> = pattern.chars().collect();
    let s: Vec<char> = s.chars().collect();
    go(&p, &s)
}

fn state_matches(pattern: Option<&str>, state: Option<&str>) -> bool {
    match (pattern, state) {
        (None, _) => true,
        (Some("*"), _) => true,
        (Some(p), Some(s)) => glob(p, s),
        (Some(_), None) => false,
    }
}

/// The effective motion settings of one element after the cascade.
#[derive(Clone, Debug, PartialEq)]
pub struct Resolved {
    pub duration: f64,
    pub easing: Easing,
    pub delay: f64,
    pub matcher: Matcher,
    pub choreo: Choreography,
    /// Index of the rule that supplied `choreo` (`usize::MAX` for the default): elements that
    /// share it are staggered together.
    pub choreo_src: usize,
    pub route: Route,
    pub morph: MorphStrategy,
    pub enter: Ghost,
    pub exit: Ghost,
    /// The rules that applied, in cascade order (for the inspector: "which rule won").
    pub applied: Vec<usize>,
}

impl Default for Resolved {
    fn default() -> Self {
        Resolved {
            duration: DEFAULT_DURATION,
            easing: Easing::default(),
            delay: 0.0,
            matcher: Matcher::ByPath,
            choreo: Choreography::Together,
            choreo_src: usize::MAX,
            route: Route::Straight,
            morph: MorphStrategy::Disc,
            enter: Ghost::fade(),
            exit: Ghost::fade(),
            applied: Vec::new(),
        }
    }
}

/// What a selector sees of an element.
pub(crate) struct Target<'a> {
    pub path: &'a KeyPath,
    pub role: Option<Role>,
    pub kind: &'a str,
    pub is_shape: bool,
}

/// Resolves elements against a rule list, caching one [`Resolved`] per distinct set of
/// applicable rules (so 10⁴ elements under three rules cost three cascades).
pub(crate) struct Resolver<'a> {
    /// The document's rules, then the defaults (`MotionRules::defaults`).
    rules: Vec<&'a Rule>,
    /// Rules whose `when` passes, ordered by (layer, specificity, index): defaults first, so
    /// every document rule comes later in the cascade.
    active: Vec<usize>,
    cache: BTreeMap<Vec<usize>, usize>,
    pub table: Vec<Resolved>,
}

impl<'a> Resolver<'a> {
    pub fn new(rules: &'a MotionRules, from_state: Option<&str>, to_state: Option<&str>) -> Resolver<'a> {
        let n_doc = rules.rules.len();
        let all: Vec<&Rule> = rules.rules.iter().chain(&rules.defaults).collect();
        let mut active: Vec<usize> = all
            .iter()
            .enumerate()
            .filter(|(_, r)| match &r.when {
                None => true,
                Some(w) => state_matches(w.from.as_deref(), from_state) && state_matches(w.to.as_deref(), to_state),
            })
            .map(|(i, _)| i)
            .collect();
        active.sort_by_key(|&i| (i < n_doc, all[i].specificity(), i));
        Resolver { rules: all, active, cache: BTreeMap::new(), table: Vec::new() }
    }

    fn selects(sel: &Selector, t: &Target) -> bool {
        if let Some(r) = sel.role {
            if t.role != Some(r) {
                return false;
            }
        }
        if let Some(k) = &sel.kind {
            let ok = k == t.kind
                || (k == "shape" && t.is_shape)
                || ((k == "instance" || k == "instances") && t.kind == "instance");
            if !ok {
                return false;
            }
        }
        if let Some(p) = &sel.key_prefix {
            if p.0.len() > t.path.0.len() || t.path.0[..p.0.len()] != p.0[..] {
                return false;
            }
        }
        true
    }

    /// Index into `table` of the element's resolved settings.
    pub fn resolve(&mut self, t: &Target) -> usize {
        let applied: Vec<usize> = self
            .active
            .iter()
            .copied()
            .filter(|&i| self.rules[i].select.as_ref().is_none_or(|s| Self::selects(s, t)))
            .collect();
        if let Some(&ix) = self.cache.get(&applied) {
            return ix;
        }
        let mut r = Resolved::default();
        for &i in &applied {
            let rule = &self.rules[i];
            if let Some(v) = rule.duration {
                r.duration = if v.is_finite() { v.max(0.0) } else { DEFAULT_DURATION };
            }
            if let Some(v) = &rule.easing {
                r.easing = v.clone();
            }
            if let Some(v) = rule.delay {
                r.delay = if v.is_finite() { v.max(0.0) } else { 0.0 };
            }
            if let Some(v) = &rule.matcher {
                r.matcher = v.clone();
            }
            if let Some(v) = &rule.choreo {
                r.choreo = v.clone();
                r.choreo_src = i;
            }
            if let Some(v) = &rule.route {
                r.route = v.clone();
            }
            if let Some(v) = rule.morph {
                r.morph = v;
            }
            if let Some(v) = &rule.enter {
                r.enter = v.clone();
            }
            if let Some(v) = &rule.exit {
                r.exit = v.clone();
            }
        }
        r.applied = applied.clone();
        let ix = self.table.len();
        self.table.push(r);
        self.cache.insert(applied, ix);
        ix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globs() {
        assert!(glob("*", "anything"));
        assert!(glob("step-?", "step-3"));
        assert!(glob("rank*", "ranked"));
        assert!(!glob("rank", "ranked"));
    }
}
