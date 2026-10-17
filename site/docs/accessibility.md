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
- whatever a click acts on as a button, and the standard library's controls as what they are: sliders adjustable, selects as the platform's own picker, checkboxes, switches and segments with their state;
- what a pointer drags — a brushable plot, a pannable map — as one stop the keyboard and the screen reader's own gestures operate (*"Spot price, range selection: nothing selected"*).

What a click, a tap or a key changed is then said politely — after whatever the reader is hearing — in the same words: the clicked mark's new label, the parts whose label changed, and the data shown when that changed (*"Pears, hidden. Total: 69 kg. Now 2 data points, from 27 to 42."*; *"Selected 1 to 100."*).

Here is the tree that comes from, for a bar chart — `datars semantics`, run when this page was built:

{{run:semantics examples/votes/doc.json|24}}

The words a host adds around the chart's own — the summary, *"Step 2 of 4"*, *"… without data"*, what an interaction changed, captions — follow the document's `locale`: English and Swedish so far (*"8 datapunkter, från 3,5 till 27,6"*, *"Steg 2 av 4"*); other locales say them in English.

## Platforms: what each gets, and what has been verified

| Platform | What the reader gets | Verified how |
|---|---|---|
| Web (`<datars-view>`, React, Vite, Next) | A focusable `figure` named by the chart; first inside it the summary; then named groups, each run of data marks a `<table>` (name and value columns), axes and legends as a line, native `<input type="range">` and `<select>` for engine-drawn sliders and selects, `role="checkbox" / "switch" / "radio"` with `aria-checked`, buttons for clickable marks, one stop for a brush or a pannable view (`role="application"`, its keys described), real links over linked text. The canvas is hidden from screen readers; chart text lies over it as selectable, findable text, also hidden from them (it's in the content already). A polite live region announces each story step once, and what a click or a key changed. | Browser tests in the repository read each engine's own accessibility tree — Chrome (Chromium), Safari's WebKit and Firefox's Gecko, through Playwright — for the name, summary, tables, roles and states, the keyboard path, a brush from the keyboard, the live region, forced colours (and Chrome for more contrast); a windowed Chrome read through the macOS Accessibility API — what VoiceOver gets. **Not yet** listened to with VoiceOver, NVDA, JAWS or TalkBack driving it, nor in Safari itself (WebKit's engine, not the browser). |
| iOS, iPadOS (DatarsKit) | VoiceOver elements with their frames, so explore-by-touch finds a bar where it's drawn: the chart first (a summary element), its controls (sliders adjustable, saying *"32,000"*; selects as a menu; switches with the toggle trait), headings in the rotor, data marks with their values, a brush as an adjustable element; double-tap runs a clickable mark; a new step, and what an action changed, are announced. | The elements dumped in the iOS simulator (iPhone 16 Pro), and VoiceOver's actions (activate, increment, decrement, escape) run on the brush there, each announcement recorded; Increase Contrast and Bold Text switched while the chart ran. **Not yet** driven by VoiceOver, nor on a device. |
| macOS (DatarsKit) | The chart a group with its summary as help, the reading order as its children: check boxes, radio buttons and the switch subrole with their values, buttons that press, a brush or a view that presses, increments and cancels; steps and changes announced; Increase Contrast followed. | Built and unit-tested; **not yet** read with VoiceOver, and Increase Contrast not switched on a Mac. |
| Android | TalkBack virtual nodes through an `AccessibilityNodeProvider`, in the same order, with screen bounds, each a stop for TalkBack's swipes and for a finger exploring the screen: the chart with its summary, the story's Previous and Next step, headings, data marks with their values, `SeekBar` sliders saying their value as drawn, a `Spinner` per select, `CheckBox`, `Switch` and `RadioButton` nodes with their state, buttons, a brush or a view adjustable like a slider; a new step, and what an action changed, are announced. | **TalkBack itself** (15.0, on an Android 15 emulator, Google APIs image), driven with real touches — swipes, double-taps, a touch to explore — and what it spoke read from its log: the first 20 stops of the votes, budget, controls and prices examples, a step, a slider, a switch, a checkbox and the brush ([what it said](#what-talkback-says)). `scripts/test-talkback.sh` repeats it. **Not yet** on a device. |
| Desktop viewer (`datars-view`) | AccessKit (VoiceOver, Narrator/NVDA, Orca): a figure named by the chart with its summary as description, sliders with their drawn value, check boxes, switches and radio buttons with their state, buttons for what a click acts on, a brush or a view with click, increment, decrement and scroll actions, a polite live region for what an action changed. | Unit tests of the tree and its actions; **not yet** with a screen reader. |
| SVG poster, PDF | The poster is labelled (title, description, roles); a PDF keeps its text as searchable text. Every bundle's text alternative (title, description, each step's narration and data) is readable without any script. | Rendered and inspected at build time; not checked with a screen reader. |
| Video (`datars video`) | WebVTT captions from each step's narration beside the MP4, and chapters per step. There is no audio description track. | Generated and checked for timing. |

The project's [status page]({{src}}/docs/19-status.md) tracks the gaps in the last column.

## What TalkBack says

TalkBack 15 on an Android 15 emulator, driven by real touches (`scripts/test-talkback.sh`), each line what it spoke after one gesture; "Heading", "Slider", "Check box" and the hints are TalkBack's own words. Abridged:

```text
votes      open        Vote share by party, Sweden 2022, 8 data points, from 4.6 to 30.3. Heading
           swipe →     Next step. Button. Double-tap to activate
           swipe →     Vote share by party, 2022 (%). Heading
           swipe →     y axis: 0, 5, 10, 15, 20, 25, 30, 35
           swipe →     share by party. Heading
           swipe →     Social Democrats, 30.3
           swipe →     Sweden Democrats, 20.5   … Moderates, 19.1 … Liberals, 4.6
           swipe →     x axis: Social Democrats, Sweden Democrats, Moderates, … Liberals
           swipe →     Note: for the Social Democrats, still Sweden's largest party.
  (on Next step) double-tap   Ranked: eight parties cleared the 4% threshold.
                 double-tap   Each bar becomes a slice.
budget     swipe →     32,000. Monthly income (SEK). Slider
           swipe ↑     33,000. Monthly income (SEK). Slider
           swipe ↑     34,000. Monthly income (SEK). Slider
           swipe ↓     33,000. Monthly income (SEK). Slider
           swipe →     Per month (SEK). Heading  … Housing, 10,240 … Other, 3,840
controls   swipe →     Highlight: Central. Drop down list
           swipe →     0. Sales between, from. Slider
           swipe →     checked. Quarter: Q1. Radio button
           swipe →     Off. Average line. Switch
           double-tap  On.
           swipe →     checked. North. Check box
           double-tap  Not checked. Now 4 data points, from 18 to 36.
prices     touch       nothing selected. Drag across the year to zoom in, range selection. Slider.
                       Double-tap to start a selection or keep it, swipe up or down to extend it
           swipe ↑     1 to 50. Drag across the year to zoom in, range selection. Slider
           swipe ↑     1 to 100. …
           double-tap  Selected 1 to 100. Now 465 data points.
```

The first run found what a dump of the node tree hadn't: TalkBack skipped every item (they weren't marked focusable for it — swipes said nothing past the chart's name), a slider said its raw number first ("32000.0. Monthly income (SEK): 32,000") and nothing when adjusted, a switch didn't say on or off, a toggle wasn't confirmed, the summary ended "10,240.." , there was no way to step the story, the prices chart said "730 data points" for a year (its detail and overview plots each counted every day), and the brush's hint gave keyboard keys. All fixed; the transcript above is after. Still to judge with people who use TalkBack every day: the prices chart's marks say a day number and a price ("1, 149.5") because that is what the example labels them — a document says "Day 1" by labelling its marks so.

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
| **Bold text** | Bold Text (iOS), live: every text drawn heavier — regular as semibold, semibold and bold as bold — and the chart laid out again. Override: `view.boldText` | Bold text (`Configuration.fontWeightAdjustment`, Android 12+), live. Override: `view.boldText` | — |
| **Reduced motion** | Reduce Motion, live. Override: `engine.setReducedMotion` | Remove animations (animator scale 0), read at creation | — |
| **More contrast** | Increase Contrast (iOS and macOS) → the high-contrast mode, live. Override: `view.followsContrast = false`; the app's own look is `view.mode` | Contrast at medium or high (Android 14+), or High contrast text where the system lets apps read it → the high-contrast mode, live. Override: `followsContrast = false` | — |
| **Dark mode** | the app sets `view.mode` | `setDarkMode` | `d` toggles |

How text size works: the theme's text sizes (`size.small`, `size.label`, `size.body`, `size.title`) grow by the reader's factor (kept between 0.75 and 3), and the chart lays out again around them — axes widen, numeric axes ask for fewer ticks, category names wrap or turn, legends and controls grow their rows and targets, tables shrink their text only as far as their columns need, labels that would still pile up give way — and moves to the new layout like any other change. Size classes are judged in the reader's text (an 800 px chart at 2 × lays out as a phone one). When rows are more than the chart's box holds — bigger text, or a phone's narrower width wrapping a row of controls — a fixed-size part that lays itself out in its box (a plot) gives back what's over first, down to half its height, so its axis stays under its marks instead of running off the bottom (where its names would be pushed back up onto the bars); parts drawn at fixed places (a control) keep their size. Past that, on the web, content the bigger text makes taller than the chart's box grows the element rather than running off the bottom or scrolling inside the page; at the theme's size the page's height always holds. Text a document sizes in pixels stays as written; posters and videos are at the theme's size.

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

A brushable plot and a pannable view are one stop each, at the start of the part they belong to, with keys of their own — screen readers pass them through (`role="application"`), and each stop's description says them:

| On a brush | Does |
|---|---|
| <kbd>Enter</kbd> / <kbd>Space</kbd> | start a selection at the start of the scale — or edit the one there is; again: keep it |
| <kbd>→</kbd> <kbd>↑</kbd> / <kbd>←</kbd> <kbd>↓</kbd> | while selecting: extend it a step, or take a step back (a nice step on a continuous scale — 1, 0.5, 50, a month — one band on a band scale) |
| <kbd>Shift</kbd>+arrow | move the whole selection a step |
| <kbd>Esc</kbd> | while selecting: put back what was there; otherwise: clear the selection |

| On a pannable view | Does |
|---|---|
| arrow keys | move the view a tenth of its size |
| <kbd>+</kbd> / <kbd>−</kbd> | zoom in and out about its middle |
| <kbd>0</kbd> | show all of it again |

Each says what it did (*"Selecting 0 to 10."*, *"0 to 20."*, *"Selected 0 to 20."*, *"Zoom 1.3 ×."*). The native hosts map their screen readers' own gestures onto the same keys: on iOS a brush is adjustable (swipe up and down to extend it, double-tap to start or keep it, the escape gesture to cancel) and a view zooms by swiping up and down and moves with a three-finger swipe; on macOS press, increment, decrement and cancel; TalkBack gets click, scroll forward and back (and left, right, up, down for a view) and dismiss; the desktop viewer's AccessKit tree click, increment, decrement and the scrolls.

The outline follows whatever in the chart has the keyboard, in the theme's accent (Highlight under forced colours).

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
| 2.1.1 Keyboard (A) | stories, controls, clickable marks, the data, brushes and pannable views are reachable and operable | drags of your own (a `scrub`, a handle): a control on the same signal |
| 2.2.2 Pause, stop, hide (A) | autoplay pauses off screen and for reduced motion; the reader can step | don't rely on autoplay alone |
| 2.3.3 Animation from interactions (AAA) | reduced motion everywhere: short crossfades | — |
| 2.4.3 Focus order (A) | figure, story buttons, controls, data, in order | — |
| 2.4.7 / 2.4.11 Focus visible, not obscured (AA) | an outline on the chart for what has the keyboard | — |
| 2.5.8 Target size (minimum) (AA) | std controls are at least 24 px tall and grow with text | marks you make clickable |
| 3.1.1 / 3.1.2 Language (A / AA) | the document's `locale` reaches numbers and the hosts' words | set `locale` |
| 4.1.2 Name, role, value (A) | native roles and states on every host | — |
| 4.1.3 Status messages (AA) | story steps, and what a click, tap or key changed, announced politely through a live region (and the native equivalents) | narration; labels that say a mark's state ("…, hidden") |

This mapping is the project's reading of the criteria against what the runtime does; it hasn't been audited by a third party.

## Known limitations

- **One screen reader driven so far: TalkBack**, on an emulator (above). The web tree is checked in Chromium, WebKit and Gecko and through the macOS Accessibility API, the iOS tree and its actions in the simulator; VoiceOver, NVDA and JAWS haven't been driven, and real devices and the Safari browser itself are untested.
- **Safari's Tab key skips buttons** unless the reader turns on *Press Tab to highlight each item*: in WebKit, Tab goes from the chart to its data, past the story's ← and → buttons (the chart's own arrow keys still step it) and its controls' buttons.
- **A click's result is said generically**: the clicked mark's new label, parts whose label changed, and how much data is shown now — not "filtered to …" in the chart's own terms. A selection that only changes how marks look, with labels that don't say it, is not announced: say the state in the label (`bar({ label: e("d.party + (coalition.has(d.party) ? ', in the coalition' : '')") })`).
- **Drags of your own** (a `scrub` on a custom mark, a draggable handle) have no keyboard path; the standard library's brushes, views and sliders do. The desktop viewer offers brushes and views to screen readers (AccessKit actions), not yet to the keyboard alone.
- **Long lists stop at 500 marks** on the web (200 instances listed per scene by the engine); the summary still counts them all.
- **Text size follows the theme's sizes only**; literal pixel sizes don't scale, posters and videos are at the theme's size, and iOS and Android apps must give a chart the height bigger text needs themselves (`contentHeight` in the status says how much).
- **Forced colours** are a web feature; the desktop viewer doesn't follow the system's contrast or bold-text settings yet, and the web has no bold-text setting to follow (a page can't read one).
- **Two languages** for the hosts' own words (English, Swedish); the summary's phrasing is generic ("data points", "regions").
- **At 2 × on a phone some charts still crowd**: a few labels overlap or are cut in dense networks and multi-panel figures (`datars labels` lists them).

## See also

- [Accessibility feature page](/features/accessibility/) — the lab: each of the above on one chart.
- [Web embedding](/docs/embed/web/) — `text-scale`, `mode`, `reduced-motion`, `readout`.
- [iOS and macOS](/docs/embed/ios/), [Android](/docs/embed/android/) — `textScale`, VoiceOver, TalkBack.
- [Stories](/docs/stories/) — narration.
- [Preview, check and debug](/docs/tools/) — `datars labels`, `lint`, `semantics`.
