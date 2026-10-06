# 18 — Themes

Custom themes are a core idea — publishers bring their own look, the engine holds no look-specific
code, organizations lock brand tokens. The idea is simple; the mechanism is where theme systems go
wrong. Implemented in `crates/datars-theme`.

## What it avoids

- A theme declaration that mixes fixed fields (`paper`, `ink`, `ink2`, `muted`, `grid`, a separate
  block of map colours) with a free-form token list and page CSS files.
- Theming chart colours by a **remap hack**: the engine draws with neutral defaults, then swaps
  known default colours for theme colours.
- Dark mode as a separate theme, with no modes.
- Untyped string tokens, so mistakes surface as wrong pixels.
- Locks that exist only at merge time; page CSS generation in the core IR.
- Theme changes that can't animate, or can't be applied by a host app to charts it downloaded.

## The model

A **theme** is a JSON file (or an SDK object) of typed **tokens**:

| Token type | JSON shape | Examples |
|---|---|---|
| Colour | a colour expression string | `"#b3261e"`, `"$accent"`, `"mix($ink, $paper, 0.9)"`, `"on($accent)"` |
| Palette | array, `{kind, colors}`, or `{generate, from, to?, n}` | `categorical`, `sequential`, `diverging` |
| Number | number | `size.label: 11`, `dim: 0.28`, `motion.duration: 0.9` |
| Font | `{family, weight, italic, src \| google}` — see [Fonts](#fonts) | `font.title` |
| Text | any other string | `motion.easing: "cubic-in-out"` |

```json
{
  "name": "acme",
  "extends": "datars/neutral",
  "tokens": {
    "brand": "#b3261e",
    "accent": "$brand",
    "paper": "#fbf8f1",
    "categorical": { "generate": "categorical", "from": "$brand", "n": 8 },
    "font.title": { "google": "Source Serif 4", "family": ["Inter"], "weight": 700 }
  },
  "modes": {
    "dark": { "paper": "#15130f", "ink": "#f2efe6" }
  },
  "locked": ["brand", "accent", "font.title"],
  "checks": [{ "contrast": ["ink", "paper"], "min": 7 }]
}
```

### Colour expressions — set a few tokens, derive the rest

`mix(a, b, t)` (OKLab), `alpha(a, x)`, `lighten(a, d)` / `darken(a, d)` (OKLCH lightness),
`saturate` / `desaturate`, `oklch(l, c, h°)`, `rgb(r, g, b)`, `on(bg)` (black or white, whichever
reads better), `contrast(bg, a, b)`, references `$token` and `$palette[i]`. The neutral theme derives
`grid`, `muted`, `rule`, `surface` and `accent-ink` from `ink`, `paper` and `accent`, so a brand that
sets three colours gets a coherent theme, and dark mode only has to change the surfaces.

Semantic colours are tokens too: `positive` and `negative` (a waterfall's rises and falls), and
`up` and `down` for prices (candles, volume, sparklines), which default to them. Markets that read
red as up (China, Japan, Korea) swap `up` and `down` in their theme without touching the rest.

### Fonts

A font token names the family text is drawn with (then its fallbacks), its weight and style —
and **where the face comes from**, so the build step can ship it with the chart. Text measures and
renders the same on every runtime because fonts travel with charts; system fonts are never used.

```json
"font.title":  { "family": "Cambon", "weight": 700, "src": "fonts/Cambon-Bold.otf" },
"font.body":   { "google": "Source Serif 4", "weight": 400 },
"font.strong": { "google": "Source Serif 4", "weight": 600, "italic": true },
"font.number": { "family": ["Brand Sans", "Inter"], "src": "https://cdn.example.com/BrandSans-Regular.ttf" }
```

| Key | Meaning |
|---|---|
| `family` | a family, or a stack (`["Brand Sans", "Inter"]`: later families supply what earlier ones lack). Optional with `google`, which names the first family |
| `weight` | 100–900, default 400: which face (static instance) of the family this token is |
| `italic` | the italic face |
| `src` | the face's file — a path relative to the document (like data URLs), an `https://` URL, or `datars:fonts/…` (the default fonts that ship with the toolchain and runtimes). TrueType/OpenType (`.ttf`, `.otf`, `.ttc`) or WOFF; WOFF2 isn't read yet |
| `google` | a [Google Fonts](https://fonts.google.com) family |

In TypeScript: `font.file("Cambon", "fonts/Cambon-Bold.otf", { weight: 700 })`,
`font.google("Source Serif 4", { weight: 600, italic: true })` (`FontToken` in `@datars/sdk`).

- **One token, one face.** The file (or Google instance) *is* that family's face at that weight:
  it's registered under the token's names, whatever its own name table says. Every face a token
  declares serves all text, so a token that only names a family (`"font.strong": { "family":
  "Cambon", "weight": 600 }`) uses the nearest weight another token sourced. Use static
  instances: a variable font draws at its default instance.
- **Acquisition happens at build time only** — `datars bundle`, `publish`, `render`, `check`,
  `dev` (and the MCP tools): project files are read from disk; Google Fonts are downloaded once
  from the css2 API (static TrueType instances, no API key) and URLs once, both into a disk cache
  (`$DATARS_CACHE`, else `~/Library/Caches/datars`, `~/.cache/datars` or `%LOCALAPPDATA%\datars`,
  under `fonts/`; `DATARS_OFFLINE=1` answers from the cache only). **No runtime ever calls Google
  or a font CDN**: published bundles carry subsets of every face (docs/12-delivery.md §Fonts).
- **Inter is an ordinary default.** The built-in `datars/neutral` theme's tokens are
  `{ "family": ["Inter"], "weight": 400, "src": "datars:fonts/Inter-Regular.ttf" }` and friends;
  a theme that sets its own font tokens doesn't ship Inter at all.
- **No silent fallback.** A family with no face and no source is a diagnostic in `datars check`
  and `lint` — `font 'GT America' (font.title) has no source; falling back to Inter` — and so is
  a source that couldn't be loaded. `datars fonts doc.ts` lists every token with its source, the
  face drawn and its licence.
- **Licences.** Bundles record each font's licence as its own name table states it (`OFL-1.1`,
  `Apache-2.0`, `UFL-1.0`, or `unknown`, which the build warns about) in the manifest's `fonts`,
  and subsets keep the copyright, trademark and licence records. A font whose OS/2 `fsType` says
  *restricted-licence embedding* is flagged by `lint` and the build; *no subsetting* ships it
  whole. OFL fonts with a Reserved Font Name (Adobe's Source family reserves "Source") treat
  modified copies — subsets included, on a strict reading — as needing another name; check the
  licence before shipping such a font subset.
- **Other scripts** (Hebrew, Arabic, CJK labels) come from document fonts —
  `data.hebrew = { "font": "fonts/NotoSansHebrew.ttf" }` (or `"google:Noto Sans Hebrew"`) — whose
  families join every text's fallback chain.

### Modes

`light` (default), `dark`, `high-contrast` live inside one theme. Hosts select the mode from platform
preferences (`color_scheme`, `contrast` signals). Resolution applies every layer's base tokens first,
then every layer's mode tokens, so a derived theme that only sets `brand` keeps the base theme's tuned
dark surfaces, while derived tokens (`grid = mix($ink, $paper, 0.9)`) re-derive per mode.

A brand that is dark everywhere (a magazine with a black page) says `"scheme": "dark"`: its base
tokens are dark values, and in light mode everything beneath it resolves as in dark mode — so the
tokens it doesn't set (map roads and parks, the sequential ramp, `surface`) are the parents' tuned
dark ones rather than light ones on a black page.

### Layers and locks

```
datars/neutral (base → its mode)  →  each extending theme (base → its mode)  →  document overrides  →  host app overrides
```

Each theme applies its base tokens and then its own block for the active mode, in chain order — the
CSS cascade. A brand that sets `accent` keeps it in dark and high-contrast mode unless it gives a
mode value too; when a child's token shadows one its parent varies by mode (a light `paper` without
a dark one), resolution says so, token by token. `datars theme brand.json --specimen out.png`
renders every std chart, the palettes, ramps and inks in all three modes side by side.

A layer can **lock** tokens; no later layer can change them (the override is ignored and reported).
This is the guardrail for org → project → story brands, as a property of the resolution itself
rather than a merge-time check. A hosted editor can use it for organization themes; the engine
doesn't care who the layers belong to.

### Checks

Themes declare checks, evaluated on the resolved theme by `datars lint` and by theme editors: WCAG
**contrast** of text tokens on their backgrounds, **distinctness** of palettes (minimum pairwise ΔE,
also under simulated deuteranopia, protanopia and tritanopia), **monotone** lightness for
sequential ramps. The built-in neutral theme passes its contrast checks in all three modes (a test).

## Late binding — why the execution is different

Scenes store colours as **inks**: `"#e8112d"`, `"$accent"`, `"$categorical[3]"`, `"$ink@0.4"`. Inks
resolve against the active theme **at frame time**, when the display list is built. Consequences:

- **Switching theme or mode needs no re-resolve** — and it can **animate**: `ResolvedTheme::lerp`
  interpolates every colour in OKLab, so a dark-mode switch is a transition like any other.
- **Downloaded bundles stay themeable** ([12](12-delivery.md)): a baked scene keeps `$accent`, and a
  host app's token overrides restyle it without republishing.
- **Everything engine-drawn is themed the same way** — marks, axes, labels, tooltips, controls, the
  basemap (`map.water`, `map.land`, `map.road`…), motion defaults (`motion.duration`, `motion.easing`).
  No remapping, no CSS.
- **Unknown tokens render magenta** and the linter names them, instead of silently falling back.

Data colours are different from theme colours: a party's colour is *data* (a key → colour mapping in
the document), while the theme supplies the default categorical palette for keys without one.

## What the standard library does

Recipes ask for tokens (`cx.token("accent")`) and get inks back, never literal colours, so every std
chart follows the theme. Recipes declare which tokens they read, so `datars theme --specimen`
(or an editor) can generate a specimen board for a theme automatically from the standard library.

## Hosts

`ResolvedTheme::to_json()` exports the resolved tokens, so a host can style its own chrome (CSS
variables on a page, a SwiftUI palette) from the same source of truth. Hosts pass their own token
layer to downloaded charts (`.tokens(AppTheme.current.datarsTokens)` in [12](12-delivery.md)).
