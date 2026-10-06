"""Tables: DataFrames (pandas, polars), dicts, records and arrays → the IR's columnar sources.

A document carries its data as ``{"values": {"col": [...]}, "key": [...], "types": {...}}``
(``packages/sdk/src/data.ts``). Conversion is vectorised (numpy ``tolist``), keeps only the
columns a chart uses, and turns what JSON can't say (NaN, NaT, ±inf) into nulls. Column types:

- numbers (ints stay ints, so the JSON stays short), booleans;
- dates → ISO ``YYYY-MM-DD`` with ``types: {col: "date"}`` (the engine's date columns are days;
  a time of day is dropped with a warning — see the guide);
- everything else (strings, categoricals, periods, objects) → text.

Column names that aren't identifiers (``"GDP per capita"``) are renamed for the document
(``GDP_per_capita``) because recipes write expressions like ``d.<field>``; the high-level charts
keep the original name for axis titles.
"""

from __future__ import annotations

import datetime as _dt
import hashlib
import json
import math
import re
import warnings
from typing import Any, Iterable, Sequence

from ._ir import is_ident

NUM, STR, DATE, BOOL = "num", "str", "date", "bool"

_warned: set[str] = set()


def _warn_once(key: str, msg: str) -> None:
    if key not in _warned:
        _warned.add(key)
        warnings.warn(msg, stacklevel=4)


def safe_name(name: Any, taken: set[str]) -> str:
    """An identifier for a column name (recipes write ``d.<name>``)."""
    s = str(name)
    if not is_ident(s) or s.startswith("$"):
        s = re.sub(r"[^A-Za-z0-9_]", "_", s.strip()) or "col"
        if s[0].isdigit():
            s = "_" + s
    base, i = s, 2
    while s in taken:
        s = f"{base}_{i}"
        i += 1
    taken.add(s)
    return s


# ---- column conversion ---------------------------------------------------------------------------


def _digits() -> int | None:
    from ._display import options

    return options.digits


def round_to_digits(arr, digits: int):
    """Round a column to ``digits`` significant digits of its largest magnitude (one decimals
    count for the column, so it stays vectorised): 18-character floats become 6–8, and a
    million-point cloud's JSON shrinks by more than half. Small values in a column spanning many
    magnitudes lose relative precision — which is why it's opt-in."""
    import numpy as np

    finite = arr[np.isfinite(arr)]
    top = float(np.abs(finite).max()) if finite.size else 0.0
    if top == 0.0:
        return arr
    decimals = max(0, digits - 1 - int(math.floor(math.log10(top))))
    return np.round(arr, decimals)


def _floats_to_list(arr) -> list:
    """A float numpy array → list with NaN/inf as None (vectorised; the fix-up is sparse)."""
    import numpy as np

    digits = _digits()
    if digits:
        arr = round_to_digits(arr, digits)
    out = arr.tolist()
    bad = ~np.isfinite(arr)
    if bad.any():
        for i in np.flatnonzero(bad):
            out[i] = None
    return out


def _as_int_if_whole(arr) -> list:
    import numpy as np

    finite = np.isfinite(arr)
    if finite.all() and arr.size and np.all(arr == np.round(arr)) and np.abs(arr).max() < 2**53:
        return arr.astype(np.int64).tolist()
    return _floats_to_list(arr)


def _dates_from_datetime64(arr, name: str) -> list:
    """numpy datetime64 → ISO dates (NaT → None); warns once when times of day are dropped."""
    import numpy as np

    days = arr.astype("datetime64[D]")
    nat = np.isnat(arr)
    if (arr[~nat] != days[~nat].astype(arr.dtype)).any():
        _warn_once(f"time:{name}", f"datars: column {name!r} has times of day; charts use its dates (day resolution)")
    out = np.datetime_as_string(days, unit="D").tolist()
    if nat.any():
        for i in np.flatnonzero(nat):
            out[i] = None
    return out


def _from_pandas_series(s, name: str) -> tuple[str, list]:
    import numpy as np
    import pandas as pd
    from pandas.api import types as pt

    dt = s.dtype
    if isinstance(dt, pd.CategoricalDtype):
        return STR, [None if v is None or (isinstance(v, float) and math.isnan(v)) else str(v) for v in s.astype(object).tolist()]
    if pt.is_bool_dtype(dt):
        vals = s.astype(object).tolist()
        return BOOL, [None if v is None or v is pd.NA or (isinstance(v, float) and math.isnan(v)) else bool(v) for v in vals]
    if pt.is_datetime64_any_dtype(dt):
        if getattr(dt, "tz", None) is not None:
            s = s.dt.tz_localize(None)
        return DATE, _dates_from_datetime64(s.to_numpy(dtype="datetime64[ns]"), name)
    if pt.is_timedelta64_dtype(dt):
        return NUM, _floats_to_list(s.dt.total_seconds().to_numpy(dtype=float, na_value=np.nan))
    if pt.is_integer_dtype(dt) and not s.isna().any():
        return NUM, s.to_numpy().tolist()
    if pt.is_numeric_dtype(dt):
        arr = s.to_numpy(dtype=float, na_value=np.nan)
        return NUM, (_as_int_if_whole(arr) if pt.is_integer_dtype(dt) else _floats_to_list(arr))
    if isinstance(dt, pd.PeriodDtype):
        return STR, [None if v is pd.NaT or v is None else str(v) for v in s.astype(object).tolist()]
    # object / string columns: look at what's in them.
    kind = pd.api.types.infer_dtype(s, skipna=True)
    if kind in ("integer", "floating", "mixed-integer-float", "decimal"):
        arr = pd.to_numeric(s, errors="coerce").to_numpy(dtype=float, na_value=np.nan)
        return NUM, (_as_int_if_whole(arr) if kind == "integer" else _floats_to_list(arr))
    if kind == "boolean":
        return BOOL, [None if v is None or v is pd.NA else bool(v) for v in s.tolist()]
    if kind in ("datetime", "datetime64"):
        return DATE, _dates_from_datetime64(pd.to_datetime(s).to_numpy(dtype="datetime64[ns]"), name)
    if kind == "date":
        return DATE, [None if v is None or v is pd.NaT else v.isoformat() for v in s.tolist()]
    return STR, [None if _is_missing(v) else str(v) for v in s.tolist()]


def _is_missing(v) -> bool:
    if v is None:
        return True
    if isinstance(v, float) and math.isnan(v):
        return True
    try:
        import pandas as pd

        return v is pd.NA or v is pd.NaT
    except ImportError:  # pragma: no cover
        return False


def _from_polars_series(s, name: str) -> tuple[str, list]:
    import polars as pl

    dt = s.dtype
    if dt == pl.Boolean:
        return BOOL, s.to_list()
    if dt.is_numeric():
        if dt.is_float():

            return NUM, _floats_to_list(s.to_numpy().astype(float))
        return NUM, s.to_list()
    if dt == pl.Date:
        return DATE, [None if v is None else v.isoformat() for v in s.to_list()]
    if dt == pl.Datetime or isinstance(dt, pl.Datetime):
        vals = s.to_list()
        if any(v is not None and (v.hour or v.minute or v.second or v.microsecond) for v in vals):
            _warn_once(f"time:{name}", f"datars: column {name!r} has times of day; charts use its dates (day resolution)")
        return DATE, [None if v is None else v.date().isoformat() for v in vals]
    if dt == pl.Duration or isinstance(dt, pl.Duration):
        return NUM, [None if v is None else v.total_seconds() for v in s.to_list()]
    return STR, [None if v is None else str(v) for v in s.cast(pl.Utf8).to_list()]


def _from_sequence(values: Sequence, name: str) -> tuple[str, list]:
    """A plain list/tuple/numpy array → typed values."""
    try:
        import numpy as np

        if isinstance(values, np.ndarray):
            if values.dtype.kind in "US":  # fixed-width text: never missing
                return STR, values.astype(str).tolist()
            if values.dtype.kind == "f":
                return NUM, _floats_to_list(values.astype(float))
            if values.dtype.kind in "iu":
                return NUM, values.tolist()
            if values.dtype.kind == "b":
                return BOOL, values.tolist()
            if values.dtype.kind == "M":
                return DATE, _dates_from_datetime64(values.astype("datetime64[ns]"), name)
            values = values.tolist()
        elif isinstance(values, (list, tuple)) and len(values) > 1000:
            # A long plain list: let numpy type it in one pass when it can (no missing values).
            arr = np.asarray(values)
            if arr.dtype.kind in "iufUS":
                return _from_sequence(arr, name)
    except ImportError:  # pragma: no cover
        pass
    vals = list(values)
    present = [v for v in vals if not _is_missing(v)]
    if present and all(isinstance(v, bool) for v in present):
        return BOOL, [None if _is_missing(v) else bool(v) for v in vals]
    if present and all(isinstance(v, (int, float)) and not isinstance(v, bool) for v in present):
        out = []
        for v in vals:
            if _is_missing(v) or (isinstance(v, float) and not math.isfinite(v)):
                out.append(None)
            else:
                out.append(v.item() if hasattr(v, "item") else v)
        return NUM, out
    if present and all(isinstance(v, (_dt.date, _dt.datetime)) for v in present):
        out = []
        for v in vals:
            if _is_missing(v):
                out.append(None)
            elif isinstance(v, _dt.datetime):
                out.append(v.date().isoformat())
            else:
                out.append(v.isoformat())
        return DATE, out
    return STR, [None if _is_missing(v) else str(v) for v in vals]


# ---- the table --------------------------------------------------------------------------------------


def _module(obj) -> str:
    return type(obj).__module__.split(".")[0]


class Data:
    """A table for a document: typed columns, an optional key, a content-addressed name.

    Built from a pandas or polars DataFrame, a Series, a dict of columns, a list of records or a
    numpy structured array. ``columns`` keeps only those (by original name); the index of a pandas
    DataFrame is available under its name (or ``"index"``) when asked for.

    >>> d = Data({"country": ["SE", "NO"], "gdp": [600, 480]}, key="country")
    >>> d.source()["key"]
    ['country']
    """

    def __init__(self, obj: Any = None, *, key: str | Sequence[str] | None = None,
                 columns: Iterable[Any] | None = None, name: str | None = None):
        self.values: dict[str, list] = {}
        self.kinds: dict[str, str] = {}
        self.names: dict[Any, str] = {}  # original column name → document name
        self._name = name
        self._json: str | None = None
        self.nrows = 0
        if obj is not None:
            self._load(obj, list(columns) if columns is not None else None)
        self.key: list[str] | None = None
        if key is not None:
            self.set_key([key] if isinstance(key, str) else list(key), strict=True)

    # -- construction --

    @classmethod
    def from_columns(cls, values: dict[str, list], kinds: dict[str, str] | None = None, *,
                     names: dict[Any, str] | None = None, key: Sequence[str] | None = None) -> "Data":
        """Already-typed columns (document names)."""
        d = cls()
        d.values = dict(values)
        d.kinds = dict(kinds) if kinds else {c: _from_sequence(v, c)[0] for c, v in values.items()}
        d.names = dict(names) if names else {c: c for c in values}
        d.nrows = len(next(iter(values.values()))) if values else 0
        if key:
            d.set_key(list(key), strict=False)
        return d

    def _add(self, original: Any, kind: str, vals: list) -> None:
        taken = set(self.values)
        doc_name = safe_name(original, taken)
        self.values[doc_name] = vals
        self.kinds[doc_name] = kind
        self.names[original] = doc_name
        self.nrows = len(vals)

    def _load(self, obj: Any, columns: list | None) -> None:
        mod = _module(obj)
        if mod == "pandas":
            import pandas as pd

            if isinstance(obj, pd.Series):
                obj = obj.to_frame(name=obj.name if obj.name is not None else "value")
            if isinstance(obj, pd.DataFrame):
                index_name = obj.index.name if obj.index.name is not None else "index"
                want = columns if columns is not None else list(obj.columns)
                for c in want:
                    if c in obj.columns:
                        col = obj[c]
                        if isinstance(col, pd.DataFrame):
                            raise ValueError(f"datars: duplicate column name {c!r}")
                        kind, vals = _from_pandas_series(col, str(c))
                    elif c == index_name or c in (obj.index.names or []):
                        idx = obj.index.get_level_values(c if c in (obj.index.names or []) else 0)
                        kind, vals = _from_pandas_series(pd.Series(idx, index=obj.index), str(c))
                    else:
                        raise KeyError(f"datars: no column {c!r} (columns: {', '.join(map(str, obj.columns))})")
                    self._add(c, kind, vals)
                self.nrows = len(obj)
                return
        if mod == "polars":
            import polars as pl

            if isinstance(obj, pl.Series):
                obj = obj.to_frame()
            if isinstance(obj, pl.LazyFrame):
                obj = obj.collect()
            want = columns if columns is not None else obj.columns
            for c in want:
                if c not in obj.columns:
                    raise KeyError(f"datars: no column {c!r} (columns: {', '.join(obj.columns)})")
                kind, vals = _from_polars_series(obj.get_column(c), str(c))
                self._add(c, kind, vals)
            self.nrows = obj.height
            return
        if hasattr(obj, "dtype") and getattr(obj.dtype, "names", None):  # numpy structured array
            obj = {n: obj[n] for n in obj.dtype.names}
        if isinstance(obj, dict):
            want = columns if columns is not None else list(obj)
            n = None
            for c in want:
                if c not in obj:
                    raise KeyError(f"datars: no column {c!r} (columns: {', '.join(map(str, obj))})")
                kind, vals = _from_sequence(obj[c], str(c))
                if n is not None and len(vals) != n:
                    raise ValueError(f"datars: column {c!r} has {len(vals)} values, expected {n}")
                n = len(vals)
                self._add(c, kind, vals)
            self.nrows = n or 0
            return
        if isinstance(obj, (list, tuple)) and (not obj or isinstance(obj[0], dict)):
            cols: list = []
            for r in obj:
                for k in r:
                    if k not in cols:
                        cols.append(k)
            want = columns if columns is not None else cols
            for c in want:
                kind, vals = _from_sequence([r.get(c) for r in obj], str(c))
                self._add(c, kind, vals)
            self.nrows = len(obj)
            return
        if hasattr(obj, "__dataframe__"):  # the dataframe interchange protocol (pyarrow, modin, …)
            import pandas as pd

            self._load(pd.api.interchange.from_dataframe(obj), columns)
            return
        raise TypeError(f"datars: can't read data from {type(obj).__name__} (a DataFrame, dict of columns or list of records)")

    # -- metadata --

    def col(self, original: Any) -> str:
        """The document name of an original column name (or a document name)."""
        if original in self.names:
            return self.names[original]
        if original in self.values:
            return original
        raise KeyError(f"datars: no column {original!r} (columns: {', '.join(map(str, self.names))})")

    def has(self, original: Any) -> bool:
        return original in self.names or original in self.values

    def kind(self, original: Any) -> str:
        return self.kinds[self.col(original)]

    def column(self, original: Any) -> list:
        return self.values[self.col(original)]

    def unique(self, original: Any) -> list:
        """Distinct non-null values, in order of first appearance."""
        seen: dict = {}
        for v in self.column(original):
            if v is not None and v not in seen:
                seen[v] = True
        return list(seen)

    def is_unique(self, originals: Sequence[Any]) -> bool:
        cols = [self.column(c) for c in originals]
        if not cols:
            return False
        rows = list(zip(*cols))
        return len(set(rows)) == len(rows) and all(v is not None for r in rows for v in r)

    def set_key(self, originals: list, *, strict: bool) -> bool:
        """Key the rows by these columns if they identify every row (else: error or no key)."""
        if not originals:
            self.key = None
            return False
        ok = all(self.has(c) for c in originals) and self.is_unique(originals)
        if ok:
            self.key = [self.col(c) for c in originals]
            self._json = None
        elif strict:
            raise ValueError(f"datars: key {originals!r} doesn't identify every row (duplicates or missing values)")
        return ok

    def extent(self, original: Any) -> tuple[float, float] | None:
        vals = [v for v in self.column(original) if v is not None]
        if not vals:
            return None
        return min(vals), max(vals)

    def with_column(self, name: str, kind: str, values: list) -> "Data":
        """A copy with one more (document-named) column."""
        d = Data.from_columns({**self.values, name: values}, {**self.kinds, name: kind}, names={**self.names, name: name})
        d.key = self.key
        return d

    def select(self, originals: Sequence[Any]) -> "Data":
        cols = [self.col(c) for c in originals]
        d = Data.from_columns({c: self.values[c] for c in cols}, {c: self.kinds[c] for c in cols},
                              names={o: self.col(o) for o in originals})
        if self.key and all(k in cols for k in self.key):
            d.key = list(self.key)
        return d

    # -- the IR --

    def source(self) -> dict:
        src: dict[str, Any] = {"values": self.values}
        if self.key:
            src["key"] = self.key
        dates = {c: DATE for c, k in self.kinds.items() if k == DATE}
        if dates:
            src["types"] = dates
        return src

    def source_json(self) -> str:
        if self._json is None:
            self._json = json.dumps(self.source(), separators=(",", ":"), ensure_ascii=False, allow_nan=False)
        return self._json

    @property
    def name(self) -> str:
        """Content-addressed: the same rows get the same name in every chart (merged once)."""
        if self._name:
            return self._name
        return "data_" + hashlib.sha1(self.source_json().encode("utf-8")).hexdigest()[:10]

    def __len__(self) -> int:
        return self.nrows

    def __repr__(self) -> str:
        cols = ", ".join(f"{c}:{k}" for c, k in self.kinds.items())
        return f"<datars.Data {self.nrows} rows [{cols}]{' key=' + str(self.key) if self.key else ''}>"


def data(obj: Any, *, key: str | Sequence[str] | None = None, columns: Iterable[Any] | None = None,
         name: str | None = None) -> Data:
    """Wrap data for a document: ``dr.data(df, key="country")`` (a DataFrame, dict or records).

    Use it to set a table's key (element identity across states: what morphs into what), to keep
    only some columns, or to name the table.
    """
    if isinstance(obj, Data):
        return obj
    return Data(obj, key=key, columns=columns, name=name)


def is_tabular(obj: Any) -> bool:
    if isinstance(obj, Data):
        return True
    mod = _module(obj)
    if mod in ("pandas", "polars"):
        return True
    if isinstance(obj, dict) and obj and all(isinstance(v, (list, tuple)) or hasattr(v, "__len__") and not isinstance(v, str) for v in obj.values()):
        return True
    if isinstance(obj, list) and obj and isinstance(obj[0], dict):
        return True
    return hasattr(obj, "__dataframe__") or bool(getattr(getattr(obj, "dtype", None), "names", None))
