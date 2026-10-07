#!/usr/bin/env bash
# The platforms page's Android captures, the counterpart of capture-ios.sh: the sample app
# (android/sample, built as scripts/test-android-sample.sh builds it) on an emulator, playing
# `platforms-everywhere` as the site build published it — each state, stepped by taps as a reader
# would — cropped to the app's DatarsView and written to site/img/platforms-android-<state>.png.
#
#   node scripts/build-site.mjs --out out/pages && datars serve out/pages --port 8960 &
#   site/figures/platforms/capture-android.sh 8960
#
# The same chart box as the iPhone's: the display is set to 1206 px wide at 480 dpi (3×, so 402 dp)
# and tall enough that the app's view — under the status bar and the app bar, above the navigation
# bar — is 2436 px (812 dp); it's restored afterwards. Fails unless DatarsView says its frames were
# drawn on the GPU (GLES on an emulator, Vulkan where it can). Light only, as on iOS: the sample
# app doesn't pass the system's dark theme on to the chart. SKIP_BUILD=1 reuses the last APK.
set -euo pipefail
cd "$(dirname "$0")/../../.."
PORT="${1:-8960}"
URL="http://127.0.0.1:$PORT/c/platforms-everywhere"
ADB="$ANDROID_HOME/platform-tools/adb"
if [ -z "${SKIP_BUILD:-}" ]; then
  bash scripts/build-android.sh >/dev/null
  bash scripts/build-android-sample.sh >/dev/null
fi
if [ -z "$("$ADB" devices | sed 1d | grep -w device)" ]; then
  nohup "$ANDROID_HOME/emulator/emulator" -avd "$("$ANDROID_HOME/emulator/emulator" -list-avds | head -1)" -no-window -no-audio -no-boot-anim -no-snapshot-save >/dev/null 2>&1 &
  "$ADB" wait-for-device
  until [ "$("$ADB" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = "1" ]; do sleep 2; done
fi
# A sample app signed with another debug key can't be replaced in place: start it over.
"$ADB" install -r out/android-sample/datars-sample.apk >/dev/null 2>&1 || { "$ADB" uninstall dev.datars.sample >/dev/null; "$ADB" install out/android-sample/datars-sample.apk >/dev/null; }
"$ADB" reverse "tcp:$PORT" "tcp:$PORT" >/dev/null
restore() { "$ADB" shell wm size reset; "$ADB" shell wm density reset; }
trap restore EXIT

launch() { "$ADB" shell am force-stop dev.datars.sample; "$ADB" shell am start -n dev.datars.sample/.MainActivity --es url "$URL" >/dev/null; }
# The DatarsView's bounds on screen, in px: "left top right bottom".
bounds() {
  "$ADB" shell uiautomator dump /sdcard/datars-ui.xml >/dev/null
  "$ADB" shell cat /sdcard/datars-ui.xml | grep -o 'class="android.view.View"[^>]*clickable="true"[^>]*bounds="[^"]*"' | head -1 \
    | sed -E 's/.*bounds="\[([0-9]+),([0-9]+)\]\[([0-9]+),([0-9]+)\]"/\1 \2 \3 \4/'
}
"$ADB" shell wm density 480
"$ADB" shell wm size 1206x2700
sleep 2; launch; sleep 4
read -r _ T _ B <<<"$(bounds)"
H=$((2700 + 2436 - (B - T)))
"$ADB" shell wm size "1206x$H"
sleep 2
"$ADB" logcat -c
launch; sleep 12
read -r L T R B <<<"$(bounds)"
[ "$L $R $((B - T))" = "0 1206 2436" ] || { echo "the view is $L $T $R $B, not 1206 × 2436" >&2; exit 1; }
TMP=$(mktemp -d)
for state in 0 1 2 3; do
  "$ADB" exec-out screencap -p > "$TMP/$state.png"
  python3 - "$TMP/$state.png" "site/img/platforms-android-$state.png" "$T" <<'PY'
import sys
from PIL import Image
im = Image.open(sys.argv[1]).convert("RGB")
top = int(sys.argv[3])
im.crop((0, top, 1206, top + 2436)).save(sys.argv[2], optimize=True)
PY
  # The sample app steps on a tap (`send("next")`): on the title row's empty end, so nothing is
  # inspected on the way.
  if [ "$state" -lt 3 ]; then "$ADB" shell input tap 1150 $((T + 40)); sleep 4; fi
done
FRAMES=$("$ADB" logcat -d -s datars:I | grep -o 'frames: .*' | tail -1 | tr -d '\r')
echo "${FRAMES:-frames: (no log from DatarsView)}"
case "$FRAMES" in *Vulkan*|*Gl*|*GL*) ;; *) echo "expected frames on the GPU" >&2; exit 1 ;; esac
echo "Android $("$ADB" shell getprop ro.build.version.release | tr -d '\r') (API $("$ADB" shell getprop ro.build.version.sdk | tr -d '\r')), $("$ADB" shell getprop ro.product.model | tr -d '\r'), $(date +%Y-%m-%d)"
