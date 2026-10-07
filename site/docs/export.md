---
title: Video, images and PDF
description: Render any datars document as a frame-exact MP4 with WebVTT captions, a PNG, an SVG or a vector PDF — or serve images of published charts — from the same document the web and apps play.
lede: The same document that plays on a page renders to video, images and print, with the same layout, text and motion. No browser, no screen capture, no GPU needed.
---

## Images: PNG, SVG, PDF

```sh
datars render chart.ts                          # out/chart-0.png, the first state, at 2× pixel density
datars render chart.ts --state ranked --out ranked.png
datars render chart.ts --out chart.svg          # vector
datars render chart.ts --out chart.pdf          # one vector PDF page
datars render chart.ts --mode dark --size 1200x630 --dpr 1 --out card.png   # a social card
```

| Flag | Meaning |
|---|---|
| `--state N` or `--state name` | Which program state (default: the first) |
| `--out file` | `.png`, `.svg` or `.pdf` (default: `out/<name>-<state>.png`) |
| `--size WxH` | The size to lay out for, in CSS pixels. The chart re-lays itself out for it — a phone size gets the phone layout, not a shrunk picture. |
| `--dpr D` | Pixel density for PNG (default 2) |
| `--mode dark` or `--mode high-contrast` | The theme mode |
| `--data name=file` | Fill a data slot from a file (below) |
| `--hash` | Print the frame's pixel hash — the same on every platform for the same state |

- **PNG** comes from the CPU reference renderer: bit-exact, identical on every machine, and what the goldens are made of.
- **SVG** keeps the chart's structure: each text run is a group labelled with its text (`aria-label`), glyph outlines are defined once and reused, and elements that link somewhere stay links.
- **PDF** is one vector page sized like the chart (1 CSS px = 0.75 pt), with layers as transparency groups. Text is drawn as glyph outlines — the engine's own layout — and also written as invisible text, so the PDF is searchable and copyable.

These three files were rendered from the Riksdag example when this site was built:

<div class="exports">
<a href="/exports/riksdag.png"><b>PNG</b><span>CPU reference, 2× density</span></a>
<a href="/exports/riksdag.svg"><b>SVG</b><span>Vector, labelled text runs</span></a>
<a href="/exports/riksdag.pdf"><b>PDF</b><span>Vector page, searchable text</span></a>
</div>

## Video: MP4 with captions

```sh
datars video story.ts                                    # out/story.mp4, .vtt (captions), .chapters.vtt
datars video story.ts --size 360x640 --dpr 3             # 1080×1920, vertical: the phone layout
datars video story.ts --size 1080x1080 --dpr 1 --fps 60  # square, 60 fps
datars video story.ts --hold 4 --out film.mp4            # 4 s on each step
```

`datars video` plays the document's program as a film: each state is held for its `hold` (set on the step, else `--hold`, default 2.5 s), then the transition to the next is rendered frame by frame at `--fps` (default 30) — the same transition plans the live views play, sampled exactly, so there are no dropped frames and no timing drift. Frames are piped to `ffmpeg` (which must be installed) as H.264 in an MP4.

**One document, every aspect ratio.** `--size` lays the document out for that box. Recipes lay out for the real size, so a 9:16 film is the phone layout of the chart, with its own label placement — not a crop of the landscape one.

**Captions.** Next to the MP4, `datars video` writes a WebVTT file from the steps' narration: each step's title and text are shown from the start of the transition into it until the transition out of it.

**Chapters.** It also writes `story.chapters.vtt`: one chapter per step, named by the step, for exactly the time the film holds it — a `<track kind="chapters">` for players, and the timestamps to seek when you need a step's frame. The film is BT.709 and tagged so, so players convert its colours as they were encoded.

This film was rendered by `datars video` from the descent example when this site was built ({{filmsize:descent-16x9}}):

<div class="video-row"><figure class="v169">{{film:descent-16x9}}<figcaption>From the world to Sweden to central Stockholm: one camera over OpenStreetMap vector tiles, rendered frame-exactly at 16:9 with WebVTT captions from the story's narration.</figcaption></figure></div>

## Filmstrips of a transition

```sh
datars film chart.ts --from bars --to pie --frames 8
```

`film` writes two images of one transition: a strip of evenly spaced frames (`…-strip.png`) and a motion-trail image of where every element travels (`…-trails.png`). Use it to check a transition — or to show one in a document, a review or a pull request — without playing it.

## The image server

`datars serve` renders published charts on request — for social cards, email, RSS readers and anywhere else scripts can't run:

```text
/render/<alias>.png?state=2&width=1200&height=630&dpr=1&mode=dark
/render/<alias>.svg?state=0
/render/<alias>.pdf
```

| Parameter | Meaning |
|---|---|
| `state` | Program state index (default 0) |
| `width`, `height` | Layout size in CSS px, 50–4000 (default: the chart's own) |
| `dpr` | PNG pixel density, 0.25–4 (default 1) |
| `mode` | `dark` for the dark theme |

It renders from the published bundle with the CPU reference — the same picture every runtime draws. The image server needs `datars serve` running; on a purely static host, render the images at build time with `datars render` instead (this site's social cards are made that way).

## Previews with real data

A chart with a [data slot](/docs/publishing/#data-slots-each-readers-own-data) renders its sample until data arrives. Give it a file instead:

```sh
datars render spending/doc.ts --data spending=user.json --out user.png
datars video spending/doc.ts --data spending=user.json
```

`--data` works with every command that loads a document — `render`, `video`, `film`, `inspect`, `semantics`, `check`, `lint`, `explain`, `profile`. Several slots: `--data a=a.csv,b=b.json`.

## The desktop viewer

`datars-view` opens a document, a bundle or a published chart in a native desktop window, through the GPU:

```sh
datars-view chart.json
datars-view chart.datars
datars-view http://127.0.0.1:8787/c/descent        # a published chart, from `datars serve`
datars-view chart.json --screenshot out.png --state 2 --dpr 2   # render offscreen through the GPU
```

<kbd>←</kbd> <kbd>→</kbd> step the program, <kbd>Esc</kbd> leaves a chapter, <kbd>D</kbd> toggles dark mode; drag and scroll for brushes and explorable maps. It speaks plain `http://` only (serve charts locally with `datars serve`). Build it from the repository with `cargo build --release -p datars-host-native`. Its accessibility tree goes to the operating system's screen reader through AccessKit (built and unit-tested; not yet driven by a real screen reader).

## Next

- [Publishing and hosting](/docs/publishing/) — bundles for the web and apps
- [States and stories](/docs/stories/) — steps, holds and narration, which become the film and its captions
- [CLI](/docs/cli/) — every command
