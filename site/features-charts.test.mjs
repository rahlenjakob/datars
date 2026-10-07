// The charts page's claim, checked: the doc.ts its recipe playground writes compiles back to the
// document the chart is playing — for every std recipe's reference figure, reduced to its first
// state as the playground plays it — and the CSV it shows reads back as the same columns.
//
//   node --test site/features-charts.test.mjs      (≈ 15 s: two esbuild runs per recipe)
import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { holds, parseCsv, reduce, toCsv, toTs, typedColumns } from "./features-charts.js";

const root = resolve(new URL("..", import.meta.url).pathname);
const toJson = (file) => JSON.parse(execFileSync("node", [join(root, "scripts/doc-to-json.mjs"), file], { maxBuffer: 1 << 26 }).toString());
/** What `doc()` fills in, so a reduced document and its compiled doc.ts compare as equals. */
const norm = (d) => JSON.parse(JSON.stringify({ datars: 1, locale: "en", ...d }));

test("when: the simple state tests the figures use", () => {
  assert.equal(holds({ expr: 'state == "a"' }, "a"), true);
  assert.equal(holds({ expr: 'state != "a"' }, "a"), false);
  assert.equal(holds({ expr: 'state == "b" || state == "a"' }, "a"), true);
  assert.equal(holds({ expr: "step > 2" }, "a"), null);
});

test("CSV: tabs, quotes and types", () => {
  const p = parseCsv('name\tvalue\tok\n"Smith, J"\t1,200\ttrue\nLee\t\tfalse\n');
  assert.deepEqual(typedColumns(p), { name: ["Smith, J", "Lee"], value: [1200, null], ok: [true, false] });
  const cols = { a: ["x, y", "z"], b: [1, 2] };
  assert.deepEqual(typedColumns(parseCsv(toCsv(cols))), cols);
});

test("doc.ts compiles back to the playground's document, for every std recipe", () => {
  const dir = mkdtempSync(join(tmpdir(), "datars-playground-"));
  try {
    const figures = readdirSync(join(root, "site/figures/std")).filter((f) => f.endsWith(".ts"));
    assert.ok(figures.length >= 80, "the std figures are there");
    for (const f of figures) {
      const reduced = reduce(toJson(join(root, "site/figures/std", f)));
      assert.equal(reduced.program, undefined, `${f}: reduced to one state`);
      const ts = join(dir, f);
      writeFileSync(ts, toTs(reduced));
      assert.deepEqual(norm(toJson(ts)), norm(reduced), `${f}: its doc.ts compiles to another document`);
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
