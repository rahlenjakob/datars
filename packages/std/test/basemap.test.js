import { test } from "node:test";
import assert from "node:assert/strict";
import { basemap, attribution } from "../dist/index.js";

test("basemap expands to a tiles node styled by map tokens", () => {
  const out = basemap.__expand({ source: "base" }, {});
  assert.equal(out.template.key, "basemap");
  const [sea, tiles] = out.template.children;
  assert.equal(sea.key, "sea");
  assert.equal(sea.fill, "$map.water");
  assert.equal(tiles.kind, "tiles");
  assert.equal(tiles.source, "base");
  const ids = tiles.layers.map((l) => l.id ?? l.layer);
  for (const want of ["land", "parks", "water", "buildings", "roads-minor", "roads-major", "roads-highway", "boundaries", "places"]) assert.ok(ids.includes(want), `${want} in ${ids}`);
  const json = JSON.stringify(tiles);
  for (const token of ["$map.land", "$map.park", "$map.building", "$map.road", "$map.road-major", "$map.border", "$map.label", "$map.label-halo"]) assert.ok(json.includes(token), token);
  const places = tiles.layers.find((l) => l.layer === "places");
  assert.ok(places.labels && places.priority, "place names are screen-space labels by priority");
  assert.ok(json.includes("tile.zoom"), "widths step with the zoom drawn at");
});

test("basemap splits around data layers and renames archive layers", () => {
  const base = basemap.__expand({ source: "base", part: "base" }, {}).template;
  const labels = basemap.__expand({ source: "base", part: "labels", layers: { places: "place" } }, {}).template;
  assert.equal(labels.key, "basemap-labels");
  const tl = labels.children.find((c) => c.kind === "tiles");
  assert.deepEqual(tl.layers.map((l) => l.layer), ["place"]);
  assert.equal(labels.children.length, 1, "no sea above the data");
  const tb = base.children.find((c) => c.kind === "tiles");
  assert.ok(!tb.layers.some((l) => l.labels), "no names below the data");
});

test("attribution sits in the corner of its box, at any size the chart is drawn", () => {
  const out = attribution.__expand({}, { size: [760, 480] });
  assert.equal(out.template.kind, "text");
  // Relative to the laid-out box, not the size it was expanded for: a published chart is expanded
  // at its document's size and drawn at the page's.
  assert.deepEqual(out.template.at, [{ expr: "box.w - 6" }, { expr: "box.h - 5" }]);
  assert.ok(out.template.text.includes("OpenStreetMap"));
});
