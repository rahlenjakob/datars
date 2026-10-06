// swift-tools-version:5.9
// DatarsKit — the datars runtime for iOS, iPadOS and macOS apps. Install once; charts arrive as
// bundles by URL and update without an App Store release (docs/12-delivery.md).
//
// The Rust library comes from `scripts/build-apple.sh` (DatarsFFI.xcframework next to this file:
// macOS, iOS device, iOS simulator). Without it — local `swift test` on macOS — CDatars links the
// static library from `cargo build -p datars-ffi --release`.
import Foundation
import PackageDescription

let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
let xcframework = FileManager.default.fileExists(atPath: root.appendingPathComponent("DatarsFFI.xcframework").path)

let ffi: Target = xcframework
    ? .binaryTarget(name: "CDatars", path: "DatarsFFI.xcframework")
    : .target(
        name: "CDatars",
        path: "Sources/CDatars",
        linkerSettings: [.unsafeFlags(["-L", "../../target/release"])]
    )

let package = Package(
    name: "DatarsKit",
    platforms: [.iOS(.v15), .macOS(.v12)],
    products: [.library(name: "DatarsKit", targets: ["DatarsKit"])],
    targets: [
        ffi,
        // Metal and QuartzCore: the GPU renderer (wgpu) draws on the view's CAMetalLayer.
        .target(name: "DatarsKit", dependencies: ["CDatars"], path: "Sources/DatarsKit", linkerSettings: [.linkedLibrary("c++"), .linkedFramework("Metal"), .linkedFramework("QuartzCore"), .linkedFramework("CoreGraphics")]),
        .testTarget(name: "DatarsKitTests", dependencies: ["DatarsKit"], path: "Tests/DatarsKitTests"),
    ]
)
