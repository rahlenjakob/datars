#!/usr/bin/env node
// Vendors the default icon set (Lucide, ISC) into assets/icons/lucide/: one packed file of every
// icon's elements, plus the set's licence. The build step reads icons from it and ships only the
// ones a chart draws (docs/12-delivery.md §Icons), so the set's size never reaches a reader.
//
//   npm pack lucide-static@<version> && tar xzf lucide-static-<version>.tgz
//   node scripts/vendor-icons.mjs package/
//
// Every Lucide icon shares one root (`viewBox="0 0 24 24"`, stroked in `currentColor`, 2 units
// wide, round caps and joins); the file keeps that root once and each icon's elements as written.
// An icon whose root differs is refused rather than silently redrawn.
import { readFileSync, writeFileSync, readdirSync, mkdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const src = process.argv[2];
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
