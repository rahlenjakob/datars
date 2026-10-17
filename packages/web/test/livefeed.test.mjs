// Live feeds (src/live.ts): rows reach the chart at the feed's pace, failures back off and recover,
// a hidden tab or an off-screen chart closes the connection, and a remounted chart gets every row.
// No browser, no network: a fake clock, a fake view and a fake socket.
import { test } from "node:test";
import assert from "node:assert/strict";
import { liveFeed, socket } from "../dist/live.js";

/** A clock whose timers run when the test moves time on. */
function clock() {
  let now = 0;
  let id = 0;
  const due = new Map();
  return {
    now: () => now,
    setTimeout: (f, ms) => { due.set(++id, { at: now + ms, f }); return id; },
    clearTimeout: (i) => due.delete(i),
    random: () => 0.5, // no jitter: exact delays
    advance(ms) {
      const end = now + ms;
      for (;;) {
        const next = [...due.entries()].filter(([, t]) => t.at <= end).sort((a, b) => a[1].at - b[1].at)[0];
        if (!next) break;
        due.delete(next[0]);
        now = next[1].at;
        next[1].f();
      }
      now = end;
    },
  };
}

/** A running chart: records what it was given. */
function view() {
  const got = [];
  return { got, dataset: { renderer: "gpu" }, provideData: (name, rows) => got.push({ name, rows: [...rows] }) };
}

test("rows arrive at the feed's pace, one version per beat", () => {
  const t = clock();
  const v = view();
  let cx;
  const feed = liveFeed({ source: "prices", view: v, connect: (c) => { cx = c; c.ok(); }, pace: 1000, timers: t });
  assert.equal(feed.status.state, "live");
  cx.push([{ p: 1 }]);
  t.advance(0);
  assert.equal(v.got.length, 1, "the first rows go at once");
  for (let i = 2; i <= 30; i++) { cx.push([{ p: i }]); t.advance(33); } // a trade every 33 ms
  t.advance(1000);
  assert.ok(v.got.length <= 3, `${v.got.length} versions for 30 trades in a second`);
  assert.deepEqual(v.got.flatMap((g) => g.rows).map((r) => r.p), Array.from({ length: 30 }, (_, i) => i + 1), "every row delivered, in order");
  assert.ok(v.got.every((g) => g.name === "prices"));
  feed.stop();
});

test("a chart that isn't running yet waits; a remounted chart gets everything kept", () => {
  const t = clock();
  let cx;
  let current = { got: [], dataset: {}, provideData(n, r) { this.got.push(r); } };
  const feed = liveFeed({ source: "s", view: () => current, connect: (c) => { cx = c; c.ok(); }, timers: t });
  cx.push([{ k: 1 }, { k: 2 }]);
  t.advance(2000);
  assert.equal(current.got.length, 0, "not running: nothing handed over (it would be lost)");
  current.dataset.renderer = "gpu";
  t.advance(300);
  assert.deepEqual(current.got.flat().map((r) => r.k), [1, 2]);
  cx.push([{ k: 3 }]);
  t.advance(1000);
  assert.deepEqual(current.got.at(-1).map((r) => r.k), [3], "then only what's new");
  // The page frees the chart and mounts a new one.
  current = { got: [], dataset: { renderer: "gpu" }, provideData(n, r) { this.got.push(r); } };
  cx.push([{ k: 4 }]);
  t.advance(1000);
  assert.deepEqual(current.got.flat().map((r) => r.k), [1, 2, 3, 4], "a new view gets every row kept");
  feed.stop();
});

test("failures back off 1, 2, 4 s … and a working connection resets the delay", () => {
  const t = clock();
  const v = view();
  const attempts = [];
  let succeed = false;
  const states = [];
  const feed = liveFeed({ source: "s", view: v, timers: t, backoff: { first: 1000, max: 4000 }, onStatus: (s) => states.push(s.state),
    connect: (c) => { attempts.push(t.now()); if (succeed) c.ok(); else c.fail(new Error("503")); } });
  t.advance(1000 + 2000 + 4000 + 4000 + 1);
  assert.deepEqual(attempts, [0, 1000, 3000, 7000, 11000], "doubling, capped at the max");
  assert.equal(feed.status.state, "retrying");
  assert.equal(feed.status.error, "503");
  succeed = true;
  t.advance(4000);
  assert.equal(feed.status.state, "live");
  assert.equal(feed.status.failures, 0);
  assert.ok(states.includes("retrying") && states.at(-1) === "live");
  feed.stop();
  assert.equal(feed.status.state, "stopped");
});

test("a hidden tab closes the connection after a moment and opens it again when it's back", () => {
  const t = clock();
  const doc = new EventTarget();
  doc.visibilityState = "visible";
  globalThis.document = doc;
  try {
    let opened = 0, closed = 0;
    const feed = liveFeed({ source: "s", view: view(), timers: t, linger: 2000, connect: (c) => { opened++; c.ok(); return () => closed++; } });
    assert.equal(opened, 1);
    doc.visibilityState = "hidden";
    doc.dispatchEvent(new Event("visibilitychange"));
    t.advance(1000);
    doc.visibilityState = "visible";
    doc.dispatchEvent(new Event("visibilitychange"));
    t.advance(5000);
    assert.equal(closed, 0, "a glance away doesn't drop the connection");
    doc.visibilityState = "hidden";
    doc.dispatchEvent(new Event("visibilitychange"));
    t.advance(2500);
    assert.equal(closed, 1);
    assert.equal(feed.status.state, "paused");
    doc.visibilityState = "visible";
    doc.dispatchEvent(new Event("visibilitychange"));
    assert.equal(opened, 2, "connects again (and can fetch what it missed)");
    assert.equal(feed.status.state, "live");
    feed.stop();
  } finally {
    delete globalThis.document;
  }
});

test("an off-screen chart pauses its feed", () => {
  const t = clock();
  let observe;
  globalThis.IntersectionObserver = class { constructor(cb) { observe = cb; } observe() {} disconnect() {} };
  try {
    let opened = 0, closed = 0;
    const slot = {};
    const feed = liveFeed({ source: "s", view: () => null, watch: slot, timers: t, connect: (c) => { opened++; c.ok(); return () => closed++; } });
    observe([{ isIntersecting: false }]);
    t.advance(3000);
    assert.equal(closed, 1);
    assert.equal(feed.status.state, "paused");
    observe([{ isIntersecting: true }]);
    assert.equal(opened, 2);
    feed.stop();
  } finally {
    delete globalThis.IntersectionObserver;
  }
});

test("the socket transport subscribes on open, parses messages and fails on close", () => {
  const t = clock();
  const sockets = [];
  globalThis.WebSocket = class {
    constructor(url) { this.url = url; this.sent = []; sockets.push(this); }
    send(m) { this.sent.push(m); }
    close() { this.closed = true; }
  };
  try {
    const v = view();
    const feed = liveFeed({ source: "prices", view: v, timers: t, connect: socket("wss://feed.example/?token=x", {
      open: (ws) => ws.send(JSON.stringify({ type: "subscribe", symbol: "AAPL" })),
      message: (m) => (m.type === "trade" ? m.data.map((d) => ({ sym: d.s, t: d.t, p: d.p })) : null),
    }) });
    const ws = sockets[0];
    ws.onopen();
    assert.deepEqual(ws.sent, ['{"type":"subscribe","symbol":"AAPL"}']);
    assert.equal(feed.status.state, "live");
    ws.onmessage({ data: '{"type":"ping"}' });
    ws.onmessage({ data: JSON.stringify({ type: "trade", data: [{ s: "AAPL", t: 1, p: 250.5 }] }) });
    t.advance(0);
    assert.deepEqual(v.got.at(-1).rows, [{ sym: "AAPL", t: 1, p: 250.5 }]);
    ws.onclose({ code: 1006, reason: "" });
    assert.equal(feed.status.state, "retrying");
    assert.ok(ws.closed, "the old socket is closed");
    t.advance(1000);
    assert.equal(sockets.length, 2, "and a new one opened after the backoff");
    feed.stop();
    assert.ok(sockets[1].closed);
  } finally {
    delete globalThis.WebSocket;
  }
});
