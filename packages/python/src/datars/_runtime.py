"""The web runtime's files, for offline live charts (widgets and standalone pages).

``@datars/web``'s build (``packages/web/dist``) is vendored into this package by
``tools/vendor_runtime.py`` at build time — ``datars.js``, the full engine (``datars_host_web``,
with the recipe sandbox that raw documents need), its WASI shim, the default Inter faces and the
countries atlas: about 6 MB, 2 MB compressed in a wheel. Looked up as:

1. ``datars.options.runtime_dir`` or ``DATARS_WEB_DIST`` (a ``packages/web/dist`` folder);
2. the vendored copy (``datars/_runtime``);
3. a build in the repository checkout this package sits in (``packages/web/dist``, for development).
"""

from __future__ import annotations

import os
from pathlib import Path

_HERE = Path(__file__).resolve().parent

#: What a notebook needs from ``packages/web/dist`` (paths relative to it).
FILES = (
    "datars.js",
    "wasm/datars_host_web.js",
    "wasm/datars_host_web_bg.wasm",
    "wasm/wasi_shim.js",
    "fonts/Inter-Regular.ttf",
    "fonts/Inter-SemiBold.ttf",
    "fonts/Inter-Bold.ttf",
    "atlas/countries.geojson",
)
#: Shipped for the licence, not loaded.
EXTRA = ("fonts/Inter-LICENSE.txt",)


def runtime_dir() -> Path | None:
    """The folder holding the runtime's files, or None when there's none."""
    from ._display import options

    cands = [options.runtime_dir, os.environ.get("DATARS_WEB_DIST"), _HERE / "_runtime", _HERE.parents[2] / "web" / "dist"]
    for c in cands:
        if c and all((Path(c) / f).is_file() for f in FILES):
            return Path(c)
    return None


def files() -> dict[str, bytes]:
    """Every runtime file by its path (raises when the runtime isn't available)."""
    d = runtime_dir()
    if d is None:
        raise FileNotFoundError(
            "datars: the web runtime isn't installed with this package (run tools/vendor_runtime.py in a checkout, "
            "or set DATARS_WEB_DIST to a built packages/web/dist)"
        )
    return {f: (d / f).read_bytes() for f in FILES}


def available() -> bool:
    return runtime_dir() is not None
