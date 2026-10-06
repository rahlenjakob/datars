// On a server <DatarsView> renders its box — at the chart's aspect, so nothing moves when the
// chart arrives — and touches nothing browser-only (no window, no runtime import).
import { test } from "node:test";
import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToString } from "react-dom/server";
import { DatarsView, useDatarsState } from "../dist/index.js";

const doc = { datars: 1, title: "Votes", size: { width: 720, height: 440 }, scene: { kind: "group", children: [] }, program: { states: [{ name: "bars" }, { name: "pie" }] } };

test("no window on the server", () => {
  assert.equal(typeof globalThis.window, "undefined");
  assert.equal(typeof globalThis.HTMLElement, "undefined");
});

test("a chart import renders a box at its aspect", () => {
  const html = renderToString(createElement(DatarsView, { chart: { kind: "datars-chart", id: "a.chart.ts", size: [960, 620], src: "/datars/c/a" } }));
  assert.match(html, /aspect-ratio:960 \/ 620/);
  assert.match(html, /data-datars-view=""/);
  assert.doesNotMatch(html, /datars-view /, "the element itself comes after hydration");
});

test("a bare document works as a chart (its size, the same box)", () => {
  const html = renderToString(createElement(DatarsView, { chart: doc, className: "chart", id: "votes" }));
  assert.match(html, /aspect-ratio:720 \/ 440/);
  assert.match(html, /class="chart"/);
  assert.match(html, /id="votes"/);
});

test("an explicit height or aspect wins; a src without either gets 5:3", () => {
  assert.match(renderToString(createElement(DatarsView, { src: "/c/x", height: 300 })), /height:300px/);
  assert.match(renderToString(createElement(DatarsView, { src: "/c/x", height: "100%" })), /height:100%/);
  assert.match(renderToString(createElement(DatarsView, { src: "/c/x", aspectRatio: 2 })), /aspect-ratio:2/);
  assert.match(renderToString(createElement(DatarsView, { src: "/c/x" })), /aspect-ratio:5 \/ 3/);
});

test("useDatarsState is null on the server", () => {
  function Probe() {
    const s = useDatarsState({ current: null });
    return createElement("p", null, s === null ? "null" : "state");
  }
  assert.match(renderToString(createElement(Probe)), /null/);
});
