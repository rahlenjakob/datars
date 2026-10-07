//! `datars-host-web` — the engine in the browser (docs/10-platforms.md). The page (`@datars/web`)
//! owns the DOM, the clock, fetching, and the accessibility mirror; this crate owns everything that
//! decides pixels. A bundle is loaded through the sans-IO loader: the page fetches what `requests()`
//! asks for and hands bytes back.

pub use datars_runtime::Core;

#[cfg(target_arch = "wasm32")]
mod web {
    use super::Core;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::Clamped;

    enum Surface {
        #[cfg(feature = "gpu")]
        Gpu(datars_render_wgpu::Renderer),
        Cpu(web_sys::CanvasRenderingContext2d),
        None,
    }

    #[wasm_bindgen]
    pub struct View {
        core: Core,
        surface: Surface,
        canvas: Option<web_sys::HtmlCanvasElement>,
        dpr: f64,
        /// The last frame's cost in ms: [engine, raster (GPU encode/submit or CPU raster + blit)],
        /// its display-list length and the tiles it drew without (still downloading) — for the
        /// page's frame profiler.
        timing: [f64; 6],
        /// The scene the last frame drew (tiles and point levels filled in): what
        /// [`View::text_layer`] describes. Kept rather than dropped at the end of the frame — the
        /// next frame replaces it, so it costs no copy.
        drawn: Option<datars_scene::Scene>,
        /// Settled frames drawn since the last moving one while stand-in meshes remained.
        settling: u32,
    }

    /// The page clock, for timing frames (never read by the engine: P1).
    fn clock_ms() -> f64 {
        web_sys::window().and_then(|w| w.performance()).map_or(0.0, |p| p.now())
    }

    /// CSS px × dpr → the canvas's physical pixel size.
    fn physical(css: f64, dpr: f64) -> u32 {
        (css * dpr).round().max(1.0) as u32
    }

    fn js<E: std::fmt::Display>(e: E) -> JsValue {
        JsValue::from_str(&e.to_string())
    }

    #[wasm_bindgen]
    impl View {
        #[wasm_bindgen(constructor)]
        pub fn new(allow_script: bool, publishers: Vec<String>) -> View {
            View { core: Core::new(allow_script, publishers), surface: Surface::None, canvas: None, dpr: 1.0, timing: [0.0; 6], drawn: None, settling: 0 }
        }

        /// Attach a canvas: WebGPU (or WebGL2) through wgpu; the CPU reference into a 2D context if
        /// no GPU path is available.
        pub async fn attach(&mut self, canvas: web_sys::HtmlCanvasElement, width: f64, height: f64, dpr: f64, prefer_cpu: bool) -> Result<String, JsValue> {
            self.dpr = dpr;
            canvas.set_width(physical(width, dpr));
            canvas.set_height(physical(height, dpr));
            self.canvas = Some(canvas.clone());
            self.core.engine.resize(width, height, dpr);
            #[cfg(not(feature = "gpu"))]
            let _ = prefer_cpu;
            #[cfg(feature = "gpu")]
            if !prefer_cpu {
                match datars_render_wgpu::Renderer::for_canvas(canvas.clone(), dpr).await {
                    Ok(r) => {
                        self.surface = Surface::Gpu(r);
                        return Ok("gpu".into());
                    }
                    Err(e) => web_sys::console::warn_1(&JsValue::from_str(&format!("datars: GPU unavailable ({e}); using the CPU renderer"))),
                }
            }
            let ctx = canvas.get_context("2d")?.ok_or("no 2d context")?.dyn_into::<web_sys::CanvasRenderingContext2d>()?;
            self.surface = Surface::Cpu(ctx);
            Ok("cpu".into())
        }

        pub fn load_doc(&mut self, json: &str) -> Result<JsValue, JsValue> {
            let d = self.core.load_doc_json(json).map_err(js)?;
            Ok(serde_wasm(&serde_json::json!(d)))
        }
        /// A new version of the document, morphing in from what is on screen (`datars dev`).
        pub fn reload_doc(&mut self, json: &str) -> Result<JsValue, JsValue> {
            let d = self.core.reload_doc_json(json).map_err(js)?;
            Ok(serde_wasm(&serde_json::json!(d)))
        }
        pub fn open_manifest(&mut self, bytes: &[u8]) -> Result<Vec<String>, JsValue> {
            self.core.open_manifest(bytes).map_err(js)
        }
        pub fn open_file(&mut self, bytes: &[u8]) -> Result<(), JsValue> {
            self.core.open_file(bytes).map_err(js)
        }
        pub fn provide_chunk(&mut self, hash: &str, bytes: &[u8]) -> Result<Vec<String>, JsValue> {
            self.core.provide_chunk(hash, bytes).map_err(js)
        }
        pub fn provide_source(&mut self, name: &str, bytes: &[u8]) -> Result<(), JsValue> {
            self.core.engine.provide(name, bytes).map_err(js)
        }
        /// Bytes for a `range` request from [`View::data_requests`] (`offset` as a JS number:
        /// archives stay below 2^53 bytes).
        pub fn provide_range(&mut self, name: &str, offset: f64, bytes: &[u8]) -> Result<(), JsValue> {
            self.core.engine.provide_range(name, offset as u64, bytes).map_err(js)
        }
        /// Read a tiles source from another archive (`datars dev` swaps in an automatic basemap
        /// rebuilt as its data arrives; the view keeps its state and where it was explored to).
        pub fn set_tiles_url(&mut self, name: &str, url: &str) -> Result<(), JsValue> {
            self.core.engine.set_tiles_url(name, url).map_err(js)
        }
        /// Where the views on screen look at their tile sources, as JSON (see `Core::tile_views_json`).
        pub fn tile_views(&mut self) -> String {
            self.core.tile_views_json()
        }
        pub fn poster(&self) -> Option<String> {
            self.core.poster.clone()
        }
        pub fn ready(&self) -> bool {
            self.core.tier.is_some()
        }
        pub fn resize(&mut self, width: f64, height: f64, dpr: f64) {
            self.dpr = dpr;
            self.core.engine.resize(width, height, dpr);
            if let Some(c) = &self.canvas {
                c.set_width(physical(width, dpr));
                c.set_height(physical(height, dpr));
            }
            #[cfg(feature = "gpu")]
            if let Surface::Gpu(r) = &mut self.surface {
                r.resize(physical(width, dpr), physical(height, dpr), dpr);
            }
        }
        /// The page's clock (`performance.now()`), before input: transitions start now, not at the
        /// last frame rendered (the page renders on demand, so that can be long ago).
        pub fn set_clock(&mut self, now_ms: f64) {
            self.core.engine.set_clock(now_ms / 1000.0);
        }
        /// Render a frame at `now_ms`; returns whether anything is still moving. When nothing is,
        /// [`View::wake_at`] says when the next frame is due anyway.
        pub fn frame(&mut self, now_ms: f64) -> Result<bool, JsValue> {
            let t0 = clock_ms();
            let converted = self.core.engine.instances_converted();
            let out = self.core.engine.frame(now_ms / 1000.0);
            let converted = self.core.engine.instances_converted() - converted;
            let t1 = clock_ms();
            self.core.wake_at = out.wake_at;
            match &mut self.surface {
                #[cfg(feature = "gpu")]
                Surface::Gpu(r) => {
                    r.set_moving(out.animating);
                    r.render(&out.display, self.core.engine.fonts()).map_err(js)?
                }
                Surface::Cpu(ctx) => {
                    let px = datars_render_cpu::render(&out.display, self.core.engine.fonts(), self.dpr);
                    let img = web_sys::ImageData::new_with_u8_clamped_array_and_sh(Clamped(&px.data), px.width, px.height)?;
                    ctx.put_image_data(&img, 0.0, 0.0)?;
                }
                Surface::None => {}
            }
            // Instances rebuilt: the renderer's (built and uploaded), or the engine's on the CPU;
            // and the renderer's meshes tessellated.
            let (rebuilt, tessellated) = match &self.surface {
                #[cfg(feature = "gpu")]
                Surface::Gpu(r) => (r.stats().instances_rebuilt as u64, r.stats().tessellated),
                _ => (converted, 0),
            };
            self.timing = [t1 - t0, clock_ms() - t1, out.display.ops.len() as f64, out.pending_tiles as f64, rebuilt as f64, tessellated as f64];
            // Text without glyphs asks for the bundle's lazily loaded script subsets.
            self.core.after_frame();
            self.drawn = Some(out.scene);
            // A settled frame drawn partly with stand-in meshes isn't final: frames keep coming,
            // each building its share of exact meshes, until none stand in (bounded, in case).
            #[cfg(feature = "gpu")]
            if !out.animating {
                if let Surface::Gpu(r) = &self.surface {
                    if r.stats().stand_ins > 0 && self.settling < 240 {
                        self.settling += 1;
                        return Ok(true);
                    }
                }
            }
            self.settling = 0;
            Ok(out.animating)
        }
        /// The texts the last frame drew, where it drew them, as JSON — for the page's text layer
        /// (real text over the canvas: selectable, copyable, found by find-in-page):
        /// `[{ path, text, role?, family, weight, faces, size, rotate, opacity, place, drag,
        /// bounds: [x, y, w, h], lines: [{ text, x, y, w, h, baseline, br?, rtl? }] }]` in CSS px, in
        /// reading (tree) order. A line's `x, y` is its box's top-left corner with the text's
        /// rotation (radians, clockwise, about its origin) applied; `w × h` is the box before it
        /// (the drawn advance × the face's ascent + descent), `baseline` down from its top. Call
        /// it when the chart has settled: text in transition is on its way somewhere else.
        pub fn text_layer(&self) -> String {
            let texts = self.drawn.as_ref().map(|s| self.core.engine.text_layer(s)).unwrap_or_default();
            serde_json::to_string(&texts).unwrap_or_else(|_| "[]".into())
        }
        /// A face's font file (a `faces` entry of [`View::text_layer`]), so the page can lay its
        /// text in the very font the canvas draws with; `undefined` for an unknown face or one
        /// that is part of a collection (a page can't pick a face out of those).
        pub fn face_bytes(&self, id: &str) -> Option<Vec<u8>> {
            let (bytes, index) = self.core.engine.fonts().face_data(id)?;
            (index == 0).then(|| bytes.to_vec())
        }
        /// The last [`View::frame`]'s cost: `[engine ms, raster ms, display ops, tiles pending,
        /// instances rebuilt, meshes tessellated]` (instances built and uploaded again rather than
        /// kept from an earlier frame: work that a phone feels long before a desktop's frame times
        /// show it; meshes: what a raster spike is usually made of). The engine part
        /// is resolve, transition planning and interpolation, and flattening; raster is the GPU
        /// encode/upload/submit (GPU time itself shows as a slower frame rate) or the CPU raster
        /// and the canvas blit.
        pub fn frame_timing(&self) -> Vec<f64> {
            self.timing.to_vec()
        }
        /// Bundle chunks still to fetch: after opening, lazily loaded font subsets that runtime
        /// text needs (check after frames). Fetch each from the bundle's `chunks/` like the first
        /// ones and hand them to `provide_chunk`.
        pub fn chunk_requests(&self) -> Vec<String> {
            self.core.requests()
        }
        /// The frame at `now_ms` as RGBA pixels from the CPU reference (worker/OffscreenCanvas hosts,
        /// snapshot export, and the cross-target determinism test).
        pub fn pixels(&mut self, now_ms: f64) -> Vec<u8> {
            self.core.frame_pixels(now_ms / 1000.0, self.dpr).1.data
        }
        /// The pixel identity of the frame at `now_ms` — equal to `datars render --hash` (P1).
        pub fn pixel_hash(&mut self, now_ms: f64) -> String {
            format!("{:016x}", self.core.frame_pixels(now_ms / 1000.0, self.dpr).1.hash())
        }
        /// Set the viewport without a canvas (headless use).
        pub fn set_size(&mut self, width: f64, height: f64, dpr: f64) {
            self.dpr = dpr;
            self.core.engine.resize(width, height, dpr);
        }
        pub fn pointer(&mut self, kind: &str, x: f64, y: f64) -> Option<String> {
            self.core.pointer(kind, x, y)
        }
        /// The CSS cursor for where the pointer last was: `pointer` over what a click acts on,
        /// `grab`/`grabbing` over a view that pans, `crosshair` over a brushable area.
        pub fn cursor(&self) -> String {
            self.core.engine.cursor().css().into()
        }
        /// Whether a press at (x, y) starts a drag the chart takes (a brush, a pan, a slider):
        /// the page keeps such a touch from scrolling ([`datars_engine::Engine::drags_at`]).
        pub fn drags_at(&self, x: f64, y: f64) -> bool {
            self.core.engine.drags_at(datars_math::Vec2::new(x, y))
        }
        /// Wheel/pinch zoom; true if an explorable view took it (then the page shouldn't scroll).
        pub fn wheel(&mut self, x: f64, y: f64, delta: f64) -> bool {
            self.core.wheel(x, y, delta)
        }
        /// Milliseconds (the frame clock) at which to call `frame` again although nothing moves:
        /// an autoplay hold ends or a live source refreshes. NaN: nothing scheduled.
        pub fn wake_at(&self) -> f64 {
            self.core.wake_at.map_or(f64::NAN, |s| s * 1000.0)
        }
        /// Idle-time work: resolve and plan the steps next to this one, so stepping starts moving
        /// at once. One piece per call; true while there is more (call from `requestIdleCallback`).
        pub fn prepare(&mut self) -> bool {
            if self.core.engine.prepare() {
                return true;
            }
            // Then the GPU: what the prepared steps land on, tessellated and uploaded ahead.
            #[cfg(feature = "gpu")]
            if let Surface::Gpu(r) = &mut self.surface {
                if let Some(list) = self.core.engine.prepared_to_warm() {
                    r.warm(&list, self.core.engine.fonts());
                    return true;
                }
            }
            false
        }
        /// Whether a transition is playing (not just a clock running, or levels and tiles still
        /// arriving): the page gives a chart in transition the frames first.
        pub fn transitioning(&self) -> bool {
            self.core.engine.transition_stats().is_some()
        }
        /// The current program state (index).
        pub fn index(&self) -> usize {
            self.core.engine.state_index()
        }
        /// Scroll scrub: show program position `pos` (state + fraction toward the next).
        pub fn seek(&mut self, pos: f64) -> usize {
            self.core.engine.seek(pos)
        }
        /// Data the engine is waiting for, as JSON `[{name, url}]` (URL sources, first loads and
        /// live refreshes). The page fetches each — relative to the bundle — and calls
        /// `provide_source`. Slots (`{name, slot}`) are for the host app to fill. Tile archive reads
        /// are `{name, url, range: [offset, length]}`: fetch with `Range: bytes=offset-(offset+length-1)`
        /// and call `provide_range` (they appear as views need tiles — check after frames).
        pub fn data_requests(&self) -> String {
            let reqs: Vec<serde_json::Value> = self
                .core
                .engine
                .requests()
                .iter()
                .filter_map(|r| match r {
                    datars_engine::Request::Source { name, url } => Some(serde_json::json!({ "name": name, "url": url })),
                    datars_engine::Request::Slot { name, slot } => Some(serde_json::json!({ "name": name, "slot": slot })),
                    datars_engine::Request::Range { name, url, offset, length } => Some(serde_json::json!({ "name": name, "url": url, "range": [offset, length] })),
                    // A built-in atlas (`countries`, …): the page fetches it from the runtime's
                    // atlas folder (published bundles carry theirs; raw documents ask).
                    datars_engine::Request::Atlas { name, atlas } => Some(serde_json::json!({ "name": name, "atlas": atlas })),
                    _ => None,
                })
                .collect();
            serde_json::Value::Array(reqs).to_string()
        }
        /// Start recording this view's inputs (a reader session → a bug report or a test).
        pub fn start_recording(&mut self) {
            self.core.engine.start_recording();
        }
        /// Stop recording; the session as JSON (`datars replay session.json` reproduces it).
        pub fn take_recording(&mut self) -> Option<String> {
            self.core.engine.take_recording().map(|s| s.to_json())
        }
        /// Run a mark's click intent by its semantics path (the keyboard route to click-to-filter).
        pub fn activate(&mut self, path: &str) -> bool {
            self.core.engine.activate(path)
        }
        /// Reduced motion (`prefers-reduced-motion`): transitions become short crossfades.
        pub fn set_reduced_motion(&mut self, on: bool) {
            self.core.engine.set_reduced_motion(on);
        }
        /// Pause or resume autoplay.
        pub fn set_playing(&mut self, on: bool) {
            self.core.engine.set_playing(on);
        }
        pub fn event(&mut self, name: &str) -> bool {
            self.core.engine.event(name)
        }
        pub fn goto(&mut self, index: usize) -> bool {
            self.core.engine.goto(index)
        }
        pub fn set_mode(&mut self, mode: &str) {
            self.core.set_mode(mode);
        }
        pub fn set_tokens(&mut self, json: &str) -> Result<(), JsValue> {
            let v: std::collections::BTreeMap<String, serde_json::Value> = serde_json::from_str(json).map_err(js)?;
            self.core.engine.set_host_tokens(v);
            Ok(())
        }
        pub fn set_signal(&mut self, name: &str, json: &str) -> Result<(), JsValue> {
            // Numbers, strings, booleans, keysets (arrays of keys), ranges ({lo, hi}), null.
            let v: serde_json::Value = serde_json::from_str(json).map_err(js)?;
            self.core.engine.set_signal_json(name, &v);
            Ok(())
        }
        /// Everything the page shows about the chart: state, narration, the semantics tree (every
        /// labelled mark — thousands for big data), theme tokens. Costly: once per state change,
        /// never per frame; [`View::narration`] and [`View::state_count`] are the cheap parts.
        pub fn status(&mut self) -> JsValue {
            serde_wasm(&self.core.status())
        }
        /// The share of the per-frame work budgets this device's frames may spend (0.1–1): the
        /// page lowers it when frames run long (a phone's CPU), for the engine and the renderer.
        pub fn set_work_scale(&mut self, scale: f64) {
            self.core.engine.set_work_scale(scale);
            #[cfg(feature = "gpu")]
            if let Surface::Gpu(r) = &mut self.surface {
                r.set_work_scale(scale);
            }
        }
        /// `status()` without `semantics` — cheap, for every step (the whole semantics tree is
        /// thousands of items on a map of counties).
        pub fn chrome(&mut self) -> JsValue {
            serde_wasm(&self.core.status_brief())
        }
        /// Every signal's value now, the way `set_signal` takes them ([`datars_engine::Engine::signal_values`]):
        /// what the reader's clicks, brushes and drags wrote, for a page that follows the chart.
        /// Cheap (a few dozen values): fine after every input.
        pub fn signals(&self) -> JsValue {
            serde_wasm(&serde_json::json!(self.core.engine.signal_values()))
        }
        /// The semantics tree alone (`status().semantics`).
        pub fn semantics(&mut self) -> JsValue {
            serde_wasm(&serde_json::Value::Array(self.core.semantics()))
        }
        /// The current state's narration (`{title, text, anchor}` or null) — cheap, for per-frame use.
        pub fn narration(&self) -> JsValue {
            serde_wasm(&serde_json::json!(self.core.engine.narration()))
        }
        /// How many top-level states the program has — cheap, for per-scroll use.
        pub fn state_count(&self) -> usize {
            self.core.engine.state_names().len()
        }
        /// What the last frame drew from big data, for pages that show it: rows in the point
        /// pyramids (`instances` with `lod`), points and tiles drawn, the deepest level; archive
        /// bytes and range requests fetched so far.
        pub fn stats(&self) -> JsValue {
            let p = self.core.engine.point_stats();
            let t = self.core.engine.tile_stats();
            serde_wasm(&serde_json::json!({ "rows": p.rows, "drawn": p.drawn, "tiles": p.tiles, "level": p.level, "bytes": t.bytes, "requests": t.requests }))
        }
        /// Everything the page's stats panel shows about the last frame (`Shift+D`, `?datars-perf`):
        /// the renderer and its caches, what the frame cost, the transition in flight.
        pub fn nerd_stats(&self) -> JsValue {
            let mut v = serde_json::json!({
                "renderer": match &self.surface { #[cfg(feature = "gpu")] Surface::Gpu(_) => "gpu", Surface::Cpu(_) => "cpu", Surface::None => "none" },
                "dpr": self.dpr,
                "tier": self.core.tier.as_ref().map(|t| format!("{t:?}")),
                "timing": self.timing,
            });
            #[cfg(feature = "gpu")]
            if let Surface::Gpu(r) = &self.surface {
                let s = r.stats();
                v["gpu"] = serde_json::json!({
                    "backend": r.backend(), "samples": r.samples(), "size": r.size(),
                    "draws": s.draws, "instances": s.instances, "passes": s.passes,
                    "tessellations": s.tessellations, "cache_hits": s.cache_hits, "evictions": s.evictions,
                    "meshes": s.meshes, "mesh_bytes": s.mesh_bytes, "arena_meshes": s.arena_meshes, "arena_bytes": s.arena_bytes,
                    "instance_sets_kept": s.instance_sets_kept, "instances_rebuilt": s.instances_rebuilt,
                });
            }
            if let Some((p, t)) = self.core.engine.transition_stats() {
                v["transition"] = serde_json::json!({
                    "progress": t, "pairs": p.pairs, "enters": p.enters, "exits": p.exits, "splits": p.splits, "merges": p.merges,
                    "flyers": p.flyers, "morphs": p.morphs, "crossfades": p.crossfades, "instances": p.instances,
                });
            }
            serde_wasm(&v)
        }
        /// Every element drawn under a point of the current frame (CSS px), topmost first —
        /// pickable or not (titles, axes, cards, marks): `[{ path, kind, role, label, text, bounds }]`.
        /// For editors: what a click on the canvas selects.
        pub fn hit_test(&self, x: f64, y: f64) -> JsValue {
            serde_wasm(&serde_json::json!(self.core.engine.hit_test(x, y)))
        }
        /// Why an element looks as it does, on the live engine (the `datars explain` data): the
        /// recipes that made it, its data row, every expression's value, the template, the node.
        /// `path` as `hit_test` returns it.
        pub fn explain(&mut self, path: &str) -> JsValue {
            serde_wasm(&serde_json::json!(self.core.engine.explain(path)))
        }
        /// Elements that link somewhere, where they are now: `[{ href, label, bounds }]` (the page
        /// puts real, focusable links over them — a map's OpenStreetMap credit, for one).
        /// For each point (`xy`: x0, y0, x1, y1, … in CSS px), whether the frame draws something there
        /// a card shouldn't cover: text, a datum or a region. One pass over the frame for all of
        /// them (a card tries a few spots, each sampled at a grid of points).
        pub fn occupied(&self, xy: Vec<f64>) -> Vec<u8> {
            let pts: Vec<datars_math::Vec2> = xy.chunks_exact(2).map(|p| datars_math::Vec2::new(p[0], p[1])).collect();
            self.core
                .engine
                .hit_test_many(&pts)
                .iter()
                .map(|hits| hits.iter().any(|h| h.kind == "text" || matches!(h.role.as_deref(), Some("datum" | "region"))) as u8)
                .collect()
        }
        pub fn links(&self) -> JsValue {
            serde_wasm(&serde_json::json!(self.core.engine.links()))
        }
        pub fn anchors(&mut self) -> JsValue {
            let out = self.core.engine.frame(0.0);
            serde_wasm(&serde_json::json!(out.anchors.iter().map(|a| serde_json::json!({ "path": a.path, "name": a.name, "x": a.x, "y": a.y })).collect::<Vec<_>>()))
        }
    }

    fn datars_expr_value_num(v: f64) -> datars_engine::SignalValue {
        datars_engine::SignalValue::Num(v)
    }
    fn datars_expr_value_str(s: &str) -> datars_engine::SignalValue {
        datars_engine::SignalValue::Str(std::sync::Arc::from(s))
    }

    fn serde_wasm(v: &serde_json::Value) -> JsValue {
        js_sys::JSON::parse(&v.to_string()).unwrap_or(JsValue::NULL)
    }
}
