// The custom recipe behind examples/worlds: every country four ways. Positions are expressions over
// each country's row, so the engine lays them out for any size and every value is exact everywhere.
import { e, group, op, recipe, shape, geom, repeat, text, t, type Prop } from "@datars/sdk";

export const CONTINENTS = ["Oceania", "South America", "North America", "Europe", "Asia", "Africa"];

export type Layout = "globe" | "orbits" | "spiral" | "blocks";
/** The orbits' plane, seen at an angle: ellipses 0.8 as tall as wide. */
const TILT = 0.8;
interface WorldsParams { data: string; source: string; layout: Layout; turn: Prop }

export const worlds = recipe<WorldsParams>({
  id: "@local/worlds/worlds",
  doc: "Every country four ways: on a globe, in orbits by continent, in a golden-angle spiral by population, and packed as blocks.",
  params: { data: t.table(), source: t.table(), layout: t.oneOf(["globe", "orbits", "spiral", "blocks"] as const, "globe"),
    turn: t.prop("The globe's central longitude in degrees (default 15); a clock signal turns it.") },
  expand(p, cx) {
    // Each planet's share of its ring is its size (√population): spaced by size, neighbours can't
    // overlap, and ordered by id, the giants land scattered around the ring.
    const sized = cx.table("sized", p.data, op.derive("sz", e("sqrt(d.pop)")));
    const counts = cx.table("counts", sized, op.aggregate(["continent"], { n: ["count"], ring_sz: ["sum", "sz"] }));
    const rows = cx.table("rows", sized,
      op.window("cumsum", "sz", "cum_sz", { partition: ["continent"], order: "id" }),
      op.window("rank", "pop", "rank_all", { order: "-pop" }),
      op.join(counts, "continent"),
      op.join(p.source, "id"));
    const R = "min(box.w, box.h - 40)";
    const cxy = ["box.w / 2", "(box.h + 30) / 2"];
    const size = `max(1.6, sqrt(d.pop) * ${R} / 820)`;
    const ring = `(${CONTINENTS.map((c, i) => `d.continent == "${c}" ? ${i} : `).join("")}5)`;
    const fill = e("key.color(d.continent)");
    const label = e("`${d.name}: ${format(d.pop, ',.1~f')} million`");
    // Hover (and the accessible name) on every mark, in every layout.
    const datum = { role: "datum", label } as const;
    let mark: ReturnType<typeof shape>;
    if (p.layout === "globe") {
      mark = shape(geom.feature(p.source, e("d.id")), { fill, stroke: { paint: "$paper", width: 0.4, nonScaling: true } as never, semantics: datum, pickable: true });
    } else if (p.layout === "blocks") {
      const tm = cx.table("blocks", rows, op.treemap({ value: "pop", width: e("box.w"), height: e("box.h - 40"), as: ["x0", "y0", "x1", "y1"] }));
      return group({ key: "worlds", children: [repeat(tm, shape(geom.rect({ x: e("d.x0 + 0.5"), y: e("d.y0 + 40.5"), w: e("max(0, d.x1 - d.x0 - 1)"), h: e("max(0, d.y1 - d.y0 - 1)") }), { key: e("d.id"), fill, semantics: datum, pickable: true })),
        repeat(tm, text(e("d.name"), [e("d.x0 + 5"), e("d.y0 + 54")], { key: e("'name-' + d.id"), when: e("d.x1 - d.x0 > 60 && d.y1 - d.y0 > 22"), style: { size: 11, weight: 600, ink: e("'on(' + key.color(d.continent) + ')'"), maxWidth: e("d.x1 - d.x0 - 8") } }))] });
    } else {
      const at = p.layout === "orbits"
        // Rings by continent, the smallest inside; around each ring by the golden angle, so the
        // largest planets of a continent spread out instead of queuing.
        ? [`${cxy[0]} + ${R} * (0.12 + ${ring} * 0.062) * cos(6.28319 * (d.cum_sz - d.sz / 2) / d.ring_sz + ${ring} * 0.9)`, `${cxy[1]} + ${R} * ${TILT} * (0.12 + ${ring} * 0.062) * sin(6.28319 * (d.cum_sz - d.sz / 2) / d.ring_sz + ${ring} * 0.9)`]
        // Phyllotaxis, smallest at the centre: the i-th smallest at radius ∝ √i, turned by the golden
        // angle — the giants land on the rim, where there's room.
        : [`${cxy[0]} + ${R} * 0.44 * sqrt((table.count("${rows}") - d.rank_all + 0.5) / table.count("${rows}")) * cos((table.count("${rows}") - d.rank_all) * 2.39996)`, `${cxy[1]} + ${R} * 0.44 * sqrt((table.count("${rows}") - d.rank_all + 0.5) / table.count("${rows}")) * sin((table.count("${rows}") - d.rank_all) * 2.39996)`];
      mark = shape(geom.circle({ cx: e(at[0]), cy: e(at[1]), r: e(size) }), { fill, semantics: datum, pickable: true });
      const names = repeat(rows, text(e("d.name"), [e(at[0]), e(`${at[1]} - ${size} - 4`)], { key: e("'name-' + d.id"), when: e("d.pop >= 150"), style: { size: 11, weight: 600, ink: "$ink", align: "middle" }, halo: ["$paper", 3] }));
      // The orbits themselves, faint, and a pale sun: decoration, not data — it mustn't read as a country.
      const system = p.layout === "orbits" ? [
        ...CONTINENTS.map((_, i) => shape(geom.ellipse({ cx: e(cxy[0]), cy: e(cxy[1]), rx: e(`${R} * ${(0.12 + i * 0.062).toFixed(3)}`), ry: e(`${R} * ${TILT} * ${(0.12 + i * 0.062).toFixed(3)}`) }), { key: `orbit-${i}`, stroke: { paint: "$muted@0.28", width: 0.75 }, semantics: { role: "decoration" } })),
        shape(geom.circle({ cx: e(cxy[0]), cy: e(cxy[1]), r: e(`${R} * 0.05`) }), { key: "sun", fill: "#fff4c2", semantics: { role: "decoration" } }),
      ] : [];
      return group({ key: "worlds", children: [...system, repeat(rows, { ...mark, key: e("d.id") } as never), names] });
    }
    // Fitted to the whole sphere, so the globe keeps its size as it turns; an ocean disc under it.
    return group({ key: "worlds", coord: { type: "geo", projection: "orthographic", center: [p.turn ?? 15, 15], fit: { sphere: true }, padding: 70 } as never,
      children: [
        shape(geom.circle({ cx: e("box.w / 2"), cy: e("box.h / 2"), r: e("min(box.w, box.h) / 2 - 70") }), { key: "ocean", fill: "$ink@0.05", stroke: { paint: "$ink@0.12", width: 1 }, semantics: { role: "decoration" } }),
        repeat(rows, { ...mark, key: e("d.id") } as never),
      ] });
  },
});

