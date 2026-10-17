#!/usr/bin/env bash
# What TalkBack says, for real: on an Android emulator with TalkBack (a Google APIs or Play image —
# com.google.android.marvin.talkback), the sample app shows example charts and TalkBack is driven
# with real touches — swipe right for the next item, double-tap to activate, swipe up and down to
# adjust, a touch to explore — written to the emulator's touchscreen as kernel input events, so
# TalkBack's own gesture detection sees them (`adb shell input` would bypass it). What TalkBack
# speaks is read from its log (its developer setting "log output level: verbose"). Prints one line
# per gesture with what was spoken after it, into out/talkback/<chart>.txt as well.
#
# Needs an emulator image `adb root` works on, a built sample (scripts/build-android-sample.sh) and
# the CLI. Turns TalkBack on, and off again at the end, with the emulator's settings as they were.
#   scripts/test-talkback.sh [chart …]      (default: votes budget controls prices)
set -euo pipefail
cd "$(dirname "$0")/.."
ADB="$ANDROID_HOME/platform-tools/adb"
PORT="${PORT:-8794}"
TB=com.google.android.marvin.talkback
PREFS=/data/user_de/0/$TB/shared_prefs/${TB}_preferences.xml
OUT=out/talkback && mkdir -p "$OUT"
t() { perl -e 'alarm shift; exec @ARGV' "$@"; }

"$ADB" shell pm path $TB >/dev/null || { echo "no TalkBack on this image: use a Google APIs or Play image" >&2; exit 1; }
"$ADB" root >/dev/null; sleep 2
"$ADB" install -r out/android-sample/datars-sample.apk >/dev/null
SITE=out/talkback-site && rm -rf "$SITE"
for c in votes budget prices; do target/release/datars publish "examples/$c/doc.json" --alias "$c" --to "$SITE" >/dev/null; done
target/release/datars publish site/charts/controls.ts --alias controls --to "$SITE" >/dev/null
target/release/datars serve "$SITE" --port "$PORT" >/dev/null 2>&1 & SERVER=$!
"$ADB" reverse tcp:$PORT tcp:$PORT >/dev/null

# Touches: protocol B multitouch on the first virtio touchscreen (0–32767 on both axes).
"$ADB" shell 'cat > /data/local/tmp/touch.sh' <<'EOF'
E=/dev/input/event1
W=$(wm size | sed 's/.* //; s/x.*//'); H=$(wm size | sed 's/.*x//')
sx() { echo $(( $1 * 32767 / W )); }
sy() { echo $(( $1 * 32767 / H )); }
down() { sendevent $E 3 47 0; sendevent $E 3 57 $3; sendevent $E 3 53 $(sx $1); sendevent $E 3 54 $(sy $2); sendevent $E 3 58 60; sendevent $E 0 0 0; }
move() { sendevent $E 3 53 $(sx $1); sendevent $E 3 54 $(sy $2); sendevent $E 0 0 0; }
up() { sendevent $E 3 57 4294967295; sendevent $E 0 0 0; }
case $1 in
  swipe) down $2 $3 101; i=1; while [ $i -le 6 ]; do move $(( $2 + ($4 - $2) * i / 6 )) $(( $3 + ($5 - $3) * i / 6 )); i=$((i+1)); done; up ;;
  tap) down $2 $3 102; up ;;
  doubletap) down $2 $3 103; up; down $2 $3 104; up ;;
esac
EOF

# TalkBack on, logging what it speaks.
WAS_SERVICES=$("$ADB" shell settings get secure enabled_accessibility_services | tr -d '\r')
WAS_ENABLED=$("$ADB" shell settings get secure accessibility_enabled | tr -d '\r')
restore() {
  "$ADB" shell settings put secure enabled_accessibility_services "${WAS_SERVICES:-null}" >/dev/null
  "$ADB" shell settings put secure accessibility_enabled "${WAS_ENABLED:-0}" >/dev/null
  kill $SERVER 2>/dev/null || true
}
trap restore EXIT
"$ADB" shell settings put secure enabled_accessibility_services $TB/$TB.TalkBackService
"$ADB" shell settings put secure accessibility_enabled 1
sleep 6
"$ADB" shell pm grant $TB android.permission.POST_NOTIFICATIONS 2>/dev/null || true
if ! "$ADB" shell grep -q pref_log_level "$PREFS" 2>/dev/null; then
  "$ADB" shell settings put secure enabled_accessibility_services null; sleep 2
  "$ADB" shell "sed -i 's#<map>#<map><string name=\"pref_log_level\">2</string>#' $PREFS"
  "$ADB" shell settings put secure enabled_accessibility_services $TB/$TB.TalkBackService; sleep 6
fi
"$ADB" shell input keyevent KEYCODE_BACK; sleep 1

said() { t 10 "$ADB" logcat -d | grep 'Speaking fragment text=' | sed 's/.*Speaking fragment text="//; s/", utteranceId=.*//' | paste -sd '|' -; }
run() { # run <chart> <gestures…>: next, prev, up, down, activate, tap:X:Y
  local chart=$1; shift
  "$ADB" logcat -c
  "$ADB" shell "am start -S -n dev.datars.sample/.MainActivity --es url http://127.0.0.1:$PORT/c/$chart" >/dev/null
  sleep 9
  echo "[open] $(said)"
  "$ADB" logcat -c
  for g in "$@"; do
    case $g in
      next) "$ADB" shell sh /data/local/tmp/touch.sh swipe 200 1300 900 1300 ;;
      prev) "$ADB" shell sh /data/local/tmp/touch.sh swipe 900 1300 200 1300 ;;
      up) "$ADB" shell sh /data/local/tmp/touch.sh swipe 540 1700 540 900 ;;
      down) "$ADB" shell sh /data/local/tmp/touch.sh swipe 540 900 540 1700 ;;
      activate) "$ADB" shell sh /data/local/tmp/touch.sh doubletap 540 1300 ;;
      tap:*) IFS=: read -r _ x y <<< "$g"; "$ADB" shell sh /data/local/tmp/touch.sh tap "$x" "$y" ;;
    esac
    sleep 2.5
    echo "[$g] $(said)"
    "$ADB" logcat -c
  done
}
NEXT20=(); for _ in $(seq 1 20); do NEXT20+=(next); done
for chart in "${@:-votes budget controls prices}"; do
  for c in $chart; do
    case $c in
      votes) run votes "${NEXT20[@]}"; run votes next activate activate activate ;;
      budget) run budget "${NEXT20[@]}"; run budget next up up down ;;
      controls) run controls "${NEXT20[@]}"; run controls next next next next next next activate activate next activate ;;
      # (The brush is reached by a touch in the overview plot, above its line.)
      prices) run prices "${NEXT20[@]}"; run prices tap:450:2050 up up activate down down activate ;;
      *) run "$c" "${NEXT20[@]}" ;;
    esac | tee "$OUT/$c.txt"
  done
done
