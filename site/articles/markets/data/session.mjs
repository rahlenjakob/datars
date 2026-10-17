// The chart's no-key data: the last full UTC day of bitcoin and ether trading on Binance, one price
// a minute, from Binance Vision's public daily files (CC BY-NC-SA 4.0, see ../README.md) — written
// to ../follow/session.json, which the chart opens on. Run by .github/workflows/markets-session.yml
// (manually until its schedule is switched on), or by hand:
//
//   node site/articles/markets/data/session.mjs [YYYY-MM-DD]     (default: yesterday, UTC)
//
// Stock prices are never written here: no free stock feed allows a public site to republish them,
// so readers follow stocks with their own key, in their own browser (../live.js).
import { inflateRawSync } from "node:zlib";
import { writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const out = join(here, "../follow/session.json");
const MIN = 60_000, DAY = 86_400_000;

/** The one file in a zip (Binance Vision's archives hold a single CSV). */
function unzipOne(buf) {
  if (buf.readUInt32LE(0) !== 0x04034b50) throw new Error("not a zip file");
  const method = buf.readUInt16LE(8);
  let size = buf.readUInt32LE(18);
  const nameLen = buf.readUInt16LE(26), extraLen = buf.readUInt16LE(28);
  const start = 30 + nameLen + extraLen;
  if (size === 0 || size === 0xffffffff) {
    // Sizes in a data descriptor or zip64: read them from the central directory instead.
    const cd = buf.lastIndexOf(Buffer.from([0x50, 0x4b, 0x01, 0x02]));
    size = buf.readUInt32LE(cd + 20);
  }
  const data = buf.subarray(start, start + size);
  return method === 0 ? data : inflateRawSync(data);
}

/** Open times are microseconds since 2025 (milliseconds before): either way, back to ms. */
const ms = (x) => (x > 1e14 ? Math.round(x / 1000) : x);

async function day(symbol, date) {
  const url = `https://data.binance.vision/data/spot/daily/klines/${symbol}/1m/${symbol}-1m-${date}.zip`;
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${res.status} ${url}`);
  const csv = unzipOne(Buffer.from(await res.arrayBuffer())).toString("utf8");
  // open time, open, high, low, close, volume, close time, …
  return csv.trim().split("\n").filter((l) => /^\d/.test(l)).map((l) => {
    const c = l.split(",");
    return { t: ms(Number(c[0])), open: Number(c[1]), close: Number(c[4]) };
  });
}

// The day as the chart writes it (AP style: Oct. 16, 2026).
const MONTHS = ["Jan.", "Feb.", "March", "April", "May", "June", "July", "Aug.", "Sept.", "Oct.", "Nov.", "Dec."];
const said = (d) => `${MONTHS[+d.slice(5, 7) - 1]} ${+d.slice(8, 10)}, ${d.slice(0, 4)}`;

const date = process.argv[2] ?? new Date(Date.now() - DAY).toISOString().slice(0, 10);
const t0 = Date.parse(`${date}T00:00:00Z`);
const coins = [
  { sym: "BTC", pair: "BTCUSDT", name: "Bitcoin" },
  { sym: "ETH", pair: "ETHUSDT", name: "Ether" },
];
const prices = [];
const meta = [];
for (const c of coins) {
  const bars = await day(c.pair, date);
  if (bars.length < 1400) throw new Error(`${c.pair} ${date}: only ${bars.length} minutes`);
  for (const b of bars) prices.push({ sym: c.sym, t: b.t, p: Math.round(b.close * 100) / 100, day: date });
  // The day's reference is its first minute's open (crypto has no close: it trades round the clock).
  meta.push({ sym: c.sym, name: c.name, venue: "Binance, in tether (USDT)", day: date, x0: t0, x1: t0 + DAY, ref: bars[0].open, refLabel: "At midnight UTC", fmt: "$,.2f", tz: "UTC", label: said(date) });
}
writeFileSync(out, JSON.stringify({
  about: `Binance Vision 1-minute klines (close), ${date} UTC, BTCUSDT and ETHUSDT. Licence: CC BY-NC-SA 4.0 (https://data.binance.vision). Times are UTC milliseconds.`,
  recorded: date,
  prices,
  meta,
}));
console.log(`${out}: ${prices.length} prices, ${date}`);
