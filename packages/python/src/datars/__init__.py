"""datars for Python: animated, themed data graphics from DataFrames — live in notebooks,
identical as PNG, SVG, PDF and MP4.

    import datars as dr

    dr.bar(df, x="country", y="gdp", color="region")             # a chart (displays itself)
    dr.scatter(gap, x="gdp", y="life", size="pop", color="continent",
               frame="year", key="country")                      # Gapminder, animated
    dr.story(dr.bar(votes, x="party", y="share"),
             (dr.pie(votes, names="party", values="share"), "Each bar becomes a slice."))
    chart.save("fig.png")   # .svg .pdf .mp4 .html .json .datars

A chart is a datars document — the IR the TypeScript SDK writes — built from the standard
library's recipes (:mod:`datars.std`, generated from their own descriptions). The engine is the
same everywhere: the notebook's live view is the web runtime, exports come from the ``datars``
CLI, and the document plays unchanged on iOS, Android and the web.
"""

from __future__ import annotations

from . import std
from ._data import Data, data
from ._display import options
from ._ir import Expr, e, field
from ._node import Node, group, raw, shape, text, use, view
from .chart import Chart, Document, Layout, Story, chart, story
from .charts import (area, bar, calendar, choropleth, cloud, donut, funnel, heatmap, hemicycle, hist, line, map,
                     pie, plot, sankey, scatter, stripes, swarm, treemap, waffle, waterfall)
from .theme import Theme, font, theme

__version__ = "0.1.0"


def row(*charts: Document, gap: float = 24, title: str | None = None) -> Layout:
    """Charts side by side (same as ``a | b | c``)."""
    return Layout(list(charts), direction="columns", gap=gap, title=title)


def column(*charts: Document, gap: float = 24, title: str | None = None) -> Layout:
    """Charts one above the other (same as ``a & b``)."""
    return Layout(list(charts), direction="rows", gap=gap, title=title)


__all__ = [
    "Chart", "Data", "Document", "Expr", "Layout", "Node", "Story", "Theme",
    "area", "bar", "calendar", "chart", "choropleth", "cloud", "column", "data", "donut", "e", "field", "font",
    "funnel", "group", "heatmap", "hemicycle", "hist", "line", "map", "options", "pie", "plot", "raw", "row",
    "sankey", "scatter", "shape", "std", "story", "stripes", "swarm", "text", "theme", "treemap", "use", "view",
    "waffle", "waterfall",
]
