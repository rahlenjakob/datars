"""Charts, stories and layouts: documents built from data.

A :class:`Chart` is one datars document — the same IR JSON the TypeScript SDK writes — assembled
from std recipes (``@datars/std/plot`` + marks, ``pie``, ``map``, …). It displays itself in
notebooks, animates between states, and saves to PNG/SVG/PDF/MP4 through the ``datars`` CLI.

Charts are values: every method returns a new chart, so re-running a notebook cell never stacks
layers twice.

- ``a + b``      overlay: b's marks drawn in a's plot (bars and a target line)
- ``a | b``      side by side; ``a & b`` stacked — views share signals (highlight one, both follow)
- ``story(a, b)`` one step per chart, marks morphing between them by key
"""

from __future__ import annotations

import copy
import json
import re
from dataclasses import dataclass, field
from typing import Any, Iterable, Sequence

from . import std
from ._data import Data
from ._ir import Expr, e, lit
from ._node import Node, Registry, group, to_ir
from .theme import theme_ir

DEFAULT_SIZE = (720, 440)
PADDING = [16, 20, 12, 12]
#: Recipes whose marks read an enclosing plot's scales (they go inside ``plot``).
PLOT_MARKS = {"bar", "line", "area", "point", "dot", "cell", "grouped", "stacked", "pareto", "swarm",
              "waterfall", "rule", "span", "annotate", "axis", "grid",
              # financial marks (compose with a plot over a date column)
              "candlestick", "ohlc", "volume", "movingAverage", "bollinger", "indexed", "drawdown"}
#: Recipes with a ``selected`` keyset parameter (click to highlight; states can set it) — read
#: from the recipes' own descriptions, so a recipe that gains one takes part.
SELECTABLE = {name for name, spec in std.RECIPES.items() if "selected" in spec.get("params", {})}


def _slug(s: str | None) -> str:
    return re.sub(r"[^a-z0-9]+", "-", (s or "chart").lower()).strip("-")[:48] or "chart"


def _recipe_name(n: Node) -> str | None:
    return n.recipe.rsplit("/", 1)[-1] if n.recipe else None


@dataclass
class _Parts:
    """What one chart contributes to a document (charts combine in stories and layouts)."""

    main: Node
    overlays: list[Node] = field(default_factory=list)
    derived: dict[str, dict] = field(default_factory=dict)
    signals: dict[str, dict] = field(default_factory=dict)
    keys: dict[str, dict] = field(default_factory=dict)
    states: list[dict] = field(default_factory=list)
    motion: list[dict] = field(default_factory=list)
    title: Node | None = None


class Document:
    """Everything that displays and exports: charts, stories, layouts (see :meth:`to_dict`)."""

    _width: int
    _height: int

    # -- the document --

    def to_dict(self) -> dict:  # pragma: no cover - abstract
        raise NotImplementedError

    def to_json(self, indent: int | None = None) -> str:
        """The document as JSON — what ``datars render``, ``<datars-view>`` and every runtime read."""
        if indent is None:
            cached = getattr(self, "_json_cache", None)
            if cached is None:
                # Tables are serialized once (their JSON also names them): splice it in rather
                # than encoding a million values a second time.
                d = self.to_dict()
                sources = getattr(self, "_sources", {})
                marks = {}
                for name in d.get("data", {}):
                    if name in sources:
                        marks[name] = f"\x00datars-source:{name}\x00"
                        d["data"][name] = marks[name]
                cached = json.dumps(d, separators=(",", ":"), ensure_ascii=False, allow_nan=False)
                for name, m in marks.items():
                    cached = cached.replace(json.dumps(m), sources[name].source_json(), 1)
                self._json_cache = cached
            return cached
        return json.dumps(self.to_dict(), indent=indent, ensure_ascii=False, allow_nan=False)

    @property
    def size(self) -> tuple[int, int]:
        return (self._width, self._height)

    @property
    def state_names(self) -> list[str]:
        """The program's state names (what ``save(state=…)`` and widgets' ``state`` take)."""
        prog = self.to_dict().get("program") or {}
        return [s["name"] for s in prog.get("states", [])]

    # -- output (see _display / _cli) --

    def _repr_mimebundle_(self, include=None, exclude=None):
        from ._display import mimebundle

        return mimebundle(self)

    def save(self, path: str, *, state: int | str | None = None, dpr: float | None = None,
             mode: str | None = None, size: tuple[int, int] | None = None, fps: int | None = None,
             hold: float | None = None) -> str:
        """Save as ``.png``, ``.svg``, ``.pdf`` (one state), ``.mp4`` (the whole program as a film,
        with WebVTT captions), ``.datars`` (a bundle), ``.json`` or ``.html`` (a standalone page).

        Rendering goes through the ``datars`` CLI (``DATARS_CLI`` or ``datars`` on PATH).
        """
        from . import _cli, _display

        low = str(path).lower()
        if low.endswith(".json"):
            with open(path, "w", encoding="utf-8") as f:
                f.write(self.to_json(indent=2))
            return str(path)
        if low.endswith(".html"):
            return _display.write_html(self, path)
        return _cli.save(self, str(path), state=state, dpr=dpr, mode=mode, size=size, fps=fps, hold=hold)

    def to_html(self, path: str | None = None, *, runtime: str | None = None) -> str:
        """A standalone HTML page with the live chart (``runtime``: ``"inline"`` — offline, the
        runtime embedded, ~7 MB — or a URL; default: inline when the runtime is installed with
        the package, else the CDN). Returns the HTML, or the path when ``path`` is given."""
        from . import _display

        html = _display.page(self, runtime=runtime)
        if path is None:
            return html
        with open(path, "w", encoding="utf-8") as f:
            f.write(html)
        return str(path)

    def show(self) -> None:
        """Display in a notebook; from a script, open the live chart in a browser (like ``plt.show()``)."""
        from ._display import show

        show(self)

    def widget(self):
        """An interactive widget (``pip install "datars[widget]"``): set ``state``, signals and
        tokens from Python; get clicks and state changes back (see :class:`datars.widget.ChartWidget`)."""
        from .widget import ChartWidget

        return ChartWidget(self)

    def image(self, format: str = "png", **kw) -> bytes:
        """Rendered bytes (``png``, ``svg``, ``pdf``) of one state, through the CLI."""
        from . import _cli

        return _cli.render(self.to_json(), format, **kw)

    def publish(self, to: str, *, alias: str | None = None) -> str:
        """Publish to a static delivery folder (``c/<alias>`` + ``chunks/``, any static host);
        returns the ``<datars-view>`` embed snippet."""
        from . import _cli

        return _cli.publish(self, to, alias=alias or _slug(self.to_dict().get("title")))

    def check(self) -> list[str]:
        """The engine's diagnostics for this document (``datars check``; empty when fine)."""
        from . import _cli

        return _cli.check(self.to_json())

    # -- composition --

    def __or__(self, other: "Document") -> "Layout":
        return Layout([self, other], direction="columns")

    def __and__(self, other: "Document") -> "Layout":
        return Layout([self, other], direction="rows")

    def _charts(self) -> list["Chart"]:  # pragma: no cover - abstract
        raise NotImplementedError


class Chart(Document):
    """One chart: a plot (with marks) or a standalone recipe, its data, states and look.

    Build one with the functions in :mod:`datars` (``bar``, ``line``, ``scatter``, ``pie``,
    ``map``, …) or from any std node with :func:`chart`. Methods return new charts.
    """

    def __init__(self) -> None:
        self._plot: dict[str, Any] | None = None
        self._marks: list[Node] = []
        self._main: Node | None = None
        self._facet: dict[str, Any] | None = None
        self._overlays: list[Node] = []
        self._derived: dict[str, dict] = {}
        self._signals: dict[str, dict] = {}
        self._keys: dict[str, dict] = {}
        self._states: list[dict] = []
        self._frame: dict[str, Any] | None = None
        self._program: dict[str, Any] = {}
        self._motion: list[dict] = []
        self._title: dict[str, Any] = {}
        self._theme: Any = None
        self._width, self._height = DEFAULT_SIZE
        self._description: str | None = None
        self._locale = "en"
        self._narration: dict[str, str] | None = None
        self._labels: dict[str, str] = {}

    def _clone(self) -> "Chart":
        c = copy.copy(self)
        for k in ("_marks", "_overlays", "_states"):
            setattr(c, k, list(getattr(self, k)))
        for k in ("_derived", "_signals", "_keys", "_program", "_title", "_labels"):
            setattr(c, k, dict(getattr(self, k)))
        c._motion = list(self._motion)
        if self._plot is not None:
            c._plot = dict(self._plot)
        c._json_cache = None
        return c

    def _charts(self) -> list["Chart"]:
        return [self]

    # -- look and labels --

    def update(self, *, title: str | None = None, subtitle: str | None = None, source: str | None = None,
               width: int | None = None, height: int | None = None, theme: Any = None,
               description: str | None = None, locale: str | None = None, **plot: Any) -> "Chart":
        """Change the title, size, theme, or any ``plot`` parameter (``legend``, ``x_label``,
        ``format``, ``prefix``, ``y_domain``, ``zero``, ``grid``, ``axes`` …, snake_case)."""
        c = self._clone()
        if title is not None:
            c._title["text"] = title
        if subtitle is not None:
            c._title["subtitle"] = subtitle
        if source is not None:
            c._title["source"] = source
        if width is not None:
            c._width = int(width)
        if height is not None:
            c._height = int(height)
        if theme is not None:
            c._theme = theme
        if description is not None:
            c._description = description
        if locale is not None:
            c._locale = locale
        if plot:
            if c._plot is None:
                raise TypeError(f"datars: {', '.join(plot)} apply to cartesian charts (plots) only")
            renames = std.RENAMES["plot"]
            for k, v in plot.items():
                name = renames.get(k, k)
                if name not in std.RECIPES["plot"]["params"]:
                    raise TypeError(f"datars: plot has no parameter {k!r}")
                if v is None:
                    c._plot.pop(name, None)
                else:
                    c._plot[name] = v
        return c

    def title(self, text: str, subtitle: str | None = None, source: str | None = None) -> "Chart":
        """Set the title (and subtitle / source line)."""
        return self.update(title=text, subtitle=subtitle, source=source)

    def properties(self, **kw: Any) -> "Chart":
        """Alias of :meth:`update` (for Altair hands)."""
        return self.update(**kw)

    def colors(self, mapping: dict[Any, str]) -> "Chart":
        """Fixed colours per category (``{"S": "#e8112d"}``) — key metadata, so they hold across
        every state, story step and chart that shows the same keys."""
        c = self._clone()
        for k, v in mapping.items():
            c._keys[str(k)] = {**c._keys.get(str(k), {}), "color": v}
        return c

    def names(self, mapping: dict[Any, str]) -> "Chart":
        """Display names per category (``{"S": "Social Democrats"}``): axis labels, legends, tooltips."""
        c = self._clone()
        for k, v in mapping.items():
            c._keys[str(k)] = {**c._keys.get(str(k), {}), "name": v}
        return c

    # -- layers --

    def add(self, *nodes: Node) -> "Chart":
        """Add std nodes: marks and annotations inside the plot (``dr.std.rule(...)``), others over
        the chart."""
        c = self._clone()
        for n in nodes:
            if c._plot is not None and _recipe_name(n) in PLOT_MARKS:
                c._marks.append(n)
            else:
                c._overlays.append(n)
        return c

    def __add__(self, other: "Chart") -> "Chart":
        if not isinstance(other, Chart):
            return NotImplemented
        if self._plot is None or other._plot is None:
            raise TypeError("datars: `+` overlays cartesian charts (plots); use `|`, `&` or dr.story for others")
        c = self._clone()
        a, b = self._plot, other._plot
        same = b.get("data") is a.get("data") or (isinstance(b.get("data"), Data) and isinstance(a.get("data"), Data) and b["data"].name == a["data"].name)
        for m in other._marks:
            if _recipe_name(m) in ("rule", "span", "annotate"):
                c._marks.append(m)
                continue
            inherit = {k: b.get(k) for k in ("data", "x", "y", "color") if b.get(k) is not None and (not same or b.get(k) != a.get(k))}
            params = std.RECIPES.get(_recipe_name(m) or "", {}).get("params", {})
            m = m.set(**{k: v for k, v in inherit.items() if k in params and k not in m.params})
            # An uncoloured layer over another takes the palette's next colour, so the two read apart.
            if not b.get("color"):
                ink = "$categorical[1]"
                if "stroke" in params and "stroke" not in m.params:
                    m = m.set(stroke=ink)
                elif "fill" in params and "fill" not in m.params:
                    m = m.set(fill=ink)
            if _recipe_name(m) == "line" and a.get("xType", "band") in ("band", "point") and "curve" not in m.params:
                m = m.set(curve="linear")
            c._marks.append(m)
        # One set of scales for both: domains that cover both charts' values.
        if not same:
            for axis in ("x", "y"):
                t = a.get(f"{axis}Type", "band" if axis == "x" else "linear")
                if a.get(f"{axis}Domain") is not None:
                    continue
                dom = _union_domain(a, b, axis, t)
                if dom is not None:
                    c._plot[f"{axis}Domain"] = dom
        c._keys.update({k: {**other._keys[k], **c._keys.get(k, {})} for k in other._keys})
        c._signals.update({k: v for k, v in other._signals.items() if k not in c._signals})
        c._derived.update(other._derived)
        c._overlays += other._overlays
        if not c._states and other._states:
            c._states, c._frame, c._program = list(other._states), other._frame, dict(other._program)
        return c

    def rule(self, *, y: Any = None, x: Any = None, label: str | None = None, dashed: bool = True,
             ink: str | None = None) -> "Chart":
        """A reference line at a data value: ``.rule(y=2, label="Target 2%")``."""
        self._need_plot("rule")
        axis, value = ("y", y) if y is not None else ("x", x)
        return self.add(std.rule(axis=axis, value=self._value(axis, value), label=label, dashed=dashed, ink=ink))

    def span(self, *, x: tuple[Any, Any] | None = None, y: tuple[Any, Any] | None = None,
             label: str | None = None, ink: str | None = None, opacity: float | None = None) -> "Chart":
        """A shaded band between two data values: ``.span(x=(2008, 2009), label="Crisis")``."""
        self._need_plot("span")
        axis, (lo, hi) = ("x", x) if x is not None else ("y", y)
        return self.add(std.span(axis=axis, from_=self._value(axis, lo), to=self._value(axis, hi), label=label, ink=ink,
                                 opacity=opacity))

    def annotate(self, text: str, *, x: Any, y: Any, dx: float | None = None, dy: float | None = None,
                 connector: str | None = None, **kw: Any) -> "Chart":
        """A callout at a data point: ``.annotate("2022 spike", x=2022, y=7.7)``."""
        self._need_plot("annotate")
        return self.add(std.annotate(text=text, x=self._pos("x", x), y=self._pos("y", y), dx=dx, dy=dy,
                                     connector=connector, **kw))

    def _pos(self, axis: str, v: Any) -> Any:
        if isinstance(v, Expr):
            return v
        t = (self._plot or {}).get(f"{axis}Type", "band" if axis == "x" else "linear")
        centre = f" + scale.{axis}.bandwidth() / 2" if t in ("band", "point") else ""
        return e(f"scale.{axis}({lit(self._value(axis, v))}){centre}")

    def _value(self, axis: str, v: Any) -> Any:
        """A data value as the axis's scale takes it: dates on a time axis are days since 1970."""
        if (self._plot or {}).get(f"{axis}Type") != "time" or isinstance(v, (int, float, Expr)) or v is None:
            return v
        return _days(v)

    def _need_plot(self, what: str) -> None:
        if self._plot is None:
            raise TypeError(f"datars: .{what}() needs a cartesian chart (a plot)")

    def facet(self, by: str, *, columns: int = 3, shared: bool = True, gap: float | None = None) -> "Chart":
        """Small multiples: this chart once per value of ``by`` (scales shared unless ``shared=False``)."""
        self._need_plot("facet")
        if self._frame:
            raise NotImplementedError("datars: facets of an animated (frame=) chart aren't supported yet")
        c = self._clone()
        d = c._plot["data"]
        if isinstance(d, Data) and not d.has(by):
            raise KeyError(f"datars: {by!r} isn't in the chart's data — charts keep only the columns they use; "
                           f"pass facet={by!r} when building it (dr.line(df, ..., facet={by!r}))")
        c._facet = {"by": d.col(by) if isinstance(d, Data) else by, "columns": columns, "shared": shared, "gap": gap}
        # Lines labelled at their ends derive a table from their rows, which a panel's group of
        # rows can't do yet (std): panels use the legend instead.
        if any(_recipe_name(m) == "line" and m.params.get("labels") for m in c._marks):
            c._marks = [m.set(labels=None) if _recipe_name(m) == "line" else m for m in c._marks]
            if c._plot.get("color"):
                c._plot["legend"] = True
        return c

    # -- states and animation --

    def step(self, name: str | None = None, *, title: str | None = None, text: str | None = None,
             highlight: Iterable[Any] | None = None, frame: Any = None, set: dict[str, Any] | None = None,
             hold: float | None = None) -> "Chart":
        """Add a program state: a story step. ``highlight`` keeps some categories strong (the
        rest recede), ``frame`` picks an animation frame, ``set`` sets any signal; ``title`` /
        ``text`` narrate it in a card drawn by the engine (in video and PNG too).

        >>> import datars as dr
        >>> c = dr.bar({"k": ["a", "b"], "v": [1, 2]}, x="k", y="v")
        >>> c.step("all").step("b", highlight=["b"], text="b leads").state_names
        ['all', 'b']
        """
        c = self._clone()
        if c._frame and not c._states_user:
            c._states = []  # explicit steps replace the one-per-frame default
        c._states_user = True
        sets: dict[str, Any] = {}
        if highlight is not None:
            c._ensure_highlight()
            sets["highlight"] = [str(h) for h in highlight]
        elif "highlight" in c._signals:
            sets["highlight"] = []
        if frame is not None:
            if not c._frame:
                raise TypeError("datars: frame= needs an animated chart (pass frame=<column> when building it)")
            sets[c._frame["signal"]] = frame
        if set:
            sets.update(set)
        st: dict[str, Any] = {"name": str(name if name is not None else (frame if frame is not None else len(c._states) + 1))}
        if sets:
            st["set"] = sets
        if title or text:
            st["narration"] = {k: v for k, v in (("title", title), ("text", text)) if v}
        if hold is not None:
            st["hold"] = hold
        c._states.append(st)
        return c

    _states_user = False

    def states(self, spec: dict[str, dict[str, Any]] | Sequence[dict[str, Any]]) -> "Chart":
        """Several steps at once: ``{"All": {}, "Leaders": {"highlight": ["S"], "text": "…"}}``."""
        c = self
        items = spec.items() if isinstance(spec, dict) else ((s.pop("name", None), s) for s in map(dict, spec))
        for name, kw in items:
            c = c.step(name, **kw)
        return c

    def narrate(self, text: str | None = None, title: str | None = None) -> "Chart":
        """Narration for a chart without steps (its story step's card, in :func:`story`)."""
        c = self._clone()
        c._narration = {k: v for k, v in (("title", title), ("text", text)) if v}
        return c

    def highlight(self, *categories: Any) -> "Chart":
        """Highlight categories from the start (clicks toggle more): ``.highlight("S", "SD")``."""
        c = self._clone()
        c._ensure_highlight()
        c._signals["highlight"] = {"type": "keyset", "default": [str(x) for x in categories]}
        return c

    def _ensure_highlight(self) -> None:
        if "highlight" not in self._signals:
            self._signals["highlight"] = {"type": "keyset", "default": []}
        self._marks = [m.set(selected="highlight") if _recipe_name(m) in SELECTABLE and "selected" not in m.params else m for m in self._marks]
        if self._main is not None and _recipe_name(self._main) in SELECTABLE and "selected" not in self._main.params:
            self._main = self._main.set(selected="highlight")

    def autoplay(self, *, hold: float | None = None, loop: bool = False) -> "Chart":
        """Play the steps by themselves (each held ``hold`` seconds); hosts pause off screen and
        for reduced motion."""
        c = self._clone()
        c._program = {**c._program, "drivers": "autoplay", "loop": loop}
        if hold is not None:
            c._program["hold"] = hold
        return c

    def scrolly(self) -> "Chart":
        """Scroll-driven steps (``<datars-view scrub>`` on a page)."""
        c = self._clone()
        c._program = {**c._program, "drivers": "scrolly"}
        return c

    def motion(self, *, duration: float | None = None, easing: str | None = None, stagger: float | None = None,
               rules: list[dict] | None = None) -> "Chart":
        """How states transition: ``duration`` (s), ``easing`` (``"cubic-in-out"``,
        ``"spring(170, 26)"`` …), ``stagger`` (0–1 spread), or raw motion ``rules`` (docs/05)."""
        c = self._clone()
        rule: dict[str, Any] = {}
        if duration is not None:
            rule["duration"] = duration
        if easing is not None:
            rule["easing"] = easing
        if stagger is not None:
            rule["choreo"] = {"type": "stagger", "order": "data", "spread": stagger}
        if rule:
            c._motion.append({"select": {"role": "datum"}, **rule})
        c._motion += list(rules or [])
        return c

    def signal(self, name: str, default: Any, *, type: str | None = None) -> "Chart":
        """Declare a signal (read in ``dr.e(...)`` expressions, set by steps or widgets)."""
        c = self._clone()
        t = type or ("bool" if isinstance(default, bool) else "num" if isinstance(default, (int, float)) else "keyset" if isinstance(default, (list, tuple)) else "str")
        c._signals[name] = {"type": t, "default": list(default) if isinstance(default, tuple) else default}
        return c

    # -- assembly --

    def _plot_node(self) -> Node:
        assert self._plot is not None
        params = dict(self._plot)
        titled = bool(self._title.get("text")) and not self._facet
        if titled:
            params.setdefault("title", self._title["text"])
            if self._title.get("subtitle"):
                params.setdefault("subtitle", self._title["subtitle"])
        # A y-axis title sits above the axis — where a plot title is. Under a title, say it in the
        # subtitle line instead (the newsroom convention: "life expectancy, years").
        # In small multiples the panel's name sits there: the facet's title line says it.
        ylab = params.pop("yLabel", None) if (titled or params.get("title") or self._facet) else None
        if ylab and not params.get("subtitle") and not self._facet:
            params["subtitle"] = ylab
        params["children"] = list(self._marks)
        return Node({"kind": "use", "recipe": "@datars/std/plot", "params": params})

    def _parts(self) -> _Parts:
        title_node = None
        t = self._title
        if self._plot is not None and not self._facet:
            main = self._plot_node()
            if t.get("source"):
                title_node = None  # the plot draws title/subtitle; a source line goes below
        elif self._plot is not None and self._facet:
            f = self._facet
            main = std.facet(data=self._plot["data"], by=f["by"], columns=f["columns"], shared=f["shared"], gap=f["gap"],
                             chart=self._plot_node())
            ylab = self._plot.get("yLabel")
            if t.get("text") or ylab:
                title_node = std.title(text=t.get("text") or ylab, subtitle=t.get("subtitle") or (ylab if t.get("text") else None),
                                       source=t.get("source"))
        else:
            if self._main is None:
                raise ValueError("datars: an empty chart")
            main = self._main
            if t.get("text"):
                title_node = std.title(text=t["text"], subtitle=t.get("subtitle"), source=t.get("source"))
        states = list(self._states)
        if self._narration and not states:
            states = [{"name": self._title.get("text") or "1", "narration": dict(self._narration), "_default": True}]
        motion = list(self._motion)
        return _Parts(main=main, overlays=list(self._overlays), derived=dict(self._derived), signals=dict(self._signals),
                      keys=dict(self._keys), states=states, motion=motion, title=title_node)

    def to_dict(self) -> dict:
        """The document (IR JSON): data, derived tables, signals, scene, motion and program."""
        reg = Registry()
        p = self._parts()
        body = p.main.opts(key="chart")
        children = [*[o for o in p.overlays if o.fields.get("z", 0) < 0], body, *[o for o in p.overlays if o.fields.get("z", 0) >= 0]]
        if any("narration" in s for s in p.states):
            children.append(_card())
        if p.title is not None:
            scene = group([p.title.opts(key="title", size={"h": "auto"}), group(children, key="content", layout={"type": "stack"})],
                          key="root", layout={"type": "rows", "gap": 10, "padding": PADDING})
        else:
            scene = group(children, key="root", layout={"type": "stack", "padding": PADDING})
        return _assemble(self, reg, scene, p.derived, p.signals, p.keys, p.states, p.motion, self._program)


def _days(v: Any) -> int:
    """Days since 1970-01-01 (the engine's date values) of a date, datetime, Timestamp or ISO string."""
    import datetime as _dt

    if hasattr(v, "to_pydatetime"):  # pandas Timestamp
        v = v.to_pydatetime()
    if hasattr(v, "astype") and str(getattr(v, "dtype", "")).startswith("datetime64"):  # numpy datetime64
        return int(v.astype("datetime64[D]").astype("int64"))
    if isinstance(v, str):
        v = _dt.date.fromisoformat(v[:10])
    if isinstance(v, _dt.datetime):
        v = v.date()
    if isinstance(v, _dt.date):
        return (v - _dt.date(1970, 1, 1)).days
    raise TypeError(f"datars: {v!r} isn't a date")


def _card() -> Node:
    """One engine-drawn card narrating every step (its title and text)."""
    return std.card(title=e("narration.title"), text=e("narration.text"), width=260).opts(key="caption")


def _union_domain(a: dict, b: dict, axis: str, scale_type: str):
    da, db = a.get("data"), b.get("data")
    fa, fb = a.get(axis), b.get(axis)
    if not (isinstance(da, Data) and isinstance(db, Data) and fa and fb and da.has(fa) and db.has(fb)):
        return None
    if scale_type in ("band", "point"):
        seen = dict.fromkeys(da.unique(fa))
        seen.update(dict.fromkeys(db.unique(fb)))
        return list(seen)
    ea, eb = da.extent(fa), db.extent(fb)
    if not ea or not eb or not all(isinstance(v, (int, float)) for v in (*ea, *eb)):
        return None
    lo, hi = min(ea[0], eb[0]), max(ea[1], eb[1])
    if axis == "y" and a.get("zero", True) and scale_type == "linear":
        lo, hi = min(lo, 0), max(hi, 0)
    return {"values": [lo, hi]}  # (a bare list would be exact: no nice rounding)


def _program(states: list[dict], opts: dict[str, Any]) -> dict | None:
    if not states:
        return None
    drivers_kind = opts.get("drivers", "story")
    hold = opts.get("hold")
    out_states = []
    for s in states:
        s = {k: v for k, v in s.items() if not k.startswith("_")}
        if hold is not None and "hold" not in s:
            s["hold"] = hold
        out_states.append(s)
    prog: dict[str, Any] = {"preset": "story", "states": out_states}
    if drivers_kind == "autoplay":
        prog["drivers"] = ["autoplay", "steps", "keys"]
    elif drivers_kind == "scrolly":
        prog["drivers"] = [{"scroll": "scrub"}, "keys"]
    else:
        prog["drivers"] = ["steps", "keys"]
    if opts.get("loop") and len(out_states) > 1:
        prog["edges"] = [{"from": out_states[-1]["name"], "on": "next", "to": out_states[0]["name"]}]
    return prog


def _assemble(doc: Document, reg: Registry, scene: Node, derived: dict, signals: dict, keys: dict,
              states: list[dict], motion: list[dict], program_opts: dict) -> dict:
    """The document dict: tables registered while the scene converts, in a stable order."""
    scene_ir = to_ir(scene, reg)
    tables = {}
    for name, d in derived.items():
        tables[name] = {"from": to_ir(d["from"], reg), "ops": to_ir(d["ops"], reg)}
    doc._sources = dict(reg.sources)  # for to_json's splice
    title = getattr(doc, "_title", {}).get("text") if isinstance(doc, Chart) else getattr(doc, "_title_text", None)
    rules = list(motion)
    if states and not any(r.get("matcher") for r in rules):
        # Marks pair by their own key wherever they sit (a bar becomes its slice, a region its bar).
        rules = [{"select": {"role": "datum"}, "matcher": "by-key"}, {"select": {"role": "region"}, "matcher": "by-key"}] + rules
    out: dict[str, Any] = {
        "datars": 1,
        "id": _slug(title),
        "title": title,
        "description": getattr(doc, "_description", None),
        "size": {"width": doc._width, "height": doc._height},
        "theme": theme_ir(getattr(doc, "_theme", None)),
        "locale": getattr(doc, "_locale", "en"),
        "data": {name: d.source() for name, d in reg.sources.items()},
        "tables": tables or None,
        "signals": signals or None,
        "keys": keys or None,
        "scene": scene_ir,
        "motion": {"rules": rules} if rules else None,
        "program": _program(states, program_opts),
    }
    return {k: v for k, v in out.items() if v is not None}


# ---- stories and layouts ---------------------------------------------------------------------------


def _merge(target: _Parts, p: _Parts) -> None:
    for k, v in p.derived.items():
        target.derived.setdefault(k, v)
    for k, v in p.signals.items():
        target.signals.setdefault(k, v)
    for k, v in p.keys.items():
        target.keys[k] = {**v, **target.keys.get(k, {})}
    target.motion += [r for r in p.motion if r not in target.motion]


class Story(Document):
    """Charts as the steps of one story: each step shows one chart (all its own steps, if it has
    any) and marks that share a key morph from chart to chart — a bar into its slice, a region
    into its bar. See :func:`story`."""

    def __init__(self, charts: list[Chart], *, autoplay: bool = False, loop: bool = False, hold: float | None = None,
                 scrolly: bool = False, title: str | None = None, size: tuple[int, int] | None = None, theme: Any = None,
                 duration: float | None = None):
        if not charts:
            raise ValueError("datars: a story needs at least one chart")
        self.charts = charts
        first = charts[0]
        self._width, self._height = size or first.size
        self._theme = theme if theme is not None else first._theme
        self._title_text = title or first._title.get("text")
        self._description = first._description
        self._locale = first._locale
        self._program = {"drivers": "scrolly" if scrolly else "autoplay" if autoplay else "story", "loop": loop}
        if hold is not None:
            self._program["hold"] = hold
        self._duration = duration

    def _charts(self) -> list[Chart]:
        return list(self.charts)

    def to_dict(self) -> dict:
        reg = Registry()
        acc = _Parts(main=group([]))
        children: list[Node] = []
        states: list[dict] = []
        names: set[str] = set()
        narrated = False
        for i, c in enumerate(self.charts):
            p = c._parts()
            _merge(acc, p)
            when = e(f'view == "{i}"')
            body = p.main
            if p.title is not None:
                body = group([p.title.opts(key="title", size={"h": "auto"}), body.opts(key="body")], layout={"type": "rows", "gap": 10})
            children.append(body.opts(key="chart", when=when))
            children += [o.opts(when=when) for o in p.overlays]
            own = p.states or [{"name": str(i + 1), "_default": True}]
            for s in own:
                s = dict(s)
                if s.pop("_default", False):
                    s["name"] = str(i + 1)
                name, n = str(s["name"]), 2
                while name in names:
                    name = f"{s['name']} ({n})"
                    n += 1
                names.add(name)
                s["name"] = name
                s["set"] = {**s.get("set", {}), "view": str(i)}
                narrated = narrated or "narration" in s
                states.append(s)
        acc.signals["view"] = {"type": "str", "default": "0"}
        if narrated:
            children.append(_card())
        motion = acc.motion
        if self._duration is not None:
            motion = motion + [{"select": {"role": "datum"}, "duration": self._duration}]
        scene = group(children, key="root", layout={"type": "stack", "padding": PADDING})
        return _assemble(self, reg, scene, acc.derived, acc.signals, acc.keys, states, motion, self._program)


class Layout(Document):
    """Charts side by side (``a | b``) or stacked (``a & b``) in one document. Signals are
    shared, so a highlight in one view shows in every view with the same keys."""

    def __init__(self, charts: list[Document], *, direction: str = "columns", gap: float = 24,
                 title: str | None = None, size: tuple[int, int] | None = None, theme: Any = None):
        flat: list[Chart] = []
        for c in charts:
            if isinstance(c, Layout) and c.direction == direction:
                flat += c.charts
            elif isinstance(c, Chart):
                flat.append(c)
            else:
                raise TypeError("datars: layouts combine charts (not stories)")
        self.charts, self.direction, self.gap = flat, direction, gap
        w = [c.size[0] for c in flat]
        h = [c.size[1] for c in flat]
        if size:
            self._width, self._height = size
        elif direction == "columns":
            self._width, self._height = min(sum(w), 1400), max(h)
        else:
            self._width, self._height = max(w), min(sum(h), 1600)
        self._theme = theme if theme is not None else flat[0]._theme
        self._title_text = title
        self._description = None
        self._locale = flat[0]._locale

    def _charts(self) -> list[Chart]:
        return list(self.charts)

    def __or__(self, other: Document) -> "Layout":
        return Layout([self, other], direction="columns") if self.direction == "columns" else Layout([self, other])

    def __and__(self, other: Document) -> "Layout":
        return Layout([self, other], direction="rows") if self.direction == "rows" else Layout([self, other], direction="rows")

    def to_dict(self) -> dict:
        reg = Registry()
        acc = _Parts(main=group([]))
        children: list[Node] = []
        states: list[dict] = []
        total = sum(c.size[0 if self.direction == "columns" else 1] for c in self.charts)
        for i, c in enumerate(self.charts):
            p = c._parts()
            _merge(acc, p)
            share = c.size[0 if self.direction == "columns" else 1] / total
            inner = [p.main.opts(key="chart"), *p.overlays]
            if p.title is not None:
                panel = group([p.title.opts(key="title", size={"h": "auto"}), group(inner, key="content", layout={"type": "stack"})],
                              layout={"type": "rows", "gap": 8})
            else:
                panel = group(inner, layout={"type": "stack"})
            sz = {"w": f"{share * 100:.1f}%"} if self.direction == "columns" else {"h": f"{share * 100:.1f}%"}
            children.append(panel.opts(key=f"view{i}", size=sz))
            if not states and p.states:
                states = p.states
        if any("narration" in s for s in states):
            children.append(_card())
        body = group(children, key="views", layout={"type": self.direction, "gap": self.gap})
        if self._title_text:
            scene = group([std.title(text=self._title_text).opts(key="title", size={"h": "auto"}), body], key="root",
                          layout={"type": "rows", "gap": 10, "padding": PADDING})
        else:
            scene = group([body], key="root", layout={"type": "stack", "padding": PADDING})
        return _assemble(self, reg, scene, acc.derived, acc.signals, acc.keys, states, acc.motion, {})


def story(*items: Any, autoplay: bool = False, loop: bool = False, hold: float | None = None, scrolly: bool = False,
          title: str | None = None, size: tuple[int, int] | None = None, theme: Any = None,
          duration: float | None = None) -> Story:
    """A story: one step per chart (or ``(chart, "narration")`` pair), morphing between them.

    >>> import datars as dr
    >>> df = {"party": ["S", "SD", "M"], "share": [30.3, 20.5, 19.1]}
    >>> s = dr.story(dr.bar(df, x="party", y="share"), (dr.pie(df, names="party", values="share"), "Bars become slices."))
    >>> s.state_names
    ['1', '2']
    """
    charts: list[Chart] = []
    for it in items:
        if isinstance(it, tuple):
            ch, *rest = it
            text_ = rest[0] if rest else None
            title_ = rest[1] if len(rest) > 1 else None
            it = ch.narrate(text_, title_)
        if isinstance(it, list):
            charts += it
            continue
        if isinstance(it, Node):
            it = chart(it)
        if not isinstance(it, Chart):
            raise TypeError(f"datars: a story step is a chart, not {type(it).__name__}")
        charts.append(it)
    return Story(charts, autoplay=autoplay, loop=loop, hold=hold, scrolly=scrolly, title=title, size=size, theme=theme,
                 duration=duration)


def chart(node: Node | Chart, *, title: str | None = None, width: int | None = None, height: int | None = None,
          theme: Any = None, **plot: Any) -> Chart:
    """A chart around any std node: ``dr.chart(dr.std.sankey(data=flows, source="a", target="b",
    value="n"))``. A plot node keeps its marks; standalone recipes draw as they are."""
    if isinstance(node, Chart):
        return node.update(title=title, width=width, height=height, theme=theme, **plot)
    c = Chart()
    if _recipe_name(node) == "plot":
        c._plot = {k: v for k, v in node.params.items() if k != "children"}
        c._marks = list(node.params.get("children", []))
    elif _recipe_name(node) in PLOT_MARKS:
        params = node.params
        c._plot = {k: params[k] for k in ("data", "x", "y", "color") if k in params}
        for k in ("xType", "yType"):
            if k in params:
                c._plot[k] = params[k]
        c._marks = [node]
    else:
        c._main = node
    return c.update(title=title, width=width, height=height, theme=theme, **plot)
