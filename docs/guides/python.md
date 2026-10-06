# datars for data scientists (Python and notebooks)

`import datars as dr` turns a DataFrame into an animated, themed chart that is live in Jupyter,
JupyterLab, VS Code and Colab — and identical as PNG, SVG, PDF or MP4. It is for people who use
matplotlib, seaborn or plotly with pandas today. Package: [`packages/python`](../../packages/python)
(example notebook: `packages/python/examples/quickstart.ipynb`).

```python
import datars as dr

dr.bar(df, x="country", y="gdp", color="region", title="GDP by country")
```

## What's different from matplotlib

- **A chart is a document, not pixels.** `dr.bar(...)` builds the datars IR — the same JSON the
  TypeScript SDK writes (`chart.to_json()`) — out of the standard library's recipes (`@datars/std`).
  The engine draws it: in the notebook through the web runtime, in files through the `datars`
  CLI, in apps on iOS and Android. What you see in the notebook is what the PNG, the MP4 and the
  published chart show, down to the line breaks.
- **Charts move.** Every chart has states; between them, marks with the same *key* (a country, a
  party) morph instead of being redrawn. `frame="year"` animates over a column; `dr.story(a, b)`
  turns a bar chart into a pie; `.step(highlight=[...], text="…")` narrates.
- **Charts are values.** Every method returns a new chart (`chart.rule(y=2)` doesn't change
  `chart`), so re-running a cell never stacks a layer twice. There is no current figure or axes.
- **Themes are tokens**, not rcParams: `theme="noir"`, or your brand's colours and fonts. Dark
  mode follows the notebook.

## A tour

```python
import pandas as pd, numpy as np
import datars as dr

dr.bar(votes, x="party", y="share", color="party", labels=True)          # bars (horizontal if y is the category)
dr.bar(sales, x="quarter", y="revenue", color="region")                  # stacked; grouped=True, normalize=True
dr.line(prices)                                                          # a wide frame: one line per column vs the index
dr.line(df, x="date", y="value", color="series").rule(y=2, label="Target")
dr.area(df, x="year", y="twh", color="source")                           # stacked areas
dr.scatter(df, x="gdp", y="life", size="pop", color="continent", label="country")
dr.hist(df, x="height", color="group", bins=30)                          # numpy binning, round edges
dr.heatmap(df, x="month", y="city", value="temp")
dr.pie(df, names="party", values="seats"); dr.donut(...); dr.treemap(...)
dr.map(df, key="iso3", value="share")                                    # choropleth: the countries atlas, GeoJSON or a GeoDataFrame
dr.cloud(df, x="x", y="y", color="cluster")                              # millions of points, pan and zoom
dr.sankey(flows, source="from", target="to", value="twh")                # and funnel, waffle, waterfall, swarm, calendar, …
```

Scale types come from dtypes: numbers are linear, dates are time axes, everything else is
categories. Only the columns a chart uses are serialized. Column names that aren't identifiers
("GDP per capita") work everywhere; axis titles keep the original name.

### Layers, annotations, small multiples, layouts

```python
(dr.bar(result, x="party", y="share") + dr.line(poll, x="party", y="poll"))     # overlay: one plot, shared scales
chart.rule(y=4, label="Threshold").span(x=("2008-09-15", "2009-06-30"), label="Crisis")
chart.annotate("2022 spike", x=2022, y=7.7)
dr.line(df, x="year", y="life", color="country", facet="continent", facet_columns=2)
dr.bar(latest, x="life", y="country") | dr.line(df, x="year", y="life", color="country")   # side by side; & stacks
```

Views in one layout share signals: clicking a country's bar highlights its line next door.

### Animation

```python
# plotly's animation_frame / animation_group: each dot glides from year to year
dr.scatter(gap, x="gdp", y="life", size="pop", color="continent", frame="year", key="country")

# sorted per frame: a bar chart race (top=10 keeps the leaders)
dr.bar(gap, x="gdp", y="country", frame="year", sort="desc", top=10)

# a story: one step per chart, marks morph by key, narration drawn by the engine
dr.story(dr.bar(votes, x="party", y="share"),
         (dr.pie(votes, names="party", values="share"), "Each bar becomes a slice."),
         autoplay=False)

# states of one chart
chart.step("All").step("Government", highlight=["M", "KD", "L"], text="…formed the government.")
chart.autoplay(hold=2, loop=True).motion(duration=0.8, easing="cubic-in-out", stagger=0.3)
```

Frames keep the axes over the whole data and put the frame's value in the corner. In a notebook
the chart plays by itself (paused off screen and for reduced motion); ← → and the buttons step.

### Looks

```python
dr.bar(df, x="k", y="v", theme="noir")                       # neutral, noir, broadsheet
brand = dr.theme("neutral", accent="#0a7d6b", mark="#0a7d6b",
                 categorical={"generate": "categorical", "from": "#0a7d6b", "n": 8},
                 font_title=dr.font.google("Source Serif 4", weight=600),
                 dark={"paper": "#0b0f0e"})
chart.update(theme=brand, title="…", width=900, height=500, legend=False, y_domain=[0, 50])
chart.colors({"S": "#e8112d"}).names({"S": "Social Democrats"})   # per-category colours and labels (hold across every state)
```

`dr.options.mode = "dark"` forces a mode; by default charts follow JupyterLab's and VS Code's theme.

## Coming from matplotlib, seaborn or plotly

| You write | In datars |
|---|---|
| `plt.plot(x, y)` | `dr.plot(x, y)` (arrays, a Series, or a DataFrame: one line per column) |
| `plt.bar(names, values)` | `dr.plot(names, values, kind="bar")` or `dr.bar(df, x=…, y=…)` |
| `plt.scatter(x, y, s=…, c=…)` | `dr.scatter(df, x=…, y=…, size=…, color=…)` |
| `plt.hist(x, bins=30)` | `dr.hist(df, x=…, bins=30)` (`nice=False` for numpy's exact edges) |
| `df.plot()` / `df.plot.bar()` / `df.plot.area()` | `dr.line(df)` / `dr.bar(df)` / `dr.area(df)` |
| `ax.set_title / set_xlabel / set_ylim` | `.update(title=…, x_label=…, y_domain=[lo, hi])` (or the same keywords when building) |
| `ax.axhline(2)` / `ax.axvspan(a, b)` / `ax.annotate` | `.rule(y=2)` / `.span(x=(a, b))` / `.annotate(text, x=…, y=…)` |
| `ax.set_yscale("log")` | `log_y=True` (scatter, line) or `update(y_type="log")` |
| `plt.subplots(1, 2)` | `a \| b` (`dr.row`), `a & b` (`dr.column`) |
| `sns.relplot(..., col="g")` / `FacetGrid` | `facet="g"` |
| `sns.barplot(hue=…)` | `dr.bar(..., color=…, grouped=True)` (datars aggregates nothing: pass the values) |
| `sns.histplot(hue=…)` | `dr.hist(..., color=…)` |
| `sns.heatmap(pivot)` | `dr.heatmap(long_df, x=…, y=…, value=…)` |
| `sns.set_theme("dark")`, rcParams | `theme="noir"`, `dr.theme(...)` |
| `px.scatter(..., animation_frame="year", animation_group="country")` | `dr.scatter(..., frame="year", key="country")` |
| `px.choropleth(locations="iso_alpha", color=…)` | `dr.map(df, key="iso_alpha", value=…)` |
| `fig.update_layout(...)` | `chart.update(...)` |
| `plt.savefig("fig.png", dpi=200)` / `fig.write_image` | `chart.save("fig.png", dpr=2)` — also `.svg`, `.pdf`, `.mp4`, `.html` |
| `plt.show()` in a script | `chart.show()` (a browser page with the live chart) |

What datars doesn't do (yet): statistical estimation (seaborn's confidence intervals, KDEs,
regression lines) — compute them in pandas/scipy and plot the result; arbitrary artists on an
axes — datars charts are built from recipes, and `dr.std` / `dr.chart` / `dr.e` reach everything
the recipes can do.

## In notebooks

A chart displays as a MIME bundle, richest first:

| Output | Where it shows | What it is |
|---|---|---|
| widget (`application/vnd.jupyter.widget-view+json`) | JupyterLab, Notebook 7, VS Code, Colab — with `pip install "datars[widget]"` (anywidget) | the live chart; the web runtime (~6 MB) comes **over the kernel connection** once per page: offline, on remote servers, never saved into the notebook |
| `text/html` | frontends without widgets, `nbconvert --to html`, nbviewer, GitHub | a static poster (SVG; PNG for dense charts) inside a block that upgrades itself to the live `<datars-view>` where scripts run and the runtime URL loads |
| `image/png` | PDF/LaTeX export, renderers that don't show HTML | one state at `dr.options.png_dpr` |

Posters and PNGs are rendered by the `datars` CLI at display time (a few tens of ms per chart); without
the CLI, charts are live-only. `dr.options`:

| Option | Default | |
|---|---|---|
| `renderer` | `"auto"` | `"widget"`, `"html"`, `"static"` (images only, like matplotlib inline), `"json"` |
| `fallback` | `"auto"` | `"svg"`: the poster only (lighter notebooks), `"png"`, `None`: no CLI at display time |
| `runtime_url` | jsDelivr `@datars/web` | where HTML outputs load the runtime; point it at a self-hosted `packages/web/dist` |
| `png_dpr` | `1` | `2` for retina PNGs |
| `digits` | `None` | round floats to N significant digits when building (big data) |
| `mode` | `"auto"` | `"light"` / `"dark"` |
| `embed_limit` | 20 MB | larger documents aren't embedded in HTML outputs (the widget still shows them) |

Environment variables: `DATARS_CLI`, `DATARS_RENDERER`, `DATARS_FALLBACK`, `DATARS_RUNTIME_URL`,
`DATARS_PNG_DPR`, `DATARS_WEB_DIST`.

When a notebook is reopened without a kernel, Jupyter shows the saved outputs untrusted: the poster
shows; run the cell for the live chart.

Frontends: **JupyterLab 4** is verified end to end (widgets live, clicks reaching Python);
Notebook 7 is the same frontend. **VS Code** and **Colab** load anywidget widgets through their own
widget managers (Colab may need `google.colab.output.enable_custom_widget_manager()`); they are not
verified yet — `dr.options.renderer = "html"` (with a reachable `runtime_url`) or `"static"` are the
fallbacks. Colab renders each output in its own frame, so each chart there fetches the runtime.
Exports with `jupyter nbconvert --to html` go live when the runtime URL loads and show the posters
when it doesn't (both verified).

### Two-way: the widget

```python
w = chart.widget()          # or any chart displayed with the widget renderer
w.state = "2024"            # go to a step        (w.next(), w.prev())
w.set_signal("highlight", ["SE", "NO"])
w.tokens = {"accent": "#e4572e"}
w.chart = other_chart       # the view morphs to the new document (keys pair marks)

@w.on_click
def picked(event):          # every click: what was hit, and the data row behind it
    print(event["fields"])  # {'country': 'SE', 'gdp': 593}

w.observe(lambda ch: print(ch["new"]), names="state")   # the reader stepped
```

Brush ranges and keyset selections made in the chart aren't mirrored to Python yet (the web runtime
doesn't expose signal values); clicks and states are.

## Big data

`dr.scatter` draws instanced points (10⁵–10⁶ are fine). `dr.cloud` takes any number: the engine
indexes the rows once into a density-preserving pyramid and each frame draws what's in view —
a sample when zoomed out, every row up close (1,000,000 rows open in Chrome in a couple of seconds).
The limit in a notebook is getting the rows to the browser: the document carries columnar JSON
(16 MB for a million x/y/category rows at `dr.options.digits = 5`). The widget sends it as a binary
buffer; HTML outputs embed it up to `embed_limit`. For more, publish (`chart.publish(dir)`): the
publish compiler writes the rows as a range-read point archive, and the chart streams only the tiles
it shows.

## Export and publish

```python
chart.save("fig.png", dpr=2, state="2024", mode="dark")   # one state (CPU reference renderer)
chart.save("fig.svg"); chart.save("fig.pdf")               # vector, text as outlines
chart.save("story.mp4", fps=30, size=(1080, 1920))        # the whole program, with WebVTT captions (ffmpeg)
chart.save("fig.html")                                     # a standalone page, runtime inlined: works offline
chart.to_json()                                            # the document: datars render/dev/publish, any runtime
chart.publish("site/", alias="gdp")                        # static delivery layout; returns the <datars-view> snippet
chart.check()                                              # the engine's diagnostics
```

`save`, `image`, `publish` and `check` run the `datars` CLI: `DATARS_CLI`, a binary shipped in
the wheel (`datars/_bin/`), `datars` on `PATH`, or a build in a checkout of this repository (`target/release`).

## Under the hood

- **`dr.std`** has every std recipe as a typed Python function (keyword arguments in
  snake_case, docstrings with defaults and theme tokens), generated from `datars describe --json`
  by `tools/gen_std.py`; a test fails when it's stale. `dr.chart(node)` wraps any of them;
  DataFrames passed to table parameters become tables.
- **`dr.e("…")`** is an engine expression (per row, per frame, in Rust): `dr.e("d.gdp / d.pop")`.
- **`dr.data(df, key=…)`** sets a table's key explicitly (what morphs into what).
- Tables are content-addressed (the same rows in two story steps are sent once).

```python
node = dr.std.pie(data=df, category="party", value="share", inner=0.6).opts(key="pie")
dr.chart(node, title="Custom")
```

## Installing

```sh
pip install "datars[widget]"      # + the datars CLI on PATH for images, PDF and MP4
```

Until wheels are published: `pip install -e packages/python[widget]` in a checkout, after
`python packages/python/tools/vendor_runtime.py` (copies the built web runtime into the package).
Native module note: rendering goes through the CLI as a subprocess; a PyO3 module over
`datars-headless` (in-process PNG/SVG, no binary to install) is planned as an optional extra.
