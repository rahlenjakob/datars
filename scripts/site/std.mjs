// The chart reference (/docs/std/…), generated from the recipes themselves so it can't go stale:
// parameters, defaults, docs and theme tokens from `datars describe --json`; a live example per
// recipe — its figure, `site/figures/std/<name>.ts`, shown with its full source — plus usage
// lifted from the real example documents, and a gallery chart where one adds something.

import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { escapeAttr, escapeHtml, inline } from "./markdown.mjs";

/** Reference sections, in order. A recipe not listed here lands in "More" (new recipes show up). */
export const STD_GROUPS = [
  ["Frames and layout", "A plot sets up scales and axes; marks go inside it.", ["plot", "facet", "title", "card", "legend", "axis", "grid"]],
  ["Marks", "Data drawn in a plot's coordinates.", ["bar", "line", "area", "point", "dot", "grouped", "stacked", "cell", "rule", "span", "annotate"]],
  ["Comparisons", "Before and after, ranks and gaps.", ["slope", "dumbbell", "bump", "lollipop", "pyramid", "marimekko"]],
  ["KPIs and planning", "Numbers that matter, and what happens when.", ["kpi", "bullet", "gauge", "progress", "gantt", "timeline"]],
  ["Tables and profiles", "Many values per thing.", ["dataTable", "radar"]],
  ["Parts of a whole and flows", "Shares, units, bridges and flows.", ["pie", "treemap", "waffle", "hemicycle", "funnel", "waterfall", "pareto", "sankey"]],
  ["Networks and hierarchies", "Links between things and things inside things.", ["network", "arcDiagram", "chord", "matrix", "tree", "dendrogram", "sunburst", "icicle"]],
  ["Distributions and time", "How values spread — binned, summarised, smoothed or every row visible — and time as the axis.", ["histogram", "boxplot", "violin", "ridgeline", "errorBars", "swarm", "stackedArea", "connectedScatter", "stripes", "calendar", "track"]],
  ["Finance", "Prices, volumes and returns over time.", ["candlestick", "ohlc", "volume", "movingAverage", "bollinger", "indexed", "drawdown", "sparkline"]],
  ["Maps", "Regions, symbols, routes and basemaps in any projection.", ["map", "tileMap", "basemap", "symbols", "dotDensity", "geoPoints", "geoLines", "route", "attribution"]],
  ["Big data", "Millions of rows drawn as what they add up to: a density of hexagons or grid cells, the contours of a crowd, thousands of series at once — binned once per data in the engine, so a frame draws the bins, not the rows — or every row as a point cloud with level of detail.", ["hexbin", "heatmap2d", "contours", "manyLines", "cloud"]],
  ["Controls", "Engine-drawn inputs that set signals — the same on the web, in apps and in video, offered to screen readers and keyboards as native controls.", ["slider", "range", "segmented", "select", "toggle", "checklist", "button"]],
];

/** Marks that live inside a plot: their usage is shown with the enclosing `plot(…)`. */
const IN_PLOT = new Set(["bar", "line", "area", "point", "dot", "grouped", "stacked", "cell", "rule", "span", "annotate", "waterfall", "pareto", "grid", "axis", "legend", "hexbin", "heatmap2d", "contours", "manyLines", "slope", "dumbbell", "lollipop", "histogram", "boxplot", "violin", "ridgeline", "errorBars", "stackedArea", "connectedScatter"]);
/** Gallery charts shown on a recipe's page under its own figure — only where they add something
 * the small figure can't (scale, a real story, a flight), so a page plays at most two charts. */
const GALLERY = { map: "renewables", basemap: "descent", cloud: "galaxy", slider: "budget", hemicycle: "riksdag", stripes: "warming", sankey: "energy", candlestick: "stocks" };

const recipeName = (id) => id.replace("@datars/std/", "");
/** A recipe's figure: `site/figures/std/<name>.ts`, published as chart `std-<name lowercased>`. */
export const figureAlias = (name) => `std-${name.toLowerCase()}`;
export const figureFile = (name) => `site/figures/std/${name}.ts`;

/** The balanced `(…)` call starting at `open` (index of the name) in `src`, or null. */
function callExtent(src, start) {
  const open = src.indexOf("(", start);
  if (open < 0) return null;
  let depth = 0, inStr = null;
  for (let i = open; i < src.length; i++) {
    const c = src[i];
    if (inStr) {
      if (c === "\\") { i++; continue; }
      if (c === inStr) inStr = null;
      continue;
    }
    if (c === '"' || c === "'" || c === "`") { inStr = c; continue; }
    if (c === "(") depth++;
    else if (c === ")") { depth--; if (depth === 0) return [start, i + 1]; }
  }
  return null;
}

/** Dedent a snippet and cut it to `max` lines. */
function tidy(code, max = 16) {
  const lines = code.split("\n");
  const ind = Math.min(...lines.slice(1).filter((l) => l.trim()).map((l) => l.match(/^\s*/)[0].length), Infinity);
  const out = [lines[0], ...lines.slice(1).map((l) => (Number.isFinite(ind) ? l.slice(ind) : l))];
  if (out.length > max) return [...out.slice(0, max - 1), "  // …"].join("\n");
  return out.join("\n");
}

/** Every example document in the repo that imports from @datars/std, with its source. */
export function exampleSources(root) {
  const files = [];
  const walk = (dir, depth) => {
    if (!existsSync(dir) || depth > 4) return;
    for (const f of readdirSync(dir)) {
      const p = join(dir, f);
      if (f === "node_modules" || f.startsWith(".")) continue;
      if (statSync(p).isDirectory()) walk(p, depth + 1);
      else if (f === "doc.ts" || (dir.endsWith("site/charts") && f.endsWith(".ts"))) files.push(p);
    }
  };
  walk(join(root, "examples"), 0);
  walk(join(root, "site/charts"), 0);
  walk(join(root, "site/articles"), 0);
  return files.map((f) => ({ file: relative(root, f), src: readFileSync(f, "utf8") })).filter((x) => x.src.includes("@datars/std"));
}

/** A real call of recipe `name` from the examples (the enclosing plot for marks). */
function usage(name, examples) {
  const re = new RegExp(`(?<![\\w.])${name}\\(\\{`, "g");
  // Short, self-contained examples first: the repo's examples, then the site's charts, then articles.
  const ranked = [...examples].sort((a, b) => rank(a.file) - rank(b.file) || a.src.length - b.src.length);
  let best = null;
  for (const ex of ranked) {
    if (!new RegExp(`import[^;]*\\b${name}\\b[^;]*from "@datars/std"`).test(ex.src)) continue;
    re.lastIndex = 0;
    for (let m; (m = re.exec(ex.src)); ) {
      let ext = callExtent(ex.src, m.index);
      if (!ext) continue;
      if (IN_PLOT.has(name)) {
        // The nearest enclosing plot( … ) call.
        let at = ex.src.lastIndexOf("plot({", m.index);
        while (at >= 0) {
          const p = callExtent(ex.src, at);
          if (p && p[0] <= m.index && p[1] >= ext[1]) { ext = p; break; }
          at = ex.src.lastIndexOf("plot({", at - 1);
        }
      }
      const lineStart = ex.src.lastIndexOf("\n", ext[0]) + 1;
      const code = ex.src.slice(ext[0], ext[1]);
      const lead = ex.src.slice(lineStart, ext[0]);
      const snippet = tidy((/^\s*$/.test(lead) ? "" : "") + code);
      const lines = snippet.split("\n").length;
      const score = rank(ex.file) * 100 + Math.abs(lines - 6);
      if (!best || score < best.score) best = { code: snippet, file: ex.file, score };
      break;
    }
    if (best && best.score < 104) break;
  }
  return best;
}
const rank = (f) => (f.startsWith("examples/") ? 0 : f.startsWith("site/charts") ? 1 : 2);

function typeCell(p) {
  if (p.type === "enum" && p.values) return p.values.map((v) => `<code>${escapeHtml(JSON.stringify(v))}</code>`).join(" | ");
  const hint = { table: "table name", field: "column name", prop: "value or expression", ink: "ink (colour or token)", children: "child nodes", json: "JSON" }[p.type];
  return `<span class="ty">${escapeHtml(p.type)}</span>${hint ? ` <small>${hint}</small>` : ""}`;
}

/** The recipe source file that defines `@datars/std/<name>`. */
function sourceFile(root, name) {
  const dir = join(root, "packages/std/src");
  for (const f of readdirSync(dir)) {
    const src = readFileSync(join(dir, f), "utf8");
    if (src.includes(`"@datars/std/${name}"`)) return `packages/std/src/${f}`;
  }
  return null;
}

/**
 * Build the reference: returns `[{ file, title, description, body, charts }]` pages for
 * /docs/std/ and /docs/std/<name>/. `siteCharts`: alias → the recipes its document uses;
 * `charts`: every chart the site publishes (figures included); `figure(alias, caption?)`: a live
 * chart with its caption and text alternative; `thumb(alias, alt)`: a chart's first state as
 * light and dark thumbnails.
 */
export function stdPages({ root, all, blob, siteCharts, figure, thumb, charts }) {
  const examples = exampleSources(root);
  const names = Object.keys(all).map(recipeName);
  // Every recipe has a figure, so a new recipe can't reach the reference without an example.
  const missing = names.filter((n) => !charts[figureAlias(n)]);
  if (missing.length) throw new Error(`chart reference: no figure for ${missing.map((n) => `std/${n}`).join(", ")} — add ${missing.map(figureFile).join(", ")} (a small document showing the recipe; see site/figures/std/bar.ts)`);
  // Groups whose recipes this build of std doesn't have are left out.
  const grouped = STD_GROUPS.map(([h, blurb, list]) => [h, blurb, list.filter((n) => names.includes(n))]).filter(([, , l]) => l.length);
  const listed = new Set(grouped.flatMap(([, , l]) => l));
  const rest = names.filter((n) => !listed.has(n));
  if (rest.length) grouped.push(["More", "", rest]);
  const order = grouped.flatMap(([, , l]) => l);

  const pages = [];

  // The index: a card per recipe, its figure's first state on top.
  const index = grouped.map(([h, blurb, list]) => `<h2 id="${h.toLowerCase().replace(/[^a-z]+/g, "-")}">${escapeHtml(h)}</h2>${blurb ? `<p>${escapeHtml(blurb)}</p>` : ""}<div class="std-grid">${list.map((n) => {
    const r = all[`@datars/std/${n}`];
    return `<a class="std-card" href="/docs/std/${n}/"><span class="std-thumb">${thumb(figureAlias(n), `${n} example`)}</span><code>${n}</code><span>${inline(firstSentence(r.doc ?? ""))}</span></a>`;
  }).join("")}</div>`).join("\n");
  pages.push({
    file: "docs/std/index.html",
    slug: "std/",
    title: "Chart reference",
    description: `Every recipe in the datars standard library — ${names.length} charts, marks, guides, maps and controls — each with a live example, its source, parameters, defaults and theme tokens.`,
    lede: `The standard library (<code>@datars/std</code>) is ${names.length} recipes written in TypeScript against the same public SDK you use. Each page below is generated from the recipe itself — its parameters, defaults, documentation and the theme tokens it reads — with a live example you can step through, the example's full source, and real uses from the examples.`,
    body: `${index}
<h2 id="using-recipes">Using a recipe</h2>
<pre><code data-lang="ts">import { plot, bar } from "@datars/std";

plot({ data: "sales", x: "region", y: "sales", children: [bar({ labels: true })] })</code></pre>
<p>Every recipe is a function from typed parameters to a scene. Parameters a recipe doesn't have are reported with the closest one it does (<em>did you mean <code>labels</code>?</em>). Marks inside a <code>plot</code> inherit its <code>data</code>, <code>x</code>, <code>y</code> and <code>color</code>. To change how a recipe draws, copy it into your project with <code>datars eject std/&lt;name&gt;</code> — see <a href="/docs/custom-recipes/">custom recipes</a>. The same reference is available offline as <code>datars describe std/&lt;name&gt;</code>, to agents as <a href="/llms.txt">llms.txt</a>, and over MCP (<code>describe_recipes</code>).</p>`,
    headings: grouped.map(([h]) => ({ level: 2, id: h.toLowerCase().replace(/[^a-z]+/g, "-"), text: h })).concat([{ level: 2, id: "using-recipes", text: "Using a recipe" }]),
  });

  for (const [gi, [group]] of grouped.entries()) {
    for (const n of grouped[gi][2]) {
      const r = all[`@datars/std/${n}`];
      const params = Object.entries(r.params ?? {}).sort(([a], [b]) => a.localeCompare(b));
      const use = usage(n, examples);
      const live = GALLERY[n] && siteCharts[GALLERY[n]]?.recipes.has(`@datars/std/${n}`) ? GALLERY[n] : null;
      const src = sourceFile(root, n);
      const fig = figureFile(n);
      const figSrc = readFileSync(join(root, fig), "utf8").replace(/\s+$/, "");
      const users = examples.filter((ex) => new RegExp(`import[^;]*\\b${n}\\b[^;]*from "@datars/std"`).test(ex.src)).map((ex) => ex.file);
      const i = order.indexOf(n);
      const prev = order[i - 1], next = order[i + 1];
      const table = params.length
        ? `<div class="table-wrap"><table class="params"><thead><tr><th>Parameter</th><th>Type</th><th>Default</th><th>Description</th></tr></thead><tbody>${params.map(([k, p]) => `<tr><td><code>${escapeHtml(k)}</code></td><td>${typeCell(p)}</td><td>${p.default === undefined ? "" : `<code>${escapeHtml(JSON.stringify(p.default))}</code>`}</td><td>${p.doc ? inline(p.doc) : ""}</td></tr>`).join("")}</tbody></table></div>`
        : "<p>No parameters.</p>";
      const body = `
${figure(figureAlias(n))}
<h2 id="example">Example</h2>
<pre><code data-lang="ts">${escapeHtml(figSrc)}</code></pre>
<p class="src-note">The document above, in full: <a href="${blob(fig)}"><code>${fig}</code></a>. Run it: <code>datars dev ${fig}</code></p>
${use ? `<h2 id="in-the-examples">In the examples</h2>
<pre><code data-lang="ts">${escapeHtml(use.code)}</code></pre>
<p class="src-note">From <a href="${blob(use.file)}"><code>${escapeHtml(use.file)}</code></a>.</p>` : ""}
${live ? `<h2 id="gallery">In the gallery</h2>\n${figure(live)}` : ""}
<h2 id="parameters">Parameters</h2>
${table}
${IN_PLOT.has(n) ? `<p class="fine">Inside a <a href="/docs/std/plot/"><code>plot</code></a>, <code>data</code>, <code>x</code>, <code>y</code>, <code>color</code>, <code>xType</code>, <code>yType</code> and <code>series</code> are inherited from it.</p>` : ""}
<h2 id="tokens">Theme tokens</h2>
${r.tokens?.length ? `<p>${r.tokens.map((t) => `<code>${escapeHtml(t)}</code>`).join(" ")}</p><p class="fine">Tokens are late-bound: the theme, the reader's mode (light, dark, high contrast) and the host app's overrides all apply without republishing. See <a href="/docs/theming/">themes and brands</a>.</p>` : "<p>None — this recipe draws only what its parameters say.</p>"}
<h2 id="customise">Make it yours</h2>
<pre><code data-lang="sh">datars describe std/${n}          # this reference, in the terminal
datars eject std/${n}             # copy the recipe into recipes/ to edit</code></pre>
<p>${src ? `The recipe's source: <a href="${blob(src)}"><code>${src}</code></a>. ` : ""}${users.length ? `Used in ${users.length} example document${users.length > 1 ? "s" : ""}: ${users.slice(0, 8).map((f) => `<a href="${blob(f)}"><code>${escapeHtml(f.replace(/\/doc\.ts$/, ""))}</code></a>`).join(", ")}${users.length > 8 ? ", …" : ""}.` : ""}</p>
<nav class="prevnext" aria-label="Previous and next recipe">${prev ? `<a class="pn-prev" href="/docs/std/${prev}/"><small>Previous</small>${prev}</a>` : "<span></span>"}${next ? `<a class="pn-next" href="/docs/std/${next}/"><small>Next</small>${next}</a>` : "<span></span>"}</nav>`;
      pages.push({
        file: `docs/std/${n}/index.html`,
        slug: `std/${n}`,
        group,
        title: `${n}`,
        titleTag: `${n} — ${group} · datars chart reference`,
        description: `${firstSentence(r.doc ?? n)} A live example, its source, parameters, defaults and theme tokens for @datars/std/${n}.`.slice(0, 300),
        lede: inline(r.doc ?? ""),
        body,
        charts: true,
        headings: [{ level: 2, id: "example", text: "Example" }, use && { level: 2, id: "in-the-examples", text: "In the examples" }, live && { level: 2, id: "gallery", text: "In the gallery" }, { level: 2, id: "parameters", text: "Parameters" }, { level: 2, id: "tokens", text: "Theme tokens" }, { level: 2, id: "customise", text: "Make it yours" }].filter(Boolean),
      });
    }
  }
  return pages;
}

function firstSentence(s) {
  // Abbreviations aren't sentence ends ("One square per unit (e.g. one per percent)").
  const t = s.replace(/\b(e\.g|i\.e|etc|vs)\./g, (m) => m.replace(/\./g, "\u0001"));
  const m = t.match(/^(.+?[.!?])(\s|$)/);
  return (m ? m[1] : t).replace(/\u0001/g, ".");
}

export { escapeAttr, firstSentence };
