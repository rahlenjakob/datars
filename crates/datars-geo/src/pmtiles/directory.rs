//! PMTiles directories: a compact, columnar list of entries sorted by tile id.
//!
//! An entry points at tile data (`run_length >= 1`: that many consecutive tile ids share the
//! bytes) or, with `run_length == 0`, at a leaf directory — a second-level directory that keeps the
//! root small for big archives. Columns are stored separately (delta tile ids, run lengths,
//! lengths, offsets) because each column compresses far better on its own; an offset of `0` means
//! "right after the previous entry's bytes".

use crate::GeoError;

/// One directory entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Entry {
    /// First tile id covered.
    pub tile_id: u64,
    /// Byte offset, relative to the tile-data section (data entries) or leaf section (leaves).
    pub offset: u64,
    pub length: u32,
    /// Consecutive tile ids sharing these bytes; `0` marks a leaf-directory pointer.
    pub run_length: u32,
}

pub(crate) fn write_varint(buf: &mut Vec<u8>, mut v: u64) {
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            buf.push(b);
            return;
        }
        buf.push(b | 0x80);
    }
}

struct Varints<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl Varints<'_> {
    fn next(&mut self) -> Result<u64, GeoError> {
        let mut out = 0u64;
        for i in 0..10 {
            let Some(&b) = self.buf.get(self.pos) else {
                return Err(GeoError::format("PMTiles: truncated directory"));
            };
            self.pos += 1;
            out |= ((b & 0x7f) as u64) << (7 * i);
            if b & 0x80 == 0 {
                return Ok(out);
            }
        }
        Err(GeoError::format("PMTiles: bad varint in directory"))
    }
}

/// Serialize entries (already sorted by tile id) into the uncompressed directory bytes.
pub(crate) fn serialize(entries: &[Entry]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(entries.len() * 4 + 4);
    write_varint(&mut buf, entries.len() as u64);
    let mut last = 0u64;
    for e in entries {
        write_varint(&mut buf, e.tile_id - last);
        last = e.tile_id;
    }
    for e in entries {
        write_varint(&mut buf, e.run_length as u64);
    }
    for e in entries {
        write_varint(&mut buf, e.length as u64);
    }
    for (i, e) in entries.iter().enumerate() {
        let contiguous = i > 0 && e.offset == entries[i - 1].offset + entries[i - 1].length as u64;
        write_varint(&mut buf, if contiguous { 0 } else { e.offset + 1 });
    }
    buf
}

/// Parse uncompressed directory bytes.
pub(crate) fn deserialize(bytes: &[u8]) -> Result<Vec<Entry>, GeoError> {
    let mut r = Varints { buf: bytes, pos: 0 };
    let n = r.next()?;
    // Every entry needs at least four bytes (one per column), so a larger count is corrupt — and
    // checking first keeps a hostile count from allocating.
    if n > (bytes.len() as u64) / 4 + 1 {
        return Err(GeoError::format("PMTiles: directory entry count exceeds its size"));
    }
    let n = n as usize;
    let mut entries = vec![Entry { tile_id: 0, offset: 0, length: 0, run_length: 0 }; n];
    let mut last = 0u64;
    for e in entries.iter_mut() {
        last = last.checked_add(r.next()?).ok_or_else(|| GeoError::format("PMTiles: tile id overflow"))?;
        e.tile_id = last;
    }
    let narrow = |v: u64| u32::try_from(v).map_err(|_| GeoError::format("PMTiles: directory value exceeds u32"));
    for e in entries.iter_mut() {
        e.run_length = narrow(r.next()?)?;
    }
    for e in entries.iter_mut() {
        e.length = narrow(r.next()?)?;
    }
    for i in 0..n {
        let v = r.next()?;
        entries[i].offset = if v == 0 {
            let prev = entries.get(i.wrapping_sub(1)).ok_or_else(|| GeoError::format("PMTiles: first offset is a back-reference"))?;
            prev.offset.checked_add(prev.length as u64).ok_or_else(|| GeoError::format("PMTiles: offset overflow"))?
        } else {
            v - 1
        };
    }
    Ok(entries)
}

/// The entry covering `tile_id`: the last entry with `tile_id <=` the target, if it's a leaf
/// pointer or a data run that reaches the target.
pub(crate) fn find(entries: &[Entry], tile_id: u64) -> Option<Entry> {
    let i = entries.partition_point(|e| e.tile_id <= tile_id);
    let e = *entries.get(i.checked_sub(1)?)?;
    if e.run_length == 0 || tile_id - e.tile_id < e.run_length as u64 {
        Some(e)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(tile_id: u64, offset: u64, length: u32, run_length: u32) -> Entry {
        Entry { tile_id, offset, length, run_length }
    }

    #[test]
    fn round_trips_with_back_references_and_leaves() {
        let entries = vec![
            e(0, 0, 100, 1),
            e(1, 100, 50, 1),   // contiguous: stored as 0
            e(5, 100, 50, 3),   // deduplicated back-reference
            e(9, 0, 100, 1),    // back to the start
            e(20, 400, 999, 0), // leaf pointer
        ];
        let bytes = serialize(&entries);
        assert_eq!(deserialize(&bytes), Ok(entries));
        assert_eq!(deserialize(&serialize(&[])), Ok(vec![]));
    }

    #[test]
    fn corrupt_directories_are_errors() {
        let bytes = serialize(&[e(0, 0, 10, 1), e(3, 10, 10, 1)]);
        for cut in 0..bytes.len() {
            assert!(deserialize(&bytes[..cut]).is_err(), "cut at {cut}");
        }
        assert!(deserialize(&[0xff, 0xff, 0xff, 0x0f]).is_err(), "huge count");
        assert!(deserialize(&[1, 0, 1, 1, 0]).is_err(), "first offset back-reference");
    }

    #[test]
    fn find_honours_runs_and_leaves() {
        let dir = vec![e(0, 0, 1, 1), e(5, 1, 1, 3), e(10, 0, 1, 0)];
        assert!(find(&dir, 0).is_some());
        assert!(find(&dir, 3).is_none(), "gap between runs");
        assert_eq!(find(&dir, 7).map(|x| x.tile_id), Some(5), "inside a run");
        assert!(find(&dir, 8).is_none(), "just past the run");
        assert_eq!(find(&dir, 99).map(|x| x.run_length), Some(0), "leaf pointer covers the rest");
        assert!(find(&[e(10, 0, 1, 1)], 5).is_none(), "before the first entry");
    }
}
