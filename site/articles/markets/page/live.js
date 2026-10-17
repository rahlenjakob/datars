// The markets page's feed: what puts live prices into the chart (../follow/doc.ts).
//
// The chart opens on a recorded day of bitcoin and ether (Binance Vision, no key needed). A reader
// who pastes a free key follows stocks and bitcoin live, in their own browser:
//
// - Finnhub: a WebSocket of trades (real-time on its free plan) and a quote per market for the
//   previous close. Its free plan has no minute history, so the line starts when the page opens.
// - Twelve Data: the session's minute bars (the whole day so far) and a poll of the chosen market
//   every minute — 1 request a minute, well inside the free plan's 8 a minute and 800 a day.
//
// Either key, or both (history from Twelve Data, trades from Finnhub). Keys live in this browser's
// localStorage only and go only to their provider. Rows reach the chart through `liveFeed`
// (@datars/web): paced to one version a second, backed off on errors, paused while the tab is
// hidden or the chart is off the screen. The chart's source merges them (upsert by market, day and
// minute), so the open minute moves and a new minute grows out of the line.
import { liveFeed, socket, poll } from "../../runtime/datars.js";

const figure = document.querySelector('figure[data-story="follow"]');
const slot = figure?.querySelector(".chart");
const view = () => slot?.querySelector("datars-view") ?? null;
const said = document.getElementById("markets-said");
const form = document.getElementById("keys");
const note = document.getElementById("keys-note");
const forget = document.getElementById("keys-forget");
const replayButton = document.getElementById("replay");

const MIN = 60_000, HOUR = 3_600_000, DAY = 86_400_000;
const STORE = "datars.markets.keys";
// A key a site build may carry (DATARS_PUBLIC_*): only for a licence that allows showing the data
// to every reader — the free plans don't (see README.md). Empty unless the build sets it.
const SITE_KEYS = { finnhub: "{{env:DATARS_PUBLIC_FINNHUB_KEY}}", twelvedata: "{{env:DATARS_PUBLIC_TWELVEDATA_KEY}}" };

const STOCKS = [
  { sym: "AAPL", name: "Apple", venue: "AAPL · Nasdaq", fh: "AAPL", td: "AAPL" },
  { sym: "MSFT", name: "Microsoft", venue: "MSFT · Nasdaq", fh: "MSFT", td: "MSFT" },
  { sym: "NVDA", name: "Nvidia", venue: "NVDA · Nasdaq", fh: "NVDA", td: "NVDA" },
  { sym: "SPY", name: "S&P 500", venue: "SPY, an S&P 500 index fund · NYSE Arca", fh: "SPY", td: "SPY" },
  { sym: "BTC", name: "Bitcoin", venue: "BTC in U.S. dollars · Binance", fh: "BINANCE:BTCUSDT", td: "BTC/USD", crypto: true },
];
const bySym = Object.fromEntries(STOCKS.map((s) => [s.sym, s]));

// ---- time: New York's wall clock, the U.S. market's hours ----------------------------------------

const nyParts = new Intl.DateTimeFormat("en-US", { timeZone: "America/New_York", hourCycle: "h23", year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", second: "2-digit" });
/** A UTC instant as New York's wall-clock time, written as if it were UTC (the chart's convention:
 * its axis then reads New York's hours). */
function nyWall(utc) {
  const p = Object.fromEntries(nyParts.formatToParts(new Date(utc)).map((x) => [x.type, x.value]));
  return Date.UTC(+p.year, +p.month - 1, +p.day, +p.hour % 24, +p.minute, +p.second) + (utc % 1000);
}
const isoDay = (wall) => new Date(wall).toISOString().slice(0, 10);
const atWall = (day, h, m = 0) => Date.parse(`${day}T00:00:00Z`) + h * HOUR + m * MIN;

// NYSE and Nasdaq: closed on these days, and closing at 1 p.m. on the early ones.
const HOLIDAYS = new Set(["2026-01-01", "2026-01-19", "2026-02-16", "2026-04-03", "2026-05-25", "2026-06-19", "2026-07-03", "2026-09-07", "2026-11-26", "2026-12-25",
  "2027-01-01", "2027-01-18", "2027-02-15", "2027-03-26", "2027-05-31", "2027-06-18", "2027-07-05", "2027-09-06", "2027-11-25", "2027-12-24"]);
const EARLY = new Set(["2026-11-27", "2026-12-24", "2027-11-26"]);
const tradingDay = (day) => { const w = new Date(`${day}T12:00:00Z`).getUTCDay(); return w > 0 && w < 6 && !HOLIDAYS.has(day); };
const closeHour = (day) => (EARLY.has(day) ? 13 : 16);

/** The U.S. market now: open or not, the session to show (today's, else the last one), the next open. */
function usMarket(now = Date.now()) {
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
function fmtDay(day, weekday = false) {
  const d = new Date(`${day}T12:00:00Z`);
  const md = `${MONTHS[d.getUTCMonth()]} ${d.getUTCDate()}`;
  return weekday ? `${WEEKDAYS[d.getUTCDay()]}, ${md}` : `${md}, ${d.getUTCFullYear()}`;
}

// ---- keys --------------------------------------------------------------------------------------

function loadKeys() {
  let saved = {};
  try { saved = JSON.parse(localStorage.getItem(STORE) ?? "{}") ?? {}; } catch { /* private mode: no saved keys */ }
  const pick = (k) => (saved[k] || (SITE_KEYS[k].startsWith("{{") ? "" : SITE_KEYS[k]) || "").trim();
  return { finnhub: pick("finnhub"), twelvedata: pick("twelvedata"), mine: Object.keys(saved).filter((k) => saved[k]) };
}
function saveKey(provider, key) {
  let saved = {};
  try { saved = JSON.parse(localStorage.getItem(STORE) ?? "{}") ?? {}; } catch { /* nothing saved */ }
  if (key) saved[provider] = key; else delete saved[provider];
  try { localStorage.setItem(STORE, JSON.stringify(saved)); } catch { /* the key lasts this visit only */ }
}

// ---- what the chart is told ----------------------------------------------------------------------

/** Every market's row of `meta` (the chart's snapshot source: all of them, every time). */
const meta = new Map();
let state = { feed: "recorded", sym: "BTC", status: "", source: "" };

function tell(v = view()) {
  if (!v) return;
  if (meta.size) v.provideData("meta", [...meta.values()]);
  for (const k of ["feed", "sym", "status", "source"]) if (state[k]) v.setSignal(k, state[k]);
}
function set(patch) {
  const changed = Object.keys(patch).filter((k) => patch[k] !== state[k]);
  if (!changed.length) return;
  state = { ...state, ...patch };
  const v = view();
  if (v) for (const k of changed) v.setSignal(k, state[k]);
  announceSoon(changed.includes("sym") || changed.includes("feed"));
}
function setMeta(row) {
  const old = meta.get(row.sym);
  meta.set(row.sym, { ...old, ...row });
  if (JSON.stringify(old) !== JSON.stringify(meta.get(row.sym))) view()?.provideData("meta", [...meta.values()]);
}

// A chart mounted again (the page frees charts far from the screen): it hears everything again.
if (slot) new MutationObserver(() => tell()).observe(slot, { childList: true });

// The reader picks a market on the chart's own switcher.
slot?.addEventListener("signal", (e) => {
  const s = e.detail?.signals?.sym;
  if (typeof s === "string" && s !== state.sym) {
    state.sym = s;
    onPick?.(s);
    announceSoon(true);
  }
}, true);
let onPick = null;

// ---- the screen reader's summary: polite, and not on every trade -----------------------------

const latest = new Map(); // sym → { p, t }
let announced = 0, announceTimer = 0;
function announceSoon(now = false) {
  clearTimeout(announceTimer);
  const wait = now ? 1500 : Math.max(0, announced + 5 * MIN - Date.now());
  announceTimer = setTimeout(announce, wait);
}
function announce() {
  if (!said) return;
  const m = meta.get(state.sym), l = latest.get(state.sym);
  if (!m) return;
  const money = (x) => x.toLocaleString("en-US", { style: "currency", currency: "USD" });
  let text = `${m.name}: `;
  if (l && Number.isFinite(m.ref)) {
    const d = l.p - m.ref, pct = (d / m.ref) * 100;
    text += `${money(l.p)}, ${d < 0 ? "down" : "up"} ${Math.abs(pct).toFixed(2)} percent. `;
  } else text += "no prices yet. ";
  said.textContent = text + state.status.replace(/·/g, ".");
  announced = Date.now();
}

// ---- the recorded day, and its replay ---------------------------------------------------------

let recorded = null;
async function recordedRows() {
  if (!recorded) {
    const src = slot?.dataset.src?.replace(/\/c\/[^/]+$/, "/session.json");
    recorded = src ? fetch(src).then((r) => (r.ok ? r.json() : Promise.reject(new Error(r.status)))) : Promise.reject(new Error("no chart"));
  }
  return recorded;
}

function showRecorded(data) {
  for (const m of data.meta) setMeta(m);
  set({ feed: "recorded", sym: state.sym === "ETH" ? "ETH" : "BTC", status: `Recorded ${data.meta[0].label} · Not live · Times are UTC`, source: "Data: Binance Vision (CC BY-NC-SA 4.0)" });
}

/** Play the recorded day again through the same path live prices take: its minutes arrive as rows
 * (thirty a second), and the line grows across the day. A replay is a session of its own (its own
 * `day`), so the recorded one is untouched and comes back when it ends. */
let replaying = null;
async function replay() {
  if (replaying) { replaying.stop(); return; }
  const data = await recordedRows();
  const sym = state.feed === "recorded" ? state.sym : "BTC";
  const run = `replay-${Date.now()}`;
  const rows = data.prices.filter((r) => r.sym === sym).map((r) => ({ ...r, day: run }));
  const m = data.meta.find((x) => x.sym === sym);
  if (state.feed !== "recorded") showRecorded(data);
  set({ sym, status: `Replay of ${fmtDay(data.recorded)}, 30 minutes a second · Not live · Times are UTC` });
  setMeta({ ...m, day: run });
  let i = 0, timer = 0;
  const feed = liveFeed({ source: "prices", view, watch: slot, pace: 250, linger: 600_000,
    connect: (cx) => {
      cx.ok();
      const tick = () => { cx.push(rows.slice(i, i + 8)); i += 8; if (i < rows.length) timer = setTimeout(tick, 250); else setTimeout(() => replaying?.stop(), 1500); };
      timer = setTimeout(tick, 400);
      return () => clearTimeout(timer);
    } });
  replaying = { stop() {
    feed.stop();
    replaying = null;
    replayButton?.setAttribute("aria-pressed", "false");
    if (replayButton) replayButton.textContent = "Replay the day";
    if (state.feed === "recorded") { setMeta(m); showRecorded(data); }
  } };
  replayButton?.setAttribute("aria-pressed", "true");
  if (replayButton) replayButton.textContent = "Stop the replay";
}
replayButton?.addEventListener("click", () => replay().catch((e) => console.warn("markets: replay", e)));

// ---- live: Twelve Data (minute bars) and Finnhub (trades) ----------------------------------------

/** One price a minute: the trade's minute on the market's wall clock. Regular hours only. */
function minuteRow(s, utc, p) {
  const wall = s.crypto ? utc : nyWall(utc);
  const day = isoDay(wall);
  const t = Math.floor(wall / MIN) * MIN;
  if (!s.crypto && (t < atWall(day, 9, 30) || t >= atWall(day, closeHour(day)))) return null;
  const l = latest.get(s.sym);
  if (!l || utc >= l.utc) latest.set(s.sym, { p, t, utc });
  return { sym: s.sym, day, t, p };
}

function sessionMeta(s, day, ref, refLabel) {
  const crypto = !!s.crypto;
  return { sym: s.sym, name: s.name, venue: s.venue, day, fmt: "$,.2f", tz: crypto ? "UTC" : "ET",
    x0: crypto ? atWall(day, 0) : atWall(day, 9, 30), x1: crypto ? atWall(day, 24) : atWall(day, closeHour(day)),
    ref, refLabel: refLabel ?? (crypto ? "At midnight UTC" : "Previous close") };
}

class Fatal extends Error {}
async function getJson(url, signal) {
  const r = await fetch(url, { signal, cache: "no-store" });
  const j = await r.json().catch(() => null);
  if (r.status === 401 || r.status === 403 || j?.code === 401 || j?.code === 403 || /api key/i.test(j?.error ?? "")) throw new Fatal(j?.message ?? j?.error ?? `refused (${r.status})`);
  if (!r.ok || j?.status === "error") throw new Error(j?.message ?? `${r.status}`);
  return j;
}

const feeds = [];
const health = { finnhub: null, twelvedata: null }; // what each feed last said
const lag = new Map(); // provider → ms between a price's time and its arrival

function twelveData(key) {
  const td = (path, q) => `https://api.twelvedata.com/${path}?${new URLSearchParams({ ...q, apikey: key })}`;
  const loaded = new Set();
  /** The session so far for one market (390 minutes, or the UTC day for bitcoin) and its reference. */
  async function history(s, signal) {
    const crypto = !!s.crypto;
    const [bars, quote] = await Promise.all([
      getJson(td("time_series", { symbol: s.td, interval: "1min", outputsize: crypto ? "1440" : "390", ...(crypto ? { timezone: "UTC" } : {}) }), signal),
      crypto ? null : getJson(td("quote", { symbol: s.td }), signal),
    ]);
    const rows = (bars.values ?? []).map((v) => ({ wall: Date.parse(v.datetime.replace(" ", "T") + "Z"), open: +v.open, close: +v.close })).filter((v) => Number.isFinite(v.wall));
    if (!rows.length) return [];
    const day = isoDay(Math.max(...rows.map((r) => r.wall)));
    const today = rows.filter((r) => isoDay(r.wall) === day).sort((a, b) => a.wall - b.wall);
    const ref = crypto ? today[0].open : +quote.previous_close;
    setMeta(sessionMeta(s, day, ref));
    const out = today.map((r) => ({ sym: s.sym, day, t: r.wall, p: r.close }));
    const last = today.at(-1);
    latest.set(s.sym, { p: last.close, t: last.wall, utc: Date.now() });
    return out;
  }
  const f = liveFeed({ source: "prices", view, watch: slot, pace: 1000,
    onStatus: (st) => { health.twelvedata = st; describe(); },
    connect: poll(async (signal) => {
      const s = bySym[state.sym] ?? STOCKS[0];
      if (!loaded.has(s.sym)) { const rows = await history(s, signal); loaded.add(s.sym); return rows; }
      if (!s.crypto && !usMarket().open) return []; // nothing trades: nothing to ask for
      const bars = await getJson(td("time_series", { symbol: s.td, interval: "1min", outputsize: "2", ...(s.crypto ? { timezone: "UTC" } : {}) }), signal);
      const rows = (bars.values ?? []).map((v) => ({ sym: s.sym, t: Date.parse(v.datetime.replace(" ", "T") + "Z"), p: +v.close }));
      for (const r of rows) r.day = isoDay(r.t);
      const newest = rows.reduce((a, r) => (r.t > a.t ? r : a), rows[0]);
      if (newest) {
        const nowWall = s.crypto ? Date.now() : nyWall(Date.now());
        lag.set("twelvedata", Math.max(0, nowWall - (newest.t + MIN)));
        latest.set(s.sym, { p: newest.p, t: newest.t, utc: Date.now() });
        if (meta.get(s.sym)?.day !== newest.day) loaded.delete(s.sym); // a new session: its history
      }
      return rows;
    }, 60_000) });
  onPickers.push(() => f.flush()); // a new market's history comes with the next poll
  return { feed: f, refresh: () => { /* next poll fetches the chosen market */ } };
}

function finnhub(key) {
  const fh = (path, q) => `https://finnhub.io/api/v1/${path}?${new URLSearchParams({ ...q, token: key })}`;
  const bySymbol = Object.fromEntries(STOCKS.map((s) => [s.fh, s]));
  // Each market's previous close (and a first price, where no history arrives).
  const quotes = (async () => {
    const out = [];
    for (const s of STOCKS) {
      try {
        const q = await getJson(fh("quote", { symbol: s.fh }));
        const at = q.t ? q.t * 1000 : Date.now();
        const wall = s.crypto ? at : nyWall(at);
        const session = s.crypto ? isoDay(wall) : usMarket().session;
        if (!meta.has(s.sym) || meta.get(s.sym).day !== session || !health.twelvedata) setMeta(sessionMeta(s, session, q.pc, s.crypto ? "Yesterday's close (UTC)" : "Previous close"));
        const r = q.c ? minuteRow(s, at, q.c) : null;
        if (r) out.push(r);
      } catch (e) {
        if (e instanceof Fatal) throw e;
      }
    }
    return out;
  })();
  const f = liveFeed({ source: "prices", view, watch: slot, pace: 1000,
    onStatus: (st) => { health.finnhub = st; describe(); },
    connect: (cx) => {
      quotes.then((rows) => cx.push(rows), (e) => { cx.fail(e); fatal("finnhub", e); });
      return socket(`wss://ws.finnhub.io?token=${encodeURIComponent(key)}`, {
        open: (ws) => { for (const s of STOCKS) ws.send(JSON.stringify({ type: "subscribe", symbol: s.fh })); },
        message: (m) => {
          if (m?.type === "error") { fatal("finnhub", new Fatal(m.msg ?? "refused")); return null; }
          if (m?.type !== "trade") return null;
          const rows = [];
          for (const d of m.data ?? []) {
            const s = bySymbol[d.s];
            const r = s && minuteRow(s, d.t, d.p);
            if (r) rows.push(r);
            if (s) lag.set("finnhub", Math.max(0, Date.now() - d.t));
          }
          return rows;
        },
      })(cx);
    } });
  return { feed: f };
}
const onPickers = [];
onPick = (sym) => { for (const p of onPickers) p(sym); describe(); };

function fatal(provider, e) {
  const i = feeds.findIndex((f) => f.provider === provider);
  if (i >= 0) { feeds[i].feed.stop(); feeds.splice(i, 1); }
  if (note) note.textContent = `${provider === "finnhub" ? "Finnhub" : "Twelve Data"} refused the key: ${e.message}. Check it, or forget it to see the recorded day.`;
  if (!feeds.length) start({ finnhub: "", twelvedata: "" });
}

/** The status line and the source line, from the market and the feeds. */
function describe() {
  if (state.feed !== "live") return;
  const s = bySym[state.sym] ?? STOCKS[0];
  const providers = feeds.map((f) => f.provider);
  const states = providers.map((p) => health[p]?.state).filter(Boolean);
  const name = (p) => (p === "finnhub" ? "Finnhub" : "Twelve Data");
  const delay = () => {
    const l = providers.map((p) => lag.get(p)).filter((x) => x !== undefined);
    if (!l.length) return providers.includes("finnhub") ? "Real-time" : "Every minute";
    const m = Math.min(...l);
    return m < 30_000 ? "Real-time" : `Delayed about ${Math.round(m / MIN)} min`;
  };
  let status;
  if (states.includes("live")) {
    if (s.crypto) status = `Live · Trades around the clock · ${delay()} · Times are UTC`;
    else {
      const mk = usMarket();
      status = mk.open ? `Live · Market open · ${delay()} · Times are New York (ET)` : `Market closed · Last session ${fmtDay(meta.get(s.sym)?.day ?? mk.session)} · Opens ${fmtDay(mk.next, true)}, 9:30 a.m. ET`;
    }
  } else if (states.includes("retrying")) status = "Connection lost · Trying again";
  else if (states.includes("paused")) status = "Paused while the chart is out of sight";
  else status = "Connecting…";
  const sources = providers.map((p) => `${name(p)}${p === "finnhub" ? " (trades)" : " (minute bars)"}`).join(" and ");
  set({ status, source: `Data: ${sources}, with your key` });
}
setInterval(describe, 30_000); // the market opens and closes while the page is open

// ---- start ------------------------------------------------------------------------------------

function stopAll() { while (feeds.length) feeds.pop().feed.stop(); }

async function start(keys) {
  stopAll();
  if (keys.finnhub || keys.twelvedata) {
    for (const [provider, make] of [["twelvedata", twelveData], ["finnhub", finnhub]]) if (keys[provider]) feeds.push({ provider, ...make(keys[provider]) });
    set({ feed: "live", sym: bySym[state.sym] ? state.sym : "AAPL", status: "Connecting…", source: "" });
    describe();
  } else {
    try { showRecorded(await recordedRows()); } catch (e) { console.warn("markets: recorded day", e); }
  }
  syncForm(keys);
}

function syncForm(keys) {
  if (!form) return;
  const mine = keys.mine ?? [];
  if (forget) forget.style.visibility = mine.length ? "visible" : "hidden";
  if (note && !note.dataset.error) {
    note.textContent = mine.length
      ? `Following live with your ${mine.map((k) => (k === "finnhub" ? "Finnhub" : "Twelve Data")).join(" and ")} key. It is kept in this browser only.`
      : "Without a key, the chart shows bitcoin and ether on the last full day recorded.";
  }
}

form?.addEventListener("submit", (e) => {
  e.preventDefault();
  const fd = new FormData(form);
  const provider = String(fd.get("provider"));
  const key = String(fd.get("key") ?? "").trim();
  if (!key) return;
  saveKey(provider, key);
  form.querySelector("input[name=key]").value = "";
  start(loadKeys());
});
forget?.addEventListener("click", () => {
  saveKey("finnhub", "");
  saveKey("twelvedata", "");
  start(loadKeys());
});

start(loadKeys());
