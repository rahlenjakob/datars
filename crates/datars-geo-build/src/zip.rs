//! Just enough ZIP to unpack a downloaded shapefile: the central directory, stored or deflated
//! entries (the flate2 the workspace already has), no ZIP64. One entry is streamed to a file.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

fn u16le(b: &[u8], i: usize) -> u64 {
    u16::from_le_bytes([b[i], b[i + 1]]) as u64
}
fn u32le(b: &[u8], i: usize) -> u64 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as u64
}

/// Copy the first entry whose name ends with `suffix` out of the archive at `zip` into `out`.
pub fn extract_suffix(zip: &Path, suffix: &str, out: &Path) -> Result<(), String> {
    let err = |e: std::io::Error| format!("{}: {e}", zip.display());
    let mut f = std::fs::File::open(zip).map_err(err)?;
    let len = f.metadata().map_err(err)?.len();
    // The end-of-central-directory record: in the last 22 bytes plus a comment of up to 64 KiB.
    let tail_len = len.min(22 + 65_535);
    f.seek(SeekFrom::Start(len - tail_len)).map_err(err)?;
    let mut tail = vec![0u8; tail_len as usize];
    f.read_exact(&mut tail).map_err(err)?;
    let eocd = (0..tail.len().saturating_sub(21)).rev().find(|&i| tail[i..i + 4] == [0x50, 0x4b, 0x05, 0x06]).ok_or("not a zip archive")?;
    let (entries, cd_size, cd_offset) = (u16le(&tail, eocd + 10), u32le(&tail, eocd + 12), u32le(&tail, eocd + 16));
    if cd_offset == 0xFFFF_FFFF || entries == 0xFFFF {
        return Err("ZIP64 archives are not supported".into());
    }
    f.seek(SeekFrom::Start(cd_offset)).map_err(err)?;
    let mut cd = vec![0u8; cd_size as usize];
    f.read_exact(&mut cd).map_err(err)?;
    let mut i = 0usize;
    for _ in 0..entries {
        if cd.len() < i + 46 || cd[i..i + 4] != [0x50, 0x4b, 0x01, 0x02] {
            return Err("corrupt central directory".into());
        }
        let (method, csize, name_len, extra_len, comment_len, local) = (u16le(&cd, i + 10), u32le(&cd, i + 20), u16le(&cd, i + 28) as usize, u16le(&cd, i + 30) as usize, u16le(&cd, i + 32) as usize, u32le(&cd, i + 42));
        let name = String::from_utf8_lossy(&cd[i + 46..i + 46 + name_len]).into_owned();
        i += 46 + name_len + extra_len + comment_len;
        if !name.ends_with(suffix) {
            continue;
        }
        if csize == 0xFFFF_FFFF {
            return Err("ZIP64 entries are not supported".into());
        }
        f.seek(SeekFrom::Start(local)).map_err(err)?;
        let mut lh = [0u8; 30];
        f.read_exact(&mut lh).map_err(err)?;
        if lh[0..4] != [0x50, 0x4b, 0x03, 0x04] {
            return Err("corrupt local header".into());
        }
        let skip = u16le(&lh, 26) + u16le(&lh, 28);
        f.seek(SeekFrom::Current(skip as i64)).map_err(err)?;
        let data = (&mut f).take(csize);
        let mut w = std::io::BufWriter::new(std::fs::File::create(out).map_err(|e| format!("{}: {e}", out.display()))?);
        let copied = match method {
            0 => std::io::copy(&mut { data }, &mut w),
            8 => std::io::copy(&mut flate2::read::DeflateDecoder::new(data), &mut w),
            m => return Err(format!("{name}: compression method {m} is not supported")),
        };
        copied.map_err(|e| format!("{name}: {e}"))?;
        return Ok(());
    }
    Err(format!("no `*{suffix}` in {}", zip.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A zip with the given (name, bytes, deflate?) entries.
    fn zip(entries: &[(&str, &[u8], bool)]) -> Vec<u8> {
        let (mut out, mut cd) = (Vec::new(), Vec::new());
        for (name, bytes, deflate) in entries {
            let data = if *deflate {
                let mut e = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
                e.write_all(bytes).unwrap();
                e.finish().unwrap()
            } else {
                bytes.to_vec()
            };
            let offset = out.len() as u32;
            let method: u16 = if *deflate { 8 } else { 0 };
            out.extend([0x50, 0x4b, 0x03, 0x04]);
            out.extend([20, 0, 0, 0]);
            out.extend(method.to_le_bytes());
            out.extend([0u8; 8]); // time, date, crc (unchecked here)
            out.extend((data.len() as u32).to_le_bytes());
            out.extend((bytes.len() as u32).to_le_bytes());
            out.extend((name.len() as u16).to_le_bytes());
            out.extend(0u16.to_le_bytes());
            out.extend(name.as_bytes());
            out.extend(&data);
            cd.extend([0x50, 0x4b, 0x01, 0x02, 20, 0, 20, 0, 0, 0]);
            cd.extend(method.to_le_bytes());
            cd.extend([0u8; 8]);
            cd.extend((data.len() as u32).to_le_bytes());
            cd.extend((bytes.len() as u32).to_le_bytes());
            cd.extend((name.len() as u16).to_le_bytes());
            cd.extend([0u8; 12]); // extra, comment, disk, internal and external attributes
            cd.extend(offset.to_le_bytes());
            cd.extend(name.as_bytes());
        }
        let cd_offset = out.len() as u32;
        out.extend(&cd);
        out.extend([0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0]);
        out.extend((entries.len() as u16).to_le_bytes());
        out.extend((entries.len() as u16).to_le_bytes());
        out.extend((cd.len() as u32).to_le_bytes());
        out.extend(cd_offset.to_le_bytes());
        out.extend([0, 0]);
        out
    }

    #[test]
    fn extracts_stored_and_deflated_entries_by_suffix() {
        let dir = crate::fetch::tests::temp("zip");
        let body: Vec<u8> = (0..5000u32).flat_map(|i| (i % 251).to_le_bytes()).collect();
        std::fs::write(dir.join("a.zip"), zip(&[("x/README.txt", b"hello", false), ("x/land.shp", &body, true), ("x/land.shx", b"idx", false)])).unwrap();
        extract_suffix(&dir.join("a.zip"), ".shp", &dir.join("out.shp")).unwrap();
        assert_eq!(std::fs::read(dir.join("out.shp")).unwrap(), body);
        extract_suffix(&dir.join("a.zip"), ".txt", &dir.join("out.txt")).unwrap();
        assert_eq!(std::fs::read(dir.join("out.txt")).unwrap(), b"hello");
        assert!(extract_suffix(&dir.join("a.zip"), ".dbf", &dir.join("x")).is_err());
        std::fs::write(dir.join("b.zip"), b"not a zip at all, clearly").unwrap();
        assert!(extract_suffix(&dir.join("b.zip"), ".shp", &dir.join("x")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
