// The iconography page's curved text: a fictional Fernmouth commuter's day as a 24-hour ring, each
// part of the day named along its own arc — upright on the ring's lower half, left out where an arc
// is too short for its name. Weekday and weekend are states, keyed by part, so the ring morphs.
import { data, doc, e, geom, group, repeat, shape, story, step, text } from "@datars/sdk";

// [key, weekday name, start, end, weekend name, start, end] in hours (past midnight: + 24).
const parts: [string, string, number, number, string, number, number][] = [
  ["sleep", "Sleep", 23, 30.5, "Sleep", 24.5, 33],
  ["morning", "Breakfast", 6.5, 7.5, "Slow breakfast", 9, 10.5],
  ["out", "Commute", 7.5, 8.5, "Walk", 10.5, 11.5],
  ["work-am", "Work", 8.5, 12, "Market", 11.5, 13],
  ["lunch", "Lunch", 12, 13, "Lunch", 13, 14.5],
  ["work-pm", "Work", 13, 17, "Beach", 14.5, 18],
  ["home", "Commute", 17, 18, "Cycle home", 18, 19],
  ["evening", "Evening", 18, 23, "Friends", 19, 24.5],
];
const col = (i: number) => parts.map((p) => p[i]);
const we = 'state == "weekend"';
const pick = (a: string, b: string) => `(${we} ? ${b} : ${a})`;
const TAU = 2 * Math.PI;
const a0 = `(${pick("d.ws", "d.es")} / 24 * ${TAU})`;
const a1 = `(${pick("d.we", "d.ee")} / 24 * ${TAU})`;
// Room round the ring for the hour labels, at the reader's text size.
const R = '(min(box.h / 2 - token("size.small") * 2.7, box.w / 2 - token("size.small") * 4.4))';
const cx = "(box.w / 2)", cy = "(box.h / 2)";
// The hole holds the day's name; at a reader's big text the ring gets thinner to make room.
const r0 = `clamp(measure("Weekend", token("size.title"), 700) / 2 + 8, ${R} * 0.6, ${R} * 0.78)`;
const rm = `((${R} + ${r0}) / 2)`;
const name = pick("d.wn", "d.en");
const size = 'token("size.label")';
const fits = `measure(${name}, ${size}, 600) < (${a1} - ${a0}) * ${rm} * 0.86 && ${size} * 1.6 < ${R} - ${r0}`;

export default doc({
  id: "iconography-day",
  title: "A Fernmouth commuter's day",
  description: "A fictional commuter's 24 hours as a ring: on a weekday, sleep, breakfast, the commute, work, lunch, work, the commute home and the evening; at the weekend, a long sleep, a slow breakfast, a walk, the market, lunch, the beach, cycling home and friends.",
  size: [480, 360],
  data: { day: data.values({ part: col(0), wn: col(1), ws: col(2), we: col(3), en: col(4), es: col(5), ee: col(6), i: parts.map((_, i) => i) }, { key: "part" }) },
  scene: group({
    key: "root",
    children: [
      repeat("day", group({
        key: e("d.part"),
        semantics: { role: "datum", label: e(`${name} + ", " + format(${pick("d.we - d.ws", "d.ee - d.es")}, ",~g") + " hours"`) },
        children: [
          shape(geom.arc({ cx: e(cx), cy: e(cy), r0: e(r0), r1: e(R), a0: e(a0), a1: e(a1) }), { key: "arc", fill: e('"$categorical[" + d.i + "]"'), stroke: { paint: "$paper", width: 2 }, semantics: { role: "decoration" } }),
          text(e(name), [0, 0], { key: "name", when: e(fits), path: { arc: [e(cx), e(cy), e(rm), e(a0), e(a1)] }, style: { size: e(size), weight: 600, ink: e('"on($categorical[" + d.i + "])"'), align: "middle", baseline: "middle" } }),
        ],
      })),
      ...[0, 6, 12, 18].map((h) => {
        const a = (h / 24) * TAU;
        return text(h === 0 ? "midnight" : h === 12 ? "noon" : `${h}:00`, [e(`${cx} + (${R} + token("size.small") * 1.1) * ${Math.sin(a).toFixed(6)}`), e(`${cy} - (${R} + token("size.small") * 1.1) * ${Math.cos(a).toFixed(6)}`)], { key: `h${h}`, style: { size: "$size.small", ink: "$muted", align: h === 6 ? "start" : h === 18 ? "end" : "middle", baseline: h === 0 ? "bottom" : h === 12 ? "top" : "middle" } });
      }),
      text(e(`${we} ? "Weekend" : "Weekday"`), [e(cx), e(`${cy} - token("size.title") * 0.15`)], { key: "which", style: { size: "$size.title", weight: 700, ink: "$ink", align: "middle", baseline: "bottom" } }),
      text("24 hours", [e(cx), e(`${cy} + token("size.title") * 0.15`)], { key: "hours", style: { size: "$size.small", ink: "$muted", align: "middle", baseline: "top" } }),
    ],
  }),
  program: story({ steps: [step("weekday", { title: "A weekday", text: "Work in two halves, a commute either side." }), step("weekend", { title: "The weekend", text: "The same day, keyed by part: it morphs." })] }),
});
