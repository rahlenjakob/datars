// The icon library (/docs/icons/library/): the built-in set, browsable. Written by the site build
// from the vendored set (assets/icons/lucide/icons.json: every icon's elements and tags) and its
// site-only index (index.json: categories, the icons derived from Feather):
//
// - `index.json` beside the page: every icon's name, tags, categories and which sprite chunk holds
//   it — what search and the category counts read (a few tens of KB gzipped, fetched after first
//   paint);
// - `sprite-<n>.json`: the icons' elements, CHUNK at a time in name order, fetched as the grid
//   scrolls to them or a search shows them — never the whole set;
// - the page's first cells, written into its HTML, so the grid paints with them and nothing moves
//   when the script arrives.

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { escapeAttr, escapeHtml } from "./markdown.mjs";

export const CHUNK = 120;
/** Icons drawn into the page itself (the first screens' worth). */
export const FIRST = 96;

/** The theme's icon tokens (crates/datars-theme/themes/neutral.json): a name → its `$icon.x`. */
function iconTokens(root) {
  const t = JSON.parse(readFileSync(join(root, "crates/datars-theme/themes/neutral.json"), "utf8")).tokens ?? {};
  return Object.fromEntries(Object.entries(t).filter(([k, v]) => k.startsWith("icon.") && typeof v === "string").map(([k, v]) => [v.replace(/^lucide:/, ""), `$${k}`]));
}

/** Write the library's data into `dir` and return the page's placeholders' values. */
export function iconLibrary(root, dir) {
  const set = JSON.parse(readFileSync(join(root, "assets/icons/lucide/icons.json"), "utf8"));
  const index = JSON.parse(readFileSync(join(root, "assets/icons/lucide/index.json"), "utf8"));
  const names = Object.keys(set.icons).sort();
  const catIds = Object.keys(index.categories).sort((a, b) => index.categories[a].title.localeCompare(index.categories[b].title));
  const catAt = Object.fromEntries(catIds.map((c, i) => [c, i]));
  const feather = new Set(index.feather);
  const tokens = iconTokens(root);
  const counts = new Array(catIds.length).fill(0);
  const rows = names.map((n, i) => {
    const cats = (index.icons[n] ?? []).filter((c) => c in catAt).map((c) => catAt[c]);
    for (const c of cats) counts[c]++;
    const parts = n.split("-");
    const row = [n, (set.tags[n] ?? []).filter((t) => !parts.includes(t)).join(" "), cats, Math.floor(i / CHUNK)];
    if (feather.has(n)) row.push(1);
    return row;
  });
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, "index.json"), JSON.stringify({
    set: set.set, version: set.version, licence: set.licence, chunk: CHUNK, tokens,
    categories: catIds.map((c, i) => [c, index.categories[c].title, counts[i]]),
    icons: rows,
  }));
  for (let k = 0; k * CHUNK < names.length; k++) {
    writeFileSync(join(dir, `sprite-${k}.json`), JSON.stringify(Object.fromEntries(names.slice(k * CHUNK, (k + 1) * CHUNK).map((n) => [n, set.icons[n]]))));
  }
  const cell = (n) => `<li role="option" id="il-${escapeAttr(n)}" data-name="${escapeAttr(n)}" aria-selected="false" tabindex="-1"><svg viewBox="0 0 24 24" aria-hidden="true">${set.icons[n]}</svg><span>${escapeHtml(n)}</span></li>`;
  return {
    count: names.length.toLocaleString("en"),
    version: set.version,
    first: names.slice(0, FIRST).map(cell).join(""),
    categories: [`<button type="button" data-cat="" aria-pressed="true">All <span>${names.length.toLocaleString("en")}</span></button>`, ...catIds.map((c, i) => `<button type="button" data-cat="${i}" aria-pressed="false">${escapeHtml(index.categories[c].title)} <span>${counts[i]}</span></button>`)].join(""),
  };
}
