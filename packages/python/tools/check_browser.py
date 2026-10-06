"""Check the live outputs in a real browser (headless Chrome): the evidence that charts go live.

    python tools/check_browser.py [--out examples/out/browser]

Builds three pages from one chart and opens each in headless Chrome (``tools/browser_check.mjs``):

1. ``standalone.html`` — ``chart.to_html()`` with the runtime inlined (offline, blob: URLs);
2. ``output.html``    — a notebook ``text/html`` output loading the runtime by URL;
3. ``widget.html``    — the anywidget front end (``ChartWidget._esm``) under a stand-in for the
   widget manager that answers ``need-runtime`` like the kernel does, then a real click on a bar
   (the ``pick`` message must carry the bar's data row) and a step driven from "Python".

Needs Node, Chrome (``CHROME`` or the macOS default), the runtime (vendored or ``DATARS_WEB_DIST``)
and, for posters, the CLI. Exits 1 when a view didn't render.
"""

from __future__ import annotations

import argparse
import functools
import http.server
import json
import shutil
import subprocess
import sys
import threading
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "src"))

import datars as dr  # noqa: E402
from datars import _display, _runtime  # noqa: E402

VOTES = {"party": ["S", "SD", "M", "V", "C", "KD", "MP", "L"], "share": [30.3, 20.5, 19.1, 6.8, 6.7, 5.3, 5.1, 4.6]}

HARNESS = """<!doctype html><meta charset="utf-8"><title>widget harness</title>
<body style="margin:0;padding:20px;width:900px"><div id="w"></div>
<script type="module">
// A stand-in for the widget manager: model state, change events, custom messages — and the
// kernel's answer to `need-runtime` (the runtime files as binary buffers).
const esm = __ESM__;
const state = __STATE__;
// Bytes traits arrive as DataViews (the document travels as a binary buffer).
state.doc = new DataView(new TextEncoder().encode(state.doc).buffer);
const listeners = {};
window.__sent = [];
const emit = (ev, ...a) => (listeners[ev] || []).slice().forEach((f) => f(...a));
const model = {
  get: (k) => state[k],
  set: (k, v) => { state[k] = v; emit("change:" + k); },
  save_changes() {},
  on(ev, f) { (listeners[ev] ??= []).push(f); },
  off(ev, f) { listeners[ev] = (listeners[ev] || []).filter((g) => g !== f); },
  send(msg) {
    window.__sent.push(msg);
    if (msg.type === "need-runtime") {
      const paths = __PATHS__;
      Promise.all(paths.map((p) => fetch("rt/" + p).then((r) => r.arrayBuffer()).then((b) => new DataView(b))))
        .then((bufs) => emit("msg:custom", { type: "runtime", paths }, bufs));
    }
  },
};
window.__model = model;
const mod = await import(URL.createObjectURL(new Blob([esm], { type: "text/javascript" })));
mod.default.render({ model, el: document.getElementById("w") });
</script>
"""


class _Quiet(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *args) -> None:  # no request log on the console
        pass


def serve(root: Path) -> tuple[http.server.ThreadingHTTPServer, int]:
    handler = functools.partial(_Quiet, directory=str(root))
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv, srv.server_address[1]


def check(url: str, shot: Path, *extra: str, wait: int = 7000) -> dict:
    cmd = ["node", str(ROOT / "tools" / "browser_check.mjs"), url, str(shot), "--wait", str(wait), *extra]
    out = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
    try:
        return json.loads(out.stdout)
    except json.JSONDecodeError:
        return {"url": url, "views": [], "logs": [out.stderr.strip() or out.stdout.strip()], "evals": []}


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", default=str(ROOT / "examples" / "out" / "browser"))
    a = ap.parse_args(argv)
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    rt = _runtime.runtime_dir()
    if rt is None:
        print("no runtime: run tools/vendor_runtime.py or set DATARS_WEB_DIST", file=sys.stderr)
        return 1
    if (out / "rt").exists():
        shutil.rmtree(out / "rt")
    shutil.copytree(rt, out / "rt")
    chart = dr.story(
        dr.bar(VOTES, x="party", y="share", color="party", title="Vote share, 2022 (%)", labels=True),
        (dr.pie(VOTES, names="party", values="share"), "Each bar becomes a slice."),
    )
    (out / "standalone.html").write_text(chart.to_html(runtime="inline"), encoding="utf-8")
    srv, port = serve(out)
    base = f"http://127.0.0.1:{port}"
    html = _display.output_html(chart, runtime_url=f"{base}/rt/datars.js")
    (out / "output.html").write_text(f'<!doctype html><meta charset="utf-8"><body style="margin:0;padding:20px;width:900px">{html}</body>', encoding="utf-8")
    images = _display.static_images(chart)
    w, h = chart.size
    d = chart.to_dict()
    state = {"doc": chart.to_json(), "poster": _display.poster_src(images) or "", "alt": d.get("title") or "", "width": w,
             "height": h, "state": "", "states": chart.state_names, "signals": {}, "tokens": {}, "mode": "auto"}
    from datars.widget import ChartWidget

    page = (HARNESS.replace("__ESM__", json.dumps(ChartWidget._esm)).replace("__STATE__", json.dumps(state))
            .replace("__PATHS__", json.dumps(list(_runtime.FILES))))
    (out / "widget.html").write_text(page, encoding="utf-8")
    ok = True
    try:
        for name in ("standalone", "output"):
            r = check(f"{base}/{name}.html", out / f"{name}.png")
            live = bool(r["views"]) and all(v.get("renderer") for v in r["views"])
            ok &= live
            print(f"{name:10s} live={live} views={r['views']} logs={r['logs']}")
        # The widget: a click on the tallest bar (S, left) → a `pick` message with its row; then
        # "Python" sets state = "2" and the view steps to the pie.
        r = check(f"{base}/widget.html", out / "widget.png", "--click", "95,260",
                  "--eval", "JSON.stringify(window.__sent.filter(m => m.type === 'pick').map(m => m.row && m.row.fields))",
                  "--eval", "window.__model.set('state', '2'), new Promise(r => setTimeout(() => r(window.__model.get('state')), 1500))",
                  "--eval", "JSON.stringify(window.__sent.map(m => m.type))")
        live = bool(r["views"]) and all(v.get("renderer") for v in r["views"])
        picks = json.loads(r["evals"][0]) if r["evals"] and isinstance(r["evals"][0], str) else r["evals"][:1]
        picked = any(p and p.get("party") == "S" for p in picks)
        ok &= live and picked
        print(f"{'widget':10s} live={live} pick rows={picks} state after set={r['evals'][1] if len(r['evals']) > 1 else None} "
              f"messages={r['evals'][2] if len(r['evals']) > 2 else None} logs={r['logs']}")
    finally:
        srv.shutdown()
    print(f"screenshots in {out}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
