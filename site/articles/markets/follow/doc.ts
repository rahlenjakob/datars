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
/** The reader's text size, as std's controls read it: bigger text, bigger rows (never smaller). */
const T = "max(1, textScale)";
/** The header's figure: as large as the width allows (and a little larger with bigger text). */
const FIG = `min(40, max(28, box.w / 18)) * sqrt(${T})`;
/** A small text's size and its line, at the reader's text size. */
const sz = (n: number) => `${n} * ${T}`;
/** The header: the market and its venue (the market alone when both don't fit), the price, and
 * the day's change beside it (or under it, where it doesn't fit beside). */
const NAME_FULL = `${M("name")} + '  ·  ' + ${M("venue")}`;
const CS = `(${FIG} * 0.5)`;
const CHANGE = `format(${CHG}, '+$,.2f') + '  (' + format(${PCT}, '+.2f') + '%)'`;
const BESIDE = `(measure(${PRICE}, ${FIG}, 700) + 20 + ${CS} * 0.6 + measure(${CHANGE}, ${CS}, 600) <= box.w)`;
/** Where the change starts (x) and its baseline (y); the header's last baseline. */
const CX = `(${BESIDE} ? measure(${PRICE}, ${FIG}, 700) + 14 : 0)`;
const BY = `(${sz(20)} + ${FIG} + (${BESIDE} ? 0 : ${CS} * 1.45))`;
/** The amount's x and width (the share follows it). */
const AX = `(${CX} + ${CS} * 0.6 + 6)`;
const AW = `measure(format(${CHG}, '+$,.2f'), ${CS}, 600)`;
const BASE = `(${HAS} ? ${BY} : ${sz(20)} + ${FIG})`;
/** The footer: the status (the page's, else the recorded day's) and the source. */
const STATUS = `(status != '' ? status : 'Recorded ' + ${M("label")} + ' · Not live · Times are UTC')`;
const LINE = sz(16.5);
const SIDE = `(measure(${STATUS}, ${sz(12.5)}) + measure(source, ${sz(12)}) + 24 <= box.w)`;
const SL = `ceil(measure(${STATUS}, ${sz(12.5)}) / (box.w * 0.92))`;
const DL = `ceil(measure(source, ${sz(12)}) / (box.w * 0.92))`;

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
    meta: data.url("session.json", { rows: "meta", key: "sym", types: { x0: "num", x1: "num", ref: "num", day: "str" } }),
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
    // It moves in steps of a quarter percent of the reference: a session's first minutes (a range
    // of a few dollars) would otherwise rescale the axis with nearly every trade.
    ylim: { from: "m", ops: [
      op.derive("q", e("d.ref * 0.0025")),
      op.derive("lo", e(`floor((${HAS} ? min(d.ref, table.min('cur', 'p')) : d.ref) / d.q) * d.q`)),
      op.derive("hi", e(`max(d.lo + d.q, ceil((${HAS} ? max(d.ref, table.max('cur', 'p')) : d.ref) / d.q) * d.q)`)),
    ] },
  },
  // A minute's price glides to its next trade; a new market morphs in. The feed's pace caps it.
  motion: motion({ duration: 0.8, easing: "cubic-out" }),
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 6, padding: [14, 18, 10, 12] },
    children: [
      // The switcher: stocks and bitcoin with a key (their names where they fit, else tickers),
      // the recorded coins without.
      group({
        key: "pick",
        size: { h: e(`32 * ${T} + 2`) },
        children: [
          segmented({ signal: "sym", options: ["AAPL", "MSFT", "NVDA", "SPY", "BTC"], labels: ["Apple", "Microsoft", "Nvidia", "S&P 500", "Bitcoin"] },
            { key: "live-pick", when: e(`feed == "live" && box.w >= 520 * ${T}`), size: { w: e("min(box.w, 500 * max(1, textScale))"), h: e(`32 * ${T}`) } }),
          segmented({ signal: "sym", options: ["AAPL", "MSFT", "NVDA", "SPY", "BTC"], labels: ["AAPL", "MSFT", "NVDA", "SPY", "BTC"] },
            { key: "live-pick-short", when: e(`feed == "live" && box.w < 520 * ${T}`), size: { w: e("box.w"), h: e(`32 * ${T}`) } }),
          segmented({ signal: "sym", options: ["BTC", "ETH"], labels: ["Bitcoin", "Ether"] },
            { key: "coin-pick", when: e('feed != "live"'), size: { w: e(`min(box.w, 220 * ${T})`), h: e(`32 * ${T}`) } }),
        ],
      }),
      group({
        key: "head",
        size: { h: e(`${BASE} + ${sz(34)}`) },
        semantics: { role: "title", label: e(`${HAS} ? ${M("name")} + ', ' + ${PRICE} + ', ' + (${CHG} < 0 ? 'down ' : 'up ') + format(abs(${PCT}), '.2f') + ' percent. ' + ${M("refLabel")} + ' ' + format(${M("ref")}, ${M("fmt")}) : ${M("name")} + ': ' + empty`) },
        // One market's header: keyed by the market (a repeat over its one row), so a new market
        // crossfades in — its price is another number, not one to count to.
        children: [repeat("m", group({ children: [
              text(e(`measure(${NAME_FULL}, ${sz(13)}, 600) > box.w ? ${M("name")} : ${NAME_FULL}`), [0, e(sz(14))], { key: "name", style: { size: e(sz(13)), weight: 600, ink: "$ink-2" } }),
              text("", [0, e(`${sz(20)} + ${FIG}`)], {
                key: "price",
                number: { value: e(LAST), format: e(M("fmt")) },
                style: { size: e(FIG), weight: 700 },
                when: e(HAS),
              }),
              // The day's change: a triangle, the amount and the share — beside the price, or under it
              // when they don't fit side by side (a phone, a reader's bigger text).
              group({
                key: "change",
                when: e(HAS),
                children: [
                  shape(geom.path(e(`${CHG} < 0
                    ? 'M' + ${CX} + ' ' + (${BY} - ${CS} * 0.62) + ' L' + (${CX} + ${CS} * 0.6) + ' ' + (${BY} - ${CS} * 0.62) + ' L' + (${CX} + ${CS} * 0.3) + ' ' + (${BY} - ${CS} * 0.1) + ' Z'
                    : 'M' + ${CX} + ' ' + (${BY} - ${CS} * 0.1) + ' L' + (${CX} + ${CS} * 0.6) + ' ' + (${BY} - ${CS} * 0.1) + ' L' + (${CX} + ${CS} * 0.3) + ' ' + (${BY} - ${CS} * 0.62) + ' Z'`)),
                    { key: "arrow", fill: e(INK) }),
                  // Numbers that count to their new values (a changed string would crossfade, two
                  // figures over each other for a moment, every beat).
                  text("", [e(AX), e(BY)], { key: "amount", number: { value: e(CHG), format: "+$,.2f" }, style: { size: e(CS), weight: 600, ink: e(INK) } }),
                  text("(", [e(`${AX} + ${AW} + ${CS} * 0.6`), e(BY)], { key: "open", style: { size: e(CS), weight: 600, ink: e(INK) } }),
                  text("", [e(`${AX} + ${AW} + ${CS} * 0.6 + measure('(', ${CS}, 600)`), e(BY)], { key: "share", number: { value: e(PCT), format: "+.2f" }, style: { size: e(CS), weight: 600, ink: e(INK) } }),
                  text("%)", [e(`${AX} + ${AW} + ${CS} * 0.6 + measure('(' + format(${PCT}, '+.2f'), ${CS}, 600)`), e(BY)], { key: "close", style: { size: e(CS), weight: 600, ink: e(INK) } }),
                ],
              }),
              text(e(`${HAS} ? ${M("refLabel")} + ' ' + format(${M("ref")}, ${M("fmt")}) : empty`), [0, e(`${BASE} + ${sz(24)}`)], { key: "ref", style: { size: e(sz(13)), ink: "$muted" } }),
        ] }))],
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
            text(e(`${M("refLabel")} + ' ' + format(${M("ref")}, ${M("fmt")})`), [e("box.w"), e(`scale.y(${M("ref")}) - 5`)], { key: "ref-label", when: e(`box.w > 520 * ${T}`), halo: ["$paper", 3], style: { size: "$size.label", ink: "$muted", align: "end" } }),
            // The latest price: a dot, ringed.
            repeat("last", group({ key: "now", children: [
              shape(geom.circle({ cx: e("scale.x(d.t)"), cy: e("scale.y(d.p)"), r: 9 }), { key: "ring", fill: e(INK), opacity: 0.16 }),
              shape(geom.circle({ cx: e("scale.x(d.t)"), cy: e("scale.y(d.p)"), r: 4 }), { key: "dot", fill: e(INK), stroke: { paint: "$paper", width: 1.5 } }),
            ] })),
          ],
        }, { key: "chart", when: e(HAS) }),
          text(e("empty"), [e("box.w / 2"), e("box.h / 2")], { key: "waiting", when: e(`!(${HAS})`), style: { size: e(sz(15)), ink: "$muted", align: "middle", baseline: "middle", maxWidth: e("box.w - 40") } }),
        ],
      }),
      // What the feed is doing, and whose data it is: one line, or each on its own lines (as many
      // as it takes) where they don't fit side by side.
      group({
        key: "foot",
        size: { h: e(`${SIDE} ? ${LINE} + 2 : (${SL} + ${DL}) * ${LINE} + 2`) },
        children: [
          text(e(STATUS), [0, e(sz(13))], { key: "status", style: { size: e(sz(12.5)), ink: "$ink-2", maxWidth: e("box.w") } }),
          text(e("source"), [e(`${SIDE} ? box.w : 0`), e(`${SIDE} ? ${sz(13)} : ${sz(13)} + ${SL} * ${LINE}`)], { key: "source", style: { size: e(sz(12)), ink: "$muted", align: e(`${SIDE} ? "end" : "start"`), maxWidth: e("box.w") } }),
        ],
      }),
    ],
  }),
});
