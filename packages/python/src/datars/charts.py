"""The pandas-first chart functions: ``dr.bar(df, x=…, y=…, color=…)`` and friends.

Each builds a :class:`~datars.chart.Chart` from std recipes (``plot`` + ``bar``/``line``/``point``
…, or ``pie``, ``treemap``, ``map``, ``cloud``), inferring what a data scientist would otherwise
spell out: scale types from dtypes (numbers → linear, dates → time, text → band), keys (what
morphs into what between states), orientation, stacking, legends vs. end labels.

``frame=<column>`` animates any of them over that column, like plotly's ``animation_frame``:
one state per value, marks keyed by ``key=`` (plotly's ``animation_group``) so they move rather
than fade, axes fixed over the whole data.
"""

from __future__ import annotations

import hashlib
import json
import math
from typing import Any, Sequence

from . import std
from ._data import DATE, NUM, STR, Data, is_tabular
from ._ir import e
from ._node import text as _text
from .chart import Chart, SELECTABLE

__all__ = ["bar", "line", "area", "scatter", "hist", "heatmap", "pie", "donut", "treemap", "map", "choropleth",
           "cloud", "plot", "waterfall", "funnel", "sankey", "waffle", "swarm", "calendar", "stripes", "hemicycle"]


# ---- shared plumbing --------------------------------------------------------------------------------

_COMMON = ("title", "subtitle", "source", "width", "height", "theme", "description", "locale")


def _common(c: Chart, kw: dict[str, Any]) -> Chart:
    """Apply the options every chart function takes (title, size, theme, colors, names, plot params)."""
    colors = kw.pop("colors", None)
    names = kw.pop("names", None)
    facet = kw.pop("facet", None)
    facet_columns = kw.pop("facet_columns", 3)
    size = kw.pop("size", None)
    if size is not None:
        kw.setdefault("width", size[0])
        kw.setdefault("height", size[1])
    upd = {k: kw.pop(k) for k in list(kw) if k in _COMMON}
    plot = {k: v for k, v in kw.items()}
    if plot and c._plot is None:
        raise TypeError(f"datars: unexpected argument(s) {', '.join(plot)}")
    c = c.update(**upd, **plot)
    if colors:
        c = c.colors(colors)
    if names:
        c = c.names(names)
    if facet is not None:
        c = c.facet(facet, columns=facet_columns)
    return c


def _table(obj: Any, x: Any = None, y: Any = None) -> Any:
    """Data from ``data=``, or from bare ``x``/``y`` arrays (``plot(x, y)``)."""
    if obj is not None:
        return obj
    if x is not None and y is not None and not isinstance(x, str) and not isinstance(y, str):
        return {"x": list(x) if not hasattr(x, "tolist") else x, "y": list(y) if not hasattr(y, "tolist") else y}
    raise TypeError("datars: pass data (a DataFrame) and column names, or x and y arrays")


def _cols(*cols: Any) -> list:
    out: list = []
    for c in cols:
        if c is None:
            continue
        for x in (c if isinstance(c, (list, tuple)) else [c]):
            if x not in out:
                out.append(x)
    return out


def _load(obj: Any, cols: list, key: Any = None) -> Data:
    """Only the columns a chart uses — a wide DataFrame costs nothing extra."""
    if isinstance(obj, Data):
        return obj
    d = Data(obj, columns=cols)
    if key is not None:
        d.set_key([key] if isinstance(key, str) else list(key), strict=True)
    return d


def _scale(d: Data, col: Any, *, band_for_numbers: bool = False) -> str:
    k = d.kind(col)
    if k == NUM:
        return "band" if band_for_numbers else "linear"
    if k == DATE:
        return "time"
    return "band"


def _label(d: Data, col: Any) -> str:
    """An axis title: the column's own name (not its document name)."""
    return str(col)


def _pandas(obj: Any):
    """A pandas DataFrame of wide data (pandas as is; polars through a dict, no pyarrow needed)."""
    import pandas as pd

    if isinstance(obj, pd.DataFrame):
        return obj
    if type(obj).__module__.split(".")[0] == "polars":
        return pd.DataFrame(obj.to_dict(as_series=False))
    return pd.DataFrame(obj)


def _melt(obj: Any, x: Any, ys: list, var: str = "series", val: str = "value") -> tuple[Any, Any, str, str]:
    """Wide → long (``df.plot()`` style: one series per column), keeping x (or the index)."""
    df = _pandas(obj)
    if x is None:
        idx = df.index.name or "index"
        df = df.reset_index().rename(columns={"index": idx} if df.index.name is None else {})
        x = idx
    long = df.melt(id_vars=[x], value_vars=ys, var_name=var, value_name=val)
    return long, x, var, val


def _inputs(data: Any, x: Any, y: Any, color: Any, *, wide: bool = True) -> tuple[Any, Any, Any, Any, bool]:
    """(table, x, y, color, melted): bare arrays become a table; wide frames (``y`` a list, or
    omitted) become long, one series per column, like ``df.plot()``."""
    if data is None:
        return _table(None, x, y), "x", "y", color, False
    if wide and _wide(data, x, y):
        ys = list(y) if isinstance(y, (list, tuple)) else _numeric_columns(data, [x])
        obj, x2, var, val = _melt(data, x, ys)
        return obj, x2, val, var, True
    return data, x, y, color, False


def _wide(obj: Any, x: Any, y: Any) -> bool:
    return isinstance(y, (list, tuple)) or (y is None and is_tabular(obj) and not isinstance(obj, Data))


def _numeric_columns(obj: Any, exclude: Sequence[Any]) -> list:
    import pandas as pd

    df = _pandas(obj)
    return [c for c in df.columns if c not in exclude and pd.api.types.is_numeric_dtype(df[c]) and not pd.api.types.is_bool_dtype(df[c])]


def _hash(*parts: Any) -> str:
    return hashlib.sha1(json.dumps(parts, sort_keys=True, default=str).encode()).hexdigest()[:8]


def _sort_rows(d: Data, col: str, desc: bool) -> Data:
    vals = d.values[col]
    order = sorted(range(d.nrows), key=lambda i: (vals[i] is None, -(vals[i] or 0) if desc else (vals[i] or 0)))
    out = Data.from_columns({c: [v[i] for i in order] for c, v in d.values.items()}, d.kinds, names=d.names)
    out.key = d.key
    return out



def _animate(c: Chart, d: Data, frame: Any, group: list[str], *, extra_ops: list[dict] | None = None,
             label: bool = True) -> str:
    """Frames: a derived table of one frame's rows keyed by ``group``, a signal, one state per value."""
    fcol = d.col(frame)
    kind = d.kind(frame)
    values = d.unique(frame)
    if kind in (NUM, DATE):
        values = sorted(values)
    if kind == DATE:  # compare as text (the signal is a string)
        d.kinds[fcol] = STR
        d._json = None
    signal = f"frame_{fcol}"
    others = [col for col in d.values if col not in group]
    ops: list[dict] = [{"op": "filter", "expr": {"expr": f"d.{fcol} == {signal}"}}]
    if group:
        ops.append({"op": "aggregate", "groupby": group, "ops": [{"op": "first", "field": o, "as": o} for o in others]})
    ops += extra_ops or []
    name = f"frame_{_hash(d.name, ops)}"
    c._derived[name] = {"from": d, "ops": ops}
    c._signals[signal] = {"type": "num" if kind == NUM else "str", "default": values[0] if values else None}
    c._frame = {"signal": signal, "col": fcol, "values": values, "table": name}
    fmt = (lambda v: str(int(v)) if float(v).is_integer() else str(v)) if kind == NUM else str
    c._states = [{"name": fmt(v), "set": {signal: v}} for v in values]
    c._states_user = False
    c._program = {"drivers": "autoplay", "loop": False, "hold": 0.6}
    c._motion = [{"select": {"role": "datum"}, "duration": 0.8, "easing": "cubic-in-out"}]
    if label:
        style = {"size": 72, "weight": 700, "ink": "$muted", "align": "end", "baseline": "bottom"}
        at = (e("box.w - 8"), e("box.h - 44"))
        if kind == NUM:
            # A number counts from frame to frame (2000 … 2010) as the marks move.
            ints = all(float(v).is_integer() for v in values)
            label_node = _text("", at, number={"value": e(signal), "format": "d" if ints else ",.2~f"},
                               key="frame-label", z=-1, opacity=0.3, style=style)
        else:
            label_node = _text(e(signal), at, key="frame-label", z=-1, opacity=0.3, style=style)
        c._overlays.append(label_node)
    return name


def _finish_plot(c: Chart, d: Data, x: str, y: str, *, xt: str, yt: str, color: str | None, table: Any,
                 x_label: Any, y_label: Any, legend: bool | None, fixed: bool) -> None:
    """Plot params: scales and axis titles; frames keep axes over the whole data."""
    p: dict[str, Any] = {"data": table, "x": d.col(x), "y": d.col(y), "xType": xt, "yType": yt}
    if color is not None:
        p["color"] = d.col(color)
        if d.kind(color) == NUM:
            p["colorType"] = "sequential"
    if legend is not None:
        p["legend"] = legend
    if x_label is not None:
        p["xLabel"] = x_label
    elif xt not in ("band", "point"):
        p["xLabel"] = _label(d, x)
    if y_label is not None:
        p["yLabel"] = y_label
    elif yt not in ("band", "point"):
        p["yLabel"] = _label(d, y)
    if fixed:
        for axis, col, t in (("x", x, xt), ("y", y, yt)):
            if t in ("linear", "log", "sqrt", "time", "band"):
                p[f"{axis}Domain"] = {"data": d, "field": d.col(col)}
    c._plot = {k: v for k, v in p.items() if v is not None and v != ""}


# ---- cartesian charts -------------------------------------------------------------------------------


def bar(data: Any = None, x: Any = None, y: Any = None, color: Any = None, *, horizontal: bool | None = None,
        stacked: bool | None = None, grouped: bool = False, normalize: bool = False, labels: bool = False,
        sort: str | None = None, top: int | None = None, frame: Any = None, key: Any = None,
        highlight: Sequence[Any] | None = None, select: bool = True, format: str | None = None,
        prefix: str | None = None, suffix: str | None = None, x_label: str | None = None, y_label: str | None = None,
        legend: bool | None = None, **kw: Any) -> Chart:
    """Bars: a category against a value — vertical, or horizontal when ``y`` is the category.

    ``color`` splits bars into series (stacked; ``grouped=True`` side by side, ``normalize=True``
    for 100 %). ``sort="desc"`` orders by value; with ``frame=`` that's a bar chart race
    (``top=10`` keeps the leaders). Clicking a bar highlights it (``select=False`` to turn off).

    >>> import datars as dr
    >>> c = dr.bar({"country": ["SE", "NO", "DK"], "gdp": [593, 485, 406]}, x="country", y="gdp")
    >>> c.to_dict()["scene"]["children"][0]["params"]["xType"]
    'band'
    """
    obj, x, y, color, melted = _inputs(data, x, y, color)
    if melted and y_label is None:
        y_label = ""  # "value" of melted columns says nothing
    d = _load(obj, _cols(x, y, color, frame, key, kw.get("facet")))
    if horizontal is None:
        horizontal = d.kind(x) == NUM and d.kind(y) != NUM
    cat, val = (y, x) if horizontal else (x, y)
    series = color if color is not None and d.col(color) != d.col(cat) else None
    if series is not None and stacked is None:
        stacked = not grouped and not d.is_unique(_cols(cat, frame))
    stacked = bool(stacked) or normalize
    c = Chart()
    group = _cols(cat, series if (stacked or grouped) else None)
    # Keys: a bar is its category (per series, per frame) — what morphs into what.
    if key is None:
        d.set_key([d.col(g) for g in _cols(group, frame)], strict=False)
    elif frame is None:
        d.set_key(_cols(key), strict=True)
    if sort and frame is None:
        d = _sort_rows(d, d.col(val), sort == "desc")
    table: Any = d
    if frame is not None:
        extra = []
        if sort:
            extra.append({"op": "sort", "by": [[d.col(val), "desc" if sort == "desc" else "asc"]]})
            c._program["race"] = True
        if top:
            extra.append({"op": "top", "n": int(top), "by": d.col(val)})
        race = bool(sort)
        table = _animate(c, d, frame, [d.col(g) for g in _cols(key or group)], extra_ops=extra)
        c._program["race"] = race
    band = "band"
    xt, yt = (_value_scale(d, x), band) if horizontal else (band, _value_scale(d, y))
    _finish_plot(c, d, x, y, xt=xt, yt=yt, color=color, table=table, x_label=x_label, y_label=y_label,
                 legend=legend if legend is not None else (True if series is not None else None), fixed=frame is not None)
    if frame is not None and c._program.get("race"):
        # A race: categories reorder each frame (the band follows the frame's sorted rows).
        c._plot.pop("yDomain" if horizontal else "xDomain", None)
    common = dict(format=format, prefix=prefix, suffix=suffix)
    if stacked:
        mark = std.stacked(offset="expand" if normalize else None, series=d.col(series) if series else None, **common)
    elif grouped:
        mark = std.grouped(series=d.col(series) if series else None, **common)
    else:
        mark = std.bar(labels=labels or None, radius=None, **common)
    c._marks = [mark]
    if select or highlight:
        c._ensure_highlight()
    if highlight:
        c = c.highlight(*highlight)
    return _common(c, kw)


def _value_scale(d: Data, col: Any) -> str:
    """The value axis of bars: numbers are linear, dates time."""
    return "time" if d.kind(col) == DATE else "linear"


def line(data: Any = None, x: Any = None, y: Any = None, color: Any = None, *, labels: bool | None = None,
         points: bool = False, curve: str | None = None, width: float | None = None, frame: Any = None,
         key: Any = None, highlight: Sequence[Any] | None = None, select: bool = True, x_label: str | None = None,
         y_label: str | None = None, legend: bool | None = None, zero: bool = False, log_y: bool = False,
         **kw: Any) -> Chart:
    """Lines: one per ``color`` series through x/y (x: numbers, dates or categories).

    Pass a wide DataFrame and no ``y`` (or ``y=[cols]``) for one line per column against the
    index — ``df.plot()`` style. Series are labelled at their ends when there are few
    (``labels=False`` for a legend). Lines draw on as they enter.
    """
    obj, x, y, color, melted = _inputs(data, x, y, color)
    if melted and y_label is None:
        y_label = ""  # "value" of melted columns says nothing
    d = _load(obj, _cols(x, y, color, frame, key, kw.get("facet")))
    c = Chart()
    series = color
    if key is None:
        d.set_key([d.col(k) for k in _cols(series, x, frame)], strict=False)
    table: Any = d
    if frame is not None:
        table = _animate(c, d, frame, [d.col(k) for k in _cols(key or series, x)])
    n = len(d.unique(color)) if color is not None else 0
    if labels is None:
        labels = color is not None and n <= 10
    xt = _scale(d, x)
    _finish_plot(c, d, x, y, xt=xt, yt="log" if log_y else "linear", color=color, table=table, x_label=x_label,
                 y_label=y_label, legend=legend if legend is not None else (color is not None and not labels),
                 fixed=frame is not None)
    c._plot["zero"] = zero
    c._marks = [std.line(labels=labels or None, points=points or None, curve=curve, width=width)]
    if select or highlight:
        c._ensure_highlight()
    if highlight:
        c = c.highlight(*highlight)
    return _common(c, kw)


def area(data: Any = None, x: Any = None, y: Any = None, color: Any = None, *, stacked: bool | None = None,
         normalize: bool = False, opacity: float | None = None, curve: str | None = None, line: bool = False,
         x_label: str | None = None, y_label: str | None = None, legend: bool | None = None, **kw: Any) -> Chart:
    """Filled areas from zero (or stacked, with ``color``: ``stacked=True``, ``normalize=True``)."""
    obj, x, y, color, melted = _inputs(data, x, y, color)
    if melted and y_label is None:
        y_label = ""  # "value" of melted columns says nothing
    d = _load(obj, _cols(x, y, color, kw.get("facet")))
    d.set_key([d.col(k) for k in _cols(color, x)], strict=False)
    c = Chart()
    stacked = (color is not None) if stacked is None else stacked
    stacked = stacked or normalize
    table: Any = d
    yy = y
    if stacked and color is not None:
        ops = [{"op": "stack", "x": d.col(x), "series": d.col(color), "value": d.col(y),
                "offset": "expand" if normalize else "zero", "as": ["y0", "y1"]}]
        table = f"stack_{_hash(d.name, ops)}"
        c._derived[table] = {"from": d, "ops": ops}
    _finish_plot(c, d, x, y, xt=_scale(d, x), yt="linear", color=color, table=table, x_label=x_label, y_label=y_label,
                 legend=legend if legend is not None else (color is not None), fixed=False)
    if stacked and color is not None:
        c._plot["y"] = "y1"
        c._plot["yDomain"] = {"data": table, "field": "y1"}
        if y_label is None:
            c._plot["yLabel"] = str(yy)
        c._marks = [std.area(y="y1", y0="y0", opacity=opacity, curve=curve)]
    else:
        c._marks = [std.area(opacity=opacity if opacity is not None else (0.3 if line else None), curve=curve)]
        if line:
            c._marks.append(std.line(curve=curve))
    return _common(c, kw)


def scatter(data: Any = None, x: Any = None, y: Any = None, color: Any = None, size: Any = None, *,
            label: Any = None, frame: Any = None, key: Any = None, r: float | None = None, size_max: float = 18,
            opacity: float | None = None, symbol: str | None = None, log_x: bool = False, log_y: bool = False,
            x_label: str | None = None, y_label: str | None = None, legend: bool | None = None, zero: bool = False,
            **kw: Any) -> Chart:
    """A dot per row (instanced — 10⁵–10⁶ points are fine; for millions see :func:`cloud`).

    ``size`` maps a column to area (bubbles, up to ``size_max`` px radius); ``label`` names each
    dot in its tooltip. With ``frame=`` and ``key=`` it's Gapminder: every dot glides from year
    to year.
    """
    obj, x, y, color, _ = _inputs(data, x, y, color, wide=False)
    d = _load(obj, _cols(x, y, color, size, label, frame, key, kw.get("facet")))
    if d.nrows > 1_000_000:
        import warnings

        warnings.warn("datars: scatter of over a million rows — dr.cloud() draws any number with level of detail", stacklevel=2)
    c = Chart()
    if size is not None:
        vals = d.column(size)
        top = max((abs(v) for v in vals if isinstance(v, (int, float))), default=0) or 1
        radii = [None if not isinstance(v, (int, float)) else round(max(1.0, size_max * math.sqrt(abs(v) / top)), 3) for v in vals]
        d = d.with_column("_r", NUM, radii)
    if key is not None and frame is None:
        d.set_key(_cols(key), strict=True)
    elif key is None and label is not None and frame is None:
        d.set_key([d.col(label)], strict=False)
    table: Any = d
    if frame is not None:
        group = _cols(key if key is not None else label if label is not None else color)
        if not group:
            raise TypeError("datars: an animated scatter needs key= (the column that identifies a dot across frames)")
        d.set_key([d.col(g) for g in group] + [d.col(frame)], strict=False)
        table = _animate(c, d, frame, [d.col(g) for g in group])
    xt = "log" if log_x else _scale(d, x)
    yt = "log" if log_y else _scale(d, y)
    _finish_plot(c, d, x, y, xt=xt, yt=yt, color=color, table=table, x_label=x_label, y_label=y_label,
                 legend=legend if legend is not None else (color is not None and d.kind(color) == STR), fixed=frame is not None)
    c._plot["zero"] = zero
    lab = None
    if label is not None:
        lc = d.col(label)
        lab = e(f"`${{key.name(d.{lc})}}: ${{format(d.{d.col(x)}, \",.3~g\")}}, ${{format(d.{d.col(y)}, \",.3~g\")}}`")
    c._marks = [std.point(r=e("d._r") if size is not None else r, label=lab, opacity=opacity, symbol=symbol)]
    return _common(c, kw)


def hist(data: Any = None, x: Any = None, color: Any = None, *, bins: int | Sequence[float] = 20,
         range: tuple[float, float] | None = None, density: bool = False, stacked: bool = True, nice: bool = True,
         x_label: str | None = None, y_label: str | None = None, **kw: Any) -> Chart:
    """A histogram: counts per bin as bars (``color`` stacks groups; ``stacked=False`` dodges).

    ``bins`` is a bin count or explicit edges, ``range`` and ``density`` as in numpy. A count asks
    for about that many bins on round edges (0, 2.5, 5 …) unless ``nice=False`` (numpy's exact,
    equal-width edges). Bars are labelled by their lower edge; tooltips give the whole bin.

    >>> import datars as dr
    >>> c = dr.hist({"v": [1, 2, 2, 3, 3, 3]}, x="v", bins=3, nice=False)
    >>> c.to_dict()["data"][next(iter(c.to_dict()["data"]))]["values"]["count"]
    [1, 2, 3]
    """
    import numpy as np

    if data is None and x is not None and not isinstance(x, str):
        data, x = {"value": x}, "value"
    d = _load(data, _cols(x, color))
    vals = np.array(d.column(x), dtype=float)
    finite = np.isfinite(vals)
    if isinstance(bins, int) and nice:
        edges = _nice_edges(vals[finite], bins, range)
    else:
        edges = np.histogram_bin_edges(vals[finite], bins=bins, range=range)
    cols: dict[str, list] = {"bin": [], "range": [], "count": [], "lo": [], "hi": []}
    groups = d.unique(color) if color is not None else [None]
    if color is not None:
        cols["group"] = []
        cv = np.array(d.column(color), dtype=object)
    for g in groups:
        sel = finite if g is None else finite & (cv == g)
        counts, _ = np.histogram(vals[sel], bins=edges, density=density)
        for i, n in enumerate(counts.tolist()):
            cols["bin"].append(_fmt_num(edges[i]))
            cols["range"].append(f"{_fmt_num(edges[i])}–{_fmt_num(edges[i + 1])}")
            cols["count"].append(round(n, 12) if density else int(n))
            cols["lo"].append(float(edges[i]))
            cols["hi"].append(float(edges[i + 1]))
            if g is not None:
                cols["group"].append(str(g))
    kinds = {"bin": STR, "range": STR, "count": NUM, "lo": NUM, "hi": NUM, **({"group": STR} if color is not None else {})}
    hd = Data.from_columns(cols, kinds, key=["bin", "group"] if color is not None else ["bin"])
    c = bar(hd, x="bin", y="count", color="group" if color is not None else None, stacked=stacked if color is not None else None,
            grouped=color is not None and not stacked, select=False, x_label=x_label if x_label is not None else str(x),
            y_label=y_label if y_label is not None else ("density" if density else "count"), **kw)
    c._plot["padding"] = 0.04
    if color is None:
        fmt = '",.4~g"' if density else '","'
        label = e("`${d.range}: ${format(d.count, " + fmt + ")}`")
        c._marks = [m.set(label=label) if m.recipe == "@datars/std/bar" else m for m in c._marks]
    return c


def _nice_edges(vals, count: int, rng: tuple[float, float] | None = None):
    """About ``count`` equal bins on round edges (steps of 1, 2, 2.5 or 5 × 10ⁿ)."""
    import numpy as np

    lo, hi = rng if rng is not None else ((float(vals.min()), float(vals.max())) if vals.size else (0.0, 1.0))
    if hi <= lo:
        hi = lo + 1
    raw = (hi - lo) / max(1, count)
    mag = 10 ** math.floor(math.log10(raw))
    step = next(m * mag for m in (1, 2, 2.5, 5, 10) if m * mag >= raw)
    start = math.floor(lo / step) * step
    n = max(1, math.ceil((hi - start) / step))
    if start + n * step < hi:  # rounding: the maximum must be inside (numpy's last bin is closed)
        n += 1
    return np.array([round(start + i * step, 12) for i in range(n + 1)])


def _fmt_num(v: float) -> str:
    v = float(v)
    if v.is_integer():
        return f"{int(v):,}"
    return f"{round(v, 10):,.4g}"


def heatmap(data: Any = None, x: Any = None, y: Any = None, value: Any = None, *, colors: str = "sequential",
            format: str | None = None, x_label: str | None = None, y_label: str | None = None, legend: bool = True,
            **kw: Any) -> Chart:
    """A cell per (x, y) coloured by ``value`` (``colors="diverging"`` around zero)."""
    d = _load(data, _cols(x, y, value, kw.get("facet")))
    d.set_key([d.col(y), d.col(x)], strict=False)
    c = Chart()
    c._plot = {"data": d, "x": d.col(x), "y": d.col(y), "xType": "band", "yType": "band", "color": d.col(value),
               "colorType": colors, "padding": 0.02, "grid": False, "legend": legend}
    if x_label:
        c._plot["xLabel"] = x_label
    if y_label:
        c._plot["yLabel"] = y_label
    c._marks = [std.cell(format=format)]
    return _common(c, kw)


# ---- standalone charts ------------------------------------------------------------------------------


def pie(data: Any = None, names: Any = None, values: Any = None, *, inner: float = 0, labels: bool | None = None,
        sort: str | None = None, total: bool | None = None, format: str | None = None, frame: Any = None,
        category: Any = None, value: Any = None, **kw: Any) -> Chart:
    """Parts of a whole as slices (``names``/``values`` like plotly, or ``category``/``value``).
    Slices keep their category's key, so a story morphs bars into slices and back."""
    names = names if names is not None else category
    values = values if values is not None else value
    colors_ = kw.pop("colors", None)
    d = _load(data, _cols(names, values, frame))
    d.set_key([d.col(names)] + ([d.col(frame)] if frame is not None else []), strict=False)
    c = Chart()
    table: Any = d
    if frame is not None:
        table = _animate(c, d, frame, [d.col(names)])
    c._main = std.pie(data=table, category=d.col(names), value=d.col(values), inner=inner or None, labels=labels,
                      sort=sort, total=total, format=format)
    if colors_:
        kw["colors"] = colors_
    return _common(c, kw)


def donut(data: Any = None, names: Any = None, values: Any = None, *, inner: float = 0.6, total: bool = True, **kw: Any) -> Chart:
    """A pie with a hole (and the total in it)."""
    return pie(data, names, values, inner=inner, total=total, **kw)


def treemap(data: Any = None, names: Any = None, values: Any = None, *, color: Any = None, labels: bool | None = None,
            format: str | None = None, stops: str | None = None, **kw: Any) -> Chart:
    """Rectangles with area ∝ value (coloured by category, or by ``color`` on a scale)."""
    d = _load(data, _cols(names, values, color))
    d.set_key([d.col(names)], strict=False)
    c = Chart()
    ct = None
    if color is not None and d.kind(color) == NUM:
        ct = "piecewise" if stops else "sequential"
    c._main = std.treemap(data=d, category=d.col(names), value=d.col(values), color=d.col(color) if color is not None else None,
                          color_type=ct, labels=labels, format=format, stops=stops)
    return _common(c, kw)


def map(data: Any = None, key: Any = None, value: Any = None, *, source: Any = "countries", id: str | None = None,
        projection: str | None = None, legend: bool = True, labels: bool | None = None, stops: str | None = None,
        diverging: bool = False, format: str | None = None, highlight: Sequence[Any] | None = None,
        select: bool = True, frame: Any = None, fit: Any = None, **kw: Any) -> Chart:
    """A choropleth: regions of a geo source coloured by ``value``, joined on ``key``.

    ``source`` is the built-in ``"countries"`` atlas (ISO 3166-1 alpha-3 ids, e.g. ``"SWE"``), a
    GeoJSON dict, a geopandas GeoDataFrame, or anything with ``__geo_interface__`` (``id`` names
    the feature property holding each region's id). ``frame=`` animates the colours over time.
    """
    geo = _geo_source(source, id)
    c = Chart()
    table: Any = None
    if data is not None:
        d = _load(data, _cols(key, value, frame))
        d.set_key([d.col(key)] + ([d.col(frame)] if frame is not None else []), strict=False)
        table = d
        if frame is not None:
            table = _animate(c, d, frame, [d.col(key)])
        kcol, vcol = d.col(key), d.col(value) if value is not None else None
        ctype = None
        if value is not None and d.kind(value) == STR:
            ctype = "categorical"
        elif diverging:
            ctype = "diverging"
        elif stops:
            ctype = "piecewise"
    else:
        kcol = vcol = ctype = None
    c._main = std.map(source=geo, data=table, key=kcol, value=vcol, projection=projection, legend=legend if value is not None else None,
                      labels=labels, stops=stops, color_type=ctype, format=format, fit=fit)
    if select or highlight:
        c._ensure_highlight()
    if highlight:
        c = c.highlight(*highlight)
    return _common(c, kw)


choropleth = map


def _geo_source(source: Any, id: str | None) -> Any:
    if isinstance(source, str):
        if source.startswith(("http://", "https://")) or source.endswith((".json", ".geojson", ".topojson")):
            return _UrlSource(source, id)
        return _Atlas(source)
    if hasattr(source, "__geo_interface__"):
        source = source.__geo_interface__
    if isinstance(source, dict) and source.get("type") == "Topology":
        return _Inline({"topojson": source, **({"id": id} if id else {})}, "geo")
    if isinstance(source, dict) and source.get("type") == "FeatureCollection":
        return _Inline({"geojson": source, **({"id": id} if id else {})}, "geo")
    raise TypeError("datars: source= takes an atlas name ('countries'), a GeoJSON/TopoJSON dict, a URL or a GeoDataFrame")


class _Inline(Data):
    """A non-tabular source (geo, atlas, URL) registered like a table."""

    def __init__(self, src: dict, prefix: str):
        super().__init__()
        self._src = src
        self._prefix = prefix

    def source(self) -> dict:
        return self._src

    @property
    def name(self) -> str:
        return f"{self._prefix}_" + hashlib.sha1(self.source_json().encode()).hexdigest()[:10]


def _Atlas(name: str) -> _Inline:
    return _Inline({"atlas": name}, name)


def _UrlSource(url: str, id: str | None) -> _Inline:
    return _Inline({"url": url, **({"id": id} if id else {})}, "geo")


def cloud(data: Any = None, x: Any = None, y: Any = None, color: Any = None, *, r: Any = None, label: Any = None,
          points: int | None = None, glow: Any = None, max_zoom: float | None = None, name: str | None = None,
          bbox: Sequence[float] | None = None, fill: Any = None, **kw: Any) -> Chart:
    """Millions of points: the engine indexes the rows once into a pyramid and each frame draws a
    density-preserving sample of what's in view (every row up close). Drag to pan, scroll to zoom.

    ``color`` picks each dot's ink from the theme's palette (categories) or its sequential ramp
    (numbers, in 7 steps) — an expression over the row alone, as level-of-detail points need.
    """
    d = _load(data, _cols(x, y, color, label))
    c = Chart()
    if color is not None and fill is None:
        fill = _row_inks(d, color)
    lab = e(f"`${{d.{d.col(label)}}}`") if label is not None else None
    c._main = std.cloud(data=d, x=d.col(x), y=d.col(y), fill=fill, r=r, label=lab, points=points, glow=glow,
                        max_zoom=max_zoom, name=name or f"{d.nrows:,} points", bbox=list(bbox) if bbox else None)
    return _common(c, kw)


def _row_inks(d: Data, color: Any, *, steps: int = 7, most: int = 24):
    """An ink per row from its own value — palette slots for categories, ramp steps for numbers —
    still late-bound theme inks (``$categorical[i]``), never baked colours."""
    col = d.col(color)
    if d.kind(color) == NUM:
        ext = d.extent(color)
        if not ext or ext[0] == ext[1]:
            return "$sequential[3]"
        lo, hi = ext
        cuts = [lo + (hi - lo) * i / steps for i in range(1, steps)]
        expr = f'"$sequential[{steps - 1}]"'
        for i in range(steps - 2, -1, -1):
            expr = f'd.{col} < {cuts[i]!r} ? "$sequential[{i}]" : {expr}'
        return e(expr)
    cats = d.unique(color)
    if len(cats) > most:
        import warnings

        warnings.warn(f"datars: {len(cats)} categories in {color!r}: the first {most} get palette colours, the rest $muted", stacklevel=3)
    expr = '"$muted"'
    for i, v in reversed(list(enumerate(cats[:most]))):
        expr = f'd.{col} == {json.dumps(str(v))} ? "$categorical[{i}]" : {expr}'
    return e(expr)


# ---- matplotlib-style -------------------------------------------------------------------------------


def plot(x: Any, y: Any = None, *args: Any, kind: str = "line", label: str | None = None, data: Any = None, **kw: Any) -> Chart:
    """``plt.plot`` without the axes object: ``dr.plot(x, y)``, ``dr.plot(y)``, ``dr.plot(series)``,
    ``dr.plot(df)`` (one line per numeric column), ``kind="scatter"|"bar"|"area"``.

    >>> import datars as dr
    >>> dr.plot([1, 2, 3], [2, 4, 3]).to_dict()["size"]
    {'width': 720, 'height': 440}
    """
    fns = {"line": line, "scatter": scatter, "bar": bar, "area": area}
    if kind not in fns:
        raise ValueError(f"datars: kind= is one of {', '.join(fns)}")
    fn = fns[kind]
    if data is not None:
        return fn(data, x, y, **kw)
    mod = type(x).__module__.split(".")[0]
    if mod in ("pandas", "polars") and y is None:
        import pandas as pd

        if isinstance(x, pd.Series):
            name = x.name if x.name is not None else "value"
            df = x.to_frame(name=name).reset_index()
            idx = df.columns[0]
            return fn(df, idx, name, **kw)
        if isinstance(x, pd.DataFrame):
            return fn(x, None, None, **kw) if kind in ("line", "area", "bar") else fn(x, **kw)
        return fn(x, **kw)
    if y is None:
        y = x
        x = list(range(len(y)))
    series = {"x": x.tolist() if hasattr(x, "tolist") else list(x), "y": y.tolist() if hasattr(y, "tolist") else list(y)}
    c = fn(series, "x", "y", **kw)
    return c


# ---- the other std charts, thin wrappers -------------------------------------------------------------


#: The fields that identify a row, per standalone recipe.
_KEYS = {"waffle": ("category",), "hemicycle": ("category",), "funnel": ("stage",), "sankey": ("source", "target"),
         "calendar": ("date",), "stripes": ("x",)}


def _standalone(recipe: str, doc: str):
    fn = getattr(std, recipe)
    spec = std.RECIPES[recipe]["params"]
    renames = std.RENAMES[recipe]

    def make(data: Any = None, **kw: Any) -> Chart:
        # Chart options, unless the recipe has a parameter of that name (a sankey's `source`).
        common = {k: kw.pop(k) for k in list(kw) if (k in _COMMON or k in ("colors", "names", "size")) and renames.get(k, k) not in spec}
        fields = {k: v for k, v in kw.items() if spec.get(renames.get(k, k), {}).get("type") == "field"}
        d = _load(data, _cols(*fields.values())) if data is not None else None
        if d is not None and d.key is None:
            # Rows keyed by what they are (a party's seats, a link's ends): units and morphs follow.
            by = [fields[f] for f in _KEYS.get(recipe, ()) if f in fields]
            if by:
                d.set_key([d.col(b) for b in by], strict=False)
        params = {k: (d.col(v) if d is not None and isinstance(v, str) and d.has(v) else v) for k, v in kw.items()}
        c = Chart()
        c._main = fn(data=d, **params) if "data" in spec else fn(**params)
        if d is not None and recipe in SELECTABLE:
            c._ensure_highlight()
        return _common(c, common)

    make.__name__ = recipe
    make.__qualname__ = recipe
    make.__doc__ = f"{doc}\n\n{std.RECIPES[recipe]['doc']}\n\nParameters are :func:`datars.std.{recipe}`'s (``data`` is a DataFrame)."
    return make


def _in_plot(recipe: str, doc: str, x_default: str = "band"):
    fn = getattr(std, recipe)

    def make(data: Any = None, x: Any = None, y: Any = None, color: Any = None, **kw: Any) -> Chart:
        common = {k: kw.pop(k) for k in list(kw) if k in _COMMON or k in ("colors", "names", "size", "x_label", "y_label", "legend", "format", "prefix", "suffix")}
        d = _load(data, _cols(x, y, color, *[v for k, v in kw.items() if k in ("total", "series")]))
        c = Chart()
        xt = x_default if x_default != "auto" else _scale(d, x)
        yt = "linear" if y is not None else "linear"
        c._plot = {"data": d, "x": d.col(x), "y": d.col(y) if y is not None else d.col(x), "xType": xt, "yType": yt}
        if color is not None:
            c._plot["color"] = d.col(color)
        for k in ("x_label", "y_label", "legend", "format", "prefix", "suffix"):
            if k in common:
                c._plot[{"x_label": "xLabel", "y_label": "yLabel"}.get(k, k)] = common.pop(k)
        params = {k: (d.col(v) if isinstance(v, str) and d.has(v) and k in ("total", "series") else v) for k, v in kw.items()}
        c._marks = [fn(**params)]
        return _common(c, common)

    make.__name__ = recipe
    make.__doc__ = f"{doc}\n\n{std.RECIPES[recipe]['doc']}"
    return make


waterfall = _in_plot("waterfall", "A waterfall (bridge): ``dr.waterfall(df, x='step', y='change', total='is_total')``.")
swarm = _in_plot("swarm", "A beeswarm along x: ``dr.swarm(df, x='value', color='group')``.", x_default="auto")
funnel = _standalone("funnel", "A funnel: ``dr.funnel(df, stage='stage', value='users')``.")
sankey = _standalone("sankey", "Flows: ``dr.sankey(df, source='from', target='to', value='amount')``.")
waffle = _standalone("waffle", "A waffle: ``dr.waffle(df, category='party', value='seats')``.")
calendar = _standalone("calendar", "A calendar heatmap: ``dr.calendar(df, date='day', value='steps')``.")
stripes = _standalone("stripes", "Warming stripes: ``dr.stripes(df, x='year', value='anomaly')``.")
hemicycle = _standalone("hemicycle", "A parliament: ``dr.hemicycle(df, category='party', value='seats')``.")
