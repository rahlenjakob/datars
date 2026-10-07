#!/usr/bin/env bash
# The platforms page's iOS captures: the SwiftUI sample app (apple/DatarsSample.swiftpm, built as
# scripts/test-ios-sample.sh builds it) in the iOS simulator, playing `platforms-everywhere` as the
# site build published it — each state — cropped to the app's chart view (the screen under the
# status bar) and written to site/img/platforms-ios-<state>.png. Light only: the sample app doesn't
# hand the system's appearance to the chart (DatarsKit's `setMode` is the host's to call).
#
#   node scripts/build-site.mjs --out out/pages && datars serve out/pages --port 8960 &
#   site/figures/platforms/capture-ios.sh http://127.0.0.1:8960
#
# Fails unless DatarsKit says its frames were drawn with Metal. The page states the device, the
# iOS version and the date these were taken (site/pages/features/platforms.html).
set -euo pipefail
cd "$(dirname "$0")/../../.."
SITE="${1:-http://127.0.0.1:8960}"
SIM="${SIM:-$(xcrun simctl list devices available | grep -m1 -oE 'iPhone 16 Pro \(([0-9A-F-]{36})\)' | grep -oE '[0-9A-F-]{36}')}"
DERIVED="${DERIVED:-out/ios-sample}"
xcrun simctl boot "$SIM" 2>/dev/null || true
xcrun simctl bootstatus "$SIM" >/dev/null
APP=$(find "$DERIVED" -name DatarsSample.app -type d | head -1)
[ -n "$APP" ] || (cd apple/DatarsSample.swiftpm && xcodebuild -scheme DatarsSample -destination "platform=iOS Simulator,id=$SIM" -derivedDataPath "../../$DERIVED" build -quiet)
APP=$(find "$DERIVED" -name DatarsSample.app -type d | head -1)
xcrun simctl install "$SIM" "$APP"
# A clean status bar (it's cropped away, but the screen is then the same every time).
xcrun simctl status_bar "$SIM" override --time 9:41 --batteryState charged --batteryLevel 100 >/dev/null 2>&1 || true
TMP=$(mktemp -d)
xcrun simctl ui "$SIM" appearance light
for state in 0 1 2 3; do
  xcrun simctl terminate "$SIM" dev.datars.sample 2>/dev/null || true
  SIMCTL_CHILD_DATARS_URL="$SITE/c/platforms-everywhere" SIMCTL_CHILD_DATARS_STATE=$state xcrun simctl launch "$SIM" dev.datars.sample >/dev/null
  sleep 10
  xcrun simctl io "$SIM" screenshot "$TMP/$state.png" >/dev/null 2>&1
  # The chart view: the full width, under the 62 pt status bar, 812 pt tall (at 3×).
  python3 - "$TMP/$state.png" "site/img/platforms-ios-$state.png" <<'PY'
import sys
from PIL import Image
im = Image.open(sys.argv[1]).convert("RGB")
im.crop((0, 62 * 3, 402 * 3, (62 + 812) * 3)).save(sys.argv[2], optimize=True)
PY
done
FRAMES=$(xcrun simctl spawn "$SIM" log show --last 5m --style compact --predicate 'subsystem == "dev.datars"' 2>/dev/null | grep -o 'frames: .*' | tail -1)
echo "${FRAMES:-frames: (no log from DatarsKit)}"
case "$FRAMES" in *Metal*) ;; *) echo "expected frames on Metal" >&2; exit 1 ;; esac
xcrun simctl list devices | grep "$SIM"
