# Articles

Ten data stories, as pages of the site, in one house style: a newspaper graphics feature with a
serif for reading and headlines, a sans for everything around the text, one accent colour, and the
sources under every figure. Each article is a folder:

- `index.html` — the page, maintained by hand: masthead (kicker, headline, standfirst, byline and
  date), the text, and its charts as slots —
  `<figure class="figure" data-story="busiest">` for a figure (`full` for a wide one), its caption
  ending in `<span class="credit">Source: …</span>` (the build adds the "Chart code" link there),
  `<section class="scrolly text-left" data-story="routes">` with `.step` elements for a scroll
  story (the chart changes state as each step passes; a `<p class="graphic-credit">` right after
  it is its source line), `<figure class="figure video vertical" data-video="…">` for a film the
  engine exports, `data-cover="<state>"` on the one the articles index shows.
- `<story>/doc.ts` — every chart, a datars document in TypeScript on `@datars/sdk` and
  `@datars/std`, with its data files and map tiles next to it (or in the article's `data/` when
  several charts share them). A story from another article is named by its path (`../warming/spiral`).
- `theme.ts` — the chart theme its stories use: every article re-exports the shared house theme,
  `editorial.ts` (over `datars/neutral`), so the charts and the pages match.

`articles.css` and `articles.js` are the pages' shared styles and behaviour (charts mount near the
screen and are freed far from it; scroll stories hand their steps to the view; stepped figures get
step buttons). `index.html` here is the articles index; its cards are filled in from the pages.
`recipes/components.js` is a package of chart types of its own (bubbles, petals, seasons, spiral,
cloud, formation) some stories use.

`node scripts/build-site.mjs` builds them (`scripts/build-articles.mjs`: compiles each `doc.ts`,
publishes it, fills the slots in, renders the covers); `node --test packages/web/test/articles.test.mjs`
opens every page in headless Chrome and checks that it loads cleanly and every chart renders.

The data and its sources are named on each page, under the figures and in its "About the data";
`oil-2020/SOURCES.md` documents the markets story's files. Where a figure is rounded, approximate or
drawn for the page (the map of Westeros), the page says so. `us-debt` is live: its first chart
asks the Treasury's API for the latest figures when the page opens (`us-debt/SOURCES.md`). Map data ©
OpenStreetMap contributors (ODbL) and Natural Earth (public domain).
