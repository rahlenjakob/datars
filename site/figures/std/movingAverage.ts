// movingAverage: the mean of the last `window` closes as a line over the price. Synthetic prices
// for a fictional ticker: a seeded random walk (mulberry32), the same in every build.
import { doc, data, e, group, op, story, step } from "@datars/sdk";
import { plot, line, movingAverage } from "@datars/std";

let s = 21;
const rand = () => { s = (s + 0x6d2b79f5) | 0; let t = Math.imul(s ^ (s >>> 15), s | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 2 ** 32 - 0.5; };
const days: { date: string; close: number }[] = [];
let close = 84;
for (let t = Date.UTC(2025, 0, 6); days.length < 130; t += 864e5) {
  if (new Date(t).getUTCDay() % 6 === 0) continue; // weekends: no trading
  close *= 1 + rand() * 0.04 + 0.0015 * Math.sin(days.length / 14);
  days.push({ date: new Date(t).toISOString().slice(0, 10), close: Math.round(close * 100) / 100 });
}
const when = (...states: string[]) => ({ when: e(states.map((st) => `state == "${st}"`).join(" || ")) });

export default doc({
  title: "Moving averages",
  description: "A 20-day average that starts only once twenty rows are on show, then computed over the full history so it starts on day one, then joined by a 50-day average.",
  size: [640, 320],
  // 130 trading days of history; the chart shows the last 70.
  data: { days: data.values(days, { key: "date" }) },
  tables: { shown: { from: "days", ops: [op.window("rank", "date", "age", { order: "-date" }), op.filter(e("d.age <= 70"))] } },
  scene: group({ key: "root", layout: { type: "stack", padding: [16, 20, 12, 12] }, children: [
    plot({ data: "shown", x: "date", y: "close", format: ".0f", title: "Harbour Tools (HBRT), close", children: [
      line({ stroke: "$muted", width: 1.5, curve: "linear" }),
      movingAverage({ window: 20 }, when("default")),
      movingAverage({ window: 20, source: "days" }, when("source", "window")),
      movingAverage({ window: 50, source: "days", stroke: "$highlight" }, when("window")),
    ] }, { key: "chart" }),
  ] }),
  program: story({
    steps: [
      step("default", { title: "movingAverage({ window: 20 })", text: "Over the rows on show, the line starts at the twentieth day." }),
      step("source", { title: "source: \"days\"", text: "Computed over the whole history: a full window from the first day." }),
      step("window", { title: "window: 50", text: "A slower average beside it; each is named in the key above the chart." }),
    ],
  }),
});
