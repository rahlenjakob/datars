//! Just enough HTTP for the reference viewer to open published charts by URL (`datars serve`, an
//! intranet, localhost): plain `http://`, GET with an optional byte range, HTTP/1.0 so bodies arrive
//! whole. Production desktop apps use their platform's networking (and TLS); the engine never
//! does IO itself (P10).

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// `scheme://host[:port]`, and the path (with query).
fn split(url: &str) -> Result<(String, String, String), String> {
    let rest = url.strip_prefix("http://").ok_or_else(|| {
        if url.starts_with("https://") {
            format!("{url}: datars-view speaks plain http only (serve locally with `datars serve`)")
        } else {
            format!("{url}: not an http:// URL")
        }
    })?;
    let (authority, path) = rest.split_once('/').map(|(a, p)| (a, format!("/{p}"))).unwrap_or((rest, "/".into()));
    let host = authority.split(':').next().unwrap_or(authority).to_string();
    let addr = if authority.contains(':') { authority.to_string() } else { format!("{authority}:80") };
    Ok((host, addr, path))
}

/// GET `url` (bytes `offset .. offset + length` when `range` is given). A server that ignores the
/// range sends the whole resource; the requested slice is cut from it.
pub fn get(url: &str, range: Option<(u64, u64)>) -> Result<Vec<u8>, String> {
    let (host, addr, path) = split(url)?;
    let mut s = TcpStream::connect(&addr).map_err(|e| format!("{url}: {e}"))?;
    let _ = s.set_read_timeout(Some(Duration::from_secs(30)));
    let range_header = range.map(|(o, l)| format!("Range: bytes={o}-{}\r\n", o + l.max(1) - 1)).unwrap_or_default();
    // One write: servers may read the request in a single call.
    let req = format!("GET {path} HTTP/1.0\r\nHost: {host}\r\n{range_header}Connection: close\r\n\r\n");
    s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).map_err(|e| format!("{url}: {e}"))?;
    let split_at = raw.windows(4).position(|w| w == b"\r\n\r\n").ok_or_else(|| format!("{url}: malformed response"))?;
    let head = String::from_utf8_lossy(&raw[..split_at]).into_owned();
    let body = raw[split_at + 4..].to_vec();
    let status: u16 = head.split_whitespace().nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
    match (status, range) {
        (206, _) => Ok(body),
        (200, Some((o, l))) if body.len() as u64 >= o + l => Ok(body[o as usize..(o + l) as usize].to_vec()),
        (200, _) => Ok(body),
        _ => Err(format!("{url}: HTTP {status}")),
    }
}

/// `rel` against `base`, as a browser resolves it (RFC 3986, dot segments removed).
pub fn resolve(base: &str, rel: &str) -> String {
    if rel.contains("://") {
        return rel.to_string();
    }
    let Ok((_, addr, base_path)) = split(base) else { return rel.to_string() };
    let joined = if rel.starts_with('/') {
        rel.to_string()
    } else {
        let dir = &base_path[..base_path.rfind('/').map_or(0, |i| i + 1)];
        format!("{dir}{rel}")
    };
    let (path, query) = joined.split_once('?').map(|(p, q)| (p.to_string(), format!("?{q}"))).unwrap_or((joined.clone(), String::new()));
    let mut out: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            s => out.push(s),
        }
    }
    let trailing = if path.ends_with('/') && !out.is_empty() { "/" } else { "" };
    let host = addr.strip_suffix(":80").unwrap_or(&addr);
    format!("http://{host}/{}{trailing}{query}", out.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn relative_urls_resolve_like_a_browser() {
        let base = "http://127.0.0.1:8787/c/descent";
        assert_eq!(resolve(base, "../../assets/tiles/x.pmtiles"), "http://127.0.0.1:8787/assets/tiles/x.pmtiles");
        assert_eq!(resolve(base, "count.json"), "http://127.0.0.1:8787/c/count.json");
        assert_eq!(resolve(base, "/chunks/b3_ab"), "http://127.0.0.1:8787/chunks/b3_ab");
        assert_eq!(resolve("http://example.org/a/b", "c?x=1"), "http://example.org/a/c?x=1");
        assert!(get("https://example.org/x", None).unwrap_err().contains("plain http"));
    }

    #[test]
    fn ranges_are_asked_for_and_cut_when_ignored() {
        // A server that ignores Range and sends the whole body with 200.
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for mut s in l.incoming().flatten().take(2) {
                let mut req = String::new();
                let mut buf = [0u8; 256];
                while !req.contains("\r\n\r\n") {
                    let n = s.read(&mut buf).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    req.push_str(&String::from_utf8_lossy(&buf[..n]));
                }
                let body = if req.contains("Range: bytes=2-4") { "0123456789" } else { "whole" };
                let _ = write!(s, "HTTP/1.0 200 OK\r\nContent-Length: {}\r\n\r\n{body}", body.len());
            }
        });
        let url = format!("http://127.0.0.1:{port}/f");
        assert_eq!(get(&url, Some((2, 3))).unwrap(), b"234");
        assert_eq!(get(&url, None).unwrap(), b"whole");
    }
}
