//! SVG path data: the reader every `d` goes through (`geom.path`, icons) and writers back to text.
//!
//! The reader takes M L H V C S Q T A Z, absolute and relative, numbers in any SVG spelling
//! (`.5.5`, `1e-3`, flags run into what follows). Arcs become cubic Béziers of at most a quarter
//! turn (deterministic trigonometry from `datars_math::m`). A malformed command ends the path
//! there, keeping what came before (as browsers do).

use datars_math::{PathData, PathEl, Vec2};

/// A small SVG path parser: M L H V C S Q T A Z, absolute and relative.
pub fn parse_svg_path(d: &str) -> PathData {
    let mut p = PathData::new();
    let bytes = d.as_bytes();
    let mut i = 0;
    let mut cmd = b'M';
    let (mut cur, mut start, mut last_c) = (Vec2::ZERO, Vec2::ZERO, Vec2::ZERO);
    // The family of the previous segment (b'C' cubic, b'Q' quadratic, 0 other): S and T reflect
    // its control point only after one of their own kind, else they start at the current point.
    let mut prev: u8 = 0;
    let num = |i: &mut usize| -> Option<f64> {
        while *i < bytes.len() && (bytes[*i].is_ascii_whitespace() || bytes[*i] == b',') {
            *i += 1;
        }
        let st = *i;
        if *i < bytes.len() && (bytes[*i] == b'-' || bytes[*i] == b'+') {
            *i += 1;
        }
        let mut dot = false;
        while *i < bytes.len() && (bytes[*i].is_ascii_digit() || (bytes[*i] == b'.' && !dot)) {
            dot |= bytes[*i] == b'.';
            *i += 1;
        }
        if *i < bytes.len() && (bytes[*i] == b'e' || bytes[*i] == b'E') {
            *i += 1;
            if *i < bytes.len() && (bytes[*i] == b'-' || bytes[*i] == b'+') {
                *i += 1;
            }
            while *i < bytes.len() && bytes[*i].is_ascii_digit() {
                *i += 1;
            }
        }
        std::str::from_utf8(&bytes[st..*i]).ok()?.parse().ok()
    };
    loop {
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b',') {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        if bytes[i].is_ascii_alphabetic() {
            cmd = bytes[i];
            i += 1;
            if cmd == b'Z' || cmd == b'z' {
                p.close();
                cur = start;
                prev = 0;
                continue;
            }
        }
        let rel = cmd.is_ascii_lowercase();
        let base = if rel { cur } else { Vec2::ZERO };
        let pt = |i: &mut usize| -> Option<Vec2> { Some(Vec2::new(num(i)?, num(i)?)) };
        let up = cmd.to_ascii_uppercase();
        let family = match up {
            b'C' | b'S' => b'C',
            b'Q' | b'T' => b'Q',
            _ => 0,
        };
        match up {
            b'M' => {
                let Some(q) = pt(&mut i) else { break };
                cur = base + q;
                start = cur;
                p.move_to(cur);
                cmd = if rel { b'l' } else { b'L' };
            }
            b'L' => {
                let Some(q) = pt(&mut i) else { break };
                cur = base + q;
                p.line_to(cur);
            }
            b'H' => {
                let Some(x) = num(&mut i) else { break };
                cur = Vec2::new(if rel { cur.x + x } else { x }, cur.y);
                p.line_to(cur);
            }
            b'V' => {
                let Some(y) = num(&mut i) else { break };
                cur = Vec2::new(cur.x, if rel { cur.y + y } else { y });
                p.line_to(cur);
            }
            b'C' => {
                let (Some(a), Some(b), Some(c)) = (pt(&mut i), pt(&mut i), pt(&mut i)) else { break };
                let (a, b, c) = (base + a, base + b, base + c);
                p.cubic_to(a, b, c);
                last_c = b;
                cur = c;
            }
            b'S' => {
                let (Some(b), Some(c)) = (pt(&mut i), pt(&mut i)) else { break };
                let a = if prev == b'C' { cur * 2.0 - last_c } else { cur };
                let (b, c) = (base + b, base + c);
                p.cubic_to(a, b, c);
                last_c = b;
                cur = c;
            }
            b'Q' => {
                let (Some(a), Some(c)) = (pt(&mut i), pt(&mut i)) else { break };
                let (a, c) = (base + a, base + c);
                p.quad_to(a, c);
                last_c = a;
                cur = c;
            }
            b'T' => {
                let Some(c) = pt(&mut i) else { break };
                let a = if prev == b'Q' { cur * 2.0 - last_c } else { cur };
                let c = base + c;
                p.quad_to(a, c);
                last_c = a;
                cur = c;
            }
            b'A' => {
                // rx ry rotation large-arc sweep x y. The flags are single digits, which SVG lets
                // run into what follows (`a10 10 0 1017 20`).
                let flag = |i: &mut usize| -> Option<bool> {
                    while *i < bytes.len() && (bytes[*i].is_ascii_whitespace() || bytes[*i] == b',') {
                        *i += 1;
                    }
                    let f = match bytes.get(*i)? {
                        b'0' => false,
                        b'1' => true,
                        _ => return None,
                    };
                    *i += 1;
                    Some(f)
                };
                let (Some(rx), Some(ry), Some(rot), Some(large), Some(sweep), Some(q)) = (num(&mut i), num(&mut i), num(&mut i), flag(&mut i), flag(&mut i), pt(&mut i)) else { break };
                let to = base + q;
                arc_to(&mut p, cur, rx, ry, rot, large, sweep, to);
                cur = to;
            }
            _ => break,
        }
        prev = family;
    }
    p
}

/// An SVG elliptical arc from `from` to `to` (SVG 1.1 F.6.5: endpoint to centre form, radii grown
/// when too small to reach), as cubic Béziers of at most a quarter turn each.
#[allow(clippy::too_many_arguments)]
fn arc_to(p: &mut PathData, from: Vec2, rx: f64, ry: f64, rot_deg: f64, large: bool, sweep: bool, to: Vec2) {
    use datars_math::m;
    if from == to {
        return;
    }
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx == 0.0 || ry == 0.0 || !rx.is_finite() || !ry.is_finite() {
        p.line_to(to);
        return;
    }
    let (sin_p, cos_p) = m::sin_cos(rot_deg.to_radians());
    let (dx, dy) = ((from.x - to.x) / 2.0, (from.y - to.y) / 2.0);
    let (x1, y1) = (cos_p * dx + sin_p * dy, -sin_p * dx + cos_p * dy);
    let lambda = x1 * x1 / (rx * rx) + y1 * y1 / (ry * ry);
    if lambda > 1.0 {
        rx *= lambda.sqrt();
        ry *= lambda.sqrt();
    }
    let (rx2, ry2) = (rx * rx, ry * ry);
    let den = rx2 * y1 * y1 + ry2 * x1 * x1;
    let coef = if den > 0.0 { ((rx2 * ry2 - den) / den).max(0.0).sqrt() } else { 0.0 } * if large == sweep { -1.0 } else { 1.0 };
    let (cxp, cyp) = (coef * rx * y1 / ry, -coef * ry * x1 / rx);
    let c = Vec2::new(cos_p * cxp - sin_p * cyp + (from.x + to.x) / 2.0, sin_p * cxp + cos_p * cyp + (from.y + to.y) / 2.0);
    let angle = |ux: f64, uy: f64, vx: f64, vy: f64| m::atan2(ux * vy - uy * vx, ux * vx + uy * vy);
    let (ux, uy) = ((x1 - cxp) / rx, (y1 - cyp) / ry);
    let (vx, vy) = ((-x1 - cxp) / rx, (-y1 - cyp) / ry);
    let start = angle(1.0, 0.0, ux, uy);
    let mut span = angle(ux, uy, vx, vy);
    if !sweep && span > 0.0 {
        span -= m::TAU;
    } else if sweep && span < 0.0 {
        span += m::TAU;
    }
    let at = |a: f64| {
        let (s, co) = m::sin_cos(a);
        (Vec2::new(c.x + rx * cos_p * co - ry * sin_p * s, c.y + rx * sin_p * co + ry * cos_p * s), Vec2::new(-rx * cos_p * s - ry * sin_p * co, -rx * sin_p * s + ry * cos_p * co))
    };
    let n = (span.abs() / (m::PI / 2.0)).ceil().max(1.0) as usize;
    let step = span / n as f64;
    let k = 4.0 / 3.0 * m::tan(step / 4.0);
    for j in 0..n {
        let (a0, a1) = (start + step * j as f64, start + step * (j + 1) as f64);
        let ((p0, d0), (p1, d1)) = (at(a0), at(a1));
        let end = if j + 1 == n { to } else { p1 };
        p.cubic_to(p0 + d0 * k, p1 - d1 * k, end);
    }
}

/// SVG path data for a path, to 2 decimals (for expressions that return geometry, e.g.
/// `geo.geodesic`: screen px, where a hundredth is plenty).
pub fn svg_path_string(p: &PathData) -> String {
    let mut out = String::new();
    let f = |v: f64| format!("{:.2}", v);
    for e in &p.els {
        match e {
            PathEl::Move { p } => out += &format!("M{} {}", f(p.x), f(p.y)),
            PathEl::Line { p } => out += &format!("L{} {}", f(p.x), f(p.y)),
            PathEl::Quad { c, p } => out += &format!("Q{} {} {} {}", f(c.x), f(c.y), f(p.x), f(p.y)),
            PathEl::Cubic { c1, c2, p } => out += &format!("C{} {} {} {} {} {}", f(c1.x), f(c1.y), f(c2.x), f(c2.y), f(p.x), f(p.y)),
            PathEl::Close => out += "Z",
        }
    }
    out
}

/// SVG path data that reads back to exactly this path: absolute commands, every coordinate in its
/// shortest round-tripping spelling (what bundles carry icons as).
pub fn exact_path_string(p: &PathData) -> String {
    let mut out = String::new();
    let mut put = |c: char, vs: &[f64]| {
        out.push(c);
        for (i, v) in vs.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            // `-0` reads back as 0 — the same point.
            let v = if *v == 0.0 { 0.0 } else { *v };
            out.push_str(&format!("{v}"));
        }
    };
    for e in &p.els {
        match *e {
            PathEl::Move { p } => put('M', &[p.x, p.y]),
            PathEl::Line { p } => put('L', &[p.x, p.y]),
            PathEl::Quad { c, p } => put('Q', &[c.x, c.y, p.x, p.y]),
            PathEl::Cubic { c1, c2, p } => put('C', &[c1.x, c1.y, c2.x, c2.y, p.x, p.y]),
            PathEl::Close => put('Z', &[]),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ends(d: &str) -> Vec<Vec2> {
        parse_svg_path(d).els.iter().filter_map(|e| match e { PathEl::Move { p } | PathEl::Line { p } | PathEl::Quad { p, .. } | PathEl::Cubic { p, .. } => Some(*p), PathEl::Close => None }).collect()
    }

    #[test]
    fn an_arc_is_a_curve_through_the_circle_not_a_chord() {
        // A half circle of radius 50 from (0, 50) to (100, 50), sweeping through the top (y = 0).
        let p = parse_svg_path("M0 50 A50 50 0 0 1 100 50");
        let cubics: Vec<_> = p.els.iter().filter(|e| matches!(e, PathEl::Cubic { .. })).collect();
        assert_eq!(cubics.len(), 2, "two quarter turns: {:?}", p.els);
        let pts = ends("M0 50 A50 50 0 0 1 100 50");
        assert!((pts[1] - Vec2::new(50.0, 0.0)).len() < 1e-9, "through the top: {:?}", pts);
        assert!((pts[2] - Vec2::new(100.0, 50.0)).len() < 1e-12, "ends exactly where it says");
        // The other sweep goes through the bottom.
        assert!((ends("M0 50 A50 50 0 0 0 100 50")[1] - Vec2::new(50.0, 100.0)).len() < 1e-9);
    }

    #[test]
    fn flags_may_run_into_the_numbers_after_them_and_radii_grow_to_reach() {
        // Radius 1 can't span 100 px: it grows to 50 (a half circle), as SVG says.
        let a = ends("M0 50 A1 1 0 0 1 100 50");
        assert!((a[1] - Vec2::new(50.0, 0.0)).len() < 1e-9, "{a:?}");
        // `a50 50 0 01100 0` is rx 50, ry 50, rotation 0, large 0, sweep 1, then 100 0.
        let b = ends("M0 50 a50 50 0 01100 0");
        assert!((b.last().unwrap().x - 100.0).abs() < 1e-9 && (b[1].y - 0.0).abs() < 1e-9, "{b:?}");
    }

    #[test]
    fn a_smooth_curve_reflects_only_a_curve_of_its_own_kind() {
        // After a line, S starts its first control point at the current point.
        let p = parse_svg_path("M0 0 L10 0 S20 10 30 0");
        let PathEl::Cubic { c1, .. } = p.els[2] else { panic!("{:?}", p.els) };
        assert_eq!(c1, Vec2::new(10.0, 0.0));
        // After a cubic, it mirrors the cubic's second control point.
        let p = parse_svg_path("M0 0 C0 10 10 10 10 0 S20 -10 20 0");
        let PathEl::Cubic { c1, .. } = p.els[2] else { panic!() };
        assert_eq!(c1, Vec2::new(10.0, -10.0));
        // T after a cubic doesn't mirror the cubic.
        let p = parse_svg_path("M0 0 C0 10 10 10 10 0 T20 0");
        let PathEl::Quad { c, .. } = p.els[2] else { panic!() };
        assert_eq!(c, Vec2::new(10.0, 0.0));
    }

    #[test]
    fn numbers_in_every_spelling_svg_allows() {
        // Lucide writes `.56.56` for two numbers and `9.591-3.676` for two more.
        let p = parse_svg_path("M2 9.5a5.5 5.5 0 0 1 9.591-3.676.56.56 0 0 0 .818 0");
        assert!(p.els.len() > 3, "{:?}", p.els);
        assert_eq!(ends("M1e1 2E-1h.5").last().copied(), Some(Vec2::new(10.5, 0.2)));
    }

    #[test]
    fn the_exact_writer_reads_back_to_the_same_path() {
        let p = parse_svg_path("M2 9.5a5.5 5.5 0 0 1 9.591-3.676.56.56 0 0 0 .818 0A5.49 5.49 0 0 1 22 9.5c0 2.29-1.5 4-3 5.5l-5.492 5.313q1 1 2 2z");
        let back = parse_svg_path(&exact_path_string(&p));
        assert_eq!(back, p);
    }
}
