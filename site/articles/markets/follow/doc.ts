// A market, followed live: one price a minute through the day's session, the open minute moving
// with every trade, the day's change in the theme's up and down colours.
//
// The data is live in the engine's own way (docs/06 §Live data): `prices` takes rows, not tables
// (`mode: "upsert"`, keyed by market, session and minute) — the page's feed (../live.js) pushes the trades
// that arrive, the open minute's vertex moves to its latest price and each new minute grows out of
// the line's end. Old sessions are retired by duration (`span`). The x axis is the whole session,
// fixed (09:30–16:00 in New York, or the UTC day for crypto), so the line grows across it instead
// of the axis sliding; the price axis is rounded to nice steps, so it moves only when a price
// crosses one.
//
// It opens on `session.json` (no key needed): the last full UTC day of bitcoin and ether on
// Binance, from Binance Vision (CC BY-NC-SA 4.0). Readers who paste a free key follow stocks.
// Times are wall-clock times in the market's own zone, written as if they were UTC (the page says
// which zone), so the axis reads 10:00 in New York as 10:00.
import { doc, data, e, geom, group, motion, op, repeat, shape, signal, text } from "@datars/sdk";
import { area, line, plot, rule, segmented } from "@datars/std";
import { markets } from "../theme";

/** The chosen market's row of `meta`, and its prices: `m` and `cur`. */
const M = (field: string) => `table.first('m', '${field}')`;
const LAST = "table.last('cur', 'p')";
const CHG = `(${LAST} - ${M("ref")})`;
const PCT = `(${CHG} / ${M("ref")} * 100)`;
const HAS = "table.count('cur') > 0";
const INK = `(${HAS} && ${CHG} < 0 ? "$down" : "$up")`;
/** The price as the header writes it (the price axis and the change use the same format). */
const PRICE = `format(${LAST}, ${M("fmt")})`;
/** The header's figure: as large as the width allows. */
const FIG = "min(40, max(28, box.w / 18))";

export default doc({
  id: "markets/follow",
  title: "A market, followed live",
  description: "One price a minute through the day's session for the market you choose, with its change on the day — live with a reader's own data key, otherwise the last recorded day of bitcoin and ether trading.",
  size: [960, 560],
  locale: "en-US",
  theme: markets,
  data: {
    prices: data.url("session.json", {
      rows: "prices",
      key: ["sym", "day", "t"],
      types: { t: "num", p: "num", sym: "str", day: "str" },
      // Rows arrive from the page's feed; a session older than four days leaves (a weekend's
      // last session stays).
      live: { mode: "upsert", span: 4 * 86400, on: "t" },
    }),
    meta: data.url("session.json", { rows: "meta", key: "sym", types: { x0: "num", x1: "num", ref: "num" } }),
  },
  signals: {
    sym: signal.str("BTC"),
    // "recorded" (the no-key day) or "live" (a reader's key): which markets the switcher offers.
    feed: signal.str("recorded"),
    // What the page knows and the engine can't: the market's state, the time zone, the source.
    // (Empty: the recorded day's own line, so the chart opens saying what it shows.)
    status: signal.str(""),
    source: signal.str("Data: Binance Vision (CC BY-NC-SA 4.0)"),
    empty: signal.str("Waiting for the first prices…"),
  },
  tables: {
    m: { from: "meta", ops: [op.filter(e("d.sym == sym"))] },
    // The chosen market's latest session only.
    cur: { from: "prices", ops: [op.filter(e(`d.sym == sym && d.day == ${M("day")}`)), op.sort("t")] },
    last: { from: "cur", ops: [op.aggregate([], { t: ["last", "t"], p: ["last", "p"] })] },
    // The price axis covers the session's prices and the reference (the previous close), so the
    // dashed line is always on it.
    ylim: { from: "m", ops: [
      op.derive("lo", e(`${HAS} ? min(d.ref, table.min('cur', 'p')) : d.ref * 0.995`)),
      op.derive("hi", e(`${HAS} ? max(d.ref, table.max('cur', 'p')) : d.ref * 1.005`)),
    ] },
  },
  // A minute's price glides to its next trade; a new market morphs in. The feed's pace caps it.
  motion: motion({ duration: 0.8, easing: "cubic-out" }),
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 6, padding: [14, 18, 10, 12] },
    children: [
      // The switcher: stocks and bitcoin with a key, the recorded coins without.
      group({
        key: "pick",
        size: { h: 34 },
        children: [
          segmented({ signal: "sym", options: ["AAPL", "MSFT", "NVDA", "SPY", "BTC"], labels: ["Apple", "Microsoft", "Nvidia", "S&P 500", "Bitcoin"] },
            { key: "live-pick", when: e('feed == "live"'), size: { w: e("min(box.w, 500)"), h: 32 } }),
          segmented({ signal: "sym", options: ["BTC", "ETH"], labels: ["Bitcoin", "Ether"] },
            { key: "coin-pick", when: e('feed != "live"'), size: { w: e("min(box.w, 220)"), h: 32 } }),
        ],
      }),
      group({
        key: "head",
        size: { h: e(`${FIG} + 60`) },
        semantics: { role: "title", label: e(`${HAS} ? ${M("name")} + ', ' + ${PRICE} + ', ' + (${CHG} < 0 ? 'down ' : 'up ') + format(abs(${PCT}), '.2f') + ' percent ' + ${M("refLabel")} : ${M("name")} + ': ' + empty`) },
        children: [
          text(e(`${M("name")} + '  ·  ' + ${M("venue")}`), [0, 14], { key: "name", style: { size: 13, weight: 600, ink: "$ink-2" } }),
          text("", [0, e(`20 + ${FIG}`)], {
            key: "price",
            number: { value: e(LAST), format: e(M("fmt")) },
            style: { size: e(FIG), weight: 700 },
            when: e(HAS),
          }),
          // The day's change, after the price: a triangle, the amount and the share.
          group({
            key: "change",
            when: e(HAS),
            children: [
              shape(geom.path(e(`'M' + (measure(${PRICE}, ${FIG}, 700) + 14) + ' ' + (${CHG} < 0 ? (${FIG} + 7) + ' L' + (measure(${PRICE}, ${FIG}, 700) + 26) + ' ' + (${FIG} + 7) + ' L' + (measure(${PRICE}, ${FIG}, 700) + 20) + ' ' + (${FIG} + 17) : (${FIG} + 17) + ' L' + (measure(${PRICE}, ${FIG}, 700) + 26) + ' ' + (${FIG} + 17) + ' L' + (measure(${PRICE}, ${FIG}, 700) + 20) + ' ' + (${FIG} + 7)) + ' Z'`)),
                { key: "arrow", fill: e(INK) }),
              text(e(`(${CHG} < 0 ? '−' : '+') + format(abs(${CHG}), ${M("fmt")}) + '  (' + (${CHG} < 0 ? '−' : '+') + format(abs(${PCT}), '.2f') + '%)'`),
                [e(`measure(${PRICE}, ${FIG}, 700) + 32`), e(`20 + ${FIG}`)],
                { key: "amount", style: { size: e(`${FIG} * 0.5`), weight: 600, ink: e(INK) } }),
            ],
          }),
          text(e(`${HAS} ? ${M("refLabel")} + ' ' + format(${M("ref")}, ${M("fmt")}) : empty`), [0, e(`${FIG} + 48`)], { key: "ref", style: { size: 13, ink: "$muted" } }),
        ],
      }),
      // The session's chart once the market has prices; until then, what the page is waiting for.
      group({
        key: "plot",
        children: [
        plot({
          data: "cur",
          x: "t",
          y: "p",
          xType: "time",
          xDomain: { data: "m", fields: ["x0", "x1"], exact: true },
          yDomain: { data: "ylim", fields: ["lo", "hi"] },
          zero: false,
          format: ",.2~f",
          prefix: "$",
          children: [
            area({ fill: e(INK), opacity: 0.08, curve: "linear" }),
            line({ curve: "linear", stroke: e(INK), width: 2 }),
            // The reference: the previous close (or midnight's price, for crypto).
            rule({ value: e(M("ref")), ink: "$muted" }),
            text(e(`${M("refLabel")} + ' ' + format(${M("ref")}, ${M("fmt")})`), [e("box.w"), e(`scale.y(${M("ref")}) - 5`)], { key: "ref-label", when: e("box.w > 520"), halo: ["$paper", 3], style: { size: 11, ink: "$muted", align: "end" } }),
            // The latest price: a dot, ringed.
            repeat("last", group({ key: "now", children: [
              shape(geom.circle({ cx: e("scale.x(d.t)"), cy: e("scale.y(d.p)"), r: 9 }), { key: "ring", fill: e(INK), opacity: 0.16 }),
              shape(geom.circle({ cx: e("scale.x(d.t)"), cy: e("scale.y(d.p)"), r: 4 }), { key: "dot", fill: e(INK), stroke: { paint: "$paper", width: 1.5 } }),
            ] })),
          ],
        }, { key: "chart", when: e(HAS) }),
          text(e("empty"), [e("box.w / 2"), e("box.h / 2")], { key: "waiting", when: e(`!(${HAS})`), style: { size: 15, ink: "$muted", align: "middle", baseline: "middle", maxWidth: e("box.w - 40") } }),
        ],
      }),
      // What the feed is doing, and whose data it is: one line, or two on a phone.
      group({
        key: "foot",
        size: { h: e("box.w < 620 ? 34 : 18") },
        children: [
          text(e(`status != '' ? status : 'Recorded ' + ${M("label")} + ' · Not live · Times are UTC'`), [0, 13], { key: "status", style: { size: 12.5, ink: "$ink-2" } }),
          text(e("source"), [e("box.w < 620 ? 0 : box.w"), e("box.w < 620 ? 31 : 13")], { key: "source", style: { size: 12, ink: "$muted", align: e('box.w < 620 ? "start" : "end"') } }),
        ],
      }),
    ],
  }),
});
