// The developers page's bench: one chart with three deliberate flaws that `datars lint` finds,
// each with its fix — the page applies the fixes on the live chart (`setDocument`) and shows what
// lint says about each combination (run on these files when the site is built). The eight files
// `bench-<key><colour><credit>.ts` are the combinations (1: fixed), published as figures, and as
// their documents for the page to hand the running view (/play/developers-bench-…json).
//
//   key    — no key on the source: rows are matched by position, so re-sorting morphs every bar
//            into its neighbour's (lint: identity/row-keys); fixed by `{ key: "name" }`.
//   colour — a colour per country: twelve categorical colours (encoding/too-many-colours); fixed
//            by colouring by continent.
//   credit — the source line set to start where it should end, so it runs off the canvas
//            (legibility/offscreen); fixed by aligning it to the end.
//
// Populations: the twelve most populous countries, rounded 2024 estimates in millions, as in the
// worlds example (approximate).
import { doc, data, e, group, motion, op, signal, step, story, text } from "@datars/sdk";
import { plot, bar } from "@datars/std";

const ROWS: [string, string, number][] = [
  ["India", "Asia", 1440], ["China", "Asia", 1410], ["United States", "North America", 335], ["Indonesia", "Asia", 280],
  ["Pakistan", "Asia", 245], ["Nigeria", "Africa", 225], ["Brazil", "South America", 216], ["Bangladesh", "Asia", 173],
  ["Russia", "Europe", 144], ["Mexico", "North America", 129], ["Ethiopia", "Africa", 127], ["Japan", "Asia", 124],
];

export function bench(fixed: { key: boolean; colour: boolean; credit: boolean }, id: string) {
  const title = "The twelve most populous countries (millions)";
  const chart = (table: string) => plot({
    data: table, x: "people", y: "name", xType: "linear", yType: "band", color: fixed.colour ? "continent" : "name",
    title, legend: fixed.colour, padding: 0.24,
    children: [bar({ labels: true, format: ",.0f" })],
  }, { key: "chart", when: e(`order == ${JSON.stringify(table)}`) });
  return doc({
    id,
    title: "The twelve most populous countries",
    description: "The twelve most populous countries in 2024, in millions — India 1,440, China 1,410, the United States 335, Indonesia 280, Pakistan 245, Nigeria 225, Brazil 216, Bangladesh 173, Russia 144, Mexico 129, Ethiopia 127, Japan 124 — ranked, then in alphabetical order.",
    size: [640, 420],
    data: {
      countries: data.values({ name: ROWS.map((r) => r[0]), continent: ROWS.map((r) => r[1]), people: ROWS.map((r) => r[2]) }, fixed.key ? { key: "name" } : {}),
    },
    tables: {
      ranked: { from: "countries", ops: [op.sort(["people", "desc"])] },
      az: { from: "countries", ops: [op.sort("name")] },
    },
    signals: { order: signal.str("ranked") },
    scene: group({
      key: "root", layout: { type: "rows", gap: 4, padding: [10, 14, 8, 6] },
      children: [
        group({ key: "plots", size: { h: "fill" }, children: [chart("ranked"), chart("az")] }),
        text("Rounded 2024 estimates (approximate)", [e("box.w"), 0], { key: "credit", size: { h: 16 }, style: { size: 11, ink: "$muted", baseline: "top", align: fixed.credit ? "end" : "start" } }),
      ],
    }),
    motion: motion({ select: { role: "datum" }, matcher: "by-key", duration: 1.1 }),
    program: story({ steps: [step("ranked", { set: { order: "ranked" } }), step("a–z", { set: { order: "az" } })] }),
  });
}
