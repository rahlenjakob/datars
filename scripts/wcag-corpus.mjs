// The documents the WCAG checks run over (crates/datars-test/tests/wcag.rs): every example, the
// site's charts (site/charts) and every figure on the site (site/figures/<section>/, the chart
// reference's std figures among them), compiled to JSON once into a folder, with a manifest of
// where each came from — the Rust tests can't compile TypeScript themselves. A document compiled
// after its source and the SDK and std last changed is kept.
//   node scripts/wcag-corpus.mjs [out/wcag-corpus]   then   DATARS_WCAG_CORPUS=out/wcag-corpus cargo test …
import { execFile } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { promisify } from "node:util";

const root = resolve(new URL("..", import.meta.url).pathname);
const out = resolve(root, process.argv[2] ?? "out/wcag-corpus");
mkdirSync(out, { recursive: true });
const run = promisify(execFile);

/** [alias, source, the folder its relative files are read from] */
const docs = [];
for (const name of readdirSync(join(root, "examples")).sort()) {
  const f = join("examples", name, "doc.json");
  if (existsSync(join(root, f))) docs.push([`examples-${name}`, f, join("examples", name)]);
}
for (const f of readdirSync(join(root, "site/charts")).sort()) {
  if (/^[a-z0-9-]+\.ts$/.test(f)) docs.push([`charts-${f.replace(/\.ts$/, "")}`, join("site/charts", f), "site/charts"]);
}
for (const section of readdirSync(join(root, "site/figures")).sort()) {
  const dir = join("site/figures", section);
  if (!statSync(join(root, dir)).isDirectory()) continue;
  for (const f of readdirSync(join(root, dir)).sort()) {
    const m = f.match(/^([A-Za-z0-9-]+)\.(ts|json)$/);
    if (m && !f.endsWith(".job.json")) docs.push([`figures-${section}-${m[1].toLowerCase()}`, join(dir, f), dir]);
  }
}

/** The newest change under a folder (the SDK's and std's sources: a document compiles through them). */
const newest = (dir) => readdirSync(dir, { recursive: true }).reduce((t, f) => Math.max(t, statSync(join(dir, f)).mtimeMs), 0);
const libs = Math.max(newest(join(root, "packages/sdk/src")), newest(join(root, "packages/std/src")));
const manifest = [];
const failed = [];
let next = 0;
async function worker() {
  while (next < docs.length) {
    const [alias, src, dir] = docs[next++];
    const json = join(out, `${alias}.json`);
    const fresh = existsSync(json) && statSync(json).mtimeMs >= Math.max(statSync(join(root, src)).mtimeMs, libs);
    if (!src.endsWith(".ts")) {
      manifest.push({ alias, src, dir, doc: src });
      continue;
    }
    if (!fresh) {
      try {
        const { stdout } = await run("node", [join(root, "scripts/doc-to-json.mjs"), join(root, src)], { cwd: root, maxBuffer: 1 << 28 });
        writeFileSync(json, stdout);
      } catch (e) {
        failed.push(`${src}: ${(e.stderr ?? e.message).toString().trim().split("\n")[0]}`);
        continue;
      }
    }
    manifest.push({ alias, src, dir, doc: json.slice(root.length + 1) });
  }
}
await Promise.all(Array.from({ length: 8 }, worker));
manifest.sort((a, b) => a.alias.localeCompare(b.alias));
writeFileSync(join(out, "manifest.json"), JSON.stringify(manifest, null, 1));
console.log(`${out}: ${manifest.length} documents${failed.length ? `, ${failed.length} didn't compile:\n  ${failed.join("\n  ")}` : ""}`);
if (failed.length) process.exitCode = 1;
