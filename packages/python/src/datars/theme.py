"""Themes: built-in looks and brand tokens (docs/18-themes.md; ``packages/sdk/src/theme.ts``).

A theme is typed tokens — colours (``"#0a7d6b"``, ``"$accent"``, ``"mix($ink, $paper, 0.9)"``),
palettes, numbers, fonts — with modes (dark, high contrast), inheritance and locks. Charts hold
late-bound inks (``$mark``), so switching a theme or a mode re-colours without rebuilding.

    dr.bar(df, x="country", y="gdp", theme="noir")
    brand = dr.theme(accent="#0a7d6b", categorical={"generate": "categorical", "from": "#0a7d6b", "n": 8})
    dr.bar(df, x="country", y="gdp", theme=brand)
"""

from __future__ import annotations

import os
from typing import Any

#: The themes every runtime knows (``packages/std/themes`` + the engine's neutral base).
BUILTIN = ("neutral", "noir", "broadsheet")


def _qualified(name: str) -> str:
    return name if "/" in name else f"datars/{name}"


class Theme:
    """A theme reference: a built-in (``"noir"``), or a custom one extending it with tokens.

    ``tokens`` are theme tokens by name (``accent``, ``paper``, ``ink``, ``mark``,
    ``categorical``, ``font.title``, ``size.title`` …); ``dark`` / ``high_contrast`` override
    tokens in those modes; ``locked`` tokens can't be overridden by hosts.
    """

    def __init__(self, extends: str = "neutral", *, name: str | None = None, tokens: dict[str, Any] | None = None,
                 dark: dict[str, Any] | None = None, high_contrast: dict[str, Any] | None = None,
                 locked: list[str] | None = None, scheme: str | None = None):
        self.extends = _qualified(extends)
        self.name = name
        self.tokens = dict(tokens or {})
        self.modes = {k: v for k, v in (("dark", dark), ("high-contrast", high_contrast)) if v}
        self.locked = list(locked or [])
        self.scheme = scheme

    def to_ir(self) -> dict:
        """The document's ``theme`` field."""
        if not (self.name or self.modes or self.locked or self.scheme):
            return {"use": self.extends, **({"tokens": self.tokens} if self.tokens else {})}
        name = self.name or "custom/python"
        d: dict[str, Any] = {"name": name, "extends": self.extends, "tokens": self.tokens}
        if self.scheme:
            d["scheme"] = self.scheme
        if self.modes:
            d["modes"] = self.modes
        if self.locked:
            d["locked"] = self.locked
        return {"use": name, "themes": [d]}

    def __repr__(self) -> str:
        return f"<datars.Theme {self.name or 'custom'} extends {self.extends} {sorted(self.tokens)}>"


def theme(extends: str = "neutral", *, name: str | None = None, dark: dict[str, Any] | None = None,
          high_contrast: dict[str, Any] | None = None, locked: list[str] | None = None,
          scheme: str | None = None, **tokens: Any) -> Theme:
    """A custom theme from a built-in and brand tokens.

    Keyword tokens use ``_`` for ``.`` and ``-`` (``font_title`` → ``font.title``,
    ``ink_2`` → ``ink-2``); pass ``tokens``-style names in a dict with ``**{...}`` when exact.

    >>> theme("noir", accent="#e4572e").to_ir()["tokens"]
    {'accent': '#e4572e'}
    """
    fixed = {}
    for k, v in tokens.items():
        fixed[_token_name(k)] = v
    return Theme(extends, name=name, tokens=fixed, dark=dark, high_contrast=high_contrast, locked=locked, scheme=scheme)


_DOTTED = ("font_", "size_", "stroke_", "radius_", "map_", "point_")


def _token_name(k: str) -> str:
    """Python keyword → token name: ``font_title`` → ``font.title``, ``ink_2`` → ``ink-2``."""
    for p in _DOTTED:
        if k.startswith(p):
            return p[:-1] + "." + k[len(p):].replace("_", "-")
    return k.replace("_", "-")


def theme_ir(value: Any) -> dict | None:
    """What a chart's ``theme=`` becomes in the document."""
    if value is None:
        return None
    if isinstance(value, Theme):
        return value.to_ir()
    if isinstance(value, str):
        return {"use": _qualified(value)}
    if isinstance(value, dict):
        if "use" in value or "themes" in value:
            return value
        if "name" in value and "tokens" in value:  # a ThemeDef (JSON, as in packages/std/themes)
            return {"use": value["name"], "themes": [value]}
        return {"use": "datars/neutral", "tokens": value}  # bare tokens
    raise TypeError(f"datars: theme= takes a name, dr.theme(...) or a dict, not {type(value).__name__}")


class font:
    """Font tokens (``font.body``, ``font.title``, …): faces travel with the chart (docs/12)."""

    @staticmethod
    def google(family: str, *, weight: int = 400, italic: bool = False, fallback: list[str] | None = None) -> dict:
        """A Google Fonts family, downloaded by the build step (``datars render``/``publish``)."""
        d: dict[str, Any] = {"google": family, "weight": weight}
        if italic:
            d["italic"] = True
        if fallback:
            d["family"] = fallback
        return d

    @staticmethod
    def file(family: str, src: str, *, weight: int = 400, italic: bool = False) -> dict:
        """A font file (TrueType/OpenType) — an absolute path, so the CLI finds it from anywhere."""
        if not src.startswith(("http://", "https://", "datars:")):
            src = os.path.abspath(os.path.expanduser(src))
        d: dict[str, Any] = {"family": family, "weight": weight, "src": src}
        if italic:
            d["italic"] = True
        return d
