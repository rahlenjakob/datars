//! Point tiles as bytes, and a pyramid as a PMTiles archive (tile type "unknown"; the tiles say
//! what they are). Decoding gives back exactly the tile that was encoded — grid positions, row
//! numbers and every stored value bit for bit — so a pyramid read from an archive draws exactly
//! like the one built in memory from the same rows.
//!
//! ```text
//! "DPT1" · x0 y0 size (f64 LE) · levels (u8) · rows (varint) · budget (varint) · n (varint)
//! columns (varint), each: name (varint length + UTF-8) · codec (u8) [· decimals (u8)]
//! gx, gy: n offsets within the tile each, bit-packed at `levels − z + 16` bits (LSB first)
//! rows: n varints, each the difference from the previous row (ascending)
//! per column: 0 raw f64 LE · 1 integers (zigzag varints) · 2 decimals: zigzag varints of v·10^d
//!             3 strings: dictionary (varint count, each varint length + UTF-8), then n varints
//!               (index + 1, 0 = null) · 4 booleans (a byte each) · 5 dates (varint zigzag + 1,
//!               0 = null)
//! ```
//! A numeric column takes the smallest codec that gives back every value exactly (checked by
//! decoding), so data produced as integers or with a few decimals costs a byte or two per row.

use super::pyramid::{Extent, Header, PointTile, Pyramid};
use datars_algo::pyramid::SUB_BITS;
use datars_data::Column;
use datars_geo::pmtiles::{Compression, TileType, Writer, WriterOptions};
use datars_geo::TileId;
use std::sync::Arc;

const MAGIC: &[u8; 4] = b"DPT1";
const P10: [f64; 7] = [1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6];

fn varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn zigzag(v: i64) -> u64 {
    ((v << 1) ^ (v >> 63)) as u64
}

fn unzigzag(v: u64) -> i64 {
    ((v >> 1) as i64) ^ -((v & 1) as i64)
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.b.len()).ok_or("point tile: truncated")?;
        let s = &self.b[self.at..end];
        self.at = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn f64(&mut self) -> Result<f64, String> {
        let mut a = [0u8; 8];
        a.copy_from_slice(self.take(8)?);
        Ok(f64::from_le_bytes(a))
    }
    fn varint(&mut self) -> Result<u64, String> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let b = self.u8()?;
            v |= ((b & 0x7f) as u64) << shift;
            if b < 0x80 {
                return Ok(v);
            }
        }
        Err("point tile: bad varint".into())
    }
    fn count(&mut self, per: usize) -> Result<usize, String> {
        // A count must fit the bytes left (each item costs at least `per` bytes): corrupt input
        // can't make the decoder allocate more than the tile's size.
        let n = self.varint()? as usize;
        if n.saturating_mul(per) > self.b.len() - self.at {
            return Err("point tile: count exceeds the data".into());
        }
        Ok(n)
    }
    fn str(&mut self) -> Result<String, String> {
        let n = self.count(1)?;
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| "point tile: bad UTF-8".to_string())
    }
}

/// Numeric codecs, smallest first.
#[derive(Clone, Copy, Debug, PartialEq)]
enum NumCodec {
    Raw,
    Int,
    Decimal(u8),
}

fn num_decode(q: i64, c: NumCodec) -> f64 {
    match c {
        NumCodec::Int => q as f64,
        NumCodec::Decimal(d) => q as f64 / P10[d as usize],
        NumCodec::Raw => f64::NAN,
    }
}

fn num_quantize(v: f64, c: NumCodec) -> Option<i64> {
    let s = match c {
        NumCodec::Int => v,
        NumCodec::Decimal(d) => v * P10[d as usize],
        NumCodec::Raw => return None,
    };
    let q = s.round();
    (q.abs() < 9.0e15).then_some(q as i64).filter(|&q| num_decode(q, c).to_bits() == v.to_bits())
}

fn num_codec(v: &[f64]) -> NumCodec {
    let fits = |c: NumCodec| v.iter().all(|&x| num_quantize(x, c).is_some());
    std::iter::once(NumCodec::Int).chain((1..=6).map(NumCodec::Decimal)).find(|&c| fits(c)).unwrap_or(NumCodec::Raw)
}

/// Encode a tile.
pub fn encode(t: &PointTile) -> Vec<u8> {
    let h = &t.header;
    let n = t.len();
    let mut out = Vec::with_capacity(64 + n * 8);
    out.extend_from_slice(MAGIC);
    for v in [h.extent.x0, h.extent.y0, h.extent.size] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.push(h.levels);
    varint(&mut out, h.rows);
    varint(&mut out, h.budget as u64);
    varint(&mut out, n as u64);
    varint(&mut out, t.columns.len() as u64);
    let codecs: Vec<(u8, NumCodec)> = t
        .columns
        .iter()
        .map(|(_, c)| match c {
            Column::Num(v) => match num_codec(v) {
                NumCodec::Raw => (0, NumCodec::Raw),
                NumCodec::Int => (1, NumCodec::Int),
                d => (2, d),
            },
            Column::Str(_) => (3, NumCodec::Raw),
            Column::Bool(_) => (4, NumCodec::Raw),
            Column::Date(_) => (5, NumCodec::Raw),
        })
        .collect();
    for ((name, _), (tag, c)) in t.columns.iter().zip(&codecs) {
        varint(&mut out, name.len() as u64);
        out.extend_from_slice(name.as_bytes());
        out.push(*tag);
        if let NumCodec::Decimal(d) = c {
            out.push(*d);
        }
    }
    let bits = offset_bits(h.levels, t.id.z);
    let cells = h.tile_cells(t.id.z);
    for (g, origin) in [(&t.gx, t.id.x as u64 * cells), (&t.gy, t.id.y as u64 * cells)] {
        pack(&mut out, g.iter().map(|&v| v - origin), bits);
    }
    let mut prev = 0u64;
    for &r in &t.row {
        varint(&mut out, r - prev);
        prev = r;
    }
    for ((_, col), (_, c)) in t.columns.iter().zip(&codecs) {
        match col {
            Column::Num(v) => match c {
                NumCodec::Raw => v.iter().for_each(|x| out.extend_from_slice(&canonical(*x).to_le_bytes())),
                c => v.iter().for_each(|x| varint(&mut out, zigzag(num_quantize(*x, *c).unwrap_or(0)))),
            },
            Column::Str(v) => {
                let mut dict: Vec<&str> = Vec::new();
                let mut index: std::collections::BTreeMap<&str, u64> = std::collections::BTreeMap::new();
                let ids: Vec<u64> = v
                    .iter()
                    .map(|s| match s {
                        None => 0,
                        Some(s) => *index.entry(s.as_ref()).or_insert_with(|| {
                            dict.push(s.as_ref());
                            dict.len() as u64
                        }),
                    })
                    .collect();
                varint(&mut out, dict.len() as u64);
                for s in dict {
                    varint(&mut out, s.len() as u64);
                    out.extend_from_slice(s.as_bytes());
                }
                ids.into_iter().for_each(|i| varint(&mut out, i));
            }
            Column::Bool(v) => out.extend(v.iter().map(|&b| b as u8)),
            Column::Date(v) => v.iter().for_each(|d| varint(&mut out, d.map_or(0, |d| zigzag(d as i64) + 1))),
        }
    }
    out
}

fn canonical(v: f64) -> f64 {
    if v.is_nan() {
        f64::NAN
    } else {
        v
    }
}

/// Bits per grid offset in a tile of level `z`: the grid cells across it.
fn offset_bits(levels: u8, z: u8) -> u32 {
    (levels.saturating_sub(z) + SUB_BITS) as u32
}

/// Append `values` as `bits`-bit fields, least significant bit first, padded to a whole byte.
fn pack(out: &mut Vec<u8>, values: impl Iterator<Item = u64>, bits: u32) {
    let (mut acc, mut have) = (0u128, 0u32);
    for v in values {
        acc |= ((v & ((1u64 << bits) - 1)) as u128) << have;
        have += bits;
        while have >= 8 {
            out.push(acc as u8);
            acc >>= 8;
            have -= 8;
        }
    }
    if have > 0 {
        out.push(acc as u8);
    }
}

/// Read `n` fields of `bits` bits written by [`pack`].
fn unpack(bytes: &[u8], n: usize, bits: u32) -> Vec<u64> {
    let mut out = Vec::with_capacity(n);
    let (mut acc, mut have, mut at) = (0u128, 0u32, 0usize);
    let mask = (1u64 << bits) - 1;
    for _ in 0..n {
        while have < bits {
            acc |= (bytes.get(at).copied().unwrap_or(0) as u128) << have;
            at += 1;
            have += 8;
        }
        out.push(acc as u64 & mask);
        acc >>= bits;
        have -= bits;
    }
    out
}

/// Decode a tile (the id is where the archive keeps it).
pub fn decode(id: TileId, bytes: &[u8]) -> Result<PointTile, String> {
    let mut r = Reader { b: bytes, at: 0 };
    if r.take(4)? != MAGIC {
        return Err("not a datars point tile".into());
    }
    let extent = Extent { x0: r.f64()?, y0: r.f64()?, size: r.f64()? };
    let levels = r.u8()?;
    if levels > super::pyramid::MAX_LEVELS || id.z > levels {
        return Err(format!("point tile {}/{}/{}: level out of range", id.z, id.x, id.y));
    }
    let header = Header { extent, levels, rows: r.varint()?, budget: r.varint()? as u32 };
    let bits = offset_bits(levels, id.z);
    let n = r.count(1)?;
    if n.saturating_mul(2 * bits as usize) / 8 > bytes.len() {
        return Err("point tile: count exceeds the data".into());
    }
    let ncols = r.count(2)?;
    let mut specs = Vec::with_capacity(ncols);
    for _ in 0..ncols {
        let name = r.str()?;
        let tag = r.u8()?;
        let c = if tag == 2 {
            let d = r.u8()?;
            if d as usize >= P10.len() {
                return Err("point tile: bad decimals".into());
            }
            NumCodec::Decimal(d)
        } else if tag == 1 {
            NumCodec::Int
        } else {
            NumCodec::Raw
        };
        specs.push((name, tag, c));
    }
    let cells = header.tile_cells(id.z);
    let mut grid = |origin: u64| -> Result<Vec<u64>, String> {
        let raw = r.take((n * bits as usize).div_ceil(8))?;
        Ok(unpack(raw, n, bits).into_iter().map(|v| origin + v.min(cells - 1)).collect())
    };
    let gx = grid(id.x as u64 * cells)?;
    let gy = grid(id.y as u64 * cells)?;
    let mut row = Vec::with_capacity(n);
    let mut prev = 0u64;
    for _ in 0..n {
        prev = prev.checked_add(r.varint()?).ok_or("point tile: row overflow")?;
        row.push(prev);
    }
    let mut columns = Vec::with_capacity(ncols);
    for (name, tag, c) in specs {
        let col = match tag {
            0 => Column::Num((0..n).map(|_| r.f64()).collect::<Result<_, _>>()?),
            1 | 2 => Column::Num((0..n).map(|_| r.varint().map(|q| num_decode(unzigzag(q), c))).collect::<Result<_, _>>()?),
            3 => {
                let k = r.count(1)?;
                let dict: Vec<Arc<str>> = (0..k).map(|_| r.str().map(|s| Arc::from(s.as_str()))).collect::<Result<_, _>>()?;
                let mut v = Vec::with_capacity(n);
                for _ in 0..n {
                    let i = r.varint()? as usize;
                    v.push(if i == 0 { None } else { Some(dict.get(i - 1).cloned().ok_or("point tile: bad string index")?) });
                }
                Column::Str(v)
            }
            4 => Column::Bool(r.take(n)?.iter().map(|&b| b != 0).collect()),
            5 => Column::Date((0..n).map(|_| r.varint().map(|v| if v == 0 { None } else { Some(unzigzag(v - 1) as i32) })).collect::<Result<_, _>>()?),
            t => return Err(format!("point tile: unknown column codec {t}")),
        };
        columns.push((name, col));
    }
    Ok(PointTile { id, header, gx, gy, row, columns })
}

/// A pyramid as a PMTiles archive (tiles gzipped), with a summary in its metadata for tools.
pub fn archive(p: &Pyramid) -> Result<Vec<u8>, String> {
    let columns: Vec<serde_json::Value> = p.tiles.values().next().map(|t| t.columns.iter().map(|(n, c)| serde_json::json!({ "name": n, "type": c.ty().name() })).collect()).unwrap_or_default();
    let meta = serde_json::json!({
        "format": "datars-points", "version": 1,
        "extent": [p.header.extent.x0, p.header.extent.y0, p.header.extent.size],
        "levels": p.header.levels, "rows": p.header.rows, "budget": p.header.budget,
        "tiles": p.tiles.len(), "columns": columns,
    });
    let mut w = Writer::new(WriterOptions { tile_type: TileType::Unknown, tile_compression: Compression::Gzip, metadata: meta, ..WriterOptions::default() });
    for t in p.tiles.values() {
        w.add_tile(t.id, encode(t));
    }
    w.finish().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::points::pyramid::{build, Options};

    #[test]
    fn tiles_round_trip_exactly() {
        let n = 3000;
        let mut rng = datars_math::Rng::new(9);
        let xs: Vec<f64> = (0..n).map(|_| rng.range(-5e4, 5e4)).collect();
        let ys: Vec<f64> = (0..n).map(|_| rng.range(-3e4, 3e4)).collect();
        let ints = Column::Num((0..n).map(|i| (i * 7 % 30000) as f64).collect());
        let decs = Column::Num((0..n).map(|i| ((i % 900) as f64 - 300.0) / 100.0).collect());
        let raw = Column::Num((0..n).map(|_| rng.next_f64()).collect());
        let gaps = Column::Num((0..n).map(|i| if i % 5 == 0 { f64::NAN } else { i as f64 }).collect());
        let names = Column::Str((0..n).map(|i| (i % 3 != 0).then(|| Arc::from(["O", "B", "A", "F", "G", "K", "M"][i % 7]))).collect());
        let flags = Column::Bool((0..n).map(|i| i % 2 == 0).collect());
        let days = Column::Date((0..n).map(|i| (i % 4 != 0).then_some(i as i32 - 1000)).collect());
        let cols = [("t", &ints), ("l", &decs), ("r", &raw), ("g", &gaps), ("c", &names), ("b", &flags), ("d", &days)].map(|(k, c)| (k.to_string(), c));
        let p = build(&xs, &ys, &cols, &Options::with_budget(64));
        for t in p.tiles.values() {
            let bytes = encode(t);
            let back = decode(t.id, &bytes).unwrap();
            assert_eq!(back.gx, t.gx);
            assert_eq!(back.row, t.row);
            for ((_, a), (_, b)) in back.columns.iter().zip(&t.columns) {
                assert_eq!(a, b);
            }
            assert_eq!(back.header, t.header);
        }
        // Integers and two-decimal values take the compact codecs; anything else stays exact.
        let nums = |c: &Column| c.as_num().unwrap().to_vec();
        assert_eq!(num_codec(&nums(&ints)), NumCodec::Int);
        assert_eq!(num_codec(&nums(&decs)), NumCodec::Decimal(2));
        assert_eq!(num_codec(&nums(&raw)), NumCodec::Raw);
        assert_eq!(num_codec(&nums(&gaps)), NumCodec::Raw);
        assert_eq!(num_codec(&[-0.0]), NumCodec::Raw);
        // A tile of positions, row numbers and one small integer column: a few bytes a row.
        let small = build(&xs, &ys, &[("t".to_string(), &ints)], &Options::with_budget(64));
        let root = &small.tiles[&TileId::new(0, 0, 0)];
        let per_row = encode(root).len() as f64 / root.len() as f64;
        assert!(per_row < 14.0, "{per_row} bytes a row");
    }

    #[test]
    fn corrupt_tiles_are_errors_not_panics() {
        let p = build(&[0.0, 1.0, 2.0], &[0.0, 1.0, 0.5], &[], &Options::with_budget(4));
        let t = &p.tiles[&TileId::new(0, 0, 0)];
        let bytes = encode(t);
        for cut in 0..bytes.len() {
            assert!(decode(t.id, &bytes[..cut]).is_err());
        }
        assert!(decode(t.id, b"nope").is_err());
        let mut huge = bytes.clone();
        huge[4 + 24 + 1 + 1 + 1] = 0xff; // the row count's first byte: a count past the data
        let _ = decode(t.id, &huge);
    }

    #[test]
    fn an_archive_holds_every_tile() {
        let mut rng = datars_math::Rng::new(1);
        let xs: Vec<f64> = (0..5000).map(|_| rng.next_f64()).collect();
        let ys: Vec<f64> = (0..5000).map(|_| rng.next_f64()).collect();
        let p = build(&xs, &ys, &[], &Options::with_budget(128));
        let bytes = archive(&p).unwrap();
        for t in p.tiles.values() {
            let raw = datars_geo::pmtiles::read_tile(&bytes, t.id).unwrap().unwrap();
            assert_eq!(decode(t.id, &raw).unwrap(), **t);
        }
        assert!(datars_geo::pmtiles::read_tile(&bytes, TileId::new(p.header.levels + 1, 0, 0)).unwrap().is_none());
    }
}
