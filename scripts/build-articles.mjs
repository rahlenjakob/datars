// Build the showcase's article pages (site/articles/ → <out>/articles/). Each article is a
// hand-written HTML page (site/articles/<article>/index.html) whose charts are slots naming a story:
//
//   <figure class="figure" data-story="busiest"><figcaption>…</figcaption></figure>
//   <section class="scrolly text-left" data-story="routes"><div class="scrolly-steps">
//     <div class="step"><div>…</div></div> …</div></section>
//
// Every story is a TypeScript document (<article>/<story>/doc.ts, or another article's as
// `../warming/spiral`). The build compiles each one, publishes it into the article's charts/
// folder, fills the slot in (the chart's address and aspect, step buttons for a figure whose story
// has steps, the scroll stage for a scroll story, a link to its source) and renders one chart per
// article as the cover on the articles index (site/articles/index.html, its cards filled in from
// the pages themselves).
//
//   node scripts/build-articles.mjs [--out out/pages] [filter]     (build-site.mjs runs it)
import { build } from "esbuild";
import { execFileSync } from "node:child_process";
import { copyFileSync, cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { embedLocalRecipes } from "./local-recipes.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
const argOut = args.indexOf("--out");
const site = resolve(root, argOut >= 0 ? args[argOut + 1] : "out/pages");
const filter = args.find((a, i) => !a.startsWith("--") && args[i - 1] !== "--out") ?? "";
const src = join(root, "site/articles");
const cli = join(root, "target/release/datars");
const out = join(site, "articles");
const stage = join(root, "out/articles-stage");

// ---- documents --------------------------------------------------------------------------------------

/** Compile a doc.ts to its document (the IR), local and file packages embedded. */
async function compile(file) {
  const tmp = join(stage, "mjs", `${relative(src, file).replace(/[\\/]/g, "_")}.mjs`);
  mkdirSync(dirname(tmp), { recursive: true });
  await build({ entryPoints: [file], bundle: true, format: "esm", platform: "node", outfile: tmp, logLevel: "error",
    alias: { "@datars/sdk": join(root, "packages/sdk/src/index.ts"), "@datars/std": join(root, "packages/std/src/index.ts") } });
  const mod = await import(`${pathToFileURL(tmp).href}?t=${Date.now()}`);
  return embedLocalRecipes(mod.default, dirname(file));
}

/** A story named by an article: `busiest`, or another article's `../warming/spiral`. */
function locate(article, story) {
  const parts = story.split("/").filter((p) => p && p !== "..");
  const [a, s] = parts.length > 1 ? parts.slice(-2) : [article, parts[0]];
  return { dir: join(src, a, s), alias: a === article ? s : `${a}-${s}` };
}

const themes = new Map();
/** The chart theme an article's stories share (its theme.ts), if it has one. */
async function articleTheme(article) {
  if (!themes.has(article)) {
    const file = join(src, article, "theme.ts");
    let def = null;
    if (existsSync(file)) {
      const tmp = join(stage, "mjs", `${article}-theme.mjs`);
      mkdirSync(dirname(tmp), { recursive: true });
      await build({ entryPoints: [file], bundle: true, format: "esm", platform: "node", outfile: tmp, logLevel: "error",
        alias: { "@datars/sdk": join(root, "packages/sdk/src/index.ts") } });
      const mod = await import(pathToFileURL(tmp).href);
      def = Object.values(mod).find((v) => v && typeof v === "object" && typeof v.name === "string" && v.tokens) ?? null;
    }
    themes.set(article, def);
  }
  return themes.get(article);
}

const hasFfmpeg = (() => { try { execFileSync("ffmpeg", ["-version"], { stdio: "ignore" }); return true; } catch { return false; } })();

const published = new Map();
/** Compile, stage (with the story's data files) and publish a story into the article's charts/. */
async function publish(article, story) {
  const key = `${article}:${story}`;
  if (published.has(key)) return published.get(key);
  const { dir, alias } = locate(article, story);
  const file = join(dir, "doc.ts");
  if (!existsSync(file)) throw new Error(`site/articles/${article}: no story \`${story}\` (${relative(root, file)})`);
  const doc = await compile(file);
  // Another article's story takes this article's look, as it did in the original: the chart theme
  // is the article's, not the story's.
  const host = await articleTheme(article);
  if (alias !== story && host) doc.theme = { use: host.name, themes: [host] };
  const at = join(stage, article, alias);
  rmSync(at, { recursive: true, force: true });
  // The story's files (data, tiles, a live source's snapshots) next to the staged document.
  cpSync(dir, at, { recursive: true, filter: (f) => !/\/doc\.(ts|json)$/.test(f) });
  const staged = join(at, "doc.json");
  writeFileSync(staged, JSON.stringify(doc));
  const to = join(out, article, "charts", alias);
  rmSync(to, { recursive: true, force: true });
  execFileSync(cli, ["publish", staged, "--alias", alias, "--to", to], { stdio: ["ignore", "ignore", "inherit"] });
  copyFileSync(file, join(to, "doc.ts"));
  const states = doc.program?.states ?? [];
  const drivers = JSON.stringify(doc.program?.drivers ?? []);
  const info = {
    alias, staged,
    src: `charts/${alias}/c/${alias}`,
    source: `charts/${alias}/doc.ts`,
    aspect: (doc.size?.height ?? 480) / (doc.size?.width ?? 800),
    title: doc.title ?? alias,
    // Step buttons for a story you step through (not one that plays itself or follows the scroll).
    stepped: states.length > 1 && !drivers.includes("autoplay") && !drivers.includes("scroll"),
  };
  published.set(key, info);
  return info;
}

// ---- pages ------------------------------------------------------------------------------------------

const attr = (s) => String(s).replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;");
const slot = (c, extra = "") => `<div class="chart" data-src="${attr(c.src)}" data-aspect="${c.aspect.toFixed(4)}" data-label="${attr(c.title)}" style="--aspect: ${c.aspect.toFixed(4)}"${extra}></div>`;
const sourceLink = (c) => `<a class="source" href="${attr(c.source)}">Chart code</a>`;

/** Fill every story slot of a page in. */
async function fill(article, html, dark) {
  const re = /<(figure|section)\b([^>]*?)\sdata-(story|video)="([^"]+)"([^>]*)>/g;
  let outHtml = "";
  let last = 0;
  let n = 0;
  const stories = [];
  for (let m; (m = re.exec(html)); ) {
    const [tag, el, before, kind, story, after] = m;
    const c = await publish(article, story);
    stories.push({ story, el, attrs: before + after, c });
    outHtml += html.slice(last, m.index) + tag;
    last = m.index + tag.length;
    if (kind === "video" && hasFfmpeg) {
      // The story as a film, exported by the engine (`datars video`): frames and captions.
      const vertical = /\bvertical\b/.test(before + after);
      const file = `video/${c.alias}.mp4`;
      mkdirSync(join(out, article, "video"), { recursive: true });
      execFileSync(cli, ["video", c.staged, "--fps", "30", "--hold", "2.5", "--size", vertical ? "540x960" : "1280x720", "--mode", dark ? "dark" : "light", "--out", join(out, article, file)], { stdio: ["ignore", "ignore", "inherit"] });
      outHtml += `\n  <video controls playsinline muted preload="metadata" src="${file}"><track kind="captions" label="Captions" src="${file.replace(/\.mp4$/, ".vtt")}" default></video>`;
      continue;
    }
    if (el === "section") {
      const id = `scrolly-${n++}`;
      outHtml = outHtml.slice(0, -1) + ` id="${id}">`;
      outHtml += `\n  <div class="scrolly-stage">${slot(c, ` data-steps="#${id} .step"`)}</div>`;
      continue;
    }
    // A figure: the chart first, step buttons under it, the source in its caption.
    outHtml += `\n  ${slot(c)}${c.stepped ? `\n  <div class="stepper" role="group" aria-label="Steps"></div>` : ""}`;
    const end = html.indexOf("</figure>", last);
    const body = html.slice(last, end);
    const cap = body.lastIndexOf("</figcaption>");
    // A caption that ends in its credit line ("Source: …") takes the link at the end of that line.
    const credit = cap >= 0 && /<span class="credit">[^<]*(?:<(?!\/span>)[^<]*)*<\/span>\s*$/.test(body.slice(0, cap)) ? body.lastIndexOf("</span>", cap) : -1;
    if (credit >= 0) {
      outHtml += `${body.slice(0, credit)} · ${sourceLink(c)}${body.slice(credit)}`;
    } else if (cap >= 0) {
      outHtml += `${body.slice(0, cap)} ${sourceLink(c)}${body.slice(cap)}`;
    } else {
      outHtml += `${body}<figcaption>${sourceLink(c)}</figcaption>`;
    }
    last = end;
  }
  return { html: outHtml + html.slice(last), stories };
}

/** What the index card shows, read off the page: its look (custom properties, else the house
 * style of articles.css), kicker, title, dek. */
function meta(html) {
  const v = (name, d) => html.match(new RegExp(`--${name}:\\s*([^;]+);`))?.[1]?.trim() ?? d;
  const text = (re) => (html.match(re)?.[1] ?? "").replace(/<[^>]+>/g, "").trim();
  return {
    page: v("page", "#ffffff"), ink: v("ink", "#121212"), accent: v("accent", "#c4431d"), paper: v("paper", v("page", "#ffffff")), title: v("font-title", "Georgia, 'Times New Roman', serif"),
    dark: /color-scheme:\s*dark/.test(html),
    kicker: text(/<p class="kicker">([\s\S]*?)<\/p>/), h1: text(/<h1>([\s\S]*?)<\/h1>/), dek: text(/<p class="dek">([\s\S]*?)<\/p>/),
  };
}

if (!existsSync(cli)) throw new Error("build the CLI first: cargo build --release -p datars-cli");
mkdirSync(out, { recursive: true });
const articles = readdirSync(src).filter((a) => existsSync(join(src, a, "index.html")) && a.includes(filter)).sort();
const cards = [];
for (const a of articles) {
  const t0 = Date.now();
  const dir = join(out, a);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  const source = readFileSync(join(src, a, "index.html"), "utf8");
  const { html, stories } = await fill(a, source, /color-scheme:\s*dark/.test(source));
  writeFileSync(join(dir, "index.html"), html);
  const m = meta(html);
  // The cover: the story the page marks `data-cover`, else its first figure; a still of one state.
  const cover = stories.find((s) => /\bdata-cover\b/.test(s.attrs)) ?? stories.find((s) => s.el === "figure") ?? stories[0];
  if (cover) {
    const state = cover.attrs.match(/data-cover="(\d+)"/)?.[1] ?? "0";
    execFileSync(cli, ["render", cover.c.staged, "--state", state, "--size", "960x600", "--dpr", "1", "--mode", m.dark ? "dark" : "light", "--out", join(dir, "cover.png")], { stdio: "ignore" });
  }
  cards.push({ a, ...m, cover: !!cover });
  console.log(`article ${a.padEnd(24)} ${String(stories.length).padStart(2)} charts  ${((Date.now() - t0) / 1000).toFixed(1)} s`);
}
for (const f of ["articles.css", "articles.js"]) copyFileSync(join(src, f), join(out, f));

// The index: its cards, each in its article's own colours and type.
const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;");
const cardHtml = cards.map((c) => `  <a class="card-link" href="${c.a}/" style="--c-page: ${c.page}; --c-ink: ${c.ink}; --c-accent: ${c.accent}; --c-paper: ${c.paper}; --c-title: ${attr(c.title)}">
    ${c.cover ? `<img src="${c.a}/cover.png" alt="" loading="lazy" width="960" height="600">` : ""}
    <div class="text"><p class="k">${esc(c.kicker)}</p><h2>${esc(c.h1)}</h2><p>${esc(c.dek)}</p></div>
  </a>`).join("\n");
// (A filtered build rebuilds only its pages: the index keeps every card.)
if (!filter) writeFileSync(join(out, "index.html"), readFileSync(join(src, "index.html"), "utf8").replace("<!-- cards -->", cardHtml));
rmSync(stage, { recursive: true, force: true });
console.log(`${relative(root, out)}: ${articles.length} articles`);
