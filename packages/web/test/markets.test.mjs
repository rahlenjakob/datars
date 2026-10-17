// The markets article's live chart (site/articles/markets), replayed from a recorded feed — no
// network, no browser: four minutes of real bitcoin trades (Binance Vision's aggTrades, sent as
// Finnhub's WebSocket sends them) go through the page's own parsing (page/feed.js) and liveFeed's
// pacing, on a fake clock, into the published chart on the wasm engine. Then the scene and what a
// screen reader gets are checked: one point a minute, the open minute at its latest trade, the
// session axis fixed, the price axis moving only in round steps, every move over before the next
// beat, and the same frames on a second run.
import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const wasmDir = join(root, "packages/web/dist/wasm");
const cli = join(root, "target/release/datars");
const article = join(root, "site/articles/markets");
const ready = existsSync(join(wasmDir, "datars_core_bg.wasm")) && existsSync(cli) && existsSync(join(root, "packages/web/dist/live.js"));

const DAY = 86_400_000;

/** A clock whose timers run when the test moves time on (as in livefeed.test.mjs). */
function clock() {
  let now = 0, id = 0;
  const due = new Map();
  return {
    now: () => now,
    setTimeout: (f, ms) => { due.set(++id, { at: now + ms, f }); return id; },
    clearTimeout: (i) => due.delete(i),
    random: () => 0.5,
    to(t) {
      for (;;) {
        const next = [...due.entries()].filter(([, x]) => x.at <= t).sort((a, b) => a[1].at - b[1].at)[0];
        if (!next) break;
        due.delete(next[0]);
        now = next[1].at;
        next[1].f();
      }
      now = t;
    },
  };
}

async function run(m, bundle, feedFile) {
  const { liveFeed } = await import(join(root, "packages/web/dist/live.js"));
  const page = await import(join(article, "page/feed.js"));
  const view = new m.View(true, []);
  let need = view.open_manifest(readFileSync(join(bundle, "c/follow")));
  while (need.length) for (const h of need) need = view.provide_chunk(h, readFileSync(join(bundle, "chunks", h.replace(":", "_"))));
  view.set_size(720, 480, 1);
  // The recorded trades are replayed as if they were happening today (a day after they did): a
  // session of their own, after the recorded day the chart opens on.
  const feed = JSON.parse(readFileSync(feedFile, "utf8"));
  const shift = DAY;
  const start = feed.start + shift;
  const btc = page.bySym.BTC;
  const today = page.isoDay(start);
  const ref = 81_615.64; // the recorded day's last close: where "today" starts from
  const meta = JSON.parse(readFileSync(join(bundle, "c/session.json"), "utf8")).meta;
  view.provide_source("meta", new TextEncoder().encode(JSON.stringify([...meta.filter((x) => x.sym !== "BTC"), page.sessionMeta(btc, today, ref)])));
  view.set_signal("feed", JSON.stringify("live"));
  view.set_signal("sym", JSON.stringify("BTC"));
  view.set_signal("status", JSON.stringify("Live · Trades around the clock · Real-time · Times are UTC"));
  view.set_signal("source", JSON.stringify("Data: Finnhub (trades), with your key"));

  const t = clock();
  const ms = () => start + t.now(); // the replayed wall clock
  const provided = [];
  const fakeView = { dataset: { renderer: "cpu" }, provideData: (name, rows) => { provided.push({ at: t.now(), rows: rows.length }); view.provide_source(name, new TextEncoder().encode(JSON.stringify(rows))); } };
  let cx;
  const f = liveFeed({ source: "prices", view: fakeView, timers: t, pace: 1000, connect: (c) => { cx = c; c.ok(); } });
  const frames = [];
  const seen = { xTicks: new Set(), yTicks: [], stillMoving: 0, beats: 0 };
  const texts = () => JSON.parse(view.text_layer()).items ?? JSON.parse(view.text_layer());
  view.frame(0);
  let last = null;
  for (const { at, msg } of feed.messages) {
    t.to(at);
    const shifted = { ...msg, data: msg.data.map((d) => ({ ...d, t: d.t + shift })) };
    const { rows, lag } = page.finnhubRows(shifted, ms());
    assert.ok(lag !== undefined && lag < 2000, "trades arrive within their second");
    cx.push(rows.map(({ sym, day, t: tt, p }) => ({ sym, day, t: tt, p })));
    last = msg.data.at(-1).p;
    // Frames as a browser would draw them through the beat: 60 a second.
    for (let k = 0; k < 60; k++) view.frame(at + k * (1000 / 60));
    // Just before the next beat the move from this one is over.
    seen.beats++;
    if (view.transitioning()) seen.stillMoving++;
    const tl = texts();
    const words = tl.map((x) => x.text ?? x.t ?? "").filter(Boolean);
    seen.xTicks.add(words.filter((w) => /^(\d+ (AM|PM)|Oct \d+)$/.test(w)).join("|"));
    seen.yTicks.push(words.filter((w) => /^\$\d{2},\d{3}(\.\d+)?$/.test(w) && !w.includes(".")).join("|"));
  }
  t.to(feed.messages.at(-1).at + 2000);
  for (let k = 0; k < 120; k++) view.frame(feed.messages.at(-1).at + 1000 + k * 16);
  f.stop();
  const semantics = view.semantics();
  const summary = view.summary();
  const hash = view.pixel_hash(feed.messages.at(-1).at + 5000);
  view.free();
  return { provided, seen, semantics, summary, hash, last, feed, ref };
}

test("a recorded feed replays into the live chart: a point a minute, steady axes, moves within the beat", { skip: !ready && "build the wasm runtime, the CLI and packages/web first" }, async (t) => {
  const m = await import(join(wasmDir, "datars_core.js"));
  m.initSync({ module: readFileSync(join(wasmDir, "datars_core_bg.wasm")) });
  const out = mkdtempSync(join(tmpdir(), "datars-markets-"));
  try {
    execFileSync(cli, ["publish", join(article, "follow/doc.ts"), "--alias", "follow", "--to", out], { stdio: ["ignore", "ignore", "inherit"] });
    const feedFile = join(article, "data/feed.json");
    const a = await run(m, out, feedFile);

    // Paced: one version a second however many trades came (237 messages, 890 trades).
    assert.ok(a.provided.length <= a.feed.messages.length + 1, `${a.provided.length} versions`);
    for (let i = 1; i < a.provided.length; i++) assert.ok(a.provided[i].at - a.provided[i - 1].at >= 999, "at most one version a second");

    // One point a minute: 17:25 to 17:28 — the minutes the trades fell in.
    const minutes = new Set(a.feed.messages.flatMap((x) => x.msg.data.map((d) => Math.floor(d.t / 60000))));
    const flat = [];
    const walk = (n) => { flat.push(n); (n.children ?? []).forEach(walk); };
    a.semantics.forEach(walk);
    const points = flat.filter((n) => n.role === "datum");
    assert.equal(points.length, minutes.size, `${points.length} points: ${points.map((p) => p.label).join("; ")}`);
    assert.ok(points.every((p) => /Oct 17 5:2\d PM/.test(p.label)), "each says its minute, in the market's time zone (UTC)");
    // The open minute stands at its latest trade, and the header says it with the day's change.
    const price = a.last.toLocaleString("en-US", { style: "currency", currency: "USD" });
    const title = flat.find((n) => n.role === "title" && /Bitcoin, \$/.test(n.label ?? ""));
    assert.ok(title, flat.filter((n) => n.role === "title").map((n) => n.label).join(" / "));
    assert.ok(title.label.startsWith(`Bitcoin, ${price}, down `), title.label);
    const pct = ((a.ref - a.last) / a.ref * 100).toFixed(2);
    assert.ok(title.label.includes(`down ${pct} percent`), title.label);
    assert.ok(a.summary.includes(`${minutes.size} data points`), a.summary);

    // Once there were prices (before them the chart says it's waiting), the session axis never
    // moved, and the price axis moved only between round numbers, rarely.
    const xSets = [...a.seen.xTicks].filter(Boolean);
    assert.deepEqual(xSets, ["Oct 17|3 AM|6 AM|9 AM|12 PM|3 PM|6 PM|9 PM|Oct 18"], "the UTC day, fixed");
    const ySets = a.seen.yTicks.filter(Boolean).filter((s, i, all) => i === 0 || s !== all[i - 1]);
    assert.ok(ySets.length <= 4, `the price axis changed ${ySets.length - 1} times: ${ySets.join(" // ")}`);
    for (const s of ySets) for (const tick of s.split("|").filter(Boolean)) assert.match(tick, /0{2}$/, `round steps: ${tick}`);
    // Every move ended before the next beat.
    assert.equal(a.seen.stillMoving, 0, `${a.seen.stillMoving} of ${a.seen.beats} beats still moving`);

    t.diagnostic(`${a.feed.messages.length} messages → ${a.provided.length} versions; ${points.length} points; price axis ${ySets.map((x) => x.split("|").join(" ")).join(" → ")}; ${a.seen.stillMoving}/${a.seen.beats} beats still moving at the next; ${title.label}`);
    // A replay is exact: the same feed, the same last frame.
    const b = await run(m, out, feedFile);
    assert.equal(b.hash, a.hash);
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
});

test("the page's market clock: New York's hours, holidays, the session to show", { skip: !ready && "build first" }, async () => {
  const page = await import(join(article, "page/feed.js"));
  const at = (iso) => page.usMarket(Date.parse(iso));
  assert.deepEqual([at("2026-10-16T14:00:00Z").open, at("2026-10-16T14:00:00Z").session], [true, "2026-10-16"], "Friday 10 a.m. in New York");
  assert.deepEqual([at("2026-10-17T15:00:00Z").open, at("2026-10-17T15:00:00Z").session, at("2026-10-17T15:00:00Z").next], [false, "2026-10-16", "2026-10-19"], "Saturday: Friday's session, Monday next");
  assert.equal(at("2026-10-19T13:00:00Z").session, "2026-10-16", "Monday before the bell: still Friday's");
  assert.equal(at("2026-10-19T13:00:00Z").next, "2026-10-19");
  assert.equal(at("2026-11-26T15:00:00Z").open, false, "Thanksgiving");
  assert.equal(at("2026-11-27T18:30:00Z").open, false, "the day after closes at 1 p.m.");
  // Daylight saving: 9:30 in New York is 13:30 UTC in October, 14:30 in December.
  assert.equal(page.nyWall(Date.parse("2026-12-01T14:30:00Z")), Date.parse("2026-12-01T09:30:00Z"));
  // A trade becomes its minute's row; outside the regular session, none.
  assert.deepEqual(page.minuteRow(page.bySym.AAPL, Date.parse("2026-10-16T13:31:42Z"), 251.2), { sym: "AAPL", day: "2026-10-16", t: Date.parse("2026-10-16T09:31:00Z"), p: 251.2 });
  assert.equal(page.minuteRow(page.bySym.AAPL, Date.parse("2026-10-16T20:00:00Z"), 251.2)?.t, Date.parse("2026-10-16T16:00:00Z"), "the closing print ends the line");
  assert.equal(page.minuteRow(page.bySym.AAPL, Date.parse("2026-10-16T20:05:00Z"), 251.2), null, "after hours");
  assert.equal(page.fmtDay("2026-09-07"), "Sept. 7, 2026");
});
