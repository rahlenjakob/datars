//! One PMTiles archive, driven sans-IO: the engine asks for byte ranges, the host answers with
//! bytes. The first range is the header + root directory; after that each tile costs one range
//! (plus, for big archives, an occasional leaf directory — parsed and cached by the reader).

use datars_geo::pmtiles::{ByteRange, Reader, Step as ReaderStep};
use datars_geo::TileId;
use std::collections::BTreeMap;

pub(crate) enum Step {
    /// These bytes are needed next.
    Needs(ByteRange),
    /// The tile's payload, decompressed.
    Ready(Vec<u8>),
    Absent,
    /// The archive can't be read (bad header, unreadable range).
    Failed(String),
}

pub(crate) struct Archive {
    pub url: String,
    reader: Option<Reader>,
    error: Option<String>,
    /// Ranges asked for and not provided yet, with the tiles waiting on each.
    pending: BTreeMap<ByteRange, Vec<TileId>>,
    /// The whole archive, when the host handed it over at once.
    whole: Option<std::sync::Arc<Vec<u8>>>,
}

impl Archive {
    pub fn new(url: &str) -> Archive {
        // The header is always needed: ask for it up front (hosts can fetch it with the document).
        let mut pending = BTreeMap::new();
        pending.insert(super::header_range(), Vec::new());
        Archive { url: url.to_string(), reader: None, error: None, pending, whole: None }
    }

    pub fn set_whole(&mut self, bytes: Vec<u8>) {
        self.whole = Some(std::sync::Arc::new(bytes));
        self.pending.clear();
    }

    /// A range out of the whole archive, if it's in memory (clamped at the end of the file).
    pub fn slice(&self, r: ByteRange) -> Option<Vec<u8>> {
        let w = self.whole.as_ref()?;
        let start = (r.offset as usize).min(w.len());
        let end = (r.end() as usize).min(w.len());
        Some(w[start..end].to_vec())
    }

    pub fn pending(&self) -> impl Iterator<Item = &ByteRange> {
        self.pending.keys().filter(move |_| self.whole.is_none())
    }

    /// What the archive's tiles are, once its header is in (vector tiles, or point tiles —
    /// `Unknown` to PMTiles).
    pub fn tile_type(&self) -> Option<datars_geo::pmtiles::TileType> {
        self.reader.as_ref().map(|r| r.header().tile_type)
    }

    pub fn zoom_range(&self) -> Option<(u8, u8)> {
        self.reader.as_ref().map(|r| (r.header().min_zoom, r.header().max_zoom))
    }

    pub fn step(&mut self, t: TileId) -> Step {
        if let Some(e) = &self.error {
            return Step::Failed(e.clone());
        }
        let Some(r) = self.reader.as_mut() else { return Step::Needs(super::header_range()) };
        match r.plan_get(t) {
            Ok(ReaderStep::Needs(range)) => Step::Needs(range),
            Ok(ReaderStep::Ready(bytes)) => Step::Ready(bytes),
            Ok(ReaderStep::Absent) => Step::Absent,
            Err(e) => Step::Failed(e.to_string()),
        }
    }

    /// Record that `t` waits on `range`. True if the range is newly requested.
    pub fn want(&mut self, range: ByteRange, t: TileId) -> bool {
        let fresh = !self.pending.contains_key(&range);
        let w = self.pending.entry(range).or_default();
        if !w.contains(&t) {
            w.push(t);
        }
        fresh
    }

    pub fn fail(&mut self, e: String) {
        self.error = Some(e);
        self.pending.clear();
    }

    /// Bytes for a range starting at `offset`. Returns the tiles that were waiting on it.
    pub fn provide(&mut self, offset: u64, bytes: &[u8]) -> Result<Vec<TileId>, String> {
        match self.reader.as_mut() {
            None => {
                if offset != 0 {
                    return Err(format!("bytes at {offset} before the header"));
                }
                // The prefix may be shorter than asked for: the archive can be smaller than 16 KiB.
                match Reader::open(bytes) {
                    Ok(r) => self.reader = Some(r),
                    Err(e) => {
                        let msg = e.to_string();
                        self.fail(msg.clone());
                        return Err(msg);
                    }
                }
                Ok(self.pending.remove(&super::header_range()).unwrap_or_default())
            }
            Some(r) => {
                if offset == 0 {
                    return Ok(Vec::new()); // the header again
                }
                // The reader checks the range is a leaf directory or tile data.
                let range = ByteRange::new(offset, bytes.len() as u64);
                r.provide(range, bytes).map_err(|e| e.to_string())?;
                Ok(self.pending.remove(&range).unwrap_or_default())
            }
        }
    }
}
