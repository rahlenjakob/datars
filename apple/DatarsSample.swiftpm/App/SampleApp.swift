import DatarsKit
import Foundation
import os
import SwiftUI

/// One chart, loaded by URL (the `DATARS_URL` launch environment, else a local `datars serve`).
/// Tap it to step through its states. `DATARS_BENCH=1` steps through every state by itself instead,
/// timing each transition, and logs one `DATARS-BENCH` line per transition (scripts/bench-native.sh).
/// `DATARS_AX=1` prints what VoiceOver walks through (the view's accessibility elements: label,
/// value, traits) a few seconds after the chart loads, as `DATARS-AX` lines on stdout.
@main
struct SampleApp: App {
    let url = URL(string: ProcessInfo.processInfo.environment["DATARS_URL"] ?? "http://127.0.0.1:8791/c/votes")!
    let bench = ProcessInfo.processInfo.environment["DATARS_BENCH"] == "1"
    let ax = ProcessInfo.processInfo.environment["DATARS_AX"] == "1"

    @State private var state = Int(ProcessInfo.processInfo.environment["DATARS_STATE"] ?? "") ?? 0

    var body: some Scene {
        WindowGroup {
            // The program follows SwiftUI state: a tap steps to the next state.
            DatarsChart(source: .url(url), state: bench ? nil : state, configure: bench ? benchmark : ax ? dumpAccessibility : nil)
                .ignoresSafeArea(edges: .bottom)
                .onTapGesture { if !bench { state += 1 } }
        }
    }

    /// What VoiceOver gets, printed: every accessibility element of the chart view, in order.
    func dumpAccessibility(_ view: DatarsChartView) {
        view.onAnnouncement = { print("DATARS-SAID \($0)"); fflush(stdout) }
        view.onLoad = { [weak view] in
            DispatchQueue.main.asyncAfter(deadline: .now() + 4) {
                guard let view else { return }
                view.updateAccessibility()
                for (i, el) in (view.accessibilityElements ?? []).enumerated() {
                    let o = el as? NSObject
                    let traits = o?.accessibilityTraits ?? []
                    var t: [String] = []
                    if traits.contains(.header) { t.append("header") }
                    if traits.contains(.button) { t.append("button") }
                    if traits.contains(.adjustable) { t.append("adjustable") }
                    if traits.contains(.summaryElement) { t.append("summary") }
                    if traits.contains(.staticText) { t.append("text") }
                    let f = o?.accessibilityFrame ?? .zero
                    print("DATARS-AX \(i) [\(t.joined(separator: ","))] \"\(o?.accessibilityLabel ?? "")\" value=\"\(o?.accessibilityValue ?? "")\" frame=\(Int(f.minX)),\(Int(f.minY)),\(Int(f.width)),\(Int(f.height))")
                }
                print("DATARS-AX-DONE")
                // `DATARS_AX_ACT="206:activate,206:increment"`: what VoiceOver's gestures do to those
                // elements (activate, increment, decrement, escape), and what each then says.
                for step in (ProcessInfo.processInfo.environment["DATARS_AX_ACT"] ?? "").split(separator: ",") {
                    // The element by its index, or the first whose label has these words.
                    let parts = step.split(separator: ":")
                    let find = { () -> Int? in Int(parts[0]) ?? (view.accessibilityElements ?? []).firstIndex { (($0 as? NSObject)?.accessibilityLabel ?? "").contains(parts[0]) } }
                    guard parts.count == 2, let i = find(), let el = (view.accessibilityElements ?? [])[safe: i] as? NSObject else { continue }
                    switch parts[1] {
                    case "activate": _ = el.accessibilityActivate()
                    case "increment": el.accessibilityIncrement()
                    case "decrement": el.accessibilityDecrement()
                    case "escape": _ = el.accessibilityPerformEscape()
                    default: break
                    }
                    let now = find().flatMap { (view.accessibilityElements ?? [])[safe: $0] as? NSObject }
                    print("DATARS-ACT \(step) → \"\(now?.accessibilityLabel ?? "")\" value=\"\(now?.accessibilityValue ?? "")\"")
                }
                fflush(stdout)
                // Then the reader's settings as the chart follows them, every two seconds for a
                // minute (Increase Contrast, Bold Text, the text size, changed while it runs).
                var left = 30
                Timer.scheduledTimer(withTimeInterval: 2, repeats: true) { t in
                    left -= 1
                    if left <= 0 { t.invalidate() }
                    let theme = view.engine.status()["tokens"] as? [String: Any]
                    let tokens = theme?["tokens"] as? [String: Any]
                    print("DATARS-SETTINGS mode=\(theme?["mode"] ?? "?") ink=\(tokens?["ink"] ?? "?") boost=\(tokens?["text.weight-boost"] ?? 0) label=\(tokens?["size.label"] ?? "?")")
                    fflush(stdout)
                }
            }
        }
    }

    /// Run the chart's benchmark once it has loaded and log the results.
    func benchmark(_ view: DatarsChartView) {
        let log = Logger(subsystem: "dev.datars", category: "bench")
        let chart = url.lastPathComponent
        view.onLoad = { [weak view] in
            guard let view else { return }
            view.runBenchmark { results in
                for r in results {
                    var line = r
                    line["chart"] = chart
                    if let data = try? JSONSerialization.data(withJSONObject: line), let json = String(data: data, encoding: .utf8) {
                        log.notice("DATARS-BENCH \(json, privacy: .public)")
                    }
                }
                log.notice("DATARS-BENCH-DONE \(chart, privacy: .public) \(view.engine.gpuBackend ?? "CPU pixels", privacy: .public)")
            }
        }
    }
}

extension Array {
    subscript(safe i: Int) -> Element? { indices.contains(i) ? self[i] : nil }
}
