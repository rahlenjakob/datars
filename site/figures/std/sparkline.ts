// sparkline: a word-sized line per ticker in a watchlist, one per `@group` of a repeat. Synthetic
// prices for fictional companies: a seeded random walk (mulberry32), the same in every build.
import { doc, data, e, group, repeat, story, step, text } from "@datars/sdk";
import { sparkline } from "@datars/std";

let s = 4;
const rand = () => { s = (s + 0x6d2b79f5) | 0; let t = Math.imul(s ^ (s >>> 15), s | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 2 ** 32 - 0.5; };
const tickers: [string, number, number][] = [["HBRT", 84, 0.003], ["MOSS", 31, 0.0005], ["QUAY", 212, -0.003], ["FJRD", 57, 0.001], ["LYNX", 12, -0.001]];
const price = tickers.map((tk) => tk[1]);
const rows: { ticker: string; date: string; close: number }[] = [];
for (let t = Date.UTC(2025, 2, 3), n = 0; n < 60; t += 864e5) {
  if (new Date(t).getUTCDay() % 6 === 0) continue; // weekends: no trading
  const date = new Date(t).toISOString().slice(0, 10);
  tickers.forEach(([ticker, , drift], i) => { price[i] *= 1 + rand() * 0.04 + drift; rows.push({ ticker, date, close: Math.round(price[i] * 100) / 100 }); });
  n++;
}
const last = 'format(group.last("close"), ",.2f") + "  " + format(group.last("close") / group.first("close") - 1, "+.1%")';
const cell = { size: "$size.body", ink: "$ink", baseline: "middle" } as const;
// A watchlist: a row per ticker — its name, a sparkline of the last sixty days, the last close.
const watchlist = (state: string, opts: { fill?: boolean; trend?: boolean }) => group({ key: "chart", when: e(`state == "${state}"`),
  layout: { type: "grid", columns: 1, gap: 12 }, children: [repeat({ groups: "prices", by: "ticker" }, group({ layout: { type: "columns", gap: 24 }, children: [
    group({ key: "name", size: { w: 60 }, children: [text(e("d.ticker"), [0, e("box.h / 2")], { style: { ...cell, weight: 600 } })] }),
    sparkline({ data: "@group", x: "date", y: "close", name: "Close", ...opts }),
    group({ key: "last", size: { w: 140 }, children: [text(e(last), [e("box.w"), e("box.h / 2")], { style: { ...cell, ink: "$ink-2", align: "end" } })] }),
  ] }))] });

export default doc({
  title: "A watchlist of sparklines",
  description: "Five tickers' last sixty days as sparklines coloured by their trend, then without the soft area, then all in one ink.",
  size: [640, 320],
  data: { prices: data.values(rows, { key: ["ticker", "date"] }) },
  scene: group({ key: "root", layout: { type: "stack", padding: [16, 20, 12, 12] }, children: [
    watchlist("default", {}), watchlist("fill", { fill: false }), watchlist("trend", { fill: false, trend: false }),
  ] }),
  program: story({
    steps: [
      step("default", { title: "sparkline()", text: "Green when the line ends above where it started, red below; a dot marks the last value." }),
      step("fill", { title: "fill: false", text: "Just the line." }),
      step("trend", { title: "trend: false", text: "Every line in $mark, whichever way it went." }),
    ],
  }),
});
