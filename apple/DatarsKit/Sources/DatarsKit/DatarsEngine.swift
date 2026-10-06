import CDatars
import Foundation

/// A datars runtime instance: load a document or a bundle, render frames, send events.
public final class DatarsEngine {
    /// Where relative data URLs resolve: the bundle's address (set by `load(.url)`).
    public var dataBase: URL?

    let view: OpaquePointer

    public init(allowScript: Bool = true) {
        view = datars_view_new(allowScript ? 1 : 0)
    }

    deinit { datars_view_free(view) }

    private func json(_ p: UnsafeMutablePointer<CChar>?) -> [String: Any] {
        guard let p else { return [:] }
        defer { datars_string_free(p) }
        let s = String(cString: p)
        return (try? JSONSerialization.jsonObject(with: Data(s.utf8))) as? [String: Any] ?? ["value": s]
    }

    private func withBytes<T>(_ d: Data, _ f: (UnsafePointer<UInt8>, Int) -> T) -> T {
        d.withUnsafeBytes { raw in f(raw.bindMemory(to: UInt8.self).baseAddress!, d.count) }
    }

    @discardableResult
    public func loadDocument(json doc: String) -> [String: Any] {
        withBytes(Data(doc.utf8)) { json(datars_view_load_doc(view, $0, $1)) }
    }

    /// Open a single-file `.datars` bundle.
    @discardableResult
    public func openBundle(file: Data) -> [String: Any] {
        withBytes(file) { json(datars_view_open_file(view, $0, $1)) }
    }

    /// Open a manifest; fetch the returned chunk hashes and hand them to `provide(chunk:bytes:)`.
    public func openManifest(_ data: Data) -> [String] {
        let r = withBytes(data) { json(datars_view_open_manifest(view, $0, $1)) }
        return r["requests"] as? [String] ?? []
    }

    public func provide(chunk hash: String, bytes: Data) -> [String] {
        let h = Data(hash.utf8)
        let r = h.withUnsafeBytes { hr in bytes.withUnsafeBytes { br in
            json(datars_view_provide_chunk(view, hr.bindMemory(to: UInt8.self).baseAddress!, h.count, br.bindMemory(to: UInt8.self).baseAddress!, bytes.count))
        } }
        return r["requests"] as? [String] ?? []
    }

    /// Fill a data slot with the app's own rows (they never leave the device).
    @discardableResult
    public func provide(source name: String, csvOrJSON: Data) -> Bool {
        let n = Data(name.utf8)
        return n.withUnsafeBytes { nr in csvOrJSON.withUnsafeBytes { br in
            datars_view_provide_source(view, nr.bindMemory(to: UInt8.self).baseAddress!, n.count, br.bindMemory(to: UInt8.self).baseAddress!, csvOrJSON.count) != 0
        } }
    }

    public func resize(width: Double, height: Double, scale: Double) {
        datars_view_resize(view, width, height, scale)
    }

    /// Render a frame (onto the attached Metal layer, else into `pixels()`); returns true while the
    /// next one is due (keep the display link running).
    @discardableResult
    public func frame(now: Double) -> Bool { datars_view_frame(view, now) != 0 }

    /// Draw frames with Metal on `layer`, `width × height` physical pixels. False without a GPU
    /// (frames stay CPU pixels). The layer must outlive the engine or `detachGPU()`.
    public func attachMetal(layer: AnyObject, width: Int, height: Int) -> Bool {
        datars_view_attach_metal_layer(view, Unmanaged.passUnretained(layer).toOpaque(), UInt32(max(1, width)), UInt32(max(1, height))) != 0
    }

    /// Back to CPU pixels (before tearing the layer down).
    public func detachGPU() { datars_view_detach_gpu(view) }

    /// "Metal 4×MSAA" while frames go to the GPU, nil on CPU pixels.
    public var gpuBackend: String? {
        guard let p = datars_view_gpu_backend(view) else { return nil }
        defer { datars_string_free(p) }
        return String(cString: p)
    }

    /// The frame clock (seconds), set by the view that renders this engine. Inputs tell the engine
    /// the time first: frames stop while nothing moves, and a transition started at the last
    /// frame's time would count as long started (a camera flight would jump to its end).
    public var clock: (() -> Double)?
    private func sync() { if let clock { datars_view_set_clock(view, clock()) } }

    /// The last frame as RGBA8 (straight alpha).
    public func pixels() -> (width: Int, height: Int, data: Data) {
        var w: UInt32 = 0, h: UInt32 = 0
        guard let p = datars_view_pixels(view, &w, &h), w > 0, h > 0 else { return (0, 0, Data()) }
        return (Int(w), Int(h), Data(bytes: p, count: Int(w * h * 4)))
    }

    /// `tap`: a touch press released where it went down — a click that also inspects what it
    /// lands on (touch has no hover), with a finger's reach.
    public enum PointerKind: Int32 { case move = 0, down, up, leave, tap }

    /// Pointer input in points; returns the inspected element's label (for a tooltip).
    public func pointer(_ kind: PointerKind, x: Double, y: Double) -> String? {
        sync()
        guard let p = datars_view_pointer(view, kind.rawValue, x, y) else { return nil }
        defer { datars_string_free(p) }
        return (try? JSONSerialization.jsonObject(with: Data(String(cString: p).utf8), options: .fragmentsAllowed)) as? String
    }

    /// What the pointer should look like where it last was: a pointing hand over what a click acts
    /// on, grab over a view that pans, a crosshair over a brushable area.
    public enum Cursor: Int32 { case arrow = 0, pointer, grab, grabbing, crosshair }
    public var cursor: Cursor { Cursor(rawValue: datars_view_cursor(view)) ?? .arrow }

    @discardableResult
    public func send(_ event: String) -> Bool {
        sync()
        return withBytes(Data(event.utf8)) { datars_view_event(view, $0, $1) != 0 }
    }

    public enum Mode: Int32 { case light = 0, dark, highContrast }
    public func setMode(_ m: Mode) { datars_view_set_mode(view, m.rawValue) }

    /// Token overrides (brand colours, fonts) — honours the theme's locks.
    public func setTokens(_ tokens: [String: Any]) {
        guard let d = try? JSONSerialization.data(withJSONObject: tokens) else { return }
        _ = withBytes(d) { datars_view_set_tokens(view, $0, $1) }
    }

    /// State, narration, the accessibility tree, resolved tokens.
    public func status() -> [String: Any] { json(datars_view_status(view)) }

    /// The last frame: `engine` and `render` time (ms), and on the GPU `draws`, `uploaded` (bytes
    /// of new meshes), `rebuilt` (instances), `tessellated` (meshes), `standIns`.
    public var frameStats: [String: Any] { json(datars_view_frame_stats(view)) }

    /// Data the engine waits for: URL sources (`name`, `url`: first loads, live refreshes) and slots
    /// (`name`, `slot`) for the app to fill with `provide(source:csvOrJSON:)`.
    public func dataRequests() -> [[String: Any]] {
        guard let p = datars_view_data_requests(view) else { return [] }
        defer { datars_string_free(p) }
        let data = Data(String(cString: p).utf8)
        return (try? JSONSerialization.jsonObject(with: data)) as? [[String: Any]] ?? []
    }

    /// Hand over bytes of a tiles archive (answering a `range` data request).
    @discardableResult
    public func provide(range name: String, offset: UInt64, bytes: Data) -> Bool {
        withBytes(Data(name.utf8)) { np, nn in withBytes(bytes) { bp, bn in datars_view_provide_range(view, np, nn, offset, bp, bn) } } != 0
    }

    /// The last frame's pixel identity (hex) — the same on every platform (P1).
    public func pixelHash() -> String { String(format: "%016llx", datars_view_pixel_hash(view)) }

    /// Scroll scrub: show program position `pos` (state + fraction toward the next). Returns the
    /// state index now current.
    @discardableResult
    public func seek(_ pos: Double) -> Int { Int(datars_view_seek(view, pos)) }

    /// Wheel/pinch zoom at a point; true if an explorable view took it.
    @discardableResult
    public func zoom(atX x: Double, y: Double, delta: Double) -> Bool { sync(); return datars_view_wheel(view, x, y, delta) != 0 }

    /// Run the click intent of the mark at `path` (a semantics item's `path`): assistive activation.
    @discardableResult
    public func activate(path: String) -> Bool {
        sync()
        return withBytes(Data(path.utf8)) { p, n in datars_view_activate(view, p, n) } != 0
    }

    /// Set a numeric signal (app state, or a native control bound to an engine-drawn one).
    public func setSignal(_ name: String, _ value: Double) {
        sync()
        withBytes(Data(name.utf8)) { p, n in datars_view_set_signal_num(view, p, n, value) }
    }

    /// Set a signal to a JSON value — a string, number, boolean, key set or null: what a native
    /// picker chose for an engine-drawn select. False when the JSON doesn't parse.
    @discardableResult
    public func setSignal(_ name: String, json: String) -> Bool {
        sync()
        return withBytes(Data(name.utf8)) { np, nn in
            withBytes(Data(json.utf8)) { jp, jn in datars_view_set_signal_json(view, np, nn, jp, jn) }
        } != 0
    }

    /// Reduced motion (Settings › Accessibility › Motion): transitions become short crossfades.
    public func setReducedMotion(_ on: Bool) { datars_view_set_reduced_motion(view, on ? 1 : 0) }
    /// The share (0.1–1, default 1) of the per-frame work budgets frames may spend: points built,
    /// tiles decoded and styled, meshes tessellated. Lower it when frames run long; `DatarsView`
    /// does this by itself.
    public func setWorkScale(_ scale: Double) { datars_view_set_work_scale(view, scale) }

    /// Pause or resume autoplay (pause while the reader interacts, for Reduce Motion, off-screen).
    public func setPlaying(_ on: Bool) { sync(); datars_view_set_playing(view, on ? 1 : 0) }

    /// When the next frame is due although nothing moves (seconds, the frame clock), if scheduled.
    public var wakeAt: Double? { let w = datars_view_wake_at(view); return w.isNaN ? nil : w }

    /// Jump to program state `index`; the next frames play the transition.
    @discardableResult
    public func goto(_ index: Int) -> Bool { sync(); return datars_view_goto(view, UInt32(index)) != 0 }
}
