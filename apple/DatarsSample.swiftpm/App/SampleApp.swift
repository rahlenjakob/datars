import DatarsKit
import Foundation
import os
import SwiftUI

/// One chart, loaded by URL (the `DATARS_URL` launch environment, else a local `datars serve`).
/// Tap it to step through its states. `DATARS_BENCH=1` steps through every state by itself instead,
/// timing each transition, and logs one `DATARS-BENCH` line per transition (scripts/bench-native.sh).
@main
struct SampleApp: App {
    let url = URL(string: ProcessInfo.processInfo.environment["DATARS_URL"] ?? "http://127.0.0.1:8791/c/votes")!
    let bench = ProcessInfo.processInfo.environment["DATARS_BENCH"] == "1"

    @State private var state = Int(ProcessInfo.processInfo.environment["DATARS_STATE"] ?? "") ?? 0

    var body: some Scene {
        WindowGroup {
            // The program follows SwiftUI state: a tap steps to the next state.
            DatarsChart(source: .url(url), state: bench ? nil : state, configure: bench ? benchmark : nil)
                .ignoresSafeArea(edges: .bottom)
                .onTapGesture { if !bench { state += 1 } }
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
