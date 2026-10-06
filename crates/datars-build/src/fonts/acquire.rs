//! Acquiring fonts at build time: Google Fonts families and font URLs are downloaded once, cached
//! on disk, and shipped inside bundles — a runtime never calls Google or a font CDN. The network
//! sits behind [`Http`], so tests use fixtures; [`Curl`] is the real one (the system `curl`, so
//! no TLS stack is compiled into datars).
//!
//! Google Fonts come from the css2 API: asked with a non-browser user agent, it answers with
//! static TrueType instances (one file per weight and style), exactly what the engine draws.
//! No API key is needed.

use std::path::{Path, PathBuf};

/// HTTP GET, as the build step needs it.
pub trait Http {
    fn get(&self, url: &str) -> Result<Vec<u8>, String>;
}

/// The user agent font requests go out with. The css2 API serves TrueType to agents it doesn't
/// know as browsers.
pub const USER_AGENT: &str = "datars-font-fetch/1 (+https://github.com/rahlenjakob/datars)";

/// GET through the system `curl` (`-f`: HTTP errors are errors).
pub struct Curl;

impl Http for Curl {
    fn get(&self, url: &str) -> Result<Vec<u8>, String> {
        let out = std::process::Command::new("curl")
            .args(["-fsSL", "--max-time", "60", "--retry", "2", "-A", USER_AGENT, url])
            .output()
            .map_err(|e| format!("running curl (needed to download fonts): {e}"))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err(if err.is_empty() { format!("{url}: download failed") } else { format!("{url}: {err}") });
        }
        Ok(out.stdout)
    }
}

/// Where downloaded fonts are kept: `$DATARS_CACHE`, else `$XDG_CACHE_HOME/datars`, else the
/// platform's user cache directory (`~/Library/Caches/datars`, `%LOCALAPPDATA%\datars`,
/// `~/.cache/datars`). Fonts live under `fonts/` in it.
pub fn default_cache_dir() -> Option<PathBuf> {
    let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    if let Some(d) = env("DATARS_CACHE") {
        return Some(d);
    }
    if let Some(d) = env("XDG_CACHE_HOME") {
        return Some(d.join("datars"));
    }
    if cfg!(windows) {
        return env("LOCALAPPDATA").map(|d| d.join("datars"));
    }
    let home = env("HOME")?;
    Some(if cfg!(target_os = "macos") { home.join("Library/Caches/datars") } else { home.join(".cache/datars") })
}

/// Is this a font URL only the build step resolves (Google Fonts, or a remote file)?
pub fn is_remote(url: &str) -> bool {
    url.starts_with("google:") || url.starts_with("https://") || url.starts_with("http://")
}

/// Font acquisition with a disk cache.
pub struct Fonts {
    http: Box<dyn Http>,
    cache: Option<PathBuf>,
    offline: bool,
}

impl Fonts {
    /// `cache`: the cache root (fonts go under `fonts/`); `None` keeps nothing on disk.
    pub fn new(http: Box<dyn Http>, cache: Option<PathBuf>) -> Fonts {
        Fonts { http, cache, offline: false }
    }

    /// A remote data source (`https://…` CSV or JSON) as it is now, never cached: a published
    /// chart ships it as its snapshot (poster, first frame), and a live source refreshes it on
    /// the reader's device. None when offline.
    pub fn fetch_data(&self, url: &str) -> Result<Vec<u8>, String> {
        if self.offline {
            return Err("offline (DATARS_OFFLINE)".into());
        }
        self.http.get(url)
    }

    /// Curl and the default cache; `DATARS_OFFLINE=1` answers from the cache only.
    pub fn from_env() -> Fonts {
        let mut f = Fonts::new(Box::new(Curl), default_cache_dir());
        f.offline = std::env::var("DATARS_OFFLINE").is_ok_and(|v| !v.is_empty() && v != "0");
        f
    }

    /// Never touch the network: answer from the cache or fail.
    pub fn offline(mut self) -> Fonts {
        self.offline = true;
        self
    }

    /// The font file for `url` (`google:<Family>:<weight>[italic]` or `https://…`), as TrueType or
    /// OpenType bytes, from the cache when it has it.
    pub fn fetch(&self, url: &str) -> Result<Vec<u8>, String> {
        let (key, from) = if let Some((family, weight, italic)) = datars_theme::parse_google_url(url) {
            let file = format!("google/{}/{weight}{}.ttf", slug(&family), if italic { "i" } else { "" });
            (file, Source::Google { family, weight, italic })
        } else if url.starts_with("https://") || url.starts_with("http://") {
            (format!("url/{}.font", &datars_bundle::chunk_hash(url.as_bytes())[3..27]), Source::Url(url.to_string()))
        } else {
            return Err(format!("{url}: not a Google Fonts family (`google:…`) or a font URL"));
        };
        let path = self.cache.as_ref().map(|c| c.join("fonts").join(&key));
        if let Some(bytes) = path.as_ref().and_then(|p| std::fs::read(p).ok()) {
            return Ok(bytes);
        }
        if self.offline {
            return Err(format!("{url}: not in the font cache and DATARS_OFFLINE is set"));
        }
        let bytes = match from {
            Source::Google { family, weight, italic } => self.google(&family, weight, italic)?,
            Source::Url(u) => decode_container(&self.http.get(&u)?).map_err(|e| format!("{u}: {e}"))?,
        };
        read_fonts::FontRef::new(&bytes).map_err(|e| format!("{url}: not a usable font ({e})"))?;
        if let Some(p) = &path {
            // Best effort: a read-only cache still builds, it just downloads again next time.
            let _ = write_atomic(p, &bytes);
        }
        Ok(bytes)
    }

    fn google(&self, family: &str, weight: u16, italic: bool) -> Result<Vec<u8>, String> {
        let css_url = google_css_url(family, weight, italic);
        let css = self.http.get(&css_url).map_err(|e| format!("Google Fonts has no {family} {weight}{}: {e}", if italic { " italic" } else { "" }))?;
        let css = String::from_utf8_lossy(&css);
        let src = font_url_in_css(&css).ok_or_else(|| format!("Google Fonts answered for {family} without a TrueType file ({css_url})"))?;
        self.http.get(&src)
    }
}

enum Source {
    Google { family: String, weight: u16, italic: bool },
    Url(String),
}

/// Is this request for a font only the build step can acquire — a Google Fonts family, or a
/// remote font file (a theme face's `font:` request, or a font source by URL)?
pub fn is_remote_font(req: &datars_engine::Request) -> bool {
    let datars_engine::Request::Source { name, url } = req else { return false };
    let font_file = [".ttf", ".otf", ".woff", ".woff2", ".ttc"].iter().any(|e| url.to_ascii_lowercase().split('?').next().unwrap_or("").ends_with(e));
    url.starts_with("google:") || (is_remote(url) && (name.starts_with("font:") || font_file))
}

/// What the build tools fetch for a document in `dir`: remote fonts through `fonts` (cache,
/// then network), everything else from disk ([`datars_headless::fetch_from_disk`]). Failures to
/// acquire a font are printed; the engine then diagnoses the fallback.
pub fn fetch_with<'a>(fonts: &'a Fonts, dir: Option<&'a Path>) -> impl Fn(&datars_engine::Request) -> Option<Vec<u8>> + 'a {
    move |req| {
        if is_remote_font(req) {
            let datars_engine::Request::Source { url, .. } = req else { return None };
            return match fonts.fetch(url) {
                Ok(b) => Some(b),
                Err(e) => {
                    eprintln!("warning: font {url}: {e}");
                    None
                }
            };
        }
        if let datars_engine::Request::Source { name, url } = req {
            if url.starts_with("https://") || url.starts_with("http://") {
                return match fonts.fetch_data(url) {
                    Ok(b) => Some(b),
                    Err(e) => {
                        eprintln!("warning: data `{name}`: {url}: {e}");
                        None
                    }
                };
            }
        }
        let bytes = datars_headless::fetch_from_disk(req, dir)?;
        // Font files next to the document may be WOFF.
        match req {
            datars_engine::Request::Source { name, .. } if name.starts_with("font:") => decode_container(&bytes).map_err(|e| eprintln!("warning: {name}: {e}")).ok(),
            _ => Some(bytes),
        }
    }
}

/// The css2 API request for one static instance.
pub fn google_css_url(family: &str, weight: u16, italic: bool) -> String {
    format!("https://fonts.googleapis.com/css2?family={}:ital,wght@{},{weight}", family.trim().replace(' ', "+"), italic as u8)
}

/// The first TrueType/OpenType `src: url(…)` in a css2 answer.
pub fn font_url_in_css(css: &str) -> Option<String> {
    css.split("url(").skip(1).find_map(|rest| {
        let (u, after) = rest.split_once(')')?;
        let u = u.trim().trim_matches(|c| c == '\'' || c == '"');
        let fmt_ok = after.trim_start().starts_with("format('truetype')") || after.trim_start().starts_with("format('opentype')") || u.ends_with(".ttf") || u.ends_with(".otf");
        (fmt_ok && u.starts_with("https://")).then(|| u.to_string())
    })
}

/// `google:Source Serif 4` → `source-serif-4` (the cache directory).
fn slug(family: &str) -> String {
    family.trim().to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// TrueType/OpenType bytes from a font file: sfnt as it is, WOFF unwrapped (zlib per table).
/// WOFF2 needs its glyph transforms undone, which isn't implemented: ship the TTF/OTF instead.
pub fn decode_container(bytes: &[u8]) -> Result<Vec<u8>, String> {
    match bytes.get(..4) {
        Some(b"wOFF") => unwoff(bytes),
        Some(b"wOF2") => Err("WOFF2 files aren't supported yet — use the font's .ttf or .otf".into()),
        _ => Ok(bytes.to_vec()),
    }
}

fn unwoff(b: &[u8]) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let be16 = |o: usize| b.get(o..o + 2).map(|s| u16::from_be_bytes([s[0], s[1]]));
    let be32 = |o: usize| b.get(o..o + 4).map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]));
    let n = be16(12).ok_or("short WOFF header")? as usize;
    let mut builder = write_fonts::FontBuilder::new();
    for i in 0..n {
        let e = 44 + i * 20;
        let (tag, off, comp, orig) = (be32(e).ok_or("short WOFF directory")?, be32(e + 4).ok_or("bad WOFF")? as usize, be32(e + 8).ok_or("bad WOFF")? as usize, be32(e + 12).ok_or("bad WOFF")? as usize);
        let data = b.get(off..off + comp).ok_or("WOFF table out of bounds")?;
        let table = if comp < orig {
            let mut out = Vec::with_capacity(orig);
            flate2::read::ZlibDecoder::new(data).read_to_end(&mut out).map_err(|e| format!("WOFF table: {e}"))?;
            out
        } else {
            data.to_vec()
        };
        builder.add_raw(read_fonts::types::Tag::from_be_bytes(tag.to_be_bytes()), table);
    }
    Ok(builder.build())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::rc::Rc;

    /// Answers from a map and counts requests.
    struct Fixture {
        files: BTreeMap<String, Vec<u8>>,
        calls: Rc<RefCell<Vec<String>>>,
    }

    impl Http for Fixture {
        fn get(&self, url: &str) -> Result<Vec<u8>, String> {
            self.calls.borrow_mut().push(url.to_string());
            self.files.get(url).cloned().ok_or_else(|| format!("{url}: 404"))
        }
    }

    const FONT: &[u8] = include_bytes!("../../../../assets/fonts/NotoSansHebrew-Regular.ttf");

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("datars-fonts-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn google_fonts_come_from_css2_then_the_cache() {
        let css_url = google_css_url("Source Serif 4", 600, true);
        assert_eq!(css_url, "https://fonts.googleapis.com/css2?family=Source+Serif+4:ital,wght@1,600");
        let css = "@font-face {\n  font-family: 'Source Serif 4';\n  src: url(https://fonts.gstatic.com/s/x/v1/abc.ttf) format('truetype');\n}\n";
        let calls = Rc::new(RefCell::new(Vec::new()));
        let files = BTreeMap::from([(css_url.clone(), css.as_bytes().to_vec()), ("https://fonts.gstatic.com/s/x/v1/abc.ttf".to_string(), FONT.to_vec())]);
        let dir = temp("google");
        let fonts = Fonts::new(Box::new(Fixture { files, calls: calls.clone() }), Some(dir.clone()));
        assert_eq!(fonts.fetch("google:Source Serif 4:600italic").unwrap(), FONT);
        assert_eq!(calls.borrow().len(), 2, "css, then the font file");
        assert!(dir.join("fonts/google/source-serif-4/600i.ttf").is_file());
        // Second time: the cache, no network.
        assert_eq!(fonts.fetch("google:Source Serif 4:600italic").unwrap(), FONT);
        assert_eq!(calls.borrow().len(), 2);
        // Offline with a warm cache works; a cold one says why.
        let offline = Fonts::new(Box::new(Fixture { files: BTreeMap::new(), calls: calls.clone() }), Some(dir.clone())).offline();
        assert_eq!(offline.fetch("google:Source Serif 4:600italic").unwrap(), FONT);
        assert!(offline.fetch("google:Source Serif 4:700").unwrap_err().contains("DATARS_OFFLINE"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_families_and_broken_answers_are_errors() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let fonts = Fonts::new(Box::new(Fixture { files: BTreeMap::new(), calls }), None);
        let e = fonts.fetch("google:No Such Family:400").unwrap_err();
        assert!(e.contains("Google Fonts has no No Such Family 400"), "{e}");
        let css_url = google_css_url("Odd", 400, false);
        let calls = Rc::new(RefCell::new(Vec::new()));
        let fonts = Fonts::new(Box::new(Fixture { files: BTreeMap::from([(css_url, b"@font-face { src: url(https://x/y.woff2) format('woff2'); }".to_vec())]), calls }), None);
        assert!(fonts.fetch("google:Odd:400").unwrap_err().contains("without a TrueType file"));
        assert!(fonts.fetch("fonts/local.ttf").is_err(), "local files are the host's (fetch_from_disk)");
    }

    /// The build tools send remote fonts through the cache and everything else to the disk.
    #[test]
    fn remote_fonts_go_through_the_cache_and_the_rest_to_disk() {
        use datars_engine::Request;
        let src = |name: &str, url: &str| Request::Source { name: name.into(), url: url.into() };
        assert!(is_remote_font(&src("font:X-400", "google:X:400")));
        assert!(is_remote_font(&src("font:X-400", "https://cdn.example.com/x")));
        assert!(is_remote_font(&src("hebrew", "https://cdn.example.com/NotoSansHebrew.ttf")));
        assert!(!is_remote_font(&src("prices", "https://data.example.com/prices.csv")), "data stays a runtime fetch");
        assert!(!is_remote_font(&src("font:X-400", "fonts/x.ttf")));
        let css_url = google_css_url("X", 400, false);
        let calls = Rc::new(RefCell::new(Vec::new()));
        let files = BTreeMap::from([(css_url, b"src: url(https://g/x.ttf) format('truetype');".to_vec()), ("https://g/x.ttf".to_string(), FONT.to_vec())]);
        let fonts = Fonts::new(Box::new(Fixture { files, calls: calls.clone() }), None);
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/hebrew");
        let fetch = fetch_with(&fonts, Some(&dir));
        assert_eq!(fetch(&src("font:X-400", "google:X:400")).unwrap(), FONT);
        assert_eq!(fetch(&src("hebrew", "../../assets/fonts/NotoSansHebrew-Regular.ttf")).unwrap(), FONT, "from disk");
        assert!(fetch(&src("font:Inter-400", "datars:fonts/Inter-Regular.ttf")).is_some(), "the default fonts");
        assert_eq!(calls.borrow().len(), 2);
    }

    #[test]
    fn font_urls_are_downloaded_once_and_woff_is_unwrapped() {
        // A WOFF 1.0 wrapping of the test font, built here: every table zlib-compressed.
        let woff = to_woff(FONT);
        let calls = Rc::new(RefCell::new(Vec::new()));
        let dir = temp("url");
        let url = "https://cdn.example.com/Brand-Regular.woff";
        let fonts = Fonts::new(Box::new(Fixture { files: BTreeMap::from([(url.to_string(), woff)]), calls: calls.clone() }), Some(dir.clone()));
        let bytes = fonts.fetch(url).unwrap();
        let f = read_fonts::FontRef::new(&bytes).unwrap();
        let orig = read_fonts::FontRef::new(FONT).unwrap();
        for t in orig.table_directory().table_records() {
            let bytes = |font: &read_fonts::FontRef| {
                let mut b = font.table_data(t.tag()).map(|d| d.as_bytes().to_vec()).unwrap_or_default();
                if t.tag() == read_fonts::types::Tag::new(b"head") {
                    b[8..12].fill(0); // the whole-file checksum adjustment, recomputed
                }
                b
            };
            assert_eq!(bytes(&f), bytes(&orig), "{}", t.tag());
        }
        assert_eq!(fonts.fetch(url).unwrap(), bytes);
        assert_eq!(calls.borrow().len(), 1);
        assert!(decode_container(b"wOF2....").unwrap_err().contains("WOFF2"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn to_woff(sfnt: &[u8]) -> Vec<u8> {
        use std::io::Write;
        let f = read_fonts::FontRef::new(sfnt).unwrap();
        let records: Vec<_> = f.table_directory().table_records().to_vec();
        let mut dir = Vec::new();
        let mut body = Vec::new();
        let base = 44 + records.len() * 20;
        for r in &records {
            let data = f.table_data(r.tag()).unwrap().as_bytes().to_vec();
            let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
            z.write_all(&data).unwrap();
            let comp = z.finish().unwrap();
            let (stored, comp_len) = if comp.len() < data.len() { (comp.clone(), comp.len()) } else { (data.clone(), data.len()) };
            dir.extend_from_slice(&r.tag().to_be_bytes());
            dir.extend_from_slice(&((base + body.len()) as u32).to_be_bytes());
            dir.extend_from_slice(&(comp_len as u32).to_be_bytes());
            dir.extend_from_slice(&(data.len() as u32).to_be_bytes());
            dir.extend_from_slice(&r.checksum().to_be_bytes());
            body.extend_from_slice(&stored);
            while body.len() % 4 != 0 {
                body.push(0);
            }
        }
        let mut out = b"wOFF".to_vec();
        out.extend_from_slice(&0x0001_0000u32.to_be_bytes());
        out.extend_from_slice(&((base + body.len()) as u32).to_be_bytes());
        out.extend_from_slice(&(records.len() as u16).to_be_bytes());
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(&(sfnt.len() as u32).to_be_bytes());
        out.extend_from_slice(&[0; 24]);
        out.extend_from_slice(&dir);
        out.extend_from_slice(&body);
        out
    }
}
