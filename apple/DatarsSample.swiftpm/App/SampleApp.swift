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
                fflush(stdout)
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
