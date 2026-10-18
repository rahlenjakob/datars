// The iconography page's "bring an SVG": four fictional parties' symbols, project SVG files of two
// or three colours each, beside their seats. Each file's colours are ink slots mapped to the
// party's own tokens (`party.<id>`, `.mark`, `.detail`), so the page restyles every logo with
// `setTokens` — their colours, a house style, one ink for print — and the file's hexes never
// reach a pixel.
import { data, doc, e, geom, group, icon, repeat, shape, text } from "@datars/sdk";

const parties = [
  { id: "labour", name: "Harbour Labour", seats: 23, c: "#c8423b", mark: "#ffffff", detail: "#7a1d18" },
  { id: "greens", name: "Coast Greens", seats: 17, c: "#2f8f5b", mark: "#ffffff", detail: "#a7e0b8" },
  { id: "first", name: "Fernmouth First", seats: 14, c: "#1d4e89", mark: "#ffffff", detail: "#f5b53d" },
  { id: "centre", name: "Liberal Centre", seats: 9, c: "#f5b53d", mark: "#1d3f6e", detail: "#1d3f6e" },
];
const files: Record<string, string> = { labour: "harbour-labour", greens: "coast-greens", first: "fernmouth-first", centre: "liberal-centre" };
const tokens: Record<string, string> = {};
for (const p of parties) Object.assign(tokens, { [`party.${p.id}`]: p.c, [`party.${p.id}.mark`]: p.mark, [`party.${p.id}.detail`]: p.detail });
const n = parties.length;
const rowH = `((box.h - 8) / ${n})`;
const y = `(4 + ${rowH} * d.rank + ${rowH} / 2)`;
const sym = `min(44, ${rowH} * 0.78)`;
const labelW = `min(170, box.w * 0.3)`;
const x0 = `(${sym} + 14 + ${labelW})`;
const barW = `((box.w - ${x0} - 44) * d.seats / 23)`;

export default doc({
  id: "iconography-parties",
  title: "Seats in the Fernmouth council",
  description: "A fictional council of 63 seats: Harbour Labour 23, Coast Greens 17, Fernmouth First 14 and Liberal Centre 9, each beside its party symbol.",
  size: [640, 240],
  theme: { use: "datars/neutral", tokens },
  data: { seats: data.values({ id: parties.map((p) => p.id), name: parties.map((p) => p.name), seats: parties.map((p) => p.seats), rank: parties.map((_, i) => i), file: parties.map((p) => `assets/${files[p.id]}.svg`) }, { key: "id" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [8, 12, 8, 8] },
    children: [
      repeat("seats", group({
        key: e("d.id"),
        semantics: { role: "datum", label: e('d.name + ": " + format(d.seats, ",.0f") + " seats"'), value: e("d.seats") },
        children: [
          icon(e("d.file"), [e(`${sym} / 2 + 2`), e(y)], { key: "symbol", size: e(sym), inks: [e('"$party." + d.id'), e('"$party." + d.id + ".mark"'), e('"$party." + d.id + ".detail"')], label: e('d.name + " symbol"') }),
          text(e("d.name"), [e(`${sym} + 12`), e(y)], { key: "name", style: { size: "$size.label", weight: 600, ink: "$ink", baseline: "middle", maxWidth: e(labelW) } }),
          shape(geom.rect({ x: e(x0), y: e(`${y} - ${rowH} * 0.22`), w: e(barW), h: e(`${rowH} * 0.44`), r: 4 }), { key: "bar", fill: e('"$party." + d.id'), semantics: { role: "decoration" } }),
          text(e('format(d.seats, ",.0f")'), [e(`${x0} + ${barW} + 8`), e(y)], { key: "seats", style: { size: "$size.label", ink: "$ink-2", baseline: "middle" } }),
        ],
      })),
    ],
  }),
});
