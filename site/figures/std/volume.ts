// volume: volume bars in a pane under the candles, on the same trading days. Synthetic prices and
// volumes for a fictional ticker: a seeded random walk (mulberry32), the same in every build.
import { doc, data, e, group, op, signal, story, step } from "@datars/sdk";
import { plot, candlestick, volume } from "@datars/std";

let s = 3;
const rand = () => { s = (s + 0x6d2b79f5) | 0; let t = Math.imul(s ^ (s >>> 15), s | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 2 ** 32 - 0.5; };
const r2 = (x: number) => Math.round(x * 100) / 100;
const days: Record<string, string | number>[] = [];
let close = 84;
for (let t = Date.UTC(2025, 2, 3); days.length < 40; t += 864e5) {
  if (new Date(t).getUTCDay() % 6 === 0) continue; // weekends: no trading
  const open = close * (1 + rand() * 0.012);
  close = open * (1 + rand() * 0.045 + 0.002);
  const high = Math.max(open, close) * (1 + Math.abs(rand()) * 0.03), low = Math.min(open, close) * (1 - Math.abs(rand()) * 0.03);
  const shares = Math.round(1.2e6 * (1 + Math.abs(rand()) * 1.5 + Math.abs(close / open - 1) * 30));
  days.push({ date: new Date(t).toISOString().slice(0, 10), open: r2(open), high: r2(high), low: r2(low), close: r2(close), volume: shares });
}
const chart = (colored: boolean) => plot({ data: "shown", x: "date", y: "close", format: ".0f", title: "Harbour Tools (HBRT), daily",
  children: [candlestick(), volume({ colored })] }, { key: "chart", when: e(`state ${colored ? "==" : "!="} "default"`) });

export default doc({
  title: "Volume",
  description: "Candles with each day's volume in a pane below, coloured like its candle, then in one muted ink, then zoomed to fifteen days: both panes share the dates.",
  size: [640, 360],
  data: { days: data.values(days, { key: "date" }) },
  // The rows on show: the last `sessions` trading days.
  tables: { shown: { from: "days", ops: [op.window("rank", "date", "age", { order: "-date" }), op.filter(e("d.age <= sessions"))] } },
  signals: { sessions: signal.num(40) },
  scene: group({ key: "root", layout: { type: "stack", padding: [16, 20, 12, 12] }, children: [chart(true), chart(false)] }),
  program: story({
    steps: [
      step("default", { set: { sessions: 40 }, title: "volume()", text: "A pane under the price area with its own axis, coloured like each day's candle." }),
      step("muted", { set: { sessions: 40 }, title: "colored: false", text: "Every bar in $muted, receding behind the candles." }),
      step("zoom", { set: { sessions: 15 }, title: "Fifteen sessions", text: "The panes share the date axis: both follow the rows on show." }),
    ],
  }),
});
