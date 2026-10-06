"""Exports through the ``datars`` CLI (skipped without one), and every chart the engine accepts."""

from __future__ import annotations

import shutil

import pytest

import datars as dr
from datars import _cli


def test_missing_cli_says_how_to_get_one(votes, no_cli, tmp_path):
    with pytest.raises(_cli.CLINotFound, match="DATARS_CLI"):
        dr.bar(votes, x="party", y="share").save(str(tmp_path / "x.png"))


def test_save_formats(cli, votes, tmp_path):
    c = dr.bar(votes, x="party", y="share", title="Votes").step("all").step("big", highlight=["S"])
    png = c.save(str(tmp_path / "a.png"), dpr=1)
    assert open(png, "rb").read(8) == b"\x89PNG\r\n\x1a\n"
    svg = c.save(str(tmp_path / "a.svg"), state="big")
    assert open(svg, encoding="utf-8").read().startswith("<svg")
    pdf = c.save(str(tmp_path / "a.pdf"))
    assert open(pdf, "rb").read(4) == b"%PDF"
    with pytest.raises(ValueError, match="don't know"):
        c.save(str(tmp_path / "a.gif"))


def test_states_render_differently(cli, votes):
    c = dr.bar(votes, x="party", y="share").step("all").step("big", highlight=["S"])
    assert c.image("png", state="all") != c.image("png", state="big")
    assert c.image("png", state=0) == c.image("png", state="all")


def test_mp4(cli, votes, tmp_path):
    if not shutil.which("ffmpeg"):
        pytest.skip("no ffmpeg")
    s = dr.story(dr.bar(votes, x="party", y="share"), dr.pie(votes, names="party", values="share"))
    out = s.save(str(tmp_path / "s.mp4"), fps=12, hold=0.5, size=(320, 200))
    assert (tmp_path / "s.mp4").stat().st_size > 1000 and out.endswith(".mp4")


def test_bundle_and_publish(cli, votes, tmp_path):
    c = dr.bar(votes, x="party", y="share", title="Votes")
    c.save(str(tmp_path / "v.datars"))
    assert (tmp_path / "v.datars").stat().st_size > 1000
    snippet = c.publish(str(tmp_path / "site"), alias="votes")
    assert (tmp_path / "site" / "c" / "votes").is_file() and 'src="c/votes"' in snippet


def test_every_chart_kind_checks_clean(cli, votes, gap):
    """The engine resolves every chart the functions build, with no diagnostics."""
    import numpy as np
    import pandas as pd

    df = pd.DataFrame({"q": ["Q1", "Q1", "Q2", "Q2"], "region": ["N", "S", "N", "S"], "sales": [1, 2, 3, 4]})
    ts = pd.DataFrame({"a": np.arange(10.0), "b": np.arange(10.0)[::-1]}, index=pd.date_range("2024-01-01", periods=10, name="day"))
    charts = {
        "bar": dr.bar(votes, x="party", y="share", labels=True),
        "hbar": dr.bar(votes, x="share", y="party", sort="desc"),
        "stacked": dr.bar(df, x="q", y="sales", color="region"),
        "grouped": dr.bar(df, x="q", y="sales", color="region", grouped=True),
        "race": dr.bar(gap, x="life", y="country", frame="year", sort="desc"),
        "line": dr.line(ts),
        "area": dr.area(df, x="q", y="sales", color="region"),
        "scatter": dr.scatter(gap, x="gdp per capita", y="life", size="pop", color="continent", label="country"),
        "gapminder": dr.scatter(gap, x="gdp per capita", y="life", size="pop", color="continent", frame="year", key="country"),
        "hist": dr.hist({"v": np.arange(100.0)}, x="v"),
        "heatmap": dr.heatmap(df, x="q", y="region", value="sales"),
        "pie": dr.pie(votes, names="party", values="share", title="Pie"),
        "donut": dr.donut(votes, names="party", values="share"),
        "treemap": dr.treemap(votes, names="party", values="share"),
        "map": dr.map(pd.DataFrame({"iso": ["SWE", "NOR"], "v": [1, 2]}), key="iso", value="v"),
        "cloud": dr.cloud({"x": np.arange(500.0), "y": np.arange(500.0) % 7}, x="x", y="y"),
        "facet": dr.line(gap, x="year", y="life", color="country", facet="continent"),
        "overlay": dr.bar(votes, x="party", y="share") + dr.line(votes.assign(t=votes.share * 1.1), x="party", y="t"),
        "annotated": dr.line(ts).rule(y=5, label="five").annotate("peak", x="2024-01-10", y=9),
        "story": dr.story(dr.bar(votes, x="party", y="share"), (dr.pie(votes, names="party", values="share"), "Slices.")),
        "layout": dr.bar(votes, x="party", y="share") | dr.pie(votes, names="party", values="share"),
        "sankey": dr.sankey(pd.DataFrame({"a": ["x", "x"], "b": ["y", "z"], "n": [3, 4]}), source="a", target="b", value="n"),
        "waffle": dr.waffle(votes, category="party", value="share"),
        "plot": dr.plot([1, 2, 3], [3, 1, 2]),
        "noir": dr.bar(votes, x="party", y="share", theme="noir"),
    }
    bad = {name: diags for name, c in charts.items() if (diags := c.check())}
    assert not bad, bad
