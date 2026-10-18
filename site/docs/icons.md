---
title: Icons, pictures and curved text
description: Draw icons from a built-in open set, your own SVGs or URLs in your theme's inks — one at a time, as symbols and markers, or thousands as one pictogram — plus PNG pictures with alt text and labels that follow arcs. Everything travels inside the published chart.
lede: Icons are vector drawings painted with your theme's inks, pictures are PNGs fitted to a box, and text can follow an arc. All three are read the same way on every platform, and a published chart carries what it draws, so no reader's device fetches an icon.
---

## An icon in one line

```ts
import { icon } from "@datars/sdk";

icon("heart", [40, 40], { size: 24, ink: "$accent", label: "Liked" })
```

`icon(name, at, options)` draws an icon centred on `at` (or hung from it by `anchor`), `size` px on its longer side, in `ink`. It's an ordinary node: give it a `key`, a `when`, a `transform`, `semantics`. It moves and fades like any shape.

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-icon-sources" data-phone-ratio="1.2" data-caption></div></div><figcaption>Six sources, then the same six repainted through their inks alone. <a href="{{src}}/site/figures/sdk/icon-sources.ts">Figure source</a></figcaption>{{alt:sdk-icon-sources}}</figure>

| Option | What it does |
|---|---|
| `size` | the side of the box the icon fits in, px (default: the theme's `size.icon`, 16, which grows with the reader's text size) |
| `ink` | the colour of `currentColor`, and of a file drawn in one colour (default `$ink`) |
| `inks` | inks for a multi-colour file's other colours, by slot |
| `rotate` | degrees clockwise about `at` |
| `anchor` | where `at` is on the box: `center` (default), `top`, `bottom`, `left`, `right`, `top-left`, … |
| `label` | what the icon stands for, read by screen readers (without it the icon is decorative) |

## Where icons come from

| Name | Source |
|---|---|
| `"heart"`, `"lucide:heart"` | the default set: [Lucide](https://lucide.dev) 1.49.0, 2,121 icons, ISC licence |
| `"$icon.warning"` | a theme icon token (see [Themes](/docs/theming/#icons)) |
| `"icons/logo.svg"` | an SVG in your project, relative to the document |
| `"https://example.com/logo.svg"` | an SVG at a URL, fetched once when the chart is built |
| `"<svg …>…</svg>"`, `{ svg }` | markup in the document |
| `{ d: "M0 0L10 5L0 10Z", viewBox: [0, 0, 10, 10], stroke: 2 }` | path data: filled in the ink, or stroked `stroke` units wide |

Find an icon by what it shows:

```sh
datars icons car          # car, car-front, car-taxi-front, …
datars icons person       # user, users, person-standing, …
```

A name the set doesn't have is a diagnostic in `datars check`, never a silent blank.

## Colours are inks

An icon never keeps the colours its file was drawn in — a theme or a dark mode would leave them behind.

- **One colour.** `currentColor`, and a file drawn in a single colour (most icon sets' black), take the node's `ink`.
- **Several colours.** A logo's other colours become **slots**, numbered in the order the file first uses them. Pass an ink for each in `inks`; slots you leave out take `$categorical[0]`, `[1]`, ….

```sh
datars icons --slots assets/badge.svg
# assets/badge.svg: 3 layers in a 48 × 48 box
#   inks[0]  was #1d4e89
#   inks[1]  was #ffffff
#   inks[2]  was #f5b53d
```

```ts
icon("assets/badge.svg", [24, 24], { size: 32, inks: ["$ink", "$paper", "$highlight"], label: "Fernmouth Transit" })
```

Gradients and patterns in a file draw as one slot each.

## What SVG is read

datars reads SVG with its own small reader, so every platform draws a file the same way. The reader runs in the tools (`datars publish`, `dev`, `render`): a published chart carries its icons already read, and the web runtime doesn't download a reader for them. It reads the parts icons and logos are made of:

- shapes: `path`, `rect` (with `rx`, `ry`), `circle`, `ellipse`, `line`, `polyline`, `polygon`
- structure: `g`, nested `svg`, and `use` or `symbol` of an element in the same file
- `transform` (matrix, translate, scale, rotate, skewX, skewY), `viewBox`, or `width` and `height`
- paint: `fill`, `fill-rule`, `fill-opacity`, `stroke`, `stroke-width`, `stroke-opacity`, line caps and joins, `opacity`, `display`, `visibility`
- these as attributes, in a `style` attribute, or from a `<style>` sheet of simple rules (tags, classes, ids)

It refuses, with a message, anything that runs code or reaches outside the file: scripts, `on…` attributes, `<foreignObject>`, embedded pictures (`<image>`), XML entity declarations, and `href`, `url(…)` or `@import` that point anywhere but the same file. It skips, with a note in `datars check`, what icons rarely need: text (convert it to outlines in your editor), clip paths and masks, filters, dashes and animation.

## Symbols, markers and thousands of icons

An icon works anywhere a symbol does:

```ts
// A scatter of stars, in each point's fill.
shape(geom.symbol({ symbol: "lucide:star", x: e("scale.x(d.x)"), y: e("scale.y(d.y)"), size: 8 }), { fill: "$accent" })

// A plane at the end of a route, turned along it.
shape(geom.path(route), { stroke: { paint: "$ink", width: 1.5 }, markers: { end: { type: "icon", icon: "plane", size: 18 } } })
```

The built-in symbol names — `circle`, `square`, `diamond`, `triangle`, `cross`, `star` — stay symbols; say `lucide:star` for the icon.

For many of one icon — a pictogram's people, a unit chart, map markers — draw them as **instances**: one node however many there are, the drawing prepared once and repeated.

```ts
instances({ from: "units", icon: "user", x: e("d.x"), y: e("d.y"), r: 10, fill: e("scale.color(d.group)") })
```

`r` is half the icon's box. Each row's `fill` paints the icon's `currentColor`. In SVG exports the drawing is written once and placed with a `<use>` per instance.

## Pictograms

The standard library's [`pictogram`](/docs/std/pictogram/) draws an icon per unit: per person, per thousand homes (`per`). It lays out a row per category with its name and value, or a grid like a waffle with a legend of the icons. Units are keyed `(category, Unit i)`, so a bar chart splits into its icons and back.

```ts
pictogram({
  data: "trips", value: "thousands", category: "mode",
  icons: { Car: "car", Bus: "bus", Train: "train-front", Bike: "bike", Walk: "footprints" },
  per: 2, unit: "k",
})
```

Legends take icons too (`legend({ scale: "color", icons })`), and so do callouts (`annotate({ …, icon: "map-pin" })`).

## Pictures

```ts
import { image } from "@datars/sdk";

image("assets/valley.png", [0, 0, 240, 150], { fit: "cover", alt: "A valley with a lake under the sun" })
```

`image(src, rect, options)` draws a picture — a PNG, JPEG or WebP file next to the document, or an `https://` URL — into `rect` (`[x, y, w, h]`).

| `fit` | |
|---|---|
| `contain` (default) | all of it, its aspect kept, centred |
| `cover` | fills the rect, cut to it |
| `fill` | stretched to the rect |
| `none` | its own size, centred, cut to the rect |

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-image-fit" data-phone-ratio="1.6" data-caption></div></div><figcaption>One picture, four fits. The dashed frame is the rect. <a href="{{src}}/site/figures/sdk/image-fit.ts">Figure source</a></figcaption>{{alt:sdk-image-fit}}</figure>

Each platform decodes pictures with its own decoder: the browser on the web, ImageIO on iOS and macOS, BitmapFactory on Android, and Rust's PNG and JPEG decoders in the tools. A PNG is lossless, so it shows the same pixels everywhere. A JPEG or WebP can differ by a shade between platforms, so use PNG where a picture must match exactly. The tools read PNG and JPEG; a WebP file draws in browsers and apps, but `datars render` and a bundle's static fallback leave it out and say so.

## Text along a path

```ts
text("Neutral", [0, 0], {
  path: { arc: [cx, cy, r, a0, a1] },   // or { d: "M … C …" }
  style: { size: "$size.label", align: "middle", baseline: "middle" },
})
```

A text with a `path` follows it: an arc (`[cx, cy, r, a0, a1]`, angles in radians clockwise from 12 o'clock, as `geom.arc`) or any SVG path data. Each letter sits on the path, turned with it. `offset` says where along the path the text's `align` point goes, as a share of its length. On a stretch that runs right to left, like the bottom of a circle, the text still reads left to right (`upright: false` to turn that off).

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-text-path" data-phone-ratio="0.8" data-caption></div></div><figcaption>Labels over the top of a ring and under it, both left to right, and a sentence along a wave. <a href="{{src}}/site/figures/sdk/text-path.ts">Figure source</a></figcaption>{{alt:sdk-text-path}}</figure>

A text longer than its path is left out, not squeezed or run past the end, and `datars labels` reports it. Measure first and give way. The [gauge](/docs/std/gauge/) does: it writes each band's name along its arc (`names`) at the reader's text size, up to what the band holds, and leaves out a name its band can't fit.

## How they travel

Icons and pictures travel with a published chart, like its fonts:

- **Fetched when the chart is built.** `datars publish`, `bundle`, `render` and `dev` read the default set from the toolchain, project files from disk, and URLs once into a cache. A reader's device never fetches an icon or a picture.
- **Read when the chart is built.** Icons travel already read, as compact drawings. A document you hand the web runtime directly (`<datars-view doc>`) gets the default set's icons one small file at a time from beside the runtime; for your own SVG files, serve it with `datars dev`, or write an icon's compact file with `datars icons --compact logo.svg` and serve that instead.
- **Only what's drawn.** A bundle carries the icons its chart draws, a few hundred bytes each, in one chunk, and each picture as its own file. It never carries the whole set.
- **Licences in the manifest.** The bundle's manifest lists the icon set, its version and licence (Lucide: ISC) and the icons drawn. Your own SVGs and pictures are listed by where they came from, and the build reminds you that it has no licence on record for them.

Until an icon arrives in a preview, nothing is drawn in its place, so nothing jumps when it appears.

## Accessibility

- **Decorative unless labelled.** An icon or picture without a `label` (or `alt`, or `semantics`) is left out of what screen readers read: most icons repeat what the text beside them says. Give one a label when it stands for a word on its own, like an up arrow that means "Rising".
- **Contrast.** `datars lint` checks every labelled icon's ink against what's behind it, and warns under 3:1 ([WCAG 1.4.11](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html)). Decorative icons aren't held to it.
- **Text size.** Icons drawn at the theme's `size.icon` grow with the reader's text size, like labels. Pictograms wrap their rows before their icons shrink past legibility.
- Pictograms read a line per category ("Car: 41k"), not a line per icon.

## Limits

- The tools read PNG and JPEG pictures, not WebP. Browsers and apps draw all three.
- Icons inside a map's tile layers aren't drawn yet. Use instances over the map's points.
- SVG text, clip paths, masks, filters and dashes aren't read (see above).
- A picture shrunk to a small fraction of its size can shimmer: export it near the size it's shown.

## See also

- [Nodes: icon and image](/docs/sdk/nodes/#icons-and-pictures) in the SDK reference
- [Themes: icon tokens](/docs/theming/#icons)
- [pictogram](/docs/std/pictogram/), [gauge](/docs/std/gauge/), [legend](/docs/std/legend/), [annotate](/docs/std/annotate/)
