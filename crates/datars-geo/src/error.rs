//! One error type for every parser and decoder in the crate. Decoders never panic on bad input:
//! malformed bytes come back as a `GeoError` saying what was wrong and where.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeoError {
    /// The text isn't valid JSON.
    Json(String),
    /// Well-formed input that isn't valid for its format (GeoJSON, TopoJSON, MVT, PMTiles).
    Format(String),
    /// A compressed payload couldn't be decompressed, or uses an unsupported codec.
    Compression(String),
}

impl GeoError {
    pub(crate) fn format(msg: impl Into<String>) -> GeoError {
        GeoError::Format(msg.into())
    }
}

impl fmt::Display for GeoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GeoError::Json(m) => write!(f, "invalid JSON: {m}"),
            GeoError::Format(m) => write!(f, "invalid data: {m}"),
            GeoError::Compression(m) => write!(f, "compression: {m}"),
        }
    }
}

impl std::error::Error for GeoError {}
