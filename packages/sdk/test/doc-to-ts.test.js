// scripts/doc-to-ts.mjs writes a JSON document as TypeScript on this SDK and std. The TypeScript must
// build exactly the document it came from — for the hand-written examples, for the article stories
// (while the importer's doc.json sits next to their doc.ts), and for the idioms the converter folds
// repetition into (a table of scenes, template expressions, per-scene lists, rule-built columns).
import { test } from "node:test";
import assert from "node:assert/strict";
import { build } from "esbuild";
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const { docToTs, normalize, firstDiff } = await import(join(root, "scripts/doc-to-ts.mjs"));
const { embedLocalRecipes } = await import(join(root, "scripts/local-recipes.mjs"));
const tmp = mkdtempSync(join(tmpdir(), "datars-doc-to-ts-"));
process.on("exit", () => rmSync(tmp, { recursive: true, force: true }));

let n = 0;
/** Compile TypeScript at `dir/name.ts` (so relative imports resolve) to its document. */
async function compile(dir, ts) {
  const src = join(dir, `.doc-to-ts-${process.pid}-${n}.ts`);
  const out = join(tmp, `${n++}.mjs`);
  writeFileSync(src, ts);
  try {
    await build({ entryPoints: [src], bundle: true, format: "esm", platform: "node", outfile: out, logLevel: "silent",
      alias: { "@datars/sdk": join(root, "packages/sdk/src/index.ts"), "@datars/std": join(root, "packages/std/src/index.ts") } });
  } finally {
    rmSync(src, { force: true });
  }
  return embedLocalRecipes((await import(pathToFileURL(out).href)).default, dir);
}

function same(a, b, what) {
  const na = normalize(a), nb = normalize(b);
  assert.ok(JSON.stringify(na) === JSON.stringify(nb), `${what}: ${firstDiff(na, nb)}`);
}

test("every example document round-trips through TypeScript", async () => {
  const names = readdirSync(join(root, "examples")).filter((x) => existsSync(join(root, "examples", x, "doc.json")));
  assert.ok(names.length > 10);
  for (const x of names) {
    const dir = join(root, "examples", x);
    const doc = JSON.parse(readFileSync(join(dir, "doc.json"), "utf8"));
    same(doc, await compile(dir, docToTs(doc)), x);
  }
});

test("the article stories' TypeScript builds the documents the importer produced", async () => {
  const articles = join(root, "site/articles");
  let checked = 0;
  for (const a of existsSync(articles) ? readdirSync(articles) : []) {
    for (const s of existsSync(join(articles, a, "index.html")) ? readdirSync(join(articles, a)) : []) {
      const dir = join(articles, a, s);
      if (!existsSync(join(dir, "doc.ts")) || !existsSync(join(dir, "doc.json"))) continue;
      same(JSON.parse(readFileSync(join(dir, "doc.json"), "utf8")), await compile(dir, readFileSync(join(dir, "doc.ts"), "utf8")), `${a}/${s}`);
      checked++;
    }
  }
  if (!checked) return; // no imported reference documents in this checkout
  assert.ok(checked > 50, `${checked} stories`);
});

test("a story's scenes become a table and templates", async () => {
  const scene = (name, title, lon, labels) => [
    { kind: "group", key: "chart", when: { expr: `scene == "${name}"` }, layout: { type: "rows", gap: 8 }, children: [
      { kind: "use", recipe: "@datars/std/title", params: { text: title }, key: "title", size: { h: "auto" } },
      { kind: "use", recipe: "@datars/std/map", key: "body", params: {
        source: "geo:countries", projection: "web-mercator",
        camera: { fit: { bbox: [{ expr: `geo.x(${lon}, 50)` }, { expr: `geo.y(${lon}, 50)` }, { expr: `geo.x(${lon + 20}, 30)` }, { expr: `geo.y(${lon + 20}, 30)` }] }, padding: 0 },
        children: labels.map(([id, text]) => ({ kind: "use", recipe: "@datars/std/annotate", params: {
          x: { expr: `geo.cx("geo:cities", "${id}")` }, y: { expr: `geo.cy("geo:cities", "${id}")` }, text, dot: true, dx: 0, dy: -10, connector: "none" } })),
      } },
    ] },
  ][0];
  const doc = {
    datars: 1, id: "t", size: { width: 800, height: 500 }, locale: "en",
    data: { "geo:countries": { atlas: "countries" }, months: { values: {
      month: Array.from({ length: 48 }, (_, i) => ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"][i % 12]),
      year: Array.from({ length: 48 }, (_, i) => 2020 + Math.floor(i / 12)),
      value: Array.from({ length: 48 }, (_, i) => Math.round(Math.sin(i) * 100) / 10),
    } } },
    signals: { scene: { type: "str", default: "west" } },
    scene: { kind: "group", key: "root", layout: { type: "stack" }, children: [
      scene("west", "The west", -10, [["lisbon", "Lisbon"], ["madrid", "Madrid"]]),
      scene("north", "The north", 5, [["oslo", "Oslo"]]),
      scene("east", "The east", 20, [["kyiv", "Kyiv"], ["minsk", "Minsk"], ["riga", "Riga"]]),
    ] },
    program: { preset: "story", drivers: ["steps", "keys"], states: ["west", "north", "east"].map((s) => ({ name: s, set: { scene: s } })) },
  };
  const ts = docToTs(doc);
  assert.match(ts, /const scenes = \[/, "one table of scenes");
  assert.match(ts, /\.\.\.scenes\.map\(\(s\) => group\(/, "the charts are a template over it");
  assert.match(ts, /e\(`scene == "\$\{s\.scene\}"`\)/, "when-expressions as template literals");
  assert.match(ts, /geoBox\(s\.box\)/, "camera boxes through the geoBox helper");
  assert.match(ts, /\.\.\.s\.notes\.map\(\(n\) => annotate\(/, "each scene's labels as small rows");
  assert.match(ts, /steps: scenes\.map\(\(s\) => step\(s\.scene/, "the steps join the same table");
  assert.match(ts, /\(_, i\) => 2020 \+ Math\.floor\(i \/ 12\)/, "columns that follow a rule are written as the rule");
  same(doc, await compile(tmp, ts), "factored");
});
