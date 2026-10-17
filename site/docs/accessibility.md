---
title: Accessibility
description: What screen readers, keyboards, text size, contrast and reduced-motion settings get from a datars chart on the web, iOS, Android and desktop — per platform, with what has been verified and how, a WCAG 2.2 mapping, how to write and check accessible charts, and the known gaps.
lede: Every mark can say what it means. The runtime turns that into what each platform's assistive technology understands — so a screen reader hears the chart's content, not "image" — and lays the chart out again for the reader's text size, contrast and motion settings.
---

This page is the project's accessibility statement: what a datars chart does for readers who use assistive technology or change their settings, on each platform, how much of it has been verified and how, and what is still missing. The [accessibility feature page](/features/accessibility/) shows it live on one chart.

## What a reader gets

Every host first names the chart — its `title` — and says what it shows: its `description`, then how much data it draws and the range, from the state on screen (*"Kilograms of each fruit a stall sold. 3 data points, from 18 to 42."*). Then the content, in one reading order:

- headings for the chart's parts (a dashboard's panels, a plot's title);
- an axis or a legend as one line (*"y axis: 0, 10, 20, 30, 40"*; the ends of a long one);
- each data mark with its name and its value apart (*"Housing"*, *"10,240"*), marks repeated in a row as one with a count (a parliament's *"S × 107"*), and marks without a value — a world map's countries without data — as one line (*"215 regions without data"*) rather than a stop each;
- notes and annotations as text;
- whatever a click acts on as a button, and the standard library's controls as what they are: sliders adjustable, selects as the platform's own picker, checkboxes, switches and segments with their state.

Here is the tree that comes from, for a bar chart — `datars semantics`, run when this page was built:

{{run:semantics examples/votes/doc.json|24}}

The words a host adds around the chart's own — the summary, *"Step 2 of 4"*, *"… without data"*, captions — follow the document's `locale`: English and Swedish so far (*"8 datapunkter, från 3,5 till 27,6"*, *"Steg 2 av 4"*); other locales say them in English.

## Platforms: what each gets, and what has been verified

| Platform | What the reader gets | Verified how |
|---|---|---|
| Web (`<datars-view>`, React, Vite, Next) | A focusable `figure` named by the chart; first inside it the summary; then named groups, each run of data marks a `<table>` (name and value columns), axes and legends as a line, native `<input type="range">` and `<select>` for engine-drawn sliders and selects, `role="checkbox" / "switch" / "radio"` with `aria-checked`, buttons for clickable marks, real links over linked text. The canvas is hidden from screen readers; chart text lies over it as selectable, findable text, also hidden from them (it's in the content already). A polite live region announces each story step once. | Chrome's accessibility tree, by browser tests in the repository (name, summary, tables, roles, the keyboard path, the live region, contrast and forced colours); a windowed Chrome read through the macOS Accessibility API — what VoiceOver gets. **Not yet** listened to with VoiceOver, NVDA, JAWS or TalkBack driving it; Safari and Firefox not checked. |
| iOS, iPadOS (DatarsKit) | VoiceOver elements with their frames, so explore-by-touch finds a bar where it's drawn: the chart first (a summary element), its controls (sliders adjustable, saying *"32,000"*; selects as a menu; switches with the toggle trait), headings in the rotor, data marks with their values; double-tap runs a clickable mark; a new step is announced. | The elements dumped in the iOS simulator (iPhone 16 Pro). **Not yet** driven by VoiceOver, nor on a device. |
| macOS (DatarsKit) | The chart a group with its summary as help, the reading order as its children: check boxes, radio buttons and the switch subrole with their values, buttons that press; steps announced. | Built and unit-tested; **not yet** read with VoiceOver. |
| Android | TalkBack virtual nodes through an `AccessibilityNodeProvider`, in the same order, with screen bounds: headings, values in the content description, `SeekBar` sliders, a `Spinner` per select, `CheckBox`, `Switch` and `RadioButton` nodes with their state, buttons; a new step is announced. | The node tree dumped with `uiautomator` on an emulator (API 35). **Not yet** driven by TalkBack, nor on a device. |
| Desktop viewer (`datars-view`) | AccessKit (VoiceOver, Narrator/NVDA, Orca): a figure named by the chart with its summary as description, sliders with their drawn value, check boxes, switches and radio buttons with their state, buttons for what a click acts on. | Unit tests of the tree and its actions; **not yet** with a screen reader. |
| SVG poster, PDF | The poster is labelled (title, description, roles); a PDF keeps its text as searchable text. Every bundle's text alternative (title, description, each step's narration and data) is readable without any script. | Rendered and inspected at build time; not checked with a screen reader. |
| Video (`datars video`) | WebVTT captions from each step's narration beside the MP4, and chapters per step. There is no audio description track. | Generated and checked for timing. |

The project's [status page]({{src}}/docs/19-status.md) tracks the gaps in the last column.

## Reader settings the charts follow

Each is the platform's own setting by default, and each can be overridden per chart. On the web:

| Setting | Followed | Override |
|---|---|---|
| **Text size** | the browser's default font size (Settings › Appearance › Font size; 20 px is 1.25 ×), live — not the page's own root size | `text-scale="1.5"` or the `textScale` property (React `textScale`); `auto` follows the reader again |
| **Reduced motion** | `prefers-reduced-motion`, live | `reduced-motion="reduce"` or `"no-preference"` |
| **More contrast** | `prefers-contrast: more` → the high-contrast mode, live | `mode="light"`, `"dark"` or `"high-contrast"` |
| **Forced colours** | Windows contrast themes (`forced-colors: active`): text, rules and surfaces in the system's colours, data colours kept, live | — |
| **Dark mode** | `prefers-color-scheme`, live | `mode` |

In the native hosts:

| Setting | iOS / iPadOS, macOS | Android | Desktop viewer |
|---|---|---|---|
| **Text size** | Dynamic Type, live (≈ 0.8 × at xSmall to 3.1 × at the largest accessibility size); macOS has no system-wide setting. Override: `view.textScale` | Settings › Display › Font size (`Configuration.fontScale`). Override: `view.textScale` | Ctrl/⌘ +, − and 0, or `--text-scale 1.5` |
| **Reduced motion** | Reduce Motion, live. Override: `engine.setReducedMotion` | Remove animations (animator scale 0), read at creation | — |
| **More contrast** | not automatic yet: `engine.setMode(.highContrast)` | not exposed yet | — |
| **Dark mode** | the app sets `mode` | `setDarkMode` | `d` toggles |

How text size works: the theme's text sizes (`size.small`, `size.label`, `size.body`, `size.title`) grow by the reader's factor (kept between 0.75 and 3), and the chart lays out again around them — axes widen, numeric axes ask for fewer ticks, category names wrap or turn, legends and controls grow their rows and targets, tables shrink their text only as far as their columns need, labels that would still pile up give way — and moves to the new layout like any other change. Size classes are judged in the reader's text (an 800 px chart at 2 × lays out as a phone one). Content of fixed size that the bigger text makes taller than the chart's box grows the element on the web rather than running off the bottom or scrolling inside the page; at the theme's size the page's height always holds. Text a document sizes in pixels stays as written; posters and videos are at the theme's size.

## Keyboard

On the web a chart is one focusable figure. With it focused:

| Key | Does |
|---|---|
| <kbd>→</kbd> or <kbd>Space</kbd> | next story step |
| <kbd>←</kbd> | previous step |
| <kbd>Esc</kbd> | back out of a drill-down chapter |
| <kbd>Tab</kbd> | on to the story's Previous and Next, the chart's controls (sliders, selects, checkboxes, switches, buttons), then into its data — one stop for all of it |
| <kbd>↑</kbd> <kbd>↓</kbd> (or <kbd>←</kbd> <kbd>→</kbd>), <kbd>Home</kbd>, <kbd>End</kbd> | in the data: the previous or next mark, the first or the last; the mark is outlined on the chart and named beside it, as a pointer's tooltip would |
| <kbd>Enter</kbd> / <kbd>Space</kbd> | on a clickable mark or a control: its click (select, filter, drill down; tick, flip) |
| <kbd>Shift</kbd>+<kbd>D</kbd> | the frame-stats panel (for developers) |

The outline follows whatever in the chart has the keyboard, in the theme's accent (Highlight under forced colours). Brushes and pannable views have no keyboard path of their own: give the same signal a control.

## Writing accessible charts

The standard library labels everything it draws; these are the author's part:

- **A title and a description for every document.** The title names the chart everywhere; the description is the first sentence of its summary, in the bundle's text alternative, and under every chart on this site. Say what the chart shows and why, not what it looks like.
  ```ts
  doc({
    title: "Renewable share of energy",
    description: "A world choropleth of the renewable share of energy use; the camera flies to Europe and the Nordic countries, which lead, then the countries become a ranked bar chart.",
  })
  ```
- **Narration for story steps**, written as you'd write alt text: what the reader should notice. Each step's `title` and `text` are announced when the reader gets there.
  ```ts
  step("europe", { set: { focus: europe }, title: "Europe", text: "Across Europe the share ranges from 13% to 85%." })
  ```
- **The `locale`** of the document (`locale: "sv-SE"`): numbers, dates and the hosts' own words follow it.
- **Semantics on marks you draw yourself.** A role, a label with the formatted value, the value itself:
  ```ts
  shape(geom.circle({ cx: e("scale.x(d.region)"), cy: e("scale.y(d.sales)"), r: 5 }), {
    key: e("d.region"),
    semantics: { role: "datum", label: e("`${key.name(d.region)}: ${format(d.sales, ',.0f')}`"), value: e("d.sales") },
  })
  ```

  | Field | Meaning |
  |---|---|
  | `role` | `group`, `datum`, `series`, `region`, `axis`, `tick`, `grid`, `legend`, `legend-item`, `annotation`, `title`, `label`, `tooltip`, `control`, `decoration` |
  | `label` | what it's called — a string or an expression over the row; for a data mark, `name: value` (hosts read the two apart) |
  | `value` | its number; a value that comes out empty marks it as *without data* |
  | `link` | a URL: the web runtime lays a real link over it, SVG and PDF keep it |
  | `kind`, `checked` | a control's native kind — `checkbox`, `switch` or `radio` — and its state; the label stays the name |

  `role: "decoration"` keeps what carries no data — gridlines, backgrounds, orbits drawn for effect — out of the reader's way. Many recipes take a `label` parameter for their marks' names (`bar({ label: e("…") })`); `cloud({ name: "4,000,000 stars" })` describes a point cloud as one item.
- **Text sized with the theme's tokens** (`"$size.label"`, `token("size.label")`), and sizes and offsets that depend on text written from them, so the reader's text size reaches them. A literal `size: 12` stays 12. Recipes that need a constant at the theme's size can multiply it by `max(1, textScale)`.
- **Labels with somewhere else to go**: `text(…, { alternatives: [{ at, align, baseline }, …] })` lets the engine move a label off the data (a line through it) or other text, and gives it a halo where nowhere is clear; `style: { contain: "wrap" }` wraps a label at the canvas edge instead of pushing it over what it labels.
- **Don't rely on colour alone.** Label data directly where you can (`bar({ labels: true })`, `line({ labels: true })`); keep categorical colours to eight or fewer.

## Checking your charts {#checks}

| Command | Tells you |
|---|---|
| `datars semantics doc.ts --state 2` | the semantics tree of a state — read a chart the way a screen reader will (agents too, over MCP) |
| `datars labels doc.ts` (or a folder) | every state at a phone (390 px), a tablet (768) and a wide column (1200), each at 1, 1.5 and 2 × text — as the web shows it, grown where bigger text needs it — and the labels a reader couldn't read: cut off at the edge, overlapping, clipped by their box, or on the data without a halo. `--widths`, `--text-scale`, `--verbose`, `--json` |
| `datars render doc.ts --size 390x600 --text-scale 2` | one state as a reader with big text on a phone gets it |
| `datars lint doc.ts --size 390x600` | the rules below, in the layouts of the boxes given |
| `datars test` | in this repository: the examples' goldens, and their unreadable labels per reading as a ratchet (`tests/golden/<name>/labels.json`) — a chart getting harder to read fails |

| Lint rule | Catches |
|---|---|
| `a11y/semantics` | interactive elements without an accessible label |
| `theme` | text tokens below their contrast minimum; palettes whose colours collapse under colour-vision deficiencies |
| `legibility/label-collisions` | labels that overlap |
| `legibility/offscreen` | labels cut off at the canvas edge |
| `legibility/clipped` | labels their box's clip cuts |
| `legibility/on-data` | labels a line of a mark runs through, or straddling a filled mark's edge, without a halo or an ink chosen for the fill |
| `encoding/too-many-colours` | more categorical colours than a reader can tell apart |

Here is `datars labels` over this repository's examples, at a phone's and a tablet's width, at this build:

{{run:labels examples --widths 390,768 --text-scale 1,1.5,2|36}}

## WCAG 2.2

How the success criteria that apply to a chart are met — by the runtime, or by you. "Helps" means datars gives the means; the result depends on the document.

| Criterion (level) | datars | You |
|---|---|---|
| 1.1.1 Non-text content (A) | A name and summary for every chart, every labelled mark in the content, a text alternative in every bundle | a meaningful `title`, `description` and mark labels |
| 1.2.2 Captions, prerecorded (A) | `datars video` writes WebVTT captions from narration | narration that says what matters |
| 1.3.1 Info and relationships (A) | groups, headings, data tables with name and value columns, axes, legends, native control roles | semantics on marks you draw |
| 1.3.2 Meaningful sequence (A) | one reading order on every host | the order of your scene follows the reading order you want |
| 1.4.1 Use of colour (A) | helps: palettes checked under colour-vision deficiencies, direct labels in std | label series, don't encode by colour alone |
| 1.4.3 Contrast (minimum) (AA) | theme checks: text tokens against their backgrounds in every mode | inks you choose outside the theme |
| 1.4.4 Resize text (AA) | the reader's text size re-lays the chart out (0.75–3 ×), on every host | size text with the theme's tokens |
| 1.4.10 Reflow (AA) | layouts for phone, tablet and wide; at 320 px the examples have no unreadable labels | test at narrow widths (`datars labels --widths 320`) |
| 1.4.11 Non-text contrast (AA) | rules, axes and the focus outline in theme tokens checked for contrast; high contrast and forced colours | data colours you set |
| 1.4.12 Text spacing (AA) | not applicable to drawn text; the content screen readers get is plain HTML | — |
| 1.4.13 Content on hover or focus (AA) | tooltips go when the pointer leaves, on a scroll or a tap elsewhere, and show the same label the content has | — |
| 2.1.1 Keyboard (A) | stories, controls, clickable marks and the data are reachable and operable | a keyboard path for brushes and pans (a control on the same signal) |
| 2.2.2 Pause, stop, hide (A) | autoplay pauses off screen and for reduced motion; the reader can step | don't rely on autoplay alone |
| 2.3.3 Animation from interactions (AAA) | reduced motion everywhere: short crossfades | — |
| 2.4.3 Focus order (A) | figure, story buttons, controls, data, in order | — |
| 2.4.7 / 2.4.11 Focus visible, not obscured (AA) | an outline on the chart for what has the keyboard | — |
| 2.5.8 Target size (minimum) (AA) | std controls are at least 24 px tall and grow with text | marks you make clickable |
| 3.1.1 / 3.1.2 Language (A / AA) | the document's `locale` reaches numbers and the hosts' words | set `locale` |
| 4.1.2 Name, role, value (A) | native roles and states on every host | — |
| 4.1.3 Status messages (AA) | story steps announced through a live region (and the native equivalents) | narration |

This mapping is the project's reading of the criteria against what the runtime does; it hasn't been audited by a third party.

## Known limitations

- **No screen reader has driven these yet.** The web tree is checked in Chrome and through the macOS Accessibility API, the native trees in the simulator and emulator; nobody has yet listened to VoiceOver, TalkBack, NVDA or JAWS read them, and real devices, Safari and Firefox are untested.
- **Brushes and pannable views have no keyboard path of their own**; give the same signal a control.
- **The live region speaks a step's narration, not the result of a click**; the control or button says its new state instead.
- **Long lists stop at 500 marks** on the web (200 instances listed per scene by the engine); the summary still counts them all.
- **Text size follows the theme's sizes only**; literal pixel sizes don't scale, posters and videos are at the theme's size, and iOS and Android apps must give a chart the height bigger text needs themselves (`contentHeight` in the status says how much).
- **More contrast** is automatic on the web only; native apps set `mode` themselves. Forced colours are a web feature.
- **Two languages** for the hosts' own words (English, Swedish); the summary's phrasing is generic ("data points", "regions").
- **At 2 × on a phone some charts still crowd**: a few labels overlap or are cut in dense networks and multi-panel figures (`datars labels` lists them).

## See also

- [Accessibility feature page](/features/accessibility/) — the lab: each of the above on one chart.
- [Web embedding](/docs/embed/web/) — `text-scale`, `mode`, `reduced-motion`, `readout`.
- [iOS and macOS](/docs/embed/ios/), [Android](/docs/embed/android/) — `textScale`, VoiceOver, TalkBack.
- [Stories](/docs/stories/) — narration.
- [Preview, check and debug](/docs/tools/) — `datars labels`, `lint`, `semantics`.
