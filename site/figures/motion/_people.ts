// The animation page's shared dataset — the 24 most populous countries, 2024 (millions, rounded,
// approximate), with where they are — and its five layouts, as one keyed scene. Not a figure
// itself: the site build registers only files named like `name.ts`, and this one starts with `_`.
import { choreo, data, e, geom, group, repeat, shape, text, type Rule, type Template } from "@datars/sdk";

const ROWS = `
IND India 1441 0 78 21
CHN China 1410 0 104 35
USA United States 335 3 -98 39
IDN Indonesia 280 0 118 -2
PAK Pakistan 245 0 69 30
NGA Nigeria 225 1 8 9
BRA Brazil 216 3 -52 -10
BGD Bangladesh 173 0 90 24
RUS Russia 144 2 60 58
MEX Mexico 129 3 -102 23
ETH Ethiopia 127 1 39 9
JPN Japan 124 0 138 36
PHL Philippines 117 0 122 12
EGY Egypt 113 1 30 27
COD DR Congo 105 1 23 -3
VNM Vietnam 100 0 106 16
IRN Iran 90 0 53 32
TUR Türkiye 86 0 35 39
DEU Germany 84 2 10 51
THA Thailand 72 0 101 15
GBR United Kingdom 68 2 -2 54
FRA France 68 2 2 46
TZA Tanzania 68 1 35 -6
ZAF South Africa 63 1 24 -29`;

/** Continents in palette order (the Americas as one: two of them would be one country each). */
export const CONTINENTS = ["Asia", "Africa", "Europe", "Americas"];

const rows = ROWS.trim().split("\n").map((line) => {
  const f = line.trim().split(/\s+/);
  const [lat, lon, cont, pop] = [f.pop(), f.pop(), f.pop(), f.pop()].map(Number);
  return { code: f[0], name: f.slice(1).join(" "), pop, cont, lon, lat };
});

/** Rank by population (0 = largest) and the place in a grid read by continent, then size. */
const byContinent = [...rows].sort((a, b) => a.cont - b.cont || b.pop - a.pop).map((r) => r.code);

export const PEOPLE = {
  code: rows.map((r) => r.code),
  name: rows.map((r) => r.name),
  pop: rows.map((r) => r.pop),
  cont: rows.map((r) => r.cont),
  lon: rows.map((r) => r.lon),
  lat: rows.map((r) => r.lat),
  i: rows.map((_, i) => i),
  g: rows.map((r) => byContinent.indexOf(r.code)),
};
export const MAX_POP = Math.max(...PEOPLE.pop);

// The plot area under the title and legend, in the box the chart is laid out in.
const TOP = 56;
const PW = "(box.w - 28)";
const PH = `(box.h - ${TOP + 12})`;
const SHORT = `min(${PW}, ${PH})`;
const ink = e("'$categorical[' + d.cont + ']'");
const datum = { role: "datum" as const, label: e("d.name + ': ' + format(d.pop, ',.0f') + ' million'"), value: e("d.pop") };

/** One layout: the countries drawn by `mark` (a shape per row, keyed by country) while `layout`
 * is `name`. Every layout's group has the same key, so a country's mark has the same key path in
 * each — the default matcher pairs it with itself. */
const layout = (name: string, children: Template[]) => group({ key: "chart", when: e(`layout == "${name}"`), children });
const each = (t: Template) => repeat("people", t);

const bw = `(${PW} / 24)`;
const bars = layout("bars", [
  each(shape(geom.rect({
    x: e(`14 + d.i * ${bw} + ${bw} * 0.14`), w: e(`${bw} * 0.72`),
    y: e(`${TOP} + ${PH} - max(1.5, d.pop / ${MAX_POP} * ${PH})`), h: e(`max(1.5, d.pop / ${MAX_POP} * ${PH})`), r: e(`min(3, ${bw} * 0.2)`),
  }), { key: e("d.code"), fill: ink, semantics: datum })),
]);

// Equirectangular, longitudes −112…142 and latitudes −36…64, fitted to the plot area.
const s = `min(${PW} / 254, ${PH} / 100)`;
const map = layout("map", [
  each(shape(geom.circle({
    cx: e(`14 + ${PW} / 2 + (d.lon - 15) * ${s}`), cy: e(`${TOP} + ${PH} / 2 - (d.lat - 14) * ${s}`),
    r: e(`sqrt(d.pop / ${MAX_POP}) * ${SHORT} * 0.13`),
  }), { key: e("d.code"), fill: ink, opacity: 0.88, semantics: datum })),
]);

// A sunflower: the largest at the centre, each next one turned by the golden angle.
const rad = `${SHORT} * 0.094 * sqrt(d.i + 0.5)`;
const spiral = layout("spiral", [
  each(shape(geom.circle({
    cx: e(`14 + ${PW} / 2 + ${rad} * cos(d.i * 2.39996)`), cy: e(`${TOP} + ${PH} / 2 + ${rad} * sin(d.i * 2.39996)`),
    r: e(`sqrt(d.pop / ${MAX_POP}) * ${SHORT} * 0.085`),
  }), { key: e("d.code"), fill: ink, semantics: datum })),
]);

// Tiles by continent, then size: 8 × 3, or 6 × 4 on a narrow box.
const cols = `(box.w < 480 ? 6 : 8)`;
const tile = `min(${PW} / ${cols}, ${PH} / (24 / ${cols}))`;
const tx = `14 + (${PW} - ${cols} * ${tile}) / 2 + (d.g % ${cols}) * ${tile}`;
const ty = `${TOP} + (${PH} - 24 / ${cols} * ${tile}) / 2 + floor(d.g / ${cols}) * ${tile}`;
const grid = layout("grid", [
  each(shape(geom.rect({ x: e(`${tx} + ${tile} * 0.06`), y: e(`${ty} + ${tile} * 0.06`), w: e(`${tile} * 0.88`), h: e(`${tile} * 0.88`), r: e(`${tile} * 0.12`) }), { key: e("d.code"), fill: ink, semantics: datum })),
  each(text(e("d.code"), [e(`${tx} + ${tile} / 2`), e(`${ty} + ${tile} / 2`)], {
    key: e("d.code + '-code'"), style: { font: "font.strong", size: e(`min(13, ${tile} * 0.26)`), ink: e("'on($categorical[' + d.cont + '])'"), align: "middle", baseline: "middle" },
  })),
]);

// The top eight as rows, named and numbered; the other sixteen leave.
const row = `(${PH} / 8)`;
const lw = `min(118, ${PW} * 0.3)`;
const len = `d.pop / ${MAX_POP} * (${PW} - ${lw} - 52)`;
const top = layout("top", [
  each(shape(geom.rect({ x: e(`14 + ${lw}`), y: e(`${TOP} + d.i * ${row} + ${row} * 0.17`), w: e(len), h: e(`${row} * 0.66`), r: 3 }), { key: e("d.code"), when: e("d.i < 8"), fill: ink, semantics: datum })),
  each(text(e("d.name"), [e(`14 + ${lw} - 8`), e(`${TOP} + (d.i + 0.5) * ${row}`)], {
    key: e("d.code + '-name'"), when: e("d.i < 8"), style: { size: "$size.label", ink: "$ink", align: "end", baseline: "middle" },
  })),
  each(text(e("format(d.pop, ',.0f')"), [e(`14 + ${lw} + ${len} + 6`), e(`${TOP} + (d.i + 0.5) * ${row}`)], {
    key: e("d.code + '-value'"), when: e("d.i < 8"), number: { value: e("d.pop"), format: ",.0f" }, style: { size: "$size.label", ink: "$muted", baseline: "middle" },
  })),
]);

/** Every layout's title, in story order. */
export const TITLES: Record<string, string> = {
  bars: "The 24 most populous countries, millions",
  map: "Where they are",
  spiral: "Largest at the centre",
  grid: "By continent",
  top: "The top eight",
};
const title = text(e(Object.entries(TITLES).reduceRight((rest, [k, t]) => `layout == "${k}" ? ${JSON.stringify(t)} : ${rest}`, '""')), [14, 12], {
  key: "title", style: { font: "font.strong", size: 14, ink: "$ink", baseline: "top", maxWidth: e("box.w - 28") },
});
const legend = group({
  key: "legend",
  children: CONTINENTS.flatMap((c, j) => [
    shape(geom.circle({ cx: 18 + j * 84, cy: 40, r: 4 }), { key: `dot-${j}`, fill: `$categorical[${j}]`, semantics: { role: "decoration" } }),
    text(c, [27 + j * 84, 40], { key: `name-${j}`, style: { size: "$size.small", ink: "$muted", baseline: "middle" } }),
  ]),
});

/** The data and scene every people figure shares: a `layout` signal picks the layout. */
export const peopleData = { people: data.values(PEOPLE, { key: "code" }) };
export const peopleScene = group({ key: "root", children: [title, legend, bars, map, spiral, grid, top] });

/** The labels inside the layouts (tile codes, names, values) leave first and arrive last, so a code
 * never waits where its tile is still flying in. */
export const labelRule: Rule = { select: { kind: "text", key: "root/chart" }, choreo: choreo.phased(0.3, 0.3, 0.4) };
