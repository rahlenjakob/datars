//! `datars-layout` — box layout for composition (titles, plots, axes, legends, dashboards).
//!
//! Engine-level because layout depends on measured text: an axis is as wide as its widest label,
//! so margins can't be guessed by recipes. A container lays out its children along one axis
//! (`rows`, `columns`), overlays them (`stack`), or places them in a `grid`. Child sizes are
//! fixed px, a percentage, `auto` (measured by a callback), or `fill` with a weight.

use datars_math::Rect;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dim {
    Px(f64),
    Percent(f64),
    Auto,
    Fill(f64),
}

impl Default for Dim {
    fn default() -> Self {
        Dim::Fill(1.0)
    }
}

impl Dim {
    /// From the IR's JSON form: a number, `"auto"`, `"fill"`, `{"fill": w}`, `"30%"`.
    pub fn from_json(v: &serde_json::Value) -> Option<Dim> {
        use serde_json::Value as J;
        match v {
            J::Null => None,
            J::Number(n) => n.as_f64().map(Dim::Px),
            J::String(s) if s == "auto" => Some(Dim::Auto),
            J::String(s) if s == "fill" => Some(Dim::Fill(1.0)),
            J::String(s) if s.ends_with('%') => s.trim_end_matches('%').trim().parse().ok().map(|p: f64| Dim::Percent(p / 100.0)),
            J::String(s) => s.parse().ok().map(Dim::Px),
            J::Object(o) => o.get("fill").and_then(|w| w.as_f64()).map(Dim::Fill),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    Stack,
    Rows,
    Columns,
    Grid(usize),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
    #[default]
    Stretch,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spec {
    pub kind: Kind,
    pub gap: f64,
    /// top, right, bottom, left
    pub padding: [f64; 4],
    pub align: Align,
}

impl Spec {
    pub fn parse(ty: &str, gap: f64, padding: [f64; 4], columns: Option<usize>, align: Option<&str>) -> Spec {
        let kind = match ty {
            "rows" => Kind::Rows,
            "columns" => Kind::Columns,
            "grid" => Kind::Grid(columns.unwrap_or(2).max(1)),
            _ => Kind::Stack,
        };
        let align = match align {
            Some("start") => Align::Start,
            Some("center") => Align::Center,
            Some("end") => Align::End,
            _ => Align::Stretch,
        };
        Spec { kind, gap, padding, align }
    }
}

/// A child's requested size. Unspecified axes default to `Fill(1)` on the main axis and stretch on
/// the cross axis.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Child {
    pub w: Option<Dim>,
    pub h: Option<Dim>,
}

/// Lay out `children` inside `container`. `measure(i, avail_w, avail_h)` returns the content size of
/// child `i` for `auto` dimensions, given what's available on each axis. Returns one rect per child.
pub fn layout(container: Rect, spec: &Spec, children: &[Child], measure: &mut dyn FnMut(usize, f64, f64) -> (f64, f64)) -> Vec<Rect> {
    let [pt, pr, pb, pl] = spec.padding;
    let inner = Rect::new(container.x + pl, container.y + pt, (container.w - pl - pr).max(0.0), (container.h - pt - pb).max(0.0));
    let n = children.len();
    if n == 0 {
        return Vec::new();
    }
    match spec.kind {
        Kind::Stack => children
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let (w, h) = cross_sizes(i, c, inner.w, inner.h, measure);
                place_cross(inner, w, h, spec.align)
            })
            .collect(),
        Kind::Rows | Kind::Columns => {
            let rows = spec.kind == Kind::Rows;
            let (main_total, cross_total) = if rows { (inner.h, inner.w) } else { (inner.w, inner.h) };
            let gaps = spec.gap * (n.saturating_sub(1)) as f64;
            let mut main = vec![0.0; n];
            let mut fills = vec![0.0; n];
            let mut fixed = 0.0;
            for (i, c) in children.iter().enumerate() {
                let d = if rows { c.h } else { c.w }.unwrap_or(Dim::Fill(1.0));
                match d {
                    Dim::Px(v) => main[i] = v.max(0.0),
                    Dim::Percent(p) => main[i] = (p * main_total).max(0.0),
                    Dim::Auto => {
                        let (mw, mh) = if rows { measure(i, cross_total, f64::INFINITY) } else { measure(i, f64::INFINITY, cross_total) };
                        main[i] = if rows { mh } else { mw }.max(0.0);
                    }
                    Dim::Fill(w) => fills[i] = w.max(0.0),
                }
                fixed += main[i];
            }
            let weight: f64 = fills.iter().sum();
            let remaining = (main_total - fixed - gaps).max(0.0);
            if weight > 0.0 {
                for i in 0..n {
                    if fills[i] > 0.0 {
                        main[i] = remaining * fills[i] / weight;
                    }
                }
            }
            let mut out = Vec::with_capacity(n);
            let mut pos = if rows { inner.y } else { inner.x };
            for (i, c) in children.iter().enumerate() {
                let cd = if rows { c.w } else { c.h };
                let cross = match cd {
                    None | Some(Dim::Fill(_)) => cross_total,
                    Some(Dim::Px(v)) => v,
                    Some(Dim::Percent(p)) => p * cross_total,
                    Some(Dim::Auto) => {
                        let (mw, mh) = if rows { measure(i, cross_total, main[i]) } else { measure(i, main[i], cross_total) };
                        if rows { mw } else { mh }
                    }
                };
                let cross = cross.min(cross_total).max(0.0);
                let align = if cd.is_none() || matches!(cd, Some(Dim::Fill(_))) { Align::Stretch } else { spec.align };
                let off = match align {
                    Align::Start | Align::Stretch => 0.0,
                    Align::Center => (cross_total - cross) / 2.0,
                    Align::End => cross_total - cross,
                };
                let cs = if align == Align::Stretch { cross_total } else { cross };
                out.push(if rows { Rect::new(inner.x + off, pos, cs, main[i]) } else { Rect::new(pos, inner.y + off, main[i], cs) });
                pos += main[i] + spec.gap;
            }
            out
        }
        Kind::Grid(cols) => {
            let rows = n.div_ceil(cols);
            let cw = ((inner.w - spec.gap * (cols - 1) as f64) / cols as f64).max(0.0);
            let rh = ((inner.h - spec.gap * (rows.saturating_sub(1)) as f64) / rows as f64).max(0.0);
            (0..n)
                .map(|i| {
                    let (r, c) = (i / cols, i % cols);
                    Rect::new(inner.x + c as f64 * (cw + spec.gap), inner.y + r as f64 * (rh + spec.gap), cw, rh)
                })
                .collect()
        }
    }
}

fn cross_sizes(i: usize, c: &Child, aw: f64, ah: f64, measure: &mut dyn FnMut(usize, f64, f64) -> (f64, f64)) -> (Option<f64>, Option<f64>) {
    let mut measured: Option<(f64, f64)> = None;
    let mut get = |d: Option<Dim>, avail: f64, is_w: bool| -> Option<f64> {
        match d {
            None | Some(Dim::Fill(_)) => None,
            Some(Dim::Px(v)) => Some(v),
            Some(Dim::Percent(p)) => Some(p * avail),
            Some(Dim::Auto) => {
                let m = *measured.get_or_insert_with(|| measure(i, aw, ah));
                Some(if is_w { m.0 } else { m.1 })
            }
        }
    };
    (get(c.w, aw, true), get(c.h, ah, false))
}

fn place_cross(inner: Rect, w: Option<f64>, h: Option<f64>, align: Align) -> Rect {
    let (w, h) = (w.unwrap_or(inner.w).min(inner.w), h.unwrap_or(inner.h).min(inner.h));
    let off = |free: f64| match align {
        Align::Start | Align::Stretch => 0.0,
        Align::Center => free / 2.0,
        Align::End => free,
    };
    Rect::new(inner.x + off(inner.w - w), inner.y + off(inner.h - h), w, h)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn no_measure(_: usize, _: f64, _: f64) -> (f64, f64) {
        (0.0, 0.0)
    }

    #[test]
    fn plot_with_measured_axes() {
        // columns: [y-axis auto (labels 34 px wide), plot fill]
        let spec = Spec { kind: Kind::Columns, gap: 4.0, padding: [10.0, 10.0, 10.0, 10.0], align: Align::Stretch };
        let kids = [Child { w: Some(Dim::Auto), h: None }, Child { w: None, h: None }];
        let r = layout(Rect::new(0.0, 0.0, 400.0, 300.0), &spec, &kids, &mut |i, _, _| if i == 0 { (34.0, 0.0) } else { (0.0, 0.0) });
        assert_eq!(r[0], Rect::new(10.0, 10.0, 34.0, 280.0));
        assert_eq!(r[1], Rect::new(48.0, 10.0, 342.0, 280.0));
    }

    #[test]
    fn rows_with_fixed_percent_and_weighted_fills() {
        let spec = Spec { kind: Kind::Rows, ..Default::default() };
        let kids = [
            Child { h: Some(Dim::Px(20.0)), w: None },
            Child { h: Some(Dim::Percent(0.1)), w: None },
            Child { h: Some(Dim::Fill(1.0)), w: None },
            Child { h: Some(Dim::Fill(3.0)), w: None },
        ];
        let r = layout(Rect::new(0.0, 0.0, 100.0, 200.0), &spec, &kids, &mut no_measure);
        assert_eq!(r.iter().map(|b| b.h).collect::<Vec<_>>(), vec![20.0, 20.0, 40.0, 120.0]);
        assert_eq!(r[3].y, 80.0);
    }

    #[test]
    fn grid_and_centered_stack() {
        let g = layout(Rect::new(0.0, 0.0, 210.0, 100.0), &Spec { kind: Kind::Grid(2), gap: 10.0, ..Default::default() }, &[Child::default(); 3], &mut no_measure);
        assert_eq!(g[2], Rect::new(0.0, 55.0, 100.0, 45.0));
        let s = layout(
            Rect::new(0.0, 0.0, 100.0, 100.0),
            &Spec { align: Align::Center, ..Default::default() },
            &[Child { w: Some(Dim::Px(20.0)), h: Some(Dim::Px(10.0)) }],
            &mut no_measure,
        );
        assert_eq!(s[0], Rect::new(40.0, 45.0, 20.0, 10.0));
    }

    #[test]
    fn dims_parse_from_json() {
        assert_eq!(Dim::from_json(&serde_json::json!(12)), Some(Dim::Px(12.0)));
        assert_eq!(Dim::from_json(&serde_json::json!("auto")), Some(Dim::Auto));
        assert_eq!(Dim::from_json(&serde_json::json!("25%")), Some(Dim::Percent(0.25)));
        assert_eq!(Dim::from_json(&serde_json::json!({"fill": 2})), Some(Dim::Fill(2.0)));
    }
}
