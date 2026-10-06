//! Fonts in bundles (docs/12-delivery.md §Fonts): every face a chart draws with travels with it,
//! cut down to the characters the chart can show — text must measure and render the same on
//! every runtime (P1), so no runtime supplies fonts of its own.
//!
//! - [`acquire`]: Google Fonts and font URLs, downloaded at build time and cached on disk.
//! - [`coverage`]: the characters a chart can show (its text, its data, its number formats).
//! - [`subset`]: TrueType subsetting that keeps glyph ids, so subsets shape exactly like the font.
//! - [`scripts`]: per-script subsets, loaded lazily, for text that only arrives at runtime.

pub mod acquire;
pub mod coverage;
pub mod scripts;
pub mod subset;

use datars_bundle::{Builder, FontCredit};
use read_fonts::{FontRef, TableProvider};
use std::collections::{BTreeMap, BTreeSet};

/// A face to ship: the font it comes from and how the runtime should register it.
#[derive(Clone, Debug)]
pub struct Face {
    /// The engine's face id (`"Inter-600"`).
    pub id: String,
    pub family: String,
    pub weight: u16,
    pub italic: bool,
    /// OS/2 `fsType` embedding bits.
    pub fs_type: u16,
    /// The whole font file and the face's index in it.
    pub bytes: Vec<u8>,
    pub index: u32,
    /// Where the build step got it (for the credits).
    pub from: String,
    /// A document font source (`data.x = { font }`): the runtime provides it under this name and
    /// its family becomes a fallback for every text. `None` for theme faces.
    pub source: Option<String>,
    /// The characters this face must cover ([`coverage`]).
    pub chars: BTreeSet<char>,
}

/// What [`pack`] added to the bundle.
#[derive(Debug, Default)]
pub struct Packed {
    /// Font chunks every playable variant loads up front.
    pub eager: Vec<String>,
    /// Script subsets fetched only when runtime text needs them (T2/T3).
    pub lazy: Vec<String>,
    pub credits: Vec<FontCredit>,
    pub decisions: Vec<String>,
}

/// Ship `faces` as chunks: each subset to its characters, plus — when the chart's text can arrive
/// at runtime (`runtime_text`) — one lazily loaded subset per script the font covers.
pub fn pack(faces: &[Face], runtime_text: bool, b: &mut Builder) -> Packed {
    let mut out = Packed::default();
    for f in faces {
        let chars = &f.chars;
        let lic = licence(&f.bytes, f.index);
        let label = format!("font `{}` ({}{} {}, {})", f.id, f.family, if f.italic { " italic" } else { "" }, f.weight, lic.id);
        if f.fs_type & datars_text::EMBED_RESTRICTED != 0 {
            out.decisions.push(format!("warning: {label}: its OS/2 fsType says restricted-licence embedding — make sure its licence lets you ship it in bundles"));
        }
        if lic.id == "unknown" {
            out.decisions.push(format!("warning: {label}: no OFL/Apache notice in its name table — make sure its licence lets you embed it"));
        }
        out.credits.push(FontCredit { family: f.family.clone(), weight: f.weight, italic: f.italic, licence: lic.id.clone(), licence_url: lic.url.clone(), copyright: lic.copyright.clone(), from: f.from.clone() });
        let base_meta = |subset: &str| {
            let mut m: BTreeMap<String, serde_json::Value> = BTreeMap::new();
            m.insert("face".into(), f.id.clone().into());
            m.insert("family".into(), f.family.clone().into());
            m.insert("weight".into(), f.weight.into());
            m.insert("italic".into(), f.italic.into());
            m.insert("licence".into(), lic.id.clone().into());
            m.insert("subset".into(), subset.into());
            if let Some(s) = &f.source {
                m.insert("source".into(), s.clone().into());
            }
            m
        };
        let whole = f.fs_type & datars_text::EMBED_NO_SUBSETTING != 0;
        let full_gz = crate::gzip_len(&f.bytes) / 1024;
        let eager = if whole { Err(subset::NotSubset::Invalid("its fsType forbids subsetting".into())) } else { subset::subset(&f.bytes, f.index, chars) };
        match eager {
            Ok(s) => {
                let gz = crate::gzip_len(&s.bytes);
                let mut m = base_meta("chart");
                m.insert("glyphs".into(), s.glyphs.into());
                out.decisions.push(format!("{label}: {} glyphs for the chart's text, {:.1} KB gzipped (whole font {full_gz} KB){}, from {}", s.glyphs, gz as f64 / 1024.0, if s.gpos_verbatim { "; GPOS kept whole" } else { "" }, f.from));
                out.eager.push(b.chunk("font", s.bytes, m));
            }
            Err(why) => {
                out.decisions.push(format!("{label}: shipped whole, {full_gz} KB gzipped ({why}), from {}", f.from));
                out.eager.push(b.chunk("font", f.bytes.clone(), base_meta("whole")));
                continue;
            }
        }
        if !runtime_text {
            continue;
        }
        // Text the build can't see (host data, live feeds, streamed tiles): the rest of each
        // script the font covers, as chunks a runtime fetches when a label needs them. Each part
        // includes the chart's characters too, so text shaped with it kerns as with the font.
        let mapped = mapped_chars(&f.bytes, f.index);
        let rest: Vec<char> = mapped.iter().copied().filter(|c| !chars.contains(c) && !c.is_control()).collect();
        let mut parts = Vec::new();
        for (script, unicodes, cs) in scripts::split(&rest) {
            let mut want = chars.clone();
            want.extend(cs);
            let Ok(s) = subset::subset(&f.bytes, f.index, &want) else { continue };
            let mut m = base_meta("script");
            m.insert("part".into(), script.clone().into());
            m.insert("unicodes".into(), unicodes.into());
            m.insert("glyphs".into(), s.glyphs.into());
            let gz = crate::gzip_len(&s.bytes);
            let h = b.chunk("font", s.bytes, m);
            b.mark(&h, true, true, None);
            parts.push(format!("{script} {:.1} KB", gz as f64 / 1024.0));
            out.lazy.push(h);
        }
        if !parts.is_empty() {
            out.decisions.push(format!("{label}: runtime text — script subsets load when needed: {}", parts.join(", ")));
        }
    }
    out
}

/// Every character a face maps to a glyph.
pub fn mapped_chars(bytes: &[u8], index: u32) -> BTreeSet<char> {
    let Ok(font) = FontRef::from_index(bytes, index) else { return BTreeSet::new() };
    let Some((_, _, sub)) = font.cmap().ok().and_then(|c| c.best_subtable()) else { return BTreeSet::new() };
    sub.iter().filter(|(_, g)| g.to_u32() != 0).filter_map(|(cp, _)| char::from_u32(cp)).collect()
}

/// A font's licence, as its name table states it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Licence {
    /// `OFL-1.1`, `Apache-2.0`, `UFL-1.0` or `unknown`.
    pub id: String,
    pub url: Option<String>,
    pub copyright: Option<String>,
}

/// Read the licence notices (name IDs 13 and 14) and the copyright (ID 0).
pub fn licence(bytes: &[u8], index: u32) -> Licence {
    let name = |id: u16| -> Option<String> {
        let font = FontRef::from_index(bytes, index).ok()?;
        let name = font.name().ok()?;
        let data = name.string_data();
        // Prefer Windows/Unicode English records, then anything readable.
        let mut best: Option<(u8, String)> = None;
        for r in name.name_record().iter().filter(|r| r.name_id().to_u16() == id) {
            let Ok(s) = r.string(data) else { continue };
            let s: String = s.chars().collect();
            let rank = match (r.platform_id(), r.language_id()) {
                (3, 0x409) => 0,
                (0, _) => 1,
                (3, _) => 2,
                _ => 3,
            };
            if !s.trim().is_empty() && best.as_ref().is_none_or(|(b, _)| rank < *b) {
                best = Some((rank, s.trim().to_string()));
            }
        }
        best.map(|(_, s)| s)
    };
    let (desc, url, copyright) = (name(13).unwrap_or_default(), name(14), name(0));
    let text = format!("{desc} {}", url.clone().unwrap_or_default()).to_lowercase();
    let id = if text.contains("open font license") || text.contains("scripts.sil.org/ofl") || text.contains("openfontlicense.org") {
        "OFL-1.1"
    } else if text.contains("apache license") || text.contains("apache.org/licenses") {
        "Apache-2.0"
    } else if text.contains("ubuntu font licen") {
        "UFL-1.0"
    } else {
        "unknown"
    };
    Licence { id: id.into(), url, copyright }
}
