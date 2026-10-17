# Markets: a live price follower

A price chart that moves as trades happen (`follow/doc.ts`), on a page whose script feeds it
(`page/live.js`). Live stock prices come from the reader's own free key; without one the chart
shows a recorded day of bitcoin and ether that may be shown publicly.

| file | what |
|---|---|
| `index.html` | the article |
| `follow/doc.ts` | the chart: a live source that takes rows (`mode: "upsert"`, key market + session + minute, `span` four days), the session as a fixed time axis (`exact`), the price axis in nice steps, the day's change in `$up`/`$down` |
| `follow/session.json` | the recorded day the chart opens on (Binance Vision, CC BY-NC-SA 4.0) |
| `data/session.mjs` | writes it: `node site/articles/markets/data/session.mjs [YYYY-MM-DD]` (default yesterday, UTC) |
| `page/live.js`, `page/markets.css` | the page's feed (copied next to `index.html` by the build), key form, replay, screen-reader summary |
| `page/feed.js` | its pure parts: the markets, New York's clock and the market's hours, provider messages → rows |
| `data/feed.json` | four minutes of real bitcoin trades (Binance Vision aggTrades, CC BY-NC-SA 4.0) as Finnhub's socket sends them, for `packages/web/test/markets.test.mjs` |
| `../../../.github/workflows/markets-session.yml` | refreshes `session.json` daily — manual only until its schedule is switched on |

## Which data, and why

Probed 2026-10-17 (a Saturday) with `curl -H "Origin: https://example.org"` and the providers' own
documents. "Public display" means showing the data to every visitor of a public website, or
committing a copy of it to a public repository.

| Source | CORS | Browser socket | Key | Free limits | Fresh | Public display | Stockholm |
|---|---|---|---|---|---|---|---|
| **Finnhub** | `*` | yes (`wss://ws.finnhub.io`, 1 per key) | free | 60/min | real-time US trades and quotes; minute candles are premium | no: "strictly for personal use", no redistribution ([terms](https://finnhub.io/terms-of-service)) | Enterprise only |
| **Twelve Data** | `*` | trial credits only | free | 8/min, 800/day | real-time US; ~12 sessions of 1-min bars | no: Basic is "internal non-display usage only" ([terms](https://twelvedata.com/terms)) | Pro plan |
| Alpha Vantage | `*` | no | free (25/day) | 25/day | real-time and 15-min delayed need a paid plan | no: personal, non-commercial ([terms](https://www.alphavantage.co/terms_of_service/)) | — |
| Massive (Polygon) | echoes origin | not free | free | 5/min | end of day | individual use only ([terms](https://massive.com/legal/individuals-terms-of-service)) | — |
| Tiingo | none | IEX feed, not confirmed free | free | 50/hour | — | no; software where each user brings a token is fine ([docs](https://www.tiingo.com/documentation/general/overview)) | — |
| EODHD | none | no | free (20/day) | 20/day | 15-min delayed, intraday paid | no display by non-professionals ([terms](https://eodhd.com/financial-apis/terms-conditions)) | paid |
| Marketstack, FMP | `*` | no | key | 100/month, 250/day | end of day | non-commercial / written approval | — |
| Stooq | — | — | CSV now behind a browser check and CAPTCHA key | — | — | automated use gated: not used | — |
| IEX Cloud | — | — | shut down 2024-08-31 | — | — | — | — |
| Nasdaq.com, Nasdaq Nordic pages | none | — | — | — | 15-min delayed | no: automated capture banned; display needs a distributor licence ([policies](https://www.nasdaq.com/docs/data/Nasdaq_European_Data_Policies_September_2026)) | same |
| FRED (SP500, DJIA, NASDAQCOM) | none | — | key | — | daily close | no: S&P Dow Jones / Nasdaq copyright, no caching ([legal](https://fred.stlouisfed.org/legal/)) | — |
| Coinbase Exchange | `*` | yes, no key | none | public | real-time crypto | no: "personal or research purposes" ([market data terms](https://www.coinbase.com/legal/market_data)) | — |
| Kraken, Binance live API | yes | yes, no key | none | — | real-time crypto | no public display licence found | — |
| **Binance Vision** | `*` | — (daily files) | none | — | the previous UTC day, 1-min | **yes, non-commercial: CC BY-NC-SA 4.0** ([terms](https://data.binance.vision/Binance_Vision-Terms_of_Use.pdf)) | — |

Nasdaq Stockholm: nothing free allows it. Twelve Data's Pro plan and Finnhub's Enterprise plan
cover it; Börsdata's API forbids public display; Avanza has no official API; Millistream sells
licensed feeds. A Swedish reader with a paid key could follow Stockholm the same way.

**The choice.**

- **Live stocks: the reader's own key, Finnhub or Twelve Data (or both).** Each reader uses
  their own personal-use licence on their own screen, and the site redistributes nothing.
  Finnhub is the only free plan with a browser WebSocket of real-time trades. Its free plan has
  no minute history, which Twelve Data has (1 request a minute while polling one market).
- **No key: Binance Vision**, the only real market data found whose terms allow a public page.
  It is crypto, not stocks, and the previous day, not live, and the chart says both. It is
  licensed **non-commercial**: Binance Vision's terms (§4.1) permit "non-monetized open
  educational projects". Whether an article on a product's site counts as non-commercial is for
  the site's owner to decide before publishing. Without that, the no-key state would have to be
  the key form alone.

**Keys.** A reader's key is kept in their browser's `localStorage` (`datars.markets.keys`) and
sent only to its provider, in that provider's own URL form (Finnhub: `token=`, Twelve Data:
`apikey=`). "Forget my keys" deletes it. The page loads no third-party scripts. A site-wide key
can be built in (`DATARS_PUBLIC_FINNHUB_KEY`, `DATARS_PUBLIC_TWELVEDATA_KEY`, read by
`scripts/build-articles.mjs` into `page/live.js`), but only for a licence that allows showing the
data to every reader: the free plans don't, and a built-in key is public to anyone who opens the
page. Never commit one.

## Honesty on the chart

- **State.** The status line says "Live · Market open · Real-time · Times are New York (ET)",
  "Market closed · Last session Oct. 16, 2026 · Opens Mon., Oct. 19, 9:30 a.m. ET", or
  "Recorded Oct. 16, 2026 · Not live · Times are UTC". Market hours are NYSE's regular session,
  with its 2026–27 holidays and early closes in `page/live.js`.
- **Delay.** It is measured, not assumed: each trade's own timestamp against the reader's clock.
  Under 30 s reads "Real-time"; more reads "Delayed about N min". Twelve Data's minute polling
  reads "Every minute" until the first poll.
- **Attribution.** The source line names the provider (Binance Vision with its licence, or
  Finnhub / Twelve Data "with your key"); the caption names all of them.

## How it moves

- Rows arrive through `liveFeed` (`@datars/web`) at most once a second — one version however
  many trades came — and the engine caps each move to the time since the last one.
- The open minute is the same key until it closes: its vertex moves. A new minute is a new key at
  the end: it grows out of the line.
- The time axis is the session (09:30–16:00 ET, or the UTC day), `exact`: it never slides or
  rescales as minutes arrive. The price axis is the prices and the previous close, widened to
  steps of 0.1 % of the reference and then to nice numbers: it moves only when a price breaks one,
  and then it glides.
- The feed closes while the tab is hidden or the chart is off the screen (after 2 s), and opens
  again when it's back. Failed connections back off from 1 s to a minute.
- The screen reader's summary (`#markets-said`, polite) speaks on a new market or a change of
  state, otherwise at most every five minutes.
- "Replay the day" plays the recorded day through the same path, as a session of its own: 30
  minutes a second.

## Tests

- `packages/web/test/markets.test.mjs` (no network): the recorded feed through `page/feed.js` and
  `liveFeed` into the published chart on the wasm engine — one version a beat, one point a
  minute, the open minute at its last trade, the session axis fixed, every move over before the
  next beat, the same frames twice; and the market clock (hours, holidays, daylight saving).
- `packages/web/test/markets.browser.test.mjs` (Chrome, network mocked, Finnhub's socket included):
  the recorded day and its replay, keys from local storage, a failing provider retried, the
  closed market said, polite announcements, the socket closed off screen and opened again,
  "Forget my keys", no layout shift, nothing leaving the machine.
