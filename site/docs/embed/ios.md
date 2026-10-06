---
title: iOS and macOS
description: Show published datars charts natively in iOS, iPadOS and macOS apps with DatarsKit — SwiftUI and UIKit views, story state from Swift, private data slots, VoiceOver.
lede: Add the DatarsKit Swift package once. Charts then arrive by URL, play natively — the same engine and the same renderer as on the web — and update without an App Store release.
---

## A chart in SwiftUI

```swift
import DatarsKit
import SwiftUI

struct ElectionView: View {
    @State private var step = 0

    var body: some View {
        DatarsChart(source: .url(URL(string: "https://charts.example.com/c/election")!),
                    state: step)
            .frame(height: 420)
            .onTapGesture { step += 1 }
    }
}
```

`source` is the chart's manifest — the `c/<alias>` file [`datars publish`](/docs/publishing/) wrote — on any static host. When `state` changes, the chart transitions to that program state (0-based), from wherever it is. Republish the chart and the app shows the new version on its next load; nothing about the chart is compiled into the app.

This is not a web view. DatarsKit runs the datars engine natively: it shapes the text with the chart's own fonts, lays out, animates and draws each frame on the GPU with Metal, through the same wgpu renderer the web runtime uses. Where the view gets no GPU, the engine's CPU reference draws the frames instead: the renderer the test goldens are made with, pixel for pixel the same on every platform. The GPU's frames are checked against it perceptually (`datars gpu`).

## Add DatarsKit

DatarsKit lives in the repository at `apple/DatarsKit`. It supports **iOS 15+ and macOS 12+** (Swift tools 5.9, Xcode 15 or newer). It isn't published to a package registry yet, so you build its native library once and add it as a local package:

```sh
# The Rust engine as an XCFramework: macOS, iOS device, iOS simulator
rustup target add aarch64-apple-darwin aarch64-apple-ios aarch64-apple-ios-sim
scripts/build-apple.sh                  # → apple/DatarsKit/DatarsFFI.xcframework
```

Then, in Xcode, **File › Add Package Dependencies… › Add Local…** and choose `apple/DatarsKit`, or in a `Package.swift`:

```swift
dependencies: [.package(path: "../datars/apple/DatarsKit")],
targets: [.target(name: "MyApp", dependencies: ["DatarsKit"])],
```

The engine is built with the recipe sandbox (for T3 bundles) and the Inter font (for raw documents) by default; `FEATURES="" scripts/build-apple.sh` leaves both out for a smaller binary that plays pre-expanded bundles only.

## UIKit and AppKit

`DatarsChartView` is a `UIView` on iOS and an `NSView` on macOS:

```swift
let chart = DatarsChartView()
chart.frame = view.bounds
chart.autoresizingMask = [.flexibleWidth, .flexibleHeight]
view.addSubview(chart)

chart.load(.url(URL(string: "https://charts.example.com/c/election")!))
chart.targetState = 2            // go to a state now, or as soon as the chart has loaded
```

| Member | What it does |
|---|---|
| `load(_ source: DatarsSource)` | Load a chart: `.url(url)` for a manifest or a `.datars` file (remote or local), `.document(json)` for a raw document during development. |
| `targetState: Int?` | The program state to show; the transition plays from where the chart is. |
| `data: [String: Data]` | The app's own rows for the chart's data slots (below). |
| `engine: DatarsEngine` | The runtime underneath, for everything else. |
| `renderLoop()` | Draw now, and keep drawing while something moves. Call it after changing the engine directly. |
| `updateAccessibility()` | Rebuild the VoiceOver elements (the view does this itself after loads, taps and data). |

On macOS the view follows the mouse: marks show their labels as tooltips, controls show where the pointer is (`hover()`), and the cursor is the one the engine asks for (`engine.cursor`) — a pointing hand over what a click acts on, an open hand over a view that pans, a crosshair over a brushable area.

The view renders on demand: a display link runs only while something moves, and a one-shot timer wakes it for autoplay holds and live refreshes. It follows **Reduce Motion** by itself: transitions become short crossfades.

## Mode, brand and state from Swift

`DatarsEngine` is the chart's runtime. Its main calls:

```swift
let engine = chart.engine
engine.setMode(.dark)                                   // .light, .dark, .highContrast
engine.setTokens(["accent": "#0f766e", "radius.bar": 7]) // brand overrides; locked tokens are kept
engine.send("next")                                     // program events: next, prev, back, goto:<state>
engine.goto(3)
engine.setSignal("income", 42000)                       // a numeric signal the document declares
chart.renderLoop()
```

| Call | What it does |
|---|---|
| `setMode(_:)` | Light, dark or high contrast. The view doesn't follow the system appearance on its own — pass it the trait collection's or SwiftUI's colour scheme. |
| `setTokens(_:)` | Theme token overrides: your app's brand on a downloaded chart. Each call replaces the previous overrides. |
| `send(_:)`, `goto(_:)` | Move the program. |
| `seek(_:)` | Program position as a number (state + fraction toward the next) — scroll-scrubbed stories inside a `ScrollView`. |
| `setSignal(_:_:)` | Set a numeric signal: app state driving the chart. |
| `setPlaying(_:)` | Pause or resume autoplay (for example while the view is off screen). |
| `setReducedMotion(_:)` | Override the Reduce Motion setting. |
| `status()` | The current state, states, narration, accessibility tree and resolved tokens. |
| `pointer(_:x:y:)`, `zoom(atX:y:delta:)` | Pointer and pinch input, for apps that wire their own gesture recognizers. |
| `pixelHash()` | The last frame's pixel identity — the same value `datars render --hash` prints for the same state. |

In SwiftUI, a small representable does it; SwiftUI calls `updateUIView` again when the colour scheme changes:

```swift
struct BrandedChart: UIViewRepresentable {
    let url: URL
    @Environment(\.colorScheme) private var colorScheme

    func makeUIView(context: Context) -> DatarsChartView {
        let chart = DatarsChartView()
        chart.load(.url(url))
        return chart
    }

    func updateUIView(_ chart: DatarsChartView, context: Context) {
        chart.engine.setMode(colorScheme == .dark ? .dark : .light)
        chart.engine.setTokens(["accent": "#0f766e"])
        chart.renderLoop()
    }
}
```

Taps reach the chart: a tap on an interactive mark runs its click intent (select, filter, drill into a chapter). To step a story on tap instead, drive `state` from a gesture, as in the first example and the sample app.

## Each user's data, on the device

A document can leave a source to the app: a [data slot](/docs/publishing/#data-slots-each-readers-own-data). The chart is built and published once; each user's rows are handed in on the device and go nowhere else — a banking app's spending chart, a fitness app's weekly summary.

```swift
DatarsChart(source: .url(chartURL), data: ["spending": rowsJSON])
```

Rows are CSV or JSON `Data`. They can be set before the chart has loaded (they wait for it), and new rows transition the chart like any other change. Rows missing the columns the chart needs are refused, and the chart keeps what it showed. Until rows arrive, the chart shows the sample data the document declares.

## Charts inside the app

Ship a `.datars` file (from [`datars bundle`](/docs/publishing/#one-file-instead-of-a-folder)) in the app's resources for charts that must work offline from the first launch:

```swift
DatarsChart(source: .url(Bundle.main.url(forResource: "spending", withExtension: "datars")!),
            data: ["spending": rowsJSON])
```

## VoiceOver

The chart reaches VoiceOver as its content, not as a picture:

- every mark with semantics is an accessibility element with its label and its frame on screen, so explore-by-touch finds the bar under your finger; titles are headers;
- interactive marks are buttons — a double-tap runs their click intent;
- engine-drawn sliders are adjustable elements — swipe up or down to change the value;
- engine-drawn switches, checkboxes, segments and buttons are buttons; a `select` is the system's menu button — a tap on it opens the iOS menu of its options (with a check on the chosen one) instead of the chart's drawn list;
- on macOS, the chart is a group whose children are the marks.

See [accessibility](/docs/accessibility/) for what the semantics contain and how to check them with `datars semantics`.

## Try the sample app

`apple/DatarsSample.swiftpm` is a one-screen SwiftUI app: it loads the chart at the `DATARS_URL` launch environment variable (default: a local `datars serve`) and steps through its states on tap.

```sh
datars publish examples/descent/doc.json --alias descent --to out/site
datars serve out/site --port 8791
open apple/DatarsSample.swiftpm       # run it in a simulator; DATARS_URL=http://127.0.0.1:8791/c/descent
```

`scripts/test-ios-sample.sh` does the whole round trip in the iOS simulator: it builds the app, publishes the world-to-Stockholm map flight *afterwards*, serves it, and screenshots the app playing it with map tiles streamed by HTTP range requests.

> **Status** DatarsKit is verified on macOS and in the iOS simulator: frames draw with Metal, the CPU reference's pixel hashes match every other target, charts published after the app was built play and update, and map tiles stream by range. It hasn't run on a physical iPhone or iPad, or shipped in an App Store app, yet. See the project's [status page]({{src}}/docs/19-status.md).

## Next

- [Publishing and hosting](/docs/publishing/) — where chart URLs come from
- [Android](/docs/embed/android/) — the same charts in a Kotlin view
- [Themes and brands](/docs/theming/) — the tokens `setTokens` accepts
