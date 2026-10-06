//! Integer compositing on premultiplied RGBA8. Every operation rounds to nearest with one
//! division by 255 per result, so it is exact and identical on every target.

use datars_color::Color;
use datars_scene::Blend;

/// `round(x / 255)` for `x` in `0..=65535`, without a division.
#[inline]
pub fn div255(x: u32) -> u32 {
    let t = x + 128;
    (t + (t >> 8)) >> 8
}

/// A straight-alpha colour as premultiplied RGBA8, with its alpha multiplied by `opacity`.
pub fn premul(c: Color, opacity: f32) -> [u8; 4] {
    let a = unit(c.a) * unit(opacity);
    let q = |v: f64| (v * 255.0 + 0.5).floor() as u8;
    [q(unit(c.r) * a), q(unit(c.g) * a), q(unit(c.b) * a), q(a)]
}

/// Clamp into [0, 1] as f64, mapping NaN to 0.
#[inline]
pub fn unit(v: f32) -> f64 {
    if v.is_nan() {
        0.0
    } else {
        (v as f64).clamp(0.0, 1.0)
    }
}

/// Scale a premultiplied pixel by `k` / 255.
#[inline]
pub fn scale(p: [u8; 4], k: u32) -> [u8; 4] {
    [div255(p[0] as u32 * k) as u8, div255(p[1] as u32 * k) as u8, div255(p[2] as u32 * k) as u8, div255(p[3] as u32 * k) as u8]
}

/// Source-over: `d = s + d·(1 − sa)`.
#[inline]
pub fn over(d: &mut [u8], s: [u8; 4]) {
    let ia = 255 - s[3] as u32;
    if ia == 0 {
        d[..4].copy_from_slice(&s);
        return;
    }
    for i in 0..4 {
        d[i] = (s[i] as u32 + div255(d[i] as u32 * ia)) as u8;
    }
}

/// Composite `s` onto `d` with a separable blend mode (premultiplied formulas, one rounding each).
#[inline]
pub fn blend(d: &mut [u8], s: [u8; 4], mode: Blend) {
    match mode {
        Blend::Normal => over(d, s),
        Blend::Multiply => {
            // s·d + s·(1 − da) + d·(1 − sa)
            let (sa, da) = (s[3] as u32, d[3] as u32);
            for i in 0..4 {
                let (sc, dc) = (s[i] as u32, d[i] as u32);
                d[i] = div255(sc * dc + sc * (255 - da) + dc * (255 - sa)).min(255) as u8;
            }
        }
        Blend::Screen => {
            // s + d − s·d
            for i in 0..4 {
                let (sc, dc) = (s[i] as u32, d[i] as u32);
                d[i] = div255(255 * sc + 255 * dc - sc * dc).min(255) as u8;
            }
        }
    }
}

/// Premultiplied → straight alpha, rounded.
#[inline]
pub fn unpremul(p: &[u8]) -> [u8; 4] {
    let a = p[3] as u32;
    match a {
        0 => [0, 0, 0, 0],
        255 => [p[0], p[1], p[2], 255],
        _ => {
            let f = |c: u8| ((c as u32 * 255 + a / 2) / a).min(255) as u8;
            [f(p[0]), f(p[1]), f(p[2]), a as u8]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn div255_is_exact_rounding() {
        for x in 0..=65535u32 {
            let want = (x as f64 / 255.0 + 0.5).floor() as u32;
            assert_eq!(div255(x), want, "{x}");
        }
    }

    #[test]
    fn over_is_identity_for_transparent_and_replace_for_opaque() {
        let mut d = [10, 20, 30, 40];
        over(&mut d, [0, 0, 0, 0]);
        assert_eq!(d, [10, 20, 30, 40]);
        over(&mut d, [1, 2, 3, 255]);
        assert_eq!(d, [1, 2, 3, 255]);
    }

    #[test]
    fn blend_modes_on_opaque_colours() {
        let mut d = [200, 100, 50, 255];
        blend(&mut d, [128, 255, 0, 255], Blend::Multiply);
        assert_eq!(d, [100, 100, 0, 255]);
        let mut d = [200, 100, 50, 255];
        blend(&mut d, [128, 0, 255, 255], Blend::Screen);
        assert_eq!(d, [228, 100, 255, 255]);
    }

    #[test]
    fn premul_round_trips_for_opaque() {
        let c = Color::rgb8(232, 17, 45);
        assert_eq!(premul(c, 1.0), [232, 17, 45, 255]);
        assert_eq!(unpremul(&premul(c, 1.0)), [232, 17, 45, 255]);
        assert_eq!(premul(c, 0.0), [0, 0, 0, 0]);
    }
}
