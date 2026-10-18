//! Pictures (raster logos, photos) as the display list carries them: decoded once, premultiplied,
//! and shared by every frame that draws them. The file's own bytes are kept for vector exports,
//! which embed them as they are.
//!
//! The engine stays format-agnostic: a picture is straight-alpha RGBA handed in
//! ([`Raster::from_rgba`]). Who decodes depends on the build — the platform hosts decode with
//! their own decoders (the browser's `createImageBitmap`, CGImageSource on Apple, BitmapFactory on
//! Android), so the web runtime carries no decoder and every format the platform reads works;
//! the tools, headless renders and the desktop host decode PNG (`png`), JPEG (`jpeg`) and WebP
//! (`webp`) in Rust
//! ([`Raster::decode`]). PNG is lossless, so every decoder gives the same pixels (P1); JPEG and
//! WebP are decoded by each platform's own decoder, and may differ by a level here and there.

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
    /// The file as shipped (PNG, JPEG, …): what SVG and PDF exports embed.
    pub encoded: Arc<[u8]>,
    /// The file's content hash: the picture's identity for caches (a GPU's texture).
    pub hash: u64,
}

/// A picture file's format, from its first bytes: `png`, `jpeg`, `webp`, `gif`, or `None`.
pub fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("jpeg")
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else if bytes.starts_with(b"GIF8") {
        Some("gif")
    } else {
        None
    }
}

/// The media type a format is embedded as (an SVG `data:` URL).
pub fn media_type(bytes: &[u8]) -> &'static str {
    match sniff(bytes) {
        Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        _ => "image/png",
    }
}

impl Raster {
    /// A picture from decoded pixels: `rgba` straight (not premultiplied) RGBA8, row-major, top
    /// row first — what a host's decoder hands in. `encoded` is the file (kept for exports, and
    /// its hash names the picture); empty, the pixels name it.
    pub fn from_rgba(width: u32, height: u32, rgba: &[u8], encoded: Arc<[u8]>) -> Result<Raster, String> {
        if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
            return Err(format!("the picture is {width} × {height} px; at most {MAX_SIDE} px a side is read"));
        }
        let n = width as usize * height as usize;
        if rgba.len() != n * 4 {
            return Err(format!("{} bytes of pixels for {width} × {height} (RGBA8 is {})", rgba.len(), n * 4));
        }
        let mut premul = Vec::with_capacity(n * 4);
        for px in rgba.as_chunks::<4>().0 {
            let a = px[3] as u32;
            let m = |c: u8| ((c as u32 * a + 127) / 255) as u8;
            premul.extend_from_slice(&[m(px[0]), m(px[1]), m(px[2]), px[3]]);
        }
        let mut hsh = Hash64::new();
        hsh.bytes(if encoded.is_empty() { rgba } else { &encoded });
        Ok(Raster { width, height, premul: premul.into(), encoded, hash: hsh.finish() })
    }

    /// Whether this build decodes the file itself ([`Raster::decode`]); if not, the host does and
    /// hands in its pixels ([`Raster::from_rgba`]).
    pub fn decodes(bytes: &[u8]) -> bool {
        let f = sniff(bytes);
        (cfg!(feature = "png") && f == Some("png")) || (cfg!(feature = "jpeg") && f == Some("jpeg")) || (cfg!(feature = "webp") && f == Some("webp"))
    }

    /// Why a file can't be read here: not a picture at all, or a format no decoder in this build
    /// reads.
    pub fn unreadable(bytes: &[u8]) -> String {
        match sniff(bytes) {
            Some(f) => format!("this build doesn't decode {} pictures (the tools read PNG, JPEG and WebP; apps and browsers decode the rest)", f.to_uppercase()),
            None => "the picture isn't a PNG, JPEG or WebP file".into(),
        }
    }

    /// Decode a file with the decoders compiled in: PNG (`png`: any bit depth and colour type;
    /// 16-bit channels rounded to 8, palettes expanded, gamma chunks ignored — the pixels as
    /// stored) and JPEG (`jpeg`, the portable scalar decoder: the same pixels on every target).
    pub fn decode(bytes: &[u8]) -> Result<Raster, String> {
        match sniff(bytes) {
            #[cfg(feature = "png")]
            Some("png") => {
                let (w, h, rgba) = decode_png(bytes)?;
                Raster::from_rgba(w, h, &rgba, Arc::from(bytes))
            }
            #[cfg(feature = "jpeg")]
            Some("jpeg") => {
                let (w, h, rgba) = decode_jpeg(bytes)?;
                Raster::from_rgba(w, h, &rgba, Arc::from(bytes))
            }
            #[cfg(feature = "webp")]
            Some("webp") => {
                let (w, h, rgba) = decode_webp(bytes)?;
                Raster::from_rgba(w, h, &rgba, Arc::from(bytes))
            }
            _ => Err(Raster::unreadable(bytes)),
        }
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

/// A PNG's pixels, straight RGBA8.
#[cfg(feature = "png")]
fn decode_png(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let mut dec = png::Decoder::new_with_limits(std::io::Cursor::new(bytes), png::Limits { bytes: 512 << 20 });
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = dec.read_info().map_err(|e| format!("PNG: {e}"))?;
    let (w, h) = (reader.info().width, reader.info().height);
    if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
        return Err(format!("the picture is {w} × {h} px; at most {MAX_SIDE} px a side is read"));
    }
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut buf).map_err(|e| format!("PNG: {e}"))?;
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    let line = frame.line_size;
    for y in 0..h as usize {
        let row = &buf[y * line..y * line + line];
        for x in 0..w as usize {
            let px = match frame.color_type {
                png::ColorType::Rgba => [row[x * 4], row[x * 4 + 1], row[x * 4 + 2], row[x * 4 + 3]],
                png::ColorType::Rgb => [row[x * 3], row[x * 3 + 1], row[x * 3 + 2], 255],
                png::ColorType::GrayscaleAlpha => [row[x * 2], row[x * 2], row[x * 2], row[x * 2 + 1]],
                png::ColorType::Grayscale => [row[x], row[x], row[x], 255],
                png::ColorType::Indexed => return Err("PNG: an indexed picture wasn't expanded".into()),
            };
            out.extend_from_slice(&px);
        }
    }
    Ok((w, h, out))
}

/// A JPEG's pixels, straight RGBA8 (opaque).
#[cfg(feature = "jpeg")]
fn decode_jpeg(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    use zune_jpeg::zune_core::{bytestream::ZCursor, colorspace::ColorSpace, options::DecoderOptions};
    let opts = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA).set_max_width(MAX_SIDE as usize).set_max_height(MAX_SIDE as usize);
    let mut dec = zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(bytes), opts);
    let px = dec.decode().map_err(|e| format!("JPEG: {e:?}"))?;
    let (w, h) = dec.dimensions().ok_or("JPEG: no size")?;
    Ok((w as u32, h as u32, px))
}

/// A WebP's pixels, straight RGBA8 (lossy or lossless; opaque without an alpha chunk).
#[cfg(feature = "webp")]
fn decode_webp(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let mut dec = image_webp::WebPDecoder::new(std::io::Cursor::new(bytes)).map_err(|e| format!("WebP: {e}"))?;
    let (w, h) = dec.dimensions();
    if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
        return Err(format!("the picture is {w} × {h} px; at most {MAX_SIDE} px a side is read"));
    }
    let n = dec.output_buffer_size().ok_or("WebP: too big")?;
    let mut buf = vec![0u8; n];
    dec.read_image(&mut buf).map_err(|e| format!("WebP: {e}"))?;
    if dec.has_alpha() {
        return Ok((w, h, buf));
    }
    Ok((w, h, buf.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect()))
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
    #[cfg(feature = "png")]
    fn a_png_decodes_premultiplied() {
        let r = Raster::decode(&png(2, 1, &[255, 0, 0, 255, 0, 0, 255, 128])).unwrap();
        assert_eq!((r.width, r.height), (2, 1));
        assert_eq!(r.texel(0, 0), [255, 0, 0, 255]);
        assert_eq!(r.texel(1, 0), [0, 0, 128, 128]);
        assert_eq!(r.texel(5, 9), [0, 0, 128, 128], "clamped to the edge");
    }

    #[test]
    #[cfg(feature = "png")]
    fn bilinear_between_texel_centres() {
        let r = Raster::decode(&png(2, 1, &[0, 0, 0, 255, 255, 255, 255, 255])).unwrap();
        assert_eq!(r.sample(0.5, 0.5), [0, 0, 0, 255]);
        assert_eq!(r.sample(1.5, 0.5), [255, 255, 255, 255]);
        assert_eq!(r.sample(1.0, 0.5)[0], 128);
    }

    #[test]
    fn other_formats_say_what_they_are() {
        assert!(Raster::decode(b"GIF89a....").unwrap_err().contains("GIF"));
        assert!(Raster::decode(b"hello").unwrap_err().contains("isn't a PNG, JPEG or WebP"));
        assert_eq!(sniff(&[0xff, 0xd8, 0xff, 0xe0]), Some("jpeg"));
        assert_eq!(media_type(&[0xff, 0xd8, 0xff, 0xe0]), "image/jpeg");
    }

    #[test]
    #[cfg(feature = "png")]
    fn host_pixels_are_premultiplied_and_checked() {
        let r = Raster::from_rgba(2, 1, &[255, 0, 0, 255, 0, 0, 255, 128], Arc::from(&b"file"[..])).unwrap();
        assert_eq!(r.texel(1, 0), [0, 0, 128, 128]);
        let png = Raster::decode(&png(2, 1, &[255, 0, 0, 255, 0, 0, 255, 128])).unwrap();
        assert_eq!(r.premul, png.premul, "a host's decode and ours give the same picture");
        assert!(Raster::from_rgba(2, 2, &[0; 8], Arc::from(&b""[..])).unwrap_err().contains("bytes of pixels"));
        assert!(Raster::from_rgba(0, 1, &[], Arc::from(&b""[..])).is_err());
    }

    #[test]
    #[cfg(feature = "webp")]
    fn a_webp_decodes_with_its_alpha() {
        // 8 × 8, lossless: red on the left half, a transparent blue on the right.
        let webp = include_bytes!("../tests/half8.webp");
        assert!(Raster::decodes(webp));
        let r = Raster::decode(webp).unwrap();
        assert_eq!((r.width, r.height), (8, 8));
        assert_eq!(r.texel(1, 1), [200, 40, 30, 255]);
        assert_eq!(r.texel(6, 1)[3], 0, "its alpha kept");
        assert_eq!(media_type(webp), "image/webp");
    }

    #[test]
    #[cfg(feature = "jpeg")]
    fn a_jpeg_decodes_opaque() {
        // An 8 × 8 mid-grey baseline JPEG (one block).
        let jpeg = include_bytes!("../tests/grey8.jpg");
        assert!(Raster::decodes(jpeg));
        let r = Raster::decode(jpeg).unwrap();
        assert_eq!((r.width, r.height), (8, 8));
        let t = r.texel(3, 3);
        assert_eq!(t[3], 255);
        assert!((t[0] as i32 - 128).abs() <= 2, "{t:?}");
    }
}
