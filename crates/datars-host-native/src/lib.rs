//! `datars-host-native` — the desktop host (docs/10-platforms.md). A winit window whose surface
//! the wgpu backend draws into; input becomes engine pointer events and program events. Frames run
//! only while something moves (`ControlFlow::Wait` otherwise).

mod a11y;
mod http;

use datars_render_wgpu::Renderer;
use datars_runtime::Core;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, NamedKey};
use winit::window::{CursorIcon, Window, WindowId};

pub enum Source {
    Doc(String),
    Bundle(Vec<u8>),
    /// A published chart by URL (`http://…/c/<alias>`): manifest, chunks, data and tile ranges
    /// fetched as the runtime asks — the same bundle a web page or an app plays.
    Url(String),
}

/// Where relative data URLs, atlases and tile archives come from.
#[derive(Clone, Debug)]
enum Origin {
    Disk(Option<std::path::PathBuf>),
    Web(String),
}

/// Events the window loop receives besides winit's own: accessibility requests.
enum UserEvent {
    A11y(accesskit_winit::Event),
}

impl From<accesskit_winit::Event> for UserEvent {
    fn from(e: accesskit_winit::Event) -> Self {
        UserEvent::A11y(e)
    }
}

struct App {
    core: Core,
    proxy: EventLoopProxy<UserEvent>,
    /// The screen-reader bridge (AccessKit), and what each of its nodes stands for.
    a11y: Option<accesskit_winit::Adapter>,
    a11y_targets: Vec<(accesskit::NodeId, a11y::Target)>,
    /// Where relative data URLs and atlases resolve (the document's directory, or its URL).
    origin: Origin,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    start: Instant,
    cursor: (f64, f64),
    dark: bool,
    title: String,
}

impl App {
    /// Fulfil the engine's data requests from disk (first loads, live refreshes).
    fn fetch_data(&mut self) {
        fulfil(&mut self.core, &self.origin);
    }

    /// The cursor the engine asks for where the pointer is (a hand over what a click acts on).
    fn show_cursor(&self) {
        let Some(w) = self.window.as_ref() else { return };
        w.set_cursor(match self.core.engine.cursor() {
            datars_engine::Cursor::Default => CursorIcon::Default,
            datars_engine::Cursor::Pointer => CursorIcon::Pointer,
            datars_engine::Cursor::Grab => CursorIcon::Grab,
            datars_engine::Cursor::Grabbing => CursorIcon::Grabbing,
            datars_engine::Cursor::Crosshair => CursorIcon::Crosshair,
        });
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        self.fetch_data();
        let (Some(w), Some(r)) = (&self.window, &mut self.renderer) else { return };
        let now = self.start.elapsed().as_secs_f64();
        let out = self.core.engine.frame(now);
        r.set_moving(out.animating);
        if let Err(e) = r.render(&out.display, self.core.engine.fonts()) {
            eprintln!("render: {e}");
        }
        if out.animating || out.pending_tiles > 0 || r.stats().stand_ins > 0 {
            // Moving, tiles were just asked for (fulfilled at the next redraw), or meshes still
            // stand in for exact ones (each frame builds its share).
            w.request_redraw();
            event_loop.set_control_flow(ControlFlow::Poll);
        } else if let Some(at) = out.wake_at {
            // Sleep until the engine's next scheduled frame (autoplay hold, live refresh).
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.start + std::time::Duration::from_secs_f64(at.max(now))));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
        let st = self.core.engine.state();
        w.set_title(&format!("{} — {st}", self.title));
        if !out.animating {
            self.update_a11y();
        }
    }

    /// Give screen readers the current semantics (only while one is listening).
    fn update_a11y(&mut self) {
        let (Some(adapter), Some(w)) = (self.a11y.as_mut(), self.window.as_ref()) else { return };
        let (tree, targets) = a11y::tree(&mut self.core.engine, &self.title, w.scale_factor());
        self.a11y_targets = targets;
        adapter.update_if_active(|| tree);
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn user_event(&mut self, _: &ActiveEventLoop, event: UserEvent) {
        let UserEvent::A11y(e) = event;
        match e.window_event {
            accesskit_winit::WindowEvent::InitialTreeRequested => self.update_a11y(),
            accesskit_winit::WindowEvent::ActionRequested(req) => {
                self.core.engine.set_clock(self.start.elapsed().as_secs_f64());
                if a11y::act(&mut self.core.engine, &self.a11y_targets, &req) {
                    if let Some(w) = self.window.as_ref() {
                        w.request_redraw();
                    }
                }
            }
            accesskit_winit::WindowEvent::AccessibilityDeactivated => {}
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let vp = self.core.engine.viewport();
        // Hidden until the accessibility adapter is attached (AccessKit's requirement).
        let attrs = Window::default_attributes().with_title(&self.title).with_inner_size(LogicalSize::new(vp.width, vp.height)).with_visible(false);
        let window = Arc::new(event_loop.create_window(attrs).expect("window"));
        self.a11y = Some(accesskit_winit::Adapter::with_event_loop_proxy(event_loop, &window, self.proxy.clone()));
        window.set_visible(true);
        let size = window.inner_size();
        let dpr = window.scale_factor();
        match Renderer::for_surface(window.clone(), size.width, size.height, dpr) {
            Ok(r) => self.renderer = Some(r),
            Err(e) => {
                eprintln!("datars: no GPU surface ({e})");
                event_loop.exit();
                return;
            }
        }
        self.core.engine.resize(size.width as f64 / dpr, size.height as f64 / dpr, dpr);
        window.request_redraw();
        self.window = Some(window);
    }

    fn new_events(&mut self, _: &ActiveEventLoop, cause: winit::event::StartCause) {
        if matches!(cause, winit::event::StartCause::ResumeTimeReached { .. }) {
            if let Some(w) = self.window.as_ref() { w.request_redraw() }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if let (Some(a), Some(w)) = (self.a11y.as_mut(), self.window.as_ref()) {
            a.process_event(w, &event);
        }
        let dpr = self.window.as_ref().map(|w| w.scale_factor()).unwrap_or(1.0);
        // Inputs first tell the engine the time: frames stop while nothing moves, and a transition
        // started at the last frame's time would count as long started (a flight would jump).
        if matches!(event, WindowEvent::CursorMoved { .. } | WindowEvent::MouseInput { .. } | WindowEvent::MouseWheel { .. } | WindowEvent::KeyboardInput { .. }) {
            self.core.engine.set_clock(self.start.elapsed().as_secs_f64());
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(s) => {
                if let Some(r) = &mut self.renderer {
                    r.resize(s.width.max(1), s.height.max(1), dpr);
                }
                self.core.engine.resize(s.width as f64 / dpr, s.height as f64 / dpr, dpr);
                if let Some(w) = self.window.as_ref() { w.request_redraw() }
            }
            WindowEvent::RedrawRequested => self.redraw(event_loop),
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x / dpr, position.y / dpr);
                if let Some(label) = self.core.pointer("move", self.cursor.0, self.cursor.1) {
                    if let Some(w) = &self.window {
                        w.set_title(&format!("{} — {label}", self.title));
                    }
                }
                self.show_cursor();
                if let Some(w) = self.window.as_ref() { w.request_redraw() }
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                self.core.pointer(if state == ElementState::Pressed { "down" } else { "up" }, self.cursor.0, self.cursor.1);
                self.show_cursor();
                if let Some(w) = self.window.as_ref() { w.request_redraw() }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -y as f64 * 16.0,
                    MouseScrollDelta::PixelDelta(p) => -p.y / dpr,
                };
                if self.core.wheel(self.cursor.0, self.cursor.1, dy) {
                    if let Some(w) = self.window.as_ref() { w.request_redraw() }
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let ev = match &event.logical_key {
                    Key::Named(NamedKey::ArrowRight) | Key::Named(NamedKey::Space) => Some("next"),
                    Key::Named(NamedKey::ArrowLeft) => Some("prev"),
                    Key::Named(NamedKey::Escape) => Some("back"),
                    Key::Character(c) if c.as_str() == "d" => {
                        self.dark = !self.dark;
                        self.core.set_mode(if self.dark { "dark" } else { "light" });
                        None
                    }
                    _ => None,
                };
                if let Some(e) = ev {
                    self.core.engine.event(e);
                }
                if let Some(w) = self.window.as_ref() { w.request_redraw() }
            }
            _ => {}
        }
    }
}

/// Fulfil pending data, atlas and tile-range requests from disk relative to a directory, or over
/// HTTP relative to the chart's URL.
fn fulfil(core: &mut Core, origin: &Origin) {
    let dir = match origin {
        Origin::Disk(d) => d.as_deref(),
        Origin::Web(base) => return fulfil_web(core, base),
    };
    for req in core.engine.requests() {
        let name = match &req {
            datars_engine::Request::Source { name, .. } | datars_engine::Request::Slot { name, .. } | datars_engine::Request::Atlas { name, .. } => name.clone(),
            datars_engine::Request::Range { name, offset, .. } => {
                if let Some(bytes) = datars_headless::fetch_from_disk(&req, dir) {
                    if let Err(e) = core.engine.provide_range(name, *offset, &bytes) {
                        eprintln!("tiles `{name}`: {e}");
                    }
                }
                continue;
            }
        };
        if let Some(bytes) = datars_headless::fetch_from_disk(&req, dir) {
            if let Err(e) = core.engine.provide(&name, &bytes) {
                eprintln!("data `{name}`: {e}");
            }
        }
    }
}

fn fulfil_web(core: &mut Core, base: &str) {
    for req in core.engine.requests() {
        match &req {
            datars_engine::Request::Source { name, url } => match http::get(&http::resolve(base, url), None) {
                Ok(bytes) => {
                    if let Err(e) = core.engine.provide(name, &bytes) {
                        eprintln!("data `{name}`: {e}");
                    }
                }
                Err(e) => eprintln!("data `{name}`: {e}"),
            },
            datars_engine::Request::Range { name, url, offset, length } => match http::get(&http::resolve(base, url), Some((*offset, *length))) {
                Ok(bytes) => {
                    if let Err(e) = core.engine.provide_range(name, *offset, &bytes) {
                        eprintln!("tiles `{name}`: {e}");
                    }
                }
                Err(e) => eprintln!("tiles `{name}`: {e}"),
            },
            // Atlases arrive with the bundle's chunks; slots are the host app's to fill.
            _ => {}
        }
    }
}

fn load(source: Source, origin: &Origin) -> Result<Core, String> {
    let mut core = Core::new(true, Vec::new());
    match source {
        Source::Doc(json) => {
            let d = core.load_doc_json(&json)?;
            for m in d {
                eprintln!("diagnostic: {m}");
            }
        }
        Source::Bundle(bytes) => core.open_file(&bytes)?,
        Source::Url(url) => {
            // The chunk store sits next to the aliases: `…/c/<alias>` → `…/chunks/<hash>`.
            let root = url.rsplit_once("/c/").map(|(r, _)| r.to_string()).unwrap_or_else(|| http::resolve(&url, ".").trim_end_matches('/').to_string());
            let mut need = core.open_manifest(&http::get(&url, None)?)?;
            while !need.is_empty() {
                let mut next = Vec::new();
                for h in need {
                    next = core.provide_chunk(&h, &http::get(&format!("{root}/chunks/{}", h.replacen(':', "_", 1)), None)?)?;
                }
                need = next;
            }
        }
    }
    fulfil(&mut core, origin);
    Ok(core)
}

/// Where a source's relative references resolve.
fn origin_of(source: &Source, path: &str) -> Origin {
    match source {
        Source::Url(u) => Origin::Web(u.clone()),
        _ => Origin::Disk(std::path::Path::new(path).parent().map(|p| p.to_path_buf())),
    }
}

/// Render state `state` through the same GPU path as the window, offscreen, to a PNG — for tests
/// and for machines where a window isn't wanted.
pub fn screenshot(source: Source, path: &str, state: usize, dpr: f64, out: &std::path::Path) -> Result<(), String> {
    let origin = origin_of(&source, path);
    let mut core = load(source, &origin)?;
    let vp = core.engine.viewport();
    core.engine.goto(state);
    // Settle: render, answer what the frame asked for (tile ranges), until nothing is missing.
    let mut frame = core.engine.frame(1e6); // long after any transition
    for _ in 0..16 {
        if core.engine.requests().is_empty() {
            break;
        }
        fulfil(&mut core, &origin);
        frame = core.engine.frame(1e6);
    }
    let (w, h) = ((vp.width * dpr).round() as u32, (vp.height * dpr).round() as u32);
    let mut r = Renderer::headless(w, h, dpr).map_err(|e| format!("no GPU: {e}"))?;
    r.render(&frame.display, core.engine.fonts()).map_err(|e| e.to_string())?;
    let rgba = r.try_read_rgba().map_err(|e| e.to_string())?;
    let mut png_bytes = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut png_bytes, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut wr = enc.write_header().map_err(|e| e.to_string())?;
        wr.write_image_data(&rgba).map_err(|e| e.to_string())?;
    }
    std::fs::write(out, png_bytes).map_err(|e| e.to_string())
}

/// Open a window showing a document or bundle; returns when the window closes.
pub fn run(source: Source, title: &str) -> Result<(), String> {
    let origin = origin_of(&source, title);
    let core = load(source, &origin)?;
    let event_loop = EventLoop::<UserEvent>::with_user_event().build().map_err(|e| e.to_string())?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let proxy = event_loop.create_proxy();
    let mut app = App { core, proxy, a11y: None, a11y_targets: Vec::new(), origin, window: None, renderer: None, start: Instant::now(), cursor: (0.0, 0.0), dark: false, title: title.to_string() };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())
}
