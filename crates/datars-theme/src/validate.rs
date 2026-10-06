//! Validation of a resolved theme against its declared checks (contrast, distinguishability under
//! colour-vision deficiencies, monotone ramps). Used by `datars lint`, theme editors, and tests.

use crate::{Check, ResolvedTheme};
use datars_color::{contrast_ratio, delta_e, simulate_cvd, Cvd};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    pub severity: Severity,
    pub check: String,
    pub message: String,
}

pub fn validate(t: &ResolvedTheme) -> Vec<Finding> {
    let mut out = Vec::new();
    for c in &t.checks {
        match c {
            Check::Contrast { contrast, min } => {
                let (fg, bg) = (&contrast[0], &contrast[1]);
                match (t.color(fg), t.color(bg)) {
                    (Some(a), Some(b)) => {
                        let r = contrast_ratio(a.over(b), b);
                        if r < *min {
                            out.push(Finding {
                                severity: if r < 3.0 { Severity::Error } else { Severity::Warning },
                                check: format!("contrast {fg}/{bg}"),
                                message: format!("{r:.2}:1, needs {min}:1 ({} on {})", a.to_hex(), b.to_hex()),
                            });
                        }
                    }
                    _ => out.push(Finding { severity: Severity::Error, check: format!("contrast {fg}/{bg}"), message: "token missing".into() }),
                }
            }
            Check::Distinct { distinct, min, cvd, first } => {
                let Some(p) = t.palette(distinct) else {
                    out.push(Finding { severity: Severity::Error, check: format!("distinct {distinct}"), message: "palette missing".into() });
                    continue;
                };
                let colors: Vec<datars_color::Color> = p.colors.iter().take(first.unwrap_or(usize::MAX)).copied().collect();
                let mut views: Vec<(&str, Vec<datars_color::Color>)> = vec![("normal vision", colors.clone())];
                if *cvd {
                    for (n, k) in [("deuteranopia", Cvd::Deuteranopia), ("protanopia", Cvd::Protanopia), ("tritanopia", Cvd::Tritanopia)] {
                        views.push((n, colors.iter().map(|c| simulate_cvd(*c, k)).collect()));
                    }
                }
                for (view, cs) in views {
                    let mut worst = (f64::INFINITY, 0, 0);
                    for i in 0..cs.len() {
                        for j in i + 1..cs.len() {
                            let d = delta_e(cs[i], cs[j]);
                            if d < worst.0 {
                                worst = (d, i, j);
                            }
                        }
                    }
                    // Colour-vision views get a looser bar: they can't all be as distinct as normal vision.
                    let bar = if view == "normal vision" { *min } else { min * 0.5 };
                    if worst.0 < bar {
                        out.push(Finding {
                            severity: Severity::Warning,
                            check: format!("distinct {distinct}"),
                            message: format!("colours {} and {} differ by ΔE {:.1} under {view} (want ≥ {bar:.1})", worst.1, worst.2, worst.0),
                        });
                    }
                }
            }
            Check::Monotone { monotone } => {
                if let Some(p) = t.palette(monotone) {
                    let ls: Vec<f64> = p.colors.iter().map(|c| c.to_oklab().l).collect();
                    let up = ls.windows(2).all(|w| w[1] >= w[0]);
                    let down = ls.windows(2).all(|w| w[1] <= w[0]);
                    if !(up || down) {
                        out.push(Finding { severity: Severity::Warning, check: format!("monotone {monotone}"), message: "lightness is not monotone".into() });
                    }
                }
            }
        }
    }
    out
}
