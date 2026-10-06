//! GPU equivalence (docs/13-testing.md, layer 10): every example state rendered by the wgpu
//! backend on this machine's GPU against the CPU reference. The one place a fuzzy comparison
//! belongs: rasterizers differ at antialiased edges, and a driver update must not fail CI — but a
//! wrong colour, a missing mark or a shifted shape must.

use datars_color::Color;
use datars_engine::Engine;
use std::path::Path;

/// Per-state comparison. ΔE is OKLab ×100 (≈1–2 is a just-noticeable difference).
#[derive(Clone, Debug)]
pub struct GpuState {
    pub example: String,
    pub state: String,
    pub mean_de: f64,
    pub p99_de: f64,
    /// Share of pixels with ΔE > 10 — visibly different.
    pub visible: f64,
    /// Visibly different pixels whose four neighbours also differ visibly: an area, not an edge.
    /// Rasterizers disagree along antialiased edges (about a pixel wide); a missing, moved or
    /// wrongly coloured mark has an interior. This is what catches a lost label.
    pub solid: usize,
    pub pass: bool,
    pub heatmap: Option<String>,
}

/// Thresholds: mean ΔE ≤ 1.0, at most 0.5 % of pixels visibly different, and no solid regions
/// (3×3 blocks of visibly different pixels) beyond a few stray ones.
pub const MAX_MEAN_DE: f64 = 1.0;
pub const MAX_VISIBLE: f64 = 0.005;
pub const MAX_SOLID: usize = 8;

fn color(px: &[u8], i: usize) -> Color {
    Color { r: px[i] as f32 / 255.0, g: px[i + 1] as f32 / 255.0, b: px[i + 2] as f32 / 255.0, a: px[i + 3] as f32 / 255.0 }
}

pub struct Comparison {
    pub mean: f64,
    pub p99: f64,
    pub visible: f64,
    pub solid: usize,
    pub heatmap: datars_render_cpu::Pixmap,
}

/// Compare two RGBA8 buffers of the same size.
pub fn compare(a: &[u8], b: &[u8], w: u32, h: u32) -> Comparison {
    let n = (w * h) as usize;
    let mut des: Vec<f64> = Vec::with_capacity(n);
    let mut heat = datars_render_cpu::Pixmap { width: w, height: h, data: vec![255; n * 4] };
    for p in 0..n {
        let i = p * 4;
        let de = if a[i..i + 4] == b[i..i + 4] { 0.0 } else { datars_color::delta_e(color(a, i), color(b, i)) };
        des.push(de);
        // Grey context, red where it differs (brighter = larger).
        let g = (a[i] as u32 + a[i + 1] as u32 + a[i + 2] as u32) / 3;
        let k = (de / 20.0).min(1.0);
        heat.data[i] = (g as f64 * 0.3 + 255.0 * 0.7 * (1.0 - k) + 255.0 * k) as u8;
        heat.data[i + 1] = ((g as f64 * 0.3 + 255.0 * 0.7) * (1.0 - k)) as u8;
        heat.data[i + 2] = ((g as f64 * 0.3 + 255.0 * 0.7) * (1.0 - k)) as u8;
    }
    let mean = des.iter().sum::<f64>() / n.max(1) as f64;
    let vis = |x: u32, y: u32| des[(y * w + x) as usize] > 10.0;
    let visible = des.iter().filter(|d| **d > 10.0).count() as f64 / n.max(1) as f64;
    // A region: a visibly different pixel whose whole 3×3 neighbourhood differs too — anything
    // 3 px thick that one renderer drew and the other didn't. Four neighbours alone also caught
    // the pixels where anti-aliased hairlines cross (a globe's country borders meeting), where
    // 4×MSAA and exact coverage round differently: noise along lines, not a missing mark.
    let mut solid = 0;
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            if (y - 1..=y + 1).all(|yy| (x - 1..=x + 1).all(|xx| vis(xx, yy))) {
                solid += 1;
            }
        }
    }
    des.sort_by(|x, y| datars_math::total_cmp(*x, *y));
    let p99 = des.get(n * 99 / 100).copied().unwrap_or(0.0);
    Comparison { mean, p99, visible, solid, heatmap: heat }
}

/// Run every example (matching `filter`) through both renderers at `dpr`.
pub fn run(root: &Path, filter: &str, dpr: f64, out_dir: &Path) -> Result<Vec<GpuState>, String> {
    let _ = std::fs::create_dir_all(out_dir);
    let mut out = Vec::new();
    let mut names: Vec<String> = std::fs::read_dir(root.join("examples")).map_err(|e| e.to_string())?.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    let mut gpu: Option<datars_render_wgpu::Renderer> = None;
    for name in names.into_iter().filter(|n| n.contains(filter)) {
        let path = root.join("examples").join(&name).join("doc.json");
        let Ok(json) = std::fs::read_to_string(&path) else { continue };
        let mut engine: Engine = datars_headless::load_at(&json, path.parent())?;
        let vp = engine.viewport();
        let (w, h) = (((vp.width * dpr).round() as u32).max(1), ((vp.height * dpr).round() as u32).max(1));
        let r = match &mut gpu {
            Some(r) => {
                r.resize(w, h, dpr);
                r
            }
            None => gpu.insert(datars_render_wgpu::Renderer::headless(w, h, dpr).map_err(|e| format!("no GPU: {e}"))?),
        };
        for (i, st) in engine.state_names().into_iter().enumerate() {
            let scene = engine.scene_for_state(i);
            let list = engine.display_list(&scene);
            let cpu = datars_render_cpu::render(&list, engine.fonts(), dpr);
            // The settled frame: while stand-in meshes remain (a frame builds its share of exact
            // ones), draw again, as a host does.
            r.render(&list, engine.fonts()).map_err(|e| e.to_string())?;
            for _ in 0..500 {
                if r.stats().stand_ins == 0 {
                    break;
                }
                r.render(&list, engine.fonts()).map_err(|e| e.to_string())?;
            }
            let g = r.try_read_rgba().map_err(|e| e.to_string())?;
            if g.len() != cpu.data.len() {
                return Err(format!("{name}/{st}: size mismatch"));
            }
            let c = compare(&cpu.data, &g, w, h);
            let (mean, p99, visible, solid, heat) = (c.mean, c.p99, c.visible, c.solid, c.heatmap);
            let pass = mean <= MAX_MEAN_DE && visible <= MAX_VISIBLE && solid <= MAX_SOLID;
            let heatmap = (!pass).then(|| {
                let p = out_dir.join(format!("{name}-{st}-gpu-diff.png"));
                let _ = std::fs::write(&p, heat.to_png());
                let _ = std::fs::write(out_dir.join(format!("{name}-{st}-gpu.png")), datars_render_cpu::Pixmap { width: w, height: h, data: g.clone() }.to_png());
                // The reference it was compared with (the same frame: a clock-driven chart drawn
                // later by `datars render` would have moved on).
                let _ = std::fs::write(out_dir.join(format!("{name}-{st}-cpu.png")), cpu.to_png());
                p.display().to_string()
            });
            out.push(GpuState { example: name.clone(), state: st, mean_de: mean, p99_de: p99, visible, solid, pass, heatmap });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas(w: u32, h: u32) -> Vec<u8> {
        vec![255; (w * h * 4) as usize]
    }
    fn paint(px: &mut [u8], w: u32, x0: u32, y0: u32, x1: u32, y1: u32, v: u8) {
        for y in y0..y1 {
            for x in x0..x1 {
                let i = ((y * w + x) * 4) as usize;
                px[i..i + 3].copy_from_slice(&[v, v, v]);
            }
        }
    }

    #[test]
    fn edge_noise_is_not_solid_but_a_missing_mark_is() {
        let (w, h) = (64, 64);
        let mut a = canvas(w, h);
        paint(&mut a, w, 10, 10, 30, 30, 0);
        // An antialiasing disagreement: the edge column of the square is grey instead of black.
        let mut b = a.clone();
        paint(&mut b, w, 29, 10, 30, 30, 128);
        assert_eq!(compare(&a, &b, w, h).solid, 0);
        // A missing 6×6 mark: solid.
        let mut c = a.clone();
        paint(&mut a, w, 40, 40, 46, 46, 0);
        paint(&mut c, w, 40, 40, 46, 46, 255);
        assert!(compare(&a, &c, w, h).solid >= 16);
        // A missing line 3 px thick: solid along its middle row.
        let mut d = a.clone();
        paint(&mut a, w, 2, 50, 60, 53, 0);
        paint(&mut d, w, 2, 50, 60, 53, 255);
        assert!(compare(&a, &d, w, h).solid >= 50);
    }

    #[test]
    fn hairline_crossings_rounding_differently_are_not_solid() {
        // Two one-pixel lines crossing, each drawn a shade off in the other renderer (MSAA's
        // coverage steps against exact coverage): a plus of differing pixels, no region.
        let (w, h) = (32, 32);
        let mut a = canvas(w, h);
        paint(&mut a, w, 0, 15, 32, 16, 40);
        paint(&mut a, w, 15, 0, 16, 32, 40);
        let mut b = canvas(w, h);
        paint(&mut b, w, 0, 15, 32, 16, 200);
        paint(&mut b, w, 15, 0, 16, 32, 200);
        let c = compare(&a, &b, w, h);
        assert!(c.visible > 0.0, "the lines do differ");
        // The crossing pixel and its four neighbours all differ (what used to count), its diagonal
        // neighbours don't.
        assert_eq!(c.solid, 0);
    }
}
