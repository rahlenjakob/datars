//! Icon sources: how a name says where an icon comes from, and the default set.
//!
//! | name | source | fetched by |
//! |---|---|---|
//! | `heart`, `lucide:heart` | the default set (Lucide) | the build step / tools, from `datars:icons/lucide/heart.json`; a raw document in the web runtime, from `icons/lucide/heart.json` beside the runtime |
//! | `icons/logo.svg` | a project file, relative to the document | like data URLs |
//! | `https://example.com/logo.svg` | a URL | the build step, once (bundles carry it) |
//! | `<svg …>…</svg>` | inline markup | nobody: it's in the document |
//!
//! (Theme icon tokens, `$icon.up`, name one of these; the engine looks them up first.)

use serde_json::Value;
use std::collections::BTreeMap;

/// The default icon set's name.
pub const DEFAULT_SET: &str = "lucide";

/// Where an icon name says the icon comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// An icon of a named set (`lucide:heart`; a bare `heart` is the default set's).
    Set { set: String, name: String },
    /// A file (relative to the document) or an `https://` URL.
    File(String),
    /// SVG markup in the document itself.
    Inline(String),
}

impl Source {
    pub fn parse(s: &str) -> Option<Source> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        if s.starts_with('<') {
            return Some(Source::Inline(s.to_string()));
        }
        if s.contains("://") || s.contains('/') || s.contains('\\') || s.to_ascii_lowercase().ends_with(".svg") {
            return Some(Source::File(s.to_string()));
        }
        match s.split_once(':') {
            Some((set, name)) if !set.is_empty() && !name.is_empty() && valid_name(name) && valid_name(set) => Some(Source::Set { set: set.to_string(), name: name.to_string() }),
            None if valid_name(s) => Some(Source::Set { set: DEFAULT_SET.to_string(), name: s.to_string() }),
            _ => None,
        }
    }

    /// The URL a host fetches for it (`None`: nothing to fetch).
    pub fn url(&self) -> Option<String> {
        match self {
            Source::Set { set, name } => Some(format!("datars:icons/{set}/{name}.json")),
            Source::File(f) => Some(f.clone()),
            Source::Inline(_) => None,
        }
    }

    /// A canonical key: `lucide:heart` for `heart` and `lucide:heart` alike.
    pub fn key(&self) -> String {
        match self {
            Source::Set { set, name } => format!("{set}:{name}"),
            Source::File(f) => f.clone(),
            Source::Inline(s) => format!("inline:{:016x}", fnv(s.as_bytes())),
        }
    }
}

fn valid_name(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn fnv(b: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &x in b {
        h ^= x as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// A packed icon set (`assets/icons/<set>/icons.json`, written by `scripts/vendor-icons.mjs`):
/// one shared root element and each icon's elements.
#[derive(Clone, Debug)]
pub struct Set {
    pub name: String,
    pub version: String,
    /// The set's licence (SPDX), recorded in every bundle that ships one of its icons.
    pub licence: String,
    pub source: String,
    root: String,
    icons: BTreeMap<String, String>,
    tags: BTreeMap<String, Vec<String>>,
}

impl Set {
    pub fn from_json(s: &str) -> Result<Set, String> {
        let v: Value = serde_json::from_str(s).map_err(|e| e.to_string())?;
        let text = |k: &str| v[k].as_str().unwrap_or("").to_string();
        let icons = v["icons"].as_object().ok_or("an icon set without `icons`")?.iter().filter_map(|(k, x)| Some((k.clone(), x.as_str()?.to_string()))).collect();
        let tags = v["tags"].as_object().map(|o| o.iter().map(|(k, x)| (k.clone(), x.as_array().map(|a| a.iter().filter_map(|t| t.as_str().map(String::from)).collect()).unwrap_or_default())).collect()).unwrap_or_default();
        Ok(Set { name: text("set"), version: text("version"), licence: text("licence"), source: text("source"), root: text("root"), icons, tags })
    }

    /// The icon as a whole SVG file, if the set has it.
    pub fn svg(&self, name: &str) -> Option<String> {
        self.icons.get(name).map(|inner| format!("{}{}</svg>", self.root, inner))
    }

    /// An icon in its compact form ([`crate::Icon::to_json`]): the files the web runtime fetches
    /// for raw documents (`datars:icons/<set>/<name>.json`), read once here by the tools.
    #[cfg(feature = "svg")]
    pub fn compact(&self, name: &str) -> Option<Result<String, String>> {
        self.svg(name).map(|s| crate::parse_svg(&s).map(|i| i.to_json()))
    }

    /// An icon's tags (what it shows, in words): the set's own, empty when it has none.
    pub fn tags(&self, name: &str) -> &[String] {
        self.tags.get(name).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn len(&self) -> usize {
        self.icons.len()
    }
    pub fn is_empty(&self) -> bool {
        self.icons.is_empty()
    }
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.icons.keys().map(String::as_str)
    }

    /// Icons whose name or tags contain every word of `query`, best first (name matches, then
    /// shorter names) — for `datars icons <query>` and agents choosing an icon.
    pub fn search(&self, query: &str) -> Vec<&str> {
        let words: Vec<String> = query.split_whitespace().map(|w| w.to_ascii_lowercase()).collect();
        let mut hits: Vec<(u8, usize, &str)> = self
            .icons
            .keys()
            .filter_map(|n| {
                let tags = self.tags.get(n).map(Vec::as_slice).unwrap_or(&[]);
                let mut by_name = true;
                for w in &words {
                    if n.contains(w.as_str()) {
                        continue;
                    }
                    by_name = false;
                    if !tags.iter().any(|t| t.to_ascii_lowercase().contains(w.as_str())) {
                        return None;
                    }
                }
                let exact = words.len() == 1 && n == &words[0];
                Some((if exact { 0 } else if by_name { 1 } else { 2 }, n.len(), n.as_str()))
            })
            .collect();
        hits.sort();
        hits.into_iter().map(|h| h.2).collect()
    }
}

/// The default set compiled in (tools and native hosts: `bundled` feature), parsed once.
#[cfg(feature = "bundled")]
pub fn bundled(set: &str) -> Option<&'static Set> {
    static LUCIDE: std::sync::OnceLock<Option<Set>> = std::sync::OnceLock::new();
    match set {
        DEFAULT_SET => LUCIDE.get_or_init(|| Set::from_json(include_str!("../../../assets/icons/lucide/icons.json")).ok()).as_ref(),
        _ => None,
    }
}

/// The SVG for a `datars:icons/<set>/<name>.json` URL (or `….svg`) from the compiled-in sets:
/// what tools answer the request with (they read SVG).
#[cfg(feature = "bundled")]
pub fn bundled_svg(url: &str) -> Option<String> {
    let rest = url.strip_prefix("datars:icons/")?;
    let (set, file) = rest.split_once('/')?;
    let name = file.strip_suffix(".json").or_else(|| file.strip_suffix(".svg"))?;
    bundled(set)?.svg(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_say_where_icons_come_from() {
        assert_eq!(Source::parse("heart"), Some(Source::Set { set: "lucide".into(), name: "heart".into() }));
        assert_eq!(Source::parse("lucide:arrow-up"), Some(Source::Set { set: "lucide".into(), name: "arrow-up".into() }));
        assert_eq!(Source::parse("icons/logo.svg"), Some(Source::File("icons/logo.svg".into())));
        assert_eq!(Source::parse("logo.SVG"), Some(Source::File("logo.SVG".into())));
        assert_eq!(Source::parse("https://example.com/a.svg"), Some(Source::File("https://example.com/a.svg".into())));
        assert!(matches!(Source::parse("<svg/>"), Some(Source::Inline(_))));
        assert_eq!(Source::parse("not an icon"), None);
        assert_eq!(Source::parse("heart").unwrap().url().as_deref(), Some("datars:icons/lucide/heart.json"));
        assert_eq!(Source::parse("heart").unwrap().key(), Source::parse("lucide:heart").unwrap().key());
    }

    #[test]
    fn a_packed_set_gives_whole_files() {
        let s = Set::from_json(r#"{"set":"t","version":"1","licence":"ISC","root":"<svg viewBox=\"0 0 24 24\">","icons":{"dot":"<circle cx=\"12\" cy=\"12\" r=\"4\"/>","dots":"<circle r=\"1\"/>"},"tags":{"dot":["point","circle"]}}"#).unwrap();
        assert_eq!(s.svg("dot").as_deref(), Some("<svg viewBox=\"0 0 24 24\"><circle cx=\"12\" cy=\"12\" r=\"4\"/></svg>"));
        assert_eq!(s.search("dot"), vec!["dot", "dots"]);
        assert_eq!(s.search("point"), vec!["dot"]);
        #[cfg(feature = "svg")]
        {
            let c = s.compact("dot").unwrap().unwrap();
            assert_eq!(crate::read_icon(c.as_bytes()).unwrap(), crate::parse_svg(&s.svg("dot").unwrap()).unwrap());
        }
    }
}
