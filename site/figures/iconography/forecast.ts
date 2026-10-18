// The iconography page's forecast: a week of a fictional coast's weather, an icon a day — from the
// built-in set (Lucide, outlines) or from the page's own SVG files (filled, two colours each). The
// files' colours never reach the chart: each slot is an ink (`$highlight`, `$muted`, `$accent`),
// so the page's themes (`setTokens`) and modes repaint both sets. Both sets are states, so the
// published bundle carries every icon either draws.
import { data, doc, e, geom, group, icon, repeat, shape, signal, story, step, text } from "@datars/sdk";

const days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const kinds = ["sun", "cloud-sun", "cloud-rain", "cloud-lightning", "cloud-rain", "cloud-sun", "sun"];
const said = { sun: "Sunny", "cloud-sun": "Bright spells", "cloud-rain": "Rain", "cloud-lightning": "Thunderstorms", cloud: "Cloudy" } as Record<string, string>;
const pick = (m: Record<string, string>, fallback: string) => Object.entries(m).reduceRight((rest, [k, v]) => `(d.kind == "${k}" ? ${JSON.stringify(v)} : ${rest})`, JSON.stringify(fallback));
// The node's own ink (an outline icon, a one-colour file) and the files' slots, by kind.
const ink = pick({ sun: "$highlight", "cloud-sun": "$highlight", "cloud-rain": "$accent", "cloud-lightning": "$highlight" }, "$muted");
const slot0 = pick({ "cloud-sun": "$highlight" }, "$muted");
const slot1 = pick({ "cloud-sun": "$muted", "cloud-rain": "$accent", "cloud-lightning": "$highlight" }, "$muted");
const colW = "(box.w / 7)";
const cx = `(${colW} * (d.i + 0.5))`;
const size = `min(52, ${colW} * 0.56, box.h * 0.24)`;

export default doc({
  id: "iconography-forecast",
  title: "A week on the Fernmouth coast",
  description: "A fictional seven-day forecast: sun on Monday, bright spells, rain on Wednesday, thunderstorms on Thursday, rain again, then bright spells and sun at the weekend, with each day's high and low and its chance of rain.",
  size: [640, 260],
  data: { week: data.values({ day: days, i: days.map((_, i) => i), kind: kinds, hi: [21, 19, 15, 14, 16, 18, 22], lo: [12, 11, 10, 9, 10, 11, 13], rain: [0, 10, 70, 90, 60, 20, 0] }, { key: "day" }) },
  signals: { set: signal.str("lucide") },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [10, 8, 10, 8] },
    children: [
      repeat("week", group({
        key: e("d.day"),
        semantics: { role: "datum", label: e(`d.day + ": " + ${pick(said, "Cloudy")} + ", " + format(d.hi, ",.0f") + "° to " + format(d.lo, ",.0f") + "°, " + format(d.rain, ",.0f") + "% chance of rain"`), value: e("d.hi") },
        children: [
          shape(geom.rect({ x: e(`${colW} * d.i + 3`), y: 0, w: e(`${colW} - 6`), h: e("box.h"), r: 10 }), { key: "card", fill: e('d.i % 2 == 0 ? "$surface" : "$paper"'), semantics: { role: "decoration" } }),
          text(e("d.day"), [e(cx), e("box.h * 0.1")], { key: "day", style: { size: "$size.label", weight: 600, ink: "$ink-2", align: "middle", baseline: "middle" } }),
          icon(e(`set == "ours" ? "assets/" + d.kind + ".svg" : d.kind`), [e(cx), e("box.h * 0.36")], { key: "icon", size: e(size), ink: e(ink), inks: [e(slot0), e(slot1)] }),
          text(e('format(d.hi, ",.0f") + "°"'), [e(cx), e("box.h * 0.62")], { key: "hi", style: { size: "$size.title", weight: 700, ink: "$ink", align: "middle", baseline: "middle" } }),
          text(e('format(d.lo, ",.0f") + "°"'), [e(cx), e("box.h * 0.62 + token(\"size.title\") * 1.15")], { key: "lo", style: { size: "$size.label", ink: "$muted", align: "middle", baseline: "middle" } }),
          icon("droplet", [e(`${cx} - token("size.small") * 1.3`), e("box.h * 0.9")], { key: "drop", size: e('token("size.small") * 1.2'), ink: e('d.rain >= 50 ? "$accent" : "$muted"') }),
          text(e('format(d.rain, ",.0f") + "%"'), [e(`${cx} - token("size.small") * 0.5`), e("box.h * 0.9")], { key: "rain", style: { size: "$size.small", ink: "$ink-2", align: "start", baseline: "middle" } }),
        ],
      })),
    ],
  }),
  program: story({
    steps: [
      step("lucide", { set: { set: "lucide" }, title: "The built-in set", text: "Lucide's outlines, in the theme's inks." }),
      step("ours", { set: { set: "ours" }, title: "Your own files", text: "Filled, two-colour SVGs: each colour an ink slot." }),
    ],
  }),
});
