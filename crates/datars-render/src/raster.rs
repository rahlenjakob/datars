//! Pictures (raster logos, photos) as the display list carries them: decoded once from the bytes a
//! bundle ships, premultiplied, and shared by every frame that draws them. PNG only: lossless, so
//! every platform decodes the same pixels (P1). The file's own bytes are kept for vector exports,
//! which embed them as they are.

use datars_math::Hash64;
use std::sync::Arc;

/// The largest picture read, per side (a 4K photo is 3840 wide).
pub const MAX_SIDE: u32 = 8192;

#[derive(Clone, Debug, PartialEq)]
pub struct Raster {
    pub width: u32,
    pub height: u32,
    /// Premultiplied RGBA8, row-major, top row first.
    pub premul: Arc<[u8]>,
    /// The file as shipped (PNG): what SVG and PDF exports embed.
    pub encoded: Arc<[u8]>,
    /// The file's content hash: the picture's identity for caches (a GPU's texture).
    pub hash: u64,
}

impl Raster {
    /// Decode a PNG (any bit depth and colour type; 16-bit channels are rounded to 8, palettes
    /// expanded, gamma chunks ignored — the pixels as stored).
    pub fn decode(bytes: &[u8]) -> Result<Raster, String> {
        if !bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
            let what = if bytes.starts_with(&[0xff, 0xd8]) {
                "a JPEG — pictures are read as PNG (lossless, the same pixels everywhere); convert it"
            } else if bytes.starts_with(b"RIFF") {
                "a WebP — pictures are read as PNG; convert it"
            } else if bytes.starts_with(b"GIF8") {
                "a GIF — pictures are read as PNG; convert it"
            } else {
                "not a PNG"
            };
            return Err(format!("the picture is {what}"));
        }
        let mut dec = png::Decoder::new_with_limits(std::io::Cursor::new(bytes), png::Limits { bytes: 512 << 20 });
        dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = dec.read_info().map_err(|e| format!("PNG: {e}"))?;
        let (w, h) = (reader.info().width, reader.info().height);
        if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
            return Err(format!("the picture is {w} × {h} px; at most {MAX_SIDE} px a side is read"));
        }
        let mut buf = vec![0u8; reader.output_buffer_size()];
        let frame = reader.next_frame(&mut buf).map_err(|e| format!("PNG: {e}"))?;
        let n = (w * h) as usize;
        let mut premul = Vec::with_capacity(n * 4);
        let px = |r: u8, g: u8, b: u8, a: u8, out: &mut Vec<u8>| {
            let m = |c: u8| ((c as u32 * a as u32 + 127) / 255) as u8;
            out.extend_from_slice(&[m(r), m(g), m(b), a]);
        };
        let line = frame.line_size;
        for y in 0..h as usize {
            let row = &buf[y * line..y * line + line];
            for x in 0..w as usize {
                match frame.color_type {
                    png::ColorType::Rgba => px(row[x * 4], row[x * 4 + 1], row[x * 4 + 2], row[x * 4 + 3], &mut premul),
                    png::ColorType::Rgb => px(row[x * 3], row[x * 3 + 1], row[x * 3 + 2], 255, &mut premul),
                    png::ColorType::GrayscaleAlpha => px(row[x * 2], row[x * 2], row[x * 2], row[x * 2 + 1], &mut premul),
                    png::ColorType::Grayscale => px(row[x], row[x], row[x], 255, &mut premul),
                    png::ColorType::Indexed => return Err("PNG: an indexed picture wasn't expanded".into()),
                }
            }
        }
        let mut hsh = Hash64::new();
        hsh.bytes(bytes);
        Ok(Raster { width: w, height: h, premul: premul.into(), encoded: Arc::from(bytes), hash: hsh.finish() })
    }

    /// The premultiplied texel at (x, y), clamped to the edges.
    #[inline]
    pub fn texel(&self, x: i64, y: i64) -> [u8; 4] {
        let x = x.clamp(0, self.width as i64 - 1) as usize;
        let y = y.clamp(0, self.height as i64 - 1) as usize;
        let i = (y * self.width as usize + x) * 4;
        [self.premul[i], self.premul[i + 1], self.premul[i + 2], self.premul[i + 3]]
    }

    /// Bilinear sample at (u, v) in texel units (texel centres at +0.5), premultiplied. Integer
    /// weights (1/256 steps), so every platform blends the same.
    pub fn sample(&self, u: f64, v: f64) -> [u8; 4] {
        let (u, v) = (u - 0.5, v - 0.5);
        let (x0, y0) = (u.floor(), v.floor());
        let fx = ((u - x0) * 256.0).round().clamp(0.0, 256.0) as u32;
        let fy = ((v - y0) * 256.0).round().clamp(0.0, 256.0) as u32;
        let (x0, y0) = (x0 as i64, y0 as i64);
        let (a, b, c, d) = (self.texel(x0, y0), self.texel(x0 + 1, y0), self.texel(x0, y0 + 1), self.texel(x0 + 1, y0 + 1));
        let mut out = [0u8; 4];
        for k in 0..4 {
            let top = a[k] as u32 * (256 - fx) + b[k] as u32 * fx;
            let bot = c[k] as u32 * (256 - fx) + d[k] as u32 * fx;
            out[k] = ((top * (256 - fy) + bot * fy + (1 << 15)) >> 16) as u8;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut e = png::Encoder::new(&mut out, w, h);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header().unwrap().write_image_data(rgba).unwrap();
        out
    }

    #[test]
    fn a_png_decodes_premultiplied() {
        let r = Raster::decode(&png(2, 1, &[255, 0, 0, 255, 0, 0, 255, 128])).unwrap();
        assert_eq!((r.width, r.height), (2, 1));
        assert_eq!(r.texel(0, 0), [255, 0, 0, 255]);
        assert_eq!(r.texel(1, 0), [0, 0, 128, 128]);
        assert_eq!(r.texel(5, 9), [0, 0, 128, 128], "clamped to the edge");
    }

    #[test]
    fn bilinear_between_texel_centres() {
        let r = Raster::decode(&png(2, 1, &[0, 0, 0, 255, 255, 255, 255, 255])).unwrap();
        assert_eq!(r.sample(0.5, 0.5), [0, 0, 0, 255]);
        assert_eq!(r.sample(1.5, 0.5), [255, 255, 255, 255]);
        assert_eq!(r.sample(1.0, 0.5)[0], 128);
    }

    #[test]
    fn other_formats_say_what_they_are() {
        assert!(Raster::decode(&[0xff, 0xd8, 0xff, 0xe0]).unwrap_err().contains("JPEG"));
        assert!(Raster::decode(b"hello").unwrap_err().contains("not a PNG"));
    }
}
