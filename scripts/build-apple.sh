#!/usr/bin/env bash
# Build DatarsFFI.xcframework (macOS arm64/x86_64, iOS device, iOS simulator) for apple/DatarsKit.
#   rustup target add aarch64-apple-darwin x86_64-apple-darwin aarch64-apple-ios aarch64-apple-ios-sim
set -euo pipefail
cd "$(dirname "$0")/.."
TARGETS="aarch64-apple-darwin aarch64-apple-ios aarch64-apple-ios-sim"
# The sandbox (QuickJS, for T3 bundles) is in by default; the app's policy (`allowScript`) decides
# at runtime. Inter is compiled in for raw documents (bundles carry their own fonts). `gpu` draws
# frames with Metal on the view's layer (wgpu); without it frames are CPU pixels. FEATURES=""
# builds with none of them.
FEATURES="${FEATURES-sandbox,bundled-fonts,gpu}"
export IPHONEOS_DEPLOYMENT_TARGET=15.0 MACOSX_DEPLOYMENT_TARGET=12.0
# QuickJS bindings are generated for iOS: bindgen needs libclang and the SDKs.
export LIBCLANG_PATH="${LIBCLANG_PATH:-$(xcode-select -p)/Toolchains/XcodeDefault.xctoolchain/usr/lib}"
export BINDGEN_EXTRA_CLANG_ARGS_aarch64_apple_ios="-isysroot $(xcrun --sdk iphoneos --show-sdk-path)"
export BINDGEN_EXTRA_CLANG_ARGS_aarch64_apple_ios_sim="-isysroot $(xcrun --sdk iphonesimulator --show-sdk-path) --target=arm64-apple-ios15.0-simulator"
for t in $TARGETS; do
  cargo build -p datars-ffi --release --target "$t" --no-default-features --features "$FEATURES"
done
OUT=apple/DatarsKit/DatarsFFI.xcframework
rm -rf "$OUT"
HEADERS=apple/DatarsKit/Sources/CDatars/include
xcodebuild -create-xcframework \
  -library target/aarch64-apple-darwin/release/libdatars_ffi.a -headers "$HEADERS" \
  -library target/aarch64-apple-ios/release/libdatars_ffi.a -headers "$HEADERS" \
  -library target/aarch64-apple-ios-sim/release/libdatars_ffi.a -headers "$HEADERS" \
  -output "$OUT"
echo "built $OUT"
