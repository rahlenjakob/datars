// The oil market through the pandemic, one chart through the story's steps: West Texas Intermediate
// crude as weekly candles over the crude in storage at Cushing, Okla.; the collapse of spring 2020
// up close, down to the day the price went below zero; its daily closes inside 20-day Bollinger
// bands; WTI against Brent rebased to 100; how far WTI sat below its 2019 high, and for how long;
// and the climb back. Data: U.S. Energy Information Administration (public domain) — see ../SOURCES.md.
import { doc, data, e, group, motion, op, signal, step, story } from "@datars/sdk";
import { annotate, bollinger, candlestick, drawdown, indexed, line, movingAverage, plot, volume } from "@datars/std";
import { financial } from "../theme";
import src from "../data.json";

const { weeks, days } = src;
const showing = (view: string) => e(`view == "${view}"`);
// A point on a band axis: the middle of the day's (or week's) band.
const at = (date: string) => e(`scale.x('${date}') + scale.x.bandwidth() / 2`);

export default doc({
  id: "oil-2020/market",
  title: "The oil market, 2019–2021",
  description: "West Texas Intermediate crude, weekly, over the crude stored at Cushing, Okla.: the spring 2020 collapse below zero, its volatility, WTI against Brent, how far below its 2019 high it fell and the recovery.",
  size: [960, 600],
  theme: financial,
  data: {
    weeks: data.values(weeks, { key: "week" }),
    days: data.values(days, { key: ["series", "date"] }),
  },
  keys: {
    WTI: { name: "U.S. crude (WTI)" },
    Brent: { name: "Brent crude" },
  },
  // The step on show, the weeks (lo–hi) and the days (jlo–jhi) it covers, and a note on the chart.
  signals: { view: signal.str("candles"), lo: signal.num(0), hi: signal.num(60), jlo: signal.num(257), jhi: signal.num(514), note: signal.str("") },
  tables: {
    shown: { from: "weeks", ops: [op.filter(e("d.i >= lo && d.i <= hi"))] },
    wti: { from: "days", ops: [op.filter(e('d.series == "WTI"'))] },
    wtiShown: { from: "wti", ops: [op.filter(e("d.j >= jlo && d.j <= jhi"))] },
    pair: { from: "days", ops: [op.filter(e('(d.series == "WTI" || d.series == "Brent") && d.j >= jlo && d.j <= jhi'))] },
  },
  // Candles keep their week as they zoom; a new kind of chart crossfades in.
  motion: motion({ duration: 1.1, easing: "cubic-in-out" }),
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [14, 18, 10, 10] },
    children: [
      plot({
        data: "shown", x: "week", y: "close", xType: "band", format: "$.0f", padding: 0.25,
        title: "Price of U.S. crude oil, per barrel",
        subtitle: "Weekly prices, and the crude stored at Cushing",
        lower: { y: "cushing", height: 0.24, format: ".0f", label: "Barrels in storage, millions" },
        children: [
          candlestick(),
          volume({ volume: "cushing", colored: false, opacity: 0.5 }),
          movingAverage({ window: 10, source: "weeks", stroke: "$accent", label: "10-week average" }, { when: e('note == "average"') }),
          annotate({ text: "−$36.98 on April 20", x: at("2020-04-24"), y: e("scale.y(-36.98)"), dx: 30, dy: 0, width: 130 }, { key: "low", when: e('note == "low"') }),
          // The lower pane sits under the price area: its top is the area's height plus the gaps.
          annotate({ text: "65.4 million barrels on May 1", x: at("2020-05-01"), y: e("scale.y.max() + 22 + scale.lower(65.4)"), dx: 20, dy: -40, width: 130 }, { key: "full", when: e('note == "storage"') }),
        ],
      }, { key: "chart", when: showing("candles") }),
      plot({
        data: "wtiShown", x: "date", y: "close", xType: "band", format: "$.0f", padding: 0,
        title: "U.S. crude oil, daily, 2020",
        subtitle: "The price inside its 20-day band",
        children: [
          bollinger({ window: 20, source: "wti", label: "20-day band" }),
          line(),
          annotate({ text: "In May the band is $55 wide", x: at("2020-05-12"), y: e("scale.y(-10)"), dx: 30, dy: 30, width: 140 }, { key: "wide" }),
        ],
      }, { key: "chart", when: showing("bands") }),
      plot({
        data: "pair", x: "date", y: "close", color: "series", xType: "band", format: ".0f", padding: 0,
        title: "Two kinds of crude oil, 2020",
        subtitle: "Price on Jan. 2 = 100",
        children: [
          indexed(),
          annotate({ text: "April 20: U.S. crude at −60, Brent at 26", x: at("2020-04-20"), y: e("scale.y(-60.5)"), dx: 30, dy: 10, width: 150 }, { key: "gap" }),
        ],
      }, { key: "chart", when: showing("indexed") }),
      plot({
        data: "wtiShown", x: "date", y: "close", xType: "band", format: ".0%", padding: 0,
        title: "U.S. crude oil, below its 2019 high",
        subtitle: "The high: $66.24 a barrel, April 23, 2019",
        children: [
          drawdown(),
          annotate({ text: "April 20, 2020: 156% below", x: at("2020-04-20"), y: e("scale.y(-1.558)"), dx: 30, dy: 0, width: 120 }, { key: "deepest" }),
          annotate({ text: "Back at the high: May 2021", x: at("2021-05-17"), y: e("scale.y(0)"), dx: -30, dy: 40, width: 120 }, { key: "back" }),
        ],
      }, { key: "chart", when: showing("drawdown") }),
    ],
  }),
  program: story({
    steps: [
      step("calm", { set: { view: "candles", lo: 0, hi: 60, note: "" }, title: "An ordinary year" }),
      step("crash", { set: { view: "candles", lo: 52, hi: 77, note: "low" }, title: "The collapse" }),
      step("storage", { set: { view: "candles", lo: 52, hi: 77, note: "storage" }, title: "Nowhere to put it" }),
      step("bands", { set: { view: "bands", jlo: 257, jhi: 514, note: "" }, title: "The wildest year" }),
      step("brent", { set: { view: "indexed", jlo: 257, jhi: 514, note: "" }, title: "An American problem" }),
      step("drawdown", { set: { view: "drawdown", jlo: 0, jhi: 640, note: "" }, title: "Less than nothing" }),
      step("recovery", { set: { view: "candles", lo: 0, hi: 130, note: "average" }, title: "The way back" }),
    ],
  }),
});
