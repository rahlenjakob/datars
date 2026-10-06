// ohlc: a bar per trading day — low to high, the open ticked left, the close right. Synthetic prices
// for a fictional ticker: a seeded random walk (mulberry32), so every build draws the same bars.
import { doc, data, e, group, op, signal, story, step } from "@datars/sdk";
import { plot, ohlc } from "@datars/std";

let s = 7;
const rand = () => { s = (s + 0x6d2b79f5) | 0; let t = Math.imul(s ^ (s >>> 15), s | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 2 ** 32 - 0.5; };
const r2 = (x: number) => Math.round(x * 100) / 100;
const days: Record<string, string | number>[] = [];
let close = 84;
for (let t = Date.UTC(2025, 2, 3); days.length < 40; t += 864e5) {
  if (new Date(t).getUTCDay() % 6 === 0) continue; // weekends: no trading
  const open = close * (1 + rand() * 0.012);
  close = open * (1 + rand() * 0.045 + 0.002);
  const high = Math.max(open, close) * (1 + Math.abs(rand()) * 0.03), low = Math.min(open, close) * (1 - Math.abs(rand()) * 0.03);
  days.push({ date: new Date(t).toISOString().slice(0, 10), open: r2(open), high: r2(high), low: r2(low), close: r2(close) });
}
const chart = (colored: boolean) => plot({ data: "shown", x: "date", y: "close", format: ".0f", title: "Harbour Tools (HBRT), daily",
  children: [ohlc({ colored })] }, { key: "chart", when: e(`state ${colored ? "==" : "!="} "default"`) });

export default doc({
  title: "OHLC bars",
  description: "Forty trading days of OHLC bars coloured by direction, then all in one ink, then the last fifteen days, where every tick widens.",
  size: [640, 320],
  data: { days: data.values(days, { key: "date" }) },
  // The rows on show: the last `sessions` trading days.
  tables: { shown: { from: "days", ops: [op.window("rank", "date", "age", { order: "-date" }), op.filter(e("d.age <= sessions"))] } },
  signals: { sessions: signal.num(40) },
  scene: group({ key: "root", layout: { type: "stack", padding: [16, 20, 12, 12] }, children: [chart(true), chart(false)] }),
  program: story({
    steps: [
      step("default", { set: { sessions: 40 }, title: "ohlc()", text: "Green when the close is at or above the open, red below." }),
      step("ink", { set: { sessions: 40 }, title: "colored: false", text: "Every bar in the theme's ink." }),
      step("zoom", { set: { sessions: 15 }, title: "Fifteen sessions", text: "Fewer rows: the open and close ticks span a wider band." }),
    ],
  }),
});
