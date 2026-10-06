//! PMTiles v3: a whole tile pyramid in one static file, addressable by `(z, x, y)` with HTTP range
//! requests — no tile server. The first 16 KiB hold the header and root directory; after that each
//! tile costs one range read (plus, for big archives, an occasional leaf directory).
//!
//! Both halves are sans-IO. The [`Reader`] never fetches: it answers "which bytes do I need next"
//! ([`Step::Needs`]) and the host fetches them however it can (file, `fetch` with `Range`, app
//! bundle, mmap) and hands them back with [`Reader::provide`]. The [`Writer`] (for the build
//! pipeline) returns the archive as bytes; writing them somewhere is the caller's business.

mod directory;
mod hilbert;
mod reader;
mod writer;

pub use hilbert::{tile_id_to_zxy, zxy_to_tile_id};
pub use reader::{read_tile, Reader, Step};
pub use writer::{Writer, WriterOptions};

use crate::{GeoBbox, GeoError};
use datars_math::Vec2;
use std::io::{Read, Write};

/// Size of the fixed header.
pub const HEADER_LEN: usize = 127;

/// Bytes a reader needs up front: the spec guarantees header + root directory fit in 16 KiB.
pub const PREFIX_LEN: u64 = 16384;

const MAGIC: &[u8; 7] = b"PMTiles";

/// Compression of tiles or of the internal directories and metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compression {
    Unknown,
    None,
    Gzip,
    Brotli,
    Zstd,
}

impl Compression {
    fn from_u8(v: u8) -> Compression {
        match v {
            1 => Compression::None,
            2 => Compression::Gzip,
            3 => Compression::Brotli,
            4 => Compression::Zstd,
            _ => Compression::Unknown,
        }
    }

    fn to_u8(self) -> u8 {
        match self {
            Compression::Unknown => 0,
            Compression::None => 1,
            Compression::Gzip => 2,
            Compression::Brotli => 3,
            Compression::Zstd => 4,
        }
    }
}

/// What the tile payloads are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileType {
    Unknown,
    Mvt,
    Png,
    Jpeg,
    Webp,
    Avif,
}

impl TileType {
    fn from_u8(v: u8) -> TileType {
        match v {
            1 => TileType::Mvt,
            2 => TileType::Png,
            3 => TileType::Jpeg,
            4 => TileType::Webp,
            5 => TileType::Avif,
            _ => TileType::Unknown,
        }
    }

    fn to_u8(self) -> u8 {
        match self {
            TileType::Unknown => 0,
            TileType::Mvt => 1,
            TileType::Png => 2,
            TileType::Jpeg => 3,
            TileType::Webp => 4,
            TileType::Avif => 5,
        }
    }
}

/// The 127-byte archive header. Offsets are absolute byte positions in the archive; lengths are
/// in bytes. `bounds` and `center` are lon/lat degrees (stored as integers ·1e7).
#[derive(Clone, Debug, PartialEq)]
pub struct Header {
    pub root_offset: u64,
    pub root_length: u64,
    pub metadata_offset: u64,
    pub metadata_length: u64,
    pub leaf_offset: u64,
    pub leaf_length: u64,
    pub data_offset: u64,
    pub data_length: u64,
    /// Tiles addressable in the archive (runs count every tile they cover).
    pub addressed_tiles: u64,
    /// Data entries across all directories.
    pub tile_entries: u64,
    /// Distinct tile payloads stored (after deduplication).
    pub tile_contents: u64,
    /// Tile data is laid out in tile-id order.
    pub clustered: bool,
    pub internal_compression: Compression,
    pub tile_compression: Compression,
    pub tile_type: TileType,
    pub min_zoom: u8,
    pub max_zoom: u8,
    pub bounds: GeoBbox,
    pub center_zoom: u8,
    pub center: Vec2,
}

fn u64_at(b: &[u8], o: usize) -> u64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[o..o + 8]);
    u64::from_le_bytes(a)
}

fn e7_at(b: &[u8], o: usize) -> f64 {
    let mut a = [0u8; 4];
    a.copy_from_slice(&b[o..o + 4]);
    i32::from_le_bytes(a) as f64 / 1e7
}

fn e7(v: f64) -> [u8; 4] {
    let i = (v * 1e7).round().clamp(i32::MIN as f64, i32::MAX as f64) as i32;
    i.to_le_bytes()
}

impl Header {
    /// Parse the header from the archive's first bytes (at least [`HEADER_LEN`]).
    pub fn parse(b: &[u8]) -> Result<Header, GeoError> {
        if b.len() < HEADER_LEN {
            return Err(GeoError::format("PMTiles: too short for a header"));
        }
        if &b[0..7] != MAGIC {
            return Err(GeoError::format("PMTiles: bad magic (not a PMTiles archive)"));
        }
        if b[7] != 3 {
            return Err(GeoError::format(format!("PMTiles: unsupported version {}", b[7])));
        }
        Ok(Header {
            root_offset: u64_at(b, 8),
            root_length: u64_at(b, 16),
            metadata_offset: u64_at(b, 24),
            metadata_length: u64_at(b, 32),
            leaf_offset: u64_at(b, 40),
            leaf_length: u64_at(b, 48),
            data_offset: u64_at(b, 56),
            data_length: u64_at(b, 64),
            addressed_tiles: u64_at(b, 72),
            tile_entries: u64_at(b, 80),
            tile_contents: u64_at(b, 88),
            clustered: b[96] == 1,
            internal_compression: Compression::from_u8(b[97]),
            tile_compression: Compression::from_u8(b[98]),
            tile_type: TileType::from_u8(b[99]),
            min_zoom: b[100],
            max_zoom: b[101],
            bounds: GeoBbox::new(e7_at(b, 102), e7_at(b, 106), e7_at(b, 110), e7_at(b, 114)),
            center_zoom: b[118],
            center: Vec2::new(e7_at(b, 119), e7_at(b, 123)),
        })
    }

    /// Serialize to the 127-byte on-disk form.
    pub fn to_bytes(&self) -> [u8; HEADER_LEN] {
        let mut h = [0u8; HEADER_LEN];
        h[0..7].copy_from_slice(MAGIC);
        h[7] = 3;
        let fields = [
            self.root_offset,
            self.root_length,
            self.metadata_offset,
            self.metadata_length,
            self.leaf_offset,
            self.leaf_length,
            self.data_offset,
            self.data_length,
            self.addressed_tiles,
            self.tile_entries,
            self.tile_contents,
        ];
        for (i, v) in fields.iter().enumerate() {
            h[8 + 8 * i..16 + 8 * i].copy_from_slice(&v.to_le_bytes());
        }
        h[96] = self.clustered as u8;
        h[97] = self.internal_compression.to_u8();
        h[98] = self.tile_compression.to_u8();
        h[99] = self.tile_type.to_u8();
        h[100] = self.min_zoom;
        h[101] = self.max_zoom;
        h[102..106].copy_from_slice(&e7(self.bounds.west));
        h[106..110].copy_from_slice(&e7(self.bounds.south));
        h[110..114].copy_from_slice(&e7(self.bounds.east));
        h[114..118].copy_from_slice(&e7(self.bounds.north));
        h[118] = self.center_zoom;
        h[119..123].copy_from_slice(&e7(self.center.x));
        h[123..127].copy_from_slice(&e7(self.center.y));
        h
    }
}

/// A byte range in the archive: what a host turns into `Range: bytes=offset-(offset+length-1)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ByteRange {
    pub offset: u64,
    pub length: u64,
}

impl ByteRange {
    pub const fn new(offset: u64, length: u64) -> ByteRange {
        ByteRange { offset, length }
    }

    /// One past the last byte (saturating).
    pub fn end(&self) -> u64 {
        self.offset.saturating_add(self.length)
    }
}

/// Decompress a payload stored with `c`. Only gzip is supported (it's what our archives use and
/// what every PMTiles producer can write); brotli and zstd are reported as errors.
pub(crate) fn decompress(data: &[u8], c: Compression) -> Result<Vec<u8>, GeoError> {
    match c {
        Compression::None | Compression::Unknown => Ok(data.to_vec()),
        Compression::Gzip => {
            let mut out = Vec::new();
            flate2::read::GzDecoder::new(data).read_to_end(&mut out).map_err(|e| GeoError::Compression(format!("gunzip: {e}")))?;
            Ok(out)
        }
        Compression::Brotli => Err(GeoError::Compression("brotli is not supported".into())),
        Compression::Zstd => Err(GeoError::Compression("zstd is not supported".into())),
    }
}

/// Compress with `c`. The gzip header carries no timestamp or file name, so output depends only on
/// the input bytes.
pub(crate) fn compress(data: &[u8], c: Compression) -> Result<Vec<u8>, GeoError> {
    match c {
        Compression::None => Ok(data.to_vec()),
        Compression::Gzip => {
            let mut enc = flate2::GzBuilder::new().mtime(0).write(Vec::new(), flate2::Compression::default());
            enc.write_all(data).map_err(|e| GeoError::Compression(format!("gzip: {e}")))?;
            enc.finish().map_err(|e| GeoError::Compression(format!("gzip: {e}")))
        }
        other => Err(GeoError::Compression(format!("cannot write {other:?} compression"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_header() -> Header {
        Header {
            root_offset: 127,
            root_length: 50,
            metadata_offset: 177,
            metadata_length: 20,
            leaf_offset: 197,
            leaf_length: 0,
            data_offset: 197,
            data_length: 1000,
            addressed_tiles: 12,
            tile_entries: 10,
            tile_contents: 9,
            clustered: true,
            internal_compression: Compression::Gzip,
            tile_compression: Compression::None,
            tile_type: TileType::Mvt,
            min_zoom: 0,
            max_zoom: 14,
            bounds: GeoBbox::new(10.5, 55.25, 24.125, 69.0625),
            center_zoom: 5,
            center: Vec2::new(18.0686, 59.3293),
        }
    }

    #[test]
    fn header_round_trips() {
        let h = sample_header();
        let b = h.to_bytes();
        assert_eq!(Header::parse(&b), Ok(h));
    }

    #[test]
    fn corrupt_headers_are_errors() {
        let b = sample_header().to_bytes();
        assert!(Header::parse(&b[..100]).is_err());
        let mut bad = b;
        bad[0] = b'X';
        assert!(Header::parse(&bad).is_err());
        let mut v2 = b;
        v2[7] = 2;
        assert!(Header::parse(&v2).is_err());
    }

    #[test]
    fn gzip_is_deterministic_and_round_trips() {
        let data = b"the same bytes, every time".repeat(10);
        let a = compress(&data, Compression::Gzip).unwrap();
        assert_eq!(a, compress(&data, Compression::Gzip).unwrap());
        assert_eq!(decompress(&a, Compression::Gzip).unwrap(), data);
        assert!(decompress(b"not gzip", Compression::Gzip).is_err());
        assert!(decompress(b"x", Compression::Brotli).is_err());
        assert!(compress(b"x", Compression::Zstd).is_err());
    }
}
