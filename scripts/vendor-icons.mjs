#!/usr/bin/env node
// Vendors the default icon set (Lucide, ISC) into assets/icons/lucide/: one packed file of every
// icon's elements, plus the set's licence. The build step reads icons from it and ships only the
// ones a chart draws (docs/12-delivery.md §Icons), so the set's size never reaches a reader.
//
//   npm pack lucide-static@<version> && tar xzf lucide-static-<version>.tgz
//   node scripts/vendor-icons.mjs package/ [lucide-<version>/]
//
// With the Lucide repository at the same tag (its `icons/*.json` and `categories/`, from
// github.com/lucide-icons/lucide), it also writes `index.json`: each icon's categories, the
// categories' titles, and the icons the licence says are derived from Feather (MIT) — for the
// website's icon library only. Nothing in a runtime or a bundle reads it.
//
// Every Lucide icon shares one root (`viewBox="0 0 24 24"`, stroked in `currentColor`, 2 units
// wide, round caps and joins); the file keeps that root once and each icon's elements as written.
// An icon whose root differs is refused rather than silently redrawn.
import { readFileSync, writeFileSync, readdirSync, mkdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const src = process.argv[2];
const repo = process.argv[3];
if (!src) {
  console.error("usage: node scripts/vendor-icons.mjs <extracted lucide-static package dir>");
  process.exit(2);
}
const pkg = JSON.parse(readFileSync(join(src, "package.json"), "utf8"));
const ROOT = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">';
const want = { xmlns: "http://www.w3.org/2000/svg", viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", "stroke-width": "2", "stroke-linecap": "round", "stroke-linejoin": "round" };
const icons = {};
const files = readdirSync(join(src, "icons")).filter((f) => f.endsWith(".svg")).sort();
for (const f of files) {
  const name = f.slice(0, -4);
  const text = readFileSync(join(src, "icons", f), "utf8").replace(/<!--[\s\S]*?-->/g, "");
  const m = text.match(/<svg([^>]*)>([\s\S]*)<\/svg>/);
  if (!m) throw new Error(`${f}: no <svg> root`);
  const attrs = Object.fromEntries([...m[1].matchAll(/([\w:-]+)="([^"]*)"/g)].map((a) => [a[1], a[2]]));
  for (const [k, v] of Object.entries(want)) {
    if (attrs[k] !== v) throw new Error(`${f}: root ${k}="${attrs[k]}" (expected "${v}")`);
  }
  // Elements one after another, whitespace between tags dropped.
  icons[name] = m[2].replace(/>\s+</g, "><").replace(/\s+\/>/g, "/>").trim();
}
let tags = {};
try { tags = JSON.parse(readFileSync(join(src, "tags.json"), "utf8")); } catch { /* optional */ }
const here = dirname(fileURLToPath(import.meta.url));
const out = join(here, "..", "assets", "icons", "lucide");
mkdirSync(out, { recursive: true });
const packed = { set: "lucide", version: pkg.version, licence: "ISC", source: `https://lucide.dev (lucide-static ${pkg.version})`, root: ROOT, icons, tags: Object.fromEntries(Object.keys(icons).filter((n) => tags[n]).map((n) => [n, tags[n]])) };
writeFileSync(join(out, "icons.json"), JSON.stringify(packed) + "\n");
writeFileSync(join(out, "LICENSE"), readFileSync(join(src, "LICENSE"), "utf8"));
console.log(`lucide ${pkg.version}: ${Object.keys(icons).length} icons → assets/icons/lucide/icons.json`);

if (repo) {
  const cats = Object.fromEntries(readdirSync(join(repo, "categories")).filter((f) => f.endsWith(".json")).sort().map((f) => {
    const c = JSON.parse(readFileSync(join(repo, "categories", f), "utf8"));
    return [f.slice(0, -5), { title: c.title, icon: c.icon }];
  }));
  // An icon's metadata is under its own name; an alias (an older name) points to it.
  const meta = {};
  for (const f of readdirSync(join(repo, "icons")).filter((f) => f.endsWith(".json"))) {
    const m = JSON.parse(readFileSync(join(repo, "icons", f), "utf8"));
    const name = f.slice(0, -5);
    meta[name] = m.categories ?? [];
    for (const a of m.aliases ?? []) meta[typeof a === "string" ? a : a.name] = m.categories ?? [];
  }
  const licence = readFileSync(join(repo, "LICENSE"), "utf8");
  const feather = (licence.match(/derived from the Feather project:\s*\n\s*\n([^\n]+)/)?.[1] ?? "").split(",").map((n) => n.trim()).filter(Boolean);
  const index = { set: "lucide", version: pkg.version, categories: cats, icons: Object.fromEntries(Object.keys(icons).map((n) => [n, meta[n] ?? []])), feather };
  writeFileSync(join(out, "index.json"), JSON.stringify(index) + "\n");
  console.log(`lucide ${pkg.version}: ${Object.keys(cats).length} categories, ${feather.length} icons from Feather → assets/icons/lucide/index.json`);
}

