"""Scene template nodes (the IR's ``scene``), mirroring ``packages/sdk/src/nodes.ts``.

A :class:`Node` is plain IR JSON that may still hold Python objects — DataFrames (registered as
tables when the document is built), expressions and other nodes. ``to_ir`` turns it into JSON.
"""

from __future__ import annotations

import copy
from typing import Any

from ._data import Data, is_tabular
from ._ir import Expr


class Registry:
    """Tables collected while a document is built: every :class:`Data` once, by content name."""

    def __init__(self) -> None:
        self.sources: dict[str, Data] = {}

    def add(self, d: Data) -> str:
        name = d.name
        prev = self.sources.get(name)
        if prev is None:
            self.sources[name] = d
        elif prev is not d and prev.source_json() != d.source_json():  # a user-chosen name, reused
            raise ValueError(f"datars: two different tables are both named {name!r}")
        return name


def to_ir(value: Any, reg: Registry) -> Any:
    """Python values → IR JSON (expressions, nodes, tables, numpy scalars; ``None`` fields drop)."""
    if isinstance(value, Node):
        return value.to_ir(reg)
    if isinstance(value, Expr):
        return value.to_ir()
    if isinstance(value, Data):
        return reg.add(value)
    if isinstance(value, dict):
        return {str(k): to_ir(v, reg) for k, v in value.items() if v is not None}
    if isinstance(value, (list, tuple)):
        return [to_ir(v, reg) for v in value]
    if isinstance(value, (str, bool, int, float)) or value is None:
        return value
    if hasattr(value, "item") and callable(value.item):  # numpy scalars
        return value.item()
    if hasattr(value, "isoformat"):
        return value.isoformat()
    if is_tabular(value):
        return reg.add(Data(value))
    raise TypeError(f"datars: can't put a {type(value).__name__} in a document")


class Node:
    """A scene node — a recipe instance (``use``) or a primitive (group, text, shape, …).

    >>> import datars as dr
    >>> n = dr.std.title(text="Hello").opts(key="head")
    >>> n.to_dict()["kind"], n.to_dict()["key"]
    ('use', 'head')
    """

    __slots__ = ("fields",)

    def __init__(self, fields: dict[str, Any]):
        if "kind" not in fields:
            raise ValueError("a node needs a `kind`")
        self.fields = fields

    @property
    def kind(self) -> str:
        return self.fields["kind"]

    @property
    def recipe(self) -> str | None:
        return self.fields.get("recipe")

    @property
    def params(self) -> dict[str, Any]:
        return self.fields.setdefault("params", {}) if self.kind == "use" else {}

    def copy(self) -> "Node":
        f = dict(self.fields)
        if "params" in f:
            f["params"] = dict(f["params"])
        return Node(f)

    def opts(self, **options: Any) -> "Node":
        """A copy with node options: ``key``, ``when``, ``opacity``, ``size``, ``layout``, ``z``, …

        (Recipe parameters live in ``params``; options are the node's own fields — docs/04.)
        """
        n = self.copy()
        for k, v in options.items():
            if v is None:
                n.fields.pop(k, None)
            else:
                n.fields[k] = v
        return n

    def set(self, **params: Any) -> "Node":
        """A copy with recipe parameters changed (document names: ``xType``, not ``x_type``)."""
        n = self.copy()
        for k, v in params.items():
            if v is None:
                n.params.pop(k, None)
            else:
                n.params[k] = v
        return n

    def to_ir(self, reg: Registry) -> dict:
        return to_ir(self.fields, reg)

    def to_dict(self) -> dict:
        """This node as IR JSON (tables it holds become content names)."""
        return self.to_ir(Registry())

    def __repr__(self) -> str:
        what = self.recipe or self.kind
        keys = ", ".join(sorted(self.params)) if self.kind == "use" else ""
        return f"<datars.Node {what}({keys})>"

    # A node on its own displays as a chart around it.
    def _repr_mimebundle_(self, include=None, exclude=None):
        from .chart import chart

        return chart(self)._repr_mimebundle_(include, exclude)

    def __deepcopy__(self, memo):
        return Node(copy.deepcopy(self.fields, memo))


def use(recipe: str, params: dict[str, Any], renames: dict[str, str] | None = None) -> Node:
    """A recipe instance: ``use("@datars/std/bar", {...})`` (what the generated ``std`` calls)."""
    renames = renames or {}
    out: dict[str, Any] = {}
    for k, v in params.items():
        if v is None:
            continue
        out[renames.get(k, k)] = v
    # DataFrames given to table params become tables; field params naming their columns follow
    # the columns' document names (``"GDP per capita"`` → ``GDP_per_capita``).
    from .std import RECIPES  # generated; loaded by the time a recipe is called

    spec = RECIPES.get(recipe.rsplit("/", 1)[-1], {}).get("params", {})
    for k, v in list(out.items()):
        if spec.get(k, {}).get("type") == "table" and not isinstance(v, (str, Data)) and is_tabular(v):
            out[k] = Data(v)
    d = out.get("data")
    if isinstance(d, Data):
        for k, v in out.items():
            if spec.get(k, {}).get("type") == "field" and isinstance(v, str) and d.has(v):
                out[k] = d.col(v)
    return Node({"kind": "use", "recipe": recipe, "params": out})


def _children(children) -> list:
    return [c for c in (children or []) if c is not None and c is not False]


def group(children: list | None = None, **options: Any) -> Node:
    """A group: ``layout`` (stack, rows, columns, grid, flow), ``scales``, ``coord``, children."""
    return Node({"kind": "group", **options, "children": _children(children)})


def view(children: list | None = None, *, camera: dict | None = None, **options: Any) -> Node:
    """A view: a camera over its children (``{"fit": {...}, "padding": 24, "explore": "cam"}``)."""
    return Node({"kind": "view", **options, "camera": camera, "children": _children(children)})


def text(content: Any, at: tuple[Any, Any], *, style: dict | None = None, **options: Any) -> Node:
    """Text at ``at``; ``style``: ``size``, ``weight``, ``ink``, ``align``, ``baseline``, ``font``."""
    return Node({"kind": "text", **options, "text": content if content is not None else "", "at": list(at), "style": style or {}})


def shape(geom: dict, *, fill: Any = None, stroke: dict | None = None, **options: Any) -> Node:
    """A shape: ``geom`` (``{"type": "rect", "x":…}``, circle, segment, path, …), fill, stroke."""
    return Node({"kind": "shape", **options, "geom": geom, "fill": fill, "stroke": stroke})


def raw(ir: dict) -> Node:
    """Any IR template given as JSON (``{"kind": …}``), for what the helpers don't cover."""
    return Node(dict(ir))
