"""The anywidget ChartWidget, on the Python side (the browser side: tools/check_browser.py)."""

from __future__ import annotations

import json

import pytest

pytest.importorskip("anywidget")

import datars as dr  # noqa: E402
from datars import _runtime  # noqa: E402
from datars.widget import ChartWidget  # noqa: E402


def test_traits_from_the_chart(votes):
    c = dr.bar(votes, x="party", y="share", title="Votes").step("a").step("b", highlight=["S"])
    w = ChartWidget(c, state="b")
    assert json.loads(w.doc) == c.to_dict()
    assert (w.width, w.height, w.alt, w.states, w.state) == (720, 440, "Votes", ["a", "b"], "b")
    assert "export default" in w._esm and "runtimeFromFiles" in w._esm


def test_assigning_a_chart_morphs(votes):
    w = dr.bar(votes, x="party", y="share").widget()
    w.chart = dr.pie(votes, names="party", values="share").step("x")
    assert json.loads(w.doc)["scene"]["children"][0]["recipe"] == "@datars/std/pie"
    assert w.states == ["x"]


def test_runtime_is_sent_on_request(votes, runtime, monkeypatch, no_cli):
    w = dr.bar(votes, x="party", y="share").widget()
    sent = []
    monkeypatch.setattr(w, "send", lambda content, buffers=None: sent.append((content, buffers)))
    w._on_msg(w, {"type": "need-runtime"}, [])
    (content, buffers), = sent
    assert content["type"] == "runtime" and content["paths"] == list(_runtime.FILES)
    assert len(buffers) == len(_runtime.FILES) and buffers[2][:4] == b"\0asm"
    assert w.poster == ""  # no CLI, no poster; the runtime is never part of the widget's state


def test_missing_runtime_is_reported(votes, monkeypatch):
    w = dr.bar(votes, x="party", y="share").widget()
    sent = []
    monkeypatch.setattr(w, "send", lambda content, buffers=None: sent.append(content))
    monkeypatch.setattr(_runtime, "runtime_dir", lambda: None)
    w._on_msg(w, {"type": "need-runtime"}, [])
    assert sent[0]["type"] == "runtime" and "isn't installed" in sent[0]["error"]


def test_clicks_reach_python(votes):
    w = dr.bar(votes, x="party", y="share").widget()
    got = []
    w.on_click(got.append)
    w._on_msg(w, {"type": "pick", "x": 90, "y": 200, "hits": [{"path": "p", "role": "datum", "label": "S: 30.3"}],
                  "row": {"table": "data_x", "index": 0, "fields": {"party": "S", "share": 30.3}}}, [])
    assert got[0]["fields"] == {"party": "S", "share": 30.3} and got[0]["hits"][0]["label"] == "S: 30.3"


def test_signals_and_events(votes, monkeypatch):
    w = dr.bar(votes, x="party", y="share").widget()
    w.set_signal("highlight", ["S"])
    assert w.signals == {"highlight": ["S"]}
    sent = []
    monkeypatch.setattr(w, "send", lambda content, buffers=None: sent.append(content))
    w.next()
    w.prev()
    assert sent == [{"type": "event", "name": "next"}, {"type": "event", "name": "prev"}]


def test_widget_renderer_bundle(votes, no_cli):
    dr.options.renderer = "widget"
    c = dr.bar(votes, x="party", y="share")
    b = c._repr_mimebundle_()
    assert "application/vnd.jupyter.widget-view+json" in b
    assert "text/html" in b  # the fallback where widgets can't render (exports, GitHub)
    assert b["application/vnd.jupyter.widget-view+json"]["model_id"] == c._last_widget.model_id


def test_widgets_carry_the_static_poster(votes, cli):
    w = dr.bar(votes, x="party", y="share").widget()
    assert w.poster.startswith("data:image/svg+xml;base64,")  # shown while the runtime arrives, and without a kernel
