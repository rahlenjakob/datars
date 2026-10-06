// The SDK emits IR the engine reads. These pin the shapes that have to match Rust serde exactly.
import { test } from "node:test";
import assert from "node:assert/strict";
import { motion, choreo, route, instances, view, group, brush, brushed, scrolly, autoplay, step, e, data } from "../dist/index.js";

test("motion rules use datars-motion's serde shapes", () => {
  const m = motion({ select: { role: "datum", key: "root/chart" }, matcher: "by-key", choreo: choreo.stagger("value", 0.3), route: route.arc(), enter: { opacity: 0 } });
  const r = m.rules[0];
  assert.deepEqual(r.select, { role: "datum", key_prefix: [["root"], ["chart"]] }, "key paths are arrays of keys");
  assert.equal(r.matcher, "by-key");
  assert.deepEqual(r.choreo, { type: "stagger", order: "value", spread: 0.3 }, "choreographies are tagged by type");
  assert.deepEqual(r.route, { type: "arc", height: 0.45 });
});

test("instances don't collide with the node's own fields", () => {
  const t = instances({ key: "points", from: "t", x: e("d.x"), y: e("d.y"), r: 3, instanceKey: e("d.id"), opacity: 0.5, size: { w: "fill" } });
  assert.equal(t.key, "points", "the node key stays the node key");
  assert.deepEqual(t.instance_key, { expr: "d.id" });
  assert.equal(t.r, 3, "radius is r, not size (size is layout)");
  assert.equal(t.instance_opacity, 0.5, "per-instance opacity is not the node's opacity");
  assert.deepEqual(t.size, { w: "fill" });
  assert.equal(t.opacity, undefined);
});

test("views clip by default and only say so when they don't", () => {
  assert.equal(view({ children: [] }).clip, undefined);
  assert.equal(view({ clip: false, children: [] }).clip, false);
  assert.deepEqual(view({ camera: { fit: { keys: ["a"] }, explore: "cam" } }).camera, { fit: { keys: ["a"] }, explore: "cam" });
});

test("brush actions and the filter expression", () => {
  assert.deepEqual(group({ on: { brush: brush("sel") } }).on, { brush: { brush: "sel", axis: "x" } });
  assert.deepEqual(brushed("sel", "d.year"), { expr: "!sel.active || (d.year >= sel.lo && d.year <= sel.hi)" });
});

test("program presets declare their drivers", () => {
  assert.deepEqual(scrolly({ steps: [step("a")] }).drivers, [{ scroll: "scrub" }, "keys"]);
  const p = autoplay({ steps: [step("a", { hold: 2 }), step("b")], loop: true });
  assert.ok(p.drivers.includes("autoplay"));
  assert.deepEqual(p.edges, [{ from: "b", on: "next", to: "a" }], "a loop wraps");
  assert.equal(p.states[0].hold, 2);
});

test("tiles sources take a URL, or ask the build for an automatic basemap", () => {
  assert.deepEqual(data.tiles("tiles/x.pmtiles"), { tiles: "tiles/x.pmtiles" });
  assert.deepEqual(data.tiles.auto(), { tiles: "auto" }, "the IR's AUTO_TILES");
});

test("a GeoJSON file by URL names its id property", () => {
  assert.deepEqual(data.url("ends.geojson", { id: "id" }), { url: "ends.geojson", id: "id" }, "the IR's Source.id");
  assert.deepEqual(data.url("count.json", { key: "party" }), { url: "count.json", key: ["party"] }, "no id unless asked");
});
