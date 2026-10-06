#!/usr/bin/env bash
# Build @datars/sdk and @datars/std and copy their bundles into the engine (built-in packages the
# sandbox loads). Run after changing anything in packages/sdk or packages/std.
set -euo pipefail
cd "$(dirname "$0")/.."
pnpm -C packages/sdk build >/dev/null
pnpm -C packages/std build >/dev/null
cp packages/sdk/dist/sdk.bundle.js crates/datars-engine/js/sdk.js
cp packages/std/dist/std.bundle.js crates/datars-engine/js/std.js
echo "sdk.js $(wc -c < crates/datars-engine/js/sdk.js) bytes, std.js $(wc -c < crates/datars-engine/js/std.js) bytes → crates/datars-engine/js/"
