//! Icons and pictures a document draws (docs/04-primitives.md §Icons, §Pictures).
//!
//! The engine never fetches (P10). An icon or picture a resolve needs and doesn't have becomes a
//! request — `icon:<key>` for an icon (its URL `datars:icons/lucide/heart.svg`, a project file, an
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
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// The request-name prefixes (`Request::Source { name }`).
pub const ICON_PREFIX: &str = "icon:";
pub const IMAGE_PREFIX: &str = "image:";

#[derive(Default)]
pub struct Assets {
    /// Icons by key (`lucide:heart`, `logo.svg`).
    pub icons: BTreeMap<String, Arc<Icon>>,
    /// Pictures by source (`photo.png`).
    pub images: BTreeMap<String, Arc<Raster>>,
    /// Why an icon or picture couldn't be read (a 404, a script in the file): said once, and not
    /// asked for again.
    pub failed: BTreeMap<String, String>,
    /// Asked for and not here yet: request name → URL.
    pub pending: BTreeMap<String, String>,
    /// Inline icons parsed so far (markup in the document), by key.
    pub inline: RefCell<BTreeMap<String, Arc<Icon>>>,
    /// Icons and pictures resolves drew (request names): what the build ships.
    pub used: RefCell<BTreeSet<String>>,
    /// Opened from a bundle: it carries everything its chart draws, so nothing is asked for (an
    /// icon it lacks draws nothing, and says so).
    pub sealed: bool,
}

impl Assets {
    /// Forget the document's assets (a new document), keeping what was fetched by URL: a reload
    /// that names the same files gets them at once.
    pub fn reset(&mut self) {
        self.pending.clear();
        self.failed.clear();
        self.inline.borrow_mut().clear();
        self.used.borrow_mut().clear();
        self.sealed = false;
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

/// What a resolve found it needs: request name → URL.
pub type Asks = RefCell<BTreeMap<String, String>>;

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
pub fn literal_sources(t: &datars_ir::Template, theme: &datars_theme::ResolvedTheme, out: &mut BTreeMap<String, String>) {
    use datars_ir::{TGeom, TKind};
    let mut icon = |p: &datars_ir::Prop| {
        if let Some(s) = p.as_str() {
            let s = s.strip_prefix('$').and_then(|tok| theme.text(tok)).unwrap_or(s);
            if let Some((name, url)) = Source::parse(s).as_ref().and_then(icon_request) {
                out.entry(name).or_insert(url);
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
                out.entry(n).or_insert(u);
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
