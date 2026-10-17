#!/usr/bin/env bash
# Xcode's accessibility audit (XCUITest's performAccessibilityAudit: contrast, hit regions, element
# detection, descriptions, Dynamic Type, clipped text, traits) over the iOS sample app in the
# simulator, on the budget, controls, votes and prices examples — each published to a local site the
# app loads by URL. Prints each chart's element tree and every issue; fails on any issue. Needs Xcode
# and the xcframework (scripts/build-apple.sh).
#   scripts/test-ios-audit.sh [chart …]
set -euo pipefail
cd "$(dirname "$0")/.."
PORT="${PORT:-8796}"
SIM="${SIM:-$(xcrun simctl list devices available | grep -m1 -oE 'iPhone[^(]*\(([0-9A-F-]{36})\)' | grep -oE '[0-9A-F-]{36}')}"
CHARTS=("$@"); [ ${#CHARTS[@]} -gt 0 ] || CHARTS=(budget controls votes prices)
xcrun simctl boot "$SIM" 2>/dev/null || true
cargo build --release -p datars-cli >/dev/null
SITE=out/ios-audit-site && rm -rf "$SITE"
for c in "${CHARTS[@]}"; do
  if [ "$c" = controls ]; then node scripts/doc-to-json.mjs site/charts/controls.ts > out/ios-audit-controls.json; src=out/ios-audit-controls.json; else src=examples/$c/doc.json; fi
  target/release/datars publish "$src" --alias "$c" --to "$SITE" >/dev/null
done
target/release/datars serve "$SITE" --port "$PORT" >/dev/null 2>&1 & SERVER=$!
trap 'kill $SERVER 2>/dev/null; xcrun simctl terminate "$SIM" dev.datars.sample 2>/dev/null || true' EXIT
(cd apple/DatarsSample.swiftpm && xcodebuild -scheme DatarsSample -destination "platform=iOS Simulator,id=$SIM" -derivedDataPath ../../out/ios-sample build -quiet)
xcrun simctl install "$SIM" "$(find out/ios-sample -name DatarsSample.app -type d | head -1)"
OUT=out/ios-audit && mkdir -p "$OUT"
(cd apple/DatarsAudit && xcodebuild build-for-testing -project Audit.xcodeproj -scheme AuditUITests -destination "platform=iOS Simulator,id=$SIM" -derivedDataPath ../../out/ios-audit/dd -quiet)
fail=0
for c in "${CHARTS[@]}"; do
  log="$OUT/$c.log"
  if TEST_RUNNER_DATARS_URL="http://127.0.0.1:$PORT/c/$c" xcodebuild test-without-building -project apple/DatarsAudit/Audit.xcodeproj -scheme AuditUITests \
      -destination "platform=iOS Simulator,id=$SIM" -derivedDataPath out/ios-audit/dd > "$log" 2>&1; then
    echo "$c: $(grep -m1 -o 'AUDIT-COUNT .*' "$log" || echo '?') issues"
  else
    fail=1; echo "$c: FAILED"; grep -E 'AUDIT-ISSUE|error:' "$log" | head -20
  fi
done
echo "trees and issues: $OUT/<chart>.log"
exit $fail
