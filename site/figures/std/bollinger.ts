// bollinger: a moving average with a band `k` standard deviations either side — how stretched and
// how volatile the price is. Synthetic prices for a fictional ticker (a seeded random walk).
import { doc, data, e, group, op, story, step } from "@datars/sdk";
import { plot, line, bollinger } from "@datars/std";

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
  title: "Bollinger bands",
  description: "A 20-day band two standard deviations wide over the rows on show, then computed over the whole history so it starts on day one, then a second band one deviation wide inside it.",
  size: [640, 320],
  // 130 trading days of history; the chart shows the last 70.
  data: { days: data.values(days, { key: "date" }) },
  tables: { shown: { from: "days", ops: [op.window("rank", "date", "age", { order: "-date" }), op.filter(e("d.age <= 70"))] } },
  scene: group({ key: "root", layout: { type: "stack", padding: [16, 20, 12, 12] }, children: [
    plot({ data: "shown", x: "date", y: "close", format: ".0f", title: "Harbour Tools (HBRT), close", children: [
      line({ stroke: "$muted", width: 1.5, curve: "linear" }),
      bollinger({}, when("default")),
      bollinger({ source: "days" }, when("source", "k")),
      bollinger({ source: "days", k: 1, opacity: 0.12 }, when("k")),
    ] }, { key: "chart" }),
  ] }),
  program: story({
    steps: [
      step("default", { title: "bollinger()", text: "20 days, 2 σ, over the rows on show: the band starts at the twentieth day." }),
      step("source", { title: "source: \"days\"", text: "Computed over the whole history: a full window from the first day." }),
      step("k", { title: "k: 1", text: "A second band, one standard deviation wide, inside the first." }),
    ],
  }),
});
