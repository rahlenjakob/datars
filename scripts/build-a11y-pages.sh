#!/usr/bin/env bash
# Plain pages, one chart each (budget, controls, votes, prices), for driving a real screen reader
# over the web element: <dir>/<chart>.html with the web runtime at <dir>/runtime. Needs the
# runtime built (scripts/build-wasm.sh, pnpm -C packages/web build) and node.
#   scripts/build-a11y-pages.sh <dir>
set -euo pipefail
cd "$(dirname "$0")/.."
SITE="${1:?usage: scripts/build-a11y-pages.sh <dir>}"
rm -rf "$SITE" && mkdir -p "$SITE/runtime"
cp -R packages/web/dist/* "$SITE/runtime/"
for c in budget votes prices; do cp "examples/$c/doc.json" "$SITE/$c.json"; done
node scripts/doc-to-json.mjs site/charts/controls.ts > "$SITE/controls.json"
for c in budget votes prices controls; do
  printf '<!doctype html><html lang="en"><meta charset="utf-8"><title>%s</title><script type="module" src="/runtime/datars.js"></script><h1>%s</h1><p>Before the chart</p><datars-view doc="/%s.json" height="460" style="width:720px"></datars-view><p>After the chart</p></html>' "$c" "$c" "$c" > "$SITE/$c.html"
done
echo "$SITE: budget, controls, votes, prices"
