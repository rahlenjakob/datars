// Build the datars website (site/) into a static folder any host serves — GitHub Pages included,
// under a subpath, with no server: every page as HTML, the web runtime, and every chart published
// as a bundle (manifest `c/<alias>` + content-addressed `chunks/`), exactly as `datars publish`
// writes it. See site/README.md for the pages, the placeholders and how nothing moves on load.
//
//   node scripts/build-site.mjs [--out out/pages] [--fast]
//
// --fast skips the example articles (site/articles, the slow part) and keeps the ones a previous
// build left in the output folder. Needs the CLI (`cargo build --release -p datars-cli`; built here
// if missing) and the web runtime (`scripts/build-wasm.sh && pnpm -C packages/web build`).
// Preview: `datars serve out/pages` (or any static server over the folder).
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, renameSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, relative, resolve } from "node:path";
import { gzipSync } from "node:zlib";
import { escapeAttr, escapeHtml, frontMatter, inline, markdown } from "./site/markdown.mjs";
import { highlight, highlightPage } from "./site/highlight.mjs";
import { DOCS_NAV, FEATURES, docsPrevNext, docsSidebar, docsToc, featureIcon, lookAccents, lookPresets, page, urlOf } from "./site/layout.mjs";
import { STD_GROUPS, figureAlias, firstSentence, stdPages } from "./site/std.mjs";
import { sdkApi } from "./site/sdk.mjs";
import { chartTokens, studioChecks, studioData, studioJson, studioTab } from "./site/studio.mjs";
import { delivered, engineTable, missesTable, nativeSummary, nativeTable, scrollTable, transitionsSummary, transitionsTable, webCards } from "./site/perf.mjs";

const root = resolve(new URL("..", import.meta.url).pathname);
const args = process.argv.slice(2);
const argOut = args.indexOf("--out");
const out = resolve(root, argOut >= 0 ? args[argOut + 1] : "out/pages");
const fast = args.includes("--fast");
const stage = `${out}-stage`; // per output folder: two builds into different folders never share it
const cli = join(root, "target/release/datars");
const dist = join(root, "packages/web/dist");
const siteDir = join(root, "site");
const t0 = Date.now();

/** The repository the site links to (sources, docs): DATARS_REPO_URL, the Actions repository, else
 * this checkout's GitHub remote. */
function gitHubRemote() {
  try {
    const url = execFileSync("git", ["remote", "get-url", "origin"], { cwd: root, stdio: ["ignore", "pipe", "ignore"] }).toString().trim();
    const m = url.match(/github\.com[:/]([^/]+\/[^/.]+)(\.git)?$/);
    return m ? `https://github.com/${m[1]}` : null;
  } catch {
    return null;
  }
}
const repo = (process.env.DATARS_REPO_URL ?? (process.env.GITHUB_REPOSITORY ? `https://github.com/${process.env.GITHUB_REPOSITORY}` : gitHubRemote() ?? "https://github.com/")).replace(/\/$/, "");
/** Where this folder sits in its repository — "" when it is the repository's root, a folder when
 * it is checked out inside a bigger one (DATARS_REPO_PATH overrides): links to files go through it. */
function repoPath() {
  if (process.env.DATARS_REPO_PATH !== undefined) return process.env.DATARS_REPO_PATH.replace(/^\/|\/$/g, "");
  try {
    return execFileSync("git", ["rev-parse", "--show-prefix"], { cwd: root, stdio: ["ignore", "pipe", "ignore"] }).toString().trim().replace(/\/$/, "");
  } catch {
    return "";
  }
}
const prefix = repoPath();
/** A file's page on GitHub, from its path in this folder: `blob("examples/votes/doc.ts")`. */
const blob = (f) => `${repo}/blob/main/${prefix ? `${prefix}/` : ""}${f}`;
/** The site's absolute root (canonical links, Open Graph, the sitemap), without a trailing slash. */
const SITE = (process.env.DATARS_SITE_URL ?? "https://datars.dev").replace(/\/$/, "");
/** The SDK reference's signatures, read from packages/sdk/src: checked first (an export no page
 * documents, or a page naming one that doesn't exist, fails the build before any chart is made). */
const sdk = sdkApi({ root, blob });

/**
 * The charts on the site: alias → document (showcase-only documents live in site/charts/), and how
 * its slot is drawn by default — `mode` (a chart that always plays dark), `min` (px), `phone` (the
 * height/width ratio on narrow screens, for charts that re-lay themselves out as rows).
 */
const CHARTS = {
  hero: { src: "site/charts/hero.ts", mode: "dark", min: 300 },
  renewables: { src: "examples/renewables/doc.json", min: 320 },
  brand: { src: "site/charts/brand.ts", min: 300, phone: 2.3 },
  descent: { src: "examples/descent/doc.json", min: 340 },
  rio: { src: "examples/rio/doc.json", min: 340 },
  budget: { src: "examples/budget/doc.json", min: 300 },
  dashboard: { src: "examples/dashboard/doc.json", min: 280 },
  prices: { src: "examples/prices/doc.json", min: 300 },
  energy: { src: "site/charts/energy.ts", min: 300 },
  warming: { src: "examples/warming/doc.json", min: 260 },
  riksdag: { src: "examples/riksdag/doc.json", min: 280 },
  shapes: { src: "examples/shapes/doc.json", min: 300 },
  votes: { src: "examples/votes/doc.json", min: 280 },
  inflation: { src: "examples/inflation/doc.json", min: 280 },
  scatter: { src: "examples/scatter/doc.json", min: 300 },
  business: { src: "examples/business/doc.json", min: 280 },
  serif: { src: "examples/serif/doc.json", min: 280 },
  spending: { src: "examples/spending/doc.json", min: 280, files: { "examples/spending/web/user.json": "data/spending/this.json", "examples/spending/web/other.json": "data/spending/other.json" } },
  election: { src: "examples/election/doc.json", min: 300, files: { "examples/election/count.json.d": "data/election" } },
  worlds: { src: "examples/worlds/doc.json", mode: "dark", min: 380 },
  galaxy: { src: "examples/galaxy/doc.json", mode: "dark", min: 360, phone: 1.15 },
  stocks: { src: "examples/stocks/doc.json", min: 340, phone: 1.3 },
  drill: { src: "site/charts/europe-drill.ts", min: 340 },
  flows: { src: "examples/flows/doc.json", min: 280 },
  hebrew: { src: "examples/hebrew/doc.json", min: 280 },
  controls: { src: "site/charts/controls.ts", min: 360, phone: 2.0 },
};

// Figures: every document in site/figures/<section>/ is a chart too, alias `<section>-<name>` — the
// chart reference's examples (std/), the SDK reference's (sdk/) and the explanations' diagrams
// (how/). Live like the rest (themes, hover, steps), but not counted or listed as the site's charts.
const figuresDir = join(siteDir, "figures");
if (existsSync(figuresDir)) {
  for (const section of readdirSync(figuresDir).sort()) {
    const dir = join(figuresDir, section);
    if (!statSync(dir).isDirectory()) continue;
    for (const f of readdirSync(dir).sort()) {
      const m = f.match(/^([A-Za-z0-9-]+)\.(ts|json)$/);
      if (m) CHARTS[`${section}-${m[1].toLowerCase()}`] = { src: `site/figures/${section}/${f}`, min: 160, figure: true };
    }
  }
}
// Documents a page rebuilds in the browser (the animation page's playground swaps in the reader's
// motion rule with `setDocument`; the charts page's recipe playground edits any std figure, the
// extensibility page's workbench its own, the developers page swaps in its bench's fixes, the
// interaction lab switches its intents on and off): published as a bundle like any chart, and also
// as the document itself, at `/play/<alias>.json`.
const RAW = ["motion-playground", "interaction-lab", /^(std|charts|extensibility)-/, /^developers-bench-/];
for (const alias of Object.keys(CHARTS)) if (RAW.some((r) => (typeof r === "string" ? r === alias : r.test(alias)))) CHARTS[alias].raw = true;
const showcase = Object.keys(CHARTS).filter((a) => !CHARTS[a].figure);

if (!existsSync(cli)) {
  console.log("building the CLI (cargo build --release -p datars-cli)…");
  execFileSync("cargo", ["build", "--release", "-p", "datars-cli"], { cwd: root, stdio: "inherit" });
}
if (!existsSync(join(dist, "datars.js")) || !existsSync(join(dist, "wasm/datars_core_bg.wasm"))) {
  console.error("the web runtime isn't built: run scripts/build-wasm.sh && pnpm -C packages/web build");
  process.exit(1);
}

// A fast build keeps the articles an earlier build made.
// (A build that failed halfway leaves them in the kept folder; the next one picks them up.)
const keptArticles = `${out}-articles-kept`;
if (fast && existsSync(join(out, "articles"))) {
  rmSync(keptArticles, { recursive: true, force: true });
  renameSync(join(out, "articles"), keptArticles);
}
rmSync(out, { recursive: true, force: true });
rmSync(stage, { recursive: true, force: true });
mkdirSync(join(stage, "c"), { recursive: true });
mkdirSync(out, { recursive: true });

// ---- charts ------------------------------------------------------------------------------------

/** Where a URL relative to `c/<alias>` lands in the site, `..` clamped at the site root (as
 * `datars publish` copies it): `../../assets/x` → `assets/x`, `count.json` → `c/count.json`. */
function sitePath(url) {
  const parts = ["c"];
  for (const seg of url.split("/")) {
    if (seg === "" || seg === ".") continue;
    if (seg === "..") parts.pop();
    else parts.push(seg);
  }
  return parts.join("/");
}

const docs = {};
/** A document's relative URLs (data files, tile archives, fonts) point at its folder in the repo,
 * often above it (`../../assets/tiles/…`). In a site under a subpath, `..` above the site root
 * escapes it, so each URL is rewritten to where the file is published, relative to `c/`, and the
 * file is staged next to the staged document so the build fetches the same bytes. Automatic
 * basemaps (`"auto"`) stay: `datars publish` ships their archive itself. */
function stageDoc(alias, src) {
  const file = join(root, src);
  const json = src.endsWith(".ts")
    ? execFileSync("node", [join(root, "scripts/doc-to-json.mjs"), file], { cwd: root, maxBuffer: 1 << 28 }).toString()
    : readFileSync(file, "utf8");
  const doc = JSON.parse(json);
  for (const s of Object.values(doc.data ?? {})) {
    for (const k of ["url", "tiles", "font"]) {
      const url = s[k];
      if (typeof url !== "string" || url === "auto" || url.includes("://") || url.startsWith("/")) continue;
      const from = resolve(dirname(file), url);
      if (!existsSync(from)) continue;
      const at = sitePath(url);
      mkdirSync(dirname(join(stage, at)), { recursive: true });
      copyFileSync(from, join(stage, at));
      s[k] = at.startsWith("c/") ? at.slice(2) : `../${at}`;
    }
  }
  // Theme font files are read at build time (and shipped as subsets): point them at the repo.
  const fonts = (v) => {
    if (Array.isArray(v)) v.forEach(fonts);
    else if (v && typeof v === "object") {
      if (typeof v.src === "string" && !v.src.includes("://") && !v.src.startsWith("datars:") && !v.src.startsWith("/")) v.src = resolve(dirname(file), v.src);
      Object.values(v).forEach(fonts);
    }
  };
  fonts(doc.theme);
  // An automatic basemap is found next to the document: stage its archive and lockfile with it.
  const staged = join(stage, "c", `${alias}.json`);
  for (const [name, s] of Object.entries(doc.data ?? {})) {
    if (s.tiles !== "auto") continue;
    for (const ext of ["auto.pmtiles", "auto.job.json"]) {
      const f = join(dirname(file), `${name}.${ext}`);
      if (existsSync(f)) copyFileSync(f, join(stage, "c", `${name}.${ext}`));
    }
  }
  writeFileSync(staged, JSON.stringify(doc));
  docs[alias] = doc;
  return staged;
}

const kb = (n) => (n < 1024 ? `${n} B` : `${(n / 1024).toFixed(n < 10240 ? 1 : 0)} KB`);
const mb = (n) => `${(n / 1048576).toFixed(n < 10485760 ? 1 : 0)} MB`;
const aspects = {}, sizes = {}, tiers = {}, archives = {}, staged = {};
for (const [alias, c] of Object.entries(CHARTS)) {
  const file = stageDoc(alias, c.src);
  staged[alias] = file;
  aspects[alias] = (docs[alias].size?.height ?? 480) / (docs[alias].size?.width ?? 800);
  execFileSync(cli, ["check", file], { stdio: ["ignore", "ignore", "inherit"] });
  const r = JSON.parse(execFileSync(cli, ["publish", file, "--alias", alias, "--to", out, "--json"], { maxBuffer: 1 << 26 }).toString());
  // What the default (core) web runtime downloads: the pre-expanded variant (T2), else baked (T1).
  sizes[alias] = r.gzip.T2 ?? r.gzip.T1 ?? r.gzip.T3;
  tiers[alias] = r.gzip;
  // Bytes of the archives a chart reads by range (point pyramids, basemaps): on the server, not
  // downloaded — a reader fetches the parts in view.
  archives[alias] = (r.copied ?? []).filter((f) => f.endsWith(".pmtiles")).reduce((n, f) => n + statSync(join(out, f)).size, 0);
  if (!c.figure) console.log(`${alias.padEnd(12)} ${kb(sizes[alias]).padStart(8)} gzipped  (${c.src})`);
  if (c.raw) {
    mkdirSync(join(out, "play"), { recursive: true });
    copyFileSync(file, join(out, "play", `${alias}.json`));
  }
}
// The delivery page's trust demo: the same chart signed by "the newsroom", signed and then changed in
// transit, and signed by someone else — each with a key made for this build (`datars keygen`; the
// secrets stay in a temporary folder and are gone after the build). `{{signer:newsroom}}` and
// `{{signer:someone}}` are their public keys, for the page's `publishers`.
const signers = {};
if (staged["delivery-v3"]) {
  const keys = mkdtempSync(join(tmpdir(), "datars-site-keys-"));
  try {
    for (const [who, alias] of [["newsroom", "delivery-signed"], ["someone", "delivery-foreign"]]) {
      signers[who] = JSON.parse(execFileSync(cli, ["keygen", "--out", join(keys, `${who}.key`), "--json"]).toString()).publisher;
      execFileSync(cli, ["publish", staged["delivery-v3"], "--alias", alias, "--to", out, "--sign", join(keys, `${who}.key`)], { stdio: "ignore" });
    }
  } finally {
    rmSync(keys, { recursive: true, force: true });
  }
  // One field changed after signing (the title, as a proxy in the middle might): the signature no
  // longer covers what's there.
  const m = JSON.parse(readFileSync(join(out, "c", "delivery-signed"), "utf8"));
  m.title = `${m.title ?? "Chart"} (edited in transit)`;
  writeFileSync(join(out, "c", "delivery-tampered"), JSON.stringify(m));
}
// Files a page's demo reads besides the chart (`files`: repo path → site path; a folder copies its
// files): accounts to hand a data slot, a live feed to replay.
for (const c of Object.values(CHARTS)) {
  for (const [from, to] of Object.entries(c.files ?? {})) {
    const src = join(root, from);
    const list = statSync(src).isDirectory() ? readdirSync(src).map((f) => [join(src, f), join(out, to, f)]) : [[src, join(out, to)]];
    for (const [a, b] of list) {
      mkdirSync(dirname(b), { recursive: true });
      copyFileSync(a, b);
    }
  }
}
// The spending chart's own sample, as rows: what the data page's "Sample" hands back.
{
  const sample = docs.spending?.data?.spending?.sample;
  if (sample) writeFileSync(join(out, "data/spending/sample.json"), JSON.stringify(sample));
}
const figureCount = Object.keys(CHARTS).length - showcase.length;
// The gallery shows every chart the site publishes: one left out is a build error, not a gap.
{
  const gallery = readFileSync(join(siteDir, "pages/gallery.html"), "utf8");
  const missing = showcase.filter((a) => !gallery.includes(`data-chart="${a}"`));
  if (missing.length) throw new Error(`site/pages/gallery.html: no card for ${missing.join(", ")} (every chart the site publishes belongs in the gallery)`);
}
if (figureCount) console.log(`figures      ${figureCount} (site/figures)`);

/** The recipes each chart's document uses (for the chart reference's live examples). */
const siteCharts = {};
for (const [alias, doc] of Object.entries(docs)) {
  const recipes = new Set();
  const walk = (v) => {
    if (Array.isArray(v)) v.forEach(walk);
    else if (v && typeof v === "object") {
      if (v.kind === "use" && typeof v.recipe === "string") recipes.add(v.recipe);
      Object.values(v).forEach(walk);
    }
  };
  walk(doc.scene);
  siteCharts[alias] = { recipes };
}

/** A chart's accessible text (the bundle's `a11y` chunk): title, description, every state. */
function a11y(alias) {
  const m = JSON.parse(readFileSync(join(out, "c", alias), "utf8"));
  const h = m.chunks.find((c) => c.kind === "a11y")?.hash;
  return h ? JSON.parse(readFileSync(join(out, "chunks", h.replace(":", "_")), "utf8")) : null;
}

/** The text alternative under a chart: real, crawlable text from the chart's own semantics. */
function altText(alias) {
  const t = a11y(alias);
  if (!t) return "";
  const title = t.title || docs[alias].title || alias;
  const desc = t.description || docs[alias].description || "";
  const states = (t.states ?? []).map((s) => {
    const labels = [];
    const seen = new Set();
    for (const it of s.items ?? []) {
      if (!["datum", "region", "series", "control"].includes(it.role) || !it.label || /: no data$/.test(it.label) || seen.has(it.label)) continue;
      seen.add(it.label);
      labels.push(it.label);
    }
    const shown = labels.slice(0, 12);
    const more = labels.length - shown.length;
    const n = s.narration ?? {};
    const head = [n.title, n.text].filter(Boolean).join(" — ");
    return `<li><b>${escapeHtml(s.state ?? "")}</b>${head ? ` — ${escapeHtml(head)}` : ""}${shown.length ? `<br><span>${escapeHtml(shown.join("; "))}${more > 0 ? ` (and ${more} more)` : ""}</span>` : ""}</li>`;
  });
  return `<details class="alt"><summary>Text description</summary><div><p><b>${escapeHtml(title)}.</b> ${escapeHtml(desc)}</p>${states.length > 1 || states[0]?.includes("<br>") ? `<ol>${states.join("")}</ol>` : ""}</div></details>`;
}

/** A story's steps and its first step's narration, under its chart: written here so the bar holds
 * its space before the chart mounts (the page script wires it to the view). */
function captionBar(alias) {
  const states = docs[alias].program?.states ?? [];
  if (states.length < 2) return "";
  const cap = (n) => n.replace(/^./, (c) => c.toUpperCase());
  // A chart that draws its narration on the canvas (expressions reading `narration.*`, a std card)
  // doesn't get it repeated under it.
  const n = JSON.stringify(docs[alias].scene).includes("narration.") ? {} : states[0].narration ?? {};
  return `<div class="caption"><div class="pills small" role="group" aria-label="Steps">${states.map((st, i) => `<button type="button" data-state="${escapeAttr(st.name)}" aria-pressed="${i === 0}">${escapeHtml(cap(st.name))}</button>`).join("")}</div><p aria-hidden="true">${n.title ? `<b>${escapeHtml(n.title)}</b>` : ""}${n.text ? `${n.title ? " — " : ""}${escapeHtml(n.text)}` : ""}</p></div>`;
}

/** A chart's first state as SVG, once per theme mode (the page's theme shows one): a thumbnail for
 * indexes, e.g. the chart reference's. A heavy SVG — a map's coastlines, a cloud's dots — becomes
 * a PNG at the document's own size instead: an index of fifty shouldn't weigh megabytes. */
const THUMB_SVG_MAX = 100 * 1024;
const thumbExt = {};
function thumb(alias, alt = "") {
  if (!thumbExt[alias]) {
    mkdirSync(join(out, "thumbs"), { recursive: true });
    const render = (mode, ext, extra = []) => {
      const file = join(out, "thumbs", `${alias}-${mode}.${ext}`);
      execFileSync(cli, ["render", staged[alias], "--state", "0", "--mode", mode, ...extra, "--out", file], { stdio: "ignore", cwd: dirname(staged[alias]) });
      return file;
    };
    thumbExt[alias] = "svg";
    for (const mode of ["light", "dark"]) {
      const svg = render(mode, "svg");
      if (statSync(svg).size > THUMB_SVG_MAX) thumbExt[alias] = "png";
    }
    if (thumbExt[alias] === "png") {
      for (const mode of ["light", "dark"]) {
        rmSync(join(out, "thumbs", `${alias}-${mode}.svg`));
        render(mode, "png", ["--dpr", "1"]);
      }
    }
  }
  const ext = thumbExt[alias];
  const w = docs[alias].size?.width ?? 800, h = docs[alias].size?.height ?? 480;
  return `<img class="thumb thumb-light" src="/thumbs/${alias}-light.${ext}" alt="${escapeAttr(alt)}" width="${w}" height="${h}" loading="lazy" decoding="async"><img class="thumb thumb-dark" src="/thumbs/${alias}-dark.${ext}" alt="" aria-hidden="true" width="${w}" height="${h}" loading="lazy" decoding="async">`;
}

/** A live chart with its caption and text alternative (the chart reference uses it). */
function figure(alias, caption) {
  const d = docs[alias];
  const text = caption ?? `<b>${escapeHtml(d.title ?? alias)}</b>${d.description ? ` — ${escapeHtml(d.description)}` : ""}`;
  const more = CHARTS[alias].figure ? "" : ` <a href="/gallery/#${alias}">In the gallery</a>`;
  return `<figure class="fig"><div class="frame"><div class="chart" data-chart="${alias}" data-caption></div></div><figcaption>${text}${more}</figcaption>{{alt:${alias}}}</figure>`;
}

// ---- rendered artefacts: social cards, exports, films -------------------------------------------

mkdirSync(join(out, "og"), { recursive: true });
const cards = new Set();
/** A chart's first state drawn by the CPU reference at 1200×630: a page's social card. */
function card(alias) {
  if (!cards.has(alias)) {
    execFileSync(cli, ["render", staged[alias], "--state", "0", "--mode", CHARTS[alias].mode ?? "light", "--dpr", "1", "--size", "1200x630", "--out", join(out, "og", `${alias}.png`)], { stdio: "ignore", cwd: dirname(staged[alias]) });
    cards.add(alias);
  }
  return `og/${alias}.png`;
}
card("worlds");
copyFileSync(join(out, "og/worlds.png"), join(out, "card.png"));

// The same document as a PNG, an SVG and a vector PDF, and as films with captions — what the
// platforms page shows and links to.
mkdirSync(join(out, "exports"), { recursive: true });
const exportsMade = [];
for (const ext of ["png", "svg", "pdf"]) {
  execFileSync(cli, ["render", staged.riksdag, "--state", "0", "--dpr", "2", "--out", join(out, "exports", `riksdag.${ext}`)], { stdio: "ignore" });
  exportsMade.push(`riksdag.${ext}`);
}
// The built-in theme's specimen board: std charts, palettes and inks in light, dark and high contrast.
execFileSync(cli, ["theme", "datars/neutral", "--specimen", join(out, "exports", "specimen.png")], { stdio: "ignore" });
const hasFfmpeg = (() => { try { execFileSync("ffmpeg", ["-version"], { stdio: "ignore" }); return true; } catch { return false; } })();
const films = {};
for (const [name, alias, size, poster] of [["worlds-9x16", "worlds", "360x640", 2], ["descent-16x9", "descent", "640x360", 2]]) {
  // The poster frame (a state the film holds), from the CPU reference: the page shows it before the film loads.
  execFileSync(cli, ["render", staged[alias], "--state", String(poster), "--size", size, "--dpr", "2", "--mode", CHARTS[alias].mode ?? "light", "--out", join(out, "exports", `${name}.png`)], { stdio: "ignore" });
  if (hasFfmpeg) {
    execFileSync(cli, ["video", staged[alias], "--size", size, "--dpr", "2", "--out", join(out, "exports", `${name}.mp4`)], { stdio: "ignore" });
    films[name] = { size, bytes: statSync(join(out, "exports", `${name}.mp4`)).size };
  }
}
if (!hasFfmpeg) console.log("no ffmpeg: the platforms page shows film posters without the films");

/** A chart rendered by the CLI for a page to show or offer for download, made once per spec:
 * `{{export:alias|state|WxH|dpr|png|light}}` (the extension picks PNG, SVG or PDF) → its URL;
 * `{{export:alias|-|WxH|dpr|mp4|light}}` → the whole program as a film, its WebVTT captions beside
 * it (no ffmpeg: no film, and an empty URL). `{{exportsize:…}}` (the same spec) → its size. */
const made = new Map();
function exported(spec) {
  if (!made.has(spec)) {
    const [alias, state, size, dpr = "2", ext = "png", mode = "light"] = spec.split("|").map((x) => x.trim());
    if (!(alias in CHARTS)) throw new Error(`{{export:${spec}}}: no chart \`${alias}\``);
    const name = `${alias}-${ext === "mp4" ? "film" : state}-${size}@${dpr}-${mode}.${ext}`;
    const file = join(out, "exports", name);
    const common = ["--size", size, "--dpr", dpr, "--mode", mode, "--out", file];
    if (ext === "mp4") {
      if (hasFfmpeg) execFileSync(cli, ["video", staged[alias], ...common], { stdio: "ignore", cwd: dirname(staged[alias]) });
    } else {
      execFileSync(cli, ["render", staged[alias], "--state", state, ...common], { stdio: "ignore", cwd: dirname(staged[alias]) });
    }
    made.set(spec, existsSync(file) ? { url: `/exports/${name}`, bytes: statSync(file).size } : { url: "", bytes: 0 });
  }
  return made.get(spec);
}

// ---- the runtime, fonts, styles ----------------------------------------------------------------

mkdirSync(join(out, "runtime/wasm"), { recursive: true });
copyFileSync(join(dist, "datars.js"), join(out, "runtime/datars.js"));
for (const f of readdirSync(join(dist, "wasm"))) {
  if (/^(datars_core|datars_host_web|wasi_shim)/.test(f)) copyFileSync(join(dist, "wasm", f), join(out, "runtime/wasm", f));
}
// The built-in atlas and default fonts, for documents a page builds in the browser (bundles carry
// their own geometry and font subsets): `datars:atlas/…` and `datars:fonts/…` resolve next to the
// runtime. Without the fonts, the theme studio's resolver, the extensibility workbench and the
// developers bench asked for Inter and got 404s.
for (const dir of ["atlas", "fonts"]) {
  if (!existsSync(join(dist, dir))) continue;
  mkdirSync(join(out, "runtime", dir), { recursive: true });
  for (const f of readdirSync(join(dist, dir))) copyFileSync(join(dist, dir, f), join(out, "runtime", dir, f));
}
const runtimeGz = gzipSync(readFileSync(join(dist, "wasm/datars_core_bg.wasm")), { level: 9 }).length + gzipSync(readFileSync(join(dist, "datars.js")), { level: 9 }).length;
const runtime = `${(runtimeGz / 1048576).toFixed(1)} MB`;

mkdirSync(join(out, "fonts"), { recursive: true });
for (const f of ["Inter-Regular.ttf", "Inter-SemiBold.ttf", "Inter-Bold.ttf", "Inter-LICENSE.txt"]) copyFileSync(join(root, "fonts", f), join(out, "fonts", f));
// The type a reader can pick in "Make it yours" (site.js): whole faces, fetched only when chosen.
for (const f of ["Newsreader-Regular.ttf", "Newsreader-SemiBold.ttf", "Newsreader-OFL.txt"]) copyFileSync(join(root, "assets/fonts", f), join(out, "fonts", f));
for (const f of readdirSync(join(root, "site/fonts"))) copyFileSync(join(root, "site/fonts", f), join(out, "fonts", f));
// The site's own scripts and stylesheets: site.js and site.css on every page, and page scripts and
// stylesheets (motion.js, studio.js, a page's `css:`) where a page asks for them.
for (const f of readdirSync(siteDir).filter((f) => /\.(js|css)$/.test(f))) copyFileSync(join(siteDir, f), join(out, f));
// Screenshots and other images the pages (and the README) show.
if (existsSync(join(siteDir, "img"))) {
  mkdirSync(join(out, "img"), { recursive: true });
  for (const f of readdirSync(join(siteDir, "img"))) copyFileSync(join(siteDir, "img", f), join(out, "img", f));
}
// For agents and tools: the compact index and the document format, at stable URLs.
copyFileSync(join(root, "llms.txt"), join(out, "llms.txt"));
mkdirSync(join(out, "schema"), { recursive: true });
copyFileSync(join(root, "docs/reference/ir.schema.json"), join(out, "schema/ir-1.json"));

// ---- generated content -------------------------------------------------------------------------

const std = JSON.parse(execFileSync(cli, ["describe", "--json"], { maxBuffer: 1 << 26 }).toString());
const recipeCount = Object.keys(std).length;
// Every recipe's parameters, defaults and docs, and the catalogue's families, for the pages that
// build documents in the browser (the charts page's recipe playground makes its controls from them).
mkdirSync(join(out, "play"), { recursive: true });
writeFileSync(join(out, "play", "std.json"), JSON.stringify({ groups: STD_GROUPS, recipes: std }));
const help = execFileSync(cli, ["help"]).toString();
/** The MCP server's tools, from its source (the same list `tools/list` answers). */
const mcpTools = [...readFileSync(join(root, "crates/datars-mcp/src/main.rs"), "utf8").matchAll(/\{ "name": "(\w+)", "description": "((?:[^"\\]|\\.)*)"/g)].map((m) => [m[1], m[2].replace(/\\"/g, '"')]);

/** The std recipes as small cards, grouped, each with its figure's thumbnail (the charts feature
 * page; the chart reference's index shows the same thumbnails). */
function stdList() {
  const names = Object.keys(std).map((id) => id.replace("@datars/std/", ""));
  return STD_GROUPS.map(([h, blurb, list]) => {
    const here = list.filter((n) => names.includes(n));
    if (!here.length) return "";
    return `<div class="std-group"><h3>${escapeHtml(h)}</h3><p>${escapeHtml(blurb)}</p><div class="std-minis">${here.map((n) => `<a class="std-mini" href="/docs/std/${n}/" title="${escapeAttr(firstSentence(std[`@datars/std/${n}`].doc ?? ""))}">${CHARTS[figureAlias(n)] ? `<span class="std-thumb">${thumb(figureAlias(n), "")}</span>` : ""}<code>${n}</code></a>`).join("")}</div></div>`;
  }).join("");
}

/** A recipe family's slug (the chart reference's heading ids): "Parts of a whole and flows" →
 * "parts-of-a-whole-and-flows". */
const familySlug = (h) => h.toLowerCase().replace(/[^a-z]+/g, "-").replace(/^-|-$/g, "");
/** Every recipe as a card in the gallery's wall, family by family: its live figure (a slot like
 * any chart's), its family, the first sentence of its documentation, and links to its reference
 * page and the charts page's playground (`{{std:cards}}`). Recipes no family lists land in "More". */
function stdCards() {
  const names = Object.keys(std).map((id) => id.replace("@datars/std/", ""));
  const listed = new Set(STD_GROUPS.flatMap(([, , l]) => l));
  const groups = [...STD_GROUPS, ["More", "", names.filter((n) => !listed.has(n))]];
  return groups.map(([h, blurb, list]) => {
    const here = list.filter((n) => names.includes(n) && CHARTS[figureAlias(n)]);
    if (!here.length) return "";
    const fam = familySlug(h);
    const cards = here.map((n) => {
      const doc = std[`@datars/std/${n}`].doc ?? "";
      const a = figureAlias(n);
      return `<article class="g-card g-type" id="${a}" data-kind="types" data-families="${fam}" data-tags="" data-words="${escapeAttr(`${n} ${h} ${doc}`)}"><div class="chart" data-chart="${a}"></div><div class="g-text"><p class="g-kicker">${escapeHtml(h)}</p><h3><code>${escapeHtml(n)}</code></h3><p>${inline(firstSentence(doc))}</p><div class="g-foot"><span class="g-meta"><a href="/docs/std/${n}/">Reference</a> · <a href="/features/charts/#play=${n}">Playground</a></span><button type="button" class="g-open" data-lab="${a}" aria-label="Open ${escapeAttr(n)} in the lab">Open in the lab<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M14 4h6v6M20 4l-8 8M10 5H5v14h14v-5"/></svg></button></div></div></article>`;
    }).join("");
    return `<section class="g-group g-family" id="${fam}" data-group="types" data-family="${fam}"><div class="wrap"><header class="g-group-head"><h3 class="g-fam-h">${escapeHtml(h)} <small>${here.length}</small></h3>${blurb ? `<p>${escapeHtml(blurb)}</p>` : ""}</header><div class="g-grid g-grid-types">${cards}</div></div></section>`;
  }).join("\n");
}
/** The recipe families a site chart's document uses (`{{families:votes}}`), as slugs. */
function familiesOf(alias) {
  const used = siteCharts[alias]?.recipes ?? new Set();
  return STD_GROUPS.filter(([, , l]) => l.some((n) => used.has(`@datars/std/${n}`))).map(([h]) => familySlug(h)).join(" ");
}

/** `datars help` as a table: every command, its arguments and what it does. */
function cliTable() {
  const rows = help.split("\n").filter((l) => /^ {2}[a-z]/.test(l)).map((l) => {
    const m = l.trim().match(/^(\S+)\s+(.*?)\s{2,}(\S.*)$/) ?? l.trim().match(/^(\S+)\s+()(.*)$/);
    return m ? [m[1], m[2], m[3]] : null;
  }).filter(Boolean);
  return `<div class="table-wrap"><table class="cli"><thead><tr><th>Command</th><th>Arguments</th><th>What it does</th></tr></thead><tbody>${rows.map(([c, a, d]) => `<tr><td id="cmd-${c}"><code>datars ${escapeHtml(c)}</code></td><td><code>${escapeHtml(a)}</code></td><td>${escapeHtml(d)}</td></tr>`).join("")}</tbody></table></div>`;
}

/** A transition as a filmstrip from `datars film`, made by this build (`{{strip:examples/votes/doc.json|1|2|6}}`:
 * document, from state, to state, frames). Light only: it shows the CPU reference's own pixels. */
function strip(spec) {
  const [doc, from, to, frames = "6"] = spec.split("|").map((x) => x.trim());
  const name = `strip-${doc.replace(/[^a-z0-9]+/gi, "-")}-${from}-${to}`;
  mkdirSync(join(out, "img"), { recursive: true });
  execFileSync(cli, ["film", join(root, doc), "--from", from, "--to", to, "--frames", frames, "--out", join(out, "img", name)], { stdio: "ignore", cwd: dirname(join(root, doc)) });
  // Its size from the PNG header, so the page reserves its box before it loads (nothing moves).
  const png = readFileSync(join(out, "img", `${name}-strip.png`));
  return `<img class="strip" src="/img/${name}-strip.png" width="${png.readUInt32BE(16)}" height="${png.readUInt32BE(20)}" alt="Frames of the transition from state ${escapeAttr(from)} to state ${escapeAttr(to)}, left to right" loading="lazy">`;
}

const runCache = new Map();
/** A CLI command's real output (`{{run:semantics examples/budget/doc.json|24}}`: at most 24 lines). */
function run(spec) {
  if (!runCache.has(spec)) {
    const [cmd, max] = spec.split("|");
    const argv = cmd.trim().split(/\s+/).map((a) => a.replace(/^'(.*)'$/, "$1"));
    let text;
    try {
      text = execFileSync(cli, argv, { cwd: root, maxBuffer: 1 << 26, stdio: ["ignore", "pipe", "pipe"] }).toString();
    } catch (e) {
      text = (e.stdout?.toString() ?? "") + (e.stderr?.toString() ?? "");
    }
    let lines = text.replace(/\s+$/, "").split("\n");
    const n = Number(max) || 40;
    if (lines.length > n) lines = [...lines.slice(0, n), `… (${lines.length - n} more lines)`];
    runCache.set(spec, `<pre class="output"><code><span class="prompt">$ datars ${escapeHtml(cmd.trim())}</span>\n${escapeHtml(lines.join("\n"))}</code></pre>`);
  }
  return runCache.get(spec);
}

/** A file from the repo (`path#L10-40` for a range), highlighted by its extension. */
function code(spec) {
  const [path, range] = spec.split("#");
  let text = readFileSync(join(root, path), "utf8").replace(/\s+$/, "");
  if (range) {
    const [a, b] = range.replace(/L/g, "").split("-").map(Number);
    text = text.split("\n").slice(a - 1, b || a).join("\n");
  }
  const lang = { ts: "ts", mjs: "js", js: "js", json: "json", swift: "swift", kt: "kotlin", html: "html", sh: "sh" }[path.split(".").pop()] ?? "";
  return `<div class="code-title"><a href="${blob(path)}">${escapeHtml(path)}</a></div><pre><code data-lang="${lang}" data-hl>${highlight(text, lang)}</code></pre>`;
}

const perfDir = join(siteDir, "perf");
function perf(spec) {
  const [kind, name, extra] = spec.split(":");
  // {{perf:delivered:warm,cold,phone,native}}: the headline cards for every platform, computed from
  // those runs; {{perf:webcards:warm,scroll}}: the web's own (stall, opening a chart, layout shift).
  if (kind === "delivered") { const [warm, cold, phone, native] = name.split(","); return delivered(perfDir, { warm, cold, phone, native }); }
  if (kind === "webcards") { const [warm, scroll] = name.split(","); return webCards(perfDir, { warm, scroll }); }
  if (kind === "transitions") return transitionsTable(perfDir, name);
  if (kind === "native") return nativeTable(perfDir, name);
  // {{perf:nativesummary:native:iOS}}: `10 of 10` steady on that platform.
  if (kind === "nativesummary") return nativeSummary(perfDir, name, extra);
  // {{perf:engine:engine}}: the engine's own costs per chart (scripts/bench-engine.mjs).
  if (kind === "engine") return engineTable(perfDir, name);
  // {{perf:misses:transitions,…,native:native}}: every transition that wasn't steady, anywhere.
  if (kind === "misses") return missesTable(perfDir, spec.slice("misses:".length).split(","));
  if (kind === "scroll") return scrollTable(perfDir, name, (p) => (p ? p.replace(/\/$/, "") : "home"));
  if (kind === "summary") {
    const s = transitionsSummary(perfDir, name);
    return extra === "steady" ? `${s.steady} of ${s.total}` : extra === "clean" ? `${s.clean} of ${s.total}` : extra === "stall" ? `${s.stall} ms` : `${s.steady} of ${s.total}`;
  }
  throw new Error(`{{perf:${spec}}}: unknown`);
}

/** Every chart's bundle sizes per tier (the delivery page). */
function sizeTable() {
  const tier = (g, t) => (g[t] ? kb(g[t]) : "—");
  return `<div class="table-wrap"><table class="perf"><thead><tr><th>Chart</th><th class="num">T0 poster + text</th><th class="num">T1 baked</th><th class="num">T2 pre-expanded</th><th class="num">T3 source</th><th class="num">Archive, read by range</th></tr></thead><tbody>${showcase.map((a) => `<tr><td><a href="/gallery/#${a}">${escapeHtml(docs[a].title ?? a)}</a></td><td class="num">${tier(tiers[a], "T0")}</td><td class="num">${tier(tiers[a], "T1")}</td><td class="num">${tier(tiers[a], "T2")}</td><td class="num">${tier(tiers[a], "T3")}</td><td class="num">${archives[a] ? mb(archives[a]) : "—"}</td></tr>`).join("")}</tbody></table></div>`;
}

/** One chart's variants as bars (`{{variants:votes}}`): each as long as its gzipped first load, made
 * of its chunks coloured by kind — what each tier carries and which chunks they share. Measured by
 * this build from the published manifest and chunks; lengths and labels are the `{{tier:…}}` sizes. */
function variantBars(alias) {
  const m = JSON.parse(readFileSync(join(out, "c", alias), "utf8"));
  const file = (h) => readFileSync(join(out, "chunks", h.replace(":", "_")));
  const meta = Object.fromEntries(m.chunks.map((c) => [c.hash, c]));
  const KINDS = [["poster", "poster"], ["a11y", "accessible text"], ["doc", "document"], ["data", "data"], ["font", "font subsets"], ["program", "program"], ["scene", "baked scenes"]];
  const order = (k) => { const i = KINDS.findIndex(([n]) => n === k); return i < 0 ? KINDS.length : i; };
  const NAMES = { T0: "poster + text", T1: "baked scenes", T2: "pre-expanded", T3: "source" };
  const rows = m.variants.map((v) => {
    const entry = JSON.parse(file(v.entry).toString());
    // Lazily loaded chunks (script subsets) aren't part of a first load, as in the tier sizes.
    const parts = [...(entry.chunks ?? []).filter((h) => !meta[h]?.lazy), v.entry]
      .map((h) => ({ kind: h === v.entry ? "entry" : meta[h]?.kind ?? "other", what: meta[h]?.meta?.state ?? meta[h]?.meta?.face ?? "", gz: gzipSync(file(h), { level: 9 }).length }))
      .sort((a, b) => order(a.kind) - order(b.kind));
    return { tier: v.tier, parts, total: tiers[alias][v.tier] ?? 0 };
  }).sort((a, b) => a.tier.localeCompare(b.tier));
  const max = Math.max(...rows.map((r) => r.total));
  const used = new Set(rows.flatMap((r) => r.parts.map((p) => p.kind)));
  const bar = (r) => {
    const sum = r.parts.reduce((n, p) => n + p.gz, 0) || 1;
    return `<span class="hv-track" style="width:${((100 * r.total) / max).toFixed(1)}%">${r.parts.map((p) => `<i class="hv-${KINDS.some(([n]) => n === p.kind) ? p.kind : "other"}" style="flex:${((100 * p.gz) / sum).toFixed(2)}" title="${escapeAttr([KINDS.find(([n]) => n === p.kind)?.[1] ?? p.kind, p.what].filter(Boolean).join(" · "))}"></i>`).join("")}</span>`;
  };
  return `<div class="hv">${rows.map((r) => `<div class="hv-row"><span class="hv-tier"><b>${r.tier}</b> ${NAMES[r.tier] ?? ""}</span>${bar(r)}<span class="hv-size">${kb(r.total)}</span></div>`).join("")}<div class="hv-key" aria-hidden="true">${KINDS.filter(([n]) => used.has(n)).map(([n, l]) => `<span><i class="hv-${n}"></i>${l}</span>`).join("")}</div></div>`;
}

// ---- pages -------------------------------------------------------------------------------------

/** The theme studio's starting data (the built-in theme, resolved by the CLI per mode), once. */
let studioCache = null;
const studio = () => (studioCache ??= studioData(root, cli));
/** A chart's first state as the engine resolves it (`datars inspect`): the inks it draws with. */
function drawnInks(alias) {
  try {
    return execFileSync(cli, ["inspect", staged[alias]], { cwd: dirname(staged[alias]), maxBuffer: 1 << 28, stdio: ["ignore", "pipe", "ignore"] }).toString();
  } catch {
    return "";
  }
}

/** Fill a page body's placeholders and chart slots (see site/README.md). */
function fill(html, where) {
  const known = (a) => {
    if (!(a in CHARTS)) throw new Error(`${where}: no chart \`${a}\``);
    return a;
  };
  let s = html
    .replaceAll("{{repo}}", repo)
    .replaceAll("{{src}}", blob("").replace(/\/$/, ""))
    .replace(/\{\{signer:(newsroom|someone)\}\}/g, (_, w) => signers[w] ?? (() => { throw new Error(`${where}: {{signer:${w}}} needs the delivery-v3 figure`); })())
    .replaceAll("{{runtime}}", runtime)
    .replaceAll("{{count:recipes}}", String(recipeCount))
    .replaceAll("{{count:charts}}", String(showcase.length))
    .replaceAll("{{count:mcp}}", String(mcpTools.length))
    .replaceAll("{{std:list}}", stdList())
    .replace(/\{\{std:cards\}\}/g, () => stdCards())
    .replace(/\{\{families:([a-z0-9_-]+)\}\}/g, (_, a) => familiesOf(known(a)))
    .replaceAll("{{look:presets}}", lookPresets())
    .replaceAll("{{look:accents}}", lookAccents())
    .replaceAll("{{features:grid}}", `<div class="feature-grid">${FEATURES.map((f) => `<a class="feature-card" href="/features/${f.slug}/">${featureIcon(f)}<b>${escapeHtml(f.name)}</b><span>${escapeHtml(f.blurb)}</span></a>`).join("")}</div>`)
    .replaceAll("{{cli:table}}", cliTable())
    .replaceAll("{{sizes:table}}", sizeTable())
    .replaceAll("{{mcp:tools}}", `<div class="table-wrap"><table><thead><tr><th>Tool</th><th>What it returns</th></tr></thead><tbody>${mcpTools.map(([n, d]) => `<tr><td><code>${n}</code></td><td>${inline(d)}</td></tr>`).join("")}</tbody></table></div>`)
    .replace(/\{\{size:([a-z0-9_-]+)\}\}/g, (_, a) => kb(sizes[known(a)]))
    .replace(/\{\{tier:([a-z0-9_-]+):(T\d)\}\}/g, (_, a, t) => kb(tiers[known(a)][t] ?? 0))
    .replace(/\{\{variants:([a-z0-9_-]+)\}\}/g, (_, a) => variantBars(known(a)))
    .replace(/\{\{archive:([a-z0-9_-]+)\}\}/g, (_, a) => {
      if (!archives[known(a)]) throw new Error(`${where}: chart \`${a}\` reads no archive`);
      return mb(archives[a]);
    })
    .replace(/\{\{film:([a-z0-9x_-]+)\}\}/g, (_, f) => films[f] ? `<video controls playsinline preload="none" poster="/exports/${f}.png" width="${films[f].size.split("x")[0]}" height="${films[f].size.split("x")[1]}"><source src="/exports/${f}.mp4" type="video/mp4"><track kind="captions" src="/exports/${f}.vtt" srclang="en" label="English" default></video>` : `<img src="/exports/${f}.png" alt="A frame of the film" width="${f.includes("9x16") ? 360 : 640}" height="${f.includes("9x16") ? 640 : 360}">`)
    .replace(/\{\{filmsize:([a-z0-9x_-]+)\}\}/g, (_, f) => (films[f] ? mb(films[f].bytes) : "—"))
    .replace(/\{\{export:([^}]+)\}\}/g, (_, p) => exported(p).url)
    .replace(/\{\{exportsize:([^}]+)\}\}/g, (_, p) => { const b = exported(p).bytes; return b ? (b < 1048576 ? kb(b) : mb(b)) : "—"; })
    .replace(/\{\{alt:([a-z0-9_-]+)\}\}/g, (_, a) => altText(known(a)))
    .replace(/\{\{code:([^}]+)\}\}/g, (_, p) => code(p))
    .replace(/\{\{run:([^}]+)\}\}/g, (_, p) => run(p))
    .replace(/\{\{strip:([^}]+)\}\}/g, (_, p) => strip(p))
    .replace(/\{\{perf:([^}]+)\}\}/g, (_, p) => perf(p))
    .replace(/\{\{studio:(colours|type|shape|maps)\}\}/g, (_, t) => studioTab(t, studio()))
    .replace(/\{\{studio:checks\}\}/g, () => studioChecks(studio()))
    .replace(/\{\{studio:data\}\}/g, () => studioJson(studio()))
    .replace(/\{\{tokens:([a-z0-9_-]+)\}\}/g, (_, a) => `<p class="tile-tokens" aria-label="Theme tokens this chart reads">${chartTokens(docs[known(a)], std, drawnInks(a)).map((t) => `<code data-tok="${t}">${escapeHtml(t)}</code>`).join("")}</p>`)
    .replace(/\{\{sdk:([^}]+)\}\}/g, (_, n) => sdk.entry(n, where))
    .replace(/\{\{sig:([^}]+)\}\}/g, (_, n) => sdk.sig(n, where));
  // A chart slot names its alias; the page mounts <datars-view src="c/<alias>"> at the document's
  // aspect ratio when the slot scrolls near. Defaults for the chart fill in what the slot omits.
  s = s.replace(/<div class="chart"([^>]*)data-chart="([a-z0-9_-]+)"([^>]*)><\/div>/g, (tag, pre, a, post) => {
    known(a);
    const attrs = pre + post;
    const c = CHARTS[a];
    const add = [`data-src="/c/${a}"`, `data-aspect="${aspects[a].toFixed(4)}"`];
    if (c.mode && !/data-mode=/.test(attrs)) add.push(`data-mode="${c.mode}"`);
    if (c.min && !/data-min=/.test(attrs)) add.push(`data-min="${c.min}"`);
    if (c.phone && !/data-phone-ratio=/.test(attrs)) add.push(`data-phone-ratio="${c.phone}"`);
    if (!/data-label=/.test(attrs)) add.push(`data-label="${escapeAttr(docs[a].title ?? a)}"`);
    // A chart with steps gets its step bar here (the element's own arrows are hidden: they'd appear
    // below the canvas after it mounts) unless the page steps it by scroll or has its own pills.
    const stepped = (docs[a].program?.states?.length ?? 0) > 1;
    const caption = /data-caption/.test(attrs) || (stepped && !/data-steps/.test(attrs) && !s.includes(`id="${a}-pills"`));
    if (caption && !/data-caption/.test(attrs)) add.push("data-caption");
    const all = `${attrs} ${add.join(" ")}`;
    // The slot's space, reserved in CSS before any script runs (the page mustn't move when the
    // chart mounts): its aspect (or `data-ratio`), minimum height and phone ratio as variables.
    const get = (n) => all.match(new RegExp(`data-${n}="([^"]+)"`))?.[1];
    const vars = [`--aspect:${get("ratio") ?? get("aspect")}`, get("min") && `--min:${get("min")}px`, get("phone-ratio") && `--phone:${get("phone-ratio")}`].filter(Boolean).join(";");
    return `<div class="chart"${pre}data-chart="${a}"${post} ${add.join(" ")} style="${vars}"></div>${caption ? captionBar(a) : ""}`;
  });
  // A placeholder left over is a typo — outside code: code shows `{{ … }}` as written (JSX objects).
  const left = s.replace(/<pre[\s\S]*?<\/pre>|<code[\s\S]*?<\/code>/g, "").match(/\{\{[^}]+\}\}/);
  if (left) throw new Error(`${where}: unknown placeholder ${left[0]}`);
  return highlightPage(s);
}

const pages = []; // { file, title, description, body, charts, og, kind }

/** Marketing pages: site/pages/**.html, bodies with front matter. */
function walk(dir, ext, fn, base = dir) {
  for (const f of readdirSync(dir).sort()) {
    const p = join(dir, f);
    if (statSync(p).isDirectory()) walk(p, ext, fn, base);
    else if (f.endsWith(ext)) fn(relative(base, p), readFileSync(p, "utf8"));
  }
}
walk(join(siteDir, "pages"), ".html", (rel, src) => {
  const [fm, body] = frontMatter(src);
  const file = rel === "index.html" || rel.endsWith("/index.html") || rel === "404.html" ? rel : rel.replace(/\.html$/, "/index.html");
  pages.push({ file, kind: "page", fm, body });
});

/** Docs: site/docs/**.md → /docs/…/ with the sidebar, the page's contents and prev/next. */
/** `{{include:docs/guides/react.md|fallback}}` in a docs page: a guide kept elsewhere in the repo,
 * its own title dropped (the page has one) and its links into the repo pointed at GitHub. Without
 * the file, the fallback text (none given: the build fails, rather than publish an empty page). */
function includeDoc(md) {
  return md.replace(/\{\{include:([^|}]+)(?:\|([^}]*))?\}\}/g, (_, path, fallback) => {
    const f = join(root, path.trim());
    if (!existsSync(f)) {
      if (fallback === undefined) throw new Error(`include: no file ${path.trim()}`);
      return fallback;
    }
    const base = dirname(path.trim());
    return readFileSync(f, "utf8")
      .replace(/^---\n[\s\S]*?\n---\n/, "")
      .replace(/^# .*\n/, "")
      .replace(/\]\((?!https?:|\/|#)([^)]+)\)/g, (m, rel) => `](${blob(join(base, rel))})`);
  });
}

walk(join(siteDir, "docs"), ".md", (rel, raw) => {
  const [fm, body] = frontMatter(raw);
  const md = includeDoc(body);
  const slug = rel.replace(/\.md$/, "").replace(/(^|\/)index$/, "");
  const file = `docs/${slug ? slug + "/" : ""}index.html`;
  pages.push({ file, kind: "doc", fm, md, slug });
});

// The chart reference, generated.
for (const p of stdPages({ root, all: std, blob, siteCharts, figure, thumb, charts: CHARTS })) pages.push({ file: p.file, kind: "std", fm: { title: p.title, description: p.description }, std: p, slug: p.slug });

/** Docs chrome around an article. */
function docsShell({ slug, title, lede, content, headings, crumbs }) {
  const side = docsSidebar(slug);
  const bc = [["/docs/", "Docs"], ...(crumbs ?? [])];
  return `<div class="wrap docs-grid">
${side.desktop}
<article class="docs-body">
${side.phone}
<nav class="crumbs" aria-label="Breadcrumb">${bc.map(([u, t]) => `<a href="${u}">${escapeHtml(t)}</a>`).join('<span aria-hidden="true">/</span>')}</nav>
<h1>${title}</h1>
${lede ? `<p class="docs-lede">${lede}</p>` : ""}
<div class="prose">
${content}
</div>
${slug.startsWith("std/") && slug !== "std/" ? "" : docsPrevNext(slug)}
</article>
${docsToc(headings)}
</div>`;
}

const written = [];
for (const p of pages) {
  const where = p.kind === "doc" ? `site/docs/${p.slug || "index"}.md` : p.kind === "std" ? p.file : `site/pages/${p.file}`;
  let body, title, description = p.fm.description ?? "", charts;
  if (p.kind === "page") {
    body = fill(p.body, where);
    title = p.fm.title;
  } else if (p.kind === "doc") {
    const { html, headings } = markdown(p.md);
    const section = DOCS_NAV.find(([, items]) => items.some(([s]) => s === p.slug))?.[0];
    const content = fill(html, where);
    body = docsShell({ slug: p.slug, title: escapeHtml(p.fm.h1 ?? p.fm.title), lede: p.fm.lede ? fill(markdown(p.fm.lede).html.replace(/^<p>|<\/p>$/g, ""), where) : "", content, headings, crumbs: p.slug ? [...(p.slug.startsWith("sdk/") ? [["/docs/sdk/", "SDK reference"]] : []), [`/docs/${p.slug}/`, p.fm.title]] : [] });
    title = p.slug ? p.fm.titleTag ?? `${p.fm.title} · datars docs` : "datars documentation";
    if (!p.slug) title = p.fm.titleTag ?? "Documentation · datars";
    void section;
  } else {
    const s = p.std;
    const content = fill(s.body, where);
    body = docsShell({ slug: s.slug, title: s.slug === "std/" ? s.title : `<code>${escapeHtml(s.title)}</code>`, lede: s.lede, content, headings: s.headings, crumbs: s.slug === "std/" ? [["/docs/std/", "Chart reference"]] : [["/docs/std/", "Chart reference"], [`/docs/${s.slug}/`, s.title]] });
    title = s.titleTag ?? `${s.title} · datars docs`;
  }
  if (!title) throw new Error(`${where}: no title`);
  if (!description) throw new Error(`${where}: no description`);
  charts = /data-chart="/.test(body);
  const og = p.fm.og ? card(p.fm.og) : "card.png";
  const url = urlOf(p.file);
  const crumbs = url.split("/").filter(Boolean);
  const jsonld = url === "/"
    ? { "@context": "https://schema.org", "@type": "SoftwareSourceCode", name: "datars", description, codeRepository: repo, programmingLanguage: ["Rust", "TypeScript"], url: SITE + "/" }
    : { "@context": "https://schema.org", "@type": "BreadcrumbList", itemListElement: [{ "@type": "ListItem", position: 1, name: "datars", item: SITE + "/" }, ...crumbs.map((c, i) => ({ "@type": "ListItem", position: i + 2, name: i === crumbs.length - 1 ? (p.fm.title ?? c) : c.replace(/-/g, " "), item: `${SITE}/${crumbs.slice(0, i + 1).join("/")}/` }))] };
  // `css: features-maps.css` in a page's front matter: its own stylesheet (site/<name>), in the head
  // with site.css, so nothing restyles after the first paint.
  const css = String(p.fm.css ?? "").split(/\s+/).filter(Boolean);
  for (const f of css) if (!existsSync(join(siteDir, f))) throw new Error(`${where}: no stylesheet site/${f}`);
  const html = page({ file: p.file, title, description, body, site: SITE, repo, runtime, ogImage: og, charts, jsonld: p.file === "404.html" ? null : jsonld, noindex: p.file === "404.html", css });
  mkdirSync(dirname(join(out, p.file)), { recursive: true });
  writeFileSync(join(out, p.file), html);
  if (p.file !== "404.html") written.push(url);
}

// The old address of the rationale page.
writeFileSync(join(out, "why.html"), `<!doctype html><meta charset="utf-8"><title>Why datars</title><link rel="canonical" href="${SITE}/why/"><meta http-equiv="refresh" content="0; url=why/"><p><a href="why/">Why datars</a></p>\n`);
writeFileSync(join(out, ".nojekyll"), ""); // serve every file as is (no Jekyll processing)

// ---- the example articles ----------------------------------------------------------------------

if (fast && existsSync(keptArticles)) {
  renameSync(keptArticles, join(out, "articles"));
  console.log("articles: kept from the previous build (--fast)");
} else if (fast) {
  console.log("articles: skipped (--fast, none to keep)");
} else {
  execFileSync("node", [join(root, "scripts/build-articles.mjs"), "--out", out], { stdio: "inherit" });
}
// Articles get their canonical address; they join the sitemap.
if (existsSync(join(out, "articles"))) {
  for (const f of ["index.html", ...readdirSync(join(out, "articles")).filter((d) => existsSync(join(out, "articles", d, "index.html"))).map((d) => `${d}/index.html`)]) {
    const p = join(out, "articles", f);
    let html = readFileSync(p, "utf8");
    const url = `/articles/${f.replace(/index\.html$/, "")}`;
    if (!html.includes('rel="canonical"')) html = html.replace("</title>", `</title>\n<link rel="canonical" href="${SITE}${url}">`);
    writeFileSync(p, html);
    written.push(url);
  }
}
rmSync(stage, { recursive: true, force: true });

// ---- sitemap, robots ---------------------------------------------------------------------------

const today = new Date().toISOString().slice(0, 10);
writeFileSync(join(out, "sitemap.xml"), `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${[...new Set(written)].sort().map((u) => `  <url><loc>${escapeHtml(SITE + u)}</loc><lastmod>${today}</lastmod></url>`).join("\n")}
</urlset>
`);
writeFileSync(join(out, "robots.txt"), `User-agent: *\nAllow: /\n\nSitemap: ${SITE}/sitemap.xml\n`);

const total = readdirSync(join(out, "chunks")).length;
console.log(`\n${out}: ${written.length} pages, ${showcase.length} charts + ${figureCount} figures, ${total} chunks, runtime ${(runtimeGz / 1048576).toFixed(2)} MB gzipped, ${((Date.now() - t0) / 1000).toFixed(0)} s`);
console.log(`preview: datars serve ${relative(process.cwd(), out) || "."} --port 8960`);
