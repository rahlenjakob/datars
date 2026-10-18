//! Icons and pictures a document draws (docs/04-primitives.md §Icons, §Pictures).
//!
//! The engine never fetches (P10). An icon or picture a resolve needs and doesn't have becomes a
//! request — `icon:<key>` for an icon (its URL `datars:icons/lucide/heart.json`, a project file, an
//! `https://` URL), `image:<src>` for a picture — that the host answers with
//! [`crate::Engine::provide`], like any data source. Tools and the build step answer them on the
//! spot ([`crate::Engine::set_asset_fetch`]); a published bundle carries every icon and picture
//! its chart draws ([`crate::Engine::add_icons`]), so a runtime never asks.
//!
//! Until it arrives an icon draws nothing — no stand-in that then jumps — and the scene resolves
//! again when it does.

use datars_icon::{Icon, Source};
use datars_render::Raster;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::sync::Arc;

/// The request-name prefixes (`Request::Source { name }`).
pub const ICON_PREFIX: &str = "icon:";
pub const IMAGE_PREFIX: &str = "image:";

/// One icon or picture, by its request name: here, asked for, waiting for the host's decoder, or
/// failed.
#[derive(Clone)]
pub enum Asset {
    /// Asked for and not here yet: its URL.
    Pending(String),
    /// Couldn't be read (a 404, a script in the file, a decoder that refused it), and why: said
    /// once, and not asked for again.
    Failed(String),
    /// A picture's file, waiting for the host's decoder (`Request::Decode`).
    Undecoded(Arc<[u8]>),
    Icon(Arc<Icon>),
    Image(Arc<Raster>),
}

/// Assets by request name, sorted: a list, not a B-tree — a document names a handful of icons and
/// pictures, and in the web runtime every map type is code of its own (a B-tree's is ~11 KB, a
/// sorted list's a few hundred bytes).
#[derive(Clone, Default)]
pub struct AssetMap(Vec<(String, Asset)>);

impl AssetMap {
    fn at(&self, k: &str) -> Result<usize, usize> {
        self.0.binary_search_by(|(n, _)| n.as_str().cmp(k))
    }
    pub fn get(&self, k: &str) -> Option<&Asset> {
        self.at(k).ok().map(|i| &self.0[i].1)
    }
    pub fn contains_key(&self, k: &str) -> bool {
        self.at(k).is_ok()
    }
    /// Set `k` (replacing what was there).
    pub fn insert(&mut self, k: String, v: Asset) {
        match self.at(&k) {
            Ok(i) => self.0[i].1 = v,
            Err(i) => self.0.insert(i, (k, v)),
        }
    }
    /// Set `k` unless it's there already.
    pub fn ask(&mut self, k: String, v: Asset) {
        if let Err(i) = self.at(&k) {
            self.0.insert(i, (k, v));
        }
    }
    pub fn retain(&mut self, mut keep: impl FnMut(&String, &Asset) -> bool) {
        self.0.retain(|(k, v)| keep(k, v));
    }
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Asset)> {
        self.0.iter().map(|(k, v)| (k, v))
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn clear(&mut self) {
        self.0.clear();
    }
}

impl IntoIterator for AssetMap {
    type Item = (String, Asset);
    type IntoIter = std::vec::IntoIter<(String, Asset)>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

/// The document's icons and pictures. One map for every kind and stage, by request name
/// (`icon:<key>`, `image:<src>`).
#[derive(Default)]
pub struct Assets {
    pub all: AssetMap,
    /// Inline icons parsed so far (markup in the document), by key.
    pub inline: RefCell<AssetMap>,
    /// Icons and pictures resolves drew (request names): what the build ships.
    pub used: RefCell<BTreeSet<String>>,
    /// Opened from a bundle: it carries everything its chart draws, so nothing is asked for (an
    /// icon it lacks draws nothing, and says so).
    pub sealed: bool,
}

/// An icon's request name.
pub fn icon_name(key: &str) -> String {
    format!("{ICON_PREFIX}{key}")
}

impl Assets {
    /// Forget the document's assets (a new document), keeping what was fetched by URL: a reload
    /// that names the same files gets them at once.
    pub fn reset(&mut self) {
        self.all.retain(|_, a| matches!(a, Asset::Icon(_) | Asset::Image(_)));
        self.inline.borrow_mut().clear();
        self.used.borrow_mut().clear();
        self.sealed = false;
    }

    /// The icon of `key` (`lucide:heart`, `logo.svg`), if it's here.
    pub fn icon(&self, key: &str) -> Option<&Arc<Icon>> {
        match self.all.get(&icon_name(key)) {
            Some(Asset::Icon(i)) => Some(i),
            _ => None,
        }
    }

    /// The picture of `src`, if it's here (decoded).
    pub fn image(&self, src: &str) -> Option<&Arc<Raster>> {
        match self.all.get(&format!("{IMAGE_PREFIX}{src}")) {
            Some(Asset::Image(r)) => Some(r),
            _ => None,
        }
    }

    /// Why a request failed, if it did.
    pub fn failed(&self, name: &str) -> Option<&str> {
        match self.all.get(name) {
            Some(Asset::Failed(why)) => Some(why),
            _ => None,
        }
    }

    /// Requests not answered yet: name → URL.
    pub fn pending(&self) -> impl Iterator<Item = (&String, &String)> {
        self.all.iter().filter_map(|(n, a)| if let Asset::Pending(u) = a { Some((n, u)) } else { None })
    }

    /// Pictures waiting for the host's decoder: name → file.
    pub fn undecoded(&self) -> impl Iterator<Item = (&String, &Arc<[u8]>)> {
        self.all.iter().filter_map(|(n, a)| if let Asset::Undecoded(f) = a { Some((n, f)) } else { None })
    }

    /// Take the pending requests out (to answer them now).
    pub fn take_pending(&mut self) -> AssetMap {
        let mut out = AssetMap::default();
        self.all.retain(|n, a| match a {
            Asset::Pending(_) => {
                out.insert(n.clone(), a.clone());
                false
            }
            _ => true,
        });
        out
    }

    pub fn has_images(&self) -> bool {
        self.all.iter().any(|(_, a)| matches!(a, Asset::Image(_)))
    }
}

/// An icon a resolve drew: where (its node's key path), what it stands for (`None`: decorative)
/// and its ink — what the non-text contrast check (WCAG 1.4.11) reads.
#[derive(Clone, Debug, PartialEq)]
pub struct IconMark {
    pub path: String,
    pub label: Option<String>,
    pub ink: datars_theme::Ink,
}

/// What a resolve found it needs: request name → `Asset::Pending(url)`.
pub type Asks = RefCell<AssetMap>;

/// The request name and URL for an icon source.
pub fn icon_request(src: &Source) -> Option<(String, String)> {
    Some((format!("{ICON_PREFIX}{}", src.key()), src.url()?))
}

/// The request name for a picture.
pub fn image_request(src: &str) -> (String, String) {
    (format!("{IMAGE_PREFIX}{src}"), src.to_string())
}

/// Literal icon and picture sources a document names in its own template (no recipe needed to
/// find them): asked for as soon as it loads, so a host's first round of requests has them.
pub fn literal_sources(t: &datars_ir::Template, theme: &datars_theme::ResolvedTheme, out: &mut AssetMap) {
    use datars_ir::{TGeom, TKind};
    let mut icon = |p: &datars_ir::Prop| {
        if let Some(s) = p.as_str() {
            let s = s.strip_prefix('$').and_then(|tok| theme.text(tok)).unwrap_or(s);
            if let Some((name, url)) = Source::parse(s).as_ref().and_then(icon_request) {
                out.ask(name, Asset::Pending(url));
            }
        }
    };
    match &t.kind {
        TKind::Icon(ti) => icon(&ti.icon),
        TKind::Instances(ti) if ti.proto == "icon" => icon(&ti.icon),
        TKind::Shape { geom: TGeom::Symbol { symbol, .. }, .. } if symbol.as_str().is_some_and(is_icon_symbol) => icon(symbol),
        TKind::Shape { markers: Some(m), .. } => {
            for end in ["start", "end"] {
                if let Some(i) = m.get(end).filter(|v| v["type"] == "icon").and_then(|v| v.get("icon")) {
                    icon(&datars_ir::Prop(i.clone()));
                }
            }
        }
        TKind::Image { asset, .. } => {
            if let Some(s) = asset.as_str().filter(|s| !s.is_empty()) {
                let (n, u) = image_request(s);
                out.ask(n, Asset::Pending(u));
            }
        }
        _ => {}
    }
    match &t.kind {
        TKind::Group { children } | TKind::View { children, .. } => children.iter().for_each(|c| literal_sources(c, theme, out)),
        TKind::Repeat { template, .. } => literal_sources(template, theme, out),
        _ => {}
    }
}

/// The built-in symbol names (`geom.symbol`); any other symbol that names an icon draws the icon.
pub const SYMBOLS: &[&str] = &["circle", "square", "diamond", "triangle", "cross", "star"];

/// Does a symbol name an icon rather than a built-in symbol? Built-in names stay symbols
/// (`star`); `lucide:star`, `$icon.…`, files and markup are icons. A bare name that isn't a
/// built-in one stays what it always drew (a circle) unless it says which set it's from.
pub fn is_icon_symbol(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() || SYMBOLS.contains(&s) {
        return false;
    }
    s.starts_with("$icon.") || s.contains(':') || s.starts_with('<') || matches!(Source::parse(s), Some(Source::File(_)))
}
