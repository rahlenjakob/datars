"""The ``datars`` CLI as a subprocess: renders (PNG/SVG/PDF), films (MP4), bundles and publishes.

The CLI is the engine's headless host (``crates/datars-headless`` behind ``crates/datars-cli``): the
same pipeline every runtime runs, so a saved PNG is exactly what the notebook shows. Found as
(first wins):

1. ``datars.options.cli`` or the ``DATARS_CLI`` environment variable;
2. a binary shipped inside this package (``datars/_bin/datars``, for platform wheels);
3. ``datars`` on ``PATH``;
4. a build in the repository checkout this package sits in (``target/release/datars``, for development).
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Any, Sequence

_HERE = Path(__file__).resolve().parent


class CLINotFound(RuntimeError):
    """No ``datars`` executable: install it, or point ``DATARS_CLI`` at one."""


def find_cli() -> str | None:
    from ._display import options

    for cand in (options.cli, os.environ.get("DATARS_CLI")):
        if cand and Path(cand).expanduser().is_file():
            return str(Path(cand).expanduser())
    exe = "datars.exe" if os.name == "nt" else "datars"
    bundled = _HERE / "_bin" / exe
    if bundled.is_file():
        return str(bundled)
    on_path = shutil.which("datars")
    if on_path:
        return on_path
    # packages/python/src/datars → target/release/datars, in a checkout of the repository
    dev = _HERE.parents[3] / "target" / "release" / exe
    return str(dev) if dev.is_file() else None


def require_cli() -> str:
    exe = find_cli()
    if not exe:
        raise CLINotFound(
            "datars: the `datars` CLI renders PNG/SVG/PDF/MP4 — install it (cargo install --path crates/datars-cli, "
            "or a release binary) and put it on PATH, or set DATARS_CLI=/path/to/datars"
        )
    return exe


def run(args: Sequence[str], *, doc_json: str | None = None, cwd: str | None = None, timeout: float = 600) -> subprocess.CompletedProcess:
    """Run the CLI with the document written to a temp ``doc.json`` (substituted for ``{doc}``)."""
    exe = require_cli()
    with tempfile.TemporaryDirectory(prefix="datars-") as tmp:
        argv = list(args)
        if doc_json is not None:
            path = os.path.join(tmp, "doc.json")
            with open(path, "w", encoding="utf-8") as f:
                f.write(doc_json)
            argv = [path if a == "{doc}" else a for a in argv]
        argv = [a.replace("{tmp}", tmp) for a in argv]
        proc = subprocess.run([exe, *argv], capture_output=True, text=True, cwd=cwd or tmp, timeout=timeout)
        if proc.returncode != 0:
            raise RuntimeError(f"datars {argv[0]} failed:\n{(proc.stderr or proc.stdout).strip()}")
        return proc


def _state_args(state: Any) -> list[str]:
    return [] if state is None else ["--state", str(state)]


def render(doc_json: str, fmt: str = "png", *, state: Any = None, dpr: float | None = None, mode: str | None = None,
           size: tuple[int, int] | None = None) -> bytes:
    """One state rendered by the CPU reference (PNG) or the vector backends (SVG, PDF)."""
    if fmt not in ("png", "svg", "pdf"):
        raise ValueError(f"datars: can't render {fmt!r} (png, svg or pdf)")
    exe = require_cli()
    with tempfile.TemporaryDirectory(prefix="datars-") as tmp:
        doc = os.path.join(tmp, "doc.json")
        with open(doc, "w", encoding="utf-8") as f:
            f.write(doc_json)
        out = os.path.join(tmp, f"out.{fmt}")
        argv = [exe, "render", doc, "--out", out, *_state_args(state)]
        if dpr is not None and fmt == "png":
            argv += ["--dpr", str(dpr)]
        if mode:
            argv += ["--mode", mode]
        if size:
            argv += ["--size", f"{int(size[0])}x{int(size[1])}"]
        proc = subprocess.run(argv, capture_output=True, text=True, cwd=tmp, timeout=300)
        if proc.returncode != 0 or not os.path.exists(out):
            raise RuntimeError(f"datars render failed:\n{(proc.stderr or proc.stdout).strip()}")
        with open(out, "rb") as f:
            return f.read()


def save(doc, path: str, *, state: Any = None, dpr: float | None = None, mode: str | None = None,
         size: tuple[int, int] | None = None, fps: int | None = None, hold: float | None = None) -> str:
    """``Document.save``: the format from the extension."""
    path = os.path.abspath(os.path.expanduser(path))
    ext = os.path.splitext(path)[1].lower().lstrip(".")
    j = doc.to_json()
    if ext in ("png", "svg", "pdf"):
        data = render(j, ext, state=state, dpr=dpr, mode=mode, size=size)
        with open(path, "wb") as f:
            f.write(data)
        return path
    if ext in ("mp4", "mov", "webm"):
        argv = ["video", "{doc}", "--out", path]
        if fps:
            argv += ["--fps", str(fps)]
        if hold is not None:
            argv += ["--hold", str(hold)]
        if dpr is not None:
            argv += ["--dpr", str(dpr)]
        if size:
            argv += ["--size", f"{int(size[0])}x{int(size[1])}"]
        run(argv, doc_json=j)
        return path
    if ext == "datars":
        run(["bundle", "{doc}", "--out", path], doc_json=j)
        return path
    raise ValueError(f"datars: don't know how to save {ext!r} (png, svg, pdf, mp4, datars, json, html)")


def publish(doc, to: str, *, alias: str) -> str:
    to = os.path.abspath(os.path.expanduser(to))
    run(["publish", "{doc}", "--alias", alias, "--to", to], doc_json=doc.to_json())
    return f'<script type="module" src="https://cdn.jsdelivr.net/npm/@datars/web/dist/datars.js"></script>\n<datars-view src="c/{alias}"></datars-view>'


def check(doc_json: str) -> list[str]:
    """``datars check --json``: the engine's diagnostics (it exits non-zero when there are any)."""
    exe = require_cli()
    with tempfile.TemporaryDirectory(prefix="datars-") as tmp:
        path = os.path.join(tmp, "doc.json")
        with open(path, "w", encoding="utf-8") as f:
            f.write(doc_json)
        proc = subprocess.run([exe, "check", path, "--json"], capture_output=True, text=True, cwd=tmp, timeout=300)
    try:
        diags = json.loads(proc.stdout).get("diagnostics", [])
    except json.JSONDecodeError:
        if proc.returncode == 0:
            return []
        raise RuntimeError(f"datars check failed:\n{(proc.stderr or proc.stdout).strip()}") from None
    return [d if isinstance(d, str) else d.get("message", json.dumps(d)) for d in diags]


def describe() -> dict:
    """Every std recipe's description (``datars describe --json``)."""
    return json.loads(run(["describe", "--json"]).stdout)
