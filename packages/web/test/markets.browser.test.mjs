// The markets article (site/articles/markets) in Chrome, with the network mocked: no request leaves
// the machine. Without a key it shows the recorded day and replays it; with keys (mocked Twelve
// Data and Finnhub, its WebSocket included) it follows a closed stock market's last session and a
// live bitcoin, says so on the chart and to screen readers (politely, not every trade), closes the
// socket when the chart is off the screen and opens it again when it's back, and recovers from a
// failing provider. Nothing on the page moves while it does (layout shift).
//
// Needs the built site (node scripts/build-site.mjs, or DATARS_SITE=<folder> with the markets
// article built into it) and Playwright:
//
//   DATARS_PLAYWRIGHT=/path/to/node_modules/@playwright/test/index.mjs node --test packages/web/test/markets.browser.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { readFileSync, existsSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join, extname, normalize } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const site = process.env.DATARS_SITE ?? join(root, "out/pages");
const playwright = process.env.DATARS_PLAYWRIGHT;
const skip = !playwright ? "set DATARS_PLAYWRIGHT to a Playwright module" : !existsSync(join(site, "articles/markets/index.html")) ? "build the site with the markets article first" : false;
const types = { ".html": "text/html; charset=utf-8", ".js": "text/javascript", ".css": "text/css", ".wasm": "application/wasm", ".json": "application/json", ".ttf": "font/ttf", ".png": "image/png", ".svg": "image/svg+xml" };
const MIN = 60_000;

function serve() {
  const server = createServer((req, res) => {
    let path = decodeURIComponent(new URL(req.url, "http://x").pathname);
    if (path.endsWith("/")) path += "index.html";
    const file = normalize(join(site, path));
    if (!file.startsWith(site) || !existsSync(file) || !statSync(file).isFile()) return res.writeHead(404).end();
    res.writeHead(200, { "Content-Type": types[extname(file)] ?? "application/octet-stream", "Cache-Control": "no-store" });
    res.end(readFileSync(file));
  });
  // The test ports: 8900–8930.
  return new Promise((resolve, reject) => {
    const tryPort = (p) => { server.once("error", (e) => (p < 8930 ? tryPort(p + 1) : reject(e))); server.listen(p, "127.0.0.1", () => resolve({ server, base: `http://127.0.0.1:${p}` })); };
    tryPort(8900 + Math.floor(Math.random() * 20));
  });
}

/** Friday's AAPL session, a seeded walk, as Twelve Data sends it (newest first, New York time). */
function aaplSession() {
  let p = 342, x = 7;
  const values = [];
  for (let m = 0; m < 390; m++) {
    x = (x * 1103515245 + 12345) % 2147483648;
    p += (x / 2147483648 - 0.495) * 0.6;
    const t = Date.UTC(2026, 9, 16, 9, 30) + m * MIN;
    values.unshift({ datetime: new Date(t).toISOString().slice(0, 19).replace("T", " "), open: p.toFixed(2), high: (p + 0.2).toFixed(2), low: (p - 0.2).toFixed(2), close: p.toFixed(2), volume: "1000" });
  }
  return { meta: { symbol: "AAPL", interval: "1min", exchange_timezone: "America/New_York" }, values, status: "ok" };
}

async function chart(p) {
  await p.waitForFunction(() => document.querySelector('figure[data-story="follow"] datars-view')?.dataset.renderer, null, { timeout: 60_000 });
  return p.locator('figure[data-story="follow"] datars-view');
}
const signals = (p) => p.evaluate(() => document.querySelector('figure[data-story="follow"] datars-view').signals);
const points = (p) => p.evaluate(() => document.querySelector('figure[data-story="follow"] datars-view').shadowRoot.querySelectorAll(".mirror tr, .mirror [role=row]").length);

test("the markets article: recorded without a key, live with mocked keys, polite, paused out of sight", { skip, timeout: 300_000 }, async () => {
  const { server, base } = await serve();
  const { chromium } = await import(playwright);
  const browser = await chromium.launch({ channel: process.env.DATARS_BROWSER_CHANNEL ?? "chrome", headless: true, args: ["--enable-unsafe-webgpu"] });
  const outside = [];
  try {
    // ---- without a key: the recorded day, and its replay ----
    {
      const ctx = await browser.newContext({ viewport: { width: 1280, height: 900 } });
      await ctx.route(/^(?!http:\/\/127\.0\.0\.1)/, (r) => { outside.push(r.request().url()); r.abort(); });
      const p = await ctx.newPage();
      const errors = [];
      p.on("pageerror", (e) => errors.push(String(e)));
      p.on("console", (m) => m.type() === "error" && errors.push(m.text()));
      await p.addInitScript(() => {
        window.__cls = 0;
        new PerformanceObserver((l) => { for (const e of l.getEntries()) if (!e.hadRecentInput) window.__cls += e.value; }).observe({ type: "layout-shift", buffered: true });
      });
      await p.goto(`${base}/articles/markets/`);
      await p.locator('figure[data-story="follow"]').scrollIntoViewIfNeeded();
      await chart(p);
      await p.waitForTimeout(1500);
      const s = await signals(p);
      assert.equal(s.feed, "recorded");
      assert.equal(s.sym, "BTC");
      assert.match(s.status, /^Recorded \w+\. \d+, 20\d\d · Not live · Times are UTC$/);
      assert.equal(s.source, "Data: Binance Vision (CC BY-NC-SA 4.0)");
      const summary = await p.evaluate(() => document.querySelector('figure[data-story="follow"] datars-view').shadowRoot.querySelector(".mirror p.summary")?.textContent ?? "");
      assert.match(summary, /One price a minute/, summary);
      // The replay: the line grows from nothing, through the same live path.
      await p.click("#replay");
      await p.waitForTimeout(600);
      assert.match((await signals(p)).status, /^Replay of /);
      // What screen readers get follows the feed, every few seconds (not every beat).
      await p.waitForTimeout(5500);
      const early = await p.evaluate(() => document.querySelector('figure[data-story="follow"] datars-view').shadowRoot.querySelector(".mirror p.summary")?.textContent);
      await p.waitForTimeout(5500);
      const later = await p.evaluate(() => document.querySelector('figure[data-story="follow"] datars-view').shadowRoot.querySelector(".mirror p.summary")?.textContent);
      const count = (t) => Number((t ?? "").match(/([\d,]+) data points/)?.[1]?.replace(/,/g, "") ?? 0);
      assert.ok(count(later) > count(early) + 60, `the replay grows: ${count(early)} → ${count(later)} points`);
      await p.click("#replay"); // stop
      await p.waitForTimeout(1500);
      assert.match((await signals(p)).status, /^Recorded /, "the recorded day is back");
      const cls = await p.evaluate(() => window.__cls);
      assert.ok(cls < 0.01, `layout shift ${cls}`);
      assert.deepEqual(errors, []);
      await ctx.close();
    }

    // ---- with keys: mocked Twelve Data and Finnhub ----
    {
      const ctx = await browser.newContext({ viewport: { width: 390, height: 844 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true });
      const asked = [];
      let tdFail = 1; // the first history request fails: the feed backs off and recovers
      await ctx.route(/^(?!http:\/\/127\.0\.0\.1)/, (r) => { outside.push(r.request().url()); r.abort(); });
      await ctx.route("https://api.twelvedata.com/**", (r) => {
        const u = new URL(r.request().url());
        asked.push(`td ${u.pathname} ${u.searchParams.get("symbol")} ${u.searchParams.get("outputsize") ?? ""}`);
        assert.equal(u.searchParams.get("apikey"), "td-test-key");
        if (u.pathname === "/time_series" && u.searchParams.get("symbol") === "AAPL" && tdFail-- > 0) return r.fulfill({ status: 503, body: '{"status":"error","code":503,"message":"busy"}' });
        if (u.pathname === "/quote") return r.fulfill({ contentType: "application/json", body: JSON.stringify({ symbol: "AAPL", previous_close: "342.00", close: "343.46", is_market_open: false }) });
        if (u.pathname === "/time_series" && u.searchParams.get("symbol") === "AAPL") return r.fulfill({ contentType: "application/json", body: JSON.stringify(aaplSession()) });
        // Bitcoin: today's minutes so far (UTC).
        const now = Date.now(), t0 = Math.floor(now / 86_400_000) * 86_400_000;
        const n = Math.min(Number(u.searchParams.get("outputsize")), Math.floor((now - t0) / MIN));
        const values = Array.from({ length: n }, (_, i) => { const t = t0 + (n - 1 - i) * MIN; const p = (81_000 + Math.sin(t / 3e6) * 300).toFixed(2); return { datetime: new Date(t).toISOString().slice(0, 19).replace("T", " "), open: p, close: p }; });
        return r.fulfill({ contentType: "application/json", body: JSON.stringify({ meta: { symbol: "BTC/USD" }, values, status: "ok" }) });
      });
      await ctx.route("https://finnhub.io/api/v1/**", (r) => {
        const u = new URL(r.request().url());
        asked.push(`fh ${u.pathname} ${u.searchParams.get("symbol")}`);
        const btc = u.searchParams.get("symbol") === "BINANCE:BTCUSDT";
        return r.fulfill({ contentType: "application/json", body: JSON.stringify(btc ? { c: 81_200, pc: 81_615.64, t: Math.floor(Date.now() / 1000) } : { c: 343.46, pc: 342, t: Math.floor(Date.UTC(2026, 9, 16, 20, 0) / 1000) }) });
      });
      const sockets = [];
      await ctx.routeWebSocket(/wss:\/\/ws\.finnhub\.io/, (ws) => {
        const s = { subs: [], closed: false, timer: null };
        sockets.push(s);
        assert.match(ws.url(), /token=fh-test-key/);
        ws.onMessage((m) => {
          const msg = JSON.parse(String(m));
          if (msg.type === "subscribe") s.subs.push(msg.symbol);
        });
        ws.onClose(() => { s.closed = true; clearInterval(s.timer); });
        let p = 81_200;
        s.timer = setInterval(() => {
          p += Math.sin(Date.now() / 900) * 4;
          ws.send(JSON.stringify({ type: "trade", data: [{ s: "BINANCE:BTCUSDT", p, t: Date.now() - 40, v: 0.01 }] }));
        }, 150);
      });
      const p = await ctx.newPage();
      const errors = [];
      p.on("pageerror", (e) => errors.push(String(e)));
      p.on("console", (m) => m.type() === "error" && errors.push(m.text()));
      await p.addInitScript(() => localStorage.setItem("datars.markets.keys", JSON.stringify({ finnhub: "fh-test-key", twelvedata: "td-test-key" })));
      await p.goto(`${base}/articles/markets/`);
      await p.locator('figure[data-story="follow"]').scrollIntoViewIfNeeded();
      await chart(p);
      // The failed history is retried after the backoff; then Friday's session is there.
      await p.waitForFunction(() => /Market closed · Last session Oct\. 16, 2026/.test(document.querySelector('figure[data-story="follow"] datars-view').signals?.status ?? ""), null, { timeout: 20_000 });
      let s = await signals(p);
      assert.equal(s.feed, "live");
      assert.equal(s.sym, "AAPL");
      assert.match(s.source, /Twelve Data \(minute bars\) and Finnhub \(trades\), with your key/);
      // The failed history was asked for again after the backoff (1 s ± 20 %).
      for (let i = 0; i < 40 && asked.filter((a) => a.startsWith("td /time_series AAPL 390")).length < 2; i++) await p.waitForTimeout(250);
      assert.equal(asked.filter((a) => a.startsWith("td /time_series AAPL 390")).length, 2, `retried once: ${asked.join(", ")}`);
      await p.waitForTimeout(6500); // the mirror follows data every few seconds
      const aaplSummary = await p.evaluate(() => document.querySelector('figure[data-story="follow"] datars-view').shadowRoot.querySelector(".mirror p.summary")?.textContent);
      assert.match(aaplSummary, /391 data points/, aaplSummary); // 390 minutes and the closing print
      assert.ok(sockets[0]?.subs.includes("AAPL") && sockets[0].subs.includes("BINANCE:BTCUSDT"), JSON.stringify(sockets[0]?.subs));
      // The screen reader hears the market once, not every trade.
      await p.waitForFunction(() => /^Apple: \$/.test(document.getElementById("markets-said").textContent), null, { timeout: 10_000 });
      // What the live region says, polled: each distinct text once.
      const said = [];
      const listen = setInterval(() => p.evaluate(() => document.getElementById("markets-said").textContent).then((t) => { if (t !== said.at(-1)) said.push(t); }, () => {}), 200);
      said.push(await p.evaluate(() => document.getElementById("markets-said").textContent));

      // Bitcoin: live trades from the socket.
      await p.evaluate(() => document.querySelector('figure[data-story="follow"] datars-view').setSignal("sym", "BTC"));
      await p.waitForFunction(() => /^Live · Trades around the clock · Real-time/.test(document.querySelector('figure[data-story="follow"] datars-view').signals?.status ?? ""), null, { timeout: 20_000 });
      const a = await p.evaluate(() => document.querySelector('figure[data-story="follow"] datars-view').shadowRoot.querySelector(".mirror p.summary")?.textContent);
      await p.waitForTimeout(6000);
      clearInterval(listen);
      said.shift(); // Apple's
      assert.ok(said.length <= 2, `announced ${said.length} times in 6 s of trades: ${said.join(" | ")}`);
      assert.ok(said.some((t) => /^Bitcoin: \$[\d,]+\.\d\d, (up|down) \d+\.\d\d percent\. Live/.test(t)), said.join(" | "));
      const b = await p.evaluate(() => document.querySelector('figure[data-story="follow"] datars-view').shadowRoot.querySelector(".mirror p.summary")?.textContent);
      assert.match(a ?? "", /data points/);
      assert.match(b ?? "", /data points/);

      // Out of sight, the socket closes; back in view, it opens again.
      const open = sockets.length;
      await p.evaluate(() => window.scrollTo(0, document.body.scrollHeight));
      await p.waitForTimeout(3500);
      assert.ok(sockets.at(-1).closed, "closed off screen");
      assert.match((await signals(p)).status, /Paused while the chart is out of sight/);
      await p.locator('figure[data-story="follow"]').scrollIntoViewIfNeeded();
      await p.waitForFunction((n) => true, open);
      await p.waitForTimeout(2500);
      assert.equal(sockets.length, open + 1, "a new connection when it's back");
      assert.match((await signals(p)).status, /^Live/);
      assert.deepEqual(errors.filter((e) => !/503|Failed to load resource/.test(e)), []);

      // Forgetting the keys goes back to the recorded day.
      await p.click("#keys-forget");
      await p.waitForTimeout(1200);
      s = await signals(p);
      assert.equal(s.feed, "recorded");
      assert.equal(await p.evaluate(() => localStorage.getItem("datars.markets.keys")), "{}");
      await ctx.close();
    }
    assert.deepEqual(outside, [], "nothing left the machine");
  } finally {
    await browser.close();
    server.close();
  }
});
