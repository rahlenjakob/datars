// The "Finance" family on one market: a fictional ticker's 120 trading days (open, high, low,
// close, volume) and two more tickers' closes. The chart shows the last 60 days; the averages and
// bands are computed over the whole history. Candles are keyed by date, so each becomes its OHLC
// bar. (A seeded random walk, mulberry32: every build draws the same prices.)
import { doc, data, e, group, op, repeat, story, step, text, motion } from "@datars/sdk";
import { plot, candlestick, ohlc, volume, line, movingAverage, bollinger, indexed, drawdown, sparkline } from "@datars/std";

let s = 7;
const rand = () => { s = (s + 0x6d2b79f5) | 0; let t = Math.imul(s ^ (s >>> 15), s | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 2 ** 32 - 0.5; };
const r2 = (x: number) => Math.round(x * 100) / 100;
const days: Record<string, string | number>[] = [];
const closes: { ticker: string; date: string; close: number }[] = [];
const others: [string, number, number][] = [["MOSS", 31, 0.0008], ["QUAY", 212, -0.0012]];
const other = others.map((o) => o[1]);
let close = 84;
for (let t = Date.UTC(2025, 0, 6); days.length < 120; t += 864e5) {
  if (new Date(t).getUTCDay() % 6 === 0) continue; // weekends: no trading
  const date = new Date(t).toISOString().slice(0, 10);
  const open = close * (1 + rand() * 0.012);
  close = open * (1 + rand() * 0.045 + 0.0015 * Math.sin(days.length / 12));
  const high = Math.max(open, close) * (1 + Math.abs(rand()) * 0.03), low = Math.min(open, close) * (1 - Math.abs(rand()) * 0.03);
  days.push({ date, open: r2(open), high: r2(high), low: r2(low), close: r2(close), volume: Math.round((1.2 + Math.abs(rand()) * 2 + Math.abs(close - open) / 2) * 1e5) });
  closes.push({ ticker: "HBRT", date, close: r2(close) });
  others.forEach(([ticker, , drift], i) => { other[i] *= 1 + rand() * 0.05 + drift; closes.push({ ticker, date, close: r2(other[i]) }); });
}

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const last60 = [op.window("rank", "date", "age", { order: "-date" }), op.filter(e("d.age <= 60"))];
const price = { data: "shown", x: "date", y: "close", format: ".0f", title: "Harbour Tools (HBRT), daily" } as const;
const cell = { size: "$size.body", ink: "$ink", baseline: "middle" } as const;

export default doc({
  title: "A fictional market, drawn by the finance recipes",
  description: "Sixty trading days of a ticker as candles with volume, as OHLC bars, with moving averages, with Bollinger bands; then three tickers rebased to 100, their drawdowns, and a watchlist of sparklines.",
  size: [640, 400],
  data: {
    days: data.values(days, { key: "date" }),
    closes: data.values(closes, { key: ["ticker", "date"] }),
  },
  tables: {
    shown: { from: "days", ops: last60 },
    recent: { from: "closes", ops: [op.window("rank", "date", "age", { order: "-date", partition: ["ticker"] }), op.filter(e("d.age <= 60"))] },
  },
  motion: motion({ select: { role: "datum" }, matcher: "by-key", duration: 1.1 }),
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [14, 18, 10, 10] },
    children: [
      plot({ ...price, children: [candlestick(), volume()] }, at("candles")),
      plot({ ...price, children: [ohlc(), volume({ colored: false })] }, at("bars")),
      plot({ ...price, children: [candlestick(), movingAverage({ window: 20, source: "days" }), movingAverage({ window: 50, source: "days", stroke: "$highlight" })] }, at("averages")),
      plot({ ...price, title: "Harbour Tools (HBRT), close", children: [line({ stroke: "$muted", width: 1.5, curve: "linear" }), bollinger({ source: "days" })] }, at("bollinger")),
      plot({ data: "recent", x: "date", y: "close", color: "ticker", format: ".0f", title: "Three tickers, rebased to 100", children: [indexed()] }, at("indexed")),
      plot({ data: "recent", x: "date", y: "close", color: "ticker", format: ".0%", title: "Below each ticker's running peak", children: [drawdown()] }, at("drawdown")),
      group({ key: "chart", when: e('state == "sparklines"'), layout: { type: "grid", columns: 1, gap: 18, padding: [30, 0, 30, 0] }, children: [
        repeat({ groups: "recent", by: "ticker" }, group({ layout: { type: "columns", gap: 24 }, children: [
          group({ key: "name", size: { w: 70 }, children: [text(e("d.ticker"), [0, e("box.h / 2")], { style: { ...cell, weight: 600 } })] }),
          sparkline({ data: "@group", x: "date", y: "close", name: "Close" }),
          group({ key: "last", size: { w: 150 }, children: [text(e('format(group.last("close"), ",.2f") + "  " + format(group.last("close") / group.first("close") - 1, "+.1%")'), [e("box.w"), e("box.h / 2")], { style: { ...cell, ink: "$ink-2", align: "end" } })] }),
        ] })),
      ] }),
    ],
  }),
  program: story({
    steps: [
      step("candles", { title: "candlestick() · volume()", text: "Trading days only — no weekend gaps — with volume in a pane below." }),
      step("bars", { title: "ohlc()", text: "Each candle becomes its OHLC bar." }),
      step("averages", { title: "movingAverage()", text: "20- and 50-day averages, computed over the whole history." }),
      step("bollinger", { title: "bollinger()", text: "A 20-day band two standard deviations wide." }),
      step("indexed", { title: "indexed()", text: "Three tickers rebased: 100 is where each started." }),
      step("drawdown", { title: "drawdown()", text: "How far each sits below its running peak." }),
      step("sparklines", { title: "sparkline()", text: "A watchlist: each ticker's sixty days in a word-sized line." }),
    ],
  }),
});
