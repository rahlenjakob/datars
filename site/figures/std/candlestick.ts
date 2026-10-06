// candlestick: a candle per trading day on a band axis (no weekend gaps). Synthetic prices for a
// fictional ticker: a seeded random walk (mulberry32), so every build draws the same candles.
import { doc, data, e, group, op, signal, story, step } from "@datars/sdk";
import { plot, candlestick } from "@datars/std";

let s = 7;
const rand = () => { s = (s + 0x6d2b79f5) | 0; let t = Math.imul(s ^ (s >>> 15), s | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 2 ** 32 - 0.5; };
const r2 = (x: number) => Math.round(x * 100) / 100;
const days: Record<string, string | number>[] = [];
let close = 84;
for (let t = Date.UTC(2025, 2, 3); days.length < 60; t += 864e5) {
  if (new Date(t).getUTCDay() % 6 === 0) continue; // weekends: no trading
  const open = close * (1 + rand() * 0.012);
  close = open * (1 + rand() * 0.045 + 0.002);
  const high = Math.max(open, close) * (1 + Math.abs(rand()) * 0.03), low = Math.min(open, close) * (1 - Math.abs(rand()) * 0.03);
  days.push({ date: new Date(t).toISOString().slice(0, 10), open: r2(open), high: r2(high), low: r2(low), close: r2(close) });
}
const chart = (hollow: boolean) => plot({ data: "shown", x: "date", y: "close", format: ".0f", title: "Harbour Tools (HBRT), daily",
  children: [candlestick({ hollow })] }, { key: "chart", when: e(`state ${hollow ? "!=" : "=="} "default"`) });

export default doc({
  title: "Candlesticks",
  description: "Sixty trading days of candles, then hollow candles, then the last twenty days, where every candle widens.",
  size: [640, 320],
  data: { days: data.values(days, { key: "date" }) },
  // The rows on show: the last `sessions` trading days.
  tables: { shown: { from: "days", ops: [op.window("rank", "date", "age", { order: "-date" }), op.filter(e("d.age <= sessions"))] } },
  signals: { sessions: signal.num(60) },
  scene: group({ key: "root", layout: { type: "stack", padding: [16, 20, 12, 12] }, children: [chart(false), chart(true)] }),
  program: story({
    steps: [
      step("default", { set: { sessions: 60 }, title: "candlestick()", text: "Green when the close is at or above the open, red below." }),
      step("hollow", { set: { sessions: 60 }, title: "hollow: true", text: "Rising candles outlined, falling ones filled." }),
      step("month", { set: { sessions: 20 }, title: "Twenty sessions", text: "Fewer rows: each candle takes a wider band." }),
    ],
  }),
});
