---
title: Documentation
titleTag: Documentation · datars
description: How to make charts, stories and maps with datars — install the CLI, write TypeScript documents, theme them, publish them, and embed them on the web, in apps, in video and print.
lede: Everything you need to make charts, stories and maps with datars, publish them, and put them in front of readers — on a web page, in an iOS or Android app, in a video or a PDF.
---

## What you need

- **The `datars` command line.** It previews, checks, renders and publishes charts. It isn't on crates.io yet: you build it from the repository with Rust ([getting started](/docs/getting-started/) has the three commands).
- **Node 20 or newer, with esbuild** in your project, for documents written in TypeScript. A document can also be plain JSON, which needs nothing but the CLI.
- **A static host** for published charts — any bucket, CDN, GitHub Pages or a folder on your own server. No database, no chart server, no API key.

> **Tip** Using React, Vite or Next.js? Start with [React, Vite and Next.js](/docs/react/): charts in a React app without adding script tags by hand.

## Start here

- [Getting started](/docs/getting-started/) — install the CLI, make a chart from a template, preview it live, publish it and embed it.
- [React, Vite and Next.js](/docs/react/) — the same, inside a React app.
- [Python and Jupyter](/docs/python/) — charts from Python, shown in notebooks.
- [Core concepts](/docs/concepts/) — documents, keys, signals, states, recipes, themes and bundles: the ideas everything else builds on.

## Write charts

- [Documents](/docs/documents/) — the shape of a document: size, theme, data, keys, scene, motion and program; layout of groups.
- [Data and tables](/docs/data/) — inline values, CSV, files and URLs, live sources, data slots filled by an app, generated rows, and every table operation.
- [Charts, scales and marks](/docs/charts/) — `plot` and the marks inside it, how scales are built, and the primitives underneath.
- [Expressions](/docs/expressions/) — the small language that positions, colours and labels every mark, and everything it can read.
- [Signals and interaction](/docs/interaction/) — hover, click, brush, drag, pan and zoom; sliders; linked views; talking to the page or app.
- [States and stories](/docs/stories/) — steps, narration, scroll stories, autoplay, films and drill-down chapters.
- [Motion](/docs/motion/) — how changes animate by key, and motion rules to choreograph them.

## Style

- [Themes and brands](/docs/theming/) — tokens, colours, fonts, light, dark and high contrast, and restyling published charts at runtime.

## Maps and big data

- [Maps](/docs/maps/) — atlases, choropleths, projections, automatic basemaps and camera flights from the world to a street.
- [Big data](/docs/big-data/) — point clouds of millions of rows, generated data and point archives read by range.

## Extend

- [Custom recipes and scenes](/docs/custom-recipes/) — write your own chart type in TypeScript, or eject a standard one and change it.

## Ship

- [Publishing and hosting](/docs/publishing/) — bundles, aliases, tiers and what a static host needs.
- [Web: `<datars-view>`](/docs/embed/web/) — the element, its attributes, events and methods.
- [iOS and macOS](/docs/embed/ios/) — DatarsKit and SwiftUI.
- [Android](/docs/embed/android/) — the Kotlin view.
- [Video, images and PDF](/docs/export/) — MP4 with captions, PNG, SVG and vector PDF from the same document.

## Quality

- [Accessibility](/docs/accessibility/) — what screen readers get, keyboard access, reduced motion and text alternatives.
- [Preview, check and debug](/docs/tools/) — the live page, check, lint, explain, film, profile and tests.
- [Coding agents and MCP](/docs/agents/) — the MCP server, `llms.txt` and an agent's loop.

## Reference

- [Chart reference](/docs/std/) — all {{count:recipes}} recipes of the standard library, generated from the recipes themselves.
- [SDK reference](/docs/sdk/) — every export of `@datars/sdk`, signatures read from the source, with live figures of the primitives, scales, motion and data operations.
- [CLI](/docs/cli/) — every `datars` command.
- [Document format](/docs/ir/) — the JSON a document compiles to, and its schema.
