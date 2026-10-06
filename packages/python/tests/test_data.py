"""DataFrames → the IR's columnar sources: types, nulls, names, keys, content names."""

from __future__ import annotations

import datetime as dt
import json
import math
import warnings

import pytest

from datars import Data, data
from datars._data import BOOL, DATE, NUM, STR, safe_name

pd = pytest.importorskip("pandas")
np = pytest.importorskip("numpy")


def test_pandas_dtypes_become_typed_columns():
    df = pd.DataFrame({
        "i": [1, 2, 3],
        "f": [1.5, float("nan"), 3.0],
        "n": pd.array([1, None, 3], dtype="Int64"),
        "b": [True, False, True],
        "d": pd.to_datetime(["2024-01-01", None, "2024-03-05"]),
        "c": pd.Categorical(["a", "b", "a"]),
        "s": ["x", None, "z"],
    })
    d = Data(df)
    assert d.kinds == {"i": NUM, "f": NUM, "n": NUM, "b": BOOL, "d": DATE, "c": STR, "s": STR}
    assert d.values["i"] == [1, 2, 3]
    assert d.values["f"] == [1.5, None, 3.0]
    assert d.values["n"] == [1, None, 3]
    assert d.values["d"] == ["2024-01-01", None, "2024-03-05"]
    assert d.values["c"] == ["a", "b", "a"]
    assert d.values["s"] == ["x", None, "z"]
    src = d.source()
    assert src["types"] == {"d": "date"}
    json.dumps(src, allow_nan=False)  # nothing JSON can't say


def test_whole_floats_in_int_columns_stay_ints_and_inf_is_null():
    d = Data({"a": np.array([1.0, np.inf, -np.inf]), "b": np.array([1, 2, 3], dtype=np.int32)})
    assert d.values["a"] == [1.0, None, None]
    assert d.values["b"] == [1, 2, 3]
    assert all(type(v) is int for v in d.values["b"])


def test_times_of_day_are_dropped_with_a_warning():
    df = pd.DataFrame({"t": pd.to_datetime(["2024-01-01 10:30", "2024-01-02 00:00"])})
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        d = Data(df)
    assert d.values["t"] == ["2024-01-01", "2024-01-02"]
    assert any("times of day" in str(x.message) for x in w)


def test_timezone_aware_dates():
    df = pd.DataFrame({"t": pd.to_datetime(["2024-06-01"]).tz_localize("Europe/Stockholm")})
    assert Data(df).values["t"] == ["2024-06-01"]


def test_object_columns_are_sniffed():
    df = pd.DataFrame({"nums": pd.Series([1, 2.5, None], dtype=object), "dates": [dt.date(2020, 1, 2), None, dt.date(2021, 3, 4)],
                       "mixed": pd.Series([1, "a", None], dtype=object)})
    d = Data(df)
    assert d.kinds["nums"] == NUM and d.values["nums"] == [1.0, 2.5, None]
    assert d.kinds["dates"] == DATE and d.values["dates"] == ["2020-01-02", None, "2021-03-04"]
    assert d.kinds["mixed"] == STR and d.values["mixed"] == ["1", "a", None]


def test_unsafe_column_names_are_renamed_but_remembered():
    df = pd.DataFrame({"GDP per capita": [1, 2], "2020": [3, 4], "ok": [5, 6]})
    d = Data(df)
    assert list(d.values) == ["GDP_per_capita", "_2020", "ok"]
    assert d.col("GDP per capita") == "GDP_per_capita"
    assert d.col("2020") == "_2020"
    assert safe_name("a b", {"a_b"}) == "a_b_2"


def test_columns_subset_and_index():
    df = pd.DataFrame({"a": [1, 2], "b": [3, 4], "wide": [0, 0]}, index=pd.Index(["x", "y"], name="k"))
    d = Data(df, columns=["k", "a"])
    assert list(d.values) == ["k", "a"]
    assert d.values["k"] == ["x", "y"]
    with pytest.raises(KeyError, match="no column"):
        Data(df, columns=["nope"])


def test_dict_records_and_numpy_structured():
    assert Data({"a": [1, 2], "b": ["x", "y"]}).kinds == {"a": NUM, "b": STR}
    r = Data([{"a": 1, "b": "x"}, {"a": 2, "c": True}])
    assert r.values == {"a": [1, 2], "b": ["x", None], "c": [None, True]}
    arr = np.array([(1, 2.0), (3, 4.0)], dtype=[("i", "i4"), ("f", "f8")])
    assert Data(arr).values == {"i": [1, 3], "f": [2.0, 4.0]}
    with pytest.raises(ValueError, match="expected 2"):
        Data({"a": [1, 2], "b": [1]})
    with pytest.raises(TypeError):
        Data(42)


def test_series():
    s = pd.Series([1, 2], name="v")
    assert Data(s).values == {"v": [1, 2]}


def test_polars():
    pl = pytest.importorskip("polars")
    df = pl.DataFrame({"a": [1, 2, None], "f": [1.5, None, float("nan")], "s": ["x", None, "z"],
                       "d": [dt.date(2024, 1, 1), None, dt.date(2024, 2, 1)], "b": [True, None, False]})
    d = Data(df)
    assert d.kinds == {"a": NUM, "f": NUM, "s": STR, "d": DATE, "b": BOOL}
    assert d.values["a"] == [1, 2, None]
    assert d.values["f"] == [1.5, None, None]
    assert d.values["d"] == ["2024-01-01", None, "2024-02-01"]
    json.dumps(d.source(), allow_nan=False)


def test_keys_must_identify_rows():
    d = data({"k": ["a", "b"], "v": [1, 2]}, key="k")
    assert d.source()["key"] == ["k"]
    with pytest.raises(ValueError, match="doesn't identify"):
        data({"k": ["a", "a"], "v": [1, 2]}, key="k")
    d2 = Data({"k": ["a", "a"], "j": [1, 2]})
    assert not d2.set_key(["k"], strict=False) and d2.key is None
    assert d2.set_key(["k", "j"], strict=False) and d2.key == ["k", "j"]


def test_content_addressed_names():
    a = Data({"x": [1, 2]})
    b = Data({"x": [1, 2]})
    c = Data({"x": [1, 3]})
    assert a.name == b.name != c.name
    assert a.name.startswith("data_")
    assert Data({"x": [1]}, name="mine").name == "mine"


def test_big_frames_convert_fast_enough():
    n = 200_000
    df = pd.DataFrame({"x": np.random.default_rng(1).random(n), "y": np.arange(n)})
    d = Data(df)
    assert len(d) == n and len(d.source_json()) > n
    assert not any(isinstance(v, float) and math.isnan(v) for v in d.values["x"][:100])
