"""Showing charts: notebook outputs, standalone pages, ``show()``.

A chart's notebook output is a MIME bundle with, richest first:

- ``application/vnd.jupyter.widget-view+json`` — the live chart as a widget, when ``anywidget``
  is installed (JupyterLab, Notebook 7, VS Code, Colab): the runtime comes over the kernel
  connection, so it works offline and on remote servers;
- ``text/html`` — the static poster (SVG, or PNG for dense charts) inside a block that upgrades
  itself to the live ``<datars-view>`` where scripts run and the runtime URL loads (a CDN by
  default: exported HTML, or frontends without widgets). Where scripts don't run — GitHub,
  nbviewer, sanitized outputs — the poster is what shows;
- ``image/png`` — for PDF/LaTeX export and renderers that don't show HTML;
- ``text/plain``.

The poster and PNG come from the ``datars`` CLI; without it the output is live-only (with a note).
"""

from __future__ import annotations

import base64
import html as _html
import json
import os
import tempfile
import uuid
import warnings
import webbrowser
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from typing import Any

_STATIC = Path(__file__).resolve().parent / "_static"

#: Where HTML outputs load the runtime from when no widget shows them (``@datars/web`` on npm).
DEFAULT_RUNTIME_URL = "https://cdn.jsdelivr.net/npm/@datars/web@0.1/dist/datars.js"


class Options:
    """``datars.options`` — how charts display (plotly's ``pio.renderers``, Altair's
    ``alt.renderers``).

    renderer
        ``"auto"`` (a widget when anywidget is installed and a kernel is running, else HTML),
        ``"widget"``, ``"html"``, ``"static"`` (images only, like matplotlib inline) or ``"json"``.
    runtime_url
        The runtime HTML outputs import (default: ``DEFAULT_RUNTIME_URL``; self-host with
        ``datars publish``/``serve`` or any copy of ``packages/web/dist``).
    fallback
        The static preview: ``"auto"`` (an SVG poster — PNG when the SVG is large — plus an
        ``image/png`` output), ``"svg"`` (the poster only: lighter notebooks), ``"png"`` or
        ``None`` (no CLI calls at display time).
    png_dpr
        Pixel ratio of the PNG fallback (``2`` for crisp retina output, at 4× the bytes).
    embed_limit
        Documents larger than this many bytes aren't embedded in HTML outputs (poster only;
        widgets still show them live).
    mode
        ``"auto"`` (follow the notebook's theme), ``"light"`` or ``"dark"``.
    digits
        Round float columns to this many significant digits when charts are built (``None``:
        exact). For big data: a million-point cloud's document shrinks by more than half at 6.
    cli, runtime_dir
        The ``datars`` executable and the runtime folder (else found automatically).
    """

    def __init__(self) -> None:
        self.renderer = os.environ.get("DATARS_RENDERER", "auto")
        self.runtime_url: str | None = os.environ.get("DATARS_RUNTIME_URL")
        self.fallback: str | None = os.environ.get("DATARS_FALLBACK", "auto")
        self.png_dpr: float = float(os.environ.get("DATARS_PNG_DPR", "1"))
        self.embed_limit: int = 20_000_000
        self.mode: str = "auto"
        self.digits: int | None = None
        self.cli: str | None = None
        self.runtime_dir: str | None = None

    def __repr__(self) -> str:
        return "datars.options(" + ", ".join(f"{k}={v!r}" for k, v in vars(self).items()) + ")"


options = Options()


def _loader_js() -> str:
    return (_STATIC / "loader.js").read_text(encoding="utf-8")


def _json_for_script(s: str) -> str:
    """JSON safe inside a ``<script>`` element."""
    return s.replace("</", "<\\/").replace("<!--", "\\u003c!--")


def _b64(data: bytes) -> str:
    return base64.b64encode(data).decode("ascii")


# ---- static fallback -------------------------------------------------------------------------------

_warned_cli = False


def static_images(doc, *, fallback: str | None = None) -> dict[str, bytes]:
    """The poster and PNG fallback: ``{"svg": …, "png": …}`` (empty without a CLI)."""
    global _warned_cli
    from . import _cli

    mode = fallback if fallback is not None else options.fallback
    if not mode or mode == "none":
        return {}
    if _cli.find_cli() is None:
        if not _warned_cli:
            _warned_cli = True
            warnings.warn("datars: no `datars` CLI found, so charts have no static preview (they show live only); "
                          "set DATARS_CLI or put datars on PATH", stacklevel=3)
        return {}
    j = doc.to_json()
    want = {"svg", "png"} if mode == "auto" else {mode}
    render_mode = options.mode if options.mode in ("light", "dark") else None
    out: dict[str, bytes] = {}

    def job(fmt: str):
        try:
            return fmt, _cli.render(j, fmt, dpr=options.png_dpr if fmt == "png" else None, mode=render_mode)
        except Exception as err:  # a preview must never break display
            warnings.warn(f"datars: static {fmt} preview failed: {err}", stacklevel=4)
            return fmt, None

    with ThreadPoolExecutor(max_workers=2) as ex:
        for fmt, data in ex.map(job, sorted(want)):
            if data:
                out[fmt] = data
    # An SVG poster of a dense chart (tens of thousands of marks) is megabytes: a PNG instead.
    if mode == "svg" and len(out.get("svg", b"")) > SVG_POSTER_LIMIT:
        _, png = job("png")
        if png:
            out = {"png": png, "png_poster_only": b"1"}
    return out


#: Above this, a poster is a PNG rather than an SVG.
SVG_POSTER_LIMIT = 250_000


def poster_src(images: dict[str, bytes]) -> str | None:
    """The poster as a data URL: SVG (text as outlines, crisp, small), PNG when the SVG is heavy."""
    svg, png = images.get("svg"), images.get("png")
    if svg and (not png or len(svg) <= max(SVG_POSTER_LIMIT, 2 * len(png))):
        return "data:image/svg+xml;base64," + _b64(svg)
    if png:
        return "data:image/png;base64," + _b64(png)
    return None


# ---- HTML ------------------------------------------------------------------------------------------


def _alt(doc) -> str:
    d = doc.to_dict()
    return d.get("title") or d.get("description") or "chart"


def output_html(doc, images: dict[str, bytes] | None = None, *, runtime_url: str | None = None) -> str:
    """The ``text/html`` of a notebook output: poster + document + a loader that goes live."""
    images = static_images(doc) if images is None else images
    w, h = doc.size
    uid = "datars-" + uuid.uuid4().hex[:12]
    src = poster_src(images)
    poster = (f'<img class="datars-poster" alt="{_html.escape(_alt(doc))}" src="{src}" '
              f'style="display:block;width:100%;height:auto">') if src else ""
    doc_json = doc.to_json()
    live = ""
    if len(doc_json) <= options.embed_limit:
        cfg = json.dumps({"url": runtime_url or options.runtime_url or DEFAULT_RUNTIME_URL, "width": w, "height": h, "mode": options.mode})
        live = (f'<script type="application/json" class="datars-doc">{_json_for_script(doc_json)}</script>'
                f'<script type="module">\n{_loader_js()}\nmountOutput({json.dumps(uid)}, {cfg});\n</script>')
    elif not src:
        poster = f'<div class="datars-note">datars: this chart ({len(doc_json) / 1e6:.0f} MB) is too large to embed; use the widget renderer.</div>'
    return f'<div id="{uid}" class="datars-output" style="position:relative;max-width:{w}px">{poster}{live}</div>'


def page(doc, *, runtime: str | None = None) -> str:
    """A standalone HTML page. ``runtime``: ``"inline"`` (offline, embedded), a URL, or None
    (inline when the runtime is installed, else the CDN)."""
    from . import _runtime

    if runtime is None:
        runtime = "inline" if _runtime.available() else (options.runtime_url or DEFAULT_RUNTIME_URL)
    w, h = doc.size
    uid = "datars-page"
    images = static_images(doc)
    src = poster_src(images)
    poster = (f'<img class="datars-poster" alt="{_html.escape(_alt(doc))}" src="{src}" '
              f'style="display:block;width:100%;height:auto">') if src else ""
    embedded = ""
    cfg: dict[str, Any] = {"width": w, "height": h, "mode": options.mode, "page": True}
    if runtime == "inline":
        cfg["inline"] = True
        embedded = "\n".join(f'<script type="application/octet-stream" data-datars-file="{p}">{_b64(b)}</script>'
                             for p, b in _runtime.files().items())
    else:
        cfg["url"] = runtime
    title = _html.escape(_alt(doc))
    return f"""<!doctype html>
<html lang="{_html.escape(doc.to_dict().get("locale", "en"))}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
  body {{ margin: 0; padding: 24px; font-family: system-ui, sans-serif; }}
  .datars-output {{ margin: 0 auto; }}
</style>
</head>
<body>
<div id="{uid}" class="datars-output" style="position:relative;max-width:{w}px">{poster}
<script type="application/json" class="datars-doc">{_json_for_script(doc.to_json())}</script>
</div>
{embedded}
<script type="module">
{_loader_js()}
mountOutput({json.dumps(uid)}, {json.dumps(cfg)});
</script>
</body>
</html>
"""


def write_html(doc, path: str, runtime: str | None = None) -> str:
    with open(path, "w", encoding="utf-8") as f:
        f.write(page(doc, runtime=runtime))
    return str(path)


# ---- the MIME bundle -------------------------------------------------------------------------------


def _in_kernel() -> bool:
    try:
        from IPython import get_ipython

        ip = get_ipython()
        return ip is not None and getattr(ip, "kernel", None) is not None
    except ImportError:
        return False


def _widget_available() -> bool:
    try:
        import anywidget  # noqa: F401

        return True
    except ImportError:
        return False


def renderer() -> str:
    r = options.renderer
    if r == "auto":
        return "widget" if _widget_available() and _in_kernel() else "html"
    return r


def text_repr(doc) -> str:
    d = doc.to_dict()
    states = [s["name"] for s in (d.get("program") or {}).get("states", [])]
    rows = sum(len(next(iter(t["values"].values()), [])) for t in d.get("data", {}).values() if isinstance(t.get("values"), dict))
    extra = f", {len(states)} states" if states else ""
    return f"<datars {type(doc).__name__} {d.get('title') or d.get('id')!r} {d['size']['width']}×{d['size']['height']}, {rows:,} rows{extra}>"


def mimebundle(doc) -> dict[str, Any]:
    r = renderer()
    bundle: dict[str, Any] = {"text/plain": text_repr(doc)}
    if r == "json":
        bundle["application/json"] = doc.to_dict()
        return bundle
    images = static_images(doc)
    if images.get("png") and not images.get("png_poster_only"):
        bundle["image/png"] = _b64(images["png"])
    if r == "static":
        if images.get("svg") and not images.get("png"):
            bundle["image/svg+xml"] = images["svg"].decode("utf-8")
        return bundle
    bundle["text/html"] = output_html(doc, images)
    if r == "widget":
        from .widget import ChartWidget

        w = ChartWidget(doc, poster=poster_src(images))
        doc._last_widget = w  # keep it (and its comm) alive with the chart
        wb = w._repr_mimebundle_()
        if isinstance(wb, tuple):
            wb = wb[0]
        bundle.update({k: v for k, v in wb.items() if k != "text/plain"})
    return bundle


def show(doc) -> None:
    if _in_kernel():
        from IPython.display import display

        display(doc)
        return
    fd, path = tempfile.mkstemp(prefix="datars-", suffix=".html")
    os.close(fd)
    write_html(doc, path)
    webbrowser.open(Path(path).as_uri())
