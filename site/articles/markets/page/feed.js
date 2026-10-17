// The markets page's data, as pure functions (no DOM, no network): the markets it offers, New
// York's clock and the U.S. market's hours, and how a provider's message becomes the chart's rows
// (one price a minute per market and session). live.js runs them in the page; the tests replay a
// recorded feed through them (packages/web/test/markets.test.mjs).
export const MIN = 60_000, HOUR = 3_600_000, DAY = 86_400_000;

export const STOCKS = [
  { sym: "AAPL", name: "Apple", venue: "AAPL · Nasdaq", fh: "AAPL", td: "AAPL" },
  { sym: "MSFT", name: "Microsoft", venue: "MSFT · Nasdaq", fh: "MSFT", td: "MSFT" },
  { sym: "NVDA", name: "Nvidia", venue: "NVDA · Nasdaq", fh: "NVDA", td: "NVDA" },
  { sym: "SPY", name: "S&P 500", venue: "SPY, an S&P 500 index fund · NYSE Arca", fh: "SPY", td: "SPY" },
  { sym: "BTC", name: "Bitcoin", venue: "BTC in U.S. dollars · Binance", fh: "BINANCE:BTCUSDT", td: "BTC/USD", crypto: true },
];
export const bySym = Object.fromEntries(STOCKS.map((s) => [s.sym, s]));

// ---- time: New York's wall clock, the U.S. market's hours ----------------------------------------

const nyParts = new Intl.DateTimeFormat("en-US", { timeZone: "America/New_York", hourCycle: "h23", year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", second: "2-digit" });
/** A UTC instant as New York's wall-clock time, written as if it were UTC (the chart's convention:
 * its axis then reads New York's hours). */
export function nyWall(utc) {
  const p = Object.fromEntries(nyParts.formatToParts(new Date(utc)).map((x) => [x.type, x.value]));
  return Date.UTC(+p.year, +p.month - 1, +p.day, +p.hour % 24, +p.minute, +p.second) + (utc % 1000);
}
export const isoDay = (wall) => new Date(wall).toISOString().slice(0, 10);
export const atWall = (day, h, m = 0) => Date.parse(`${day}T00:00:00Z`) + h * HOUR + m * MIN;

// NYSE and Nasdaq: closed on these days, and closing at 1 p.m. on the early ones.
const HOLIDAYS = new Set(["2026-01-01", "2026-01-19", "2026-02-16", "2026-04-03", "2026-05-25", "2026-06-19", "2026-07-03", "2026-09-07", "2026-11-26", "2026-12-25",
  "2027-01-01", "2027-01-18", "2027-02-15", "2027-03-26", "2027-05-31", "2027-06-18", "2027-07-05", "2027-09-06", "2027-11-25", "2027-12-24"]);
const EARLY = new Set(["2026-11-27", "2026-12-24", "2027-11-26"]);
export const tradingDay = (day) => { const w = new Date(`${day}T12:00:00Z`).getUTCDay(); return w > 0 && w < 6 && !HOLIDAYS.has(day); };
export const closeHour = (day) => (EARLY.has(day) ? 13 : 16);

/** The U.S. market now: open or not, the session to show (today's, else the last one), the next open. */
export function usMarket(now = Date.now()) {
  const wall = nyWall(now);
  const today = isoDay(wall);
  const open = tradingDay(today) && wall >= atWall(today, 9, 30) && wall < atWall(today, closeHour(today));
  let session = today;
  if (!tradingDay(today) || wall < atWall(today, 9, 30)) {
    do session = isoDay(Date.parse(`${session}T12:00:00Z`) - DAY); while (!tradingDay(session));
  }
  let next = today;
  if (!tradingDay(today) || wall >= atWall(today, 9, 30)) {
    do next = isoDay(Date.parse(`${next}T12:00:00Z`) + DAY); while (!tradingDay(next));
  }
  return { open, session, next, wall };
}

// Dates as a newsroom writes them: Oct. 16, 2026; Mon., Oct. 19 (AP: March to July in full, Sept.).
const MONTHS = ["Jan.", "Feb.", "March", "April", "May", "June", "July", "Aug.", "Sept.", "Oct.", "Nov.", "Dec."];
const WEEKDAYS = ["Sun.", "Mon.", "Tue.", "Wed.", "Thu.", "Fri.", "Sat."];
export function fmtDay(day, weekday = false) {
  const d = new Date(`${day}T12:00:00Z`);
  const md = `${MONTHS[d.getUTCMonth()]} ${d.getUTCDate()}`;
  return weekday ? `${WEEKDAYS[d.getUTCDay()]}, ${md}` : `${md}, ${d.getUTCFullYear()}`;
}

/** One price a minute: the trade's minute on the market's wall clock. Regular hours only. */
export function minuteRow(s, utc, p) {
  const wall = s.crypto ? utc : nyWall(utc);
  const day = isoDay(wall);
  const t = Math.floor(wall / MIN) * MIN;
  // The regular session, and its closing auction (printed at 4:00:00 p.m.): the line ends on the
  // close, at the end of the axis.
  if (!s.crypto && (t < atWall(day, 9, 30) || t > atWall(day, closeHour(day)))) return null;
  return { sym: s.sym, day, t, p };
}

export function sessionMeta(s, day, ref, refLabel) {
  const crypto = !!s.crypto;
  return { sym: s.sym, name: s.name, venue: s.venue, day, fmt: "$,.2f", tz: crypto ? "UTC" : "ET",
    x0: crypto ? atWall(day, 0) : atWall(day, 9, 30), x1: crypto ? atWall(day, 24) : atWall(day, closeHour(day)),
    ref, refLabel: refLabel ?? (crypto ? "At midnight UTC" : "Previous close") };
}

/** A Finnhub WebSocket message as rows (`trade` messages; anything else is none), and each trade's
 * delay (now − its timestamp), newest last. */
export function finnhubRows(m, now = Date.now()) {
  if (m?.type !== "trade") return { rows: [], lag: undefined };
  const bySymbol = Object.fromEntries(STOCKS.map((s) => [s.fh, s]));
  const rows = [];
  let lag;
  for (const d of m.data ?? []) {
    const s = bySymbol[d.s];
    if (!s || !Number.isFinite(d.p) || !Number.isFinite(d.t)) continue;
    const r = minuteRow(s, d.t, d.p);
    if (r) rows.push({ ...r, utc: d.t });
    lag = Math.max(0, now - d.t);
  }
  return { rows, lag };
}

/** Twelve Data's minute bars (`time_series`, newest first, wall-clock times) as rows, oldest first. */
export function twelveRows(s, json) {
  return (json?.values ?? [])
    .map((v) => ({ sym: s.sym, t: Date.parse(v.datetime.replace(" ", "T") + "Z"), p: +v.close, open: +v.open }))
    .filter((r) => Number.isFinite(r.t) && Number.isFinite(r.p))
    .map((r) => ({ ...r, day: isoDay(r.t) }))
    .sort((a, b) => a.t - b.t);
}
