---
title: Themes and brands
description: Give every datars chart your brand — colour tokens, palettes, fonts, sizes and corners — with light, dark and high-contrast modes, locks, checks and runtime overrides.
lede: A theme is a set of typed tokens. Every chart in the standard library draws with them, so one theme restyles all of your charts — and an app can restyle a published chart at runtime.
---

## A theme in ten lines

```ts
import { doc, font, theme } from "@datars/sdk";

const newsroom = theme({
  name: "newsroom",
  extends: "datars/neutral",          // start from the built-in theme
  tokens: {
    paper: "#fbf8f1",
    ink: "#1f1c17",
    accent: "#8c2f1b",
    "font.title": font.google("Source Serif 4", { weight: 600 }),
    "size.title": 20,
  },
  modes: { dark: { paper: "#15130f", ink: "#f2efe6" } },
});

export default doc({ title: "Europe's longest rivers", theme: newsroom, /* data, scene… */ });
```

Set three colours and the rest follows: the built-in theme derives its greys, gridlines, surfaces and label inks from `paper`, `ink` and `accent` with colour expressions, so a brand that changes those three gets a coherent chart in every mode.

To tweak a single chart without defining a theme, give the document its own token overrides:

```ts
theme: { use: "datars/neutral", tokens: { paper: "#0b1020", ink: "#eef2ff" } },
```

<figure class="fig"><div class="frame"><div class="chart" data-chart="brand"></div></div><figcaption><b>One chart, every token.</b> Bars, lines, a world map and a card that draw only with theme tokens — no literal colours. The <a href="/features/theming/">theming page</a> lets you switch brands and modes on it live.</figcaption>{{alt:brand}}</figure>

## Tokens

A token has a type, and the type decides what you can write:

| Type | Written as | Examples |
|---|---|---|
| Colour | a colour or a colour expression | `"#b3261e"`, `"$accent"`, `"mix($ink, $paper, 0.9)"`, `"on($accent)"` |
| Palette | a list, `{ kind, colors }`, or `{ generate, from, to?, n }` | `categorical`, `sequential`, `diverging` |
| Number | a number | `"size.label": 11`, `dim: 0.28`, `"motion.duration": 0.9` |
| Font | `{ family, weight, italic, src \| google }` | `"font.title"` |
| Text | any other string | `"motion.easing": "cubic-in-out"` |

### The built-in theme's tokens

`datars/neutral` is the theme every document uses unless it says otherwise. These are the tokens most brands touch; a theme may add its own and refer to them as `$name`.

| Token | Default (light) | What reads it |
|---|---|---|
| `paper` | `#ffffff` | the background |
| `ink` | `#1d1f24` | text, axis lines |
| `ink-2` | `mix($ink, $paper, 0.22)` | secondary text, value labels |
| `muted` | `mix($ink, $paper, 0.35)` | tick labels, notes |
| `grid`, `rule` | mixes of ink and paper | gridlines, rules |
| `surface`, `surface-ink` | a tint of paper, `$ink` | panels, tooltips |
| `accent`, `accent-ink` | `#4269d0`, `on($accent)` | the highlight colour and text on it |
| `mark` | `$accent` | a mark's colour when no colour scale applies |
| `positive`, `negative`, `highlight` | green, red, yellow | gains and losses, emphasis |
| `dim` | `0.28` | how far unselected marks recede |
| `categorical` | ten colours | colours per category (keys without a colour of their own) |
| `sequential`, `diverging` | five-colour ramps | choropleths, heatmaps, stripes |
| `font.body`, `font.strong`, `font.title`, `font.number` | Inter 400, 600, 700, 400 | all text |
| `size.small`, `size.label`, `size.body`, `size.title` | `10`, `11`, `13`, `17` | text sizes (px) |
| `radius.mark`, `radius.bar`, `radius.card` | `2`, `0`, `8` | corner radii |
| `stroke.line`, `stroke.rule`, `stroke.grid` | `2`, `1`, `1` | line widths |
| `point.radius` | `4` | dots in scatters and line points |
| `band.padding` | `0.22` | gaps between bars |
| `motion.duration`, `motion.easing`, `motion.stagger` | `0.9`, `"cubic-in-out"`, `0.3` | default motion |
| `map.water`, `map.land`, `map.park`, `map.building`, `map.road`, `map.road-major`, `map.border`, `map.label`, `map.label-halo`, `map.marker`, `map.no-data` | light basemap colours | basemaps and choropleths |
| `card`, `card-ink`, `card-ink-2`, `card-line` | tints of paper and ink | text cards over charts |

The whole file is [`crates/datars-theme/themes/neutral.json`]({{src}}/crates/datars-theme/themes/neutral.json). Each recipe's page in the [chart reference](/docs/std/) lists the tokens it reads.

### Colour expressions

Colours can be computed from other tokens, so a theme sets a few and derives the rest:

| Function | What it does |
|---|---|
| `mix(a, b, t)` | a mix of two colours, `t` of the way from `a` to `b` (in OKLab) |
| `alpha(a, x)` | `a` at opacity `x` |
| `lighten(a, d)`, `darken(a, d)` | lighter or darker (OKLCH lightness) |
| `saturate(a, d)`, `desaturate(a, d)` | more or less colourful |
| `oklch(l, c, h)`, `rgb(r, g, b)` | colours by their components |
| `on(bg)` | black or white, whichever reads better on `bg` |
| `contrast(bg, a, b)` | whichever of `a` or `b` contrasts more with `bg` |
| `$token`, `$palette[i]` | another token, or one colour of a palette |

Derived tokens re-derive in each mode: `grid` is `mix($ink, $paper, 0.9)`, so when dark mode swaps `paper` and `ink`, the gridlines follow.

### Palettes

```ts
categorical: ["#b3261e", "#1d3f6e", "#c9922e", "#4d6b3c"],
sequential: { kind: "sequential", colors: ["#f1e2d0", "#d58566", "#962a1f"] },
diverging: { generate: "diverging", from: "#2166ac", to: "#b2182b", n: 7 },
categorical: { generate: "categorical", from: "$brand", n: 8 },
```

`generate` builds a palette from one or two colours: a categorical set around your brand colour, a sequential ramp, or a diverging ramp between two colours through a light middle.

### Colours that are data

A party's colour isn't a theme decision — it's data. Declare it once per key and every recipe uses it; keys without a colour take the theme's `categorical` palette:

```ts
keys: { S: { name: "Social Democrats", color: "#e8112d" }, M: { name: "Moderates", color: "#1b49dd" } },
```

## Fonts travel with the chart

A font token says which face to draw with *and where it comes from*. The build step (`datars publish`, `bundle`, `render`, `dev`) reads the file or downloads the Google Fonts family once into a disk cache, cuts it down to the characters the chart can show, and ships that subset inside the bundle. No runtime ever downloads a font from Google or uses a system font, so line breaks and label placement are identical everywhere.

```ts
import { font } from "@datars/sdk";

"font.body":  font.file("Newsreader", "fonts/Newsreader-Regular.ttf"),        // a file next to the document
"font.title": font.file("Newsreader", "fonts/Newsreader-SemiBold.ttf", { weight: 600 }),
"font.number": font.google("Source Serif 4", { weight: 400 }),              // a Google Fonts family
```

- Files are TrueType, OpenType or WOFF (not WOFF2 yet), by path relative to the document or an `https://` URL.
- One token is one face. Use static instances; a variable font draws at its default instance.
- A family with no source is reported by `datars check` and `datars lint` (“font 'GT America' has no source; falling back to Inter”) — never a silent fallback.
- Licences are read from each font and recorded in the bundle; `lint` warns about fonts whose licence restricts embedding.
- Labels in other scripts (Hebrew, Arabic, CJK) come from a document font, whose glyphs join every label's fallback chain: `data: { hebrew: data.font("fonts/NotoSansHebrew-Regular.ttf") }` or `data.font("google:Noto Sans Hebrew")`.

`datars fonts doc.ts` lists every font token with the face it resolved to, its source and its licence:

```sh
$ datars fonts examples/serif/doc.json
font.body      Newsreader    400 ../../assets/fonts/Newsreader-Regular.ttf    Newsreader-400, OFL-1.1
font.strong    Newsreader    600 ../../assets/fonts/Newsreader-SemiBold.ttf   Newsreader-600, OFL-1.1
font.title     Newsreader    600 ../../assets/fonts/Newsreader-SemiBold.ttf   Newsreader-600, OFL-1.1
```

## Light, dark and high contrast

Every theme holds three modes. Base tokens apply first, then the active mode's, in the order the themes extend each other — like the CSS cascade. A brand that only sets `accent` keeps the built-in theme's tuned dark surfaces and gets its accent in all three modes.

```ts
modes: {
  dark: { paper: "#191714", ink: "#ece6d8", accent: "#e2574c" },
  "high-contrast": { accent: "#8c1d14" },
},
```

A brand that is dark everywhere — a magazine with a black page — says so with `scheme: "dark"`. Its base tokens are its dark values, and in light mode everything it doesn't set resolves as in dark mode, so the basemap, the ramps and the surfaces stay dark too:

```ts
theme({ name: "night", extends: "datars/neutral", scheme: "dark", tokens: { paper: "#0b0620", ink: "#f3ecff", accent: "#ff2bd6" } })
```

On the web the chart follows the reader's `prefers-color-scheme` unless the page sets a mode. In apps, the host passes the platform's setting.

## Restyle a published chart at runtime

Scenes store colours as late-bound inks (`$accent`, `$categorical[2]`), resolved when each frame is drawn. So a chart that was published months ago can wear today's brand, or the app's dark mode, without being republished. Type and size tokens re-lay the chart out.

**Web** — the `mode` attribute and `setTokens()` on [`<datars-view>`](/docs/embed/web/):

```html
<datars-view id="chart" src="https://charts.example.com/c/sales" mode="dark"></datars-view>
<script type="module">
  document.getElementById("chart").setTokens({
    accent: "#0f766e",
    categorical: ["#0f766e", "#6cb8d0", "#e8a33d"],
    "radius.bar": 7,
  });
</script>
```

`mode` is `light`, `dark` or `high-contrast`.

Font tokens can name a face on your own server — `"font.title": { family: ["Manrope"], weight: 700, src: "/fonts/Manrope-Bold.ttf" }` — and the runtime fetches it (a `datarequest` the page can answer from a cache it shares between charts). Tokens set before a chart opens are in its first frame, faces included: it waits up to 1.5 s for them rather than opening in a fallback and re-flowing, and skips the poster, which shows the published look. This website does exactly that for every chart it shows: the reader's look from *Make it yours* on the [home page](/#make-it-yours) (or the palette button in the header) is kept in `localStorage` and applied to each chart as it mounts.

**iOS and macOS** — `DatarsEngine.setMode(_:)` (`.light`, `.dark`, `.highContrast`) and `setTokens(_:)` on the chart view's engine:

```swift
let chart = DatarsChartView()
chart.load(.url(URL(string: "https://charts.example.com/c/sales")!))
chart.engine.setMode(.dark)
chart.engine.setTokens(["accent": "#0f766e", "radius.bar": 7])
```

**Android** — `setDarkMode(dark: Boolean)` on `DatarsView`. Token overrides aren't exposed in the Android view yet.

Overrides honour the theme's locks (below). Unknown token names draw in magenta and are named by the linter, so a typo shows instead of hiding.

## Locks

A theme can lock tokens so no later layer — the document, or the app that embeds it — can change them. The override is ignored and reported. Use it to keep an organisation's brand colour and type fixed while letting teams change everything else:

```ts
theme({ name: "acme", extends: "datars/neutral", tokens: { brand: "#b3261e", accent: "$brand" }, locked: ["brand", "accent", "font.title"] })
```

The layers, in order: `datars/neutral` → each theme that extends it → the document's `tokens` → the host app's `setTokens`.

## Checks

A theme declares checks, and `datars lint` runs them on the resolved theme in every mode:

```ts
checks: [
  { contrast: ["ink", "paper"], min: 7 },              // WCAG contrast ratio
  { contrast: ["accent-ink", "accent"], min: 4.5 },
  { distinct: "categorical", min: 8, cvd: true, first: 8 }, // pairwise ΔE, also under colour-vision deficiencies
  { monotone: "sequential" },                            // a ramp that only gets darker (or lighter)
],
```

The built-in theme passes its own checks in all three modes. Two commands help while designing a theme as a JSON file:

```sh
datars theme brand.json --mode dark              # resolve it: every diagnostic and failed check
datars theme brand.json --specimen specimen.png  # every std chart, palette and ink in light, dark and high contrast
```

## See also

- [Accessibility](/docs/accessibility/) — high-contrast mode and the colour checks in context.
- [Maps](/docs/maps/) — the `map.*` tokens on basemaps.
- [Chart reference](/docs/std/) — the tokens each recipe reads.
