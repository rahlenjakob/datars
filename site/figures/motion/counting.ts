// Numbers count: a text that is a number (`number: { value, format }`) ticks through the values
// between its old and new value, formatted at every frame — the year, the people, the share in
// cities, the life expected — while each bar grows to its value. Figures: UN World Population and
// Urbanization Prospects (rounded; 2050 is the median projection).
import { doc, e, geom, group, motion, shape, signal, step, story, text, type Template } from "@datars/sdk";

const YEARS: [string, number, number, number][] = [
  ["1950", 2.5, 30, 46.5],
  ["1990", 5.3, 43, 64.0],
  ["2024", 8.1, 57, 73.3],
  ["2050", 9.7, 68, 77.2],
];

const BIG = "min(40, box.w * 0.2)";
const BASE = `20 + ${BIG} * 0.82`;

const tile = (key: string, label: string, value: string, format: string, unit: string, max: number): Template => group({
  key,
  children: [
    text(label, [0, 0], { key: "label", style: { size: "$size.label", ink: "$muted", baseline: "top" } }),
    // The number and its unit share a baseline, under the label.
    text("", [0, e(BASE)], { key: "value", number: { value: e(value), format }, style: { font: "font.title", size: e(BIG), ink: "$ink", baseline: "alphabetic" }, semantics: { role: "datum", label: e(`'${label}: ' + format(${value}, '${format}') + '${unit}'`), value: e(value) } }),
    text(unit.trim(), [e(`measure(format(${value}, '${format}'), ${BIG}, 700) + 6`), e(BASE)], { key: "unit", style: { size: "$size.body", ink: "$ink-2", baseline: "alphabetic" } }),
    shape(geom.rect({ x: 0, y: e(`${BASE} + 16`), w: e("box.w"), h: 6, r: 3 }), { key: "track", fill: "$grid", semantics: { role: "decoration" } }),
    shape(geom.rect({ x: 0, y: e(`${BASE} + 16`), w: e(`box.w * ${value} / ${max}`), h: 6, r: 3 }), { key: "bar", fill: "$accent", semantics: { role: "decoration" } }),
  ],
});

const tiles = (columns: number, when: string) => group({
  key: "tiles", when: e(when), size: { h: "fill" },
  layout: { type: "grid", columns, gap: 18 },
  children: [
    tile("people", "World population", "people", ".1f", " billion", 10),
    tile("urban", "Living in cities", "urban", ".0f", " %", 100),
    tile("life", "Life expectancy at birth", "life", ".1f", " years", 80),
  ],
});

export default doc({
  id: "motion-counting",
  title: "Numbers that count",
  description: "The world in 1950, 1990, 2024 and 2050: population, the share living in cities and life expectancy, each a number that counts to its new value as a bar grows.",
  size: [640, 200],
  signals: { year: signal.num(1950), people: signal.num(2.5), urban: signal.num(30), life: signal.num(46.5) },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 18, padding: [14, 18, 14, 18] },
    children: [
      text("", [0, 0], { key: "year", number: { value: e("year"), format: "d" }, size: { h: 34 }, style: { font: "font.title", size: 30, ink: "$accent", baseline: "top" }, semantics: { role: "title", label: e("'The world in ' + year") } }),
      tiles(3, 'sizeClass != "phone"'),
      tiles(1, 'sizeClass == "phone"'),
    ],
  }),
  motion: motion({ duration: 1.6, easing: "cubic-in-out" }),
  program: story({
    steps: YEARS.map(([y, people, urban, life]) => step(y, { set: { year: Number(y), people, urban, life } })),
  }),
});
