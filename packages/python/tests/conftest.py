"""Shared fixtures. No test touches the network; CLI, runtime, jsonschema and anywidget tests skip
when those aren't installed."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

import datars as dr
from datars import _cli, _display, _runtime

REPO_PUBLIC = Path(__file__).resolve().parents[3]  # public/
SCHEMA = REPO_PUBLIC / "docs" / "reference" / "ir.schema.json"


@pytest.fixture(autouse=True)
def _fresh_options(monkeypatch):
    """Each test starts from default display options (and no stray renderer env)."""
    monkeypatch.setattr(_display, "options", _display.Options())
    monkeypatch.setattr(dr, "options", _display.options, raising=False)
    yield


@pytest.fixture
def cli():
    exe = _cli.find_cli()
    if not exe:
        pytest.skip("no datars CLI (set DATARS_CLI)")
    return exe


@pytest.fixture
def no_cli(monkeypatch):
    monkeypatch.setattr(_cli, "find_cli", lambda: None)
    monkeypatch.setattr(_display, "_warned_cli", True)


@pytest.fixture
def runtime():
    if not _runtime.available():
        pytest.skip("no web runtime (tools/vendor_runtime.py or DATARS_WEB_DIST)")
    return _runtime.runtime_dir()


@pytest.fixture(scope="session")
def validate():
    """Validate a document against the IR's JSON Schema (``datars schema``)."""
    jsonschema = pytest.importorskip("jsonschema")
    if not SCHEMA.is_file():
        pytest.skip("no ir.schema.json")
    schema = json.loads(SCHEMA.read_text())
    v = jsonschema.Draft202012Validator(schema)

    def check(doc: dict) -> None:
        errors = sorted(v.iter_errors(doc), key=lambda e: list(e.path))
        assert not errors, "\n".join(f"{list(e.path)}: {e.message}" for e in errors[:5])

    return check


@pytest.fixture
def votes():
    pd = pytest.importorskip("pandas")
    return pd.DataFrame({"party": ["S", "SD", "M", "V", "C", "KD", "MP", "L"],
                         "share": [30.3, 20.5, 19.1, 6.8, 6.7, 5.3, 5.1, 4.6]})


@pytest.fixture
def gap():
    pd = pytest.importorskip("pandas")
    rows = []
    countries = [("SWE", "Europe"), ("USA", "Americas"), ("CHN", "Asia"), ("NGA", "Africa")]
    for i, (c, k) in enumerate(countries):
        for j, y in enumerate([2000, 2010, 2020]):
            rows.append({"country": c, "continent": k, "year": y, "gdp per capita": 1000.0 * (i + 1) * (1 + j),
                         "life": 60 + i * 3 + j * 2, "pop": 10 + 20 * i + 5 * j})
    return pd.DataFrame(rows)
