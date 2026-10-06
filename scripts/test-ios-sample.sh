#!/usr/bin/env bash
# The SwiftUI sample app (apple/DatarsSample.swiftpm) in the iOS simulator: built once, it plays a
# chart published afterwards from a static site over HTTP — here the descent, tiles streamed by
# HTTP Range — and a declarative `state` drives its program. Leaves out/ios-sample-*.png.
set -euo pipefail
cd "$(dirname "$0")/.."
SIM="${SIM:-$(xcrun simctl list devices available | grep -m1 -oE 'iPhone[^(]*\(([0-9A-F-]{36})\)' | grep -oE '[0-9A-F-]{36}')}"
xcrun simctl boot "$SIM" 2>/dev/null || true
cargo build --release -p datars-cli >/dev/null
(cd apple/DatarsSample.swiftpm && xcodebuild -scheme DatarsSample -destination "platform=iOS Simulator,id=$SIM" -derivedDataPath ../../out/ios-sample build -quiet)
SITE=out/ios-site && rm -rf "$SITE"
target/release/datars publish examples/descent/doc.json --alias descent --to "$SITE" >/dev/null
target/release/datars serve "$SITE" --port 8792 >/dev/null 2>&1 & SERVER=$!
trap 'kill $SERVER' EXIT
APP=$(find out/ios-sample -name DatarsSample.app -type d | head -1)
xcrun simctl terminate "$SIM" dev.datars.sample 2>/dev/null || true
xcrun simctl install "$SIM" "$APP"
for state in 0 2; do
  xcrun simctl terminate "$SIM" dev.datars.sample 2>/dev/null || true
  SIMCTL_CHILD_DATARS_URL=http://127.0.0.1:8792/c/descent SIMCTL_CHILD_DATARS_STATE=$state xcrun simctl launch "$SIM" dev.datars.sample >/dev/null
  sleep 12
  xcrun simctl io "$SIM" screenshot "out/ios-sample-$state.png" >/dev/null
done
echo "out/ios-sample-0.png, out/ios-sample-2.png"
# Which renderer drew: DatarsKit logs it when the view attaches its Metal layer.
FRAMES=$(xcrun simctl spawn "$SIM" log show --last 2m --style compact --predicate 'subsystem == "dev.datars"' 2>/dev/null | grep -o 'frames: .*' | tail -1)
echo "${FRAMES:-frames: (no log from DatarsKit)}"
case "$FRAMES" in *Metal*) ;; *) echo "expected frames on Metal" >&2; exit 1 ;; esac
