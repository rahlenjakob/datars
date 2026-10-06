"""Helpers for walking documents in tests."""

from __future__ import annotations


def plot_params(doc: dict) -> dict:
    """The params of the document's plot (the chart node)."""
    node = find(doc["scene"], lambda n: n.get("recipe") == "@datars/std/plot")
    assert node is not None, "no plot in the document"
    return node["params"]


def find(node, pred):
    if isinstance(node, dict):
        if pred(node):
            return node
        for v in node.values():
            r = find(v, pred)
            if r is not None:
                return r
    elif isinstance(node, list):
        for v in node:
            r = find(v, pred)
            if r is not None:
                return r
    return None


def find_all(node, pred, out=None):
    out = [] if out is None else out
    if isinstance(node, dict):
        if pred(node):
            out.append(node)
        for v in node.values():
            find_all(v, pred, out)
    elif isinstance(node, list):
        for v in node:
            find_all(v, pred, out)
    return out
