//! Resolution: layer themes (base first), modes, and overrides, honouring locks; then evaluate
//! expressions into concrete values.

use crate::{CExpr, Check, FontSpec, Mode, Palette, PaletteDef, PaletteKind, Scheme, Theme, TokenValue};
use datars_color::{contrast_ratio, palette as pal, Color, Oklab};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq)]
pub struct Diag {
    pub token: String,
    pub message: String,
}

/// A fully resolved theme: concrete values by token name.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResolvedTheme {
    pub name: String,
    pub mode: Mode,
    pub colors: BTreeMap<String, Color>,
    pub palettes: BTreeMap<String, Palette>,
    pub numbers: BTreeMap<String, f64>,
    pub fonts: BTreeMap<String, FontSpec>,
    pub texts: BTreeMap<String, String>,
    pub checks: Vec<Check>,
}

impl ResolvedTheme {
    pub fn color(&self, name: &str) -> Option<Color> {
        self.colors.get(name).copied()
    }
    pub fn palette(&self, name: &str) -> Option<&Palette> {
        self.palettes.get(name)
    }
    pub fn number(&self, name: &str) -> Option<f64> {
        self.numbers.get(name).copied()
    }
    pub fn font(&self, name: &str) -> Option<&FontSpec> {
        self.fonts.get(name)
    }
    pub fn text(&self, name: &str) -> Option<&str> {
        self.texts.get(name).map(|s| s.as_str())
    }

    /// Interpolate two resolved themes (theme and mode switches animate): colours in OKLab,
    /// numbers linearly, palettes element-wise when their lengths match; fonts and text switch at ½.
    pub fn lerp(a: &ResolvedTheme, b: &ResolvedTheme, t: f64) -> ResolvedTheme {
        if t <= 0.0 {
            return a.clone();
        }
        if t >= 1.0 {
            return b.clone();
        }
        let mut out = if t < 0.5 { a.clone() } else { b.clone() };
        for (k, cb) in &b.colors {
            if let Some(ca) = a.colors.get(k) {
                out.colors.insert(k.clone(), ca.lerp_oklab(*cb, t));
            }
        }
        for (k, pb) in &b.palettes {
            if let Some(pa) = a.palettes.get(k) {
                if pa.colors.len() == pb.colors.len() {
                    let colors = pa.colors.iter().zip(&pb.colors).map(|(x, y)| x.lerp_oklab(*y, t)).collect();
                    out.palettes.insert(k.clone(), Palette { kind: pb.kind, colors });
                }
            }
        }
        for (k, nb) in &b.numbers {
            if let Some(na) = a.numbers.get(k) {
                out.numbers.insert(k.clone(), na + (nb - na) * t);
            }
        }
        out
    }

    /// Resolved tokens as JSON (hosts can style their own chrome consistently, e.g. CSS variables).
    pub fn to_json(&self) -> serde_json::Value {
        let mut o = serde_json::Map::new();
        for (k, c) in &self.colors {
            o.insert(k.clone(), c.to_hex().into());
        }
        for (k, p) in &self.palettes {
            o.insert(k.clone(), p.colors.iter().map(|c| c.to_hex()).collect::<Vec<_>>().into());
        }
        for (k, n) in &self.numbers {
            o.insert(k.clone(), (*n).into());
        }
        for (k, f) in &self.fonts {
            o.insert(k.clone(), serde_json::to_value(f).unwrap_or_default());
        }
        for (k, s) in &self.texts {
            o.insert(k.clone(), s.clone().into());
        }
        serde_json::json!({ "name": self.name, "mode": self.mode.name(), "tokens": o })
    }
}

/// Resolve a theme chain (base first) in `mode`, then apply `overrides` layers (a document's own
/// tokens, then a host app's). A token locked by a layer can't be overridden by any later layer; a
/// theme's own modes may still refine what it locks. Returns the theme and any diagnostics.
pub fn resolve(chain: &[&Theme], mode: Mode, overrides: &[&BTreeMap<String, TokenValue>]) -> (ResolvedTheme, Vec<Diag>) {
    let mut diags = Vec::new();
    let mut merged: BTreeMap<String, TokenValue> = BTreeMap::new();
    let mut lock_owner: BTreeMap<String, usize> = BTreeMap::new();
    let mut checks = Vec::new();

    let apply = |layer: &BTreeMap<String, TokenValue>, owner: usize, merged: &mut BTreeMap<String, TokenValue>, lock_owner: &BTreeMap<String, usize>, diags: &mut Vec<Diag>| {
        for (k, v) in layer {
            if let Some(&o) = lock_owner.get(k) {
                if o < owner {
                    diags.push(Diag { token: k.clone(), message: "locked by an earlier layer; override ignored".into() });
                    continue;
                }
            }
            merged.insert(k.clone(), v.clone());
        }
    };
    // Each theme's base, then its own mode block, in chain order (the CSS cascade): a child's
    // token wins in every mode unless the child gives a mode value too. So a brand's `accent`
    // survives dark mode — and a child that shadows a value its parent varies by mode is told.
    // Beneath a dark-first theme (`scheme: dark`), light mode resolves as dark: its base tokens are
    // dark values, and what it leaves unset should come from the parents' dark block.
    let dark_below = if mode == Mode::Light { chain.iter().rposition(|t| t.scheme == Scheme::Dark) } else { None };
    let mut varied_by: BTreeMap<&str, &str> = BTreeMap::new();
    for (j, t) in chain.iter().enumerate() {
        let m = if dark_below.is_some_and(|d| j < d) { Mode::Dark } else { mode };
        let own_mode = if m != Mode::Light { t.modes.get(&m) } else { None };
        // A dark-first theme's base shadowing its parents' dark values is the point, not a slip.
        let quiet = t.scheme == Scheme::Dark && (mode == Mode::Dark || dark_below == Some(j));
        for k in t.tokens.keys() {
            if quiet {
                break;
            }
            if let Some(parent) = varied_by.get(k.as_str()) {
                if !own_mode.is_some_and(|m| m.contains_key(k)) {
                    diags.push(Diag { token: k.clone(), message: format!("set for every mode by {}, but {parent} varies it in {} mode: add modes.{}.{k} if it needs a different value there", t.name, m.name(), m.name()) });
                }
            }
        }
        apply(&t.tokens, j, &mut merged, &lock_owner, &mut diags);
        for k in &t.locked {
            lock_owner.entry(k.clone()).or_insert(j);
        }
        checks.extend(t.checks.iter().cloned());
        if let Some(m) = own_mode {
            apply(m, j, &mut merged, &lock_owner, &mut diags);
            for k in m.keys() {
                varied_by.insert(k.as_str(), t.name.as_str());
            }
        }
    }
    for (k, layer) in overrides.iter().enumerate() {
        apply(layer, chain.len() + k, &mut merged, &lock_owner, &mut diags);
    }

    let mut ev = Eval { tokens: &merged, colors: BTreeMap::new(), palettes: BTreeMap::new(), stack: BTreeSet::new(), diags: Vec::new() };
    let mut out = ResolvedTheme { name: chain.last().map(|t| t.name.clone()).unwrap_or_default(), mode, checks, ..Default::default() };
    for (k, v) in &merged {
        match v {
            TokenValue::Color(_) => {
                if let Some(c) = ev.color(k) {
                    out.colors.insert(k.clone(), c);
                }
            }
            TokenValue::Palette(_) => {
                if let Some(p) = ev.palette(k) {
                    out.palettes.insert(k.clone(), p);
                }
            }
            TokenValue::Number(n) => {
                out.numbers.insert(k.clone(), *n);
            }
            TokenValue::Font(f) => {
                out.fonts.insert(k.clone(), f.clone());
            }
            TokenValue::Text(s) => {
                out.texts.insert(k.clone(), s.clone());
            }
        }
    }
    diags.extend(ev.diags);
    (out, diags)
}

struct Eval<'a> {
    tokens: &'a BTreeMap<String, TokenValue>,
    colors: BTreeMap<String, Option<Color>>,
    palettes: BTreeMap<String, Option<Palette>>,
    stack: BTreeSet<String>,
    diags: Vec<Diag>,
}

impl Eval<'_> {
    fn color(&mut self, name: &str) -> Option<Color> {
        if let Some(c) = self.colors.get(name) {
            return *c;
        }
        if !self.stack.insert(name.to_string()) {
            self.diags.push(Diag { token: name.into(), message: "reference cycle".into() });
            return None;
        }
        let r = match self.tokens.get(name) {
            Some(TokenValue::Color(e)) => {
                let e = e.clone();
                self.expr(name, &e)
            }
            Some(TokenValue::Palette(_)) => self.palette(name).map(|p| p.get(0)),
            Some(_) => {
                self.diags.push(Diag { token: name.into(), message: "not a colour".into() });
                None
            }
            None => {
                self.diags.push(Diag { token: name.into(), message: "unknown token".into() });
                None
            }
        };
        self.stack.remove(name);
        self.colors.insert(name.to_string(), r);
        r
    }

    fn palette(&mut self, name: &str) -> Option<Palette> {
        if let Some(p) = self.palettes.get(name) {
            return p.clone();
        }
        if !self.stack.insert(format!("[{name}]")) {
            self.diags.push(Diag { token: name.into(), message: "reference cycle".into() });
            return None;
        }
        let r = match self.tokens.get(name) {
            Some(TokenValue::Palette(PaletteDef::Colors { kind, colors })) => {
                let (kind, colors) = (*kind, colors.clone());
                let cs: Option<Vec<Color>> = colors.iter().map(|e| self.expr(name, e)).collect();
                cs.map(|colors| Palette { kind, colors })
            }
            Some(TokenValue::Palette(PaletteDef::Generate { kind, from, to, n })) => {
                let (kind, from, to, n) = (*kind, from.clone(), to.clone(), *n);
                let f = self.expr(name, &from);
                let t = to.and_then(|t| self.expr(name, &t));
                f.map(|f| Palette {
                    kind,
                    colors: match kind {
                        PaletteKind::Categorical => pal::generate_categorical(f, n),
                        PaletteKind::Sequential => pal::generate_sequential(f, n),
                        PaletteKind::Diverging => {
                            let half = n.max(3) / 2;
                            let mut lo = pal::generate_sequential(f, half + 1);
                            lo.reverse();
                            let hi = pal::generate_sequential(t.unwrap_or(f), half + 1);
                            let mid = Color::parse("#f2f2f2").unwrap_or(Color::WHITE);
                            let mut v: Vec<Color> = lo[..half].to_vec();
                            v.push(mid);
                            v.extend_from_slice(&hi[1..]);
                            v
                        }
                    },
                })
            }
            _ => {
                self.diags.push(Diag { token: name.into(), message: "not a palette".into() });
                None
            }
        };
        self.stack.remove(&format!("[{name}]"));
        self.palettes.insert(name.to_string(), r.clone());
        r
    }

    fn num(&mut self, e: &CExpr) -> Option<f64> {
        match e {
            CExpr::Num(n) => Some(*n),
            CExpr::Ref { name, index: None } => match self.tokens.get(name) {
                Some(TokenValue::Number(n)) => Some(*n),
                _ => None,
            },
            _ => None,
        }
    }

    fn expr(&mut self, owner: &str, e: &CExpr) -> Option<Color> {
        match e {
            CExpr::Lit(c) => Some(*c),
            CExpr::Num(_) => None,
            CExpr::Ref { name, index: None } => self.color(name),
            CExpr::Ref { name, index: Some(i) } => self.palette(name).map(|p| p.get(*i)),
            CExpr::Call { f, args } => {
                let bad = |ev: &mut Self, msg: &str| {
                    ev.diags.push(Diag { token: owner.into(), message: format!("{f}(): {msg}") });
                    None
                };
                let c = |ev: &mut Self, i: usize| args.get(i).and_then(|a| ev.expr(owner, a));
                let n = |ev: &mut Self, i: usize| args.get(i).and_then(|a| ev.num(a));
                match f.as_str() {
                    "mix" => match (c(self, 0), c(self, 1), n(self, 2)) {
                        (Some(a), Some(b), t) => Some(a.lerp_oklab(b, t.unwrap_or(0.5))),
                        _ => bad(self, "mix(a, b, t)"),
                    },
                    "alpha" => match (c(self, 0), n(self, 1)) {
                        (Some(a), Some(x)) => Some(a.with_alpha(x as f32)),
                        _ => bad(self, "alpha(a, x)"),
                    },
                    "lighten" | "darken" => match (c(self, 0), n(self, 1)) {
                        (Some(a), Some(d)) => {
                            let (l, ch, h) = a.to_oklab().to_lch();
                            let l = if f == "lighten" { l + d } else { l - d };
                            Some(Oklab::from_lch(l.clamp(0.0, 1.0), ch, h, a.a as f64).to_color())
                        }
                        _ => bad(self, "lighten(a, d)"),
                    },
                    "saturate" | "desaturate" => match (c(self, 0), n(self, 1)) {
                        (Some(a), Some(d)) => {
                            let (l, ch, h) = a.to_oklab().to_lch();
                            let ch = if f == "saturate" { ch * (1.0 + d) } else { ch * (1.0 - d).max(0.0) };
                            Some(Oklab::from_lch(l, ch, h, a.a as f64).to_color())
                        }
                        _ => bad(self, "saturate(a, d)"),
                    },
                    "oklch" => match (n(self, 0), n(self, 1), n(self, 2)) {
                        (Some(l), Some(ch), Some(h)) => Some(Oklab::from_lch(l, ch, h.to_radians(), 1.0).to_color()),
                        _ => bad(self, "oklch(l, c, hue°)"),
                    },
                    "rgb" => match (n(self, 0), n(self, 1), n(self, 2)) {
                        (Some(r), Some(g), Some(b)) => Some(Color::rgb8(r as u8, g as u8, b as u8)),
                        _ => bad(self, "rgb(r, g, b)"),
                    },
                    "on" => match c(self, 0) {
                        Some(bg) => {
                            let dark = Color::parse("#111111").unwrap_or(Color::BLACK);
                            Some(if contrast_ratio(Color::WHITE, bg) >= contrast_ratio(dark, bg) { Color::WHITE } else { dark })
                        }
                        None => bad(self, "on(background)"),
                    },
                    "contrast" => match (c(self, 0), c(self, 1), c(self, 2)) {
                        (Some(bg), Some(a), Some(b)) => Some(if contrast_ratio(a, bg) >= contrast_ratio(b, bg) { a } else { b }),
                        _ => bad(self, "contrast(bg, a, b)"),
                    },
                    _ => bad(self, "unknown function"),
                }
            }
        }
    }
}
