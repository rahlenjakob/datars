#!/usr/bin/env bash
# What VoiceOver says, for real, in Safari: the budget, controls, votes and prices examples on
# plain pages, VoiceOver's cursor moved through them and keys pressed, and what it last said after
# each step printed (and kept in out/voiceover/<chart>.txt). It runs on your screen, with your
# VoiceOver — so: VoiceOver on, and "Allow VoiceOver to be controlled with AppleScript" on
# (VoiceOver Utility → General); the script changes no setting. Safari opens and closes its
# window. Needs the CLI and the web runtime built (scripts/build-wasm.sh, pnpm -C packages/web build).
#   scripts/test-voiceover.sh [chart …]
set -euo pipefail
cd "$(dirname "$0")/.."
PORT="${PORT:-8795}"
OUT=out/voiceover && mkdir -p "$OUT"
SITE=out/voiceover-site
bash scripts/build-a11y-pages.sh "$SITE" >/dev/null
target/release/datars serve "$SITE" --port "$PORT" >/dev/null 2>&1 & SERVER=$!
trap 'kill $SERVER 2>/dev/null; osascript -e "tell application \"Safari\" to close every window" >/dev/null 2>&1 || true' EXIT
osa() { perl -e 'alarm 20; exec @ARGV' osascript -e "$1" 2>&1; }
# VoiceOver takes AppleScript only with "Allow VoiceOver to be controlled with AppleScript" on: on
# this macOS a flag file, not a default — and VoiceOver must answer.
[ -f /private/var/db/Accessibility/.VoiceOverAppleScriptEnabled ] || { echo "VoiceOver's AppleScript control is off" >&2; exit 1; }
osa 'tell application "VoiceOver" to return content of last phrase' >/dev/null || { echo "VoiceOver doesn't answer" >&2; exit 1; }
said() { osa 'tell application "VoiceOver" to return content of last phrase'; }

# read <chart> <steps…>: right, left, into, out, act, or key:<return|escape|left|right|up|down|tab>
# (key:shift-right …): the chart's page in Safari, VoiceOver in its web content, then each step.
read_chart() {
  local app=Safari chart=$1; shift
  osascript -e 'tell application "Safari" to close every window' >/dev/null 2>&1 || true
  if [ -n "${NOOPEN:-}" ]; then
    : # (the caller has the page in front)
  elif [ "$app" = "Google Chrome" ]; then
    # (A profile of its own: no first-run screens, and nothing of the user's touched.)
    open -na "Google Chrome" --args --user-data-dir="$(dirname "$0")/chrome-profile" --no-first-run --no-default-browser-check --disable-search-engine-choice-screen --force-renderer-accessibility "http://127.0.0.1:$PORT/$chart.html"
  else
    open -a "$app" "http://127.0.0.1:$PORT/$chart.html"
  fi
  sleep ${WAIT:-6}
  [ -n "${NOOPEN:-}" ] || osa "tell application \"$app\" to activate" >/dev/null
  sleep 1
  # Into the page: out to the window (VoiceOver remembers where it was), across to the web content,
  # in, and to its first item.
  for _ in 1 2 3 4 5 6 7; do osa 'tell application "VoiceOver" to tell vo cursor to move out of item' >/dev/null; sleep 0.3; done
  last=""
  for _ in $(seq 1 24); do
    osa 'tell application "VoiceOver" to tell vo cursor to move right' >/dev/null; sleep 0.8
    now=$(said)
    [ -n "${DEBUG:-}" ] && echo "  (setup) $now"
    # (The web area itself — not "Out of … web content", which a move out also says.)
    case "$now" in "Out of"*|"In "*) ;; *"web content"*) break ;; esac
    # Nothing further along this level (Chrome's window is one group): in, and on.
    if [ "$now" = "$last" ]; then osa 'tell application "VoiceOver" to tell vo cursor to move into item' >/dev/null; sleep 0.8; fi
    last=$now
  done
  osa 'tell application "VoiceOver" to tell vo cursor to move into item' >/dev/null; sleep 0.8
  osa 'tell application "VoiceOver" to tell vo cursor to move to first item' >/dev/null; sleep 0.8
  echo "[open] $(said)"
  for s in "$@"; do
    case $s in
      right|left|up|down) osa "tell application \"VoiceOver\" to tell vo cursor to move $s" >/dev/null ;;
      into) osa 'tell application "VoiceOver" to tell vo cursor to move into item' >/dev/null ;;
      out) osa 'tell application "VoiceOver" to tell vo cursor to move out of item' >/dev/null ;;
      first) osa 'tell application "VoiceOver" to tell vo cursor to move to first item' >/dev/null ;;
      act) osa 'tell application "VoiceOver" to tell vo cursor to perform action' >/dev/null ;;
      key:*) k=${s#key:}; mods=""; case $k in shift-*) mods=" using shift down"; k=${k#shift-};; esac
        case $k in return) c=36;; escape) c=53;; left) c=123;; right) c=124;; down) c=125;; up) c=126;; tab) c=48;; space) c=49;; plus) c=24;; minus) c=27;; zero) c=29;; esac
        osa "tell application \"System Events\" to key code $c$mods" >/dev/null ;;
    esac
    # (Read before VoiceOver's hint — "You are currently on …" — replaces the item's words.)
    sleep ${PAUSE:-0.8}
    echo "[$s] $(said)"
  done
}
R22=(); for _ in $(seq 1 22); do R22+=(right); done
R11=(); for _ in $(seq 1 11); do R11+=(right); done
for c in ${@:-budget controls votes prices}; do
  case $c in
    budget) { read_chart budget "${R22[@]}"; PAUSE=1.2 read_chart budget right right right right right key:right key:right key:left; } ;;
    controls) { read_chart controls "${R22[@]}"; PAUSE=1.2 read_chart controls "${R11[@]}" act act right act; } ;;
    votes) { read_chart votes "${R22[@]}"; PAUSE=2 read_chart votes right right right right act act act; } ;;
    # The brush: the figure clicked by VoiceOver (the keyboard's place), then Tab to it.
    prices) PAUSE=1.5 read_chart prices right right act key:tab key:tab key:tab key:tab key:return key:right key:right key:shift-right key:return key:escape ;;
  esac | tee "$OUT/$c.txt"
done
