#!/usr/bin/env bash
# Subset the bundled fonts (fonts/source → fonts/). Every target renders with the same bundled
# faces (P1), so the subset is the engine's built-in coverage: Latin (incl. Extended-A), Greek,
# basic Cyrillic, and the punctuation, currency, arrows, maths and shapes charts use. Other
# scripts arrive as font chunks with the bundle (docs/12-delivery.md). Needs fonttools.
set -euo pipefail
cd "$(dirname "$0")/.."
RANGES="U+0000-00FF,U+0100-017F,U+0218-021B,U+02C6-02DD,U+0370-03FF,U+0400-045F,U+0490-0491,U+1E9E,U+2000-206F,U+2070-209F,U+20A0-20CF,U+2100-214F,U+2150-218F,U+2190-21FF,U+2200-22FF,U+2300-23FF,U+25A0-25FF,U+2600-26FF,U+2713-2717,U+FB01-FB02,U+FEFF,U+FFFD"
for f in fonts/source/Inter-*.ttf; do
  out="fonts/$(basename "$f")"
  pyftsubset "$f" --unicodes="$RANGES" --layout-features+=tnum,pnum,lnum,case,zero,ss01,cv05 --name-IDs="*" --name-languages="*" --no-hinting --desubroutinize --output-file="$out"
  echo "$out: $(wc -c < "$out" | tr -d ' ') bytes ($(gzip -9 -c "$out" | wc -c | tr -d ' ') gzipped)"
done
