// The iconography page's accessibility figure: three ferry, bus and train lines' punctuality this
// week. The mode icons beside each name are decorative (the name says it); each trend arrow is
// labelled ("Rising", "Falling", "Steady"), so a screen reader hears it. Everything is sized by the
// theme's tokens, so the page's text-size buttons grow it all.
import { data, doc, e, group, icon, repeat, text } from "@datars/sdk";

const lines = [
  { id: "ferry", name: "Harbour ferry", mode: "ship", on: 97, trend: "steady" },
  { id: "bus", name: "Coast bus 4", mode: "bus", on: 92, trend: "up" },
  { id: "train", name: "Valley train", mode: "train-front", on: 81, trend: "down" },
];
const t = (m: Record<string, string>) => Object.entries(m).reduceRight((rest, [k, v]) => `(d.trend == "${k}" ? ${JSON.stringify(v)} : ${rest})`, '""');
const rowH = "((box.h - 8) / 3)";
const y = `(4 + ${rowH} * d.i + ${rowH} / 2)`;
const S = 'token("size.icon") * 1.5';
// A narrow box at a reader's big text: the name takes the row's first line, the value its second.
const room = `box.w - ${S} * 2 - 30 - measure("100%", token("size.title"), 700)`;
const stack = `measure(d.name, token("size.body"), 600) > ${room}`;
const up = `(${stack} ? token("size.body") * 0.75 : 0)`;

export default doc({
  id: "iconography-a11y",
  title: "On time this week",
  description: "Three fictional lines' share of journeys on time this week: the harbour ferry 97 per cent, steady; coast bus 4 92 per cent, rising; the valley train 81 per cent, falling.",
  size: [480, 200],
  data: { lines: data.values({ id: lines.map((l) => l.id), name: lines.map((l) => l.name), mode: lines.map((l) => l.mode), on: lines.map((l) => l.on), trend: lines.map((l) => l.trend), i: lines.map((_, i) => i) }, { key: "id" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [6, 12, 6, 10] },
    children: [
      repeat("lines", group({
        key: e("d.id"),
        semantics: { role: "datum", label: e('d.name + ": " + format(d.on, ",.0f") + "% on time"'), value: e("d.on") },
        children: [
          icon(e("d.mode"), [e(`${S} / 2`), e(y)], { key: "mode", size: e(S), ink: "$ink-2" }),
          text(e("d.name"), [e(`${S} + 10`), e(`${y} - ${up}`)], { key: "name", style: { size: "$size.body", weight: 600, ink: "$ink", baseline: "middle" } }),
          text(e('format(d.on, ",.0f") + "%"'), [e(`box.w - ${S} - 10`), e(`${y} + ${up} * 1.2`)], { key: "on", style: { size: "$size.title", weight: 700, ink: "$ink", align: "end", baseline: "middle" } }),
          icon(e(t({ up: "trending-up", down: "trending-down" }) + ' == "" ? "minus" : ' + t({ up: "trending-up", down: "trending-down" })), [e(`box.w - ${S} / 2`), e(`${y} + ${up} * 1.2`)], { key: "trend", size: e(S), ink: e(t({ up: "$positive", down: "$negative" }) + ' == "" ? "$ink-2" : ' + t({ up: "$positive", down: "$negative" })), label: e(t({ up: "Rising", down: "Falling", steady: "Steady" })) }),
        ],
      })),
    ],
  }),
});
