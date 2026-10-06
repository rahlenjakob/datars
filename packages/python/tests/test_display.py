"""Notebook outputs: MIME bundles per renderer, the HTML that goes live, the static fallback."""

from __future__ import annotations

import base64
import json
import re

import pytest

import datars as dr
from datars import _display, _runtime


def test_html_output_without_cli_is_live_only(votes, no_cli):
    html = _display.output_html(dr.bar(votes, x="party", y="share"))
    assert 'class="datars-poster"' not in html
    assert '<script type="application/json" class="datars-doc">' in html
    assert "mountOutput(" in html and _display.DEFAULT_RUNTIME_URL in html
    doc = re.search(r'class="datars-doc">(.*?)</script>', html, re.S).group(1)
    assert json.loads(doc)["datars"] == 1


def test_documents_cant_break_out_of_their_script_tag(no_cli):
    evil = dr.bar({"k": ["</script><script>alert(1)</script>", "<!--"], "v": [1, 2]}, x="k", y="v")
    html = _display.output_html(evil)
    body = re.search(r'class="datars-doc">(.*?)</script>', html, re.S).group(1)
    assert "</script" not in body and "<!--" not in body
    assert json.loads(body)["data"]  # still valid JSON (<\/ is a JSON escape)


def test_mimebundle_html_renderer(votes, no_cli):
    dr.options.renderer = "html"
    b = dr.bar(votes, x="party", y="share", title="Votes")._repr_mimebundle_()
    assert set(b) == {"text/plain", "text/html"}
    assert "Votes" in b["text/plain"] and "8 rows" in b["text/plain"]


def test_mimebundle_json_renderer(votes, no_cli):
    dr.options.renderer = "json"
    b = dr.bar(votes, x="party", y="share")._repr_mimebundle_()
    assert b["application/json"]["scene"]["kind"] == "group"


def test_embed_limit(votes, no_cli):
    dr.options.embed_limit = 10
    html = _display.output_html(dr.bar(votes, x="party", y="share"))
    assert "datars-doc" not in html and "too large" in html


def test_static_fallback_with_the_cli(votes, cli):
    dr.options.renderer = "html"
    b = dr.bar(votes, x="party", y="share")._repr_mimebundle_()
    assert set(b) == {"text/plain", "text/html", "image/png"}
    assert base64.b64decode(b["image/png"])[:8] == b"\x89PNG\r\n\x1a\n"
    m = re.search(r'class="datars-poster" alt="[^"]*" src="data:image/svg\+xml;base64,([^"]+)"', b["text/html"])
    assert m and base64.b64decode(m.group(1)).startswith(b"<svg")


def test_static_renderer_is_images_only(votes, cli):
    dr.options.renderer = "static"
    b = dr.bar(votes, x="party", y="share")._repr_mimebundle_()
    assert set(b) == {"text/plain", "image/png"}


def test_dense_charts_get_a_png_poster(cli):
    import numpy as np

    rng = np.random.default_rng(0)
    c = dr.scatter({"x": rng.random(30_000), "y": rng.random(30_000)}, x="x", y="y")
    images = _display.static_images(c)
    assert len(images["svg"]) > 250_000
    assert _display.poster_src(images).startswith("data:image/png;base64,")


def test_svg_fallback_is_the_poster_only(votes, cli):
    import numpy as np

    dr.options.renderer = "html"
    dr.options.fallback = "svg"
    b = dr.bar(votes, x="party", y="share")._repr_mimebundle_()
    assert "image/png" not in b and "data:image/svg+xml;base64," in b["text/html"]
    rng = np.random.default_rng(0)
    dense = dr.scatter({"x": rng.random(30_000), "y": rng.random(30_000)}, x="x", y="y")._repr_mimebundle_()
    assert "image/png" not in dense and "data:image/png;base64," in dense["text/html"]


def test_fallback_off_means_no_cli_calls(votes, monkeypatch):
    dr.options.fallback = None
    monkeypatch.setattr("datars._cli.render", lambda *a, **k: pytest.fail("rendered"))
    assert _display.static_images(dr.bar(votes, x="party", y="share")) == {}


def test_standalone_page_inlines_the_runtime(votes, runtime, no_cli):
    html = dr.bar(votes, x="party", y="share").to_html()
    for f in _runtime.FILES:
        assert f'data-datars-file="{f}"' in html
    assert '"inline": true' in html and '"page": true' in html
    assert html.startswith("<!doctype html>")


def test_standalone_page_by_url(votes, no_cli):
    html = dr.bar(votes, x="party", y="share").to_html(runtime="https://example.org/datars.js")
    assert 'type="application/octet-stream" data-datars-file="wasm' not in html and "https://example.org/datars.js" in html


def test_save_html_and_json(tmp_path, votes, no_cli):
    c = dr.bar(votes, x="party", y="share")
    c.save(str(tmp_path / "c.json"))
    assert json.loads((tmp_path / "c.json").read_text()) == c.to_dict()
    c.save(str(tmp_path / "c.html"))
    page = (tmp_path / "c.html").read_text()
    assert page.startswith("<!doctype html>") and "mountOutput" in page and 'class="datars-doc"' in page


def test_loader_speaks_the_runtime_contract():
    js = (_display._STATIC / "loader.js").read_text()
    for needle in ("runtimeFromFiles", "runtimeFromUrl", "runtimeFromPage", "mountOutput", '"wasm/datars_host_web_bg.wasm"',
                   ".assets", "runtimeAssets", '"./wasi_shim.js"', "setDocument", '"pick"', '"state"'):
        assert needle in js, needle


def test_show_from_a_script_opens_a_page(votes, no_cli, monkeypatch):
    opened = []
    monkeypatch.setattr(_display, "_in_kernel", lambda: False)
    monkeypatch.setattr(_display.webbrowser, "open", opened.append)
    dr.bar(votes, x="party", y="share").show()
    (url,) = opened
    assert url.startswith("file://") and url.endswith(".html")


def test_auto_renderer(monkeypatch):
    monkeypatch.setattr(_display, "_in_kernel", lambda: False)
    assert _display.renderer() == "html"
    monkeypatch.setattr(_display, "_in_kernel", lambda: True)
    monkeypatch.setattr(_display, "_widget_available", lambda: True)
    assert _display.renderer() == "widget"
    monkeypatch.setattr(_display, "_widget_available", lambda: False)
    assert _display.renderer() == "html"
