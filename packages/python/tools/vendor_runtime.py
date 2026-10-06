"""Copy the web runtime (``packages/web/dist``) into the package, for offline live charts.

    python tools/vendor_runtime.py [path/to/packages/web/dist]   # default: ../web/dist

Run before building a wheel (``python -m build``); ``src/datars/_runtime`` is not in git. Only
what notebooks use is copied: ``datars.js``, the full engine (``datars_host_web``: raw documents
need the recipe sandbox), its WASI shim, the Inter faces (+ licence) and the countries atlas.
Optionally ``--cli path/to/datars`` also copies a CLI binary into ``src/datars/_bin`` (for
platform wheels that render PNG/SVG/PDF/MP4 without a separate install).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import stat
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "src"))

from datars._runtime import EXTRA, FILES  # noqa: E402


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("dist", nargs="?", default=str(ROOT.parent / "web" / "dist"))
    ap.add_argument("--cli", help="also vendor this datars CLI binary into src/datars/_bin")
    a = ap.parse_args(argv)
    src = Path(a.dist)
    missing = [f for f in FILES if not (src / f).is_file()]
    if missing:
        print(f"{src} lacks {', '.join(missing)} — build the web runtime (scripts/build-wasm.sh, pnpm -r build)", file=sys.stderr)
        return 1
    out = ROOT / "src" / "datars" / "_runtime"
    if out.exists():
        shutil.rmtree(out)
    manifest = {}
    for f in (*FILES, *EXTRA):
        if not (src / f).is_file():
            continue
        (out / f).parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(src / f, out / f)
        manifest[f] = hashlib.sha256((out / f).read_bytes()).hexdigest()
    (out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    total = sum((out / f).stat().st_size for f in manifest)
    print(f"vendored {len(manifest)} files ({total / 1e6:.1f} MB) into {out.relative_to(ROOT)}")
    if a.cli:
        b = ROOT / "src" / "datars" / "_bin"
        b.mkdir(exist_ok=True)
        dst = b / Path(a.cli).name
        shutil.copyfile(a.cli, dst)
        dst.chmod(dst.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
        print(f"vendored the CLI into {dst.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
