// swift-tools-version: 5.9
// A SwiftUI app that plays a datars chart by URL — built once, it shows charts published after it
// (and their republishes) with no App Store release. Build and run in the simulator with
// scripts/test-ios-sample.sh, or open in Xcode / Swift Playgrounds.
import AppleProductTypes
import PackageDescription

let package = Package(
    name: "DatarsSample",
    platforms: [.iOS("16.0")],
    products: [
        .iOSApplication(
            name: "DatarsSample",
            targets: ["App"],
            bundleIdentifier: "dev.datars.sample",
            displayVersion: "0.1",
            bundleVersion: "1",
            supportedDeviceFamilies: [.phone, .pad],
            supportedInterfaceOrientations: [.portrait, .landscapeLeft, .landscapeRight]
        ),
    ],
    dependencies: [.package(path: "../DatarsKit")],
    targets: [.executableTarget(name: "App", dependencies: [.product(name: "DatarsKit", package: "DatarsKit")], path: "App")]
)
