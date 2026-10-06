//! `datars-runtime` — the installed runtime's core, shared by every host (web, iOS/macOS, Android,
//! desktop): an engine plus the bundle loader and content store, and the status hosts render chrome
//! from. Hosts add only a surface, a clock, input, networking and the accessibility bridge.

use datars_bundle::{Capabilities, ContentStore, Loader, MemStore};
pub use datars_engine;
use datars_engine::{Engine, Pointer};
use std::sync::Arc;

/// Platform-independent core of the web view (unit-testable natively).
pub struct Core {
    pub engine: Engine,
    loader: Option<Loader>,
    store: MemStore,
    caps: Capabilities,
    pub tier: Option<datars_bundle::Tier>,
    pub poster: Option<String>,
    pub text: Option<serde_json::Value>,
    /// From the last frame: when the next frame is due without input (autoplay, live refresh).
    pub wake_at: Option<f64>,
    /// Lazily loaded font chunks already asked for (script subsets for runtime text).
    lazy_fonts: std::collections::BTreeSet<String>,
}

impl Default for Core {
    fn default() -> Self {
        Core::new(true, Vec::new())
    }
}

impl Core {
    pub fn new(allow_script: bool, publishers: Vec<String>) -> Core {
        Core::with_engine(Engine::new(), allow_script, publishers)
    }

    /// A core whose engine has no built-in fonts, whatever this build carries (tests of what a
    /// web runtime does).
    pub fn without_builtin_fonts(allow_script: bool, publishers: Vec<String>) -> Core {
        Core::with_engine(Engine::without_builtin_fonts(), allow_script, publishers)
    }

    fn with_engine(engine: Engine, allow_script: bool, publishers: Vec<String>) -> Core {
        Core { engine, loader: None, store: MemStore::default(), caps: datars_engine::bundle::capabilities(allow_script, publishers), tier: None, poster: None, text: None, wake_at: None, lazy_fonts: Default::default() }
    }

    pub fn load_doc_json(&mut self, json: &str) -> Result<Vec<String>, String> {
        let (doc, notes) = datars_ir::Doc::from_json_checked(json)?;
        let mut out: Vec<String> = self.engine.load(doc).into_iter().map(|d| d.message).collect();
        // A source document is the programmable variant: the view is ready to render it.
        self.tier = Some(datars_bundle::Tier::T3);
        self.loader = None;
        self.engine.add_diagnostics(notes.clone());
        out.extend(notes);
        Ok(out)
    }

    /// A new version of a source document, morphing in from what is on screen (the edit loop).
    pub fn reload_doc_json(&mut self, json: &str) -> Result<Vec<String>, String> {
        let (doc, notes) = datars_ir::Doc::from_json_checked(json)?;
        let mut out: Vec<String> = self.engine.reload(doc).into_iter().map(|d| d.message).collect();
        self.tier = Some(datars_bundle::Tier::T3);
        self.loader = None;
        self.engine.add_diagnostics(notes.clone());
        out.extend(notes);
        Ok(out)
    }

    /// Open a manifest; returns the chunk hashes to fetch.
    pub fn open_manifest(&mut self, manifest: &[u8]) -> Result<Vec<String>, String> {
        let l = Loader::open(manifest, &self.caps, &self.store).map_err(|e| e.to_string())?;
        let req = l.requests();
        self.loader = Some(l);
        self.try_finish()?;
        Ok(req)
    }

    /// Open a single-file `.datars` bundle.
    pub fn open_file(&mut self, bytes: &[u8]) -> Result<(), String> {
        let b = datars_bundle::from_single_file(bytes).map_err(|e| e.to_string())?;
        for (h, c) in &b.chunks {
            self.store.put(h, Arc::from(c.clone()));
        }
        self.open_manifest(&b.manifest.to_json()).map(|_| ())
    }

    /// Provide a fetched chunk; returns the chunks still needed (empty = ready). A lazily loaded
    /// font chunk arriving after the chart opened goes straight to the engine.
    pub fn provide_chunk(&mut self, hash: &str, bytes: &[u8]) -> Result<Vec<String>, String> {
        let l = self.loader.as_mut().ok_or("no bundle is being loaded")?;
        l.provide(hash, Arc::from(bytes.to_vec()), &mut self.store).map_err(|e| e.to_string())?;
        let lazy_font = l.manifest.chunk(hash).filter(|c| c.kind == "font" && c.lazy).map(|c| c.meta.clone());
        let req = l.requests();
        self.try_finish()?;
        if let (Some(meta), Some(tier)) = (lazy_font, self.tier) {
            datars_engine::bundle::add_font_chunk(&mut self.engine, &meta, bytes, tier == datars_bundle::Tier::T1)?;
        }
        Ok(req)
    }

    /// After a frame: text the engine laid out without glyphs (runtime data in a script the
    /// eager font subsets don't cover) asks for the bundle's lazily loaded script subsets, which
    /// then appear in [`Core::requests`]. Hosts call this after rendering (frame_pixels does).
    pub fn after_frame(&mut self) {
        let (Some(l), Some(_)) = (self.loader.as_mut(), self.tier) else { return };
        let missing = self.engine.missing_chars();
        if missing.is_empty() {
            return;
        }
        let Some(entry) = l.entry().cloned() else { return };
        let mut here = Vec::new();
        for h in datars_engine::bundle::lazy_fonts_for(&l.manifest, &entry, &missing) {
            if self.lazy_fonts.insert(h.clone()) {
                // A single-file bundle (or a warm cache) already has it: no request needed.
                match (l.chunk(&h), l.manifest.chunk(&h)) {
                    (Some(bytes), Some(c)) => here.push((c.meta.clone(), bytes)),
                    _ => {
                        l.want(&h);
                    }
                }
            }
        }
        let baked = self.tier == Some(datars_bundle::Tier::T1);
        for (meta, bytes) in here {
            let _ = datars_engine::bundle::add_font_chunk(&mut self.engine, &meta, &bytes, baked);
        }
    }

    fn try_finish(&mut self) -> Result<(), String> {
        let Some(l) = &self.loader else { return Ok(()) };
        if l.entry().is_some() && self.poster.is_none() {
            let (p, t) = datars_engine::bundle::fallback(l);
            self.poster = p;
            self.text = t;
        }
        if l.is_ready() && self.tier.is_none() {
            self.tier = Some(datars_engine::bundle::open_loader(&mut self.engine, l)?);
        }
        Ok(())
    }

    pub fn pointer(&mut self, kind: &str, x: f64, y: f64) -> Option<String> {
        let p = match kind {
            "move" => Pointer::Move { x, y },
            "down" => Pointer::Down { x, y },
            "up" => Pointer::Up { x, y },
            "tap" => Pointer::Tap { x, y },
            _ => Pointer::Leave,
        };
        self.engine.pointer(p)
    }

    /// Wheel/pinch zoom at (x, y); true if an explorable view took it.
    pub fn wheel(&mut self, x: f64, y: f64, delta: f64) -> bool {
        self.engine.wheel(datars_math::Vec2::new(x, y), delta)
    }

    pub fn set_mode(&mut self, mode: &str) {
        self.engine.set_mode(match mode {
            "dark" => datars_theme::Mode::Dark,
            "high-contrast" => datars_theme::Mode::HighContrast,
            _ => datars_theme::Mode::Light,
        });
    }

    /// Status for the page: state, narration, semantics, theme tokens (for host chrome).
    pub fn status(&mut self) -> serde_json::Value {
        let mut v = self.status_brief();
        v["semantics"] = serde_json::Value::Array(self.semantics());
        v
    }

    /// Every semantic item of the current frame (the accessible tree): `[{role, label, depth,
    /// rect, path, actionable}]` — for big data, thousands of them.
    pub fn semantics(&mut self) -> Vec<serde_json::Value> {
        if self.tier == Some(datars_bundle::Tier::T0) {
            return Vec::new();
        }
        self.engine.semantic_items().into_iter().map(semantic_json).collect()
    }

    /// The semantic items a reader can activate (`actionable`, in [`Core::semantics`]' shape):
    /// what a host offers as buttons to keyboards and screen readers. Cheap, so part of
    /// [`Core::status_brief`].
    pub fn actions(&mut self) -> Vec<serde_json::Value> {
        if self.tier == Some(datars_bundle::Tier::T0) {
            return Vec::new();
        }
        self.engine.actionable_items().into_iter().map(semantic_json).collect()
    }

    /// [`Core::status`] without the semantics: what a host's chrome needs on every step (state,
    /// steps, narration, controls, what a click acts on, theme tokens), cheap however many marks
    /// the chart has.
    pub fn status_brief(&mut self) -> serde_json::Value {
        let actions = self.actions();
        serde_json::json!({
            "state": self.engine.state(),
            "index": self.engine.state_index(),
            "states": self.engine.state_names(),
            "narration": self.engine.narration(),
            "narrationDrawn": self.engine.draws_narration(),
            "controls": self.engine.controls().into_iter().map(|c| match c.kind {
                "select" => serde_json::json!({ "kind": "select", "signal": c.signal, "label": c.label, "current": c.current, "rect": c.rect,
                    "options": c.options.iter().map(|(v, l)| serde_json::json!({ "value": v, "label": l })).collect::<Vec<_>>() }),
                _ => serde_json::json!({ "kind": "slider", "signal": c.signal, "label": c.label, "min": c.min, "max": c.max, "step": c.step, "value": c.value, "rect": c.rect }),
            }).collect::<Vec<_>>(),
            "actions": actions,
            "tier": self.tier.map(|t| format!("{t:?}")),
            "tokens": self.engine.theme().to_json(),
            "diagnostics": self.engine.diagnostics().iter().map(|d| d.message.clone()).collect::<Vec<_>>(),
        })
    }
}

/// A [`datars_engine::SemanticItem`] as hosts read it.
fn semantic_json(s: datars_engine::SemanticItem) -> serde_json::Value {
    serde_json::json!({ "role": s.role, "label": s.label, "depth": s.depth, "rect": [s.rect.x, s.rect.y, s.rect.w, s.rect.h], "path": s.path, "actionable": s.actionable })
}

impl Core {
    /// Where the views on screen look at their tile sources, as JSON `[{source, bbox, zoom, state,
    /// explore}]` — a dev server fetches map data for where a reader explores
    /// ([`datars_engine::Engine::tile_views_now`]).
    pub fn tile_views_json(&mut self) -> String {
        let v: Vec<serde_json::Value> = self.engine.tile_views_now().into_iter().map(|t| serde_json::json!({ "source": t.source, "bbox": t.bbox, "zoom": t.zoom, "state": t.state, "explore": t.explore })).collect();
        serde_json::Value::Array(v).to_string()
    }

    /// Render the current frame with the CPU reference rasterizer (RGBA8 straight alpha) — the
    /// portable surface for hosts without a GPU path yet (and for exact screenshots).
    pub fn frame_pixels(&mut self, now: f64, dpr: f64) -> (bool, datars_render_cpu::Pixmap) {
        let out = self.engine.frame(now);
        self.wake_at = out.wake_at;
        let px = datars_render_cpu::render(&out.display, self.engine.fonts(), dpr);
        self.after_frame();
        (out.animating, px)
    }

    pub fn requests(&self) -> Vec<String> {
        self.loader.as_ref().map(|l| l.requests()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_brief_status_offers_what_a_click_acts_on() {
        // Hosts build their keyboard buttons from it, without the whole semantics tree.
        let mut core = Core::default();
        let doc = r#"{"datars": 1, "size": {"width": 200, "height": 100}, "signals": {"s": {"type": "string", "default": ""}},
            "scene": {"kind": "group", "key": "root", "children": [
              {"kind": "shape", "key": "go", "geom": {"type": "rect", "x": 0, "y": 0, "w": 50, "h": 50}, "fill": "$accent",
               "semantics": {"role": "control", "label": "=s == \"on\" ? \"Turn off\" : \"Turn on\""}, "on": {"activate": {"set": "s", "value": "on"}}},
              {"kind": "shape", "key": "mark", "geom": {"type": "rect", "x": 60, "y": 0, "w": 50, "h": 50}, "fill": "$ink",
               "semantics": {"role": "datum", "label": "a mark"}}]}}"#;
        core.load_doc_json(doc).unwrap();
        core.engine.frame(0.0);
        let brief = core.status_brief();
        let actions = brief["actions"].as_array().unwrap();
        assert_eq!(actions.len(), 1, "{actions:?}");
        assert_eq!(actions[0]["label"], "Turn on");
        assert_eq!(actions[0]["actionable"], true);
        assert!(core.engine.activate(actions[0]["path"].as_str().unwrap()));
        assert_eq!(core.status_brief()["actions"][0]["label"], "Turn off", "the label follows the state");
        assert!(core.semantics().len() > 1, "the full tree still has the rest");
    }

    #[test]
    fn tile_archives_ask_for_their_header_first() {
        let mut core = Core::default();
        let doc = r#"{"datars": 1, "data": {"base": {"tiles": "tiles/x.pmtiles"}},
            "scene": {"kind": "view", "coord": {"type": "geo", "projection": "web-mercator"}, "children": [{"kind": "tiles", "source": "base", "layers": []}]}}"#;
        core.load_doc_json(doc).unwrap();
        let r = core.engine.requests();
        assert_eq!(r, vec![datars_engine::Request::Range { name: "base".into(), url: "tiles/x.pmtiles".into(), offset: 0, length: 16384 }]);
        // Not a PMTiles archive: the engine reports it and stops asking.
        core.engine.provide_range("base", 0, b"not an archive").unwrap_err();
        assert!(core.engine.requests().is_empty());
    }
}
