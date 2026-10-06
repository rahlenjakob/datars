//! The archive writer for the build pipeline: collect tiles, then `finish` into archive bytes.
//!
//! Identical tiles are stored once (the thousands of all-ocean tiles of a world archive cost one
//! copy), consecutive ids with identical content collapse into one run-length entry, and the
//! directory splits into leaves when the root wouldn't fit the first 16 KiB.

use super::directory::{self, Entry};
use super::{compress, Compression, Header, TileType, HEADER_LEN, PREFIX_LEN};
use crate::tile::MAX_LAT;
use crate::{GeoBbox, GeoError, TileId};
use datars_math::{Hash64, Vec2};
use std::collections::BTreeMap;

/// Archive-level settings.
#[derive(Clone, Debug)]
pub struct WriterOptions {
    pub tile_type: TileType,
    /// How tile payloads are stored: `None` or `Gzip`.
    pub tile_compression: Compression,
    /// Geographic bounds of the tiled region.
    pub bounds: GeoBbox,
    /// Suggested initial view centre (lon, lat).
    pub center: Vec2,
    /// Suggested initial zoom; defaults to the archive's minimum zoom.
    pub center_zoom: Option<u8>,
    /// The JSON metadata blob (vector layers, attribution, …).
    pub metadata: serde_json::Value,
    /// Budget for the compressed root directory, so header + root fit the first 16 KiB.
    pub max_root_bytes: usize,
    /// Force leaf directories of this many entries (mostly for tests; `None` = only when needed).
    pub leaf_entries: Option<usize>,
}

impl Default for WriterOptions {
    fn default() -> WriterOptions {
        WriterOptions {
            tile_type: TileType::Mvt,
            tile_compression: Compression::Gzip,
            bounds: GeoBbox::new(-180.0, -MAX_LAT, 180.0, MAX_LAT),
            center: Vec2::ZERO,
            center_zoom: None,
            metadata: serde_json::Value::Object(serde_json::Map::new()),
            max_root_bytes: PREFIX_LEN as usize - HEADER_LEN,
            leaf_entries: None,
        }
    }
}

/// Collects tiles and bakes a PMTiles v3 archive.
pub struct Writer {
    opts: WriterOptions,
    /// Raw tile bytes by PMTiles id (which is also the on-disk order).
    tiles: BTreeMap<u64, Vec<u8>>,
    /// The first invalid tile added, reported by `finish`.
    invalid: Option<TileId>,
}

impl Writer {
    pub fn new(opts: WriterOptions) -> Writer {
        Writer { opts, tiles: BTreeMap::new(), invalid: None }
    }

    /// Add a tile's raw (uncompressed) bytes; adding the same tile again replaces it. An invalid
    /// tile id makes [`Writer::finish`] fail.
    pub fn add_tile(&mut self, tile: TileId, bytes: Vec<u8>) {
        if !tile.is_valid() {
            self.invalid.get_or_insert(tile);
            return;
        }
        self.tiles.insert(tile.pmtiles_id(), bytes);
    }

    /// Tiles added so far.
    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    /// The finished archive: header | root directory | metadata | leaf directories | tile data.
    pub fn finish(self) -> Result<Vec<u8>, GeoError> {
        if let Some(t) = self.invalid {
            return Err(GeoError::format(format!("PMTiles: invalid tile {t:?}")));
        }
        if !matches!(self.opts.tile_compression, Compression::None | Compression::Gzip) {
            return Err(GeoError::Compression(format!("cannot write {:?} tiles", self.opts.tile_compression)));
        }
        let internal = Compression::Gzip;

        // Tile data, deduplicated: hash → indices into `contents` (a collision falls back to
        // comparing bytes), each content compressed and stored once.
        let mut data: Vec<u8> = Vec::new();
        let mut contents: Vec<(&[u8], u64, u32)> = Vec::new(); // raw bytes, offset, length
        let mut by_hash: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
        let mut entries: Vec<Entry> = Vec::with_capacity(self.tiles.len());
        for (&id, raw) in &self.tiles {
            let mut h = Hash64::new();
            h.bytes(raw);
            let candidates = by_hash.entry(h.finish()).or_default();
            let found = candidates.iter().copied().find(|&i| contents[i].0 == raw.as_slice());
            let (offset, length) = match found {
                Some(i) => (contents[i].1, contents[i].2),
                None => {
                    let stored = compress(raw, self.opts.tile_compression)?;
                    let length = u32::try_from(stored.len()).map_err(|_| GeoError::format("PMTiles: tile larger than 4 GiB"))?;
                    let offset = data.len() as u64;
                    data.extend_from_slice(&stored);
                    candidates.push(contents.len());
                    contents.push((raw, offset, length));
                    (offset, length)
                }
            };
            match entries.last_mut() {
                // Same bytes as the previous id: extend its run.
                Some(last) if last.offset == offset && last.length == length && last.tile_id + last.run_length as u64 == id && last.run_length < u32::MAX => {
                    last.run_length += 1
                }
                _ => entries.push(Entry { tile_id: id, offset, length, run_length: 1 }),
            }
        }

        let (root, leaves) = build_directories(&entries, internal, self.opts.max_root_bytes, self.opts.leaf_entries)?;
        let metadata = compress(&serde_json::to_vec(&self.opts.metadata).map_err(|e| GeoError::Json(e.to_string()))?, internal)?;

        let zoom = |id: &u64| TileId::from_pmtiles_id(*id).map_or(0, |t| t.z);
        let min_zoom = self.tiles.keys().next().map_or(0, zoom);
        let max_zoom = self.tiles.keys().next_back().map_or(0, zoom);

        let root_offset = HEADER_LEN as u64;
        let metadata_offset = root_offset + root.len() as u64;
        let leaf_offset = metadata_offset + metadata.len() as u64;
        let data_offset = leaf_offset + leaves.len() as u64;
        let header = Header {
            root_offset,
            root_length: root.len() as u64,
            metadata_offset,
            metadata_length: metadata.len() as u64,
            leaf_offset,
            leaf_length: leaves.len() as u64,
            data_offset,
            data_length: data.len() as u64,
            addressed_tiles: self.tiles.len() as u64,
            tile_entries: entries.len() as u64,
            tile_contents: contents.len() as u64,
            clustered: true,
            internal_compression: internal,
            tile_compression: self.opts.tile_compression,
            tile_type: self.opts.tile_type,
            min_zoom,
            max_zoom,
            bounds: self.opts.bounds,
            center_zoom: self.opts.center_zoom.unwrap_or(min_zoom),
            center: self.opts.center,
        };

        let mut out = Vec::with_capacity(data_offset as usize + data.len());
        out.extend_from_slice(&header.to_bytes());
        out.extend_from_slice(&root);
        out.extend_from_slice(&metadata);
        out.extend_from_slice(&leaves);
        out.extend_from_slice(&data);
        Ok(out)
    }
}

/// `(root, leaves)`, both compressed. A root-only directory when it fits the budget; otherwise
/// leaves of a size chosen like go-pmtiles does (start at max(4096, n / 3500) entries and grow by
/// 1.2× until the root fits).
fn build_directories(entries: &[Entry], c: Compression, max_root: usize, forced: Option<usize>) -> Result<(Vec<u8>, Vec<u8>), GeoError> {
    if let Some(k) = forced {
        return build_leaves(entries, c, k.max(1));
    }
    let root = compress(&directory::serialize(entries), c)?;
    if root.len() <= max_root {
        return Ok((root, Vec::new()));
    }
    let mut leaf_size = (entries.len() as f64 / 3500.0).max(4096.0);
    loop {
        let size = leaf_size as usize;
        let (root, leaves) = build_leaves(entries, c, size)?;
        // A single leaf is as small as the root gets; stop there even if over budget.
        if root.len() <= max_root || size >= entries.len() {
            return Ok((root, leaves));
        }
        leaf_size *= 1.2;
    }
}

fn build_leaves(entries: &[Entry], c: Compression, size: usize) -> Result<(Vec<u8>, Vec<u8>), GeoError> {
    let mut leaves = Vec::new();
    let mut root_entries = Vec::new();
    for chunk in entries.chunks(size) {
        let leaf = compress(&directory::serialize(chunk), c)?;
        root_entries.push(Entry { tile_id: chunk[0].tile_id, offset: leaves.len() as u64, length: leaf.len() as u32, run_length: 0 });
        leaves.extend_from_slice(&leaf);
    }
    Ok((compress(&directory::serialize(&root_entries), c)?, leaves))
}

#[cfg(test)]
mod tests {
    use super::super::{read_tile, Reader, Step};
    use super::*;

    #[test]
    fn empty_archive_is_valid() {
        let bytes = Writer::new(WriterOptions::default()).finish().unwrap();
        let r = Reader::open(&bytes).unwrap();
        assert_eq!(r.header().addressed_tiles, 0);
        assert_eq!(read_tile(&bytes, TileId::new(0, 0, 0)), Ok(None));
    }

    #[test]
    fn invalid_tile_fails_finish() {
        let mut w = Writer::new(WriterOptions::default());
        w.add_tile(TileId::new(1, 2, 0), vec![1]);
        assert!(w.finish().is_err());
    }

    #[test]
    fn replacing_a_tile_keeps_the_last() {
        let mut w = Writer::new(WriterOptions::default());
        w.add_tile(TileId::new(0, 0, 0), b"old".to_vec());
        w.add_tile(TileId::new(0, 0, 0), b"new".to_vec());
        assert_eq!(w.len(), 1);
        let bytes = w.finish().unwrap();
        assert_eq!(read_tile(&bytes, TileId::new(0, 0, 0)), Ok(Some(b"new".to_vec())));
    }

    #[test]
    fn runs_collapse_consecutive_identical_tiles() {
        let mut w = Writer::new(WriterOptions::default());
        // Four z1 tiles are ids 1..=4, consecutive; give ids 2..=4 the same bytes.
        w.add_tile(TileId::new(1, 0, 0), b"a".to_vec());
        for (x, y) in [(0, 1), (1, 1), (1, 0)] {
            w.add_tile(TileId::new(1, x, y), b"b".to_vec());
        }
        let bytes = w.finish().unwrap();
        let mut r = Reader::open(&bytes).unwrap();
        assert_eq!(r.header().tile_entries, 2);
        assert_eq!(r.header().tile_contents, 2);
        assert_eq!(r.header().addressed_tiles, 4);
        assert_eq!(r.plan_get(TileId::new(0, 0, 0)), Ok(Step::Absent));
        assert_eq!(read_tile(&bytes, TileId::new(1, 1, 0)), Ok(Some(b"b".to_vec())));
    }

    #[test]
    fn deterministic_bytes() {
        let build = || {
            let mut w = Writer::new(WriterOptions { leaf_entries: Some(3), ..WriterOptions::default() });
            for x in 0..4 {
                for y in 0..4 {
                    w.add_tile(TileId::new(2, x, y), format!("{x}/{y}").into_bytes());
                }
            }
            w.finish().unwrap()
        };
        assert_eq!(build(), build());
    }
}
