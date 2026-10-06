"""The chart functions build the documents a data scientist means (and valid IR)."""

from __future__ import annotations

import pytest

import datars as dr
from helpers import find, find_all, plot_params

pd = pytest.importorskip("pandas")
np = pytest.importorskip("numpy")


def marks(doc):
    return plot_params(doc)["children"]


def table(doc, name):
    return doc["data"][name]["values"]


# ---- bar -----------------------------------------------------------------------------------------


def test_bar_vertical(votes, validate):
    c = dr.bar(votes, x="party", y="share", title="Votes")
    d = c.to_dict()
    validate(d)
    p = plot_params(d)
    assert (p["xType"], p["yType"], p["x"], p["y"], p["title"]) == ("band", "linear", "party", "share", "Votes")
    assert [m["recipe"] for m in p["children"]] == ["@datars/std/bar"]
    src = d["data"][p["data"]]
    assert src["key"] == ["party"]  # a bar is its category: what morphs into what
    assert set(src["values"]) == {"party", "share"}
    assert p["subtitle"] == "share"  # the y axis title goes under the plot title, not over it
    assert d["signals"]["highlight"] == {"type": "keyset", "default": []}
    assert p["children"][0]["params"]["selected"] == "highlight"


def test_bar_horizontal_is_inferred(votes):
    p = plot_params(dr.bar(votes, x="share", y="party").to_dict())
    assert (p["xType"], p["yType"]) == ("linear", "band")


def test_bar_only_serializes_used_columns(votes):
    wide = votes.assign(extra=1, other="z")
    d = dr.bar(wide, x="party", y="share").to_dict()
    assert set(next(iter(d["data"].values()))["values"]) == {"party", "share"}


def test_bar_series_stack_by_default_and_group_on_request(validate):
    df = pd.DataFrame({"q": ["Q1", "Q1", "Q2", "Q2"], "region": ["N", "S", "N", "S"], "sales": [1, 2, 3, 4]})
    st = dr.bar(df, x="q", y="sales", color="region")
    validate(st.to_dict())
    assert marks(st.to_dict())[0]["recipe"] == "@datars/std/stacked"
    assert plot_params(st.to_dict())["legend"] is True
    gr = dr.bar(df, x="q", y="sales", color="region", grouped=True)
    assert marks(gr.to_dict())[0]["recipe"] == "@datars/std/grouped"
    nm = dr.bar(df, x="q", y="sales", color="region", normalize=True)
    assert marks(nm.to_dict())[0]["params"]["offset"] == "expand"
    src = next(iter(st.to_dict()["data"].values()))
    assert src["key"] == ["q", "region"]


def test_bar_sort(votes):
    d = dr.bar(votes.sample(frac=1, random_state=3), x="party", y="share", sort="desc").to_dict()
    assert table(d, plot_params(d)["data"])["share"] == sorted(votes.share, reverse=True)


def test_bar_race_frames(validate):
    df = pd.DataFrame({"year": [2000] * 3 + [2010] * 3, "name": list("abc") * 2, "v": [1, 2, 3, 3, 1, 2]})
    c = dr.bar(df, x="v", y="name", frame="year", sort="desc", top=2)
    d = c.to_dict()
    validate(d)
    p = plot_params(d)
    t = d["tables"][p["data"]]
    ops = [o["op"] for o in t["ops"]]
    assert ops == ["filter", "aggregate", "sort", "top"]
    assert "yDomain" not in p  # categories reorder each frame
    assert p["xDomain"] == {"data": t["from"], "field": "v"}  # the value axis holds still
    assert c.state_names == ["2000", "2010"]
    assert d["program"]["drivers"][0] == "autoplay"


def test_wide_frame_bars():
    df = pd.DataFrame({"a": [1, 2], "b": [3, 4]}, index=pd.Index(["x", "y"], name="k"))
    d = dr.bar(df).to_dict()
    p = plot_params(d)
    assert (p["x"], p["y"], p["color"]) == ("k", "value", "series")


# ---- line / area -----------------------------------------------------------------------------------


def test_line_wide_dataframe_like_df_plot(validate):
    ts = pd.DataFrame({"a": [1.0, 2.0, 3.0], "b": [3.0, 2.0, 1.0]}, index=pd.date_range("2024-01-01", periods=3, name="date"))
    c = dr.line(ts)
    d = c.to_dict()
    validate(d)
    p = plot_params(d)
    assert (p["x"], p["y"], p["color"], p["xType"]) == ("date", "value", "series", "time")
    assert marks(d)[0]["params"]["labels"] is True  # few series: labelled at their ends
    src = d["data"][p["data"]]
    assert src["types"] == {"date": "date"} and src["key"] == ["series", "date"]


def test_line_many_series_get_a_legend():
    df = pd.DataFrame({"x": list(range(3)) * 12, "s": [f"s{i}" for i in range(12) for _ in range(3)], "y": range(36)})
    d = dr.line(df, x="x", y="y", color="s").to_dict()
    assert "labels" not in marks(d)[0]["params"] and plot_params(d)["legend"] is True


def test_area_stacked_uses_a_stack_table(validate):
    df = pd.DataFrame({"x": [1, 1, 2, 2], "s": ["a", "b", "a", "b"], "y": [1, 2, 3, 4]})
    d = dr.area(df, x="x", y="y", color="s").to_dict()
    validate(d)
    p = plot_params(d)
    assert d["tables"][p["data"]]["ops"][0]["op"] == "stack"
    assert marks(d)[0]["params"]["y0"] == "y0" and p["y"] == "y1"


# ---- scatter -----------------------------------------------------------------------------------------


def test_scatter_sizes_are_areas():
    df = pd.DataFrame({"x": [1, 2, 3], "y": [1, 2, 3], "pop": [100, 25, 0]})
    d = dr.scatter(df, x="x", y="y", size="pop", size_max=20).to_dict()
    r = table(d, plot_params(d)["data"])["_r"]
    assert r == [20.0, 10.0, 1.0]
    assert marks(d)[0]["params"]["r"] == {"expr": "d._r"}


def test_gapminder_frames(gap, validate):
    c = dr.scatter(gap, x="gdp per capita", y="life", size="pop", color="continent", frame="year", key="country")
    d = c.to_dict()
    validate(d)
    p = plot_params(d)
    t = d["tables"][p["data"]]
    assert t["ops"][0] == {"op": "filter", "expr": {"expr": "d.year == frame_year"}}
    assert t["ops"][1]["op"] == "aggregate" and t["ops"][1]["groupby"] == ["country"]  # dots keyed by country: they glide
    assert p["x"] == "gdp_per_capita" and p["xLabel"] == "gdp per capita"
    assert p["xDomain"] == {"data": t["from"], "field": "gdp_per_capita"}  # axes hold still over the years
    assert d["signals"]["frame_year"] == {"type": "num", "default": 2000}
    assert c.state_names == ["2000", "2010", "2020"]
    assert [s["set"] for s in d["program"]["states"]] == [{"frame_year": y} for y in (2000, 2010, 2020)]
    label = find(d["scene"], lambda n: n.get("key") == "frame-label")
    assert label["number"] == {"value": {"expr": "frame_year"}, "format": "d"}  # the year counts between frames
    assert any(r.get("matcher") == "by-key" for r in d["motion"]["rules"])


def test_animated_scatter_needs_a_key(gap):
    with pytest.raises(TypeError, match="key="):
        dr.scatter(gap.drop(columns=["continent"]), x="life", y="pop", frame="year")


def test_scatter_log_axes():
    p = plot_params(dr.scatter({"x": [1, 10], "y": [1, 100]}, x="x", y="y", log_x=True, log_y=True).to_dict())
    assert (p["xType"], p["yType"]) == ("log", "log")


# ---- hist / heatmap ------------------------------------------------------------------------------


def test_hist_nice_bins(validate):
    vals = np.linspace(0, 9.9, 100)
    d = dr.hist({"v": vals}, x="v", bins=5).to_dict()
    validate(d)
    t = table(d, plot_params(d)["data"])
    assert t["bin"] == ["0", "2", "4", "6", "8"]
    assert sum(t["count"]) == 100
    assert t["range"][0] == "0–2"


def test_hist_exact_numpy_bins_and_groups():
    vals = [1, 2, 2, 3, 3, 3]
    d = dr.hist({"v": vals}, x="v", bins=3, nice=False).to_dict()
    assert table(d, plot_params(d)["data"])["count"] == list(np.histogram(vals, bins=3)[0])
    g = dr.hist({"v": vals, "g": ["a", "b"] * 3}, x="v", color="g", bins=3, nice=False).to_dict()
    assert marks(g)[0]["recipe"] == "@datars/std/stacked"


def test_heatmap(validate):
    df = pd.DataFrame({"x": ["a", "b", "a", "b"], "y": ["p", "p", "q", "q"], "v": [1, 2, 3, 4]})
    d = dr.heatmap(df, x="x", y="y", value="v").to_dict()
    validate(d)
    p = plot_params(d)
    assert (p["xType"], p["yType"], p["colorType"]) == ("band", "band", "sequential")
    assert marks(d)[0]["recipe"] == "@datars/std/cell"


# ---- standalone recipes --------------------------------------------------------------------------


def test_pie_and_donut(votes, validate):
    d = dr.pie(votes, names="party", values="share", title="Votes").to_dict()
    validate(d)
    pie = find(d["scene"], lambda n: n.get("recipe") == "@datars/std/pie")
    assert pie["params"]["category"] == "party" and pie["key"] == "chart"
    assert find(d["scene"], lambda n: n.get("recipe") == "@datars/std/title")["params"]["text"] == "Votes"
    dn = find(dr.donut(votes, names="party", values="share").to_dict()["scene"], lambda n: n.get("recipe") == "@datars/std/pie")
    assert dn["params"]["inner"] == 0.6 and dn["params"]["total"] is True


def test_treemap(votes, validate):
    d = dr.treemap(votes, names="party", values="share").to_dict()
    validate(d)
    assert find(d["scene"], lambda n: n.get("recipe") == "@datars/std/treemap")


def test_map_with_the_countries_atlas(validate):
    df = pd.DataFrame({"iso": ["SWE", "NOR"], "v": [1, 2]})
    d = dr.map(df, key="iso", value="v").to_dict()
    validate(d)
    m = find(d["scene"], lambda n: n.get("recipe") == "@datars/std/map")
    assert d["data"][m["params"]["source"]] == {"atlas": "countries"}
    assert m["params"]["key"] == "iso" and m["params"]["selected"] == "highlight"


def test_map_from_geojson_and_geo_interface():
    fc = {"type": "FeatureCollection", "features": [{"type": "Feature", "id": "a", "properties": {"name": "A"},
                                                     "geometry": {"type": "Point", "coordinates": [0, 0]}}]}

    class Geo:
        __geo_interface__ = fc

    for src in (fc, Geo()):
        d = dr.map({"k": ["a"], "v": [1]}, key="k", value="v", source=src, id="name").to_dict()
        m = find(d["scene"], lambda n: n.get("recipe") == "@datars/std/map")
        assert d["data"][m["params"]["source"]] == {"geojson": fc, "id": "name"}


def test_cloud_colours_from_the_row(validate):
    df = pd.DataFrame({"x": np.arange(1000.0), "y": np.arange(1000.0), "g": ["a", "b"] * 500, "v": np.arange(1000.0)})
    d = dr.cloud(df, x="x", y="y", color="g").to_dict()
    validate(d)
    cloud = find(d["scene"], lambda n: n.get("recipe") == "@datars/std/cloud")
    # Level-of-detail points evaluate per row: palette slots by value, still theme inks.
    assert cloud["params"]["fill"] == {"expr": 'd.g == "a" ? "$categorical[0]" : d.g == "b" ? "$categorical[1]" : "$muted"'}
    assert cloud["params"]["name"] == "1,000 points"
    ramp = find(dr.cloud(df, x="x", y="y", color="v").to_dict()["scene"], lambda n: n.get("recipe") == "@datars/std/cloud")
    assert ramp["params"]["fill"]["expr"].count("$sequential[") == 7


@pytest.mark.parametrize("fn,kw", [
    (dr.sankey, dict(source="a", target="b", value="n")),
    (dr.funnel, dict(stage="a", value="n")),
    (dr.waffle, dict(category="a", value="n")),
    (dr.hemicycle, dict(category="a", value="n")),
])
def test_thin_wrappers(fn, kw, validate):
    df = pd.DataFrame({"a": ["x", "y"], "b": ["y", "z"], "n": [3, 4]})
    d = fn(df, **kw, title="T").to_dict()
    validate(d)
    node = find(d["scene"], lambda n: n.get("recipe", "").endswith("/" + fn.__name__))
    assert node is not None and set(kw) <= set(node["params"])


def test_waterfall_and_swarm_sit_in_a_plot(validate):
    df = pd.DataFrame({"item": ["start", "a", "end"], "delta": [10, -2, 8], "total": [True, False, True]})
    d = dr.waterfall(df, x="item", y="delta", total="total").to_dict()
    validate(d)
    assert marks(d)[0]["recipe"] == "@datars/std/waterfall"
    s = dr.swarm(pd.DataFrame({"v": [1.0, 2.0, 2.1]}), x="v").to_dict()
    assert marks(s)[0]["recipe"] == "@datars/std/swarm" and plot_params(s)["xType"] == "linear"


# ---- matplotlib-style ------------------------------------------------------------------------------


def test_plot_like_matplotlib(validate):
    c = dr.plot([0, 1, 2], [0, 1, 4])
    validate(c.to_dict())
    p = plot_params(c.to_dict())
    assert (p["x"], p["y"], p["xType"]) == ("x", "y", "linear")
    assert plot_params(dr.plot([3, 1, 2]).to_dict())["x"] == "x"
    s = pd.Series([1, 2, 3], index=pd.Index([10, 20, 30], name="t"), name="v")
    assert (plot_params(dr.plot(s).to_dict())["x"], plot_params(dr.plot(s).to_dict())["y"]) == ("t", "v")
    assert marks(dr.plot([1, 2], [3, 4], kind="scatter").to_dict())[0]["recipe"] == "@datars/std/point"
    with pytest.raises(ValueError):
        dr.plot([1], [1], kind="violin")


# ---- composition ---------------------------------------------------------------------------------


def test_overlay_shares_scales(votes, validate):
    target = pd.DataFrame({"party": votes.party, "target": [35, 18, 18, 8, 7, 6, 6, 5]})
    c = dr.bar(votes, x="party", y="share") + dr.line(target, x="party", y="target")
    d = c.to_dict()
    validate(d)
    p = plot_params(d)
    ln = p["children"][1]
    assert ln["recipe"] == "@datars/std/line" and ln["params"]["y"] == "target" and ln["params"]["stroke"] == "$categorical[1]"
    assert ln["params"]["curve"] == "linear"
    assert p["yDomain"] == {"values": [0, 35]}
    assert len(d["data"]) == 2


def test_annotations(votes, validate):
    c = dr.bar(votes, x="party", y="share").rule(y=4, label="Threshold").span(y=(0, 4)).annotate("Largest", x="S", y=30.3)
    d = c.to_dict()
    validate(d)
    kinds = [m["recipe"].rsplit("/", 1)[-1] for m in marks(d)]
    assert kinds == ["bar", "rule", "span", "annotate"]
    note = marks(d)[3]["params"]
    assert note["x"] == {"expr": 'scale.x("S") + scale.x.bandwidth() / 2'} and note["y"] == {"expr": "scale.y(30.3)"}
    assert marks(d)[2]["params"]["from"] == 0  # `from_` → `from`
    with pytest.raises(TypeError):
        dr.pie(votes, names="party", values="share").rule(y=1)


def test_facet(gap, validate):
    d = dr.line(gap, x="year", y="life", color="country", facet="continent", facet_columns=2).to_dict()
    validate(d)
    f = find(d["scene"], lambda n: n.get("recipe") == "@datars/std/facet")
    assert f["params"]["by"] == "continent" and f["params"]["columns"] == 2
    assert f["params"]["chart"]["recipe"] == "@datars/std/plot"
    with pytest.raises(KeyError, match="facet='continent'"):
        dr.line(gap, x="year", y="life", color="country").facet("continent")


def test_story_morphs_between_charts(votes, validate):
    s = dr.story(dr.bar(votes, x="party", y="share"), (dr.pie(votes, names="party", values="share"), "Slices."), autoplay=True)
    d = s.to_dict()
    validate(d)
    charts = find_all(d["scene"], lambda n: n.get("key") == "chart")
    assert [c["when"] for c in charts] == [{"expr": 'view == "0"'}, {"expr": 'view == "1"'}]
    assert [st["set"]["view"] for st in d["program"]["states"]] == ["0", "1"]
    assert d["program"]["states"][1]["narration"] == {"text": "Slices."}
    assert find(d["scene"], lambda n: n.get("key") == "caption")  # one card narrates every step
    assert len(d["data"]) == 1  # the same rows, once
    assert {"select": {"role": "datum"}, "matcher": "by-key"} in d["motion"]["rules"]
    assert d["program"]["drivers"][0] == "autoplay"


def test_story_expands_charts_with_their_own_steps(gap, votes):
    anim = dr.scatter(gap, x="gdp per capita", y="life", color="continent", frame="year", key="country")
    s = dr.story(anim, dr.bar(votes, x="party", y="share"))
    assert s.state_names == ["2000", "2010", "2020", "2"]


def test_layouts_share_signals(votes):
    both = dr.bar(votes, x="party", y="share") | dr.pie(votes, names="party", values="share")
    d = both.to_dict()
    views = find(d["scene"], lambda n: n.get("key") == "views")
    assert views["layout"]["type"] == "columns" and len(views["children"]) == 2
    assert d["size"]["width"] == 1400 and "highlight" in d["signals"]
    stacked = dr.column(dr.bar(votes, x="party", y="share"), dr.bar(votes, x="share", y="party"))
    assert find(stacked.to_dict()["scene"], lambda n: n.get("key") == "views")["layout"]["type"] == "rows"


# ---- states, looks ---------------------------------------------------------------------------------


def test_steps_highlight_and_narrate(votes, validate):
    c = (dr.bar(votes, x="party", y="share")
         .step("All")
         .step("Big three", highlight=["S", "SD", "M"], title="Three parties", text="…hold most seats.")
         .autoplay(hold=2, loop=True)
         .motion(duration=0.9, easing="cubic-in-out", stagger=0.3))
    d = c.to_dict()
    validate(d)
    st = d["program"]["states"]
    assert st[0]["set"] == {"highlight": []} and st[1]["set"] == {"highlight": ["S", "SD", "M"]}
    assert st[1]["narration"] == {"title": "Three parties", "text": "…hold most seats."}
    assert all(s["hold"] == 2 for s in st)
    assert d["program"]["edges"] == [{"from": "Big three", "on": "next", "to": "All"}]
    assert {"select": {"role": "datum"}, "duration": 0.9, "easing": "cubic-in-out",
            "choreo": {"type": "stagger", "order": "data", "spread": 0.3}} in d["motion"]["rules"]
    assert c.states({"A": {}, "B": {"highlight": ["S"]}}).state_names == ["All", "Big three", "A", "B"]


def test_frame_steps_replace_the_default(gap):
    c = dr.scatter(gap, x="gdp per capita", y="life", key="country", frame="year").step("then", frame=2000).step("now", frame=2020)
    assert c.state_names == ["then", "now"]


def test_scrolly_and_signals(votes):
    d = dr.bar(votes, x="party", y="share").signal("k", 3).step("a").scrolly().to_dict()
    assert d["signals"]["k"] == {"type": "num", "default": 3}
    assert d["program"]["drivers"] == [{"scroll": "scrub"}, "keys"]


def test_themes_and_key_metadata(votes, validate):
    c = dr.bar(votes, x="party", y="share", color="party", theme="noir", colors={"S": "#e8112d"}, names={"S": "Social Democrats"})
    d = c.to_dict()
    validate(d)
    assert d["theme"] == {"use": "datars/noir"}
    assert d["keys"]["S"] == {"color": "#e8112d", "name": "Social Democrats"}
    brand = dr.theme("noir", accent="#0a7d6b", font_title=dr.font.google("Source Serif 4", weight=600), dark={"paper": "#000"})
    d2 = c.update(theme=brand).to_dict()
    assert d2["theme"]["themes"][0]["tokens"]["font.title"] == {"google": "Source Serif 4", "weight": 600}
    assert d2["theme"]["themes"][0]["extends"] == "datars/noir"
    assert dr.bar(votes, x="party", y="share", theme={"accent": "#f00"}).to_dict()["theme"] == {"use": "datars/neutral", "tokens": {"accent": "#f00"}}


def test_update_and_immutability(votes):
    a = dr.bar(votes, x="party", y="share")
    b = a.update(title="T", width=500, height=300, y_domain=[0, 50], legend=False)
    assert a.to_dict()["size"] == {"width": 720, "height": 440} and "title" not in plot_params(a.to_dict())
    p = plot_params(b.to_dict())
    assert b.size == (500, 300) and p["title"] == "T" and p["yDomain"] == [0, 50] and p["legend"] is False
    with pytest.raises(TypeError, match="no parameter"):
        a.update(nonsense=1)
    a2 = a.rule(y=1)
    assert len(marks(a.to_dict())) == 1 and len(marks(a2.to_dict())) == 2


def test_std_nodes_accept_dataframes(votes, validate):
    node = dr.std.pie(data=votes.rename(columns={"share": "vote share"}), category="party", value="vote share")
    d = dr.chart(node, title="Pie").to_dict()
    validate(d)
    pie = find(d["scene"], lambda n: n.get("recipe") == "@datars/std/pie")
    assert pie["params"]["value"] == "vote_share" and pie["params"]["data"] in d["data"]
    assert node._repr_mimebundle_ is not None
    b = dr.chart(dr.std.bar(data=votes, x="party", y="share"))
    assert plot_params(b.to_dict())["x"] == "party"


def test_expressions_and_raw_nodes(votes, validate):
    c = dr.bar(votes, x="party", y="share").add(dr.text(dr.e("'n = ' + format(table.count('x'), 'd')"), (0, 0), key="n"))
    d = c.to_dict()
    validate(d)
    assert find(d["scene"], lambda n: n.get("key") == "n")["text"] == {"expr": "'n = ' + format(table.count('x'), 'd')"}
    assert dr.field("a b").src == 'd["a b"]' and dr.field("ab").src == "d.ab"


def test_polars_frames(validate):
    pl = pytest.importorskip("polars")
    df = pl.DataFrame({"party": ["S", "SD", "M"], "share": [30.3, 20.5, 19.1], "extra": [1, 2, 3]})
    d = dr.bar(df, x="party", y="share").to_dict()
    validate(d)
    src = next(iter(d["data"].values()))
    assert src["values"] == {"party": ["S", "SD", "M"], "share": [30.3, 20.5, 19.1]} and src["key"] == ["party"]
    wide = pl.DataFrame({"t": [1, 2, 3], "a": [1.0, 2.0, 3.0], "b": [3.0, 2.0, 1.0]})
    p = plot_params(dr.line(wide, x="t").to_dict())
    assert (p["x"], p["y"], p["color"]) == ("t", "value", "series")
