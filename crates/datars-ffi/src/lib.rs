//! `datars-ffi` — the runtime behind a C ABI (docs/10-platforms.md). Swift (`DatarsKit`) and
//! Kotlin (`datars-android`, via the JNI shims below) wrap these functions; strings cross as
//! UTF-8 pointers + lengths, results as JSON strings freed with `datars_string_free`.
//!
//! Frames go to the GPU once a host attaches its surface (`gpu` feature): wgpu on Metal through a
//! `CAMetalLayer` (`datars_view_attach_metal_layer`), on Vulkan or GLES through an Android
//! `Surface` (the JNI `attachSurface`). Before that, or where there's no GPU, frames are the CPU
//! reference's RGBA buffer the host draws (a CGImage, an Android Bitmap) — portable and bit-exact,
//! and what the goldens and `datars_view_pixel_hash` compare.

// C entry points take raw pointers with a documented contract (a view from `datars_view_new`,
// buffers of the given length); marking them `unsafe fn` changes nothing for C, Swift or JNI.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use datars_runtime::Core;
use std::ffi::{c_char, CString};

pub struct DatarsView {
    core: Core,
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    dpr: f64,
    hash: u64,
    /// From the last frame: something moves, so the host should render the next one.
    animating: bool,
    /// The GPU renderer on the host's surface, once attached.
    #[cfg(feature = "gpu")]
    gpu: Option<datars_render_wgpu::Renderer>,
    /// Frames drawn after the motion stopped while stand-in meshes remained (bounded).
    settling: u32,
    /// The host's share of the per-frame work budgets (`datars_view_set_work_scale`), kept for a
    /// GPU renderer attached later.
    work: f64,
    /// The last frame's engine and render time (ms; the render part NaN on CPU pixels, where one
    /// call does both) and the renderer's meshes tessellated before it.
    frame_ms: (f64, f64),
    tessellated: u64,
    /// The Android window the GPU surface draws into, released after the renderer.
    #[cfg(all(feature = "gpu", target_os = "android"))]
    window: *mut std::ffi::c_void,
}

fn s(ptr: *const u8, len: usize) -> String {
    if ptr.is_null() {
        return String::new();
    }
    // SAFETY: the caller passes a valid UTF-8 buffer of `len` bytes (documented contract).
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    String::from_utf8_lossy(bytes).into_owned()
}

fn b<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
    if ptr.is_null() {
        return &[];
    }
    // SAFETY: the caller passes a valid buffer of `len` bytes that outlives the call.
    unsafe { std::slice::from_raw_parts(ptr, len) }
}

fn out(v: serde_json::Value) -> *mut c_char {
    CString::new(v.to_string()).map(|c| c.into_raw()).unwrap_or(std::ptr::null_mut())
}

fn view<'a>(v: *mut DatarsView) -> Option<&'a mut DatarsView> {
    // SAFETY: `v` comes from `datars_view_new` and is used from one thread at a time.
    unsafe { v.as_mut() }
}

/// Create a view. `allow_script` = 0 refuses bundles that need the JS sandbox (T3).
#[no_mangle]
pub extern "C" fn datars_view_new(allow_script: i32) -> *mut DatarsView {
    Box::into_raw(Box::new(DatarsView {
        core: Core::new(allow_script != 0, Vec::new()),
        pixels: Vec::new(),
        width: 0,
        height: 0,
        dpr: 1.0,
        hash: 0,
        animating: false,
        #[cfg(feature = "gpu")]
        gpu: None,
        settling: 0,
        work: 1.0,
        frame_ms: (f64::NAN, f64::NAN),
        tessellated: 0,
        #[cfg(all(feature = "gpu", target_os = "android"))]
        window: std::ptr::null_mut(),
    }))
}

#[no_mangle]
pub extern "C" fn datars_view_free(v: *mut DatarsView) {
    if !v.is_null() {
        // SAFETY: created by `datars_view_new`, freed once.
        drop(unsafe { Box::from_raw(v) });
    }
}

#[no_mangle]
pub extern "C" fn datars_string_free(p: *mut c_char) {
    if !p.is_null() {
        // SAFETY: created by `CString::into_raw` in this crate.
        drop(unsafe { CString::from_raw(p) });
    }
}

/// Load a document (JSON IR). Returns `{"ok": bool, "diagnostics": [...]}`.
#[no_mangle]
pub extern "C" fn datars_view_load_doc(v: *mut DatarsView, json: *const u8, len: usize) -> *mut c_char {
    let Some(v) = view(v) else { return std::ptr::null_mut() };
    out(match v.core.load_doc_json(&s(json, len)) {
        Ok(d) => serde_json::json!({ "ok": true, "diagnostics": d }),
        Err(e) => serde_json::json!({ "ok": false, "error": e }),
    })
}

/// Open a single-file `.datars` bundle.
#[no_mangle]
pub extern "C" fn datars_view_open_file(v: *mut DatarsView, bytes: *const u8, len: usize) -> *mut c_char {
    let Some(v) = view(v) else { return std::ptr::null_mut() };
    out(match v.core.open_file(b(bytes, len)) {
        Ok(()) => serde_json::json!({ "ok": true, "tier": v.core.tier.map(|t| format!("{t:?}")) }),
        Err(e) => serde_json::json!({ "ok": false, "error": e }),
    })
}

/// Open a manifest; returns `{"requests": [chunk hashes]}` the host must fetch.
#[no_mangle]
pub extern "C" fn datars_view_open_manifest(v: *mut DatarsView, bytes: *const u8, len: usize) -> *mut c_char {
    let Some(v) = view(v) else { return std::ptr::null_mut() };
    out(match v.core.open_manifest(b(bytes, len)) {
        Ok(r) => serde_json::json!({ "ok": true, "requests": r }),
        Err(e) => serde_json::json!({ "ok": false, "error": e }),
    })
}

#[no_mangle]
pub extern "C" fn datars_view_provide_chunk(v: *mut DatarsView, hash: *const u8, hash_len: usize, bytes: *const u8, len: usize) -> *mut c_char {
    let Some(v) = view(v) else { return std::ptr::null_mut() };
    out(match v.core.provide_chunk(&s(hash, hash_len), b(bytes, len)) {
        Ok(r) => serde_json::json!({ "ok": true, "requests": r, "ready": v.core.tier.is_some() }),
        Err(e) => serde_json::json!({ "ok": false, "error": e }),
    })
}

/// Provide bytes for a data source or slot (the host's own data never leaves the device).
#[no_mangle]
pub extern "C" fn datars_view_provide_source(v: *mut DatarsView, name: *const u8, name_len: usize, bytes: *const u8, len: usize) -> i32 {
    let Some(v) = view(v) else { return 0 };
    v.core.engine.provide(&s(name, name_len), b(bytes, len)).is_ok() as i32
}

#[no_mangle]
pub extern "C" fn datars_view_resize(v: *mut DatarsView, width: f64, height: f64, dpr: f64) {
    if let Some(v) = view(v) {
        v.core.engine.resize(width, height, dpr);
        v.dpr = dpr;
        #[cfg(feature = "gpu")]
        if let Some(r) = v.gpu.as_mut() {
            r.resize((width * dpr).round().max(1.0) as u32, (height * dpr).round().max(1.0) as u32, dpr);
        }
    }
}

/// Render a frame at `now` seconds: onto the attached GPU surface, else into the view's RGBA
/// buffer. Returns 1 while the host should draw the next frame — something moves, or (GPU) the
/// settled frame still has stand-in meshes the next ones replace with exact ones.
#[no_mangle]
pub extern "C" fn datars_view_frame(v: *mut DatarsView, now: f64) -> i32 {
    let Some(v) = view(v) else { return 0 };
    #[cfg(feature = "gpu")]
    if let Some(r) = v.gpu.as_mut() {
        let t0 = std::time::Instant::now();
        let out = v.core.engine.frame(now);
        let t1 = std::time::Instant::now();
        v.tessellated = r.stats().tessellations;
        v.core.wake_at = out.wake_at;
        r.set_moving(out.animating);
        // A GPU stack that panics (a driver the backend trips on) costs the GPU path, not the app.
        let fonts = v.core.engine.fonts();
        let drawn = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| r.render(&out.display, fonts))).unwrap_or_else(|_| Err(datars_render_wgpu::GpuError::Surface("the GPU renderer panicked".into())));
        v.core.after_frame();
        v.frame_ms = ((t1 - t0).as_secs_f64() * 1000.0, t1.elapsed().as_secs_f64() * 1000.0);
        let mut animating = out.animating;
        match drawn {
            Ok(()) if !animating && r.stats().stand_ins > 0 && v.settling < 240 => {
                v.settling += 1;
                animating = true;
            }
            Ok(()) => v.settling = 0,
            // The device or surface is gone: back to CPU pixels until the host attaches again.
            Err(_) => {
                v.gpu = None;
                animating = true;
            }
        }
        v.animating = animating;
        return animating as i32;
    }
    let t0 = std::time::Instant::now();
    let (animating, px) = v.core.frame_pixels(now, v.dpr);
    v.frame_ms = (t0.elapsed().as_secs_f64() * 1000.0, f64::NAN);
    v.hash = px.hash();
    v.width = px.width;
    v.height = px.height;
    v.pixels = px.data;
    v.animating = animating;
    animating as i32
}

/// Whether the last frame was mid-animation (what `datars_view_frame` returned), without rendering.
#[no_mangle]
pub extern "C" fn datars_view_animating(v: *mut DatarsView) -> i32 {
    view(v).map_or(0, |v| v.animating as i32)
}

/// Tell the engine the time (seconds, the frame clock) without rendering. Call before an input
/// (event, goto, pointer, signal) when frames may have stopped: transitions start at the engine's
/// clock, and a clock left at the last frame would count a camera flight as long started.
#[no_mangle]
pub extern "C" fn datars_view_set_clock(v: *mut DatarsView, now: f64) {
    if let Some(v) = view(v) {
        v.core.engine.set_clock(now);
    }
}

/// The last frame's pixel identity — equal to `datars render --hash` and the goldens (P1).
#[no_mangle]
pub extern "C" fn datars_view_pixel_hash(v: *mut DatarsView) -> u64 {
    view(v).map_or(0, |v| v.hash)
}

/// Wheel/pinch zoom at (x, y) in points; `delta` > 0 zooms out. Returns 1 if an explorable view
/// took it.
#[no_mangle]
pub extern "C" fn datars_view_wheel(v: *mut DatarsView, x: f64, y: f64, delta: f64) -> i32 {
    view(v).map_or(0, |v| v.core.wheel(x, y, delta) as i32)
}

/// Scroll scrub: show program position `pos` (state index + fraction toward the next state).
/// Returns the state index now current.
#[no_mangle]
pub extern "C" fn datars_view_seek(v: *mut DatarsView, pos: f64) -> u32 {
    view(v).map_or(0, |v| v.core.engine.seek(pos) as u32)
}

/// Run the click intent of the mark at `path` (a semantics item's `path`) — VoiceOver/TalkBack
/// activation of interactive marks. Returns 1 if one ran.
#[no_mangle]
pub extern "C" fn datars_view_activate(v: *mut DatarsView, path: *const u8, len: usize) -> i32 {
    view(v).map_or(0, |v| v.core.engine.activate(&s(path, len)) as i32)
}

/// Set a numeric signal (a native control bound to an engine-drawn one, app state).
#[no_mangle]
pub extern "C" fn datars_view_set_signal_num(v: *mut DatarsView, name: *const u8, len: usize, value: f64) {
    if let Some(v) = view(v) {
        v.core.engine.set_signal(&s(name, len), datars_runtime::datars_engine::SignalValue::Num(value));
    }
}

/// Set a signal to a JSON value — a string, a number, a boolean, a key set (an array), `null`:
/// what a native picker chose for an engine-drawn select. Returns 0 when the JSON doesn't parse.
#[no_mangle]
pub extern "C" fn datars_view_set_signal_json(v: *mut DatarsView, name: *const u8, len: usize, json: *const u8, json_len: usize) -> i32 {
    let Some(v) = view(v) else { return 0 };
    match serde_json::from_str::<serde_json::Value>(&s(json, json_len)) {
        Ok(value) => {
            v.core.engine.set_signal_json(&s(name, len), &value);
            1
        }
        Err(_) => 0,
    }
}

/// Reduced motion (the platform setting): transitions become short crossfades.
#[no_mangle]
pub extern "C" fn datars_view_set_reduced_motion(v: *mut DatarsView, on: i32) {
    if let Some(v) = view(v) {
        v.core.engine.set_reduced_motion(on != 0);
    }
}

/// The share of the per-frame work budgets this device's frames may spend (0.1–1, default 1):
/// points built, tiles decoded and styled, meshes tessellated and uploaded. The budgets are sized
/// for a desktop; a host whose frames run long (a phone) lowers it, and detail arrives over more
/// frames instead of frames dropping. For the engine and the GPU renderer.
#[no_mangle]
pub extern "C" fn datars_view_set_work_scale(v: *mut DatarsView, scale: f64) {
    if let Some(v) = view(v) {
        v.work = if scale.is_finite() { scale.clamp(0.1, 1.0) } else { 1.0 };
        v.core.engine.set_work_scale(v.work);
        #[cfg(feature = "gpu")]
        if let Some(r) = &mut v.gpu {
            r.set_work_scale(v.work);
        }
    }
}

/// Pause (0) or resume (1) autoplay.
#[no_mangle]
pub extern "C" fn datars_view_set_playing(v: *mut DatarsView, on: i32) {
    if let Some(v) = view(v) {
        v.core.engine.set_playing(on != 0);
    }
}

/// Seconds (the frame clock) at which the next frame is due although nothing moves — an autoplay
/// hold ending, a live refresh; NaN if nothing is scheduled. Valid after `datars_view_frame`.
#[no_mangle]
pub extern "C" fn datars_view_wake_at(v: *mut DatarsView) -> f64 {
    view(v).and_then(|v| v.core.wake_at).unwrap_or(f64::NAN)
}

/// Jump to program state `index` (no transition is started if it's already current). Returns 1 if
/// the state changed.
#[no_mangle]
pub extern "C" fn datars_view_goto(v: *mut DatarsView, index: u32) -> i32 {
    view(v).map_or(0, |v| v.core.engine.goto(index as usize) as i32)
}

/// The last frame's pixels: RGBA8 straight alpha, `width * height * 4` bytes, valid until the next
/// `datars_view_frame`.
#[no_mangle]
pub extern "C" fn datars_view_pixels(v: *mut DatarsView, width: *mut u32, height: *mut u32) -> *const u8 {
    let Some(v) = view(v) else { return std::ptr::null() };
    // SAFETY: out-params provided by the caller.
    unsafe {
        if !width.is_null() {
            *width = v.width;
        }
        if !height.is_null() {
            *height = v.height;
        }
    }
    v.pixels.as_ptr()
}

/// Pointer input (`kind`: 0 move, 1 down, 2 up, 3 leave, 4 tap — a touch press released in place,
/// after its down: a click that also inspects, with a finger's reach). Returns the label of what it
/// lands on (a tooltip), or null.
#[no_mangle]
pub extern "C" fn datars_view_pointer(v: *mut DatarsView, kind: i32, x: f64, y: f64) -> *mut c_char {
    let Some(v) = view(v) else { return std::ptr::null_mut() };
    let k = ["move", "down", "up", "leave", "tap"].get(kind as usize).copied().unwrap_or("leave");
    match v.core.pointer(k, x, y) {
        Some(l) => out(serde_json::Value::String(l)),
        None => std::ptr::null_mut(),
    }
}

/// The cursor for where the pointer last was: 0 default, 1 pointer (a click acts here), 2 grab (a
/// view that pans), 3 grabbing (while it's panned), 4 crosshair (a brushable area).
#[no_mangle]
pub extern "C" fn datars_view_cursor(v: *mut DatarsView) -> i32 {
    let Some(v) = view(v) else { return 0 };
    match v.core.engine.cursor().css() {
        "pointer" => 1,
        "grab" => 2,
        "grabbing" => 3,
        "crosshair" => 4,
        _ => 0,
    }
}

/// A program event: `next`, `prev`, `back`, `goto:<name>`. Returns 1 if the state changed.
#[no_mangle]
pub extern "C" fn datars_view_event(v: *mut DatarsView, name: *const u8, len: usize) -> i32 {
    let Some(v) = view(v) else { return 0 };
    v.core.engine.event(&s(name, len)) as i32
}

/// `0` light, `1` dark, `2` high contrast.
#[no_mangle]
pub extern "C" fn datars_view_set_mode(v: *mut DatarsView, mode: i32) {
    if let Some(v) = view(v) {
        v.core.set_mode(["light", "dark", "high-contrast"].get(mode as usize).copied().unwrap_or("light"));
    }
}

/// Host token overrides as a JSON object.
#[no_mangle]
pub extern "C" fn datars_view_set_tokens(v: *mut DatarsView, json: *const u8, len: usize) -> i32 {
    let Some(v) = view(v) else { return 0 };
    match serde_json::from_str(&s(json, len)) {
        Ok(t) => {
            v.core.engine.set_host_tokens(t);
            1
        }
        Err(_) => 0,
    }
}

/// Data the engine waits for, as JSON `[{"name", "url"} | {"name", "slot"}]`: URL sources (first
/// loads and live refreshes) the host fetches relative to the bundle and hands to
/// `datars_view_provide_source`; slots are for the app to fill.
#[no_mangle]
pub extern "C" fn datars_view_data_requests(v: *mut DatarsView) -> *mut c_char {
    let Some(v) = view(v) else { return std::ptr::null_mut() };
    let reqs: Vec<serde_json::Value> = v
        .core
        .engine
        .requests()
        .iter()
        .filter_map(|r| match r {
            datars_runtime::datars_engine::Request::Source { name, url } => Some(serde_json::json!({ "name": name, "url": url })),
            datars_runtime::datars_engine::Request::Slot { name, slot } => Some(serde_json::json!({ "name": name, "slot": slot })),
            datars_runtime::datars_engine::Request::Range { name, url, offset, length } => Some(serde_json::json!({ "name": name, "url": url, "range": [offset, length] })),
            _ => None,
        })
        .collect();
    out(serde_json::Value::Array(reqs))
}

/// Hand over bytes `offset …` of a tiles source's archive (answering a `range` data request).
/// Returns 1 on success.
#[no_mangle]
pub extern "C" fn datars_view_provide_range(v: *mut DatarsView, name: *const u8, name_len: usize, offset: u64, bytes: *const u8, len: usize) -> i32 {
    view(v).map_or(0, |v| v.core.engine.provide_range(&s(name, name_len), offset, b(bytes, len)).is_ok() as i32)
}

/// State, narration, semantics (accessibility tree), tokens, diagnostics — as JSON.
#[no_mangle]
pub extern "C" fn datars_view_status(v: *mut DatarsView) -> *mut c_char {
    let Some(v) = view(v) else { return std::ptr::null_mut() };
    out(v.core.status())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_doc_renders_through_the_c_abi() {
        let doc = br#"{"datars":1,"size":{"width":120,"height":80},"scene":{"kind":"shape","geom":{"type":"rect","x":10,"y":10,"w":50,"h":30},"fill":"$accent"}}"#;
        let v = datars_view_new(0);
        let r = datars_view_load_doc(v, doc.as_ptr(), doc.len());
        datars_string_free(r);
        datars_view_resize(v, 120.0, 80.0, 1.0);
        datars_view_frame(v, 0.0);
        let (mut w, mut h) = (0u32, 0u32);
        let p = datars_view_pixels(v, &mut w, &mut h);
        assert_eq!((w, h), (120, 80));
        // SAFETY: the buffer is w*h*4 bytes.
        let px = unsafe { std::slice::from_raw_parts(p, (w * h * 4) as usize) };
        let i = ((20 * w + 20) * 4) as usize;
        assert_eq!(&px[i..i + 3], &[0x42, 0x69, 0xd0], "the accent fill");
        let st = datars_view_status(v);
        datars_string_free(st);
        datars_view_free(v);
    }
}

/// Draw frames with the GPU (Metal) on the app's `CAMetalLayer`, `width × height` physical pixels
/// (the view's size × its scale). Returns 1 when frames go to the layer from now on, 0 when there's
/// no GPU for it — frames stay CPU pixels. The layer must stay alive until the view is freed or
/// `datars_view_detach_gpu`.
#[cfg(all(feature = "gpu", any(target_os = "ios", target_os = "macos")))]
#[no_mangle]
pub extern "C" fn datars_view_attach_metal_layer(v: *mut DatarsView, layer: *mut std::ffi::c_void, width: u32, height: u32) -> i32 {
    let Some(v) = view(v) else { return 0 };
    if layer.is_null() {
        return 0;
    }
    v.gpu = None;
    let target = move || datars_render_wgpu::wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(layer);
    // SAFETY: the host keeps the layer alive while the view draws into it (documented contract).
    let dpr = v.dpr;
    match std::panic::catch_unwind(move || unsafe { datars_render_wgpu::Renderer::for_raw_surface(target, width.max(1), height.max(1), dpr) }) {
        Ok(Ok(mut r)) => {
            r.set_work_scale(v.work);
            v.gpu = Some(r);
            1
        }
        _ => 0,
    }
}

/// Stop drawing on the GPU surface (before the host tears it down); frames are CPU pixels again.
#[no_mangle]
pub extern "C" fn datars_view_detach_gpu(v: *mut DatarsView) {
    if let Some(v) = view(v) {
        #[cfg(feature = "gpu")]
        {
            v.gpu = None;
        }
        #[cfg(all(feature = "gpu", target_os = "android"))]
        android::release_window(v);
    }
}

/// Which GPU backend draws the view ("Metal", "Vulkan", "Gl"), or null on CPU pixels. Free with
/// `datars_string_free`.
#[no_mangle]
pub extern "C" fn datars_view_gpu_backend(v: *mut DatarsView) -> *mut c_char {
    let Some(_v) = view(v) else { return std::ptr::null_mut() };
    #[cfg(feature = "gpu")]
    if let Some(r) = &_v.gpu {
        return CString::new(format!("{} {}×MSAA", r.backend(), r.samples())).map(|c| c.into_raw()).unwrap_or(std::ptr::null_mut());
    }
    std::ptr::null_mut()
}

/// The last frame, as JSON: `engine` and `render` time (ms; on CPU pixels `engine` is the whole
/// frame and `render` null), and from the GPU renderer `draws`, `uploaded` (bytes of new meshes),
/// `rebuilt` (instances built and uploaded), `tessellated` (meshes) and `standIns` (meshes still
/// waiting for their share). For benchmarks and stats readouts. Free with `datars_string_free`.
#[no_mangle]
pub extern "C" fn datars_view_frame_stats(v: *mut DatarsView) -> *mut c_char {
    let Some(v) = view(v) else { return std::ptr::null_mut() };
    let r = |x: f64| (x * 100.0).round() / 100.0;
    #[allow(unused_mut)]
    let mut stats = serde_json::json!({ "engine": r(v.frame_ms.0), "render": r(v.frame_ms.1) });
    #[cfg(feature = "gpu")]
    if let Some(g) = &v.gpu {
        let s = g.stats();
        stats["draws"] = s.draws.into();
        stats["uploaded"] = s.arena_bytes.into();
        stats["rebuilt"] = s.instances_rebuilt.into();
        stats["tessellated"] = (s.tessellations - v.tessellated).into();
        stats["standIns"] = s.stand_ins.into();
    }
    out(stats)
}

/// JNI entry points for the Kotlin library (`dev.datars.Native`). Thin: they forward to the C ABI.
#[cfg(target_os = "android")]
pub mod android {
    use super::*;
    use jni::objects::{JByteArray, JClass, JObject, JString};
    use jni::sys::{jboolean, jbyteArray, jdouble, jint, jlong, jstring};
    use jni::JNIEnv;

    #[cfg(feature = "gpu")]
    #[link(name = "android")]
    extern "C" {
        fn ANativeWindow_fromSurface(env: *mut jni::sys::JNIEnv, surface: jni::sys::jobject) -> *mut std::ffi::c_void;
        fn ANativeWindow_release(window: *mut std::ffi::c_void);
    }

    #[link(name = "log")]
    extern "C" {
        fn __android_log_write(prio: i32, tag: *const c_char, text: *const c_char) -> i32;
    }

    /// A line in logcat under the `datars` tag (6: error, 4: info).
    pub(crate) fn log(prio: i32, msg: &str) {
        let text = CString::new(msg.replace('\0', " ")).unwrap_or_default();
        // SAFETY: NUL-terminated strings that outlive the call.
        unsafe { __android_log_write(prio, c"datars".as_ptr(), text.as_ptr()) };
    }

    /// Rust panics and warnings to logcat: Android drops stderr, and a panic would otherwise leave
    /// only a SIGABRT with no message — or a GPU surface that fails, only its caller's guess.
    fn log_panics() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            std::panic::set_hook(Box::new(|info| log(6, &format!("panic: {info}"))));
            if ::log::set_logger(&Logcat).is_ok() {
                ::log::set_max_level(::log::LevelFilter::Warn);
            }
        });
    }

    struct Logcat;
    impl ::log::Log for Logcat {
        fn enabled(&self, m: &::log::Metadata) -> bool {
            m.level() <= ::log::Level::Warn
        }
        fn log(&self, r: &::log::Record) {
            if self.enabled(r.metadata()) {
                log(if r.level() == ::log::Level::Error { 6 } else { 5 }, &format!("{}: {}", r.target(), r.args()));
            }
        }
        fn flush(&self) {}
    }

    /// Release the view's Android window (after its renderer is gone).
    #[cfg(feature = "gpu")]
    pub(crate) fn release_window(v: &mut DatarsView) {
        v.gpu = None;
        if !v.window.is_null() {
            // SAFETY: acquired by `ANativeWindow_fromSurface` in `attachSurface`, released once.
            unsafe { ANativeWindow_release(v.window) };
            v.window = std::ptr::null_mut();
        }
    }

    impl Drop for DatarsView {
        fn drop(&mut self) {
            #[cfg(feature = "gpu")]
            release_window(self);
        }
    }

    /// Draw frames with the GPU (Vulkan, else GLES; GLES first with `gl_first`, for emulators) on
    /// an `android.view.Surface`, `w × h` physical pixels. False when there's no GPU for it: frames
    /// stay CPU pixels. Call `detachSurface` from `surfaceDestroyed`, before the surface goes.
    #[cfg(feature = "gpu")]
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_attachSurface(env: JNIEnv, _: JClass, v: jlong, surface: JObject, w: jint, h: jint, gl_first: jboolean) -> jboolean {
        use datars_render_wgpu::wgpu::{self, rwh};
        let Some(view) = view(v as *mut DatarsView) else { return 0 };
        release_window(view);
        // SAFETY: a live JNIEnv and Surface for the duration of the call; the window holds its own
        // reference to the surface until released.
        let window = unsafe { ANativeWindow_fromSurface(env.get_raw(), surface.as_raw()) };
        let Some(ptr) = std::ptr::NonNull::new(window) else { return 0 };
        let target = move || wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(rwh::RawDisplayHandle::Android(rwh::AndroidDisplayHandle::new())),
            raw_window_handle: rwh::RawWindowHandle::AndroidNdk(rwh::AndroidNdkWindowHandle::new(ptr)),
        };
        // SAFETY: the window stays acquired until `release_window`, which drops the renderer first.
        let dpr = view.dpr;
        let order: &[wgpu::Backends] = if gl_first != 0 { &[wgpu::Backends::GL, wgpu::Backends::PRIMARY] } else { &[wgpu::Backends::PRIMARY, wgpu::Backends::GL] };
        let made = std::panic::catch_unwind(move || unsafe { datars_render_wgpu::Renderer::for_raw_surface_in(order, target, w.max(1) as u32, h.max(1) as u32, dpr) });
        match made {
            Ok(Ok(mut r)) => {
                r.set_work_scale(view.work);
                view.gpu = Some(r);
                view.window = window;
                1
            }
            failed => {
                if let Ok(Err(e)) = failed {
                    log(4, &format!("no GPU for the surface: {e}"));
                }
                // SAFETY: acquired above and not kept.
                unsafe { ANativeWindow_release(window) };
                0
            }
        }
    }
    #[cfg(feature = "gpu")]
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_detachSurface(_: JNIEnv, _: JClass, v: jlong) {
        datars_view_detach_gpu(v as *mut DatarsView)
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_setWorkScale(_: JNIEnv, _: JClass, v: jlong, scale: jdouble) {
        datars_view_set_work_scale(v as *mut DatarsView, scale)
    }
    /// Render a frame onto the attached surface (no pixels cross); true while the next is due.
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_drawFrame(_: JNIEnv, _: JClass, v: jlong, now: jdouble) -> jboolean {
        (datars_view_frame(v as *mut DatarsView, now) != 0) as jboolean
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_frameStats(mut env: JNIEnv, _: JClass, v: jlong) -> jstring {
        let r = datars_view_frame_stats(v as *mut DatarsView);
        to_j(&mut env, r)
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_gpuBackend(mut env: JNIEnv, _: JClass, v: jlong) -> jstring {
        let r = datars_view_gpu_backend(v as *mut DatarsView);
        to_j(&mut env, r)
    }

    fn jstr(env: &mut JNIEnv, s: &JString) -> String {
        env.get_string(s).map(|s| s.into()).unwrap_or_default()
    }
    fn to_j(env: &mut JNIEnv, p: *mut c_char) -> jstring {
        if p.is_null() {
            return std::ptr::null_mut();
        }
        // SAFETY: p is a CString from this crate.
        let s = unsafe { std::ffi::CStr::from_ptr(p) }.to_string_lossy().into_owned();
        datars_string_free(p);
        env.new_string(s).map(|j| j.into_raw()).unwrap_or(std::ptr::null_mut())
    }

    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_viewNew(_: JNIEnv, _: JClass, allow_script: jboolean) -> jlong {
        log_panics();
        datars_view_new(allow_script as i32) as jlong
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_viewFree(_: JNIEnv, _: JClass, v: jlong) {
        datars_view_free(v as *mut DatarsView)
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_loadDoc(mut env: JNIEnv, _: JClass, v: jlong, json: JString) -> jstring {
        let s = jstr(&mut env, &json);
        let r = datars_view_load_doc(v as *mut DatarsView, s.as_ptr(), s.len());
        to_j(&mut env, r)
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_openFile(mut env: JNIEnv, _: JClass, v: jlong, bytes: JByteArray) -> jstring {
        let b = env.convert_byte_array(&bytes).unwrap_or_default();
        let r = datars_view_open_file(v as *mut DatarsView, b.as_ptr(), b.len());
        to_j(&mut env, r)
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_openManifest(mut env: JNIEnv, _: JClass, v: jlong, bytes: JByteArray) -> jstring {
        let b = env.convert_byte_array(&bytes).unwrap_or_default();
        let r = datars_view_open_manifest(v as *mut DatarsView, b.as_ptr(), b.len());
        to_j(&mut env, r)
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_provideChunk(mut env: JNIEnv, _: JClass, v: jlong, hash: JString, bytes: JByteArray) -> jstring {
        let h = jstr(&mut env, &hash);
        let b = env.convert_byte_array(&bytes).unwrap_or_default();
        let r = datars_view_provide_chunk(v as *mut DatarsView, h.as_ptr(), h.len(), b.as_ptr(), b.len());
        to_j(&mut env, r)
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_resize(_: JNIEnv, _: JClass, v: jlong, w: jdouble, h: jdouble, dpr: jdouble) {
        datars_view_resize(v as *mut DatarsView, w, h, dpr)
    }
    /// Render a frame; returns RGBA pixels (width, height via `lastSize`) — null if empty.
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_frame(env: JNIEnv, _: JClass, v: jlong, now: jdouble) -> jbyteArray {
        datars_view_frame(v as *mut DatarsView, now);
        let (mut w, mut h) = (0u32, 0u32);
        let p = datars_view_pixels(v as *mut DatarsView, &mut w, &mut h);
        if p.is_null() || w == 0 {
            return std::ptr::null_mut();
        }
        // SAFETY: w*h*4 bytes owned by the view until the next frame.
        let px = unsafe { std::slice::from_raw_parts(p, (w * h * 4) as usize) };
        env.byte_array_from_slice(px).map(|a| a.into_raw()).unwrap_or(std::ptr::null_mut())
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_lastWidth(_: JNIEnv, _: JClass, v: jlong) -> jint {
        view(v as *mut DatarsView).map(|v| v.width as jint).unwrap_or(0)
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_lastHeight(_: JNIEnv, _: JClass, v: jlong) -> jint {
        view(v as *mut DatarsView).map(|v| v.height as jint).unwrap_or(0)
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_animating(_: JNIEnv, _: JClass, v: jlong) -> jboolean {
        (datars_view_animating(v as *mut DatarsView) != 0) as jboolean
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_setClock(_: JNIEnv, _: JClass, v: jlong, now: jdouble) {
        datars_view_set_clock(v as *mut DatarsView, now)
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_pointer(mut env: JNIEnv, _: JClass, v: jlong, kind: jint, x: jdouble, y: jdouble) -> jstring {
        let r = datars_view_pointer(v as *mut DatarsView, kind, x, y);
        to_j(&mut env, r)
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_event(mut env: JNIEnv, _: JClass, v: jlong, name: JString) -> jboolean {
        let s = jstr(&mut env, &name);
        (datars_view_event(v as *mut DatarsView, s.as_ptr(), s.len()) != 0) as jboolean
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_setMode(_: JNIEnv, _: JClass, v: jlong, mode: jint) {
        datars_view_set_mode(v as *mut DatarsView, mode)
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_provideRange(mut env: JNIEnv, _: JClass, v: jlong, name: JString, offset: jlong, bytes: JByteArray) -> jboolean {
        let n = jstr(&mut env, &name);
        let b = env.convert_byte_array(&bytes).unwrap_or_default();
        (datars_view_provide_range(v as *mut DatarsView, n.as_ptr(), n.len(), offset.max(0) as u64, b.as_ptr(), b.len()) != 0) as jboolean
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_activate(mut env: JNIEnv, _: JClass, v: jlong, path: JString) -> jboolean {
        let p = jstr(&mut env, &path);
        (datars_view_activate(v as *mut DatarsView, p.as_ptr(), p.len()) != 0) as jboolean
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_setSignalNum(mut env: JNIEnv, _: JClass, v: jlong, name: JString, value: jdouble) {
        let n = jstr(&mut env, &name);
        datars_view_set_signal_num(v as *mut DatarsView, n.as_ptr(), n.len(), value)
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_setSignalJson(mut env: JNIEnv, _: JClass, v: jlong, name: JString, json: JString) -> jboolean {
        let (n, j) = (jstr(&mut env, &name), jstr(&mut env, &json));
        (datars_view_set_signal_json(v as *mut DatarsView, n.as_ptr(), n.len(), j.as_ptr(), j.len()) != 0) as jboolean
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_dataRequests(mut env: JNIEnv, _: JClass, v: jlong) -> jstring {
        let r = datars_view_data_requests(v as *mut DatarsView);
        to_j(&mut env, r)
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_provideSource(mut env: JNIEnv, _: JClass, v: jlong, name: JString, bytes: JByteArray) -> jboolean {
        let n = jstr(&mut env, &name);
        let b = env.convert_byte_array(&bytes).unwrap_or_default();
        (datars_view_provide_source(v as *mut DatarsView, n.as_ptr(), n.len(), b.as_ptr(), b.len()) != 0) as jboolean
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_status(mut env: JNIEnv, _: JClass, v: jlong) -> jstring {
        let r = datars_view_status(v as *mut DatarsView);
        to_j(&mut env, r)
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_gotoState(_: JNIEnv, _: JClass, v: jlong, index: jint) -> jboolean {
        (datars_view_goto(v as *mut DatarsView, index.max(0) as u32) != 0) as jboolean
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_seek(_: JNIEnv, _: JClass, v: jlong, pos: jdouble) -> jint {
        datars_view_seek(v as *mut DatarsView, pos) as jint
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_setPlaying(_: JNIEnv, _: JClass, v: jlong, on: jboolean) {
        datars_view_set_playing(v as *mut DatarsView, on as i32)
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_setReducedMotion(_: JNIEnv, _: JClass, v: jlong, on: jboolean) {
        datars_view_set_reduced_motion(v as *mut DatarsView, on as i32)
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_wakeAt(_: JNIEnv, _: JClass, v: jlong) -> jdouble {
        datars_view_wake_at(v as *mut DatarsView)
    }
    #[no_mangle]
    pub extern "system" fn Java_dev_datars_Native_pixelHash(_: JNIEnv, _: JClass, v: jlong) -> jlong {
        datars_view_pixel_hash(v as *mut DatarsView) as jlong
    }
}
