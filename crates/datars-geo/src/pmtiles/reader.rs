//! The sans-IO reader. A lookup is a small state machine the host drives:
//!
//! ```text
//! plan_get(tile) ─► Needs(range) ─► host fetches ─► provide(range, bytes) ─► plan_get(tile) ─► …
//!                ─► Ready(tile bytes)      ─► Absent
//! ```
//!
//! Leaf directories are parsed once and cached (bounded, oldest evicted first — deterministic).
//! Tile payloads handed to `provide` wait until the next `plan_get` for a tile stored at that
//! range, which decompresses and returns them.

use super::directory::{self, Entry};
use super::{decompress, ByteRange, Header, PREFIX_LEN};
use crate::{GeoError, TileId};
use std::collections::BTreeMap;

/// Leaf directories kept in memory. Leaves are cheap to refetch (and HTTP-cacheable).
const MAX_LEAVES: usize = 256;
/// Tile payloads provided but not yet consumed.
const MAX_PENDING: usize = 64;
/// Directory depth guard: root plus up to three levels of leaves.
const MAX_DEPTH: usize = 4;

/// The next thing a tile lookup needs.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// Fetch these bytes and [`Reader::provide`] them, then call [`Reader::plan_get`] again.
    Needs(ByteRange),
    /// The tile, decompressed.
    Ready(Vec<u8>),
    /// The archive has no such tile.
    Absent,
}

/// A PMTiles reader over an archive it never touches directly.
pub struct Reader {
    header: Header,
    root: Vec<Entry>,
    /// Parsed leaf directories by absolute offset, with the insertion stamp for eviction.
    leaves: BTreeMap<u64, (u64, Vec<Entry>)>,
    /// Provided tile payloads (still compressed) by range, with insertion stamps.
    pending: BTreeMap<ByteRange, (u64, Vec<u8>)>,
    stamp: u64,
}

impl Reader {
    /// Open from the archive's first bytes: at least the header and the root directory.
    /// [`PREFIX_LEN`] bytes are always enough.
    pub fn open(prefix: &[u8]) -> Result<Reader, GeoError> {
        let header = Header::parse(prefix)?;
        let root_end = header.root_offset.checked_add(header.root_length).filter(|&e| e <= prefix.len() as u64);
        let Some(root_end) = root_end else {
            return Err(GeoError::format(format!(
                "PMTiles: prefix of {} bytes doesn't hold the root directory (ends at {}; spec limit {PREFIX_LEN})",
                prefix.len(),
                header.root_offset.saturating_add(header.root_length)
            )));
        };
        let raw = &prefix[header.root_offset as usize..root_end as usize];
        let root = directory::deserialize(&decompress(raw, header.internal_compression)?)?;
        Ok(Reader { header, root, leaves: BTreeMap::new(), pending: BTreeMap::new(), stamp: 0 })
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Where the JSON metadata lives.
    pub fn metadata_range(&self) -> ByteRange {
        ByteRange::new(self.header.metadata_offset, self.header.metadata_length)
    }

    /// Decompress and parse the metadata bytes fetched from [`Reader::metadata_range`]. An empty
    /// metadata section is an empty object.
    pub fn parse_metadata(&self, bytes: &[u8]) -> Result<serde_json::Value, GeoError> {
        if bytes.is_empty() {
            return Ok(serde_json::Value::Object(serde_json::Map::new()));
        }
        let raw = decompress(bytes, self.header.internal_compression)?;
        serde_json::from_slice(&raw).map_err(|e| GeoError::Json(e.to_string()))
    }

    /// The next step towards `tile`'s bytes.
    pub fn plan_get(&mut self, tile: TileId) -> Result<Step, GeoError> {
        if !tile.is_valid() || tile.z > self.header.max_zoom {
            return Ok(Step::Absent);
        }
        let id = tile.pmtiles_id();
        let mut leaf: Option<u64> = None;
        for _ in 0..MAX_DEPTH {
            let dir: &[Entry] = match leaf {
                None => &self.root,
                Some(off) => &self.leaves[&off].1,
            };
            let Some(e) = directory::find(dir, id) else { return Ok(Step::Absent) };
            if e.run_length == 0 {
                let range = ByteRange::new(self.header.leaf_offset.saturating_add(e.offset), e.length as u64);
                if !self.leaves.contains_key(&range.offset) {
                    return Ok(Step::Needs(range));
                }
                leaf = Some(range.offset);
                continue;
            }
            let range = ByteRange::new(self.header.data_offset.saturating_add(e.offset), e.length as u64);
            if range.length == 0 {
                return Ok(Step::Ready(Vec::new()));
            }
            return match self.pending.remove(&range) {
                Some((_, raw)) => decompress(&raw, self.header.tile_compression).map(Step::Ready),
                None => Ok(Step::Needs(range)),
            };
        }
        Err(GeoError::format("PMTiles: leaf directories nested too deeply"))
    }

    /// Hand over bytes the reader asked for. Leaf directories are parsed and cached; tile payloads
    /// are held for the next [`Reader::plan_get`].
    pub fn provide(&mut self, range: ByteRange, bytes: &[u8]) -> Result<(), GeoError> {
        if bytes.len() as u64 != range.length {
            return Err(GeoError::format(format!("PMTiles: provided {} bytes for a {}-byte range", bytes.len(), range.length)));
        }
        let h = &self.header;
        let within = |off: u64, len: u64| range.offset >= off && range.end() <= off.saturating_add(len);
        self.stamp += 1;
        if h.leaf_length > 0 && within(h.leaf_offset, h.leaf_length) {
            let entries = directory::deserialize(&decompress(bytes, h.internal_compression)?)?;
            evict_oldest(&mut self.leaves, MAX_LEAVES);
            self.leaves.insert(range.offset, (self.stamp, entries));
            Ok(())
        } else if within(h.data_offset, h.data_length) {
            evict_oldest(&mut self.pending, MAX_PENDING);
            self.pending.insert(range, (self.stamp, bytes.to_vec()));
            Ok(())
        } else {
            Err(GeoError::format(format!("PMTiles: range {range:?} is neither a leaf directory nor tile data")))
        }
    }

    /// Drive a lookup to completion with a synchronous `fetch` (a slice, a file, a blocking HTTP
    /// client — still sans-IO from the reader's side).
    pub fn get_with<E>(&mut self, tile: TileId, mut fetch: impl FnMut(ByteRange) -> Result<Vec<u8>, E>) -> Result<Option<Vec<u8>>, E>
    where
        E: From<GeoError>,
    {
        // Each leaf level costs one fetch and the tile one more.
        for _ in 0..MAX_DEPTH + 2 {
            match self.plan_get(tile)? {
                Step::Absent => return Ok(None),
                Step::Ready(bytes) => return Ok(Some(bytes)),
                Step::Needs(range) => {
                    let bytes = fetch(range)?;
                    self.provide(range, &bytes)?;
                }
            }
        }
        Err(GeoError::format("PMTiles: lookup did not converge").into())
    }
}

/// Make room for one more entry by dropping the oldest-inserted ones.
fn evict_oldest<K: Ord + Clone, V>(map: &mut BTreeMap<K, (u64, V)>, cap: usize) {
    while map.len() >= cap {
        let Some(oldest) = map.iter().min_by_key(|(_, (stamp, _))| *stamp).map(|(k, _)| k.clone()) else { return };
        map.remove(&oldest);
    }
}

/// Read one tile from a whole archive held in memory.
pub fn read_tile(archive: &[u8], tile: TileId) -> Result<Option<Vec<u8>>, GeoError> {
    let mut reader = Reader::open(archive)?;
    reader.get_with(tile, |r: ByteRange| {
        let end = usize::try_from(r.end()).ok().filter(|&e| e <= archive.len());
        match end {
            Some(end) => Ok(archive[r.offset as usize..end].to_vec()),
            None => Err(GeoError::format(format!("PMTiles: range {r:?} past the end of the archive"))),
        }
    })
}
