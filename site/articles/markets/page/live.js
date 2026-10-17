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
import { MIN, DAY, STOCKS, bySym, nyWall, isoDay, usMarket, fmtDay, minuteRow, sessionMeta, finnhubRows, twelveRows } from "./feed.js";

const figure = document.querySelector('figure[data-story="follow"]');
const slot = figure?.querySelector(".chart");
const view = () => slot?.querySelector("datars-view") ?? null;
const said = document.getElementById("markets-said");
const form = document.getElementById("keys");
const note = document.getElementById("keys-note");
const forget = document.getElementById("keys-forget");
const replayButton = document.getElementById("replay");

const STORE = "datars.markets.keys";
// A key a site build may carry (DATARS_PUBLIC_*): only for a licence that allows showing the data
// to every reader — the free plans don't (see README.md). Empty unless the build sets it.
const SITE_KEYS = { finnhub: "{{env:DATARS_PUBLIC_FINNHUB_KEY}}", twelvedata: "{{env:DATARS_PUBLIC_TWELVEDATA_KEY}}" };

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
const onPickers = [];

// ---- the screen reader's summary: polite, and not on every trade -----------------------------

const latest = new Map(); // sym → { p, t }
let announced = 0, announceTimer = 0;
let announceAt = Infinity;
/** Say it soon (a new market, a new state: in 1.5 s, once things have settled) or at the next
 * five-minute mark — whichever is sooner than what is already planned. */
function announceSoon(now = false) {
  const at = Date.now() + (now ? 1500 : Math.max(0, announced + 5 * MIN - Date.now()));
  if (at >= announceAt) return;
  clearTimeout(announceTimer);
  announceAt = at;
  announceTimer = setTimeout(() => { announceAt = Infinity; announce(); }, at - Date.now());
}
/** What the summary last spoke about (market and feed): a new one is said as soon as it has a price. */
let announcedFor = "";
function announce() {
  if (!said) return;
  const m = meta.get(state.sym), l = latest.get(state.sym);
  if (!m) return;
  announcedFor = l ? `${state.feed}:${state.sym}` : "";
  const money = (x) => x.toLocaleString("en-US", { style: "currency", currency: "USD" });
  let text = `${m.name}: `;
  if (l && Number.isFinite(m.ref)) {
    const d = l.p - m.ref, pct = (d / m.ref) * 100;
    text += `${money(l.p)}, ${d < 0 ? "down" : "up"} ${Math.abs(pct).toFixed(2)} percent. `;
  } else text += "no prices yet. ";
  said.textContent = text + state.status.replace(/ · /g, ". ") + ".";
  announced = Date.now();
}

// ---- the recorded day, and its replay ---------------------------------------------------------

let recorded = null;
async function recordedRows() {
  if (!recorded) {
    // Published next to the chart's manifest, as its live source's snapshot.
    const src = slot?.dataset.src?.replace(/\/c\/[^/]+$/, "/c/session.json");
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

/** The newest price per market, for the screen reader's summary. */
function noteLatest(rows) {
  for (const r of rows) {
    const l = latest.get(r.sym);
    if (!l || r.t >= l.t) latest.set(r.sym, { p: r.p, t: r.t });
  }
  // The chosen market's first price: say it (once; after that, every five minutes at most).
  if (rows.some((r) => r.sym === state.sym) && announcedFor !== `${state.feed}:${state.sym}`) announceSoon(true);
  return rows;
}

function twelveData(key) {
  const td = (path, q) => `https://api.twelvedata.com/${path}?${new URLSearchParams({ ...q, apikey: key })}`;
  const series = (s, size) => td("time_series", { symbol: s.td, interval: "1min", outputsize: String(size), ...(s.crypto ? { timezone: "UTC" } : {}) });
  const loaded = new Set();
  let cx = null;
  /** The session so far for one market (390 minutes, or the UTC day for bitcoin) and its reference. */
  async function history(s, signal) {
    loaded.add(s.sym); // asked once; a failure asks again (below)
    try {
      return await historyNow(s, signal);
    } catch (e) {
      loaded.delete(s.sym);
      throw e;
    }
  }
  async function historyNow(s, signal) {
    const [bars, quote] = await Promise.all([getJson(series(s, s.crypto ? 1440 : 390), signal), s.crypto ? null : getJson(td("quote", { symbol: s.td }), signal)]);
    const all = twelveRows(s, bars);
    if (!all.length) return [];
    const day = all.at(-1).day;
    const today = all.filter((r) => r.day === day);
    setMeta(sessionMeta(s, day, s.crypto ? today[0].open : +quote.previous_close));
    return noteLatest(today.map(({ sym, day, t, p }) => ({ sym, day, t, p })));
  }
  const f = liveFeed({ source: "prices", view, watch: slot, pace: 1000,
    onStatus: (st) => { health.twelvedata = st; describe(); },
    connect: (c) => {
      cx = c;
      return poll(async (signal) => {
        const s = bySym[state.sym] ?? STOCKS[0];
        if (!loaded.has(s.sym)) return history(s, signal);
        if (!s.crypto && !usMarket().open) return []; // nothing trades: nothing to ask for
        const rows = twelveRows(s, await getJson(series(s, 2), signal)).map(({ sym, day, t, p }) => ({ sym, day, t, p }));
        const newest = rows.at(-1);
        if (newest) {
          // A bar is stamped with its minute's start and arrives once that minute is over.
          lag.set("twelvedata", Math.max(0, (s.crypto ? Date.now() : nyWall(Date.now())) - (newest.t + MIN)));
          if (meta.get(s.sym)?.day !== newest.day) return history(s, signal); // a new session
        }
        return noteLatest(rows);
      }, 60_000)(c);
    } });
  // A market picked on the chart: its history now, not at the next poll.
  onPickers.push((sym) => {
    const s = bySym[sym];
    if (s && !loaded.has(sym) && cx) history(s, cx.signal).then((rows) => cx.push(rows), (e) => (e instanceof Fatal ? fatal("twelvedata", e) : cx.fail(e)));
  });
  return { feed: f };
}

function finnhub(key) {
  const fh = (path, q) => `https://finnhub.io/api/v1/${path}?${new URLSearchParams({ ...q, token: key })}`;
  // Each market's previous close (and its last price: a first point before any trade arrives).
  async function quotes(signal) {
    const out = [];
    for (const s of STOCKS) {
      const q = await getJson(fh("quote", { symbol: s.fh }), signal).catch((e) => { if (e instanceof Fatal) throw e; return null; });
      if (!q) continue;
      const at = q.t ? q.t * 1000 : Date.now();
      const session = s.crypto ? isoDay(at) : usMarket().session;
      // Twelve Data's history, when it is there, knows the session better.
      if (!health.twelvedata || !meta.has(s.sym)) setMeta(sessionMeta(s, session, q.pc, s.crypto ? "Yesterday's close (UTC)" : "Previous close"));
      const r = q.c ? minuteRow(s, at, q.c) : null;
      if (r) out.push(r);
    }
    return noteLatest(out);
  }
  const f = liveFeed({ source: "prices", view, watch: slot, pace: 1000,
    onStatus: (st) => { health.finnhub = st; describe(); },
    connect: (cx) => {
      quotes(cx.signal).then((rows) => cx.push(rows), (e) => (e instanceof Fatal ? fatal("finnhub", e) : undefined));
      return socket(`wss://ws.finnhub.io?token=${encodeURIComponent(key)}`, {
        open: (ws) => { for (const s of STOCKS) ws.send(JSON.stringify({ type: "subscribe", symbol: s.fh })); },
        message: (m) => {
          if (m?.type === "error") { fatal("finnhub", new Fatal(m.msg ?? "refused")); return null; }
          const { rows, lag: l } = finnhubRows(m);
          if (l !== undefined) lag.set("finnhub", l);
          return noteLatest(rows.map(({ sym, day, t, p }) => ({ sym, day, t, p })));
        },
      })(cx);
    } });
  return { feed: f };
}
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
    // Stocks are what a key is for: a reader who just came to follow them starts on Apple.
    set({ feed: "live", sym: state.feed === "live" && bySym[state.sym] ? state.sym : "AAPL", status: "Connecting…", source: "" });
    for (const [provider, make] of [["twelvedata", twelveData], ["finnhub", finnhub]]) if (keys[provider]) feeds.push({ provider, ...make(keys[provider]) });
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
