#!/usr/bin/env bash
# Build the web runtime: crates/datars-host-web → packages/web/dist/wasm (WebGPU + WebGL2 via wgpu,
# the CPU reference as a fallback, QuickJS for T3 bundles).
#
# One-time setup:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version 0.2.128
#   brew install llvm wasi-libc     # QuickJS is C: needs a clang with a wasm backend + a libc
set -euo pipefail
cd "$(dirname "$0")/.."
LLVM="${LLVM:-$(brew --prefix llvm 2>/dev/null || echo /opt/homebrew/opt/llvm)}"
SYSROOT="${WASI_SYSROOT:-$(brew --prefix wasi-libc 2>/dev/null || echo /opt/homebrew/opt/wasi-libc)/share/wasi-sysroot}"
export CC_wasm32_unknown_unknown="$LLVM/bin/clang"
export AR_wasm32_unknown_unknown="$LLVM/bin/llvm-ar"
export CFLAGS_wasm32_unknown_unknown="--sysroot=$SYSROOT -I$SYSROOT/include/wasm32-wasip1 -D__wasi__=1 -D_WASI_EMULATED_SIGNAL"
export BINDGEN_EXTRA_CLANG_ARGS_wasm32_unknown_unknown="--sysroot=$SYSROOT -I$SYSROOT/include/wasm32-wasip1 -D__wasi__=1"
export LIBCLANG_PATH="${LIBCLANG_PATH:-$LLVM/lib}"
export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS="-L $SYSROOT/lib/wasm32-wasip1 -l static=c"
OUT=packages/web/dist/wasm
mkdir -p "$OUT"
# Two engines: `datars_core` (WebGPU + CPU renderers, no sandbox) plays everything the publish
# compiler pre-expanded (T0–T2) — nearly every bundle; `datars_host_web` adds QuickJS for T3 bundles
# and raw source documents. <datars-view> loads the core one unless the chart needs the sandbox.
# Browsers without WebGPU (Safari before iOS 26, WebGPU switched off) load `datars_core_gl` instead:
# the core engine plus the WebGL2 fallback (`webgl` feature, ~665 KB gzipped more) — only they
# download it; the CPU renderer is too slow for big charts (a galaxy of stars at a few frames a
# second). BUILDS="name:features ..." overrides.
BUILDS="${BUILDS-datars_core:gpu datars_core_gl:gpu,webgl datars_host_web:sandbox,gpu}"
cp scripts/wasi_shim.js "$OUT/wasi_shim.js"
# No font is compiled into the web runtime: bundles carry their fonts as chunks. Raw documents
# (`<datars-view doc>`, `datars dev`) fetch the default family from here (`datars:fonts/…`).
mkdir -p packages/web/dist/fonts
cp fonts/Inter-Regular.ttf fonts/Inter-SemiBold.ttf fonts/Inter-Bold.ttf fonts/Inter-LICENSE.txt packages/web/dist/fonts/
# wasm-bindgen, then the glue's fix-ups: the web target's files for engine $1 in directory $2.
bindgen() {
  wasm-bindgen --target web --no-typescript --out-name "$1" --out-dir "$2" target/wasm32-unknown-unknown/wasm-release/datars_host_web.wasm
  # QuickJS's libc imports a few WASI functions; route them to the shim and hand it the memory.
  if grep -q 'wasi_snapshot_preview1' "$2/$1.js"; then
    sed -i.bak 's#from "wasi_snapshot_preview1"#from "./wasi_shim.js"#g' "$2/$1.js" && rm -f "$2/$1.js.bak"
    printf '%s\n' 'import { __setMemory as __datarsSetMemory } from "./wasi_shim.js";' | cat - "$2/$1.js" > "$2/tmp.js" && mv "$2/tmp.js" "$2/$1.js"
    sed -i.bak 's#^    wasm = instance.exports;$#    wasm = instance.exports; __datarsSetMemory(wasm.memory);#' "$2/$1.js" && rm -f "$2/$1.js.bak"
  fi
}
for build in $BUILDS; do
  NAME="${build%%:*}"
  FEATURES="${build#*:}"
  cargo build -p datars-host-web --profile wasm-release --target wasm32-unknown-unknown --no-default-features --features "$FEATURES"
  bindgen "$NAME" "$OUT"
  # Optional binaryen pass (WASM_OPT=1; `cargo install wasm-opt`). Off by default: measured on the
  # core engine, every level (-O1…-Oz, --converge) shrinks the raw file but grows the *gzipped* size
  # (1.97 MB → 1.99–2.14 MB) — rustc's opt-level "z" output compresses better. Semantics-preserving
  # either way (no fast-math), so pixels stay bit-exact. Serve with brotli: the core is ~1.49 MB.
  if [ "${WASM_OPT-0}" = 1 ] && command -v wasm-opt >/dev/null 2>&1; then
    wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext --enable-mutable-globals --enable-multivalue --enable-reference-types \
      "$OUT/${NAME}_bg.wasm" -o "$OUT/${NAME}_bg.opt.wasm" && mv "$OUT/${NAME}_bg.opt.wasm" "$OUT/${NAME}_bg.wasm"
  fi
  echo "$OUT/${NAME}_bg.wasm: $(du -h "$OUT/${NAME}_bg.wasm" | cut -f1), $(gzip -9 -c "$OUT/${NAME}_bg.wasm" | wc -c | awk '{printf "%.0f KB", $1/1024}') gzipped"
done

# Profiling engines (NAMES=1): the same builds linked with their function names, in
# target/wasm-names/ — `perf-browser.mjs --profile` serves them in place of the shipped ones, so a
# page's CPU profile names Rust functions instead of `wasm-function[956]`. Never shipped: with a
# name section in its input, wasm-bindgen's output differs (named closure exports, a few KB more).
if [ "${NAMES-0}" = 1 ]; then
  NAMED=target/wasm-names
  mkdir -p "$NAMED"
  cp scripts/wasi_shim.js "$NAMED/wasi_shim.js"
  for build in $BUILDS; do
    NAME="${build%%:*}"
    CARGO_PROFILE_WASM_RELEASE_STRIP=false cargo build -p datars-host-web --profile wasm-release --target wasm32-unknown-unknown --no-default-features --features "${build#*:}"
    bindgen "$NAME" "$NAMED"
    echo "$NAMED/${NAME}_bg.wasm: with function names (profiling only)"
  done
  # Leave the target's last link a shipped (stripped) one, as without NAMES.
  touch crates/datars-host-web/src/lib.rs
fi
