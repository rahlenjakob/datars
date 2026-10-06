// drawdown: how far a price sits below its running peak, as an area hanging from zero — then one
// line per ticker. Synthetic prices for fictional companies (a seeded random walk, mulberry32).
import { doc, data, e, group, op, story, step } from "@datars/sdk";
import { plot, drawdown } from "@datars/std";

let s = 23;
const rand = () => { s = (s + 0x6d2b79f5) | 0; let t = Math.imul(s ^ (s >>> 15), s | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 2 ** 32 - 0.5; };
const tickers: [string, number, number][] = [["HBRT", 84, 0.001], ["MOSS", 31, 0.0005], ["QUAY", 212, -0.001]];
const price = tickers.map((tk) => tk[1]);
const rows: { ticker: string; date: string; close: number }[] = [];
for (let t = Date.UTC(2025, 2, 3), n = 0; n < 80; t += 864e5) {
  if (new Date(t).getUTCDay() % 6 === 0) continue; // weekends: no trading
  const date = new Date(t).toISOString().slice(0, 10);
  tickers.forEach(([ticker, , drift], i) => { price[i] *= 1 + rand() * 0.06 + drift; rows.push({ ticker, date, close: Math.round(price[i] * 100) / 100 }); });
  n++;
}
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });

export default doc({
  title: "Drawdown",
  description: "One ticker's fall from its running peak as a shaded area, then three tickers as lines, each labelled with where it ends.",
  size: [640, 320],
  data: { prices: data.values(rows, { key: ["ticker", "date"] }) },
  tables: { hbrt: { from: "prices", ops: [op.filter(e('d.ticker == "HBRT"'))] } },
  scene: group({ key: "root", layout: { type: "stack", padding: [16, 20, 12, 12] }, children: [
    plot({ data: "hbrt", x: "date", y: "close", format: ".0%", title: "Below the running peak: HBRT", children: [drawdown()] }, at("default")),
    plot({ data: "prices", x: "date", y: "close", color: "ticker", format: ".0%", title: "Below the running peak", children: [drawdown()] }, at("tickers")),
  ] }),
  program: story({
    steps: [
      step("default", { title: "drawdown()", text: "0 at every new high; the area shows how deep and how long each fall was." }),
      step("tickers", { title: "color: \"ticker\"", text: "One line per series (areas would overlap), labelled with its current drawdown." }),
    ],
  }),
});
