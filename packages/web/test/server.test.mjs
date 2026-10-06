// The runtime module loads on a server (a page that uses charts, rendered there) and does
// nothing; bundlers can point it at its files (setRuntimeBase) and must leave its engine import
// alone (no `new URL("./literal", import.meta.url)` for them to rewrite, webpackIgnore).
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const dist = join(dirname(fileURLToPath(import.meta.url)), "../dist");

test("importing the runtime on a server defines nothing and throws nothing", async () => {
  assert.equal(typeof globalThis.HTMLElement, "undefined");
  const m = await import(join(dist, "index.js"));
  assert.equal(typeof m.setRuntimeBase, "function");
  assert.equal(typeof m.DatarsView, "function");
});

test("the bundled runtime keeps bundlers off its engine import and its file URLs", () => {
  for (const f of ["datars.js", "index.js"]) {
    const src = readFileSync(join(dist, f), "utf8");
    assert.match(src, /webpackIgnore: true/, f);
    assert.match(src, /@vite-ignore/, f);
    assert.doesNotMatch(src, /new URL\(\s*["'`]\.\//, `${f}: no new URL("./…", import.meta.url)`);
  }
});
