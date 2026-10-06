---
title: Android
description: Show published datars charts natively in Android apps with the datars AAR — a Kotlin view for layouts and Jetpack Compose, private data slots, TalkBack, charts shipped in the app.
lede: Add the library once. Charts arrive by URL, play natively — the same engine and the same renderer as on the web and iOS — and update without a Play Store release.
---

## A chart in an Activity

```kotlin
import dev.datars.DatarsView

class ElectionActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val chart = DatarsView(this)
        setContentView(chart)
        chart.load("https://charts.example.com/c/election")
        chart.setOnClickListener { chart.send("next") }   // tap to step the story
    }
}
```

`load` takes the chart's manifest — the `c/<alias>` file [`datars publish`](/docs/publishing/) wrote — on any static host, or a `.datars` file by URL. Republish the chart and the app shows the new version on its next load; nothing about the chart is compiled into the app.

`DatarsView` is an ordinary `TextureView`. It runs the datars engine natively through JNI: text shaped with the chart's own fonts, layout, motion, and each frame drawn on the GPU with Vulkan, else OpenGL ES, through the same wgpu renderer as on the web and iOS. Where the view gets no GPU, the engine's CPU reference draws the frames into a bitmap instead: the renderer the test goldens are made with, pixel for pixel the same on every platform. Network requests run on a background thread; the engine only ever runs on the main thread.

## Add the library

The library is in the repository at `android/`. It targets **Android 7.0 (API 24) and newer** and ships native code for **arm64-v8a** (phones) and **x86_64** (emulators). It isn't on Maven Central yet, so you build it once:

```sh
# 1. The Rust engine for Android (needs the NDK: ANDROID_NDK_HOME, or ANDROID_HOME with an ndk/)
rustup target add aarch64-linux-android x86_64-linux-android
scripts/build-android.sh                   # → android/datars/src/main/jniLibs/<abi>/libdatars_ffi.so

# 2. The AAR
cd android && gradle :datars:assembleRelease
```

Then depend on it from your app — as a module (`implementation(project(":datars"))`, the way the sample app does) or as the built AAR file. The library has no dependencies of its own and declares the `INTERNET` permission.

The engine is built with the recipe sandbox (for T3 bundles) and the Inter font (for raw documents) by default; `FEATURES="" scripts/build-android.sh` leaves both out for a smaller library that plays pre-expanded bundles only.

## The view

| Method | What it does |
|---|---|
| `load(url)` | Load a chart by URL: a manifest (`…/c/<alias>`) or a `.datars` file. Chunks, data files and map tiles are fetched relative to it. |
| `loadAsset(path)` | Load a `.datars` file from the app's `assets/` — no network needed. |
| `loadDocument(json)` | Load a raw document (JSON), for development. |
| `provideData(name, json)` | Fill a data slot with the app's own rows (JSON or CSV, as a `String` or `ByteArray`). Returns `false` if the rows lack the columns the chart needs. |
| `send(event)` | Program events: `next`, `prev`, `back`, `goto:<state>`. |
| `goTo(index)` | Go to a program state (0-based); the transition plays. |
| `seek(pos)` | Program position as a number (state + fraction toward the next), for scroll-scrubbed stories. |
| `setPlaying(on)` | Pause or resume autoplay (for example when the view scrolls off screen). |
| `setDarkMode(dark)` | Dark or light theme mode. The view doesn't follow the system setting on its own. |
| `pixelHash()` | The last frame's pixel identity — the same value `datars render --hash` prints for the same state. |

The view renders on demand: at most one frame per vsync while something moves, then nothing until the engine's next scheduled moment (an autoplay hold, a live refresh). Touches reach the chart — taps run interactive marks' click intents, drags brush and pan explorable views — and then your `OnClickListener`.

## Jetpack Compose

There's no datars composable; wrap the view with Compose's standard `AndroidView` interop:

```kotlin
@Composable
fun ElectionChart(url: String, step: Int, modifier: Modifier = Modifier) {
    val dark = isSystemInDarkTheme()
    AndroidView(
        factory = { context -> DatarsView(context).apply { load(url) } },
        update = { chart ->
            chart.setDarkMode(dark)
            chart.goTo(step)      // takes effect once the chart has loaded
        },
        modifier = modifier.fillMaxWidth().height(420.dp),
    )
}
```

## Each user's data, on the device

A document can leave a source to the app: a [data slot](/docs/publishing/#data-slots-each-readers-own-data). The chart is built and published once; each user's rows are handed in on the device and go nowhere else.

```kotlin
chart.loadAsset("spending.datars")            // shipped in the app: works offline from the first launch
chart.provideData("spending", rowsJson)       // this user's rows
```

Rows can be provided before the chart has loaded — they wait for it — and new rows transition the chart like any other change. Until rows arrive, the chart shows the sample data the document declares.

## TalkBack

The chart reaches TalkBack as its content, not as a picture. The view exposes the engine's semantics as virtual accessibility nodes:

- every mark with semantics is a node with its label and its bounds on screen, so explore-by-touch finds the bar under your finger;
- interactive marks are buttons — a double-tap runs their click intent;
- engine-drawn sliders are seek bars with their range — swipe to step the value, or set it directly;
- engine-drawn switches, checkboxes, segments and buttons are buttons; a `select` is a spinner whose double-tap opens Android's own list dialog — the same dialog a tap on it opens for everyone;
- the view's content description is the chart's title.

"Remove animations" (Settings › Accessibility) is honoured as reduced motion: transitions become short crossfades. The setting is read when the view is created.

## Try the sample app

`android/sample` is a one-activity app: it loads the chart at the `url` intent extra (default: a local `datars serve` reached through `adb reverse`) and steps through its states on tap.

```sh
datars publish examples/votes/doc.json --alias votes --to out/site
datars serve out/site --port 8791 &
adb reverse tcp:8791 tcp:8791
cd android && gradle :sample:installDebug
adb shell am start -n dev.datars.sample/.MainActivity --es url http://127.0.0.1:8791/c/votes
```

`scripts/test-android-sample.sh` does the whole round trip on an emulator: it installs the app, publishes a chart *afterwards*, plays it, republishes it and checks the app shows the new version on relaunch.

> **Status** The library builds into an AAR, its CPU reference's pixel hashes match every other target, and on an Android emulator the sample app draws with OpenGL ES (the emulator's Vulkan can't run on a macOS host) and plays charts published after it was installed — including the world-to-Stockholm map flight with tiles streamed by range. It hasn't run on a physical device yet, and TalkBack's node tree has been checked but not yet driven by TalkBack itself. See the project's [status page]({{src}}/docs/19-status.md).

## Next

- [Publishing and hosting](/docs/publishing/) — where chart URLs come from
- [iOS and macOS](/docs/embed/ios/) — the same charts in SwiftUI
- [Accessibility](/docs/accessibility/) — what the semantics contain
