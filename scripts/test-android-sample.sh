#!/usr/bin/env bash
# The Phase 2 exit on Android: the sample app (built once) plays a chart published afterwards, from
# a static site over HTTP, and shows a republish on relaunch. Boots the first AVD if needed; leaves
# screenshots in out/android-*.png.
set -euo pipefail
cd "$(dirname "$0")/.."
ADB="$ANDROID_HOME/platform-tools/adb"
bash scripts/build-android.sh >/dev/null
bash scripts/build-android-sample.sh
if [ -z "$("$ADB" devices | sed 1d | grep -w device)" ]; then
  nohup "$ANDROID_HOME/emulator/emulator" -avd "$("$ANDROID_HOME/emulator/emulator" -list-avds | head -1)" -no-window -no-audio -no-boot-anim -no-snapshot-save >/dev/null 2>&1 &
  "$ADB" wait-for-device
  until [ "$("$ADB" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = "1" ]; do sleep 2; done
fi
"$ADB" install -r out/android-sample/datars-sample.apk >/dev/null
"$ADB" logcat -c
# Publish *after* the app is installed, serve it, and let the device reach the host.
SITE=out/android-site && rm -rf "$SITE"
target/release/datars publish examples/votes/doc.json --alias votes --to "$SITE" >/dev/null
target/release/datars serve "$SITE" --port 8791 >/dev/null 2>&1 & SERVER=$!
trap 'kill $SERVER' EXIT
"$ADB" reverse tcp:8791 tcp:8791 >/dev/null
launch() { "$ADB" shell am force-stop dev.datars.sample; "$ADB" shell am start -n dev.datars.sample/.MainActivity --es url http://127.0.0.1:8791/c/votes >/dev/null; sleep 6; }
launch; "$ADB" exec-out screencap -p > out/android-published.png
"$ADB" shell input tap 540 1200; sleep 3; "$ADB" exec-out screencap -p > out/android-step.png
python3 -c "import json; d=json.load(open('examples/votes/doc.json')); d['scene']['children'][0]['params']['title']='Republished'; json.dump(d, open('out/votes-republished.json','w'))"
target/release/datars publish out/votes-republished.json --alias votes --to "$SITE" --revision 2 >/dev/null
launch; "$ADB" exec-out screencap -p > out/android-republished.png
echo "out/android-published.png, out/android-step.png, out/android-republished.png"
# Which renderer drew: DatarsView logs it when its surface arrives (Vulkan, else GL; CPU pixels without a GPU).
FRAMES=$("$ADB" logcat -d -s datars:I | grep -o 'frames: .*' | tail -1 | tr -d '\r')
echo "${FRAMES:-frames: (no log from DatarsView)}"
case "$FRAMES" in *Vulkan*|*Gl*) ;; *) echo "expected frames on the GPU" >&2; exit 1 ;; esac
