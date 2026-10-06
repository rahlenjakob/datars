//! The single-file `.datars` container: header, manifest, chunk index, chunk data. The index sits
//! at the front so a reader can fetch it with one range request and then read chunks by range.

use crate::{chunk_hash, Bundle, BundleError, Manifest};

pub const MAGIC: &[u8; 8] = b"DATARS\0\x01";

pub fn to_single_file(b: &Bundle) -> Vec<u8> {
    let manifest = b.manifest.to_json();
    let mut index: Vec<u8> = Vec::new();
    let mut offset = 0u64;
    for (hash, bytes) in &b.chunks {
        index.extend_from_slice(&(hash.len() as u32).to_le_bytes());
        index.extend_from_slice(hash.as_bytes());
        index.extend_from_slice(&offset.to_le_bytes());
        index.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        offset += bytes.len() as u64;
    }
    let mut out = Vec::with_capacity(16 + manifest.len() + index.len() + offset as usize);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(manifest.len() as u32).to_le_bytes());
    out.extend_from_slice(&manifest);
    out.extend_from_slice(&(b.chunks.len() as u32).to_le_bytes());
    out.extend_from_slice(&index);
    for bytes in b.chunks.values() {
        out.extend_from_slice(bytes);
    }
    out
}

pub fn from_single_file(data: &[u8]) -> Result<Bundle, BundleError> {
    let bad = |m: &str| BundleError::Format(m.to_string());
    if data.len() < 16 || &data[..8] != MAGIC {
        return Err(bad("not a .datars file"));
    }
    let mut p = 8usize;
    let rd32 = |p: &mut usize| -> Result<u32, BundleError> {
        let v = data.get(*p..*p + 4).ok_or_else(|| bad("truncated"))?;
        *p += 4;
        Ok(u32::from_le_bytes(v.try_into().unwrap()))
    };
    let rd64 = |p: &mut usize| -> Result<u64, BundleError> {
        let v = data.get(*p..*p + 8).ok_or_else(|| bad("truncated"))?;
        *p += 8;
        Ok(u64::from_le_bytes(v.try_into().unwrap()))
    };
    let ml = rd32(&mut p)? as usize;
    let manifest = Manifest::from_json(data.get(p..p + ml).ok_or_else(|| bad("truncated manifest"))?)?;
    p += ml;
    let n = rd32(&mut p)? as usize;
    let mut entries = Vec::with_capacity(n);
    for _ in 0..n {
        let hl = rd32(&mut p)? as usize;
        let hash = std::str::from_utf8(data.get(p..p + hl).ok_or_else(|| bad("truncated index"))?).map_err(|_| bad("bad hash"))?.to_string();
        p += hl;
        let off = rd64(&mut p)?;
        let len = rd64(&mut p)?;
        entries.push((hash, off, len));
    }
    let base = p;
    let mut b = Bundle { manifest, chunks: Default::default() };
    for (hash, off, len) in entries {
        let s = base + off as usize;
        let bytes = data.get(s..s + len as usize).ok_or_else(|| bad("truncated chunk"))?.to_vec();
        let actual = chunk_hash(&bytes);
        if actual != hash {
            return Err(BundleError::Hash { expected: hash, actual });
        }
        b.chunks.insert(hash, bytes);
    }
    Ok(b)
}
