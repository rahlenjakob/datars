"""Expressions and the IR's JSON conventions (mirrors ``packages/sdk/src/prop.ts``).

A property is a literal or an expression the engine evaluates per row and per frame, in Rust —
never Python at render time ("code builds the graph; the engine runs the graph").
"""

from __future__ import annotations

import json
import math
import re

_IDENT = re.compile(r"^[A-Za-z_$][A-Za-z0-9_$]*$")


class Expr:
    """An engine expression (``{"expr": "…"}`` in the IR): ``e("d.share * 100")``.

    Expressions read the current row as ``d``, signals by name, scales as ``scale.x(…)``, theme
    tokens as ``token("…")`` — the same language the TypeScript SDK writes (docs/06).
    """

    __slots__ = ("src",)

    def __init__(self, src: str):
        if not isinstance(src, str):
            raise TypeError(f"an expression is source text, got {type(src).__name__}")
        self.src = src

    def to_ir(self) -> dict:
        return {"expr": self.src}

    def __repr__(self) -> str:
        return f"e({self.src!r})"

    def __eq__(self, other: object) -> bool:
        return isinstance(other, Expr) and other.src == self.src

    def __hash__(self) -> int:
        return hash(("expr", self.src))


def e(src: str) -> Expr:
    """An expression: ``dr.e("d.gdp / d.pop")``, ``dr.e("scale.x(2008)")``, ``dr.e("year")``."""
    return Expr(src)


def field(name: str) -> Expr:
    """The current row's column: ``field("share")`` → ``d.share`` (quoted when not an identifier)."""
    return Expr(f"d.{name}" if _IDENT.match(name) else f"d[{json.dumps(name)}]")


def lit(value) -> str:
    """A Python value as expression source (a string literal is quoted)."""
    if isinstance(value, Expr):
        return f"({value.src})"
    if value is None:
        return "null"
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, (int, float)):
        if isinstance(value, float) and not math.isfinite(value):
            return "null"
        return repr(value)
    return json.dumps(str(value), ensure_ascii=False)


def is_ident(name: str) -> bool:
    return bool(_IDENT.match(name))
