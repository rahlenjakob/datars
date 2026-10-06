//! `datars-text` — the engine's text stage (docs/11-rendering.md, "Text").
//!
//! The engine shapes and lays out every piece of text itself, so line breaks, widths and glyph
//! positions are identical on web, iOS, Android, desktop, video and static exports (P1, P8).
//! Backends never measure text; they draw the glyph runs this crate produces, fetching outlines
//! through [`datars_render::GlyphSource`], which [`FontDb`] implements.
//!
//! - [`FontDb`]: fonts the host adds as bytes (sans-IO, P10) — bundle font chunks, document font
//!   sources — plus, with the `bundled-fonts` feature, the default family (Inter 400/600/700).
//! - [`layout`] / [`measure`]: shaping (harfrust, kerning and ligatures on), font fallback along a
//!   family stack, UAX #14 line breaking, UAX #9 bidi reordering, alignment and baselines.
//!   [`lines`] describes a layout line by line (characters, box, baseline) for hosts that lay
//!   selectable text over the drawn glyphs.
//! - [`format`] and [`locale`]: d3-format-like number formatting and strftime-like dates.
//!
//! Determinism: shaping works in integer font units and converts to px with plain f64
//! arithmetic; caches are pure memoization (a hit returns exactly what a miss would compute).

mod fontdb;
pub mod format;
mod layout;
pub mod locale;
mod shape;

pub use fontdb::{bundled_file, bundled_name, FaceInfo, FaceMetrics, FontDb, EMBED_NO_SUBSETTING, EMBED_RESTRICTED};
pub use layout::{layout, lines, measure, LineBox};

use std::fmt;

/// Errors from loading fonts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextError {
    /// The bytes are not a readable OpenType/TrueType font or collection.
    InvalidFont(String),
    /// The file parsed but contained no usable faces (e.g. no outlines or no character map).
    NoFaces,
}

impl fmt::Display for TextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TextError::InvalidFont(e) => write!(f, "invalid font: {e}"),
            TextError::NoFaces => write!(f, "font file contains no usable faces"),
        }
    }
}

impl std::error::Error for TextError {}
