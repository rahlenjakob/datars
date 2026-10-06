# 13 — Testing: visual and motion tests that agents can trust

The visual suite rests on one idea: every step of every story as a frame, every transition as a
strip of frames, an animated report showing baseline beside current. It lets a coding agent see what
it broke — including *motion*. It also avoids what usually limits such suites: needing a browser, a
GPU and a running server; comparing pixels with loose thresholds; sampling only a handful of frames
per move; waiting on network tiles; and failures that say only "pixels differ".

## Goals

1. **Exact.** Deterministic engine → compare hashes and exact values, not fuzzy pixels. A 1 px label
   shift is a failure; a GPU driver update is not.
2. **Fast.** The whole gallery in seconds, in-process, in parallel — and incremental: only tests
   whose inputs changed run.
3. **Anywhere.** No browser, no GPU, no network, no server: Linux CI and cloud agents run the full
   suite.
4. **Motion is first-class.** Transitions are sampled densely and checked for the bugs that only
   exist between keyframes (flashes, pops, inside-out shapes, off-screen excursions).
5. **Explains itself.** A failure says *what* changed, *where it came from*, and shows it in one image.

## Layers — cheapest first

| # | Layer | What it checks | Needs | Cost |
|---|---|---|---|---|
| 1 | **Scene snapshots** | resolved scene at each state, as stable text | engine only | µs–ms per state |
| 2 | **Transition sweeps** | a hash of every sampled frame of every transition (default 64 samples) | engine only | ms per transition |
| 3 | **Motion invariants** | properties of every sampled frame and of sequences of frames | engine only | ms per transition |
| 4 | **Pixel goldens** | CPU reference rasterization of states + filmstrip frames, exact | CPU raster | 5–20 ms per frame, cached by hash |
| 5 | **Interaction replays** | scripted event logs (inspect X, activate Y, brush a range, rotate) → snapshots + sweeps | engine only | ms |
| 6 | **Accessibility snapshots** | semantics tree, reading order, labels, contrast | engine only | µs |
| 7 | **Bundle equivalence** | every bundle variant (T1 baked, T2 residual, source form) produces the same frame hashes as running the source document; the T0 poster equals the CPU frame | engine only | ms per variant |
| 8 | **Cross-platform conformance** | the same frame hashes on native x86-64 (Linux, CI) and arm64, in wasm32 under Node (CI), and through the C ABI (the Swift package's tests on macOS in CI; the Android emulator by `scripts/test-android.sh`) | CI + scripts | minutes |
| 9 | **Version skew** (planned) | bundles from the current compiler open on the last N runtime releases (the right variant is chosen, fallbacks are accessible); old bundles open on the new runtime | archived runtimes | minutes, daily |
| 10 | **GPU equivalence** | wgpu output vs CPU reference, perceptual (`datars gpu`: mean and p99 ΔE, visible and solid differences) | a GPU (Metal on macOS today; a device matrix is planned) | seconds, on demand |
| 11 | **Host integration** | DOM overlays, ARIA mirroring, scroll drivers, poster → live handover (no flash, no layout shift), a sample web page and sample iOS/Android apps loading newly published bundles without being rebuilt | browsers, simulators | few tests |
| 12 | **Budgets** | bundle and runtime bytes against `budgets.json` (`datars budgets`, CI); frame-stepped engine and GPU cost per transition (`datars profile`); stalls, dropped frames, cold loads over throttled networks, scrolling and layout shift in real Chrome (`scripts/perf-browser.mjs`, runs committed with the site; `--profile` names the functions behind a transition's worst 50 ms, from engines built with `NAMES=1 scripts/build-wasm.sh`; a transition that drops frames lists its slowest frame and what it did — meshes tessellated, ops, instances rebuilt — without a profiler shifting the timing); the engine's own costs for a light chart and each heavy kind (`scripts/bench-engine.mjs`: `datars profile --json` into `site/perf/engine.json`); native frame rates from the sample apps stepping published charts in the iOS simulator and the Android emulator (`scripts/bench-native.mjs`, `runBenchmark` in DatarsKit and datars-android: each frame's CPU time split into engine and render, with its draws, bytes uploaded, instances rebuilt and meshes tessellated — `datars_view_frame_stats`) | CI; a machine with a GPU and Chrome | minutes, on demand |

Layers 1–7 run on every change, locally and in CI. They are where agents live.

## What gets tested — enumerated automatically

The harness walks each example's **program**: every state, every transition a program can traverse
(neighbouring steps, jumps, chapter entry and exit, filter presets), each crossed with a test matrix:

- size classes (`phone`, `tablet`, `wide`) and orientations
- themes (light, dark, high contrast)
- locales (at least `en` and `sv`, plus an RTL locale for layout)
- reduced motion on/off
- scripted interactions declared beside the example

Adding an example adds its tests; nobody writes a list of frames by hand.

## Scene snapshots

A stable, human-readable dump of the resolved scene: sorted, rounded to 1/100 px, with keys, roles
and provenance. Reviewed in pull requests like code.

```
view main [0 0 880 480] coord=cartesian x=band(party) y=linear[0,35]
  group marks  recipe=std/bar@1.2.0
    rect ("S")   x=62.00 y=40.21 w=48.00 h=319.79 fill=#e8112d  datum=votes#0  role=datum "S: 30.3 %"
    rect ("SD")  x=126.00 y=141.40 w=48.00 h=218.60 fill=#ddd600 datum=votes#1  role=datum "SD: 20.5 %"
  group axis-y  recipe=std/axis@1.2.0
    text ("tick", 10)  "10" at (52.00, 268.57) size=11px align=right
```

A change reads as a diff: `rect ("SD") y 141.40 → 139.12`, `text ("tick", 35) added`. An agent can
read that without looking at a picture — and when it does look, it knows where to look.

## Transition capture — denser, and more useful

Five frames per move miss most of what goes wrong. Because `frame(t)` is pure and cheap, datars
samples every transition at 64 points (configurable, and adaptive: denser where more changes per
unit of t), hashes all of them, and checks invariants on all of them. It rasterizes a subset for
humans and agents:

| Artifact | What it shows | Why it helps |
|---|---|---|
| **Filmstrip** | 8–12 frames side by side, baseline row above current row, diff row below | a whole move at a glance, and the diff row points at what changed |
| **Motion trails** | one image: every element's path across the transition, drawn as a fading trail over the end state | a whole transition's motion in a single image an agent can inspect |
| **Onion skin** | start, middle and end frames overlaid at decreasing opacity | spot overlaps and wrong pairings instantly |
| **Timing chart** | a bar per element showing its window and easing | see stagger order and phasing at a glance |
| **Animated side-by-side** | baseline vs current as APNG/WebP, played in the report | the motion itself, not samples of it |
| **Frames of interest** | auto-picked samples where an invariant came closest to failing or where most elements changed | the frame that matters, not an arbitrary midpoint |

## Motion invariants (checked on every sample)

- `t = 0` and `t = 1` equal the two states exactly.
- No NaN or infinite values; nothing outside the viewport unless clipped on purpose.
- **No flashes:** an entering element's opacity never decreases; an exiting one's never increases; a
  paired element doesn't blink (opacity dip and recovery).
- **No pops:** per-element displacement between adjacent samples stays under a bound (scaled by the
  transition's duration), so nothing teleports.
- **No inside-out shapes:** morphing outlines keep a consistent winding and never cross zero area.
- **Sane pairing:** if most elements enter and exit instead of pairing, the keys are probably wrong —
  flagged with the offending key sets.
- **Label budget:** no more than a declared share of labels collide mid-flight.

Invariants are also available as assertions for custom tests:

```ts
test("ranked → dots moves every party, no flashes", async ({ doc }) => {
  const tr = await doc.transition("ranked", "dots");
  expect(tr).noFlashes();
  expect(tr.element(["SD"]).path()).toStayWithin(doc.view("main").plot);
  expect(tr.at(0.5).overlaps({ role: "datum" })).toBeLessThan(3);
});
```

## Pixel goldens without the fragility

- Rendered by the **CPU reference** rasterizer: deterministic, so comparison is by hash — exact, no
  thresholds, no per-backend baselines.
- **Raster cache keyed by display-list hash:** if the scene didn't change, the pixels didn't either,
  so re-runs rasterize almost nothing.
- **Golden storage:** the repository stores hashes and text snapshots (kilobytes); images live in a
  content-addressed artifact store (a bucket or LFS) and are fetched only for review, so the
  repository doesn't grow by tens of megabytes of PNGs.
- **Fixtures, not network:** tiles, fonts and data come from fixture stores through the sans-IO
  interface. Nothing waits for anything to "settle"; `frame()` reports completeness explicitly.
- **Several sizes** from the test matrix, not only 960×640.

GPU backends are compared against the CPU reference perceptually with `datars gpu` (layer 10) — the
one place fuzzy comparison belongs; a device matrix running it across browsers and GPUs is planned.

## Speed

| Work | Estimate (to validate in Phase 2) |
|---|---|
| ~115 documents, ~6 states and ~6 transitions each, × 3 size classes | ~2,000 states, ~2,000 transitions |
| Scene snapshots + a11y snapshots | < 1 s |
| 64-sample sweeps + invariants | ~130k frame evaluations, a few seconds across cores |
| Pixel goldens on a cold cache | tens of seconds; on a warm cache, only changed scenes |

It also runs **incrementally**: every test's inputs (document, packages, data, engine version) are
hashed, so a change to the bar recipe re-runs only documents that use it. A typical agent iteration
touches seconds of tests, not the whole suite.

## Failures an agent can act on

`datars test` prints a summary and writes `report.json` and `report.html`:

```
FAIL warming/spiral  transition "seasons" → "spiral"  (size=phone)
  37 of 64 samples differ; first at t=0.19
  changed: 1,212 elements · path radius +4.2 % (std/polar@1.3.0 → recipes/spiral.ts:41)
  invariant: no-pops violated at t=0.52  key=("1998", 214)  jump 41 px between samples
  artifacts: trails.png  filmstrip.png  onion.png  timing.png  diff.txt
```

- `datars test --explain <id>` prints the element-level diff with provenance (which recipe line and
  data row produced each changed element).
- `datars test --accept <id>` updates one golden after review; `--accept-all` requires a reason.
- The same results are available as MCP tools (`run_tests`, `explain_failure`, `accept_golden`) with
  image paths, so an agent can look at the trail image and read the diff in one step
  ([14](14-devtools-and-agents.md)).

## Flakiness is a bug

`datars test --determinism` runs every sample twice and on native and wasm, and compares hashes.
Any difference is a P1 violation and fails the build; it is never "retried until green".

## Porting oracle

During development, the 115 stories of an earlier corpus ([15](15-chart-coverage.md)) were ported
with an importer. Their original frames served as an oracle: the report showed the original frame
beside the datars frame with a perceptual score, so a reviewer (or agent) could confirm "the port
looks like the original" before accepting a golden. The corpus, its importer and those baselines
live outside this repository and are not part of its test suite; the goldens here come from
`examples/`.
