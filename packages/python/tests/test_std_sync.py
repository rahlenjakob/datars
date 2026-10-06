"""The Python std wrappers are generated from the recipes' own descriptions and stay in sync."""

from __future__ import annotations

import importlib.util
import inspect
import json
from pathlib import Path

import pytest

import datars as dr
from datars import _cli, std
from datars.chart import PLOT_MARKS

ROOT = Path(__file__).resolve().parents[1]


def _gen():
    spec = importlib.util.spec_from_file_location("gen_std", ROOT / "tools" / "gen_std.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def test_std_py_is_generated_from_recipes_json():
    gen = _gen()
    recipes = json.loads((ROOT / "src" / "datars" / "_recipes.json").read_text())
    assert (ROOT / "src" / "datars" / "std.py").read_text() == gen.generate(recipes), "run python tools/gen_std.py"


def test_recipes_json_matches_the_cli(cli):
    """The checked-in snapshot is the CLI's std (refresh: python tools/gen_std.py --refresh)."""
    snapshot = json.loads((ROOT / "src" / "datars" / "_recipes.json").read_text())
    assert _cli.describe() == snapshot, "std changed: python tools/gen_std.py --refresh"


def test_every_recipe_has_a_function_with_its_params():
    for name, spec in std.RECIPES.items():
        fn = getattr(std, _gen().snake(name))
        params = set(inspect.signature(fn).parameters)
        want = {_gen().snake(p) for p in spec["params"]}
        assert params == want, name
        assert spec["doc"].split()[0] in fn.__doc__


def test_generated_calls_build_use_nodes():
    n = std.plot(data="t", x="a", y="b", x_type="linear", y_domain=[0, 1], children=[std.line(width=2)])
    d = n.to_dict()
    assert d["recipe"] == "@datars/std/plot"
    assert d["params"]["xType"] == "linear" and d["params"]["yDomain"] == [0, 1]
    assert d["params"]["children"][0]["params"] == {"width": 2}
    assert "zero" not in d["params"]  # None = the recipe's default: not written
    assert std.span(from_=1, to=2).to_dict()["params"] == {"from": 1, "to": 2}


def test_every_recipe_has_a_place():
    """New std recipes must get a decision: a plot mark, a chart function, or a helper."""
    charts = {"plot", "pie", "treemap", "map", "cloud", "sankey", "funnel", "waffle", "calendar", "stripes", "hemicycle", "facet"}
    helpers = {"card", "title", "legend", "slider", "basemap", "attribution", "symbols", "geoPoints", "geoLines", "sparkline",
               "route", "track", "dotDensity"}
    unplaced = set(std.RECIPES) - PLOT_MARKS - charts - helpers
    assert not unplaced, f"place these recipes (chart.PLOT_MARKS or a dr.* function): {sorted(unplaced)}"
    for name in charts - {"facet", "plot"}:
        assert hasattr(dr, name) or name == "map", name


def test_std_reference_mentions_every_recipe():
    ref = ROOT.parent.parent / "docs" / "reference" / "std.md"
    if not ref.is_file():
        pytest.skip("no docs/reference/std.md")
    text = ref.read_text()
    for name in std.RECIPES:
        assert f"## {name}" in text, name
