//! Font subsetting for bundles: keep the glyphs a chart can show, and nothing else.
//!
//! Glyph ids are **retained**: a dropped glyph becomes an empty outline instead of being
//! renumbered. So every glyph id in a baked scene (T1), every GSUB/GPOS/GDEF reference and every
//! composite component stays valid, and a subset shapes exactly like the full font for any text
//! it covers — the property the tier tests check pixel for pixel (P1).
//!
//! Which glyphs: the covered characters' glyphs, everything the shaper can substitute them with
//! through the features it applies by default (the engine passes no user features, so stylistic
//! sets, tabular figures and the like never apply), and the components of composite glyphs.
//!
//! What is rewritten: `cmap` (only the covered characters), `glyf`/`loca` (only the kept glyphs'
//! outlines), `hmtx` (dropped glyphs' metrics zeroed), `post` (version 3, no glyph names), and
//! `GSUB`, `GPOS` and `GDEF` (substitutions, pair kerning, mark attachment and glyph classes
//! filtered to the kept glyphs — the bulk of a text font's layout data; contextual lookups are
//! kept as they are, their glyph ids still valid). Hinting, bitmap, colour and variation tables
//! are dropped (the engine draws unhinted outlines at the default instance). Only TrueType
//! outlines are subset; CFF fonts ship whole.

use read_fonts::collections::IntSet;
use read_fonts::tables::glyf::Glyph;
use read_fonts::types::{GlyphId, GlyphId16, Tag};
use read_fonts::{FontData, FontRead, FontRef, TableProvider};
use std::collections::{BTreeMap, BTreeSet};
use write_fonts::from_obj::ToOwnedTable;
use write_fonts::tables::gpos::{
    BaseArray, BaseRecord, Class1Record, ComponentRecord, ExtensionSubtable, Gpos, LigatureArray, LigatureAttach, Mark2Array, Mark2Record, MarkArray, MarkBasePosFormat1, MarkLigPosFormat1, MarkMarkPosFormat1, PairPos, PairPosFormat1, PairPosFormat2, PairSet, PositionLookup,
};
use write_fonts::tables::gsub::{AlternateSet, AlternateSubstFormat1, Gsub, Ligature, LigatureSet, LigatureSubstFormat1, MultipleSubstFormat1, Sequence, SingleSubst, SingleSubstFormat2, SubstitutionLookup};
use write_fonts::tables::layout::{ClassDef, CoverageTable};

/// A subset font and what it kept.
#[derive(Clone, Debug)]
pub struct Subset {
    pub bytes: Vec<u8>,
    /// Glyphs with outlines (the closure of the covered characters, .notdef included).
    pub glyphs: usize,
    /// Covered characters the font maps.
    pub chars: usize,
    /// A layout table (GSUB, GPOS, GDEF) kept whole because rewriting it failed (still
    /// correct, just larger).
    pub gpos_verbatim: bool,
}

/// Features HarfBuzz-class shapers apply without being asked, for any script: the only ones
/// whose substitutions a subset must keep, since the engine requests no others.
const DEFAULT_FEATURES: [&[u8; 4]; 62] = [
    b"abvf", b"abvm", b"abvs", b"akhn", b"blwf", b"blwm", b"blws", b"calt", b"ccmp", b"cfar", b"cjct", b"clig", b"cswh", b"curs", b"dist", b"dnom", b"fin2", b"fin3", b"fina", b"frac", b"half", b"haln", b"init", b"isol", b"jalt", b"kern", b"liga", b"ljmo", b"locl", b"ltra", b"ltrm", b"mark", b"med2", b"medi", b"mkmk", b"mset", b"nukt", b"numr", b"pref", b"pres", b"pstf", b"psts", b"rand", b"rclt", b"rkrf", b"rlig", b"rphf", b"rtla", b"rtlm", b"rvrn", b"stch", b"tjmo", b"vatu", b"vert", b"vjmo", b"vrt2", b"Harf", b"HARF", b"Buzz", b"BUZZ", b"trak", b"valt",
];

/// Why a font was not subset (the caller ships it whole).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotSubset {
    /// CFF/CFF2 outlines: this subsetter handles TrueType (`glyf`) outlines.
    Cff,
    /// Unreadable or inconsistent data.
    Invalid(String),
}

impl std::fmt::Display for NotSubset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotSubset::Cff => write!(f, "CFF outlines (only TrueType outlines are subset)"),
            NotSubset::Invalid(e) => write!(f, "unreadable font: {e}"),
        }
    }
}

fn invalid(e: impl std::fmt::Display) -> NotSubset {
    NotSubset::Invalid(e.to_string())
}

/// Tables a subset keeps (rewritten or verbatim). Everything else — hinting (`fpgm`, `prep`,
/// `cvt `, `gasp`, `hdmx`, `LTSH`, `VDMX`), signatures, bitmaps, colour, variations — is dropped.
const KEEP_VERBATIM: [&[u8; 4]; 6] = [b"head", b"hhea", b"maxp", b"OS/2", b"kern", b"vhea"];

/// Name records a subset keeps: copyright (0), family and style (1, 2, 16, 17), trademark (7),
/// licence text and URL (13, 14) — the notices OFL and Apache fonts must travel with.
const KEEP_NAMES: [u16; 8] = [0, 1, 2, 7, 13, 14, 16, 17];

/// Subset face `index` of `data` (a font or collection) to the characters in `chars`. The result
/// is a standalone single-face font; the same inputs always give the same bytes.
pub fn subset(data: &[u8], index: u32, chars: &BTreeSet<char>) -> Result<Subset, NotSubset> {
    let font = FontRef::from_index(data, index).map_err(invalid)?;
    if font.table_data(Tag::new(b"glyf")).is_none() {
        return Err(if font.table_data(Tag::new(b"CFF ")).is_some() || font.table_data(Tag::new(b"CFF2")).is_some() { NotSubset::Cff } else { NotSubset::Invalid("no outlines".into()) });
    }
    let num_glyphs = font.maxp().map_err(invalid)?.num_glyphs() as u32;
    let cmap = font.cmap().map_err(invalid)?;

    // Characters → glyphs, then everything shaping can reach from them.
    let mut mapping: Vec<(char, GlyphId)> = Vec::new();
    let mut keep: IntSet<GlyphId> = IntSet::empty();
    keep.insert(GlyphId::NOTDEF);
    for &c in chars {
        if let Some(g) = cmap.map_codepoint(c).filter(|g| g.to_u32() != 0 && g.to_u32() < num_glyphs) {
            mapping.push((c, g));
            keep.insert(g);
        }
    }
    let mut tags: IntSet<Tag> = IntSet::empty();
    for t in DEFAULT_FEATURES {
        // Fractions only form around U+2044 FRACTION SLASH.
        if !chars.contains(&'\u{2044}') && matches!(t, b"frac" | b"numr" | b"dnom") {
            continue;
        }
        tags.insert(Tag::new(t));
    }
    // The engine shapes without a language, so only each script's default language system
    // applies (no `locl` forms for Romanian, Dutch, Serbian…).
    let languages: IntSet<Tag> = IntSet::empty();
    // The lookups default features reach (and the lookups contextual ones call), then the glyphs
    // those produce from the covered ones. Lookups outside the set never run.
    let mut gsub_lookups: Option<IntSet<u16>> = None;
    if let Ok(gsub) = font.gsub() {
        match gsub.collect_features(&IntSet::all(), &languages, &tags).and_then(|f| gsub.collect_lookups(&f)) {
            Ok(mut lookups) => {
                let _ = gsub.closure_lookups(&keep, &mut lookups);
                let _ = gsub.closure_glyphs(&lookups, &mut keep);
                gsub_lookups = Some(lookups);
            }
            Err(_) => {
                let _ = gsub.closure_glyphs(&IntSet::all(), &mut keep);
            }
        }
    }
    let head = font.head().map_err(invalid)?;
    let loca = font.loca(head.index_to_loc_format() == 1).map_err(invalid)?;
    let glyf = font.glyf().map_err(invalid)?;
    // Composite glyphs draw their components.
    let mut pending: Vec<GlyphId> = keep.iter().collect();
    while let Some(g) = pending.pop() {
        if let Ok(Some(Glyph::Composite(c))) = loca.get_glyf(g, &glyf) {
            for (component, _) in c.component_glyphs_and_flags() {
                let component = GlyphId::from(component);
                if component.to_u32() < num_glyphs && keep.insert(component) {
                    pending.push(component);
                }
            }
        }
    }
    let kept: BTreeSet<u32> = keep.iter().map(|g| g.to_u32()).filter(|&g| g < num_glyphs).collect();

    // glyf + loca: kept outlines in id order, dropped ones empty.
    let glyf_data = font.table_data(Tag::new(b"glyf")).ok_or_else(|| invalid("no glyf"))?;
    let glyf_bytes = glyf_data.as_bytes();
    let mut new_glyf: Vec<u8> = Vec::new();
    let mut offsets: Vec<u32> = Vec::with_capacity(num_glyphs as usize + 1);
    for g in 0..num_glyphs {
        offsets.push(new_glyf.len() as u32);
        if kept.contains(&g) {
            let (start, end) = (loca.get_raw(g as usize).ok_or_else(|| invalid("loca"))?, loca.get_raw(g as usize + 1).ok_or_else(|| invalid("loca"))?);
            if start < end {
                new_glyf.extend_from_slice(glyf_bytes.get(start as usize..end as usize).ok_or_else(|| invalid("glyf offsets"))?);
                if new_glyf.len() % 2 == 1 {
                    new_glyf.push(0);
                }
            }
        }
    }
    offsets.push(new_glyf.len() as u32);
    let short = new_glyf.len() / 2 <= u16::MAX as usize;
    let new_loca: Vec<u8> = if short { offsets.iter().flat_map(|&o| ((o / 2) as u16).to_be_bytes()).collect() } else { offsets.iter().flat_map(|&o| o.to_be_bytes()).collect() };

    let mut b = write_fonts::FontBuilder::new();
    for tag in KEEP_VERBATIM {
        if let Some(t) = font.table_data(Tag::new(tag)) {
            b.add_raw(Tag::new(tag), t.as_bytes().to_vec());
        }
    }
    if let Some(t) = font.table_data(Tag::new(b"name")) {
        b.add_raw(Tag::new(b"name"), trim_names(t.as_bytes()).unwrap_or_else(|| t.as_bytes().to_vec()));
    }
    // head: the loca format this subset uses (the builder fills in the checksum adjustment).
    let mut head_bytes = font.table_data(Tag::new(b"head")).ok_or_else(|| invalid("no head"))?.as_bytes().to_vec();
    if head_bytes.len() < 54 {
        return Err(invalid("short head"));
    }
    head_bytes[50..52].copy_from_slice(&(if short { 0i16 } else { 1i16 }).to_be_bytes());
    b.add_raw(Tag::new(b"head"), head_bytes);
    b.add_raw(Tag::new(b"glyf"), new_glyf);
    b.add_raw(Tag::new(b"loca"), new_loca);

    let cmap = write_fonts::tables::cmap::Cmap::from_mappings(mapping.iter().copied()).map_err(|e| invalid(format!("{e:?}")))?;
    b.add_raw(Tag::new(b"cmap"), write_fonts::dump_table(&cmap).map_err(invalid)?);
    b.add_raw(Tag::new(b"hmtx"), zero_dropped_metrics(&font, &kept, b"hhea", b"hmtx", num_glyphs)?);
    if font.table_data(Tag::new(b"vmtx")).is_some() && font.table_data(Tag::new(b"vhea")).is_some() {
        b.add_raw(Tag::new(b"vmtx"), zero_dropped_metrics(&font, &kept, b"vhea", b"vmtx", num_glyphs)?);
    }
    if let Some(post) = font.table_data(Tag::new(b"post")) {
        // Version 3: the metrics, no glyph names (shaping and drawing never read them).
        let mut p = post.as_bytes().get(..32).ok_or_else(|| invalid("short post"))?.to_vec();
        p[0..4].copy_from_slice(&0x0003_0000u32.to_be_bytes());
        b.add_raw(Tag::new(b"post"), p);
    }
    let mut gpos_verbatim = false;
    let gpos_lookups: Option<IntSet<u16>> = font.gpos().ok().and_then(|gpos| {
        let keep_ids: IntSet<GlyphId> = kept.iter().map(|&g| GlyphId::new(g)).collect();
        let mut lookups = gpos.collect_features(&IntSet::all(), &languages, &tags).and_then(|f| gpos.collect_lookups(&f)).ok()?;
        gpos.closure_lookups(&keep_ids, &mut lookups).ok()?;
        Some(lookups)
    });
    let layout: [&[u8; 4]; 3] = [b"GSUB", b"GPOS", b"GDEF"];
    for tag in layout {
        if let Some(raw) = font.table_data(Tag::new(tag)) {
            let rewritten = match tag {
                b"GSUB" => subset_gsub(raw, &kept, gsub_lookups.as_ref()),
                b"GPOS" => subset_gpos(raw, &kept, gpos_lookups.as_ref()),
                _ => subset_gdef(raw, &kept),
            };
            match rewritten {
                Some(bytes) => {
                    b.add_raw(Tag::new(tag), bytes);
                }
                None => {
                    gpos_verbatim = true;
                    b.add_raw(Tag::new(tag), raw.as_bytes().to_vec());
                }
            }
        }
    }
    Ok(Subset { bytes: b.build(), glyphs: kept.len(), chars: mapping.len(), gpos_verbatim })
}

/// The `name` table with only [`KEEP_NAMES`], Windows records when there are any (else all
/// platforms'), as format 0. `None` if it can't be read (the caller keeps it whole).
fn trim_names(t: &[u8]) -> Option<Vec<u8>> {
    let be16 = |o: usize| t.get(o..o + 2).map(|s| u16::from_be_bytes([s[0], s[1]]));
    let (count, storage) = (be16(2)? as usize, be16(4)? as usize);
    let records: Vec<[u16; 6]> = (0..count).map(|i| (0..6).map(|k| be16(6 + i * 12 + k * 2)).collect::<Option<Vec<u16>>>().map(|v| [v[0], v[1], v[2], v[3], v[4], v[5]])).collect::<Option<_>>()?;
    let windows = records.iter().any(|r| r[0] == 3);
    let keep: Vec<&[u16; 6]> = records.iter().filter(|r| KEEP_NAMES.contains(&r[3]) && (!windows || r[0] == 3)).collect();
    let mut head: Vec<u8> = Vec::new();
    let mut strings: Vec<u8> = Vec::new();
    head.extend_from_slice(&0u16.to_be_bytes());
    head.extend_from_slice(&(keep.len() as u16).to_be_bytes());
    head.extend_from_slice(&(6 + 12 * keep.len() as u16).to_be_bytes());
    for r in keep {
        let (len, off) = (r[4] as usize, r[5] as usize);
        let bytes = t.get(storage + off..storage + off + len)?;
        for v in [r[0], r[1], r[2], r[3], len as u16, strings.len() as u16] {
            head.extend_from_slice(&v.to_be_bytes());
        }
        strings.extend_from_slice(bytes);
    }
    head.extend_from_slice(&strings);
    Some(head)
}

/// `hmtx`/`vmtx` with the dropped glyphs' side bearings (and advances, where a glyph has its own)
/// zeroed: never read for text the subset covers, and zeros compress to nothing. The last long
/// metric's advance is shared by every glyph after it, so it stays.
fn zero_dropped_metrics(font: &FontRef, kept: &BTreeSet<u32>, hea: &[u8; 4], mtx: &[u8; 4], num_glyphs: u32) -> Result<Vec<u8>, NotSubset> {
    let hea = font.table_data(Tag::new(hea)).ok_or_else(|| invalid("no metrics header"))?;
    let n_long = u16::from_be_bytes(hea.as_bytes().get(34..36).ok_or_else(|| invalid("short metrics header"))?.try_into().unwrap_or([0, 0])) as u32;
    let mut m = font.table_data(Tag::new(mtx)).ok_or_else(|| invalid("no metrics"))?.as_bytes().to_vec();
    for g in (0..num_glyphs).filter(|g| !kept.contains(g)) {
        let (at, len) = if g < n_long { (g as usize * 4, 4) } else { (n_long as usize * 4 + (g - n_long) as usize * 2, 2) };
        let from = if g < n_long && g + 1 == n_long { at + 2 } else { at };
        if let Some(bytes) = m.get_mut(from..at + len) {
            bytes.fill(0);
        }
    }
    Ok(m)
}

/// GPOS with every pair-kerning and mark-attachment subtable filtered to the kept glyphs (their
/// classes renumbered densely). Other subtables are kept as they are. Lookup indices never change
/// (features and contextual lookups refer to them), so a lookup may end up with no subtables.
/// `None` if the table can't be read or written (the caller keeps it whole).
fn subset_gpos(raw: FontData, kept: &BTreeSet<u32>, live: Option<&IntSet<u16>>) -> Option<Vec<u8>> {
    let read = read_fonts::tables::gpos::Gpos::read(raw).ok()?;
    let mut gpos: Gpos = read.to_owned_table();
    let keep = |g: GlyphId16| kept.contains(&(g.to_u16() as u32));
    for (index, lookup) in gpos.lookup_list.lookups.iter_mut().enumerate() {
        if live.is_some_and(|l| !l.contains(index as u16)) {
            clear_position_lookup(lookup);
            continue;
        }
        match &mut **lookup {
            PositionLookup::Pair(l) => l.subtables.retain_mut(|st| filter_pair(st, &keep)),
            PositionLookup::MarkToBase(l) => l.subtables.retain_mut(|st| filter_mark_base(st, &keep)),
            PositionLookup::MarkToMark(l) => l.subtables.retain_mut(|st| filter_mark_mark(st, &keep)),
            PositionLookup::MarkToLig(l) => l.subtables.retain_mut(|st| filter_mark_lig(st, &keep)),
            PositionLookup::Extension(l) => l.subtables.retain_mut(|ext| match &mut **ext {
                ExtensionSubtable::Pair(e) => filter_pair(&mut e.extension, &keep),
                ExtensionSubtable::MarkToBase(e) => filter_mark_base(&mut e.extension, &keep),
                ExtensionSubtable::MarkToMark(e) => filter_mark_mark(&mut e.extension, &keep),
                ExtensionSubtable::MarkToLig(e) => filter_mark_lig(&mut e.extension, &keep),
                ExtensionSubtable::Contextual(e) => context_can_match(&e.extension, &keep),
                ExtensionSubtable::ChainContextual(e) => chain_can_match(&e.extension, &keep),
                _ => true,
            }),
            PositionLookup::Contextual(l) => l.subtables.retain(|st| context_can_match(st, &keep)),
            PositionLookup::ChainContextual(l) => l.subtables.retain(|st| chain_can_match(st, &keep)),
            _ => {}
        }
    }
    write_fonts::dump_table(&gpos).ok()
}

/// Drop every subtable of a lookup no default feature reaches (it never runs); the lookup stays,
/// so lookup indices keep their meaning.
fn clear_position_lookup(l: &mut PositionLookup) {
    match l {
        PositionLookup::Single(l) => l.subtables.clear(),
        PositionLookup::Pair(l) => l.subtables.clear(),
        PositionLookup::Cursive(l) => l.subtables.clear(),
        PositionLookup::MarkToBase(l) => l.subtables.clear(),
        PositionLookup::MarkToLig(l) => l.subtables.clear(),
        PositionLookup::MarkToMark(l) => l.subtables.clear(),
        PositionLookup::Contextual(l) => l.subtables.clear(),
        PositionLookup::ChainContextual(l) => l.subtables.clear(),
        PositionLookup::Extension(l) => l.subtables.clear(),
    }
}

fn clear_substitution_lookup(l: &mut SubstitutionLookup) {
    match l {
        SubstitutionLookup::Single(l) => l.subtables.clear(),
        SubstitutionLookup::Multiple(l) => l.subtables.clear(),
        SubstitutionLookup::Alternate(l) => l.subtables.clear(),
        SubstitutionLookup::Ligature(l) => l.subtables.clear(),
        SubstitutionLookup::Contextual(l) => l.subtables.clear(),
        SubstitutionLookup::ChainContextual(l) => l.subtables.clear(),
        SubstitutionLookup::Extension(l) => l.subtables.clear(),
        SubstitutionLookup::Reverse(l) => l.subtables.clear(),
    }
}

/// Can a contextual subtable match text made of kept glyphs? Only if its (first) input coverage
/// has a kept glyph — every format checks that coverage first. Rules are kept as they are.
fn context_can_match(ctx: &write_fonts::tables::layout::SequenceContext, keep: &impl Fn(GlyphId16) -> bool) -> bool {
    use write_fonts::tables::layout::SequenceContext as S;
    match ctx {
        S::Format1(f) => f.coverage.iter().any(keep),
        S::Format2(f) => f.coverage.iter().any(keep),
        S::Format3(f) => f.coverages.iter().all(|c| c.iter().any(keep)),
    }
}

/// [`context_can_match`] for chained contexts: format 3 needs every backtrack, input and
/// lookahead coverage to have a kept glyph.
fn chain_can_match(ctx: &write_fonts::tables::layout::ChainedSequenceContext, keep: &impl Fn(GlyphId16) -> bool) -> bool {
    use write_fonts::tables::layout::ChainedSequenceContext as C;
    match ctx {
        C::Format1(f) => f.coverage.iter().any(keep),
        C::Format2(f) => f.coverage.iter().any(keep),
        C::Format3(f) => f.backtrack_coverages.iter().chain(&f.input_coverages).chain(&f.lookahead_coverages).all(|c| c.iter().any(keep)),
    }
}

/// GSUB with single, multiple, alternate and ligature substitutions filtered to entries whose
/// input and output glyphs are all kept (what default features can't reach drops out). Lookup
/// indices never change; contextual subtables are kept as they are.
fn subset_gsub(raw: FontData, kept: &BTreeSet<u32>, live: Option<&IntSet<u16>>) -> Option<Vec<u8>> {
    let read = read_fonts::tables::gsub::Gsub::read(raw).ok()?;
    let mut gsub: Gsub = read.to_owned_table();
    let keep = |g: GlyphId16| kept.contains(&(g.to_u16() as u32));
    for (index, lookup) in gsub.lookup_list.lookups.iter_mut().enumerate() {
        if live.is_some_and(|l| !l.contains(index as u16)) {
            clear_substitution_lookup(lookup);
            continue;
        }
        match &mut **lookup {
            SubstitutionLookup::Single(l) => l.subtables.retain_mut(|st| filter_single(st, &keep)),
            SubstitutionLookup::Multiple(l) => l.subtables.retain_mut(|st| filter_multiple(st, &keep)),
            SubstitutionLookup::Alternate(l) => l.subtables.retain_mut(|st| filter_alternate(st, &keep)),
            SubstitutionLookup::Ligature(l) => l.subtables.retain_mut(|st| filter_ligature(st, &keep)),
            SubstitutionLookup::Extension(l) => l.subtables.retain_mut(|ext| match &mut **ext {
                write_fonts::tables::gsub::ExtensionSubtable::Single(e) => filter_single(&mut e.extension, &keep),
                write_fonts::tables::gsub::ExtensionSubtable::Multiple(e) => filter_multiple(&mut e.extension, &keep),
                write_fonts::tables::gsub::ExtensionSubtable::Alternate(e) => filter_alternate(&mut e.extension, &keep),
                write_fonts::tables::gsub::ExtensionSubtable::Ligature(e) => filter_ligature(&mut e.extension, &keep),
                write_fonts::tables::gsub::ExtensionSubtable::Contextual(e) => context_can_match(&e.extension, &keep),
                write_fonts::tables::gsub::ExtensionSubtable::ChainContextual(e) => chain_can_match(&e.extension, &keep),
                _ => true,
            }),
            SubstitutionLookup::Contextual(l) => l.subtables.retain(|st| context_can_match(st, &keep)),
            SubstitutionLookup::ChainContextual(l) => l.subtables.retain(|st| chain_can_match(st, &keep)),
            _ => {}
        }
    }
    write_fonts::dump_table(&gsub).ok()
}

fn filter_single(st: &mut SingleSubst, keep: &impl Fn(GlyphId16) -> bool) -> bool {
    let pairs: Vec<(GlyphId16, GlyphId16)> = match st {
        SingleSubst::Format1(f) => f.coverage.iter().map(|g| (g, GlyphId16::new((g.to_u16() as i32 + f.delta_glyph_id as i32).rem_euclid(65536) as u16))).collect(),
        SingleSubst::Format2(f) => f.coverage.iter().zip(f.substitute_glyph_ids.iter().copied()).collect(),
    };
    let pairs: Vec<(GlyphId16, GlyphId16)> = pairs.into_iter().filter(|(a, b)| keep(*a) && keep(*b)).collect();
    if pairs.is_empty() {
        return false;
    }
    *st = match st {
        SingleSubst::Format1(f) => SingleSubst::format_1(CoverageTable::from_iter(pairs.iter().map(|p| p.0)), f.delta_glyph_id),
        SingleSubst::Format2(_) => SingleSubst::Format2(SingleSubstFormat2::new(CoverageTable::from_iter(pairs.iter().map(|p| p.0)), pairs.iter().map(|p| p.1).collect())),
    };
    true
}

fn filter_multiple(st: &mut MultipleSubstFormat1, keep: &impl Fn(GlyphId16) -> bool) -> bool {
    let kept: Vec<(GlyphId16, Sequence)> = covered(&st.coverage, keep).into_iter().filter_map(|(i, g)| st.sequences.get(i).filter(|s| s.substitute_glyph_ids.iter().all(|x| keep(*x))).map(|s| (g, (**s).clone()))).collect();
    if kept.is_empty() {
        return false;
    }
    *st = MultipleSubstFormat1::new(CoverageTable::from_iter(kept.iter().map(|k| k.0)), kept.into_iter().map(|k| k.1).collect());
    true
}

fn filter_alternate(st: &mut AlternateSubstFormat1, keep: &impl Fn(GlyphId16) -> bool) -> bool {
    // Alternates are picked by index (`rand`), so a set is kept whole or not at all.
    let kept: Vec<(GlyphId16, AlternateSet)> = covered(&st.coverage, keep).into_iter().filter_map(|(i, g)| st.alternate_sets.get(i).filter(|s| s.alternate_glyph_ids.iter().all(|x| keep(*x))).map(|s| (g, (**s).clone()))).collect();
    if kept.is_empty() {
        return false;
    }
    *st = AlternateSubstFormat1::new(CoverageTable::from_iter(kept.iter().map(|k| k.0)), kept.into_iter().map(|k| k.1).collect());
    true
}

fn filter_ligature(st: &mut LigatureSubstFormat1, keep: &impl Fn(GlyphId16) -> bool) -> bool {
    let mut first = Vec::new();
    let mut sets = Vec::new();
    for (i, g) in covered(&st.coverage, keep) {
        let Some(set) = st.ligature_sets.get(i) else { continue };
        let ligs: Vec<Ligature> = set.ligatures.iter().filter(|l| keep(l.ligature_glyph) && l.component_glyph_ids.iter().all(|c| keep(*c))).map(|l| (**l).clone()).collect();
        if !ligs.is_empty() {
            first.push(g);
            sets.push(LigatureSet::new(ligs));
        }
    }
    if first.is_empty() {
        return false;
    }
    *st = LigatureSubstFormat1::new(CoverageTable::from_iter(first), sets);
    true
}

/// GDEF with its glyph classes, mark attachment classes, mark glyph sets, attachment points and
/// ligature carets limited to the kept glyphs.
fn subset_gdef(raw: FontData, kept: &BTreeSet<u32>) -> Option<Vec<u8>> {
    let read = read_fonts::tables::gdef::Gdef::read(raw).ok()?;
    let mut gdef: write_fonts::tables::gdef::Gdef = read.to_owned_table();
    let keep = |g: GlyphId16| kept.contains(&(g.to_u16() as u32));
    let filter_classes = |c: &ClassDef| -> ClassDef { c.iter().filter(|(g, cls)| keep(*g) && *cls != 0).collect() };
    if let Some(c) = gdef.glyph_class_def.as_mut() {
        *c = filter_classes(c);
    }
    if let Some(c) = gdef.mark_attach_class_def.as_mut() {
        *c = filter_classes(c);
    }
    if let Some(sets) = gdef.mark_glyph_sets_def.as_mut() {
        // Sets are referred to by index: filter each, keep them all.
        for cov in sets.coverages.iter_mut() {
            let glyphs: Vec<GlyphId16> = cov.iter().filter(|g| keep(*g)).collect();
            **cov = CoverageTable::from_iter(glyphs);
        }
    }
    if let Some(list) = gdef.attach_list.as_mut() {
        let kept: Vec<(GlyphId16, write_fonts::tables::gdef::AttachPoint)> = covered(&list.coverage, &keep).into_iter().filter_map(|(i, g)| list.attach_points.get(i).map(|p| (g, (**p).clone()))).collect();
        *list = write_fonts::tables::gdef::AttachList::new(CoverageTable::from_iter(kept.iter().map(|k| k.0)), kept.into_iter().map(|k| k.1).collect());
    }
    if let Some(list) = gdef.lig_caret_list.as_mut() {
        let kept: Vec<(GlyphId16, write_fonts::tables::gdef::LigGlyph)> = covered(&list.coverage, &keep).into_iter().filter_map(|(i, g)| list.lig_glyphs.get(i).map(|p| (g, (**p).clone()))).collect();
        *list = write_fonts::tables::gdef::LigCaretList::new(CoverageTable::from_iter(kept.iter().map(|k| k.0)), kept.into_iter().map(|k| k.1).collect());
    }
    write_fonts::dump_table(&gdef).ok()
}

/// Kept glyphs of a coverage table, with their coverage indices, in coverage order.
fn covered(c: &CoverageTable, keep: &impl Fn(GlyphId16) -> bool) -> Vec<(usize, GlyphId16)> {
    c.iter().enumerate().filter(|(_, g)| keep(*g)).collect()
}

/// Dense renumbering of the classes in use: class 0 stays 0 (it means "every other glyph"),
/// the rest keep their order.
fn dense(classes: impl IntoIterator<Item = u16>) -> BTreeMap<u16, u16> {
    let mut used: BTreeSet<u16> = classes.into_iter().collect();
    used.insert(0);
    used.into_iter().enumerate().map(|(new, old)| (old, new as u16)).collect()
}

fn filter_pair(st: &mut PairPos, keep: &impl Fn(GlyphId16) -> bool) -> bool {
    match st {
        PairPos::Format1(p) => filter_pair1(p, keep),
        PairPos::Format2(p) => filter_pair2(p, keep),
    }
}

fn filter_pair1(p: &mut PairPosFormat1, keep: &impl Fn(GlyphId16) -> bool) -> bool {
    let mut glyphs = Vec::new();
    let mut sets: Vec<PairSet> = Vec::new();
    for (i, g) in covered(&p.coverage, keep) {
        let Some(set) = p.pair_sets.get(i) else { continue };
        let records: Vec<_> = set.pair_value_records.iter().filter(|r| keep(r.second_glyph)).cloned().collect();
        if !records.is_empty() {
            glyphs.push(g);
            sets.push(PairSet::new(records));
        }
    }
    if glyphs.is_empty() {
        return false;
    }
    *p = PairPosFormat1::new(CoverageTable::from_iter(glyphs), sets);
    true
}

fn filter_pair2(p: &mut PairPosFormat2, keep: &impl Fn(GlyphId16) -> bool) -> bool {
    let first: Vec<GlyphId16> = covered(&p.coverage, keep).into_iter().map(|(_, g)| g).collect();
    if first.is_empty() {
        return false;
    }
    let c1 = dense(first.iter().map(|g| p.class_def1.get(*g)));
    // Second glyphs: any kept glyph the class def lists (unlisted glyphs are class 0).
    let second: Vec<(GlyphId16, u16)> = p.class_def2.iter().filter(|(g, _)| keep(*g)).collect();
    let c2 = dense(second.iter().map(|(_, c)| *c));
    let records: Vec<Class1Record> = c1
        .keys()
        .map(|&old1| {
            let row = p.class1_records.get(old1 as usize);
            Class1Record::new(c2.keys().map(|&old2| row.and_then(|r| r.class2_records.get(old2 as usize)).cloned().unwrap_or_default()).collect())
        })
        .collect();
    // The value format comes from the first record, so every record must carry it explicitly.
    if records.iter().flat_map(|r| &r.class2_records).any(|r| r.value_record1.format() != records[0].class2_records[0].value_record1.format() || r.value_record2.format() != records[0].class2_records[0].value_record2.format()) {
        return true; // leave it as it was rather than guess
    }
    let class_def1: ClassDef = first.iter().map(|g| (*g, c1[&p.class_def1.get(*g)])).filter(|(_, c)| *c != 0).collect();
    let class_def2: ClassDef = second.iter().map(|(g, c)| (*g, c2[c])).filter(|(_, c)| *c != 0).collect();
    *p = PairPosFormat2::new(CoverageTable::from_iter(first), class_def1, class_def2, records);
    true
}

/// Mark records for the kept marks (classes renumbered by `classes`), and the kept mark glyphs.
fn kept_marks(coverage: &CoverageTable, array: &MarkArray, keep: &impl Fn(GlyphId16) -> bool) -> (Vec<GlyphId16>, Vec<(u16, write_fonts::tables::gpos::AnchorTable)>) {
    let mut glyphs = Vec::new();
    let mut records = Vec::new();
    for (i, g) in covered(coverage, keep) {
        if let Some(r) = array.mark_records.get(i) {
            glyphs.push(g);
            records.push((r.mark_class, (*r.mark_anchor).clone()));
        }
    }
    (glyphs, records)
}

/// Renumber mark classes densely (no class 0 special case here: every class is a column).
fn mark_classes(records: &[(u16, write_fonts::tables::gpos::AnchorTable)]) -> BTreeMap<u16, u16> {
    records.iter().map(|(c, _)| *c).collect::<BTreeSet<u16>>().into_iter().enumerate().map(|(new, old)| (old, new as u16)).collect()
}

fn mark_array(records: Vec<(u16, write_fonts::tables::gpos::AnchorTable)>, classes: &BTreeMap<u16, u16>) -> MarkArray {
    MarkArray::new(records.into_iter().map(|(c, a)| write_fonts::tables::gpos::MarkRecord::new(classes[&c], a)).collect())
}

fn filter_mark_base(st: &mut MarkBasePosFormat1, keep: &impl Fn(GlyphId16) -> bool) -> bool {
    let (marks, records) = kept_marks(&st.mark_coverage, &st.mark_array, keep);
    let bases = covered(&st.base_coverage, keep);
    if marks.is_empty() || bases.is_empty() {
        return false;
    }
    let classes = mark_classes(&records);
    let base_records: Vec<BaseRecord> = bases
        .iter()
        .map(|(i, _)| {
            let old = st.base_array.base_records.get(*i);
            BaseRecord::new(classes.keys().map(|&c| old.and_then(|r| r.base_anchors.get(c as usize)).and_then(|a| a.as_ref().cloned())).collect())
        })
        .collect();
    *st = MarkBasePosFormat1::new(CoverageTable::from_iter(marks), CoverageTable::from_iter(bases.into_iter().map(|(_, g)| g)), mark_array(records, &classes), BaseArray::new(base_records));
    true
}

fn filter_mark_mark(st: &mut MarkMarkPosFormat1, keep: &impl Fn(GlyphId16) -> bool) -> bool {
    let (marks, records) = kept_marks(&st.mark1_coverage, &st.mark1_array, keep);
    let bases = covered(&st.mark2_coverage, keep);
    if marks.is_empty() || bases.is_empty() {
        return false;
    }
    let classes = mark_classes(&records);
    let mark2: Vec<Mark2Record> = bases
        .iter()
        .map(|(i, _)| {
            let old = st.mark2_array.mark2_records.get(*i);
            Mark2Record::new(classes.keys().map(|&c| old.and_then(|r| r.mark2_anchors.get(c as usize)).and_then(|a| a.as_ref().cloned())).collect())
        })
        .collect();
    *st = MarkMarkPosFormat1::new(CoverageTable::from_iter(marks), CoverageTable::from_iter(bases.into_iter().map(|(_, g)| g)), mark_array(records, &classes), Mark2Array::new(mark2));
    true
}

fn filter_mark_lig(st: &mut MarkLigPosFormat1, keep: &impl Fn(GlyphId16) -> bool) -> bool {
    let (marks, records) = kept_marks(&st.mark_coverage, &st.mark_array, keep);
    let ligs = covered(&st.ligature_coverage, keep);
    if marks.is_empty() || ligs.is_empty() {
        return false;
    }
    let classes = mark_classes(&records);
    let attaches: Vec<LigatureAttach> = ligs
        .iter()
        .map(|(i, _)| {
            let old = st.ligature_array.ligature_attaches.get(*i);
            let components = old.map(|a| a.component_records.as_slice()).unwrap_or(&[]);
            LigatureAttach::new(components.iter().map(|comp| ComponentRecord::new(classes.keys().map(|&c| comp.ligature_anchors.get(c as usize).and_then(|a| a.as_ref().cloned())).collect())).collect())
        })
        .collect();
    *st = MarkLigPosFormat1::new(CoverageTable::from_iter(marks), CoverageTable::from_iter(ligs.into_iter().map(|(_, g)| g)), mark_array(records, &classes), LigatureArray::new(attaches));
    true
}
