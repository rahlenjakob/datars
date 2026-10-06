"""The chart as a Jupyter widget (optional: ``pip install "datars[widget]"`` — anywidget).

Two-way, over the kernel connection:

- Python → chart: assign ``w.chart = other_chart`` (the view morphs to it — keys pair marks),
  ``w.state = "2024"`` (go to a step), ``w.signals = {"highlight": ["SE"]}``, ``w.tokens =
  {"accent": "#e4572e"}``, ``w.mode = "dark"``; ``w.next()`` / ``w.prev()``.
- chart → Python: ``w.state`` follows the reader's steps (observe it with ``w.observe``), and
  ``@w.on_click`` gets every click: what was hit and the data row behind it.

The web runtime (≈6 MB) is sent over the comm the first time a page needs it and never stored
in widget state, so saved notebooks stay small and it works offline and on remote kernels.

    w = dr.bar(df, x="country", y="gdp").widget()
    @w.on_click
    def picked(event):
        print(event["row"])        # {"country": "SE", "gdp": 593}
    w
"""

from __future__ import annotations

from pathlib import Path
from typing import Any, Callable

try:
    import anywidget
    import traitlets
except ImportError as err:  # pragma: no cover - exercised without the extra
    raise ImportError('datars: widgets need anywidget — pip install "datars[widget]"') from err

from . import _runtime

_STATIC = Path(__file__).resolve().parent / "_static"


def _esm() -> str:
    return (_STATIC / "loader.js").read_text(encoding="utf-8") + "\n" + (_STATIC / "widget.js").read_text(encoding="utf-8")


class ChartWidget(anywidget.AnyWidget):
    """A live chart bound to Python (see the module docs)."""

    _esm = _esm()
    _css = ".datars-widget { width: 100%; } .datars-widget .datars-output { margin: 0; }"

    #: The document's JSON as UTF-8 bytes: a binary buffer on the comm, not a string inside JSON
    #: (a million-row document would be escaped a second time).
    doc = traitlets.Bytes(b"").tag(sync=True)
    poster = traitlets.Unicode("").tag(sync=True)
    alt = traitlets.Unicode("").tag(sync=True)
    width = traitlets.Int(720).tag(sync=True)
    height = traitlets.Int(440).tag(sync=True)
    state = traitlets.Unicode("").tag(sync=True)
    states = traitlets.List(traitlets.Unicode()).tag(sync=True)
    signals = traitlets.Dict().tag(sync=True)
    tokens = traitlets.Dict().tag(sync=True)
    mode = traitlets.Unicode("auto").tag(sync=True)

    def __init__(self, chart: Any = None, *, poster: str | None = None, state: str | None = None, **kw: Any):
        from ._display import options

        self._click_handlers: list[Callable[[dict], Any]] = []
        self._error: str | None = None
        self._chart = chart
        init: dict[str, Any] = {"mode": options.mode}
        if chart is not None:
            w, h = chart.size
            d = chart.to_dict()
            init.update(doc=chart.to_json().encode("utf-8"), width=w, height=h, alt=d.get("title") or "chart",
                        states=[s["name"] for s in (d.get("program") or {}).get("states", [])])
        if poster is None and chart is not None:
            # The static preview shows while the runtime arrives — and where no kernel answers
            # (a saved notebook's widget state rendered by nbconvert or nbviewer).
            from ._display import poster_src, static_images

            poster = poster_src(static_images(chart))
        if poster:
            init["poster"] = poster
        if state is not None:
            init["state"] = str(state)
        init.update(kw)
        super().__init__(**init)
        self.on_msg(self._on_msg)

    # -- Python → chart --

    @property
    def chart(self):
        """The chart shown; assign another to morph the view to it."""
        return self._chart

    @chart.setter
    def chart(self, chart) -> None:
        self._chart = chart
        d = chart.to_dict()
        self.states = [s["name"] for s in (d.get("program") or {}).get("states", [])]
        self.doc = chart.to_json().encode("utf-8")

    def next(self) -> None:
        """Next step (as the → key)."""
        self.send({"type": "event", "name": "next"})

    def prev(self) -> None:
        """Previous step."""
        self.send({"type": "event", "name": "prev"})

    def set_signal(self, name: str, value: Any) -> None:
        """Set one signal (``highlight``, a frame, a slider's value…)."""
        self.signals = {**self.signals, name: value}

    # -- chart → Python --

    def on_click(self, handler: Callable[[dict], Any]) -> Callable[[dict], Any]:
        """Call ``handler(event)`` on every click: ``{"x", "y", "hits": [{path, role, label, …}],
        "row": {"table", "index", "fields"} | None}``. Usable as a decorator."""
        self._click_handlers.append(handler)
        return handler

    def _on_msg(self, _widget, content: dict, buffers) -> None:
        kind = content.get("type")
        if kind == "need-runtime":
            try:
                files = _runtime.files()
            except FileNotFoundError as err:
                self.send({"type": "runtime", "error": str(err)})
                return
            self.send({"type": "runtime", "paths": list(files)}, buffers=list(files.values()))
        elif kind == "pick":
            event = {k: v for k, v in content.items() if k != "type"}
            row = event.get("row")
            if isinstance(row, dict) and "fields" in row:
                event["fields"] = row["fields"]
            for h in list(self._click_handlers):
                h(event)
        elif kind == "error":
            self._error = content.get("message")
