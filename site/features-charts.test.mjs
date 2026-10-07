// The charts page's helpers and markup, checked.
//
// - The doc.ts writer (`toTs`, which the extensibility page's workbench shows) compiles back to the
//   document it was written from: for every std recipe's reference figure, as its first state.
// - The CSV reader the "your data" box uses.
// - The page lists every recipe of the standard library, in its family, once.
//
//   node --test site/features-charts.test.mjs      (≈ 15 s: two esbuild runs per recipe)
import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { parseCsv, toTs, typedColumns } from "./features-charts.js";
import { STD_GROUPS } from "../scripts/site/std.mjs";

const root = resolve(new URL("..", import.meta.url).pathname);
const toJson = (file) => JSON.parse(execFileSync("node", [join(root, "scripts/doc-to-json.mjs"), file], { maxBuffer: 1 << 26 }).toString());
/** What `doc()` fills in, so a document and its compiled doc.ts compare as equals. */
const norm = (d) => JSON.parse(JSON.stringify({ datars: 1, locale: "en", ...d }));

/** Does a `when` like `state == "a" || state != "b"` hold in state `name`? `null`: not that simple. */
function holds(when, name) {
  const src = typeof when === "string" ? when : when?.expr;
  if (typeof src !== "string") return null;
  let any = false;
  for (const part of src.split("||")) {
    const m = part.trim().match(/^state\s*(==|!=)\s*"([^"]*)"$/);
    if (!m) return null;
    if ((m[1] === "==") === (m[2] === name)) any = true;
  }
  return any;
}

/** A figure's document as one chart: what its first state shows, without the program — nodes for
 * other states dropped, the first state's signal settings folded into the signals' defaults. */
function reduce(input) {
  const doc = structuredClone(input);
  const first = doc.program?.states?.[0];
  if (!first) return doc;
  let unsure = false;
  const walk = (v) => {
    if (Array.isArray(v)) {
      for (let i = v.length - 1; i >= 0; i--) {
        const n = v[i];
        if (n && typeof n === "object" && n.when) {
          const h = holds(n.when, first.name);
          if (h === false) { v.splice(i, 1); continue; }
          if (h === true) delete n.when;
          else unsure = true;
        }
        walk(n);
      }
    } else if (v && typeof v === "object") {
      for (const x of Object.values(v)) walk(x);
    }
  };
  walk(doc.scene);
  if (unsure) {
    // Keep a program of the first state alone: the engine decides the rest.
    doc.program = { states: [{ name: first.name, ...(first.set && { set: first.set }) }] };
    return doc;
  }
  for (const [name, value] of Object.entries(first.set ?? {})) {
    if (doc.signals?.[name]) doc.signals[name].default = value;
  }
  delete doc.program;
  return doc;
}


test("doc.ts compiles back to its document, for every std recipe's first state", () => {
  const dir = mkdtempSync(join(tmpdir(), "datars-doc-ts-"));
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

test("CSV: tabs, quotes and types", () => {
  const p = parseCsv('name\tvalue\tok\n"Smith, J"\t1,200\ttrue\nLee\t\tfalse\n');
  assert.deepEqual(typedColumns(p), { name: ["Smith, J", "Lee"], value: [1200, null], ok: [true, false] });
});

test("the page lists every recipe in its family, once", () => {
  const html = readFileSync(join(root, "site/pages/features/charts.html"), "utf8");
  const listed = [...html.matchAll(/data-recipe="([A-Za-z0-9]+)"/g)].map((m) => m[1]);
  const all = STD_GROUPS.flatMap(([, , list]) => list);
  assert.deepEqual([...listed].sort(), [...all].sort());
  for (const [family, , list] of STD_GROUPS) {
    const at = html.indexOf(`data-family="${family}"`);
    assert.ok(at >= 0, `${family}: a section`);
    const next = html.indexOf("data-family=", at + 1);
    const section = html.slice(at, next < 0 ? undefined : next);
    for (const n of list) assert.ok(section.includes(`data-recipe="${n}"`), `${n} under ${family}`);
  }
});
