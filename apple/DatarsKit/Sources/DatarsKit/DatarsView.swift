import Foundation
import QuartzCore
import os
#if canImport(UIKit)
import UIKit
public typealias PlatformView = UIView
#elseif canImport(AppKit)
import AppKit
public typealias PlatformView = NSView
#endif
#if canImport(SwiftUI)
import SwiftUI
#endif

/// Loads a chart bundle over the network (sans-IO loop over the runtime's requests).
public enum DatarsSource {
    case url(URL)          // a manifest (alias or pinned) or a single `.datars` file
    case document(String)  // a document JSON (development)
}

public extension DatarsEngine {
    func load(_ source: DatarsSource) async throws {
        switch source {
        case .document(let json):
            loadDocument(json: json)
        case .url(let url):
            dataBase = url
            let (data, _) = try await URLSession.shared.data(from: url)
            if url.pathExtension == "datars" {
                openBundle(file: data)
                return
            }
            let base = url.deletingLastPathComponent().deletingLastPathComponent()
            var need = openManifest(data)
            while !need.isEmpty {
                var next: [String] = []
                for h in need {
                    let (bytes, _) = try await URLSession.shared.data(from: base.appendingPathComponent("chunks").appendingPathComponent(h.replacingOccurrences(of: ":", with: "_")))
                    next = provide(chunk: h, bytes: bytes)
                }
                need = next
            }
        }
    }
}

/// A view that shows a datars chart. Renders on demand: frames run only while something moves.
/// Frames go to the GPU (Metal, on a layer of the view's own) where there is one; elsewhere the CPU
/// reference draws them into an image.
open class DatarsChartView: PlatformView {
    public let engine: DatarsEngine
    private var image: CGImage?
    /// The layer Metal presents frames on, once `engine.attachMetal` took it.
    private let metal = CAMetalLayer()
    private var gpu = false
    /// In the background: no frames (the system refuses GPU work there).
    private var suspended = false
    /// The work share this view's engine was last given (see `Work`).
    private var workTold = 1.0

    /// Every frame, for a benchmark or a stats readout: the CPU time it took (the engine, and the
    /// GPU encode and present; ms), whether something moved, and when it was drawn (s).
    public var onFrame: ((_ cpuMs: Double, _ moving: Bool, _ at: CFTimeInterval) -> Void)?
    /// Called once a chart has loaded (and its first frame is drawn).
    public var onLoad: (() -> Void)?
    #if canImport(UIKit)
    /// Every tap on the chart: the label of what it landed on (nil: nothing), and where. Touch has
    /// no hover, so a tap is how a reader asks what a mark is — a line's value anywhere along it.
    public var onTap: ((_ label: String?, _ at: CGPoint) -> Void)?
    /// Show what a tap lands on in a small label above the finger, until the next tap or the next
    /// step (default). Off, apps show `onTap`'s label their own way.
    public var showsTooltips = true
    private let tip = DatarsTooltip()
    /// Where the touch being tracked went down: released there, it's a tap.
    private var touchStart: CGPoint?
    #if canImport(UIKit)
    /// The platform's own picker over each engine-drawn select: an invisible button whose menu
    /// lists the options (a tap opens it instead of the chart's drawn list).
    private var pickers: [UIButton] = []
    private var pickerKey = ""
    #endif
    #endif
    private let start = Date()
    #if canImport(UIKit)
    private var link: CADisplayLink?
    #else
    private var timer: Timer?
    #endif
    /// A one-shot timer for the engine's next scheduled frame (autoplay hold, live refresh).
    private var wake: Timer?
    /// Data requests being fetched (by source name).
    private var fetching = Set<String>()

    /// Fetch what the engine asks for (URL sources: first loads, live refreshes), relative to the
    /// bundle, and hand it over.
    private func fetchData() {
        for r in engine.dataRequests() {
            // Absolute and standardized: a relative URL handed to URLSession goes out with its dot
            // segments (`/c/../../assets/…`), which servers rightly refuse.
            guard let name = r["name"] as? String, let rel = r["url"] as? String,
                  let url = URL(string: rel, relativeTo: engine.dataBase)?.absoluteURL.standardized else { continue }
            // Tile archives come in byte ranges (HTTP Range); other sources whole.
            let range = r["range"] as? [NSNumber]
            let id = range.map { "\(name)@\($0[0])" } ?? name
            guard !fetching.contains(id) else { continue }
            fetching.insert(id)
            Task { @MainActor in
                defer { fetching.remove(id) }
                var req = URLRequest(url: url)
                req.cachePolicy = .reloadIgnoringLocalCacheData
                if let range {
                    let (off, len) = (range[0].uint64Value, range[1].uint64Value)
                    req.setValue("bytes=\(off)-\(off + len - 1)", forHTTPHeaderField: "Range")
                    if let (data, _) = try? await URLSession.shared.data(for: req) {
                        // A server that ignores Range (or a file: URL) sends the whole archive:
                        // slice it.
                        let part = data.count != Int(len) && data.count >= Int(off + len) ? data.subdata(in: Int(off)..<Int(off + len)) : data
                        engine.provide(range: name, offset: off, bytes: part)
                        requestFrame()
                    }
                } else if let (data, _) = try? await URLSession.shared.data(for: req) {
                    _ = engine.provide(source: name, csvOrJSON: data)
                    requestFrame()
                }
            }
        }
    }

    public init(engine: DatarsEngine = DatarsEngine()) {
        self.engine = engine
        super.init(frame: .zero)
        let start = self.start
        engine.clock = { Date().timeIntervalSince(start) }
        metal.isOpaque = false
        metal.isHidden = true
        #if canImport(UIKit)
        isAccessibilityElement = false
        layer.addSublayer(metal)
        addSubview(tip)
        NotificationCenter.default.addObserver(self, selector: #selector(motionSettingChanged), name: UIAccessibility.reduceMotionStatusDidChangeNotification, object: nil)
        NotificationCenter.default.addObserver(self, selector: #selector(didEnterBackground), name: UIApplication.didEnterBackgroundNotification, object: nil)
        NotificationCenter.default.addObserver(self, selector: #selector(willEnterForeground), name: UIApplication.willEnterForegroundNotification, object: nil)
        #else
        wantsLayer = true
        layer?.addSublayer(metal)
        NSWorkspace.shared.notificationCenter.addObserver(self, selector: #selector(motionSettingChanged), name: NSWorkspace.accessibilityDisplayOptionsDidChangeNotification, object: nil)
        #endif
        applyMotionSetting()
        workTold = Work.share
        engine.setWorkScale(workTold)
    }

    /// The layer goes with the view; the engine may outlive it.
    deinit { engine.detachGPU() }

    static let log = Logger(subsystem: "dev.datars", category: "render")

    @objc private func didEnterBackground() {
        suspended = true
        #if canImport(UIKit)
        link?.invalidate()
        link = nil
        #endif
    }

    @objc private func willEnterForeground() {
        suspended = false
        relayout()
        renderLoop()
    }

    /// Follow the system's Reduce Motion setting.
    private func applyMotionSetting() {
        #if canImport(UIKit)
        engine.setReducedMotion(UIAccessibility.isReduceMotionEnabled)
        #else
        engine.setReducedMotion(NSWorkspace.shared.accessibilityDisplayShouldReduceMotion)
        #endif
    }

    @objc private func motionSettingChanged() { applyMotionSetting(); renderLoop() }

    required public init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    private var loaded = false

    /// The program state to show (SwiftUI's `DatarsChart(state:)`): applied now if the chart is
    /// loaded, else as soon as it is — the transition plays from where the chart is.
    public var targetState: Int? {
        didSet { applyTargetState() }
    }

    private func applyTargetState() {
        guard loaded, let s = targetState else { return }
        if engine.goto(s) {
            #if canImport(UIKit)
            tip.isHidden = true // the chart moves on; so does what the tap was on
            #endif
            renderLoop()
        }
    }

    /// The app's own rows for the chart's data slots, by slot name (CSV or JSON). They never leave
    /// the device; set them any time — before the chart has loaded they wait for it, and new rows
    /// transition the chart like any other change. A slot keeps its sample until it's given rows.
    public var data: [String: Data] = [:] {
        didSet { applyData() }
    }
    private var appliedData: [String: Data] = [:]

    private func applyData() {
        guard loaded else { return }
        var changed = false
        for (name, bytes) in data where appliedData[name] != bytes {
            // Rows without the columns the chart needs are refused; it keeps what it showed.
            if engine.provide(source: name, csvOrJSON: bytes) { appliedData[name] = bytes; changed = true }
        }
        if changed { renderLoop(); updateAccessibility() }
    }

    public func load(_ source: DatarsSource) {
        Task { @MainActor in
            try? await engine.load(source)
            loaded = true
            appliedData = [:]
            applyData()
            relayout()
            renderLoop()
            applyTargetState()
            updateAccessibility()
            onLoad?()
        }
    }

    /// Step through every state of the chart, `hold` seconds apart, timing each transition frame
    /// by frame — the web runtime's frame profiler's summary: frames, fps, the worst gap between
    /// frames, 60 Hz frames dropped, CPU per frame (p50, p95) and its engine and render parts, and
    /// the GPU work behind them (draws, bytes uploaded, instances rebuilt, meshes tessellated).
    /// `done` gets one per transition. For measuring a chart on a device (the sample app's
    /// `DATARS_BENCH=1`).
    public func runBenchmark(hold: Double = 2.5, done: @escaping ([[String: Any]]) -> Void) {
        let names = engine.status()["states"] as? [String] ?? []
        guard names.count > 1 else { done([]); return }
        var results: [[String: Any]] = []
        var frames: [Frame] = []
        var recording = false
        let before = onFrame
        onFrame = { [unowned self] cpu, moving, at in
            before?(cpu, moving, at)
            if recording && moving { frames.append(Frame(cpu: cpu, at: at, stats: engine.frameStats)) }
        }
        func step(_ i: Int) {
            guard i < names.count else {
                onFrame = before
                done(results)
                return
            }
            frames = []
            recording = true
            if engine.goto(i) { renderLoop() }
            DispatchQueue.main.asyncAfter(deadline: .now() + hold) {
                recording = false
                results.append(Self.summary("\(names[i - 1]) → \(names[i])", frames))
                step(i + 1)
            }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + hold) { step(1) }
    }

    struct Frame {
        var cpu: Double
        var at: CFTimeInterval
        var stats: [String: Any]
    }

    static func summary(_ label: String, _ f: [Frame]) -> [String: Any] {
        let gaps = zip(f.dropFirst(), f).map { ($0.at - $1.at) * 1000 }
        let ms = (f.last?.at ?? 0) - (f.first?.at ?? 0)
        let sixty = 1000.0 / 60.0
        let r = { (v: Double) in (v * 10).rounded() / 10 }
        let spread = { (values: [Double]) -> [String: Double] in
            let v = values.filter(\.isFinite).sorted()
            let at = { (q: Double) in v.isEmpty ? 0 : v[min(v.count - 1, Int(q * Double(v.count)))] }
            return ["p50": r(at(0.5)), "p95": r(at(0.95)), "max": r(v.last ?? 0)]
        }
        let stat = { (key: String) in f.map { ($0.stats[key] as? NSNumber)?.doubleValue ?? .nan } }
        var out: [String: Any] = [
            "label": label,
            "frames": f.count,
            "fps": ms > 0 ? r(Double(f.count - 1) / ms) : 0,
            "dropped": gaps.reduce(0) { $0 + max(0, Int(($1 / sixty).rounded()) - 1) },
            "worst": r(gaps.max() ?? 0),
            // Which frame the worst gap came before (0: the first after the step).
            "worstAt": gaps.firstIndex(of: gaps.max() ?? 0).map { $0 + 1 } ?? 0,
            "cpu": spread(f.map(\.cpu)),
            "engine": spread(stat("engine")),
            "render": spread(stat("render")),
        ]
        for key in ["draws", "uploaded", "rebuilt", "tessellated"] where f.contains(where: { $0.stats[key] != nil }) {
            out[key] = spread(stat(key))
        }
        return out
    }

    private var scale: CGFloat {
        #if canImport(UIKit)
        return window?.screen.scale ?? UIScreen.main.scale
        #else
        return window?.backingScaleFactor ?? 2
        #endif
    }

    private func relayout() {
        engine.resize(width: Double(bounds.width), height: Double(bounds.height), scale: Double(scale))
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        metal.frame = bounds
        metal.contentsScale = scale
        CATransaction.commit()
        // Attach once there's a size (and again if a frame lost the device); the renderer sizes the
        // layer's drawables from then on.
        if bounds.width > 0, bounds.height > 0, !suspended, !gpu || engine.gpuBackend == nil {
            gpu = engine.attachMetal(layer: metal, width: Int((bounds.width * scale).rounded()), height: Int((bounds.height * scale).rounded()))
            metal.isHidden = !gpu
            if gpu { image = nil }
            Self.log.notice("frames: \(self.engine.gpuBackend ?? "CPU pixels (no GPU for the layer)", privacy: .public)")
        }
    }

    #if canImport(UIKit)
    open override func layoutSubviews() { super.layoutSubviews(); relayout(); renderLoop() }
    #else
    open override func layout() { super.layout(); relayout(); renderLoop() }
    #endif

    /// Show new data: on the next frame if frames are running (tiles arrive in bursts during a
    /// flight — rendering each on arrival would starve the display link), else now.
    private func requestFrame() {
        #if canImport(UIKit)
        if link != nil { return }
        #else
        if timer != nil { return }
        #endif
        renderLoop()
    }

    /// Render one frame, and keep going while the engine says something moves.
    public func renderLoop() {
        if suspended { return }
        let now = Date().timeIntervalSince(start)
        let t0 = CACurrentMediaTime()
        let moving = engine.frame(now: now)
        let cpuMs = (CACurrentMediaTime() - t0) * 1000
        if moving {
            let share = Work.note(cpuMs)
            if abs(share - workTold) > 0.04 { workTold = share; engine.setWorkScale(share) }
        }
        // A frame that lost the device fell back to CPU pixels: show those until a re-attach.
        if gpu, engine.gpuBackend == nil { gpu = false; metal.isHidden = true }
        fetchData()
        // Nothing moving: sleep until the engine's next scheduled frame, if any.
        wake?.invalidate()
        wake = nil
        if !moving, let at = engine.wakeAt {
            wake = Timer.scheduledTimer(withTimeInterval: max(0, at - now), repeats: false) { [weak self] _ in
                self?.renderLoop()
                self?.updateAccessibility()
            }
        }
        let px = gpu ? (width: 0, height: 0, data: Data()) : engine.pixels()
        if px.width > 0, let provider = CGDataProvider(data: px.data as CFData) {
            image = CGImage(width: px.width, height: px.height, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: px.width * 4,
                            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.last.rawValue),
                            provider: provider, decode: nil, shouldInterpolate: true, intent: .defaultIntent)
        }
        #if canImport(UIKit)
        onFrame?(cpuMs, moving, link?.timestamp ?? t0)
        if !gpu { setNeedsDisplay() }
        if moving && link == nil {
            link = CADisplayLink(target: self, selector: #selector(tick))
            link?.add(to: .main, forMode: .common)
        } else if !moving { link?.invalidate(); link = nil }
        #else
        onFrame?(cpuMs, moving, t0)
        if !gpu { needsDisplay = true }
        if moving && timer == nil {
            timer = Timer.scheduledTimer(withTimeInterval: 1.0 / 60.0, repeats: true) { [weak self] _ in self?.renderLoop() }
        } else if !moving { timer?.invalidate(); timer = nil }
        #endif
    }

    #if canImport(UIKit)
    @objc private func tick() { renderLoop() }

    open override func draw(_ rect: CGRect) {
        guard let image, let ctx = UIGraphicsGetCurrentContext() else { return }
        ctx.saveGState()
        ctx.translateBy(x: 0, y: bounds.height)
        ctx.scaleBy(x: 1, y: -1)
        ctx.draw(image, in: bounds)
        ctx.restoreGState()
    }

    open override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        touchStart = touches.first?.location(in: self)
    }

    open override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        touchStart = nil
    }

    open override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let p = touches.first?.location(in: self) else { return }
        // Released where it went down: a tap, which inspects what it lands on (with a finger's
        // reach) as well as clicking it.
        let tap = touchStart.map { hypot(p.x - $0.x, p.y - $0.y) < 10 } ?? true
        touchStart = nil
        let label = engine.pointer(tap ? .tap : .up, x: p.x, y: p.y)
        if tap {
            onTap?(label, p)
            showTip(label, at: p)
        }
        renderLoop()
        updateAccessibility()
    }

    /// A tap's label above the finger (it covers what's below it), within the view; nothing: hidden.
    private func showTip(_ label: String?, at p: CGPoint) {
        guard showsTooltips, let label, !label.isEmpty else {
            tip.isHidden = true
            return
        }
        tip.text = label
        var size = tip.intrinsicContentSize
        size.width = min(size.width, bounds.width - 8)
        let x = min(max(4, p.x - size.width / 2), bounds.width - size.width - 4)
        let above = p.y - 28 - size.height
        let y = above >= 0 ? above : min(p.y + 28, bounds.height - size.height)
        tip.frame = CGRect(x: x, y: y, width: size.width, height: size.height)
        tip.isHidden = false
        bringSubviewToFront(tip)
    }

    /// The engine's semantics tree as accessibility elements (VoiceOver reads the chart's content).
    public func updateAccessibility() {
        let items = engine.status()["semantics"] as? [[String: Any]] ?? []
        let status = engine.status()
        // A control that's clicked (a switch, a checkbox, a segment, a button) is a button like any
        // interactive mark; one that's dragged (a slider's or a range's thumb) is an adjustable
        // element below.
        var elements: [Any] = items.filter { ($0["role"] as? String) != "control" || ($0["actionable"] as? Bool) == true }.map { item in
            let e: UIAccessibilityElement
            if item["actionable"] as? Bool == true, let path = item["path"] as? String {
                e = DatarsActionable(view: self, path: path)
            } else {
                e = UIAccessibilityElement(accessibilityContainer: self)
            }
            e.accessibilityLabel = item["label"] as? String
            e.accessibilityFrameInContainerSpace = Self.frame(item["rect"]) ?? bounds
            if (item["role"] as? String) == "title" { e.accessibilityTraits = .header }
            return e
        }
        let controls = status["controls"] as? [[String: Any]] ?? []
        let selects = controls.filter { ($0["kind"] as? String) == "select" }
        updatePickers(selects)
        // A select is its picker button (a native menu), not the drawn box.
        let selectLabels = Set(selects.compactMap { $0["label"] as? String })
        elements.removeAll { ($0 as? UIAccessibilityElement)?.accessibilityLabel.map(selectLabels.contains) ?? false }
        elements.insert(contentsOf: pickers as [Any], at: 0)
        // Engine-drawn sliders as adjustable elements: swipe up/down changes the value.
        for c in controls where (c["kind"] as? String ?? "slider") == "slider" {
            elements.insert(DatarsAdjustable(view: self, control: c), at: 0)
        }
        accessibilityElements = elements
    }

    /// One menu button per select, over its box: the options with a check on the chosen one; a
    /// choice sets the signal. Rebuilt only when a select changes (an open menu isn't torn down).
    private func updatePickers(_ selects: [[String: Any]]) {
        let key = selects.map { "\($0["signal"] ?? "")|\($0["current"] ?? "")|\($0["rect"] ?? "")|\(($0["options"] as? [Any])?.count ?? 0)" }.joined(separator: ";")
        guard key != pickerKey else { return }
        pickerKey = key
        pickers.forEach { $0.removeFromSuperview() }
        pickers = selects.compactMap { c in
            guard let signal = c["signal"] as? String, let frame = Self.frame(c["rect"]),
                  let options = c["options"] as? [[String: Any]], !options.isEmpty else { return nil }
            let current = Self.json(c["current"])
            let actions = options.map { o -> UIAction in
                let value = Self.json(o["value"])
                return UIAction(title: o["label"] as? String ?? value, state: value == current ? .on : .off) { [weak self] _ in
                    guard let self else { return }
                    self.engine.setSignal(signal, json: value)
                    self.renderLoop()
                    self.updateAccessibility()
                }
            }
            let b = UIButton(type: .custom)
            b.frame = frame
            b.backgroundColor = .clear
            b.menu = UIMenu(title: c["label"] as? String ?? "", options: .singleSelection, children: actions)
            b.showsMenuAsPrimaryAction = true
            b.accessibilityLabel = c["label"] as? String
            addSubview(b)
            return b
        }
        bringSubviewToFront(tip)
    }

    /// A JSON value (from the status) written back as JSON text: `"SE"`, `3`, `true`, `null`.
    private static func json(_ v: Any?) -> String {
        guard let v, !(v is NSNull) else { return "null" }
        guard let d = try? JSONSerialization.data(withJSONObject: v, options: [.fragmentsAllowed]) else { return "null" }
        return String(decoding: d, as: UTF8.self)
    }
    #else
    open override var isFlipped: Bool { true }

    open override func draw(_ dirtyRect: NSRect) {
        guard let image, let ctx = NSGraphicsContext.current?.cgContext else { return }
        ctx.saveGState()
        ctx.translateBy(x: 0, y: bounds.height)
        ctx.scaleBy(x: 1, y: -1)
        ctx.draw(image, in: bounds)
        ctx.restoreGState()
    }

    open override func mouseUp(with event: NSEvent) {
        let p = convert(event.locationInWindow, from: nil)
        _ = engine.pointer(.up, x: p.x, y: p.y)
        showCursor()
        renderLoop()
    }

    // Hover: controls show where the pointer is, marks their tooltips' labels, and the pointer
    // takes the cursor the engine asks for.
    open override func updateTrackingAreas() {
        super.updateTrackingAreas()
        for a in trackingAreas { removeTrackingArea(a) }
        addTrackingArea(NSTrackingArea(rect: .zero, options: [.mouseMoved, .mouseEnteredAndExited, .cursorUpdate, .activeInKeyWindow, .inVisibleRect], owner: self))
    }

    open override func mouseMoved(with event: NSEvent) { move(event, .move) }
    open override func mouseDragged(with event: NSEvent) { move(event, .move) }
    open override func mouseDown(with event: NSEvent) { move(event, .down) }
    open override func mouseExited(with event: NSEvent) {
        _ = engine.pointer(.leave, x: 0, y: 0)
        toolTip = nil
        NSCursor.arrow.set()
        renderLoop()
    }
    open override func cursorUpdate(with event: NSEvent) { showCursor() }

    private func move(_ event: NSEvent, _ kind: DatarsEngine.PointerKind) {
        let p = convert(event.locationInWindow, from: nil)
        let label = engine.pointer(kind, x: p.x, y: p.y)
        if kind == .move, toolTip != label { toolTip = label }
        showCursor()
        renderLoop()
    }

    private func showCursor() {
        switch engine.cursor {
        case .pointer: NSCursor.pointingHand.set()
        case .grab: NSCursor.openHand.set()
        case .grabbing: NSCursor.closedHand.set()
        case .crosshair: NSCursor.crosshair.set()
        case .arrow: NSCursor.arrow.set()
        }
    }

    public func updateAccessibility() {
        let items = engine.status()["semantics"] as? [[String: Any]] ?? []
        setAccessibilityRole(.group)
        setAccessibilityLabel(items.first?["label"] as? String ?? "Chart")
        setAccessibilityChildren(items.dropFirst().map { item in
            let e = NSAccessibilityElement()
            e.setAccessibilityParent(self)
            e.setAccessibilityRole((item["role"] as? String) == "title" ? .staticText : .group)
            e.setAccessibilityLabel(item["label"] as? String)
            e.setAccessibilityFrameInParentSpace(Self.frame(item["rect"]) ?? bounds)
            return e
        })
    }
    #endif

    /// A semantics `rect` ([x, y, w, h] in points, top-left origin) as a frame.
    static func frame(_ v: Any?) -> CGRect? {
        guard let a = v as? [Double], a.count == 4, a.allSatisfy({ $0.isFinite }) else { return nil }
        return CGRect(x: a[0], y: a[1], width: a[2], height: a[3])
    }
}

#if canImport(UIKit)
/// An interactive mark (select, filter, drill): double-tap runs its click intent.
final class DatarsActionable: UIAccessibilityElement {
    weak var chart: DatarsChartView?
    let path: String
    init(view: DatarsChartView, path: String) {
        chart = view
        self.path = path
        super.init(accessibilityContainer: view)
        accessibilityTraits = .button
    }
    override func accessibilityActivate() -> Bool {
        guard let chart, chart.engine.activate(path: path) else { return false }
        chart.renderLoop()
        chart.updateAccessibility()
        return true
    }
}

/// An engine-drawn slider as VoiceOver sees it.
final class DatarsAdjustable: UIAccessibilityElement {
    weak var chart: DatarsChartView?
    let signal: String
    let lo, hi, step: Double
    var value: Double

    init(view: DatarsChartView, control c: [String: Any]) {
        chart = view
        signal = c["signal"] as? String ?? ""
        lo = c["min"] as? Double ?? 0
        hi = c["max"] as? Double ?? 1
        let s = c["step"] as? Double ?? 0
        step = s > 0 ? s : (hi - lo) / 20
        value = c["value"] as? Double ?? lo
        super.init(accessibilityContainer: view)
        accessibilityLabel = c["label"] as? String
        accessibilityTraits = .adjustable
        accessibilityFrameInContainerSpace = view.bounds
        accessibilityValue = String(format: "%g", value)
    }

    private func set(_ v: Double) {
        value = min(hi, max(lo, v))
        accessibilityValue = String(format: "%g", value)
        chart?.engine.setSignal(signal, value)
        chart?.renderLoop()
    }
    override func accessibilityIncrement() { set(value + step) }
    override func accessibilityDecrement() { set(value - step) }
}
#endif

#if canImport(SwiftUI) && canImport(UIKit)
/// SwiftUI: `DatarsChart(source: .url(URL(string: "https://charts.example.com/c/votes")!))`.
/// Pass `state` to drive the program declaratively: when it changes, the chart transitions there.
/// A chart shipped in the app: `.url(Bundle.main.url(forResource: "spending", withExtension: "datars")!)`;
/// the user's rows for its data slots: `data: ["spending": rowsJSON]`.
public struct DatarsChart: UIViewRepresentable {
    let source: DatarsSource
    let state: Int?
    let data: [String: Data]
    let configure: ((DatarsChartView) -> Void)?
    /// `configure`: set the underlying view up before it loads (`onFrame`, `onLoad`, a benchmark).
    public init(source: DatarsSource, state: Int? = nil, data: [String: Data] = [:], configure: ((DatarsChartView) -> Void)? = nil) { self.source = source; self.state = state; self.data = data; self.configure = configure }
    public func makeUIView(context: Context) -> DatarsChartView { let v = DatarsChartView(); configure?(v); v.data = data; v.load(source); return v }
    public func updateUIView(_ uiView: DatarsChartView, context: Context) {
        if uiView.targetState != state { uiView.targetState = state }
        if uiView.data != data { uiView.data = data }
    }
}
#elseif canImport(SwiftUI) && canImport(AppKit)
public struct DatarsChart: NSViewRepresentable {
    let source: DatarsSource
    let state: Int?
    let data: [String: Data]
    public init(source: DatarsSource, state: Int? = nil, data: [String: Data] = [:]) { self.source = source; self.state = state; self.data = data }
    public func makeNSView(context: Context) -> DatarsChartView { let v = DatarsChartView(); v.data = data; v.load(source); return v }
    public func updateNSView(_ nsView: DatarsChartView, context: Context) {
        if nsView.targetState != state { nsView.targetState = state }
        if nsView.data != data { nsView.data = data }
    }
}
#endif

/// The app's share of the engines' per-frame work budgets (points built, tiles decoded, map features
/// styled, meshes tessellated and uploaded — `DatarsEngine.setWorkScale`). They're sized for a
/// desktop, and a phone's CPU runs the same work several times slower, so views start at half.
/// Then: a moving frame that nearly missed its refresh cuts the share at once; the average of the
/// moving frames lowers it while they run past ~8 ms and raises it again when there's room. Detail
/// then arrives over more frames instead of frames dropping. One for the app: it's the device that
/// is slow, not a chart. (The web runtime's rule, `noteFrame`.)
@MainActor
enum Work {
    static var share = 0.5
    private static var frameMs = 0.0

    static func note(_ ms: Double) -> Double {
        frameMs = frameMs == 0 ? ms : frameMs * 0.85 + ms * 0.15
        if ms > 14 { share = max(0.2, share * 0.7) }
        else if frameMs > 8 { share = max(0.2, share * 0.9) }
        else if frameMs < 4 && ms < 8 { share = min(1, share * 1.05) }
        return share
    }
}

#if canImport(UIKit)
/// A tap's tooltip: the label in a small dark bubble, as the web runtime draws it.
final class DatarsTooltip: UILabel {
    private let pad = UIEdgeInsets(top: 4, left: 8, bottom: 4, right: 8)

    override init(frame: CGRect) {
        super.init(frame: frame)
        font = .systemFont(ofSize: 12, weight: .medium)
        textColor = .white
        backgroundColor = UIColor(white: 0.08, alpha: 0.92)
        layer.cornerRadius = 4
        clipsToBounds = true
        numberOfLines = 0
        isUserInteractionEnabled = false
        isHidden = true
    }

    required init?(coder: NSCoder) { fatalError("not from a nib") }

    override func drawText(in rect: CGRect) { super.drawText(in: rect.inset(by: pad)) }

    override var intrinsicContentSize: CGSize {
        let s = super.intrinsicContentSize
        return CGSize(width: s.width + pad.left + pad.right, height: s.height + pad.top + pad.bottom)
    }
}
#endif
