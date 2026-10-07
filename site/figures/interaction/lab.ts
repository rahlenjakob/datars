// The interaction lab (site/pages/features/interaction.html): four ways to touch one chart, every
// one of them a line in the document. Brush the days, click the bikes, zoom and pan the trips, drag
// the time limit — each writes a signal, and the tables and marks that read it follow. The page
// shows those signals as they change (`view.signals`, the `signal` event) and swaps lines in and
// out of this document with `setDocument` (it's published as /play/interaction-lab.json too).
//
// A month of a made-up bike share: 420 trips, deterministic (a seeded generator), not real data.
import { brushed, data, doc, e, group, instances, interactive, op, scrub, shape, geom, signal, text, view } from "@datars/sdk";
import { area, bar, line, plot } from "@datars/std";

// ---- the data -----------------------------------------------------------------------------------
let seed = 20261007;
const rnd = () => {
  seed = (seed * 1664525 + 1013904223) >>> 0;
  return seed / 2 ** 32;
};
const normal = () => Math.sqrt(-2 * Math.log(rnd() + 1e-12)) * Math.cos(2 * Math.PI * rnd());
const KINDS = { classic: { share: 0.55, km: 2.6, kmh: 15 }, ebike: { share: 0.35, km: 4.6, kmh: 20 }, cargo: { share: 0.1, km: 2.2, kmh: 11 } };
const id: string[] = [], day: number[] = [], kind: string[] = [], km: number[] = [], min: number[] = [];
// Weekdays busier than weekends, three wet days quieter (1 October 2026 is a Thursday).
const weight = (d: number) => ([5, 6].includes((d + 2) % 7) ? 0.7 : 1.15) * ([8, 9, 21].includes(d) ? 0.35 : 1);
const days = Array.from({ length: 30 }, (_, i) => i + 1);
const total = days.reduce((s, d) => s + weight(d), 0);
let n = 0;
for (const d of days) {
  const count = Math.round(((420 * weight(d)) / total) * (0.75 + 0.5 * rnd()));
  for (let i = 0; i < count; i++) {
    const r = rnd();
    const k = r < KINDS.classic.share ? "classic" : r < KINDS.classic.share + KINDS.ebike.share ? "ebike" : "cargo";
    const p = KINDS[k as keyof typeof KINDS];
    const dist = Math.min(13.5, Math.max(0.4, p.km * Math.exp(0.5 * normal())));
    const stops = rnd() < 0.18 ? 6 + rnd() * 18 : rnd() * 3;
    const minutes = Math.min(58, (dist / (p.kmh * (1 + 0.15 * normal()))) * 60 + stops);
    id.push(`t${++n}`);
    day.push(d);
    kind.push(k);
    km.push(Math.round(dist * 10) / 10);
    min.push(Math.round(Math.max(2, minutes)));
  }
}

// ---- the chart --------------------------------------------------------------------------------
/** The trips' plane, in the view's own units: 4 per minute up, and across as many per km as fill
 * the view's box (`box` is the view's), so the plane fills it on a phone and on a desktop alike. */
const XK = "clamp(240 * box.w / box.h / 14, 12, 60)";
const X = `d.km * ${XK}`, Y = "(60 - d.min) * 4";
const xAt = (k: number) => e(`${k} * ${XK}`);

export default doc({
  id: "interaction-lab",
  title: "A month of bike-share trips",
  description: "420 made-up trips in October: trips per day, trips by bike, and every trip's distance and time. Brush days, click bikes, zoom and pan the trips, drag the time limit.",
  size: [760, 560],
  data: { trips: data.values({ id, day, kind, km, min }, { key: "id" }) },
  keys: { classic: { name: "Classic", color: "#4269d0" }, ebike: { name: "E-bike", color: "#efb118" }, cargo: { name: "Cargo", color: "#ff725c" } },
  signals: {
    days: signal.range(),          // drag across the days
    kinds: signal.keyset(),        // click a bike
    limit: signal.num(30),         // drag the line
  },
  tables: {
    // The days' panel counts the bikes picked; the bikes' panel counts the days brushed.
    perDay: { from: "trips", ops: [op.filter(e("kinds.isEmpty() || kinds.has(d.kind)")), op.aggregate(["day"], { trips: ["count"] })] },
    perKind: { from: "trips", ops: [op.filter(brushed("days", "d.day")), op.aggregate(["kind"], { trips: ["count"] })] },
    shown: { from: "trips", ops: [op.filter(brushed("days", "d.day")), op.filter(e("kinds.isEmpty() || kinds.has(d.kind)"))] },
    over: { from: "shown", ops: [op.filter(e("d.min > limit"))] },
  },
  scene: group({
    key: "root",
    semantics: { role: "group", label: "A month of bike-share trips" },
    layout: { type: "rows", gap: 14, padding: [14, 18, 12, 12] },
    children: [
      group({ key: "top", size: { h: { expr: "box.w < 520 ? 330 : 190" } }, layout: { type: "columns", gap: 22, wrap: 520 }, children: [
        plot({ data: "perDay", x: "day", y: "trips", xType: "linear", xDomain: [1, 30], format: ".0f", title: "Trips per day · drag to pick days", brush: "days",
          children: [area({ opacity: 0.22 }), line({ width: 1.5 })] }, { key: "days" }),
        plot({ data: "perKind", x: "trips", y: "kind", xType: "linear", yType: "band", color: "kind", format: ".0f", title: "By bike · click to filter",
          children: [bar({ selected: "kinds", labels: true, format: ".0f", label: e("`${key.name(d.kind)}: ${d.trips} trips`") })] }, { key: "kinds", size: { w: "36%" } }),
      ] }),
      text(e("`Every trip · ${table.count('shown')} shown, ${table.count('over')} over ${limit} min`"), [0, 16],
        { key: "summary", style: { size: "$size.title", weight: 700 }, size: { h: 22 } }),
      group({ key: "plane", scales: { limitY: { type: "linear", domain: [60, 0], range: [0, 240] } }, children: [
        view({ key: "trips", camera: { fit: { bbox: [0, 0, xAt(14), 240] }, padding: 10, explore: "zoom", minZoom: 1, maxZoom: 12 }, children: [
          shape(geom.rect({ x: -400, y: -400, w: 1360, h: 1040 }), { key: "ground", fill: "$surface@0" }),
          // Gridlines every 10 minutes and 2 km, hairlines at any zoom.
          ...[0, 10, 20, 30, 40, 50, 60].map((m) => shape(geom.segment({ x1: 0, y1: (60 - m) * 4, x2: xAt(14), y2: (60 - m) * 4 }), { key: `h${m}`, stroke: { paint: "$grid", width: 1, nonScaling: true }, semantics: { role: "grid" } })),
          ...[0, 2, 4, 6, 8, 10, 12, 14].map((k) => shape(geom.segment({ x1: xAt(k), y1: 0, x2: xAt(k), y2: 240 }), { key: `v${k}`, stroke: { paint: "$grid", width: 1, nonScaling: true }, semantics: { role: "grid" } })),
          ...[10, 20, 30, 40, 50].map((m) => text(`${m} min`, [0, (60 - m) * 4], { key: `hl${m}`, pin: true, offset: [4, -4], style: { size: 10, ink: "$muted" } })),
          ...[2, 4, 6, 8, 10, 12].map((k) => text(`${k} km`, [xAt(k), 240], { key: `vl${k}`, pin: true, offset: [3, -4], style: { size: 10, ink: "$muted" } })),
          instances({ key: "dots", from: "shown", x: e(X), y: e(Y), r: 3.4, screenSize: true,
            fill: e("key.color(d.kind)"), opacity: e("d.min > limit ? 0.95 : 0.3"),
            label: e("`${key.name(d.kind)}, ${d.day} October: ${format(d.km, '.1f')} km in ${d.min} min`"),
            semantics: { role: "series", label: "Trips: time against distance" } }),
          // The limit: a line to drag up and down (the band around it is what the pointer finds).
          group({ key: "limit", on: { drag: scrub("limit", { axis: "y", scale: "limitY", step: 1, min: 5, max: 55 }) }, semantics: { role: "control", label: e("`Time limit: ${limit} minutes`") }, children: [
            shape(geom.rect({ x: 0, y: e("(60 - limit) * 4 - 8"), w: xAt(14), h: 16 }), { key: "hit", fill: "$accent@0", pickable: true }),
            shape(geom.segment({ x1: 0, y1: e("(60 - limit) * 4"), x2: xAt(14), y2: e("(60 - limit) * 4") }), { key: "rule", stroke: { paint: "$accent", width: 2, dash: [6, 4], nonScaling: true } }),
            text(e("`${limit} min`"), [xAt(14), e("(60 - limit) * 4")], { key: "tag", pin: true, offset: [-4, -6], style: { size: 11, weight: 600, ink: "$accent", align: "end" } }),
          ] }),
        ] }),
      ] }),
    ],
  }),
  program: interactive(),
});
