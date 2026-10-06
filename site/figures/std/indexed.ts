// indexed: three tickers at very different prices, rebased to 100 at the first day on show, so they
// compare as performance. Synthetic prices for fictional companies (a seeded random walk).
import { doc, data, e, group, op, signal, story, step } from "@datars/sdk";
import { plot, indexed } from "@datars/std";

let s = 5;
const rand = () => { s = (s + 0x6d2b79f5) | 0; let t = Math.imul(s ^ (s >>> 15), s | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 2 ** 32 - 0.5; };
const tickers: [string, number, number][] = [["HBRT", 84, 0.003], ["MOSS", 31, 0.0005], ["QUAY", 212, -0.002]];
const price = tickers.map((tk) => tk[1]);
const rows: { ticker: string; date: string; close: number }[] = [];
for (let t = Date.UTC(2025, 2, 3), n = 0; n < 60; t += 864e5) {
  if (new Date(t).getUTCDay() % 6 === 0) continue; // weekends: no trading
  const date = new Date(t).toISOString().slice(0, 10);
  tickers.forEach(([ticker, , drift], i) => { price[i] *= 1 + rand() * 0.04 + drift; rows.push({ ticker, date, close: Math.round(price[i] * 100) / 100 }); });
  n++;
}
const chart = (percent: boolean) => plot({ data: "shown", x: "date", y: "close", color: "ticker", format: percent ? "+.0%" : ".0f",
  title: "Three tickers, rebased", children: [indexed({ percent })] }, { key: "chart", when: e(`state ${percent ? "!=" : "=="} "default"`) });

export default doc({
  title: "Indexed performance",
  description: "Three tickers rebased to 100 on the first day, then as percentage change, then over the last twenty days only, where every line starts from zero again.",
  size: [640, 320],
  data: { prices: data.values(rows, { key: ["ticker", "date"] }) },
  // The rows on show: each ticker's last `sessions` trading days.
  tables: { shown: { from: "prices", ops: [op.window("rank", "date", "age", { partition: ["ticker"], order: "-date" }), op.filter(e("d.age <= sessions"))] } },
  signals: { sessions: signal.num(60) },
  scene: group({ key: "root", layout: { type: "stack", padding: [16, 20, 12, 12] }, children: [chart(false), chart(true)] }),
  program: story({
    steps: [
      step("default", { set: { sessions: 60 }, title: "indexed()", text: "Every series divided by its first value: 100 is where each started." }),
      step("percent", { set: { sessions: 60 }, title: "percent: true", text: "Change from the start instead, with format \"+.0%\" on the plot." }),
      step("range", { set: { sessions: 20 }, title: "Twenty sessions", text: "Filter the rows and every line rebases to the new first day." }),
    ],
  }),
});
