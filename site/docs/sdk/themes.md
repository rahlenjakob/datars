---
title: Themes
titleTag: Themes — SDK reference · datars docs
description: theme(), ThemeDef, font tokens and the font helpers of @datars/sdk, and the ink syntax every colour in a scene uses — with a live swatch figure that follows the page into dark mode.
lede: Scenes never hold theme colours. A colour is an ink — `"$accent"`, `"$categorical[3]"`, `"#e8112d"` — resolved when each frame is drawn, against the theme, the reader's mode (light, dark, high contrast) and any overrides the host sets at runtime.
---

## Inks

Anywhere a colour goes — `fill`, `stroke.paint`, `style.ink`, a halo, a backdrop — you write an ink:

| Ink | Is |
|---|---|
| `"$accent"` | a colour token of the theme |
| `"$categorical[3]"` | a palette entry by index |
| `"$sequential~0.42"` | a position along a palette used as a ramp |
| `"$accent@0.4"` | a token with alpha |
| `"on($accent)"` | the theme's ink or paper, whichever reads better on that colour (text on a coloured mark) |
| `"#e8112d"` | a literal colour, the same in every mode |

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-theme-inks" data-phone-ratio="1.17" data-caption></div></div><figcaption>The built-in theme's tokens, palette, ramps and ink forms. Switch the page between light and dark: every swatch but the literal follows. <a href="{{src}}/site/figures/sdk/theme-inks.ts">Figure source</a></figcaption>{{alt:sdk-theme-inks}}</figure>

Colour scales hand out inks too (`scale.c(v)`), and numbers and fonts are tokens as well: `size: "$size.label"`, `font: "font.title"`, `token("radius.card")` in an expression.

## Themes

{{sdk:theme}}

`theme(def)` returns the definition as it is — it's there for the types. Pass it as the document's `theme` (`doc()` registers and uses it), or publish it for hosts to apply.

{{sdk:ThemeDef}}

```ts
import { font, theme } from "@datars/sdk";

export const brand = theme({
  name: "acme",
  extends: "datars/neutral",
  tokens: {
    accent: "#b3261e",
    ink: "#1b1b1f",
    "ink-2": "mix($ink, $paper, 0.25)",
    categorical: { kind: "categorical", colors: ["#b3261e", "#1d4e89", "#e0a100"] },
    "font.title": font.google("Source Serif 4", { weight: 600 }),
  },
  modes: { dark: { accent: "#f2b8b5", paper: "#141218" } },
  locked: ["accent"],
});
```

Tokens are colours (literals, other tokens, colour expressions like `mix(…)`), palettes, numbers, fonts and text; `modes` override them for dark and high-contrast; `locked` tokens can't be changed by a later layer (the document's tokens, a host's `setTokens`); `checks` are contrast and palette rules `datars lint` runs in every mode. The built-in theme's tokens are in [themes and brands](/docs/theming/), with the whole theming guide.

## Fonts

{{sdk:FontToken}}

{{sdk:font}}

| Helper | Makes |
|---|---|
| {{sig:font.file}} | a token for a font file next to the document (or an `https://` URL) |
| {{sig:font.google}} | a token for a Google Fonts family, downloaded by the build step; `fallback` families cover characters it lacks |

Fonts travel with the chart: the build acquires each face, subsets it to the chart's text and ships it in the bundle, so text measures and draws the same everywhere — system fonts are never used. For other scripts, a document can also add fallback faces as data: [`data.font`](/docs/sdk/data/#data).
