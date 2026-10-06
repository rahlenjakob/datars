//! Vector tiles (docs/09-geo.md, "The basemap"). A `tiles` source is a PMTiles archive the host
//! serves by byte range; a `tiles` template node draws its features through per-layer templates.
//!
//! Tiles depend on the camera, and cameras move *between* resolved scenes (a flight is a motion
//! plan interpolating a view's camera). So a `tiles` node resolves to an empty placeholder group
//! plus a [`Binding`] (source, layer templates, projection, signals), and the frame pass
//! [`fill::fill_scene`] fills every placeholder of the scene about to be drawn — settled or
//! mid-flight — from the camera it has in that frame: the tiles covering the view at a zoom that
//! matches it, fetched through the sans-IO archive reader, decoded once, turned into nodes once
//! (templates resolved per feature and batched per style), placed by a transform and clipped to
//! their square. Missing tiles draw an ancestor meanwhile; labels are placed in screen space.
//!
//! Nothing here knows what a road or a lake is: layer names, filters and styles come from the
//! document (the std `basemap` recipe supplies them).

mod archive;
mod decode;
pub(crate) mod demand;
pub(crate) mod fill;
mod layers;
mod view;

pub(crate) use archive::Archive;
pub(crate) use decode::DecodedTile;
pub(crate) use fill::{fill_scene, FillCx, FillReport, LabelId, LabelMode};
pub(crate) use layers::{feature_geom, BuildEnv, LayerCx};

use crate::resolve::{Cx, Resolver};
use datars_expr::Value;
use datars_geo::pmtiles::ByteRange;
use datars_geo::{Projection, TileId};
use datars_ir::{SourceKind, TTiles};
use datars_math::Hash64;
use datars_scene::{KeyPath, NodeKind};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::Arc;

/// A synchronous range reader a host can install instead of answering `Request::Range`s one by
/// one: `(url, offset, length) → bytes` (fewer at the end of the file). The engine still does no IO
/// — like `pmtiles::Reader::get_with`, the host's function does.
pub type RangeFetch = dyn Fn(&str, u64, u64) -> Option<Vec<u8>>;

/// Decoded tiles kept in memory (LRU).
const MAX_DECODED: usize = 384;
/// Built tile node sets kept in memory (LRU; one per tile × zoom × binding).
const MAX_BUILT: usize = 384;

/// What a `tiles` node resolved to: everything the frame pass needs to fill its placeholder.
#[derive(Clone, Debug)]
pub(crate) struct Binding {
    pub source: String,
    pub spec: Rc<TTiles>,
    pub proj: Rc<Projection>,
    /// Signal values when the node resolved (layer templates evaluate against them).
    pub signals: Rc<BTreeMap<String, Value>>,
    /// Identity of what this binding draws — the spec, the projection and the values of the
    /// signals its templates read — so built tiles are reused across frames and states.
    pub fingerprint: u64,
}

/// Where a view looks at a tiles source in one frame — what an archive for the document must
/// hold there ([`crate::Engine::tile_views`]).
#[derive(Clone, Debug, PartialEq)]
pub struct TileView {
    pub source: String,
    /// Lon/lat `[west, south, east, north]` the view shows (within the Web-Mercator world).
    pub bbox: [f64; 4],
    /// The fractional tile zoom it asks for: tiles span the node's `tile_size` px at
    /// `round(zoom)` (`log2(world px / tile size)`).
    pub zoom: f64,
    /// The program state it settles in; empty for a frame of a flight.
    pub state: String,
    /// The reader can pan and zoom from here (the view's camera explores).
    pub explore: bool,
}

impl TileView {
    pub(crate) fn from_seen(s: &demand::Seen, state: &str, explore: bool) -> TileView {
        use datars_geo::tile::world_to_lonlat;
        let w = s.extent.world;
        let (nw, se) = (world_to_lonlat(datars_math::Vec2::new(w.x, w.y)), world_to_lonlat(datars_math::Vec2::new(w.x1(), w.y1())));
        TileView { source: s.source.clone(), bbox: [nw.x, se.y, se.x, nw.y], zoom: s.extent.zoom, state: state.to_string(), explore }
    }
}

/// Counters for inspection, tests and performance reports.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TileStats {
    /// Byte ranges requested (or fetched through a [`RangeFetch`]).
    pub requests: u64,
    /// Bytes handed to the engine.
    pub bytes: u64,
    /// Tiles decoded (MVT → geometry + feature tables), and their bytes.
    pub decoded: u64,
    pub decoded_bytes: u64,
    /// Tile node sets built (layer templates resolved per feature), and the features in them.
    pub built: u64,
    pub built_features: u64,
    /// Tiles drawn, summed over fills.
    pub drawn: u64,
    /// The last fill: tiles it wanted, drew from their own data, drew from an ancestor while
    /// theirs were pending or absent, still pending; labels placed.
    pub last_wanted: u32,
    pub last_ready: u32,
    pub last_fallback: u32,
    pub last_pending: u32,
    pub last_labels: u32,
}

/// A tiny LRU map with deterministic eviction (insertion/access stamps, oldest first). Stamps are
/// indexed too, so finding the oldest is a lookup, not a scan: a zoom step inserts hundreds of
/// cells into a cache of thousands.
pub(crate) struct Lru<K: Ord + Clone, V: Clone> {
    map: BTreeMap<K, (u64, V)>,
    by_stamp: BTreeMap<u64, K>,
    stamp: u64,
    cap: usize,
}

impl<K: Ord + Clone, V: Clone> Lru<K, V> {
    pub fn new(cap: usize) -> Lru<K, V> {
        Lru { map: BTreeMap::new(), by_stamp: BTreeMap::new(), stamp: 0, cap }
    }
    pub fn get(&mut self, k: &K) -> Option<V> {
        self.stamp += 1;
        let s = self.stamp;
        let e = self.map.get_mut(k)?;
        self.by_stamp.remove(&e.0);
        self.by_stamp.insert(s, k.clone());
        e.0 = s;
        Some(e.1.clone())
    }
    pub fn insert(&mut self, k: K, v: V) {
        while self.map.len() >= self.cap && !self.map.contains_key(&k) {
            let Some((_, oldest)) = self.by_stamp.pop_first() else { break };
            self.map.remove(&oldest);
        }
        self.stamp += 1;
        if let Some((old, _)) = self.map.insert(k.clone(), (self.stamp, v)) {
            self.by_stamp.remove(&old);
        }
        self.by_stamp.insert(self.stamp, k);
    }
    pub fn remove(&mut self, k: &K) -> Option<V> {
        let (s, v) = self.map.remove(k)?;
        self.by_stamp.remove(&s);
        Some(v)
    }
    /// Whether `k` is cached (without counting as a use).
    pub fn contains(&self, k: &K) -> bool {
        self.map.contains_key(k)
    }
    pub fn clear(&mut self) {
        self.map.clear();
        self.by_stamp.clear();
    }
    pub fn retain(&mut self, keep: impl Fn(&K) -> bool) {
        self.map.retain(|k, _| keep(k));
        self.by_stamp.retain(|_, k| keep(k));
    }
}

#[cfg(test)]
mod lru_tests {
    use super::Lru;

    #[test]
    fn evicts_the_least_recently_used() {
        let mut l = Lru::new(3);
        l.insert(1, "a");
        l.insert(2, "b");
        l.insert(3, "c");
        assert_eq!(l.get(&1), Some("a")); // 1 is now the most recent
        l.insert(4, "d"); // evicts 2
        assert_eq!(l.get(&2), None);
        l.insert(3, "c2"); // an update, no eviction
        l.insert(5, "e"); // evicts 1 (4 and 3 are more recent)
        assert_eq!((l.get(&1), l.get(&3), l.get(&4), l.get(&5)), (None, Some("c2"), Some("d"), Some("e")));
        l.retain(|k| *k != 4);
        l.insert(6, "f");
        l.insert(7, "g"); // 3 goes (5, 6 are newer)
        assert_eq!((l.get(&3), l.get(&5), l.get(&6), l.get(&7)), (None, Some("e"), Some("f"), Some("g")));
    }
}

/// Where a tile's bytes stand (any tile format).
pub(crate) enum Raw {
    Ready(Vec<u8>),
    /// Bytes are requested (or the archive is still opening).
    Pending,
    /// The archive has no such tile (or can't be read).
    Absent,
}

/// Where a tile stands.
pub(crate) enum Lookup {
    Ready(Arc<DecodedTile>),
    /// Bytes are requested (or the archive is still opening).
    Pending,
    /// Here, but this frame's decode budget is spent: next frame.
    Later,
    /// The archive has no such tile (or can't be read).
    Absent,
}

/// Tile bytes (decompressed) decoded per frame at most, beyond the first tile: a camera flight
/// arriving at street level decodes a few tiles a frame, the coarser ones standing in meanwhile,
/// instead of freezing a frame (or several) on all of them — and the renderer tessellates what
/// arrives a few meshes at a time too.
pub(crate) const DECODE_PER_FRAME: usize = 64 * 1024;

/// Features styled per frame (tiles at a zoom they haven't been drawn at, at least one a frame);
/// the rest keep a nearby zoom's styling, or an ancestor's, a frame or two longer. A zoom crossing
/// a whole level would otherwise restyle every tile on screen at once — and hand the renderer all
/// their new meshes in one frame. Counted in features, not tiles: styling costs about half a
/// microsecond a feature, and a street tile holds thousands (three of Rio's took 11 ms). A tile
/// whose geometry is heavier counts half its path elements instead: its first draw tessellates
/// them (a street tile's buildings, one path of 38,000 elements, took 12 ms), so nothing else
/// joins it in that frame.
pub(crate) const BUILD_FEATURES_PER_FRAME: usize = 8000;

/// Build-cache key: binding fingerprint, source, data tile, zoom drawn at.
type BuiltKey = (u64, String, TileId, u8);

/// Every tile source of the loaded document, and the caches. Lives in a `RefCell` in the engine:
/// filling a frame reads caches and records requests from `&self` paths (`plan_at`).
pub(crate) struct TileState {
    pub archives: BTreeMap<String, Archive>,
    /// Point pyramids (`instances` with `lod`): archives above hold their tiles too.
    pub points: crate::points::Store,
    decoded: Lru<(String, TileId), Arc<DecodedTile>>,
    absent: BTreeSet<(String, TileId)>,
    built: Lru<BuiltKey, Rc<layers::TileNodes>>,
    pub stats: TileStats,
    pub diags: Vec<String>,
    /// This frame's decode budget in bytes (`None`: decode everything — a still render), and
    /// tiles decoded so far this frame.
    pub decode_left: Option<usize>,
    pub decoded_now: u32,
    /// This frame's styling budget in features (`None`: style everything), and tiles styled so far
    /// this frame.
    pub builds_left: Option<usize>,
    pub built_now: u32,
    /// Decoding and styling one big tile in the same frame took 26 ms in a browser, so they take
    /// turns: `decoded_heavy` — this frame decoded a tile bigger than its decode budget (the
    /// frame's one-tile allowance), and a tile too heavy for the styling budget waits a frame (its
    /// neighbour zoom or ancestor standing in), setting `style_waits`; the next frame then styles
    /// first (`style_first`: no tile over the decode budget).
    pub decoded_heavy: bool,
    pub style_waits: bool,
    pub style_first: bool,
    /// How far along the transition in flight this frame is (live frames only): a tile too big
    /// for a frame's decode budget waits for the landing, its ancestor standing in.
    pub flight: Option<f64>,
    /// Tile bytes that arrived but wait for a frame with budget to decode them (or were
    /// prefetched): the archive reader hands a tile's bytes out once, and dropped they'd be
    /// fetched again.
    waiting: Lru<(String, TileId), Arc<Vec<u8>>>,
    /// Expressions compiled for tiles and point cells, shared by their resolvers (see
    /// [`TileState::exprs`]).
    exprs: crate::resolve::SharedExprs,
    /// Per binding (fingerprint), what each visible square drew last live frame, and the squares
    /// whose finer data is fading in over what they drew before (see `fill::FADE_S`).
    pub(crate) shown: BTreeMap<u64, BTreeMap<TileId, fill::Shown>>,
    pub(crate) fades: BTreeMap<(u64, TileId), fill::Fade>,
}

/// Tiles whose bytes can wait to be decoded (past that, the least recently wanted are dropped and
/// fetched again when needed).
const MAX_WAITING: usize = 512;

/// From this far into a transition, tiles too big for a frame's budget decode anyway (one a
/// frame): the camera slows to land, and the detail shows.
pub(crate) const LANDING: f64 = 0.85;

impl TileState {
    /// A tile of `bytes` must wait for a later frame: this frame's budget is spent (after at least
    /// one tile), or it's too big to decode mid-flight at all.
    pub fn over_budget(&self, bytes: usize) -> bool {
        self.decode_left.is_some_and(|left| bytes > left && (self.decoded_now > 0 || self.style_first || self.flight.is_some_and(|t| t < LANDING)))
    }

    /// A tile's bytes: waiting ones first, else from the archive (see [`TileState::raw`]).
    pub fn raw_or_waiting(&mut self, source: &str, t: TileId, fetch: Option<&RangeFetch>) -> Raw {
        match self.waiting.remove(&(source.to_string(), t)) {
            Some(b) => Raw::Ready(Arc::try_unwrap(b).unwrap_or_else(|b| (*b).clone())),
            None => self.raw(source, t, fetch),
        }
    }

    /// Keep bytes that can't be decoded this frame for a later one.
    pub fn wait(&mut self, source: &str, t: TileId, bytes: Vec<u8>) {
        self.waiting.insert((source.to_string(), t), Arc::new(bytes));
    }

    /// The compiled expressions resolvers made for tiles and point cells share.
    pub(crate) fn exprs(&self) -> crate::resolve::SharedExprs {
        if self.exprs.borrow().len() > MAX_EXPRS {
            self.exprs.borrow_mut().clear();
        }
        self.exprs.clone()
    }
}

/// Compiled expressions a [`TileState`] keeps at most (a document's templates compile a few dozen;
/// this only bounds sources built from data).
const MAX_EXPRS: usize = 4096;

impl TileState {
    pub fn new(doc: &datars_ir::Doc) -> TileState {
        let archives = doc.data.iter().filter_map(|(name, s)| match &s.from {
            SourceKind::Tiles(url) => Some((name.clone(), Archive::new(&datars_ir::tiles_file(name, url)))),
            _ => None,
        });
        TileState { archives: archives.collect(), points: crate::points::Store::new(), decoded: Lru::new(MAX_DECODED), absent: BTreeSet::new(), built: Lru::new(MAX_BUILT), stats: TileStats::default(), diags: Vec::new(), decode_left: None, decoded_now: 0, builds_left: None, built_now: 0, decoded_heavy: false, style_waits: false, style_first: false, flight: None, waiting: Lru::new(MAX_WAITING), exprs: Default::default(), shown: BTreeMap::new(), fades: BTreeMap::new() }
    }

    /// Byte ranges still wanted, per source.
    pub fn requests(&self) -> Vec<crate::Request> {
        let mut out = Vec::new();
        for (name, a) in &self.archives {
            for r in a.pending() {
                out.push(crate::Request::Range { name: name.clone(), url: a.url.clone(), offset: r.offset, length: r.length });
            }
        }
        out
    }

    /// Bytes for a range the engine asked for. Tiles waiting on it are decoded now.
    pub fn provide(&mut self, source: &str, offset: u64, bytes: &[u8]) -> Result<(), String> {
        let a = self.archives.get_mut(source).ok_or_else(|| format!("`{source}` is not a tiles source"))?;
        self.stats.bytes += bytes.len() as u64;
        let waiting = a.provide(offset, bytes)?;
        // Decode what waited on these bytes now (the reader holds provided payloads only until
        // the next lookup of a tile stored there): vector tiles, or point tiles.
        let points = a.tile_type().is_some_and(|t| t != datars_geo::pmtiles::TileType::Mvt);
        for t in waiting {
            if points {
                crate::points::fill::archive_tile(self, source, t, None);
            } else {
                self.lookup(source, t, None);
            }
        }
        Ok(())
    }

    /// The whole archive at once (instead of ranges).
    pub fn provide_whole(&mut self, source: &str, bytes: Vec<u8>) -> Result<(), String> {
        let a = self.archives.get_mut(source).ok_or_else(|| format!("`{source}` is not a tiles source"))?;
        self.stats.bytes += bytes.len() as u64;
        a.set_whole(bytes);
        Ok(())
    }

    /// Read `source` from another archive from now on (a rebuilt one: `datars dev` fills an
    /// automatic basemap in as its data arrives). Everything read from the old one is dropped.
    pub fn reset(&mut self, source: &str, url: &str) -> Result<(), String> {
        let a = self.archives.get_mut(source).ok_or_else(|| format!("`{source}` is not a tiles source"))?;
        *a = Archive::new(url);
        self.absent.retain(|k| k.0 != source);
        self.decoded.retain(|k| k.0 != source);
        self.built.retain(|k| k.1 != source);
        Ok(())
    }

    /// The archive's zoom range, once its header is in.
    pub fn zoom_range(&self, source: &str) -> Option<(u8, u8)> {
        self.archives.get(source)?.zoom_range()
    }

    /// A decoded tile if it's in memory (no request).
    pub fn cached(&mut self, source: &str, t: TileId) -> Option<Arc<DecodedTile>> {
        self.decoded.get(&(source.to_string(), t))
    }

    /// Advance a tile's lookup as far as possible: decode it if its bytes are in, request what's
    /// missing otherwise (or fetch it synchronously through `fetch`).
    pub fn lookup(&mut self, source: &str, t: TileId, fetch: Option<&RangeFetch>) -> Lookup {
        let key = (source.to_string(), t);
        if let Some(d) = self.decoded.get(&key) {
            return Lookup::Ready(d);
        }
        if self.absent.contains(&key) {
            return Lookup::Absent;
        }
        match self.raw_or_waiting(source, t, fetch) {
            Raw::Ready(bytes) if self.over_budget(bytes.len()) => {
                self.wait(source, t, bytes);
                Lookup::Later
            }
            Raw::Ready(bytes) => match decode::decode(t, &bytes) {
                Ok(d) => {
                    // At least one tile a frame, so a big one still gets through.
                    self.decoded_heavy |= self.decode_left.is_some_and(|left| bytes.len() > left);
                    self.decode_left = self.decode_left.map(|left| left.saturating_sub(bytes.len()));
                    self.decoded_now += 1;
                    self.stats.decoded += 1;
                    self.stats.decoded_bytes += bytes.len() as u64;
                    let d = Arc::new(d);
                    self.decoded.insert(key, d.clone());
                    Lookup::Ready(d)
                }
                Err(e) => {
                    self.diags.push(format!("tiles `{source}` {}/{}/{}: {e}", t.z, t.x, t.y));
                    self.absent.insert(key);
                    Lookup::Absent
                }
            },
            Raw::Pending => Lookup::Pending,
            Raw::Absent => {
                self.absent.insert(key);
                Lookup::Absent
            }
        }
    }

    /// A tile's bytes (decompressed), as far as the archive can get them now: read from the
    /// archive in memory, fetched through `fetch`, or requested from the host (`Pending`).
    pub fn raw(&mut self, source: &str, t: TileId, fetch: Option<&RangeFetch>) -> Raw {
        let Some(a) = self.archives.get_mut(source) else { return Raw::Absent };
        // Header, up to three leaf levels, the tile: a handful of steps at most.
        for _ in 0..8 {
            match a.step(t) {
                archive::Step::Needs(range) => {
                    // The whole archive in memory (bundled in an app, provided at once).
                    if let Some(bytes) = a.slice(range) {
                        if let Err(e) = a.provide(range.offset, &bytes) {
                            self.diags.push(format!("tiles `{source}`: {e}"));
                            return Raw::Absent;
                        }
                        continue;
                    }
                    if let Some(f) = fetch {
                        self.stats.requests += 1;
                        match f(&a.url, range.offset, range.length) {
                            Some(bytes) => {
                                self.stats.bytes += bytes.len() as u64;
                                if let Err(e) = a.provide(range.offset, &bytes) {
                                    self.diags.push(format!("tiles `{source}`: {e}"));
                                    return Raw::Absent;
                                }
                                continue;
                            }
                            None => {
                                a.fail(format!("{} bytes at {} not available", range.length, range.offset));
                                self.diags.push(format!("tiles `{source}`: {} can't be read", a.url));
                                return Raw::Absent;
                            }
                        }
                    }
                    if a.want(range, t) {
                        self.stats.requests += 1;
                    }
                    return Raw::Pending;
                }
                archive::Step::Ready(bytes) => return Raw::Ready(bytes),
                archive::Step::Absent => return Raw::Absent,
                archive::Step::Failed(e) => {
                    let msg = format!("tiles `{source}`: {e}");
                    if !self.diags.contains(&msg) {
                        self.diags.push(msg);
                    }
                    return Raw::Absent;
                }
            }
        }
        Raw::Pending
    }

    /// Drop built nodes (styles changed: signals, theme, mode). Decoded tiles stay.
    pub fn clear_built(&mut self) {
        self.built.clear();
        self.points.clear_built();
    }
}

/// Resolve a `tiles` template node: an empty group now, filled per frame (see the module docs).
pub(crate) fn resolve(r: &Resolver, t: &TTiles, cx: &Cx, path: &KeyPath) -> Option<NodeKind> {
    let Some(proj) = cx.proj.clone() else {
        r.diag(format!("tiles `{}` outside a geo coordinate system", t.source));
        return None;
    };
    if !matches!(r.doc.data.get(&t.source).map(|s| &s.from), Some(SourceKind::Tiles(_))) {
        r.diag(format!("`{}` is not a tiles source", t.source));
        return None;
    }
    let mut h = Hash64::new();
    h.str(&serde_json::to_string(t).unwrap_or_default());
    h.str(&serde_json::to_string(&*proj).unwrap_or_default());
    // The signals the layer templates read (not the others: hovering elsewhere mustn't rebuild).
    let mut srcs = Vec::new();
    expressions(&serde_json::to_value(t).unwrap_or_default(), &mut srcs);
    let mut read = BTreeSet::new();
    for s in &srcs {
        if let Some(c) = r.compiled(s) {
            read.extend(c.signals());
        }
    }
    for name in &read {
        h.str(name);
        h.str(&format!("{:?}", r.signals.get(name)));
    }
    let b = Binding { source: t.source.clone(), spec: Rc::new(t.clone()), proj, signals: Rc::new(r.signals.clone()), fingerprint: h.finish() };
    r.tiles.borrow_mut().push((path.to_string(), Rc::new(b)));
    Some(NodeKind::Group { children: Vec::new() })
}

/// Expression sources in a JSON template (`"=…"` strings and `{"expr": …}` objects).
fn expressions(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::String(s) if s.starts_with('=') => out.push(s[1..].to_string()),
        serde_json::Value::Object(o) => match o.get("expr").and_then(|e| e.as_str()) {
            Some(e) if o.len() == 1 => out.push(e.to_string()),
            _ => o.values().for_each(|x| expressions(x, out)),
        },
        serde_json::Value::Array(a) => a.iter().for_each(|x| expressions(x, out)),
        _ => {}
    }
}

/// The byte range a reader needs first (header + root directory).
pub(crate) fn header_range() -> ByteRange {
    ByteRange::new(0, datars_geo::pmtiles::PREFIX_LEN)
}

#[cfg(test)]
pub(crate) mod testutil;

#[cfg(test)]
mod tests;
