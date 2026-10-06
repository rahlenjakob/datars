# datars for Python

Animated, themed data graphics from DataFrames — **live in Jupyter, JupyterLab, VS Code and
Colab**, identical as PNG, SVG, PDF and MP4. For people who use matplotlib, seaborn or plotly with
pandas today.

```python
import datars as dr

dr.bar(votes, x="party", y="share", color="party", labels=True, title="Vote share (%)")
```

A chart is a datars *document* — the same IR the TypeScript SDK writes — built from the standard
library's recipes and drawn by the datars engine: in the notebook through the web runtime, in files
through the `datars` CLI, unchanged on the web, iOS and Android.

The guide for data scientists (tour, animation, notebooks, big data, and a **coming from
matplotlib / seaborn / plotly** table): [`docs/guides/python.md`](../../docs/guides/python.md).
A worked notebook: [`examples/quickstart.ipynb`](examples/quickstart.ipynb).

## Install

```sh
pip install -e ".[widget]"          # in a checkout, after: python tools/vendor_runtime.py
```

- **Core**: no required dependencies (pandas, polars, numpy are used when you pass them).
- **`[widget]`** (anywidget): live, two-way charts in JupyterLab / Notebook 7 / VS Code / Colab;
  the web runtime comes over the kernel connection, so it works offline and on remote servers.
- **The `datars` CLI** (`DATARS_CLI`, or `datars` on `PATH`) renders the static previews and
  every export. Without it, charts show live only.
- **The web runtime** (`packages/web/dist`: `datars.js`, the wasm engine, Inter, the countries
  atlas — 6 MB) is vendored into `src/datars/_runtime` by `tools/vendor_runtime.py` before a build
  (not in git); `DATARS_WEB_DIST` points at a build instead.

## Quickstart

```python
import pandas as pd
import datars as dr

votes = pd.DataFrame({"party": ["S", "SD", "M", "V", "C", "KD", "MP", "L"],
                      "share": [30.3, 20.5, 19.1, 6.8, 6.7, 5.3, 5.1, 4.6]})

chart = dr.bar(votes, x="party", y="share", color="party", title="Vote share, 2022 (%)")
chart                                              # displays: live widget / HTML + static poster
chart.rule(y=4, label="Threshold")                 # a new chart (charts are values)
dr.line(prices_wide_df)                            # df.plot() style: a line per column
dr.hist(people, x="height", color="group")
dr.map(renew, key="iso3", value="share")

# animation — datars' superpower
dr.scatter(gap, x="gdp", y="life", size="pop", color="continent", frame="year", key="country")
dr.story(dr.bar(votes, x="party", y="share"),
         (dr.pie(votes, names="party", values="share"), "Each bar becomes a slice."))

# export (through the CLI)
chart.save("fig.png", dpr=2); chart.save("fig.svg"); chart.save("fig.pdf")
dr.story(...).save("story.mp4")
chart.save("fig.html")                             # standalone, offline
chart.to_json()                                    # the document

# two-way
w = chart.widget()
w.on_click(lambda e: print(e["fields"]))           # the clicked bar's row
w.state = "2"                                      # drive it from Python
```

`plt.plot(x, y)` → `dr.plot(x, y)`; more in the guide.

## How it fits together

| Piece | Where |
|---|---|
| chart functions (`bar`, `line`, `scatter`, `hist`, `map`, `cloud`, …), inference of scales, keys, stacking, frames | `src/datars/charts.py` |
| `Chart`, `Story`, `Layout`: overlays, steps, highlight, facets, program and motion, document assembly | `src/datars/chart.py` |
| `dr.std`: every std recipe as a typed function, **generated** from `datars describe --json` | `src/datars/std.py` ← `tools/gen_std.py`, `src/datars/_recipes.json` |
| DataFrames (pandas, polars, dicts, records, numpy) → columnar sources; content-addressed tables | `src/datars/_data.py` |
| MIME bundles, HTML outputs, standalone pages, options | `src/datars/_display.py`, `src/datars/_static/loader.js` |
| anywidget front end and Python side | `src/datars/widget.py`, `src/datars/_static/widget.js` |
| the CLI as a subprocess (render, video, bundle, publish, check) | `src/datars/_cli.py` |

The web runtime needed one addition for notebooks (in `packages/web`): a page can supply the
runtime's files itself (`runtimeAssets`, blob: URLs for a runtime that arrived over a kernel
connection), and `<datars-view>` dispatches `pick` on clicks (what was hit and its data row).

## Development

```sh
uv venv .venv && uv pip install --python .venv/bin/python -e ".[test,widget]" polars nbclient ipykernel
python tools/vendor_runtime.py                     # the web runtime from ../web/dist
DATARS_CLI=../../target/release/datars .venv/bin/python -m pytest      # no network; CLI/runtime tests skip without them
python tools/gen_std.py --refresh --cli ../../target/release/datars    # after std changes
python tools/check_browser.py                      # headless Chrome: standalone page, HTML output, widget (click → Python)
```

`tools/jupyterlab_check.mjs` runs a notebook in a real JupyterLab (`--LabApp.expose_app_in_browser`)
and checks every chart went live, a click reached Python and Python drove the chart.
The example notebook is executed with `DATARS_RENDERER=html DATARS_FALLBACK=svg jupyter execute
--inplace examples/quickstart.ipynb` so its saved outputs are light and render on GitHub.

Licence: proprietary — © 2026 Jakob Råhlén, all rights reserved (see LICENSE at the repository root).
