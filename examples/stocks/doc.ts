// Stock charts: candles and volume on trading days (no weekend gaps), a zoom from six months to
// one, moving averages and Bollinger bands over the full history, then three tickers rebased to
// 100 and their drawdowns — with a watchlist of sparklines on top.
// Synthetic prices for fictional companies (a seeded random walk, generated here): not market data.
import { doc, data, e, group, repeat, signal, story, step, text, op } from "@datars/sdk";
import { plot, candlestick, volume, movingAverage, bollinger, indexed, drawdown, sparkline } from "@datars/std";

// ---- synthetic OHLCV ----------------------------------------------------------------------------

/** mulberry32: a small seeded generator (integer maths only, so the same numbers everywhere). */
function rng(seed: number) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
/** A standard normal draw, near enough: the sum of twelve uniforms, less six. */
const normal = (r: () => number) => { let s = 0; for (let i = 0; i < 12; i++) s += r(); return s - 6; };
const round2 = (x: number) => Math.round(x * 100) / 100;

// Weekdays from 1 October 2024, less a Nordic exchange's holidays.
const holidays = new Set(["2024-12-24", "2024-12-25", "2024-12-26", "2024-12-31", "2025-01-01", "2025-01-06", "2025-04-18", "2025-04-21", "2025-05-01", "2025-05-29", "2025-06-06", "2025-06-20", "2025-12-24", "2025-12-25", "2025-12-26", "2025-12-31"]);
const sessions: string[] = [];
for (let t = Date.UTC(2024, 9, 1); sessions.length < 320; t += 86_400_000) {
  const d = new Date(t);
  const iso = d.toISOString().slice(0, 10);
  if (d.getUTCDay() !== 0 && d.getUTCDay() !== 6 && !holidays.has(iso)) sessions.push(iso);
}

// Each ticker: a start price, daily drift and volatility, typical volume, and a stretch of bad
// news (a sell-off) so the drawdowns have something to show.
const tickers = [
  { id: "NORD", name: "Nordhavn", start: 142, drift: 0.0009, vol: 0.017, volume: 3.2e6, shock: [170, 196, -0.011], seed: 150 },
  { id: "VIKA", name: "Vikafjell", start: 58, drift: 0.0006, vol: 0.022, volume: 5.5e6, shock: [120, 150, -0.009], seed: 298 },
  { id: "ASKR", name: "Askr", start: 310, drift: 0.0004, vol: 0.009, volume: 0.9e6, shock: [230, 240, -0.006], seed: 366 },
] as const;

const cols: Record<string, (string | number)[]> = { ticker: [], date: [], open: [], high: [], low: [], close: [], volume: [] };
for (const tk of tickers) {
  const r = rng(tk.seed);
  let prev = tk.start;
  sessions.forEach((date, i) => {
    const [s0, s1, hit] = tk.shock;
    const ret = tk.drift + (i >= s0 && i < s1 ? hit : 0) + tk.vol * normal(r);
    const open = round2(prev * (1 + tk.vol * 0.35 * normal(r)));
    const close = round2(prev * (1 + ret));
    const high = round2(Math.max(open, close) * (1 + Math.abs(normal(r)) * tk.vol * 0.45));
    const low = round2(Math.min(open, close) * (1 - Math.abs(normal(r)) * tk.vol * 0.45));
    const vol = Math.round(tk.volume * Math.max(0.35, 1 + (Math.abs(ret) / tk.vol) * 0.45 + 0.22 * normal(r)));
    for (const [k, v] of Object.entries({ ticker: tk.id, date, open, high, low, close, volume: vol })) cols[k].push(v);
    prev = close;
  });
}

// ---- the document -------------------------------------------------------------------------------

const inRange = [op.window("rank", "date", "age", { partition: ["ticker"], order: "-date" }), op.filter(e("d.age <= sessions"))];
const showing = (v: string[]) => e(v.map((x) => `view == "${x}"`).join(" || "));

export default doc({
  id: "stocks",
  title: "Stock charts",
  description: "Candlesticks with volume on trading days, moving averages and Bollinger bands, then three tickers rebased to 100 and their drawdowns. Synthetic prices.",
  size: [760, 500],
  data: { prices: data.values(cols, { key: ["ticker", "date"] }) },
  keys: Object.fromEntries(tickers.map((t) => [t.id, { name: t.name }])),
  signals: { view: signal.str("price"), sessions: signal.num(126), peers: signal.num(2) },
  tables: {
    // One ticker's whole history (averages and bands compute over it), and the sessions on show.
    nord: { from: "prices", ops: [op.filter(e('d.ticker == "NORD"'))] },
    shown: { from: "nord", ops: inRange },
    // The comparison: two tickers, then three.
    peers: { from: "prices", ops: [op.filter(e('d.ticker == "NORD" || d.ticker == "VIKA" || (peers >= 3 && d.ticker == "ASKR")')), ...inRange] },
    watch: { from: "prices", ops: inRange },
  },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 12, padding: [14, 18, 10, 12] },
    children: [
      // A watchlist: last close, change over the range and a sparkline per ticker.
      group({
        key: "watchlist",
        size: { h: 40 },
        layout: { type: "grid", columns: 3, gap: 16 },
        children: [repeat({ groups: "watch", by: "ticker" }, group({
          layout: { type: "columns", gap: 8 },
          children: [
            group({ key: "quote", size: { w: "auto" }, children: [
              text(e("key.name(d.ticker)"), [0, 0], { key: "name", style: { size: "$size.label", weight: 600, ink: "$ink", baseline: "top" } }),
              text(e('format(group.last("close"), ",.2f") + "  " + format(group.last("close") / group.first("close") - 1, "+.1%")'), [0, 18], {
                key: "last", style: { size: "$size.label", ink: e('group.last("close") >= group.first("close") ? "$up" : "$down"'), baseline: "top" },
              }),
            ] }),
            sparkline({ data: "@group", x: "date", y: "close", name: "close" }),
          ],
        }))],
      }),
      group({
        key: "charts",
        children: [
          plot({
            data: "shown", x: "date", y: "close", xType: "band", format: ".0f", padding: 0.3,
            title: "Nordhavn (NORD), daily",
            children: [
              bollinger({ source: "nord" }, { when: showing(["indicators"]) }),
              candlestick(),
              movingAverage({ window: 20, source: "nord" }, { when: showing(["indicators"]) }),
              movingAverage({ window: 50, source: "nord", stroke: "$highlight" }, { when: showing(["indicators"]) }),
              volume(),
            ],
          }, { key: "chart", when: showing(["price", "indicators"]) }),
          plot({
            data: "peers", x: "date", y: "close", color: "ticker", xType: "band", format: ".0f",
            title: "Performance, rebased to 100 at the start",
            children: [indexed()],
          }, { key: "chart", when: showing(["indexed"]) }),
          plot({
            data: "peers", x: "date", y: "close", color: "ticker", xType: "band", format: ".0%",
            title: "Drawdown from each one's running peak",
            children: [drawdown()],
          }, { key: "chart", when: showing(["drawdown"]) }),
        ],
      }),
      text("Synthetic prices for fictional companies — not market data.", [0, 0], { key: "note", size: { h: 12 }, style: { size: "$size.small", ink: "$muted", baseline: "top" } }),
    ],
  }),
  program: story({
    steps: [
      step("candles", { set: { view: "price", sessions: 126 }, title: "Six months of Nordhavn: a candle per trading day, volume below" }),
      step("month", { set: { view: "price", sessions: 21 }, title: "The last month: every candle widens, the axis counts weeks" }),
      step("indicators", { set: { view: "indicators", sessions: 252 }, title: "A year, with its 20- and 50-day averages and Bollinger bands" }),
      step("indexed", { set: { view: "indexed", sessions: 252, peers: 2 }, title: "Rebased to 100: Nordhavn against Vikafjell" }),
      step("peers", { set: { view: "indexed", sessions: 252, peers: 3 }, title: "Add Askr, the steady one" }),
      step("drawdown", { set: { view: "drawdown", sessions: 252, peers: 3 }, title: "How far each fell from its peak" }),
    ],
  }),
});
