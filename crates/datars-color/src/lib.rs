//! Colour. Stored as straight-alpha sRGB in f32 (what authors write and what backends blend);
//! interpolated in OKLab by default (perceptually even, no muddy midpoints).

use datars_math::m;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub mod palette;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Default for Color {
    fn default() -> Self {
        Color::BLACK
    }
}

impl Color {
    pub const BLACK: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const WHITE: Color = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    pub const TRANSPARENT: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Color {
        Color { r, g, b, a }
    }
    pub fn rgb8(r: u8, g: u8, b: u8) -> Color {
        Color { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a: 1.0 }
    }
    pub fn with_alpha(self, a: f32) -> Color {
        Color { a, ..self }
    }
    pub fn fade(self, k: f32) -> Color {
        Color { a: self.a * k, ..self }
    }

    /// `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, or a CSS-style name from a small set.
    pub fn parse(s: &str) -> Option<Color> {
        let s = s.trim();
        if let Some(h) = s.strip_prefix('#') {
            let hex = |i: usize, n: usize| u8::from_str_radix(&h[i..i + n], 16).ok();
            return match h.len() {
                3 | 4 => {
                    let d = |i: usize| hex(i, 1).map(|v| v * 17);
                    Some(Color::rgb8(d(0)?, d(1)?, d(2)?).with_alpha(if h.len() == 4 { d(3)? as f32 / 255.0 } else { 1.0 }))
                }
                6 | 8 => Some(Color::rgb8(hex(0, 2)?, hex(2, 2)?, hex(4, 2)?).with_alpha(if h.len() == 8 { hex(6, 2)? as f32 / 255.0 } else { 1.0 })),
                _ => None,
            };
        }
        match s {
            "black" => Some(Color::BLACK),
            "white" => Some(Color::WHITE),
            "transparent" | "none" => Some(Color::TRANSPARENT),
            "red" => Some(Color::rgb8(255, 0, 0)),
            "green" => Some(Color::rgb8(0, 128, 0)),
            "blue" => Some(Color::rgb8(0, 0, 255)),
            "gray" | "grey" => Some(Color::rgb8(128, 128, 128)),
            _ => None,
        }
    }

    pub fn to_hex(self) -> String {
        let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        if self.a >= 1.0 {
            format!("#{:02x}{:02x}{:02x}", q(self.r), q(self.g), q(self.b))
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", q(self.r), q(self.g), q(self.b), q(self.a))
        }
    }

    /// 8-bit RGBA, straight alpha.
    pub fn to_rgba8(self) -> [u8; 4] {
        let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        [q(self.r), q(self.g), q(self.b), q(self.a)]
    }

    pub fn to_oklab(self) -> Oklab {
        let (r, g, b) = (to_linear(self.r as f64), to_linear(self.g as f64), to_linear(self.b as f64));
        let l = 0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b;
        let mm = 0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b;
        let s = 0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b;
        let (l_, m_, s_) = (m::cbrt(l), m::cbrt(mm), m::cbrt(s));
        Oklab {
            l: 0.210_454_255_3 * l_ + 0.793_617_785_0 * m_ - 0.004_072_046_8 * s_,
            a: 1.977_998_495_1 * l_ - 2.428_592_205_0 * m_ + 0.450_593_709_9 * s_,
            b: 0.025_904_037_1 * l_ + 0.782_771_766_2 * m_ - 0.808_675_766_0 * s_,
            alpha: self.a as f64,
        }
    }

    /// Relative luminance (WCAG).
    pub fn luminance(self) -> f64 {
        0.2126 * to_linear(self.r as f64) + 0.7152 * to_linear(self.g as f64) + 0.0722 * to_linear(self.b as f64)
    }

    /// Interpolate in OKLab with premultiplied alpha ([`Oklab::mix`]).
    pub fn lerp_oklab(self, o: Color, t: f64) -> Color {
        if t <= 0.0 {
            return self;
        }
        if t >= 1.0 {
            return o;
        }
        self.to_oklab().mix(o.to_oklab(), t).to_color()
    }

    /// Interpolate in straight sRGB.
    pub fn lerp_srgb(self, o: Color, t: f64) -> Color {
        let t = t as f32;
        Color {
            r: self.r + (o.r - self.r) * t,
            g: self.g + (o.g - self.g) * t,
            b: self.b + (o.b - self.b) * t,
            a: self.a + (o.a - self.a) * t,
        }
    }

    /// Composite over an opaque background (for contrast checks).
    pub fn over(self, bg: Color) -> Color {
        let a = self.a;
        Color { r: self.r * a + bg.r * (1.0 - a), g: self.g * a + bg.g * (1.0 - a), b: self.b * a + bg.b * (1.0 - a), a: 1.0 }
    }
}

/// WCAG 2 contrast ratio between two colours (1…21).
pub fn contrast_ratio(a: Color, b: Color) -> f64 {
    let (la, lb) = (a.luminance(), b.luminance());
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

pub fn to_linear(c: f64) -> f64 {
    if c <= 0.040_45 { c / 12.92 } else { m::pow((c + 0.055) / 1.055, 2.4) }
}

pub fn to_srgb(c: f64) -> f64 {
    if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * m::pow(c, 1.0 / 2.4) - 0.055 }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oklab {
    pub l: f64,
    pub a: f64,
    pub b: f64,
    pub alpha: f64,
}

impl Oklab {
    /// Interpolate with premultiplied alpha, as CSS Color 4 mixes: each end's colour counts by its
    /// alpha, so a translucent end doesn't lend its full strength mid-way (an opaque grey turning
    /// into a faint blue never looks stronger than either end; a fade from transparent takes the
    /// other end's colour throughout). Ends of equal alpha — every opaque pair — mix exactly as
    /// straight OKLab; `t` 0 and 1 give the ends exactly.
    pub fn mix(self, o: Oklab, t: f64) -> Oklab {
        if t == 0.0 {
            return self;
        }
        if t == 1.0 {
            return o;
        }
        let alpha = self.alpha + (o.alpha - self.alpha) * t;
        if self.alpha == o.alpha || alpha <= 0.0 {
            let s = |x: f64, y: f64| x + (y - x) * t;
            return Oklab { l: s(self.l, o.l), a: s(self.a, o.a), b: s(self.b, o.b), alpha };
        }
        let (wa, wb) = (self.alpha, o.alpha);
        let p = |x: f64, y: f64| (x * wa + (y * wb - x * wa) * t) / alpha;
        Oklab { l: p(self.l, o.l), a: p(self.a, o.a), b: p(self.b, o.b), alpha }
    }

    pub fn to_color(self) -> Color {
        let l_ = self.l + 0.396_337_777_4 * self.a + 0.215_803_757_3 * self.b;
        let m_ = self.l - 0.105_561_345_8 * self.a - 0.063_854_172_8 * self.b;
        let s_ = self.l - 0.089_484_177_5 * self.a - 1.291_485_548_0 * self.b;
        let (l, mm, s) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
        let r = 4.076_741_662_1 * l - 3.307_711_591_3 * mm + 0.230_969_929_2 * s;
        let g = -1.268_438_004_6 * l + 2.609_757_401_1 * mm - 0.341_319_396_5 * s;
        let b = -0.004_196_086_3 * l - 0.703_418_614_7 * mm + 1.707_614_701_0 * s;
        Color {
            r: to_srgb(r).clamp(0.0, 1.0) as f32,
            g: to_srgb(g).clamp(0.0, 1.0) as f32,
            b: to_srgb(b).clamp(0.0, 1.0) as f32,
            a: self.alpha.clamp(0.0, 1.0) as f32,
        }
    }
    /// (lightness, chroma, hue radians).
    pub fn to_lch(self) -> (f64, f64, f64) {
        (self.l, m::hypot(self.a, self.b), m::atan2(self.b, self.a))
    }
    pub fn from_lch(l: f64, c: f64, h: f64, alpha: f64) -> Oklab {
        Oklab { l, a: c * m::cos(h), b: c * m::sin(h), alpha }
    }
}

/// Colour-vision deficiency simulation (Machado et al. 2009, severity 1.0), in linear RGB.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Cvd {
    Protanopia,
    Deuteranopia,
    Tritanopia,
}

pub fn simulate_cvd(c: Color, kind: Cvd) -> Color {
    let mtx: [[f64; 3]; 3] = match kind {
        Cvd::Protanopia => [[0.152286, 1.052583, -0.204868], [0.114503, 0.786281, 0.099216], [-0.003882, -0.048116, 1.051998]],
        Cvd::Deuteranopia => [[0.367322, 0.860646, -0.227968], [0.280085, 0.672501, 0.047413], [-0.011820, 0.042940, 0.968881]],
        Cvd::Tritanopia => [[1.255528, -0.076749, -0.178779], [-0.078411, 0.930809, 0.147602], [0.004733, 0.691367, 0.303900]],
    };
    let v = [to_linear(c.r as f64), to_linear(c.g as f64), to_linear(c.b as f64)];
    let o = |row: [f64; 3]| to_srgb((row[0] * v[0] + row[1] * v[1] + row[2] * v[2]).clamp(0.0, 1.0)) as f32;
    Color { r: o(mtx[0]), g: o(mtx[1]), b: o(mtx[2]), a: c.a }
}

/// ΔE in OKLab (Euclidean), ×100 for a JND scale of roughly 1–2.
pub fn delta_e(a: Color, b: Color) -> f64 {
    let (x, y) = (a.to_oklab(), b.to_oklab());
    ((x.l - y.l).powi(2) + (x.a - y.a).powi(2) + (x.b - y.b).powi(2)).sqrt() * 100.0
}

impl Serialize for Color {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Color, D::Error> {
        let s = String::deserialize(d)?;
        Color::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("not a colour: {s}")))
    }
}

impl datars_math::StableHash for Color {
    fn stable_hash(&self, h: &mut datars_math::Hash64) {
        h.f32(self.r);
        h.f32(self.g);
        h.f32(self.b);
        h.f32(self.a);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hex_round_trip() {
        let c = Color::parse("#e8112d").unwrap();
        assert_eq!(c.to_hex(), "#e8112d");
        assert_eq!(Color::parse("#fff").unwrap(), Color::WHITE);
        assert_eq!(Color::parse("#00000080").unwrap().to_hex(), "#00000080");
    }
    #[test]
    fn oklab_round_trip_is_close() {
        for hex in ["#e8112d", "#1f77b4", "#ffffff", "#000000", "#7f7f7f"] {
            let c = Color::parse(hex).unwrap();
            let back = c.to_oklab().to_color();
            assert!(delta_e(c, back) < 0.01, "{hex} → {}", back.to_hex());
        }
    }
    #[test]
    fn contrast_black_white() {
        assert!((contrast_ratio(Color::BLACK, Color::WHITE) - 21.0).abs() < 1e-9);
    }
    #[test]
    fn oklab_midpoint_is_not_muddy() {
        let mid = Color::parse("#0000ff").unwrap().lerp_oklab(Color::parse("#ffff00").unwrap(), 0.5);
        assert!(mid.to_oklab().l > 0.5, "OKLab midpoint keeps lightness: {}", mid.to_hex());
    }
    #[test]
    fn mixing_is_premultiplied_by_alpha() {
        // From a transparent black, the other end's colour throughout: only the alpha rises.
        let red = Color::parse("#e8112d").unwrap();
        let clear = Color { a: 0.0, ..Color::BLACK };
        let mid = clear.lerp_oklab(red, 0.5);
        assert!((mid.a - 0.5).abs() < 1e-6 && (mid.r - red.r).abs() < 1e-4 && (mid.g - red.g).abs() < 1e-4 && (mid.b - red.b).abs() < 1e-4, "{mid:?}");
        // Exact ends.
        let (la, lb) = (clear.to_oklab(), red.to_oklab());
        assert_eq!(la.mix(lb, 0.0), la);
        assert_eq!(la.mix(lb, 1.0), lb);
    }
}
