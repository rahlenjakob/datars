//! Palette helpers (colour math only). Palettes themselves are theme tokens (`datars-theme`).

use crate::Color;

/// Evaluate a multi-stop ramp at n ∈ [0, 1] (OKLab between stops).
pub fn ramp(stops: &[Color], n: f64) -> Color {
    match stops.len() {
        0 => Color::BLACK,
        1 => stops[0],
        k => {
            let p = n.clamp(0.0, 1.0) * (k - 1) as f64;
            let i = (p.floor() as usize).min(k - 2);
            stops[i].lerp_oklab(stops[i + 1], p - i as f64)
        }
    }
}

/// The smallest pairwise ΔE (OKLab ×100) in a set of colours — a distinguishability measure.
pub fn min_pairwise_delta_e(colors: &[Color]) -> f64 {
    let mut best = f64::INFINITY;
    for i in 0..colors.len() {
        for j in i + 1..colors.len() {
            best = best.min(crate::delta_e(colors[i], colors[j]));
        }
    }
    best
}

/// `n` categorical colours starting from a brand colour, as far apart as a greedy search can put
/// them — measured as the smallest ΔE with normal vision *and* under protanopia, deuteranopia and
/// tritanopia, so a generated palette passes the colour-blind checks themes declare. Candidates are
/// an OKLCH grid inside sRGB at mid lightness (every colour reads on light and dark paper).
/// Deterministic: ties go to the first candidate in grid order.
pub fn generate_categorical(brand: Color, n: usize) -> Vec<Color> {
    use crate::{delta_e, simulate_cvd, Cvd};
    let views = |c: Color| [c, simulate_cvd(c, Cvd::Protanopia), simulate_cvd(c, Cvd::Deuteranopia), simulate_cvd(c, Cvd::Tritanopia)];
    let apart = |a: &[Color; 4], b: &[Color; 4]| (0..4).map(|k| delta_e(a[k], b[k])).fold(f64::INFINITY, f64::min);
    let mut cands: Vec<(Color, [Color; 4])> = Vec::new();
    for li in [0.50, 0.58, 0.66, 0.74, 0.82] {
        for ci in [0.08, 0.12, 0.16, 0.20] {
            for hi in 0..36 {
                let want = crate::Oklab::from_lch(li, ci, datars_math::m::TAU * hi as f64 / 36.0, 1.0);
                let col = want.to_color();
                let got = col.to_oklab();
                // Outside sRGB the conversion clamps: keep only colours that come back as asked.
                if ((got.l - want.l).powi(2) + (got.a - want.a).powi(2) + (got.b - want.b).powi(2)).sqrt() * 100.0 < 1.0 {
                    cands.push((col, views(col)));
                }
            }
        }
    }
    let mut out: Vec<(Color, [Color; 4])> = vec![(brand, views(brand))];
    while out.len() < n {
        let mut best: Option<(f64, usize)> = None;
        for (i, (_, v)) in cands.iter().enumerate() {
            let d = out.iter().map(|(_, o)| apart(v, o)).fold(f64::INFINITY, f64::min);
            if best.is_none_or(|(bd, _)| d > bd) {
                best = Some((d, i));
            }
        }
        let Some((_, i)) = best else { break };
        out.push(cands.remove(i));
    }
    out.into_iter().take(n).map(|(c, _)| c).collect()
}

/// A sequential ramp from a light tint to a dark shade of `base`, in OKLCH.
pub fn generate_sequential(base: Color, n: usize) -> Vec<Color> {
    let (_, c, h) = base.to_oklab().to_lch();
    (0..n.max(2))
        .map(|i| {
            let t = i as f64 / (n.max(2) - 1) as f64;
            crate::Oklab::from_lch(0.96 - 0.62 * t, 0.02 + c * t, h, 1.0).to_color()
        })
        .collect()
}

#[cfg(test)]
mod generator_tests {
    use super::*;
    use crate::{delta_e, simulate_cvd, Color, Cvd};

    #[test]
    fn generated_palettes_stay_distinct_for_colour_blind_readers() {
        for hex in ["#c2410c", "#1d4e89", "#ef6092", "#2fa36f", "#4269d0", "#7c5cff", "#111111"] {
            let brand = Color::parse(hex).unwrap();
            let p = generate_categorical(brand, 8);
            assert_eq!(p.len(), 8);
            assert_eq!(p[0], brand, "the brand leads");
            let mut worst = f64::INFINITY;
            for i in 0..8 {
                for j in i + 1..8 {
                    for sim in [None, Some(Cvd::Protanopia), Some(Cvd::Deuteranopia), Some(Cvd::Tritanopia)] {
                        let f = |c: Color| sim.map_or(c, |k| simulate_cvd(c, k));
                        worst = worst.min(delta_e(f(p[i]), f(p[j])));
                    }
                }
            }
            assert!(worst >= 8.0, "{hex}: two colours only ΔE {worst:.1} apart");
            assert_eq!(generate_categorical(brand, 8), p, "deterministic");
        }
    }
}
