// @datars/web — the datars runtime for web pages. Install once (one script), then charts arrive as
// bundles by URL and update without redeploying the page (docs/12-delivery.md).
//
//   <script type="module" src="https://cdn…/datars/1/datars.js"></script>
//   <datars-view src="https://charts.example.com/c/votes"></datars-view>
//
// The page owns the DOM: the poster and accessible text show first, the canvas takes over when the
// runtime is ready, the chart's words lie over it as real text (selectable, findable: see
// textlayer.ts), narration cards sit at the engine's anchors, and a hidden list mirrors the
// engine's semantics for screen readers. Everything that decides pixels runs in the wasm engine.

import { FrameProfiler, perfRequested } from "./perf.js";
import { TEXT_CSS, TextLayer } from "./textlayer.js";
// Push feeds (WebSocket, server-sent events, polls) into a chart's live sources.
export { liveFeed, socket, events, poll } from "./live.js";
export type { LiveFeed, LiveFeedOptions, FeedContext, FeedStatus } from "./live.js";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Wasm = any;

const engines = new Map<string, Promise<Wasm>>();
/** How long a chart's first frame waits for the faces the page's tokens name (see `themeFaces`). */
const THEME_FACE_WAIT = 1500;
/** How often the screen readers' mirror (summary, data table) follows a live feed, at most (ms). */
const MIRROR_DATA_EVERY = 5000;
/** A chart ready within this many ms of starting never shows its poster (it would only flash). */
const POSTER_DELAY = 600;
/** An engine is loaded on this page: charts starting now are ready in a few hundred ms and fade
 * in — they show no poster at all. */
let engineOnPage = false;
/** Charts attach to the GPU one at a time: two WebGPU adapter/device requests in flight at once
 * can leave one unanswered in Chromium (two charts mounting together, one never appears). */
let gpuTurn: Promise<unknown> = Promise.resolve();
function oneAtATime<T>(f: () => Promise<T>): Promise<T> {
  const p = gpuTurn.then(f, f);
  gpuTurn = p.catch(() => undefined);
  return p;
}

/** Views mid-motion on this page. A chart starting up (parsing its bundle, its first frame, the
 * GPU) holds its heavy steps until none is moving — or a while at most — so a chart loading
 * further down the page doesn't make the one being read stutter. */
const moving = new Set<object>();
let quiet: (() => void)[] = [];
/** Give back a canvas's WebGL context now, once its engine is freed. A context outlives its
 * canvas until garbage collection, and browsers allow only a handful at once (Safari's limit is
 * hit after a page re-adds a few charts): a chart taken off the page must not hold one. A canvas
 * drawn with WebGPU or the CPU has no WebGL context, and `getContext` returns null for it. */
function releaseGl(canvas: HTMLCanvasElement | undefined) {
  const gl = canvas?.getContext("webgl2") as WebGL2RenderingContext | null | undefined;
  gl?.getExtension("WEBGL_lose_context")?.loseContext();
}

/** A semantics item as the runtime gives it (`status().semantics`, `chrome().actions`). */
interface MirrorItem { role: string; label: string; depth: number; path: string; actionable: boolean; rect?: number[]; value?: number; kind?: "checkbox" | "switch" | "radio" | "brush" | "pan"; checked?: boolean; missing?: boolean; index?: number; total?: number }

/** What a mirror built from these actions (`chrome().actions`) offers: their paths and labels. */
function actionsKey(actions: { path: string; label: string }[]): string {
  return actions.map((a) => `${a.path}\t${a.label}`).join("\n");
}

function setMoving(v: object, on: boolean) {
  if (on) moving.add(v);
  else if (moving.delete(v) && moving.size === 0) {
    const q = quiet;
    quiet = [];
    q.forEach((f) => f());
  }
}
function quietTurn(maxMs = 1500): Promise<void> {
  if (moving.size === 0) return Promise.resolve();
  return new Promise((resolve) => {
    const t = setTimeout(resolve, maxMs);
    quiet.push(() => { clearTimeout(t); resolve(); });
  });
}

/** The page's scrolling: a chart coming into view prefers to open while the reader pauses — its
 * poster shows meanwhile — so its short tasks (a chunk opened, the canvas attached: a few ms to a
 * few tens) don't land mid-scroll. A preference with a budget: a start waits for a pause at most
 * `START_PAUSE_MS` in all, and not at all once the chart is on screen. Waiting up to 3 s at each
 * of its steps, a chart scrolled to during a long scroll sat on its poster for seconds while in
 * view — slower than any hitch it saved. */
let lastScroll = -1e9;
const START_PAUSE_MS = 400;
// (Imported on a server — SSR of a page that uses charts — the module loads and does nothing.)
if (typeof addEventListener === "function") addEventListener("scroll", () => { lastScroll = performance.now(); }, { passive: true, capture: true });
async function scrollPause(until: number, onScreen: () => boolean): Promise<void> {
  while (performance.now() - lastScroll < 180 && performance.now() < until && !onScreen()) {
    await new Promise((r) => setTimeout(r, 60));
  }
  if (!onScreen()) await quietTurn();
}

/** The page's share of the engines' per-frame work budgets (points built, tiles decoded, map
 * features styled, meshes tessellated — `View.set_work_scale`). They're sized natively, and wasm
 * runs the same work two to three times slower (a phone's CPU slower again), so the page starts
 * at half and never takes more than three quarters. Then: a frame that nearly missed its refresh
 * (a street tile arriving mid-flight) cuts the share at once; the average of the moving frames
 * lowers it while they run past ~8 ms and raises it again when there's room. Detail then
 * arrives over more frames instead of frames dropping. One for the page: it's the device that is
 * slow, not a chart. */
let workScale = 0.5;
let frameMs = 0;

/** Device px per CSS px the charts render at. Touch screens start at 2 at most: a 3× phone drew
 * 2.25 times a 2× one's pixels for a difference the eye barely sees at that density, and big charts
 * are fill-bound there (a galaxy's glow of 30,000 soft discs). Then, page-wide, `renderScale` of
 * that: lowered while frames come slowly though the main thread has time to spare — the GPU is
 * the bottleneck, and fewer pixels are the one thing that helps — and raised again after a couple
 * of seconds at full rate. */
const coarsePointer = typeof matchMedia !== "undefined" && matchMedia("(pointer: coarse)").matches;
let renderScale = 1;
/** Bumped when `renderScale` changes: each chart re-sizes its canvas on its next frame. */
let renderGen = 0;
function renderDpr(): number {
  const d = typeof devicePixelRatio === "number" && devicePixelRatio > 0 ? devicePixelRatio : 1;
  return Math.max(1, (coarsePointer ? Math.min(d, 2) : d) * renderScale);
}
let gapMs = 0;
let lastScaleChange = 0;
let fullRateSince = 0;
/** A moving frame's interval since the last one (rAF timestamps); the main thread's share of it
 * is `frameMs` (see `noteFrame`). */
function noteGpu(interval: number, now: number) {
  if (typeof document !== "undefined" && document.visibilityState !== "visible") return;
  if (!(interval > 0 && interval < 250)) return; // a pause, not a slow frame
  gapMs = gapMs ? gapMs * 0.9 + interval * 0.1 : interval;
  const gpuBound = gapMs > 22 && frameMs < gapMs * 0.5;
  if (gpuBound && renderScale > 0.5 && now - lastScaleChange > 500) {
    renderScale = Math.max(0.5, renderScale * 0.85);
    lastScaleChange = now;
    fullRateSince = 0;
    renderGen++;
  } else if (gapMs < 17.5 && renderScale < 1) {
    fullRateSince ||= now;
    if (now - fullRateSince > 2000 && now - lastScaleChange > 2000) {
      renderScale = Math.min(1, renderScale / 0.85);
      lastScaleChange = now;
      fullRateSince = 0;
      renderGen++;
    }
  } else {
    fullRateSince = 0;
  }
}
function noteFrame(ms: number): number {
  frameMs = frameMs ? frameMs * 0.85 + ms * 0.15 : ms;
  if (ms > 14) workScale = Math.max(0.2, workScale * 0.7);
  else if (frameMs > 8) workScale = Math.max(0.2, workScale * 0.9);
  else if (frameMs < 4 && ms < 8) workScale = Math.min(0.75, workScale * 1.05);
  return workScale;
}

/** Each engine build's wasm memory, for the stats panel (module namespaces can't carry it). */
const memories = new WeakMap<object, { buffer: ArrayBuffer }>();

/** Bundle chunks being fetched, by URL. Charts on one page share chunks (the fonts, a common
 * basemap) and often ask for them at the same moment: one download serves them all. Dropped once
 * it lands; a later ask is the HTTP cache's (chunks are immutable). */
const chunkFetches = new Map<string, Promise<Uint8Array>>();
function fetchChunk(url: string): Promise<Uint8Array> {
  let got = chunkFetches.get(url);
  if (!got) {
    got = fetch(url)
      .then((res) => (res.ok ? res.arrayBuffer() : Promise.reject(new Error(`${res.status} ${url}`))))
      .then((b) => new Uint8Array(b))
      .finally(() => chunkFetches.delete(url));
    chunkFetches.set(url, got);
  }
  return got;
}

/**
 * Runtime files the page supplies itself, by their path in this package's `dist/`
 * (`wasm/datars_host_web.js`, `wasm/datars_host_web_bg.wasm`, `fonts/Inter-Regular.ttf`,
 * `atlas/countries.geojson`) → a URL, often a `blob:` one. Looked up first: for hosts that can't
 * serve files at all — a notebook widget receives the runtime over its kernel connection, offline,
 * and has no URL to serve it from. Also reachable as `customElements.get("datars-view").assets`, so
 * a second copy of this module finds the map of the copy that defined the element.
 */
export const runtimeAssets = new Map<string, string>();

/** Where the runtime's files (`wasm/`, `fonts/`, `atlas/`) are served from, when not next to
 * this script. */
let runtimeBase: string | null = null;

/** Serve the runtime's files from somewhere other than next to this script. A bundler (Vite,
 * webpack, Next.js) moves this module into the app's own chunks, where `import.meta.url` is no
 * longer the package folder: the framework plugins (`@datars/vite`, `@datars/next`) copy
 * `dist/{wasm,fonts,atlas}` to a public path and point the runtime at it. Call before the first
 * chart starts; `globalThis.DATARS_RUNTIME` works too (set before this module loads). */
export function setRuntimeBase(url: string): void {
  runtimeBase = url.endsWith("/") ? url : `${url}/`;
}

/** A runtime file's URL (`wasm/…`, `fonts/…`, `atlas/…`): supplied by the page (`runtimeAssets`),
 * else under the configured base, else next to this script. Resolved at call time, never by the
 * bundler: `new URL(<string literal>, import.meta.url)` would be rewritten (or globbed) by Vite and
 * webpack, and the engine picks its build at run time. */
function runtimeFile(path: string): string {
  const supplied = runtimeAssets.get(path);
  if (supplied) return supplied;
  const configured = runtimeBase ?? ((globalThis as { DATARS_RUNTIME?: string }).DATARS_RUNTIME || null);
  const base = configured ? new URL(configured.endsWith("/") ? configured : `${configured}/`, document.baseURI).href : import.meta.url;
  return new URL(path, base).href;
}

/** The engine builds: `core` plays everything the publish compiler pre-expanded (nearly every
 * bundle); `core-gl` is the same with WebGL2 for browsers without WebGPU; `full` adds the recipe
 * sandbox for T3 bundles and raw source documents. */
export function engineUrl(kind: "core" | "core-gl" | "full"): string {
  return runtimeFile(kind === "full" ? "wasm/datars_host_web.js" : kind === "core-gl" ? "wasm/datars_core_gl.js" : "wasm/datars_core.js");
}

/** Whether WebGPU works here — an adapter, not just `navigator.gpu` (the iOS simulator and
 * blocklisted GPUs have the API and no adapter). Asked once, as the runtime loads, alongside the
 * first manifest: browsers without it load the engine with WebGL2 (Safari before iOS 26), which
 * draws big charts on the GPU instead of the CPU. */
const webgpuWorks: Promise<boolean> = (() => {
  const gpu = typeof navigator === "undefined" ? undefined : (navigator as { gpu?: { requestAdapter(): Promise<unknown> } }).gpu;
  return gpu ? gpu.requestAdapter().then((a) => !!a, () => false) : Promise.resolve(false);
})();

/** Load an engine build (once per page per build; every view shares it). */
export function loadEngine(url = engineUrl("core")): Promise<Wasm> {
  let p = engines.get(url);
  if (!p) {
    // A native import at run time: bundlers must leave it alone (the URL is only known here).
    p = import(/* @vite-ignore */ /* webpackIgnore: true */ url).then(async (m) => {
      // An engine the page supplied (a `blob:` module) can't find its wasm next to itself: hand
      // it the supplied `…_bg.wasm` of the same build.
      const js = [...runtimeAssets].find(([, u]) => u === url)?.[0];
      const wasmUrl = js && runtimeAssets.get(js.replace(/\.js$/, "_bg.wasm"));
      const out = await m.default(wasmUrl ? { module_or_path: wasmUrl } : undefined);
      engineOnPage = true;
      // The instance's memory, for the stats panel (every view on the page shares it).
      if (out?.memory) memories.set(m, out.memory);
      return m;
    });
    engines.set(url, p);
  }
  return p;
}

/** Where `datars:fonts/<file>` requests resolve: the default fonts shipped next to the runtime
 * (`dist/fonts/`), fetched only when a raw document needs them — bundles carry their own. */
export function datarsAsset(url: string): string {
  return runtimeFile(url.slice("datars:".length));
}

/** A request for a font file (a theme face's `font:<face>`, or a font source by path or URL). */
function isFontRequest(name: string, url: string): boolean {
  return name.startsWith("font:") || url.startsWith("google:") || /\.(ttf|otf|woff2?|ttc)(\?|$)/i.test(url);
}

/** Does a manifest offer a variant that runs without the sandbox? */
function coreSuffices(manifest: { variants?: { requires?: { modules?: string[] } }[] }): boolean {
  return (manifest.variants ?? []).some((v) => !(v.requires?.modules ?? []).includes("sandbox"));
}

const CSS = `
:host { display: block; position: relative; font-family: var(--datars-font, Inter, system-ui, sans-serif); color: var(--ink); }
.stage { position: relative; width: 100%; }
canvas, .poster { position: absolute; inset: 0; width: 100%; height: 100%; }
canvas { transition: opacity .22s ease-out; }
@media (prefers-reduced-motion: reduce) { :host(:not([reduced-motion="no-preference"])) canvas { transition: none; } }
:host([reduced-motion="reduce"]) canvas, :host([reduced-motion=""]) canvas { transition: none; }
.poster svg { width: 100%; height: 100%; }
.fail { position: absolute; inset: 0; margin: 0; display: grid; place-items: center; padding: 16px; text-align: center; font-size: 13px; line-height: 1.4; color: var(--muted, #6b6b6b); }
.poster[hidden], canvas[hidden] { display: none; }
/* Native pickers over engine-drawn selects on touch screens: invisible, but a tap opens the
   platform's own list (16 px keeps iOS from zooming the page to the control). */
.pickers select { position: absolute; opacity: 0.001; font-size: 16px; border: 0; padding: 0; margin: 0; appearance: none; -webkit-appearance: none; cursor: pointer; }
.links a { position: absolute; display: block; border-radius: 3px; }
.links a:hover { background: rgba(127,127,127,.12); }
.links a:focus-visible { outline: 2px solid var(--accent, #4269d0); outline-offset: 1px; }
.card { position: absolute; max-width: 280px; padding: 10px 12px; border-radius: 8px; background: var(--surface, #fff); color: var(--surface-ink, #111);
  box-shadow: 0 2px 12px rgba(0,0,0,.14); font-size: 14px; line-height: 1.4; transition: opacity .3s; -webkit-user-select: text; user-select: text; cursor: text; }
.card h3 { margin: 0 0 4px; font-size: 18px; }
.card:empty { opacity: 0; pointer-events: none; }
.tip { position: absolute; pointer-events: none; padding: 4px 8px; border-radius: 4px; font-size: 12px; background: var(--ink, #111); color: var(--paper, #fff); white-space: nowrap; }
.tip[hidden] { display: none; }
/* Where the keyboard is: the mirror's buttons and sliders are hidden, so the mark or control a
   focused one stands for is outlined on the chart. */
.focus { position: absolute; pointer-events: none; border-radius: 4px; outline: 2px solid var(--accent, #4269d0); outline-offset: 2px; }
.focus[hidden] { display: none; }
@media (forced-colors: active) { .focus, .links a:focus-visible { outline-color: Highlight; } .tip { border: 1px solid CanvasText; } }
.controls { display: flex; gap: 8px; justify-content: center; margin-top: 8px; }
.controls[hidden] { display: none; }
.controls button { font: inherit; padding: 4px 12px; border-radius: 6px; border: 1px solid var(--rule, #999); background: var(--paper, #fff); color: var(--ink, #111); cursor: pointer; }
.perf { position: absolute; left: 6px; top: 6px; z-index: 2; pointer-events: none; max-width: calc(100% - 12px); padding: 6px 8px; border-radius: 6px; background: rgba(0,0,0,.8); color: #fff; font: 11px/1.35 ui-monospace, Menlo, monospace; }
.perf[hidden] { display: none; }
.perf pre { margin: 0; font: inherit; white-space: pre-wrap; }
.perf canvas { display: block; width: 240px; max-width: 100%; height: 36px; margin-top: 6px; }
.perf button { float: right; margin: -4px -4px 0 8px; padding: 0 4px; pointer-events: auto; background: none; border: 0; color: #fff; font: 14px/1 system-ui, sans-serif; cursor: pointer; }
.sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); white-space: nowrap; }
/* The reader's default font size (the browser's Font size setting), whatever the page's own CSS
   says: \`medium\` is that setting. Its width is watched, so a change there reaches the chart. */
.em { position: absolute; visibility: hidden; pointer-events: none; font-size: medium; width: 1em; height: 0; overflow: hidden; }
${TEXT_CSS}`;

/** `HTMLElement`, or (on a server, where there is none) a stand-in so the module still loads. */
const ElementBase: typeof HTMLElement = typeof HTMLElement === "undefined" ? (class {} as typeof HTMLElement) : HTMLElement;

export class DatarsView extends ElementBase {
  static observedAttributes = ["src", "doc", "state", "mode", "height", "no-controls", "reduced-motion", "text-scale"];
  /** Runtime files the page supplies (see `runtimeAssets`). */
  static assets = runtimeAssets;
  private root: ShadowRoot;
  private stage!: HTMLDivElement;
  private canvas!: HTMLCanvasElement;
  private poster!: HTMLDivElement;
  private card!: HTMLDivElement;
  private tip!: HTMLDivElement;
  private mirror!: HTMLDivElement;
  /** What the mirror's buttons offer (each action's path and label), as last built. */
  private mirrorActions = "";
  private live!: HTMLDivElement;
  private view: Wasm = null;
  /** A document handed in by the page (`setDocument`), shown instead of `src`/`doc`. */
  private pendingDoc: string | null = null;
  /** Data the page handed in (slots), kept so it survives loading, reloads and src changes. */
  private hostData = new Map<string, Uint8Array>();
  /** Signals and theme tokens the page set, likewise (a chart set up before it loads opens so). */
  private hostSignals = new Map<string, unknown>();
  private hostTokens: Record<string, unknown> | null = null;
  private base = location.href;
  /** Where the bundle's chunks live (`…/chunks/<hash>`), for lazily loaded font subsets. */
  private chunkBase = "";
  private warnedGoogle = false;
  private fetching = new Set<string>();
  private raf = 0;
  private timer = 0;
  private idle = 0;
  /** Counts mirror syncs; `describedGen` is the one whose whole semantics tree the mirror holds
   * (see `describeSoon`). */
  private chromeGen = 0;
  private describedGen = -1;
  /** Rows arrived since the mirror was built (`noteData`), and when it was. */
  private dataStale = false;
  private describedAt = -Infinity;
  private describeTimer = 0;
  private describeIdle = 0;
  /** On screen (any part of it); frames off screen wait until it is. */
  private onScreen = true;
  private drawn = false;
  /** Frames skipped while yielding to a chart in transition. */
  private yielded = 0;
  /** The canvas fades in over the poster at its first frame. */
  private fadeIn = false;
  private layoutRaf = 0;
  private lastIndex = -1;
  private size = { w: 800, h: 480 };
  /** Host listeners attached (once per element). */
  private wired = false;
  /** Counts starts: a start still loading when the element leaves (or starts again) gives up. */
  private generation = 0;
  private stopScrub: (() => void) | null = null;
  private stopSteps: (() => void) | null = null;
  private stopPlayback: (() => void) | null = null;
  /** Re-applies playback and reduced motion (see `watchPlayback`). */
  private applyPlayback: (() => void) | null = null;
  /** A `seek` asked for before the chart opened: applied when it does. */
  private pendingSeek: number | null = null;
  /** The frame profiler and stats panel (`perf`, Shift+D), and the state names it labels
   * transitions with. */
  private profiler: FrameProfiler | null = null;
  private stateNames: string[] = [];
  private wasm: Wasm = null;

  constructor() {
    super();
    this.root = this.attachShadow({ mode: "open" });
  }

  connectedCallback() {
    this.root.innerHTML = `<style>${CSS}</style>
      <div class="stage" part="stage"><div class="poster" part="poster"></div><canvas hidden part="canvas" aria-hidden="true"></canvas>
      <div class="texts" part="texts" aria-hidden="true"></div><div class="card" part="card" aria-hidden="true"></div><div class="links" part="links"></div><div class="pickers"></div><div class="tip" hidden part="tooltip"></div><div class="focus" hidden part="focus"></div><div class="perf" hidden part="perf"></div></div>
      <div class="sr mirror"></div>
      <div class="controls" part="controls" hidden><button data-ev="prev" aria-label="Previous">←</button><button data-ev="next" aria-label="Next">→</button></div>
      <div class="sr" aria-live="polite"></div><span class="em" aria-hidden="true"></span>`;
    this.stage = this.root.querySelector(".stage")!;
    // The height the page asked for, from the first frame the element is on the page: laid out a
    // frame later, the chart would grow into its space and push the page down.
    this.reserveHeight();
    this.canvas = this.root.querySelector("canvas")!;
    this.poster = this.root.querySelector(".poster")!;
    this.card = this.root.querySelector(".card")!;
    this.linkLayer = this.root.querySelector(".links")!;
    this.tip = this.root.querySelector(".tip")!;
    this.mirror = this.root.querySelector(".mirror")!;
    this.live = this.root.querySelector("[aria-live]")!;
    // A sighted keyboard user sees where the hidden list's focus is: the item's box on the chart.
    const ring = this.root.querySelector<HTMLDivElement>(".focus")!;
    this.mirror.addEventListener("focusin", (e) => {
      const el = e.target as HTMLElement;
      const r = el.dataset?.rect?.split(",").map(Number);
      if (!r || r.length !== 4 || !(r[2] > 0 && r[3] > 0)) { ring.hidden = true; this.tip.hidden = true; return; }
      Object.assign(ring.style, { left: `${r[0]}px`, top: `${r[1]}px`, width: `${r[2]}px`, height: `${r[3]}px` });
      ring.hidden = false;
      // A data mark the keyboard is on says what it is, as the pointer's tooltip would.
      if (el.dataset.label) {
        this.tip.textContent = el.dataset.label;
        this.tip.hidden = false;
        this.tipPinned = false;
        const tw = this.tip.offsetWidth, th = this.tip.offsetHeight;
        const above = r[1] - th - 8 >= 0;
        this.tip.style.left = `${Math.max(0, Math.min(r[0] + r[2] / 2 - tw / 2, this.size.w - tw))}px`;
        this.tip.style.top = `${above ? r[1] - th - 8 : Math.min(r[1] + r[3] + 8, this.size.h - th)}px`;
      } else this.tip.hidden = true;
    });
    this.mirror.addEventListener("focusout", (e) => {
      // Moving to another of the list's items: its own focusin places the ring.
      if (!this.mirror.contains(e.relatedTarget as Node | null)) { ring.hidden = true; this.tip.hidden = true; }
    });
    // The data's rows are one stop for Tab; the arrow keys (and Home, End) move between them.
    this.mirror.addEventListener("keydown", (e) => {
      const el = e.target as HTMLElement;
      if (!el.dataset || !("nav" in el.dataset)) return;
      const stops = [...this.mirror.querySelectorAll<HTMLElement>("[data-nav]")];
      const i = stops.indexOf(el);
      const k = e.key;
      const j = k === "ArrowDown" || k === "ArrowRight" ? Math.min(stops.length - 1, i + 1)
        : k === "ArrowUp" || k === "ArrowLeft" ? Math.max(0, i - 1)
        : k === "Home" ? 0 : k === "End" ? stops.length - 1 : -1;
      if (j < 0) return;
      e.preventDefault();
      el.tabIndex = -1;
      stops[j].tabIndex = 0;
      stops[j].focus();
    });
    this.tabIndex = 0;
    this.setAttribute("role", "figure");
    this.root.querySelectorAll("button").forEach((b) => b.addEventListener("click", () => this.send((b as HTMLElement).dataset.ev!)));
    if (!this.wired) {
      // The host element's own listeners, once: it may be taken off the page and put back.
      this.wired = true;
      this.addEventListener("keydown", (e) => {
        // Arrows and Space step the story when the chart itself has the keyboard — not when one
        // of its list's controls does: a slider's arrows move its thumb, Space presses a button.
        const own = e.composedPath()[0] === this;
        if (own && (e.key === "ArrowRight" || e.key === " ")) { this.send("next"); e.preventDefault(); }
        if (own && e.key === "ArrowLeft") { this.send("prev"); e.preventDefault(); }
        if (e.key === "Escape") this.send("back");
        if (e.key === "D" && e.shiftKey && !e.metaKey && !e.ctrlKey && !e.altKey) this.showStats(!this.profiler?.visible);
      });
      // Layout sets the stage's height, which resizes this element: done in the next frame, not
      // inside the observer (a change during delivery is a "ResizeObserver loop" error).
      // Off screen, a chart draws nothing more (its clock, levels and tiles resume when it's back):
      // the page's frames go to what the reader sees.
      new IntersectionObserver((entries) => {
        const on = entries.some((e) => e.isIntersecting);
        if (on === this.onScreen) return;
        this.onScreen = on;
        if (on) this.kick();
      }).observe(this);
      new ResizeObserver(() => {
        cancelAnimationFrame(this.layoutRaf);
        this.layoutRaf = requestAnimationFrame(() => this.layout());
      }).observe(this);
      matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => this.applyMode());
      // More contrast asked for, or the system's own colours forced (Windows contrast themes).
      matchMedia("(prefers-contrast: more)").addEventListener("change", () => this.applyMode());
      matchMedia("(forced-colors: active)").addEventListener("change", () => { this.applyTokens(); this.applyMode(); });
    }
    // The reader's text size: the probe's width is their default font size in px.
    this.em = this.root.querySelector<HTMLSpanElement>(".em")!;
    new ResizeObserver(() => this.applyTextScale()).observe(this.em);
    const texts = this.root.querySelector<HTMLDivElement>(".texts")!;
    this.textLayer = new TextLayer(this.root, texts, (id) => this.view?.face_bytes?.(id));
    const wheel = (e: WheelEvent) => {
      if (!this.view) return;
      const r = this.canvas.getBoundingClientRect();
      const scale = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? innerHeight : 1;
      // Only an explorable view takes the wheel; otherwise the page scrolls as usual.
      // The content moves under the pointer: whatever the tooltip named isn't there any more.
      if (this.view.wheel(e.clientX - r.left, e.clientY - r.top, e.deltaY * scale)) { e.preventDefault(); this.tip.hidden = true; this.textLayer.hide(); this.kick(); this.noteSignals(); }
    };
    // The pointer leaving the chart (not just going from the canvas to text over it, or back).
    // A finger leaves after every tap (touch has no hover): what the tap showed stays.
    const leave = (e: PointerEvent) => {
      if (e.pointerType === "touch") return;
      const to = e.relatedTarget as Node | null;
      if (to && (to === this.canvas || texts.contains(to))) return;
      this.view?.pointer("leave", 0, 0);
      this.tip.hidden = true;
      this.kick();
    };
    // Touch: a finger that lands where the chart drags (a brush, a pan, a slider's thumb) is the
    // chart's — the page doesn't scroll with it, which would cancel the drag a few pixels in.
    // Every other touch scrolls the page as usual. Two fingers on an explorable view pinch-zoom.
    this.stage.addEventListener("touchstart", (e) => {
      if (!this.view?.drags_at) return;
      const r = this.canvas.getBoundingClientRect();
      const t = e.changedTouches[0];
      if (t && this.view.drags_at(t.clientX - r.left, t.clientY - r.top)) e.preventDefault();
    }, { passive: false });
    const cancel = (e: PointerEvent) => {
      this.touches.delete(e.pointerId);
      if (!this.touches.size) this.pinch = 0;
      // The browser took the gesture (a scroll, a system gesture): the chart's drag ends here.
      this.view?.pointer("leave", 0, 0);
      this.tip.hidden = true;
      this.kick();
      this.noteSignals();
    };
    this.canvas.addEventListener("pointercancel", cancel);
    texts.addEventListener("pointercancel", cancel);
    this.canvas.addEventListener("pointermove", (e) => { if (!this.pinched(e, "move")) this.pointer("move", e); });
    this.canvas.addEventListener("pointerdown", (e) => { this.canvas.setPointerCapture(e.pointerId); if (this.pinched(e, "down")) return; this.press(e, false); this.pointer("down", e); });
    this.canvas.addEventListener("wheel", wheel, { passive: false });
    this.canvas.addEventListener("pointerup", (e) => { if (!this.pinched(e, "up")) this.pointer(this.tapped(e) ? "tap" : "up", e); });
    this.canvas.addEventListener("pointerleave", leave);
    // Text over the canvas is the chart too: hovering it reaches the marks underneath (tooltips),
    // a click on it is a click on the chart. A press that moves is the reader selecting text: no
    // pointer capture (the browser's selection needs the pointer), and no click for the chart when
    // it ends. (Text where a press starts the chart's own drag — a pan, a brush — takes no pointer
    // at all: see `.pass`.)
    texts.addEventListener("pointermove", (e) => this.pointer("move", e));
    texts.addEventListener("pointerdown", (e) => {
      if (e.button !== 0) return; // a right click: the context menu (Copy)
      this.press(e, true);
      this.pointer("down", e);
    });
    texts.addEventListener("pointerup", (e) => { if (e.button === 0) this.pointer(this.tapped(e) ? "tap" : "up", e); });
    // A tooltip a tap pinned goes when the page scrolls or the reader taps elsewhere.
    addEventListener("scroll", this.onScroll, { passive: true, capture: true });
    document.addEventListener("pointerdown", this.onPagePress, { capture: true });
    texts.addEventListener("pointerout", leave);
    texts.addEventListener("wheel", wheel, { passive: false });
    // The narration card is page text (selectable as any): it hides what's under it, so the
    // pointer stops there — but the wheel still zooms an explorable view behind it.
    this.card.addEventListener("wheel", wheel, { passive: false });
    // Copying chart text: a line per text, in reading order.
    document.addEventListener("copy", this.onCopy);
    // A web font the layer's spans use arrived: they're stretched to the drawn widths again.
    document.fonts?.addEventListener?.("loadingdone", this.onFonts);
    void this.start();
  }

  private textLayer!: TextLayer;
  private textTimer = 0;
  /** A press in progress (on the canvas, or on text: possibly a selection), and whether it moved. */
  private pressed: { x: number; y: number; moved: boolean; text: boolean } | null = null;
  /** The release that ends a text selection is no click on the chart. */
  private suppressUp = false;

  private onCopy = (e: ClipboardEvent) => {
    const text = this.textLayer?.selectedText();
    if (text == null || !e.clipboardData) return;
    e.clipboardData.setData("text/plain", text);
    e.preventDefault();
  };
  private onFonts = () => this.textLayer?.fit();
  private onScroll = () => this.unpin();
  private onPagePress = (e: PointerEvent) => {
    if (!e.composedPath().includes(this)) this.unpin();
  };

  /** A press starts: until it ends (wherever the pointer is then), note whether it moves. */
  private press(e: PointerEvent, text: boolean) {
    this.pressed = { x: e.clientX, y: e.clientY, moved: false, text };
    // Selecting: over the chart's text the browser extends the selection itself; anywhere else
    // (the canvas, outside the chart) the text layer does (see `TextLayer.extendTo`). The page's
    // own text is unselectable meanwhile: the browser would try to carry the selection out of
    // this element's shadow tree into it, and make a mess of both.
    const page = document.documentElement.style;
    const pageSelect = [page.getPropertyValue("user-select"), page.getPropertyValue("-webkit-user-select")];
    const drag = (ev: MouseEvent) => {
      if (!this.pressed?.moved) return;
      const under = ev.composedPath()[0];
      if (under instanceof HTMLSpanElement && under.parentNode === this.textLayer.el) return;
      this.textLayer.extendTo(ev.clientX, ev.clientY);
    };
    if (text) {
      addEventListener("mousemove", drag, true);
      page.setProperty("user-select", "none");
      page.setProperty("-webkit-user-select", "none");
    }
    const end = (ev: PointerEvent) => {
      if (ev.pointerId !== e.pointerId) return;
      removeEventListener("pointerup", end, true);
      removeEventListener("pointercancel", end, true);
      if (text) {
        removeEventListener("mousemove", drag, true);
        page.setProperty("user-select", pageSelect[0]);
        page.setProperty("-webkit-user-select", pageSelect[1]);
      }
      const p = this.pressed;
      this.pressed = null;
      // This listener runs first (capture on the window): the chart's own `up` for this release
      // comes next, and is dropped.
      if (p?.text && (p.moved || ev.type === "pointercancel")) {
        this.suppressUp = true;
        setTimeout(() => { this.suppressUp = false; });
        this.scheduleTexts(0);
      }
    };
    addEventListener("pointerup", end, true);
    addEventListener("pointercancel", end, true);
  }

  /** Bring the text layer to the settled chart after `ms` (restarted by every frame until then). */
  private scheduleTexts(ms: number) {
    clearTimeout(this.textTimer);
    this.textTimer = window.setTimeout(() => {
      this.textTimer = 0;
      if (!this.view?.text_layer) return;
      // Not under a selection being made: its spans stay until the press ends.
      if (this.pressed?.text) return this.scheduleTexts(120);
      this.textLayer.update(this.view.text_layer());
      // The mirror too: a step's buttons (a sortable table's headers) are there once it settles.
      // And the height: text laid out in its fonts (they may have arrived since) may need more.
      const brief = this.view.chrome?.();
      const actions = brief?.actions;
      if (actions && actionsKey(actions) !== this.mirrorActions) this.syncChrome();
      else if (brief) this.growForText(brief);
    }, ms);
  }

  /** Taken off the page, the chart lets go of its engine at once — its scene, tables, GPU device
   * and canvas — rather than when the garbage collector gets round to it: every view on a page
   * shares one wasm memory, so a long article that unmounts charts far off screen stays small.
   * Put back, it starts again (`connectedCallback`). */
  disconnectedCallback() {
    this.generation++; // a start still loading gives up
    clearTimeout(this.describeTimer);
    this.describeTimer = 0;
    cancelAnimationFrame(this.raf);
    cancelAnimationFrame(this.layoutRaf);
    clearTimeout(this.timer);
    clearTimeout(this.textTimer);
    ((globalThis as any).cancelIdleCallback ?? clearTimeout)(this.idle);
    ((globalThis as any).cancelIdleCallback ?? clearTimeout)(this.describeIdle);
    this.describeIdle = 0;
    this.describedGen = -1;
    this.raf = 0;
    document.removeEventListener("copy", this.onCopy);
    document.fonts?.removeEventListener?.("loadingdone", this.onFonts);
    removeEventListener("scroll", this.onScroll, { capture: true });
    document.removeEventListener("pointerdown", this.onPagePress, { capture: true });
    this.textLayer?.reset();
    for (const stop of [this.stopScrub, this.stopSteps, this.stopPlayback]) stop?.();
    this.stopScrub = this.stopSteps = this.stopPlayback = this.applyPlayback = null;
    const drewOnGpu = this.dataset.renderer === "gpu";
    this.view?.free();
    this.view = null;
    if (drewOnGpu) releaseGl(this.canvas);
    this.profiler = null;
    setMoving(this, false);
    this.lastIndex = -1;
    this.fetching.clear();
    delete this.dataset.renderer;
  }

  /** The stage at the `height` attribute's size, before the engine or a layout pass is there. */
  private reserveHeight() {
    const h = Number(this.getAttribute("height"));
    if (this.stage && h > 0) this.stage.style.height = `${h}px`;
  }

  attributeChangedCallback(name: string) {
    if (name === "height") this.reserveHeight();
    if (name === "no-controls") this.showControls();
    if (name === "reduced-motion") this.applyPlayback?.();
    if (name === "text-scale") this.applyTextScale();
    if (!this.view) return;
    if (name === "mode") this.applyMode();
    if (name === "state") {
      // A step by index, or by name (`state="pie"`).
      const s = this.getAttribute("state") ?? "0";
      if (s !== "" && Number.isNaN(Number(s))) return this.send(`goto:${s}`);
      this.now(), this.timed(() => this.view.goto(Number(s) || 0)), this.kick();
    }
  }

  /** Show (or hide) the stats panel — "stats for nerds": renderer, frame times and a frame graph,
   * the renderer's caches, the transition in flight. Shift+D on a focused chart toggles it. */
  showStats(on = true) {
    if (!on && !this.profiler) return;
    const p = this.startProfiler();
    if (p) p.visible = on;
  }

  /** The frame profiler's record, for tools (`datars dev`'s Profile tab draws it): the last few
   * seconds of frames as `[engine ms, raster ms, gap ms]`, whether frames are coming now, and the
   * renderer's stats (backend, draws, meshes, caches). Starts the profiler, hidden, the first time;
   * each transition it measures after that is a `perf` event. Null until the chart is running. */
  frameRecord(): { frames: [number, number, number][]; moving: boolean; stats: Record<string, any> | null } | null {
    const p = this.startProfiler();
    return p ? { ...p.record(), stats: this.view?.nerd_stats?.() ?? null } : null;
  }

  private startProfiler(): FrameProfiler | null {
    if (!this.view?.frame_timing) return null;
    if (!this.profiler) {
      this.profiler = new FrameProfiler(
        this.root.querySelector<HTMLElement>(".perf")!,
        () => ({
          stats: this.view?.nerd_stats?.() ?? null,
          state: this.stateNames[this.view?.index() ?? 0] ?? "",
          index: this.view?.index() ?? 0,
          states: this.stateNames.length,
          size: this.size,
          src: this.getAttribute("src") ?? this.getAttribute("doc") ?? "(document)",
          memory: (this.wasm && memories.get(this.wasm)?.buffer.byteLength) ?? null,
        }),
        (s) => this.dispatchEvent(new CustomEvent("perf", { detail: s })),
      );
    }
    return this.profiler;
  }

  /** Run an input through the engine, telling the profiler when it came and what it cost (the
   * new state resolved and its transition planned happen here, before any frame). */
  private timed<T>(f: () => T): T {
    const from = this.view ? (this.stateNames[this.view.index()] ?? String(this.view.index())) : "";
    const t0 = performance.now();
    const r = f();
    this.profiler?.input(t0, performance.now() - t0, from);
    return r;
  }

  /** Bring the engine's clock to now before input: this page renders on demand, so the last frame
   * can be long ago, and a transition timed from it would already be over. */
  private now() {
    this.view?.set_clock?.(performance.now());
  }

  /** Load the `doc` attribute's document again and morph to it from what is on screen — the
   * edit loop (`datars dev` calls this when a file changes). Returns false when it can't (no
   * running view, or not a source document), so the page can reload instead. */
  async reload(): Promise<boolean> {
    const doc = this.getAttribute("doc");
    if (!this.view || !doc || !this.view.reload_doc) return false;
    const text = await (await fetch(doc, { cache: "no-store" })).text();
    this.now();
    this.view.reload_doc(text);
    this.textLayer.reset(); // its fonts may have changed too
    this.kick();
    this.syncChrome();
    return true;
  }

  /** Show a document held by the page (an editor's working copy, JSON text or object): the chart
   * morphs from what's on screen to it, keeping the step it's on. The first call starts the chart
   * when there's no `src`/`doc`. */
  async setDocument(doc: string | object): Promise<void> {
    const text = typeof doc === "string" ? doc : JSON.stringify(doc);
    this.pendingDoc = text;
    if (this.view?.ready() && this.view.reload_doc) {
      this.now();
      this.view.reload_doc(text);
      this.textLayer.reset();
      for (const [name, bytes] of this.hostData) {
        try { this.view.provide_source(name, bytes); } catch (e) { console.warn("datars: data", name, e); }
      }
      this.kick();
      this.syncChrome();
      return;
    }
    if (this.isConnected) await this.start();
  }

  /** Fire a program event (`next`, `prev`, `goto:<name>`, `back`). */
  send(ev: string) {
    this.now();
    if (this.timed(() => this.view?.event(ev))) {
      this.tip.hidden = true; // the chart moves on; so does what's under the pointer
      this.kick();
      this.syncChrome();
      this.dispatchEvent(new CustomEvent("state", { detail: this.stateDetail() }));
      this.noteSignals();
    }
  }

  /** Set a signal from the host page (filters, sliders, app state). */
  setSignal(name: string, value: unknown) {
    // Kept until the chart is running (and for its next start): set before it loads, it opens so.
    this.hostSignals.set(name, value);
    this.now();
    this.timed(() => this.view?.set_signal(name, JSON.stringify(value)));
    this.kick();
    this.noteSignals();
  }

  /** Every signal's value now, `{ name: value }` in the shapes `setSignal` takes: numbers,
   * strings, booleans, key sets as arrays of keys, null for nothing. Built-ins too: `inspected`
   * (the key under the pointer), a brush's `<name>.lo`, `.hi` and `.active`, an explorable view's
   * `<name>.x`, `.y` and `.zoom`. Null until the chart is running. A `signal` event tells when
   * they change (`detail: { signals, changed }`, the names that did). */
  get signals(): Record<string, unknown> | null {
    return this.view?.ready() && this.view.signals ? this.view.signals() : null;
  }

  /** The signals as last reported (JSON per name), to tell what an input changed. */
  private signalsSeen: Record<string, string> | null = null;
  /** After anything that may change a signal — the reader's pointer, wheel, keyboard or screen
   * reader, a step, the page's own `setSignal` — a `signal` event names those that changed (a
   * few dozen values compared: cheap enough for every pointer move over the chart). */
  private noteSignals() {
    const now = this.signals;
    if (!now) return;
    const seen: Record<string, string> = {};
    const changed: string[] = [];
    for (const [k, v] of Object.entries(now)) {
      seen[k] = JSON.stringify(v);
      if (this.signalsSeen && this.signalsSeen[k] !== seen[k]) changed.push(k);
    }
    const first = !this.signalsSeen;
    this.signalsSeen = seen;
    if (first || changed.length) this.dispatchEvent(new CustomEvent("signal", { detail: { signals: now, changed } }));
  }

  /** Read a tiles source from another archive: `datars dev` swaps in an automatic basemap rebuilt
   * as its map data arrives. The view keeps its state, and where a reader explored it to. */
  setTilesUrl(name: string, url: string) {
    this.view?.set_tiles_url?.(name, url);
    this.kick();
  }

  /** Where the views on screen look at their tile sources now — `[{source, bbox, zoom, state,
   * explore}]` (lon/lat bbox, fractional tile zoom). `datars dev` fetches map data for them. */
  tileViews(): { source: string; bbox: number[]; zoom: number; state: string; explore: boolean }[] {
    return JSON.parse(this.view?.tile_views?.() ?? "[]");
  }

  /** Override theme tokens (brand colours, fonts) — honours the theme's locks. */
  setTokens(tokens: Record<string, unknown>) {
    this.hostTokens = tokens;
    this.applyTokens();
  }

  /** The page's tokens, under the system's colours when it forces them. */
  private applyTokens() {
    this.view?.set_tokens(JSON.stringify(this.tokensNow()));
    this.kick();
    this.syncChrome();
  }

  private tokensNow(): Record<string, unknown> {
    return { ...(this.hostTokens ?? {}), ...this.forcedColors() };
  }

  /** With forced colours (a Windows contrast theme), the chart's text, rules and surfaces in the
   * system's colours (CanvasText on Canvas, Highlight for the accent) — the browser can't recolour
   * a canvas itself. Data colours stay: they tell the series apart. */
  private forcedColors(): Record<string, string> {
    if (typeof matchMedia === "undefined" || !matchMedia("(forced-colors: active)").matches || !this.root) return {};
    const probe = document.createElement("span");
    probe.style.cssText = "position:absolute;visibility:hidden;color:CanvasText;background-color:Canvas;border:1px solid Highlight;outline:1px solid GrayText";
    this.root.appendChild(probe);
    const cs = getComputedStyle(probe);
    const hex = (c: string) => {
      const m = /rgba?\((\d+),\s*(\d+),\s*(\d+)/.exec(c);
      return m ? `#${[m[1], m[2], m[3]].map((x) => Number(x).toString(16).padStart(2, "0")).join("")}` : "";
    };
    const [ink, paper, accent, gray] = [cs.color, cs.backgroundColor, cs.borderTopColor, cs.outlineColor].map(hex);
    probe.remove();
    if (!ink || !paper) return {};
    const out: Record<string, string> = { ink, paper, "ink-2": ink, muted: ink, rule: ink, "surface-ink": ink, "card-ink": ink, "card-ink-2": ink, surface: paper, card: paper, "card-line": ink, "map.label": ink, "map.label-halo": paper };
    if (accent) Object.assign(out, { accent, highlight: accent });
    // Gridlines recede: the system's text colour, faint (its GrayText can be a loud green).
    out.grid = `mix(${ink}, ${paper}, 0.7)`;
    void gray;
    return out;
  }

  /** The mode the chart shows in: the `mode` attribute, else the reader's — high contrast when
   * they ask for more (`prefers-contrast: more`) or the system forces its colours, else dark or
   * light as `prefers-color-scheme` says. */
  private wantedMode(): string {
    const a = this.getAttribute("mode");
    if (a) return a;
    if (typeof matchMedia === "undefined") return "light";
    if (matchMedia("(forced-colors: active)").matches || matchMedia("(prefers-contrast: more)").matches) return "high-contrast";
    return matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }

  /** With the profiler on: each start-up phase, [name, when (performance.now()), ms], collected
   * in `window.__datarsStarts` — what a chart coming into view costs the page (scrolling). */
  private startLog: { src: string; phases: [string, number, number][] } | null = null;
  private timed_<T>(name: string, f: () => T): T {
    const t0 = performance.now();
    const r = f();
    this.startLog?.phases.push([name, Math.round(t0), Math.round((performance.now() - t0) * 10) / 10]);
    return r;
  }

  /** Start, and say so when it fails: an `error` event (`detail.message`) and a short note in the
   * chart's box — never a silent blank or an unhandled rejection. A manifest refused by `publishers`
   * is one: the page asked for charts it can trust, and this one isn't signed by a key it named. */
  private async start() {
    const gen = this.generation + 1;
    try {
      await this.startChart();
    } catch (e) {
      if (gen !== this.generation || !this.isConnected) return; // a newer start, or off the page
      const message = e instanceof Error ? e.message : String(e);
      const refused = /signature|publisher|unsigned/i.test(message);
      this.poster.replaceChildren();
      clearTimeout(this.posterTimer);
      this.posterTimer = 0;
      const note = document.createElement("p");
      note.className = "fail";
      note.setAttribute("part", "error");
      note.setAttribute("role", "note");
      note.textContent = refused ? "Not shown: this chart couldn't be verified as signed by a publisher this page trusts." : "This chart couldn't be shown.";
      this.stage.querySelector(".fail")?.remove();
      this.stage.appendChild(note);
      this.dataset.failed = refused ? "refused" : "error";
      this.dispatchEvent(new CustomEvent("error", { detail: { message, refused } }));
      if (!refused) console.warn("datars:", message);
    }
  }

  private async startChart() {
    const gen = ++this.generation;
    this.stage.querySelector(".fail")?.remove();
    delete this.dataset.failed;
    this.startedAt = performance.now();
    this.engineWasHere = engineOnPage;
    this.startLog = perfRequested(this) ? { src: this.getAttribute("src") ?? this.getAttribute("doc") ?? "", phases: [] } : null;
    if (this.startLog) ((window as any).__datarsStarts ??= []).push(this.startLog);
    // Still wanted after an await? The element may have left the page (or started again) meanwhile.
    const live = () => gen === this.generation && this.isConnected;
    // One budget for the whole start to wait for the page to stop scrolling (`scrollPause`).
    const pauseUntil = performance.now() + START_PAUSE_MS;
    const onScreen = () => this.onScreen;
    const publishers = (this.getAttribute("publishers") ?? "").split(/\s+/).filter(Boolean);
    const allowScript = this.hasAttribute("allow-script") || !this.hasAttribute("no-script");
    const src = this.getAttribute("src");
    const doc = this.getAttribute("doc");
    this.base = new URL(src ?? doc ?? ".", location.href).href;
    // Pick the smallest engine that can play this chart: the manifest says what its variants need.
    let manifest: Uint8Array | null = null;
    let full = !!doc || !!this.pendingDoc || !!src?.endsWith(".datars");
    if (src && !full) {
      manifest = new Uint8Array(await (await fetch(src)).arrayBuffer());
      try {
        const m = JSON.parse(new TextDecoder().decode(manifest));
        full = !coreSuffices(m);
        // The poster downloads alongside the engine (not after it) — unless the page only trusts
        // signed charts: then nothing from the bundle shows before the engine has checked it.
        if (!publishers.length) void this.fetchPoster(m, this.base.replace(/[?#].*$/, "").replace(/\/c\/[^/]+$/, "").replace(/\/manifest\.json$/, ""), live);
      } catch { full = true; }
    }
    const kind = full && allowScript ? "full" : this.hasAttribute("cpu") || (await webgpuWorks) ? "core" : "core-gl";
    const wasm = await loadEngine(this.getAttribute("engine") ?? engineUrl(kind));
    if (!live()) return;
    // The view stays private to this function until it's attached: callbacks (resize, pointer,
    // visibility) must not reach it while `attach` holds it across an await.
    const view = new wasm.View(allowScript, publishers);
    const abandon = () => view.free();
    this.dataset.engine = kind;
    if (this.pendingDoc) {
      view.load_doc(this.pendingDoc);
    } else if (doc) {
      const d = await (await fetch(doc)).text();
      if (!live()) return abandon();
      view.load_doc(d);
    } else if (src && src.endsWith(".datars")) {
      const bytes = new Uint8Array(await (await fetch(src)).arrayBuffer());
      if (!live()) return abandon();
      view.open_file(bytes);
    } else if (src) {
      // Chunks sit next to the manifest's folder (`…/c/<alias>` → `…/chunks/<hash>`). Resolve the
      // manifest URL first: a relative `src="c/votes"` (a site under a subpath, e.g. GitHub Pages)
      // must find `chunks/` beside `c/`, not under it.
      const base = this.base.replace(/[?#].*$/, "").replace(/\/c\/[^/]+$/, "").replace(/\/manifest\.json$/, "");
      this.chunkBase = base;
      const mbytes = manifest ?? new Uint8Array(await (await fetch(src)).arrayBuffer());
      let need: string[] = this.timed_("manifest", () => view.open_manifest(mbytes));
      while (need.length) {
        this.showPoster(view);
        const got = await Promise.all(need.map(async (h) => [h, await fetchChunk(`${base}/chunks/${h.replace(":", "_")}`)] as const));
        // Opening the document (the last chunk in) is the heavy part: preferably not while another
        // chart moves or the page scrolls.
        await scrollPause(pauseUntil, onScreen);
        if (!live()) return abandon();
        for (const [h, b] of got) need = this.timed_("open", () => view.provide_chunk(h, b));
      }
    }
    // Data the page provided before the chart was here (a slot filled while the bundle loaded).
    for (const [name, bytes] of this.hostData) {
      try { view.provide_source(name, bytes); } catch (e) { console.warn("datars: data", name, e); }
    }
    // Likewise signals and theme tokens: the first frame already shows them, in the faces they name.
    const tokens = this.tokensNow();
    if (Object.keys(tokens).length) {
      try { view.set_tokens?.(JSON.stringify(tokens)); } catch (e) { console.warn("datars: tokens", e); }
      await this.themeFaces(view, live);
      if (!live()) return abandon();
    }
    for (const [name, value] of this.hostSignals) {
      try { view.set_signal?.(name, JSON.stringify(value)); } catch (e) { console.warn("datars: signal", name, e); }
    }
    // At the reader's text size from the first frame: never a theme-sized chart that then grows.
    view.set_text_scale?.((this.textScaleTold = this.textScale));
    this.timed_("poster", () => this.showPoster(view));
    if (!view.ready()) return abandon(); // T0 only: the poster and text are the chart
    // The `state` attribute is where the chart opens (no transition from the first state).
    const initial = this.getAttribute("state");
    if (initial && Number.isNaN(Number(initial))) view.event(`goto:${initial}`);
    else if (initial !== null && Number(initial) > 0) view.goto(Number(initial));
    const brief = view.chrome?.() ?? view.status(); // no semantics tree
    this.stateCount = brief.states?.length ?? 0;
    this.showControls();
    await scrollPause(pauseUntil, onScreen);
    if (!live()) return abandon();
    const attachAt = performance.now();
    const canvas = this.canvas;
    const mode = await oneAtATime<string>(() => view.attach(canvas, this.size.w, this.size.h, renderDpr(), this.hasAttribute("cpu")));
    this.renderGen = renderGen;
    this.startLog?.phases.push(["attach (wall)", Math.round(attachAt), Math.round((performance.now() - attachAt) * 10) / 10]);
    if (!live()) {
      abandon();
      if (mode === "gpu") releaseGl(canvas);
      return;
    }
    this.view = view;
    // The page's work share from the first frame (the engine starts at a native desktop's).
    if (view.set_work_scale) view.set_work_scale((this.workScale = workScale));
    this.dataset.renderer = mode;
    this.stateNames = (brief.states as string[] | undefined) ?? [];
    this.wasm = wasm;
    if (perfRequested(this)) this.showStats();
    this.timed_("layout", () => {
      this.applyMode();
      this.layout();
    });
    clearTimeout(this.posterTimer);
    this.posterTimer = 0;
    // The canvas takes over, fading in over the poster if one showed (a cut would flash); then the
    // poster's DOM goes (a dot map's poster is thousands of nodes).
    const hadPoster = this.poster.childElementCount > 0;
    this.canvas.style.opacity = hadPoster ? "0" : "";
    this.fadeIn = hadPoster;
    this.canvas.hidden = false;
    const dropPoster = () => {
      this.poster.hidden = true;
      this.timed_("poster drop", () => this.poster.replaceChildren());
    };
    if (hadPoster) setTimeout(dropPoster, 260);
    else dropPoster();
    this.syncChrome();
    this.watchPlayback();
    if (this.pendingSeek !== null) this.seek(this.pendingSeek);
    if (this.hasAttribute("scrub")) {
      const onScroll = () => this.scrub();
      addEventListener("scroll", onScroll, { passive: true });
      this.stopScrub = () => removeEventListener("scroll", onScroll);
      this.scrub();
    }
    if (this.hasAttribute("steps")) this.stopSteps = this.triggerSteps(this.getAttribute("steps")!);
    this.kick();
  }

  /** The arrows under a story, unless the page drives the steps itself (`no-controls`). */
  private stateCount = 0;
  private showControls() {
    const c = this.root.querySelector<HTMLElement>(".controls");
    if (c) c.hidden = this.stateCount < 2 || this.hasAttribute("no-controls");
  }

  /** When this start began, and whether an engine was already loaded then: a poster shows only
   * if the chart takes a while — never when the engine was here. */
  private startedAt = 0;
  private engineWasHere = false;

  /** The poster (the chart's T0 SVG), if loading takes long enough to need one — a chart ready
   * within POSTER_DELAY shows no poster at all (it would flash) — and only in the mode the chart
   * shows in (a light poster before a dark chart flashes too). */
  private showPoster(view: Wasm = this.view) {
    const svg: string | undefined = view?.poster();
    if (svg) this.displayPoster(svg);
  }

  /** The poster straight from the bundle, without the engine: the page's loader fetches it while
   * the (much bigger) engine downloads, so on a slow first visit the chart's picture is there
   * early. Posters too big to be worth it on a slow connection are skipped. */
  private async fetchPoster(manifest: { chunks?: { hash: string; kind: string; bytes?: number }[] }, chunkBase: string, live: () => boolean) {
    const c = manifest.chunks?.find((c) => c.kind === "poster");
    if (!c || (c.bytes ?? 0) > 200_000) return;
    try {
      const svg = await (await fetch(`${chunkBase}/chunks/${c.hash.replace(":", "_")}`)).text();
      if (live()) this.displayPoster(svg);
    } catch {
      /* the engine's copy may still show */
    }
  }

  private displayPoster(svg: string) {
    if (this.poster.innerHTML || this.posterTimer || this.engineWasHere) return;
    // The poster is the published look: before a chart the page restyles, it would flash.
    if (Object.keys(this.tokensNow()).length) return;
    const want = this.wantedMode();
    const drawn = /data-mode="(\w+)"/.exec(svg.slice(0, 300))?.[1];
    if (drawn && drawn !== want) return;
    // Drawn at the theme's text size: for a reader with bigger text, the chart would change under
    // it as it took over.
    if (Math.abs(this.textScale - 1) > 0.01) return;
    const show = () => {
      this.posterTimer = 0;
      if (!this.view && this.isConnected && !this.poster.innerHTML) {
        this.poster.innerHTML = svg;
        this.startLog?.phases.push(["poster shown", Math.round(performance.now()), 0]);
      }
    };
    const wait = POSTER_DELAY - (performance.now() - this.startedAt);
    if (wait > 0) this.posterTimer = window.setTimeout(show, wait);
    else show();
  }
  private posterTimer = 0;
  /** The work share this chart's engine was last given (see `noteFrame`). */
  private workScale = 1; // what this view's engine was last told (it starts at 1)
  /** The page's render-scale generation this canvas is sized for (see `renderDpr`). */
  private renderGen = -1;
  /** The last moving frame's rAF timestamp (0: the previous frame didn't move). */
  private lastTick = 0;
  /** When a frame only a clock asks for may run next (see `tick`). */
  private clockAt = 0;

  private applyMode() {
    this.view?.set_mode(this.wantedMode());
    this.syncChrome();
    this.kick();
  }

  /** Taller than the page asked, for the reader's bigger text (see `growForText`): the width and
   * text size it was for, and the height. */
  private grown = { key: "", h: 0 };

  /** A reader's bigger text can make a chart's fixed-size content — rows of controls, a list of
   * goals, a stack of tiles — taller than the height the page gave it: rather than letting its
   * text run off the bottom, the element grows to fit (only past the theme's text size, so a
   * page's layout holds for every other reader). */
  private growForText(s: Record<string, any>) {
    const want = Number(s.contentHeight);
    const key = `${this.size.w}|${this.textScale}`;
    if (!(this.textScale > 1.001) || !(want > this.size.h + 1) || (this.grown.key === key && this.grown.h >= want)) return;
    this.grown = { key, h: Math.ceil(want) };
    this.layout();
  }

  private layout() {
    const w = this.clientWidth || 800;
    const aspect = this.view ? 0 : 0.6;
    const asked = Number(this.getAttribute("height")) || Math.round(w * (aspect || 0.6));
    const h = this.grown.key === `${w}|${this.textScale}` ? Math.max(asked, this.grown.h) : asked;
    this.stage.style.height = `${h}px`;
    // A new size lays the chart out again: its text moves.
    if (w !== this.size.w || h !== this.size.h) this.textLayer?.hide();
    this.size = { w, h };
    this.resizeCanvas();
    this.kick();
  }

  /** The canvas's backing store at the page's render resolution (see `renderDpr`). */
  private resizeCanvas() {
    const { w, h } = this.size;
    const dpr = renderDpr();
    this.canvas.width = Math.round(w * dpr);
    this.canvas.height = Math.round(h * dpr);
    this.view?.resize(w, h, dpr);
    this.renderGen = renderGen;
  }

  /** Fingers on the canvas (touch pointers), and the distance between two of them while they
   * pinch (0 when not pinching: until every finger has lifted, one left behind doesn't pan). */
  private touches = new Map<number, [number, number]>();
  private pinch = 0;

  /** Two fingers on an explorable view: their spread zooms it around their midpoint, as a wheel
   * would. Returns whether the pointer event was the pinch's (the chart doesn't see it as a press,
   * a drag or a tap). */
  private pinched(e: PointerEvent, kind: "down" | "move" | "up"): boolean {
    if (e.pointerType !== "touch" || !this.view) return false;
    const r = this.canvas.getBoundingClientRect();
    const p: [number, number] = [e.clientX - r.left, e.clientY - r.top];
    if (kind === "up") {
      this.touches.delete(e.pointerId);
      const was = this.pinch > 0;
      if (!this.touches.size) this.pinch = 0;
      if (was) this.pressAt = null;
      return was;
    }
    if (kind === "down") this.touches.set(e.pointerId, p);
    else if (this.touches.has(e.pointerId)) this.touches.set(e.pointerId, p);
    if (this.touches.size < 2) return this.pinch > 0;
    const [a, b] = [...this.touches.values()];
    const spread = Math.hypot(a[0] - b[0], a[1] - b[1]);
    const mid: [number, number] = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2];
    if (!this.pinch) {
      // Only over a view that pans (else two fingers are the page's: its own zoom, a scroll).
      if (!this.view.drags_at?.(mid[0], mid[1])) return false;
      this.view.pointer("leave", 0, 0); // the first finger's pan ends here
      this.pinch = spread || 1;
      this.textLayer.hide();
      return true;
    }
    // Both fingers move at every step of a pinch: the zoom follows once a frame, not once per
    // finger's event (each re-lays the view out).
    if (spread > 0) {
      this.pinchTo = { spread, mid };
      this.pinchRaf ||= requestAnimationFrame(() => {
        this.pinchRaf = 0;
        const to = this.pinchTo;
        if (!to || !this.pinch || !this.view) return;
        if (this.view.wheel(to.mid[0], to.mid[1], -Math.log(to.spread / this.pinch) / 0.0015)) {
          this.tip.hidden = true;
          this.kick();
          this.noteSignals();
        }
        this.pinch = to.spread;
      });
    }
    return true;
  }
  private pinchTo: { spread: number; mid: [number, number] } | null = null;
  private pinchRaf = 0;

  /** Where the last press went down, to tell a click from a drag. */
  private pressAt: [number, number] | null = null;
  /** A tap's tooltip, shown until the next tap, a scroll or a tap elsewhere (touch has no hover to
   * end it). */
  private tipPinned = false;

  /** A touch (or pen) press released where it went down: a tap, the reader asking what's there —
   * the engine inspects what it lands on, with a finger's reach. */
  private tapped(e: PointerEvent): boolean {
    if (e.pointerType === "mouse" || !this.pressAt) return false;
    const r = this.canvas.getBoundingClientRect();
    return Math.hypot(e.clientX - r.left - this.pressAt[0], e.clientY - r.top - this.pressAt[1]) < 10;
  }

  /** Put away a tap's tooltip and what it inspected. */
  private unpin() {
    if (!this.tipPinned) return;
    this.tipPinned = false;
    this.tip.hidden = true;
    this.view?.pointer("leave", 0, 0);
    this.kick();
  }

  /** What's under a point (CSS px from the element's top left): every element hit, top first
   * (`{ path, kind, role, label, bounds }`), and the top datum's data row (`{ table, index,
   * fields }`) when there is one. */
  pick(x: number, y: number): { x: number; y: number; hits: { path: string; role?: string; label?: string }[]; row: unknown } {
    const hits = (this.view?.hit_test(x, y) ?? []) as { path: string; role?: string; label?: string }[];
    const datum = hits.find((h) => h.role === "datum" || h.role === "region") ?? null;
    const why = datum && this.view?.explain ? (this.view.explain(datum.path) as { row?: unknown }[]) : [];
    return { x, y, hits, row: why?.[0]?.row ?? null };
  }

  private pointer(kind: string, e: PointerEvent) {
    if (!this.view) return;
    const p = this.pressed;
    if (p && !p.moved && Math.abs(e.clientX - p.x) + Math.abs(e.clientY - p.y) > 4) {
      p.moved = true;
      // A drag on the canvas (a pan, a brush) moves the text. One from text selects it: the chart
      // lets go of the press (and of any hover) — no brush or pan starts under a selection.
      if (!p.text) this.textLayer.hide();
      else this.view.pointer("leave", 0, 0);
    }
    if (p?.text && p.moved && kind === "move") {
      this.tip.hidden = true; // selecting text: no tooltips on the way
      return;
    }
    if ((kind === "up" || kind === "tap") && this.suppressUp) {
      this.suppressUp = false;
      return;
    }
    const r = this.canvas.getBoundingClientRect();
    const x = e.clientX - r.left, y = e.clientY - r.top;
    // A press released where it went down is a click: hosts hear what it landed on (`pick`) —
    // an app routes it, a notebook widget sends it to Python.
    if (kind === "down") this.pressAt = [x, y];
    const release = kind === "up" || kind === "tap";
    if (release && this.pressAt && Math.hypot(x - this.pressAt[0], y - this.pressAt[1]) < (kind === "tap" ? 10 : 5)) {
      this.dispatchEvent(new CustomEvent("pick", { detail: this.pick(x, y) }));
    }
    if (release) this.pressAt = null;
    this.now();
    const label: string | undefined = this.view.pointer(kind, x, y);
    if (release) this.sayChange();
    // The engine says what the pointer can do here: a hand over what a click acts on, grab over a
    // view that pans, a crosshair over a brushable area.
    if (e.pointerType !== "touch") {
      const cursor: string = this.view.cursor?.() ?? "default";
      if (this.canvas.style.cursor !== cursor) this.canvas.style.cursor = cursor;
    }
    if (label && kind !== "down") {
      this.tip.textContent = label;
      this.tip.hidden = false;
      this.tipPinned = kind === "tap";
      const tw = this.tip.offsetWidth, th = this.tip.offsetHeight, gap = 12;
      let left: number, top: number;
      if (kind === "tap") {
        // Above the finger (it covers what's below it), centred on it; below where there's no room.
        left = x - tw / 2;
        top = y - 28 - th >= 0 ? y - 28 - th : y + 28;
      } else {
        // Beside the pointer, flipped to the other side near an edge so it never leaves the chart.
        left = x + gap + tw > this.size.w ? x - gap - tw : x + gap;
        top = y + gap + th > this.size.h ? y - gap - th : y + gap;
      }
      this.tip.style.left = `${Math.max(0, Math.min(left, this.size.w - tw))}px`;
      this.tip.style.top = `${Math.max(0, Math.min(top, this.size.h - th))}px`;
    } else if (kind === "move" || kind === "tap" || (kind === "up" && e.pointerType !== "mouse")) {
      // Nothing there, or a finger's drag ended: no tooltip.
      this.tip.hidden = true;
      this.tipPinned = false;
    }
    this.kick();
    if (release) this.syncChrome();
    this.noteSignals();
  }

  /** Run frames until nothing moves (render on demand — battery matters). */
  private kick() {
    if (this.raf || !this.view?.ready()) return;
    clearTimeout(this.timer);
    const tick = (now: number) => {
      this.raf = 0;
      if (this.drawn && !this.onScreen) {
        setMoving(this, false);
        return; // picked up again by the visibility observer
      }
      // Another chart is in transition and this one is only busy (a clock, levels or tiles
      // arriving): every third frame, so the one being read keeps its frame rate.
      if (moving.size > 0 && !moving.has(this) && !this.view.transitioning?.() && ++this.yielded % 3 !== 0) {
        this.raf = requestAnimationFrame(tick);
        return;
      }
      // A clock (a turning globe) moves the scene a fraction of a pixel a frame: on a slow device,
      // where re-drawing it costs most of a frame, it draws less often (the clock runs on time, so
      // as fast) and leaves the main thread to the reader — a tap, a scroll, the next step.
      if (this.drawn && now < this.clockAt && !this.view.transitioning?.()) {
        this.raf = requestAnimationFrame(tick);
        return;
      }
      // The page's render scale changed (see `noteGpu`): this canvas follows, before drawing.
      if (this.renderGen !== renderGen) this.resizeCanvas();
      const t0 = performance.now();
      const animating = this.drawn ? this.view.frame(now) : this.timed_("first frame", () => this.view.frame(now));
      if (animating && this.drawn) {
        const mainMs = performance.now() - t0;
        const s = noteFrame(mainMs);
        if (Math.abs(s - this.workScale) > 0.04 && this.view.set_work_scale) this.view.set_work_scale((this.workScale = s));
        if (this.lastTick) noteGpu(now - this.lastTick, now);
      }
      this.lastTick = animating ? now : 0;
      this.drawn = true;
      if (this.fadeIn) {
        this.fadeIn = false;
        this.canvas.style.opacity = "1"; // the first frame is drawn: fade it in (see `.stage canvas`)
      }
      const transition = animating && (this.view.transitioning?.() ?? true);
      // At most 40% of the main thread for clock frames: the next waits 1.5× this one's work (a
      // frame under ~10 ms waits for nothing).
      this.clockAt = animating && !transition ? now + 1.5 * (performance.now() - t0) : 0;
      setMoving(this, transition);
      if (!transition) this.describeSoon();
      // The text layer follows the settled chart: hidden while a transition carries the text
      // elsewhere, rebuilt a moment after the last frame (a clock or tiles arriving keep frames
      // coming without moving much: every so often then).
      if (transition) this.textLayer.hide();
      if (!animating || transition || !this.textTimer) this.scheduleTexts(animating && !transition ? 400 : 80);
      this.profiler?.frame(now, this.view.frame_timing(), animating, this.stateNames[this.view.index()] ?? String(this.view.index()));
      const pageStart = performance.now();
      this.fetchData();
      const index = this.view.index();
      if (index !== this.lastIndex) {
        // Autoplay (or a scrub) moved the program: narration, mirror and listeners follow.
        this.lastIndex = index;
        this.syncChrome();
        this.dispatchEvent(new CustomEvent("state", { detail: this.stateDetail() }));
        this.noteSignals();
      }
      this.placeCard(animating);
      this.placeLinks();
      this.profiler?.page(performance.now() - pageStart);
      if (animating) {
        this.raf = requestAnimationFrame(tick);
      } else {
        this.prepareIdle();
        // Sleep until the engine has something to do (a hold ends, a live refresh) — no frame loop.
        const wake = this.view.wake_at();
        if (Number.isFinite(wake)) this.timer = window.setTimeout(() => this.kick(), Math.max(0, wake - performance.now()));
      }
    };
    this.raf = requestAnimationFrame(tick);
  }

  /** While the reader is on a step, let the engine prepare the steps next to it in idle time
   * (resolve and plan), so the next one starts moving at once. */
  private prepareIdle() {
    if (!this.view?.prepare) return;
    const ric: (f: (d: { timeRemaining(): number }) => void, o?: { timeout: number }) => number =
      (globalThis as any).requestIdleCallback ?? ((f) => window.setTimeout(() => f({ timeRemaining: () => 8 }), 60));
    const cic: (h: number) => void = (globalThis as any).cancelIdleCallback ?? clearTimeout;
    cic(this.idle);
    this.idle = ric((d) => {
      this.idle = 0;
      if (!this.view || this.raf) return; // moving again: the frames come first
      // Another chart on the page moving: its frames come first too.
      if (moving.size > 0) return void quietTurn(5000).then(() => this.prepareIdle());
      let more = true;
      while (more && d.timeRemaining() > 2 && moving.size === 0) more = this.view.prepare();
      // What the prepared steps will need (map tiles along their flights, around the view)
      // downloads now, behind anything the view itself asks for.
      this.fetchData(true);
      if (more) this.prepareIdle();
    }, { timeout: 2000 });
  }

  /** Fulfil the engine's data requests (URL sources: first loads and live refreshes; tile archive
   * ranges as views need tiles), relative to the bundle's address. Slots are left to the host page
   * (`provideData`). */
  /** `ahead`: asked for in idle time (the next steps, around the view) — fetched behind what the
   * view shows now. */
  private fetchData(ahead = false) {
    this.fetchChunks();
    // Answers go to the engine that asked: one freed meanwhile (the element left) gets nothing.
    const view = this.view;
    const reqs: { name: string; url?: string; range?: [number, number]; atlas?: string }[] = JSON.parse(this.view?.data_requests() ?? "[]");
    for (const r of reqs) {
      if (r.atlas) {
        // A built-in atlas for a raw document: from the runtime's own `atlas/` folder (or the
        // page's `atlas-base`, or a file the page supplied).
        if (this.fetching.has(r.name)) continue;
        this.fetching.add(r.name);
        const base = this.getAttribute("atlas-base");
        fetch(base ? new URL(`${r.atlas}.geojson`, base) : runtimeFile(`atlas/${r.atlas}.geojson`))
          .then((res) => (res.ok ? res.arrayBuffer() : Promise.reject(new Error(`${res.status} atlas ${r.atlas}`))))
          .then((b) => { if (this.view !== view) return; view.provide_source(r.name, new Uint8Array(b)); this.kick(); })
          .catch((e) => console.warn("datars: atlas", r.name, e))
          // A new document (setDocument, reload) asks for its atlas again: answer it again.
          .finally(() => this.fetching.delete(r.name));
        continue;
      }
      if (r.url && r.range) {
        this.fetchRange(r.name, r.url, r.range[0], r.range[1], ahead);
        continue;
      }
      if (!r.url || this.fetching.has(r.name)) continue;
      const got = this.requestBytes(r.name, r.url);
      if (!got) continue;
      this.fetching.add(r.name);
      got
        .then((b) => { if (this.view !== view) return; view.provide_source(r.name, b); this.noteData(); this.kick(); })
        .catch((e) => console.warn("datars: data", r.name, e))
        .finally(() => this.fetching.delete(r.name));
    }
  }

  /** New rows (the page's, a live refresh): what the mirror says is stale — rebuilt once the
   * chart settles (see `describeSoon`). */
  private noteData() {
    this.dataStale = true;
    this.chromeGen++;
    this.describeSoon();
  }

  /** The bytes a request asks for — as the page answers it, else fetched — or null for one no
   * runtime fetches (a Google Fonts family: a published bundle carries it). */
  private requestBytes(name: string, requested: string): Promise<Uint8Array> | null {
    // Fonts: the default ones ship next to the runtime; a `font-server` (the dev server) acquires
    // the rest the way the build step does.
    let url: string | URL = new URL(requested, this.base);
    const fontServer = this.getAttribute("font-server");
    if (requested.startsWith("datars:")) url = datarsAsset(requested);
    else if (fontServer && isFontRequest(name, requested)) url = new URL(`${fontServer}?src=${encodeURIComponent(requested)}`, location.href);
    else if (requested.startsWith("google:")) {
      if (!this.warnedGoogle) console.warn(`datars: ${requested} is a Google Fonts family; publish a bundle (or use \`datars dev\`) to ship it`);
      this.warnedGoogle = true;
      return null;
    }
    // The page may answer the request itself — its own API client, auth headers, a mock, a
    // replayed feed: a `datarequest` listener calls `preventDefault()` and `respond(data)` (bytes,
    // text, an object, or a promise of one). Live sources ask again on every refresh.
    let answer: Promise<unknown> | null = null;
    const ev = new CustomEvent("datarequest", { cancelable: true, detail: { name, url: String(url), respond: (d: unknown) => { answer = Promise.resolve(d); } } });
    return !this.dispatchEvent(ev) && answer
      ? (answer as Promise<unknown>).then((d) => (d instanceof Uint8Array ? d : d instanceof ArrayBuffer ? new Uint8Array(d) : new TextEncoder().encode(typeof d === "string" ? d : JSON.stringify(d))))
      : fetch(url, { cache: "no-cache" }).then((res) => (res.ok ? res.arrayBuffer() : Promise.reject(new Error(`${res.status} ${requested}`)))).then((b) => new Uint8Array(b));
  }

  /** The faces the page's tokens name (a reader's chosen type), before the chart's first frame: it
   * opens in them rather than in a fallback that then re-flows. A face slower than this arrives
   * like any other font, afterwards. */
  private async themeFaces(view: Wasm, live: () => boolean) {
    const reqs: { name: string; url?: string }[] = JSON.parse(view.data_requests?.() ?? "[]");
    const faces = reqs.filter((r) => r.name.startsWith("font:") && r.url && !this.fetching.has(r.name));
    if (!faces.length) return;
    const loads = faces.map((r) => {
      const got = this.requestBytes(r.name, r.url!);
      if (!got) return Promise.resolve();
      this.fetching.add(r.name);
      return got
        .then((b) => { if (live()) view.provide_source(r.name, b); this.kick(); })
        .catch((e) => console.warn("datars: font", r.name, e))
        .finally(() => this.fetching.delete(r.name));
    });
    await Promise.race([Promise.all(loads), new Promise((ok) => setTimeout(ok, THEME_FACE_WAIT))]);
  }

  /** Lazily loaded bundle chunks the engine asked for after opening (font subsets for scripts
   * that runtime text needs), from the bundle's `chunks/`. */
  private fetchChunks() {
    const view = this.view;
    const need: string[] = view?.chunk_requests?.() ?? [];
    for (const h of need) {
      const id = `chunk:${h}`;
      if (!this.chunkBase || this.fetching.has(id)) continue;
      this.fetching.add(id);
      fetchChunk(`${this.chunkBase}/chunks/${h.replace(":", "_")}`)
        .then((b) => { if (this.view !== view) return; view.provide_chunk(h, b); this.kick(); })
        .catch((e) => console.warn("datars: chunk", h, e));
    }
  }

  /** A tile archive is read by HTTP `Range` request, once per range. A server that ignores `Range`
   * sends the whole archive, which the engine then reads from memory. A failed range stays marked
   * (not retried in a loop). */
  private fetchRange(name: string, url: string, offset: number, length: number, ahead = false) {
    const id = `${name}@${offset}`;
    // Asked for ahead and still queued, now wanted on screen: it goes first.
    const queued = this.ahead.get(id);
    if (queued && !ahead) {
      this.ahead.delete(id);
      queued();
      return;
    }
    if (this.fetching.has(id)) return;
    this.fetching.add(id);
    const view = this.view;
    const go = () => {
      this.aheadInFlight += ahead ? 1 : 0;
      this.fetchNow(name, url, offset, length, id, view, ahead);
    };
    // Fetching ahead keeps to a couple of requests at a time, so it never holds up what the
    // view is waiting for.
    if (ahead && this.aheadInFlight >= 2) this.ahead.set(id, go);
    else go();
  }

  /** Queued fetches ahead, by range id, and how many ahead are in flight. */
  private ahead = new Map<string, () => void>();
  private aheadInFlight = 0;

  private pumpAhead() {
    for (const [id, go] of this.ahead) {
      if (this.aheadInFlight >= 2) break;
      this.ahead.delete(id);
      go();
    }
  }

  private fetchNow(name: string, url: string, offset: number, length: number, id: string, view: Wasm, ahead: boolean) {
    fetch(new URL(url, this.base), { headers: { Range: `bytes=${offset}-${offset + length - 1}` } })
      .then(async (res) => {
        if (!res.ok) throw new Error(`${res.status} ${url}`);
        const bytes = new Uint8Array(await res.arrayBuffer());
        if (this.view !== view) return;
        const t0 = performance.now();
        if (res.status === 206) view.provide_range(name, offset, bytes);
        else view.provide_source(name, bytes);
        this.profiler?.data(performance.now() - t0);
        this.fetching.delete(id);
        this.kick();
      })
      .catch((e) => console.warn("datars: tiles", name, e))
      .finally(() => {
        if (ahead) {
          this.aheadInFlight--;
          this.pumpAhead();
        }
      });
  }

  /** Provide data for a slot (or replace a source) from the host page: CSV or JSON bytes/text, or
   * rows (an array of records / an object of columns). Kept and applied whenever the chart is
   * ready, so it can be called before the bundle has loaded. Throws if the data doesn't have the
   * columns the chart needs (the chart keeps showing what it had). */
  provideData(name: string, data: string | Uint8Array | object) {
    const bytes = data instanceof Uint8Array ? data : new TextEncoder().encode(typeof data === "string" ? data : JSON.stringify(data));
    if (this.view?.ready()) this.view.provide_source(name, bytes);
    this.hostData.set(name, bytes);
    this.noteData();
    this.kick();
  }

  /** Every element drawn under a point (CSS px, relative to the element), topmost first —
   * pickable or not: `[{ path, kind, role, label, text, bounds: [x, y, w, h] }]`. For editors. */
  hitTest(x: number, y: number): { path: string; kind: string; role?: string; label?: string; text?: string; bounds: [number, number, number, number] }[] {
    return this.view?.ready() ? this.view.hit_test(x, y) : [];
  }

  /** Why an element looks as it does, on the live chart: recipes, data row, expression values,
   * template and drawn node (`path` as `hitTest` returns it). */
  explain(path: string): unknown[] {
    return this.view?.ready() ? this.view.explain(path) : [];
  }

  /** The chart's state: current step, steps, narration, semantics, theme tokens, diagnostics. */
  get status(): Record<string, unknown> | null {
    return this.view?.ready() ? this.view.status() : null;
  }

  /** What a screen reader walks through, in order — the chart (its name, and its summary as
   * `value`), headings, axes and legends as a line each, data marks with their values apart,
   * checkboxes, switches and radio buttons with `checked`, buttons: `[{kind, label, value, rect,
   * path, checked?}]`, `rect` in CSS px from the chart's corner. The same order the native
   * runtimes give VoiceOver and TalkBack; the element's own hidden content follows it (with data
   * runs as tables). `null` until loaded. Reads the whole semantics tree: not every frame. */
  get readout(): { kind: string; label: string; value: string; rect: number[] | null; path: string; checked?: boolean }[] | null {
    return this.view?.ready() && this.view.readout ? this.view.readout() : null;
  }

  /** What the last frame drew from big data: `rows` in its point pyramids, points `drawn`, `tiles`
   * and the deepest `level`; archive `bytes` and range `requests` fetched so far. */
  get stats(): { rows: number; drawn: number; tiles: number; level: number; bytes: number; requests: number } | null {
    return this.view?.ready() && this.view.stats ? this.view.stats() : null;
  }

  /** `el.data = { spending: rows }`: every slot at once (see `provideData`). */
  set data(slots: Record<string, string | Uint8Array | object>) {
    for (const [name, d] of Object.entries(slots)) this.provideData(name, d);
  }

  /** Scroll scrub (`scrub` attribute): the program follows the element's scroll container. The
   * container is the nearest `[data-scrub]` ancestor, else the parent; its passage through the
   * viewport maps to positions 0 … states − 1. */
  private scrub() {
    if (!this.view?.ready()) return;
    const box = (this.closest("[data-scrub]") ?? this.parentElement ?? this).getBoundingClientRect();
    const span = box.height - innerHeight;
    const progress = span > 0 ? Math.min(1, Math.max(0, -box.top / span)) : 0;
    this.seek(progress * Math.max(0, this.view.state_count() - 1));
  }

  /** Put the program at `position`, in states: `0` is the first state, `states − 1` the last, and
   * a fraction is the transition between two states that far through — the frame it would show
   * then, exactly. For scrubbers and scroll-driven stories (`scrub` uses it). Nothing animates on
   * its own: the chart holds the position until the next `seek`, step or event. Asked before the
   * chart has opened, it's applied when it does. */
  seek(position: number) {
    if (!Number.isFinite(position)) return;
    if (!this.view?.ready()) {
      this.pendingSeek = position;
      return;
    }
    this.pendingSeek = null;
    const n = Math.max(0, this.view.state_count() - 1);
    this.now();
    this.view.seek(Math.min(n, Math.max(0, position)));
    this.kick();
  }

  private em: HTMLSpanElement | null = null;
  /** The text size the engine was last given. */
  private textScaleTold = 1;

  /** The chart's text size as a multiple of the theme's: the `text-scale` attribute when it's a
   * number (`1`: the theme's own sizes, whatever the reader's setting), else — `auto` or no
   * attribute — the reader's: the browser's default font size over 16 px (Settings › Appearance ›
   * Font size; 20 px reads as 1.25). Page zoom isn't text size: it scales the whole chart already.
   * The engine keeps it between 0.75 and 3. */
  get textScale(): number {
    const a = this.getAttribute("text-scale");
    const n = a === null || a === "" || a === "auto" ? NaN : Number(a);
    if (Number.isFinite(n) && n > 0) return n;
    const px = this.em?.getBoundingClientRect().width || (typeof document !== "undefined" ? parseFloat(getComputedStyle(document.documentElement).fontSize) : 16) || 16;
    return Math.round((px / 16) * 1000) / 1000;
  }

  set textScale(v: number | "auto" | null) {
    if (v === null || v === "auto") this.removeAttribute("text-scale");
    else this.setAttribute("text-scale", String(v));
  }

  /** A new text size (the reader's setting changed, or the attribute): the chart lays out again
   * around it, moving there like any other change. */
  private applyTextScale() {
    const s = this.textScale;
    if (!this.view || Math.abs(s - this.textScaleTold) < 1e-3) return;
    this.textScaleTold = s;
    this.now();
    this.view.set_text_scale?.(s);
    this.textLayer?.hide();
    this.kick();
    this.scheduleTexts(400);
    // (Taller text may need a taller chart: the status says.)
    this.syncChrome();
  }

  /** Whether the chart plays reduced motion now: the `reduced-motion` attribute when it says
   * (`reduce`, or `no-preference` for a reader who opted in to the full motion), else the reader's
   * `prefers-reduced-motion`. */
  get reducedMotion(): boolean {
    const a = this.getAttribute("reduced-motion");
    if (a === null) return typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches;
    return a !== "no-preference";
  }

  /** Scroll trigger (`steps=".step"`): each matching element on the page is a story step; when one
   * crosses the middle of the viewport the program goes to that step (by index, or by its
   * `data-state` name) and the transition plays. */
  private triggerSteps(selector: string): () => void {
    const steps = [...document.querySelectorAll<HTMLElement>(selector)];
    const io = new IntersectionObserver((entries) => {
      // The step at the middle now: steps a fast scroll crossed in the same frame would each plan
      // a transition only for the next to replace it.
      const hits = entries.filter((e) => e.isIntersecting);
      const el = hits[hits.length - 1]?.target as HTMLElement | undefined;
      if (!el) return;
      const name = el.dataset.state;
      if (name) return this.send(`goto:${name}`);
      const i = steps.indexOf(el);
      if (i < 0 || !this.view) return;
      // The engine's clock is the last frame's: a chart that has been still while the reader read
      // the step would start the transition seconds in the past, and it would jump to its end.
      this.now();
      if (this.timed(() => this.view.goto(i))) { this.kick(); this.syncChrome(); }
    }, { rootMargin: "-50% 0px -50% 0px" });
    steps.forEach((s) => io.observe(s));
    return () => io.disconnect();
  }

  /** Autoplay runs only while visible and without reduced motion (the reader's setting, or the
   * `reduced-motion` attribute: see `reducedMotion`). */
  private watchPlayback() {
    const reduced = matchMedia("(prefers-reduced-motion: reduce)");
    let visible = true;
    const apply = () => {
      const r = this.reducedMotion;
      this.view?.set_reduced_motion(r);
      this.view?.set_playing(visible && !r);
      this.kick();
    };
    const io = new IntersectionObserver((es) => { visible = es.some((e) => e.isIntersecting); apply(); });
    io.observe(this);
    reduced.addEventListener("change", apply);
    this.applyPlayback = apply;
    this.stopPlayback = () => { io.disconnect(); reduced.removeEventListener("change", apply); };
    apply();
  }

  /** A `state` event's detail: the status, its `semantics` made only when a listener reads them
   * (thousands of items on a map of counties: every step would pay for them). */
  private stateDetail(): Record<string, unknown> {
    const view = this.view;
    if (!view.chrome || !view.semantics) return view.status();
    const s = view.chrome() as Record<string, unknown>;
    let semantics: unknown;
    Object.defineProperty(s, "semantics", { enumerable: true, get: () => (semantics ??= view.semantics()) });
    return s;
  }

  private syncChrome() {
    if (!this.view) return;
    // Without the semantics tree (thousands of items on a big map): not needed here.
    const s = this.view.chrome?.() ?? this.view.status();
    if (s.phrases) this.phrases = s.phrases;
    // The chart's words are in the document's language (WCAG 3.1.2): its content, its
    // announcements and its buttons say so to a screen reader's voice and a braille table.
    if (s.locale) for (const el of [this.mirror, this.live, this.root.querySelector<HTMLElement>(".controls")]) if (el && el.lang !== s.locale) el.lang = s.locale;
    const t = s.tokens?.tokens ?? {};
    for (const k of ["ink", "paper", "muted", "rule", "surface", "surface-ink", "accent"]) if (t[k]) this.style.setProperty(`--${k}`, t[k]);
    const n = s.narration;
    // A chart that draws its own narration (a card on the canvas) gets no second one here; the
    // live region below still announces it.
    this.card.innerHTML = n && (n.title || n.text) && !s.narrationDrawn ? `${n.title ? `<h3></h3>` : ""}<div></div>` : "";
    if (n?.title) this.card.querySelector("h3")?.replaceChildren(n.title);
    if (n?.text) this.card.querySelector("div")?.replaceChildren(n.text);
    this.mirrorActions = actionsKey(s.actions ?? []);
    this.placePickers(s.controls ?? []);
    this.chromeGen++;
    // The whole tree follows in idle time (`describeSoon`). Until the first one is there — or with
    // no tree to come — the mirror is built now from what a click acts on; after that it keeps the
    // last tree until the next replaces it, rather than dropping every mark for a moment.
    if (s.semantics || this.describedGen < 0 || !this.view.semantics) this.fillMirror(s, s.semantics ?? s.actions ?? []);
    else {
      // The buttons say the state now (a label that follows it: "…, in the coalition"), before
      // the rest of the tree catches up.
      for (const a of s.actions ?? []) {
        // (A brush's or a view's stop says what it holds in its name.)
        const d = a.kind === "brush" || a.kind === "pan" ? [...this.mirror.querySelectorAll<HTMLElement>("[role=application][data-path]")].find((x) => x.dataset.path === a.path) : null;
        if (d) { if (d.getAttribute("aria-label") !== a.label) d.setAttribute("aria-label", a.label); continue; }
        const b = [...this.mirror.querySelectorAll<HTMLButtonElement>("button[data-path]")].find((x) => x.dataset.path === a.path);
        if (b && b.textContent !== a.label) b.textContent = a.label;
        if (b && a.kind) b.setAttribute("aria-checked", String(!!a.checked));
      }
    }
    if (s.semantics) this.describedGen = this.chromeGen;
    // A story's ends: there's no step before the first or after the last, and a screen reader says
    // so ("dimmed") — the buttons keep the keyboard (aria-disabled, not disabled).
    const index = s.index ?? 0, count = s.states?.length ?? 0;
    this.root.querySelector('[data-ev="prev"]')?.setAttribute("aria-disabled", String(index <= 0));
    this.root.querySelector('[data-ev="next"]')?.setAttribute("aria-disabled", String(index >= count - 1));
    this.announce(s);
    this.growForText(s);
  }

  /** What the chart shows, said first (`Engine::accessible_summary`), as last described. */
  private summary = "";
  /** The step the live region last announced. */
  private announced = -1;
  /** The words the element says around the chart's own, in the document's language (the
   * status's `phrases`; English until it's there). */
  private phrases: Record<string, string> = {};

  private ph(key: string, args: Record<string, string | number> = {}): string {
    const fallback: Record<string, string> = { step: "Step {i} of {n}", note: "Note: {text}", legend: "Legend", data: "Data", controls: "Controls", chart: "Chart", more: "{n} more",
      "regions-missing.one": "{n} region without data", "regions-missing.other": "{n} regions without data", "points-missing.one": "{n} data point without data", "points-missing.other": "{n} data points without data" };
    let out = this.phrases[key] ?? fallback[key] ?? key;
    for (const [k, v] of Object.entries(args)) out = out.split(`{${k}}`).join(String(v));
    return out;
  }

  /** A new step, said once by the live region: its narration (title and text), else where the
   * reader is in the story ("Step 2 of 4"). The first state isn't announced: the chart's name and
   * description say what it is when the reader gets to it. */
  private announce(s: Record<string, any>) {
    const index = s.index ?? 0;
    if (index === this.announced) return;
    const first = this.announced < 0;
    this.announced = index;
    if (first) return;
    const n = s.narration;
    const said = [n?.title, n?.text].filter((x) => x && String(x).trim()).join(". ");
    const count = s.states?.length ?? 0;
    this.live.textContent = said || (count > 1 ? this.ph("step", { i: index + 1, n: count }) : "");
  }

  /** The whole semantics tree into the mirror, in idle time and only while no transition runs:
   * the brief status that keeps every step cheap lists only what a click acts on, and a screen
   * reader needs every labelled mark — a bar's party and value, the title, the axes. */
  private describeSoon() {
    if (this.describedGen === this.chromeGen || this.describeIdle || !this.view?.semantics) return;
    // Only data changed (a live feed): at most every few seconds — a reader browsing the table
    // gets today's rows, and the page isn't rebuilding it every beat.
    const since = performance.now() - this.describedAt;
    if (this.dataStale && since < MIRROR_DATA_EVERY) {
      if (!this.describeTimer) this.describeTimer = window.setTimeout(() => { this.describeTimer = 0; this.describeSoon(); }, MIRROR_DATA_EVERY - since);
      return;
    }
    const ric: (f: () => void, o?: { timeout: number }) => number = (globalThis as any).requestIdleCallback ?? ((f) => window.setTimeout(f, 60));
    this.describeIdle = ric(() => {
      this.describeIdle = 0;
      const view = this.view;
      if (!view || this.describedGen === this.chromeGen) return;
      // The frame loop asks again when it settles — but a feed may never let it settle (a beat
      // shorter than a move): stale data is described mid-motion, every few seconds.
      if (view.transitioning?.() && !this.dataStale) return;
      this.describedGen = this.chromeGen;
      this.dataStale = false;
      this.describedAt = performance.now();
      const brief = view.chrome?.() ?? view.status();
      if (view.summary) brief.summary = view.summary();
      this.fillMirror(brief, view.semantics());
    }, { timeout: 1000 });
  }

  /** Most marks the mirror lists: past that a list helps no one (the chart's text alternative has
   * the data), and building it would cost the page. */
  private static MIRROR_MAX = 500;

  /** What screen readers and keyboards get (the canvas is a picture to them): engine-drawn
   * controls as native ones; then the chart's content as the semantics tree says it — groups as
   * named groups, an axis or a legend as one line ("y axis: 0 to 1,600"), annotations as text,
   * and each run of data marks as a table (name, value), its rows reachable with the arrow keys
   * and outlined on the chart as the keyboard moves; clickable marks are buttons. The chart's
   * name goes on the element itself (`aria-label`), its summary first inside it. */
  /** The first data mark the mirror lists (its page), and what it was last built from. */
  private mirrorFrom = 0;
  private lastMirror: [Record<string, any>, MirrorItem[]] | null = null;

  private fillMirror(s: Record<string, any>, items: MirrorItem[]) {
    this.lastMirror = [s, items];
    // Built aside, then merged into the mirror on the page (`morph`): the elements that stay what
    // they were (a switch whose state flipped, a slider whose value moved) are the same elements,
    // so a screen reader on one stays there. (Rebuilt from scratch, VoiceOver in Safari lost its
    // place after every toggle — its element was gone — and landed on the figure.)
    const live = this.mirror;
    this.mirror = document.createElement("div");
    try {
      this.buildMirror(s, items);
    } finally {
      const fresh = this.mirror;
      this.mirror = live;
      DatarsView.morph(live, fresh);
    }
  }

  /** What identifies a mirror element across rebuilds: its kind, role and what it stands for. */
  private static mirrorKey(e: Element): string {
    const d = (e as HTMLElement).dataset ?? {};
    return [e.tagName, e.getAttribute("role") ?? "", d.path ?? "", d.signal ?? "", d.ev ?? "", d.options ?? ""].join("\u0000");
  }

  /** Make `live`'s children what `fresh`'s are, keeping the elements that stand for the same thing
   * (their attributes and text updated) and moving in the rest. */
  private static morph(live: Element, fresh: Element) {
    // Each fresh element takes the first live one that stands for the same thing (in order),
    // updated; the rest are new. Then the live list is put in the fresh order, moving only what
    // is out of place (a moved element would lose the keyboard).
    const pool = new Map<string, Element[]>();
    for (const l of live.children) {
      const k = DatarsView.mirrorKey(l);
      pool.set(k, [...(pool.get(k) ?? []), l]);
    }
    const out: Node[] = [...fresh.childNodes].map((f) => {
      if (!(f instanceof Element)) return f;
      const l = pool.get(DatarsView.mirrorKey(f))?.shift();
      if (!l) return f;
      for (const a of [...l.attributes]) if (!f.hasAttribute(a.name)) l.removeAttribute(a.name);
      for (const a of [...f.attributes]) if (l.getAttribute(a.name) !== a.value) l.setAttribute(a.name, a.value);
      // (A slider's value, but not under the reader's own hand.)
      if (l instanceof HTMLInputElement && f instanceof HTMLInputElement && l.value !== f.value && (l.getRootNode() as ShadowRoot).activeElement !== l) l.value = f.value;
      if (l instanceof HTMLElement && f instanceof HTMLElement && l.tabIndex !== f.tabIndex) l.tabIndex = f.tabIndex;
      DatarsView.morph(l, f);
      return l;
    });
    out.forEach((n, i) => {
      if (live.childNodes[i] !== n) live.insertBefore(n, live.childNodes[i] ?? null);
    });
    while (live.childNodes.length > out.length) live.lastChild!.remove();
  }

  private buildMirror(s: Record<string, any>, items: MirrorItem[]) {
    // Where the keyboard is, read before the mirror is rebuilt: rebuilding mustn't lose it.
    const active = this.root.activeElement as HTMLElement | null;
    const focused = active?.dataset?.signal, focusedPath = active?.dataset?.path, focusedItem = active?.dataset?.item;
    // Engine-drawn controls as native ones: keyboards and screen readers can operate them.
    const controls = (s.controls ?? []) as any[];
    if (controls.length) {
      // A named group, not a list: VoiceOver in Safari read a list's marker ("•") as a stop of its
      // own before each control, and "end of list" after.
      const ul = document.createElement("div");
      ul.setAttribute("role", "group");
      ul.setAttribute("aria-label", this.ph("controls"));
      for (const c of controls) {
        const li = document.createElement("div");
        if (c.kind === "select") {
          // A select as a real one: its options, the chosen one, the same signal.
          const sel = this.nativeSelect(c);
          // Its name (the label without the chosen option, which the select says itself).
          const chosen = /:\s*(.+)$/.exec(c.label);
          sel.setAttribute("aria-label", chosen ? c.label.slice(0, c.label.lastIndexOf(":")).trim() : c.label);
          if (c.rect) sel.dataset.rect = c.rect.join(",");
          li.appendChild(sel);
          ul.appendChild(li);
          if (c.signal === focused) queueMicrotask(() => sel.focus());
          continue;
        }
        const input = document.createElement("input");
        Object.assign(input, { type: "range", min: String(c.min), max: String(c.max), step: c.step > 0 ? String(c.step) : "any", value: String(c.value) });
        // Its name, then what the drawn control says ("32,000") as its value — not the raw number,
        // and not the value twice (VoiceOver read "32,000 Monthly income (SEK): 32,000 slider").
        const said = /:\s*(.+)$/.exec(c.label)?.[1];
        input.setAttribute("aria-label", said ? c.label.slice(0, c.label.lastIndexOf(":")).trim() : c.label);
        if (said) input.setAttribute("aria-valuetext", said);
        input.dataset.signal = c.signal;
        if (c.rect) input.dataset.rect = c.rect.join(",");
        input.addEventListener("input", () => { this.view?.set_signal(c.signal, input.value); this.kick(); this.noteSignals(); });
        input.addEventListener("change", () => this.syncChrome());
        li.appendChild(input);
        ul.appendChild(li);
        if (c.signal === focused) queueMicrotask(() => input.focus()); // rebuilding mustn't steal the keyboard's place
      }
      this.mirror.appendChild(ul);
    }
    // A select is its native select above, not its drawn box; a dragged control (a slider's
    // thumb) is its range input.
    const selectLabels = new Set(controls.filter((c) => c.kind === "select").map((c) => c.label));
    const shown = items.filter((i) => !(i.role === "control" && (!i.actionable || selectLabels.has(i.label))));
    // The chart's name: the document's title, else its root's label, else what the page said.
    const rootLabel = shown[0]?.depth === 0 && !shown[0].actionable && shown[0].role !== "control" ? shown[0].label : "";
    const name = (s.name as string | undefined)?.trim() || rootLabel || shown.find((i) => i.role === "title" && i.label.trim())?.label || "";
    if (name) this.setAttribute("aria-label", name);
    else if (!this.getAttribute("aria-label")) this.setAttribute("aria-label", this.ph("chart"));
    // The summary — what the chart shows — as the first thing in the chart's content, read in
    // order by every screen reader. (Not `aria-description`: VoiceOver with Chrome files that
    // under "more content", unread, and readers that do say it would say it twice.)
    if (typeof s.summary === "string") this.summary = s.summary;
    // Then the step's narration (the card over the chart is for the eye; this is its text in the
    // reading order: after what the chart shows, before its content and the story's buttons).
    const n = s.narration;
    if (n && (n.title || n.text) && !s.narrationDrawn) {
      const at = this.mirror.firstChild;
      if (n.title) {
        const h = document.createElement("h3");
        h.className = "narration";
        h.textContent = n.title;
        this.mirror.insertBefore(h, at);
      }
      if (n.text) {
        const p = document.createElement("p");
        p.className = "narration";
        p.textContent = n.text;
        this.mirror.insertBefore(p, at);
      }
    }
    if (this.summary) {
      const p = document.createElement("p");
      p.className = "summary";
      p.textContent = this.summary;
      this.mirror.prepend(p);
    }
    // The flat list (pre-order, with depths) as a tree.
    type N = { item: MirrorItem; kids: N[] };
    const top: N[] = [];
    const stack: N[] = [];
    for (const item of shown) {
      const node = { item, kids: [] };
      while (stack.length && stack[stack.length - 1].item.depth >= item.depth) stack.pop();
      (stack.length ? stack[stack.length - 1].kids : top).push(node);
      stack.push(node);
    }
    // Marks past the mirror's page (`MIRROR_MAX`, from `mirrorFrom`) are a button away, not gone.
    let seen = 0, left = 0, before = 0;
    const isData = (i: MirrorItem) => i.role === "datum" || i.role === "region" || (i.actionable && i.role !== "control");
    const descend = (n: N, role: string): string[] => n.kids.flatMap((k) => (k.item.role === role ? [k.item.label] : descend(k, role))).filter((l) => l && l.trim());
    /** Labels as one line: all of a few, the ends of many. */
    const line = (labels: string[]) => (labels.length <= 12 ? labels.join(", ") : `${labels[0]} to ${labels[labels.length - 1]}`);
    const render = (nodes: N[], into: HTMLElement, caption: string, groupLabel: string) => {
      let run: MirrorItem[] = [];
      const flush = () => {
        if (!run.length) return;
        into.appendChild(this.dataTable(run, caption, focusedPath, focusedItem));
        run = [];
      };
      // Marks without data (a map's regions with no value): one line, after the rest.
      const missing = nodes.filter((n) => (n.item.role === "datum" || n.item.role === "region") && n.item.missing);
      for (const n of nodes) {
        const { role, label } = n.item;
        if (missing.includes(n)) continue;
        if (isData(n.item)) {
          const k = n.item.actionable ? -1 : seen++;
          if (k >= 0 && k < this.mirrorFrom) { before++; continue; }
          if (k >= this.mirrorFrom + DatarsView.MIRROR_MAX) { left++; continue; }
          run.push(n.item);
          continue;
        }
        flush();
        if (role === "tick" || role === "legend-item") continue; // said with their axis or legend
        if (role === "axis" || role === "legend") {
          const p = document.createElement("p");
          const parts = descend(n, role === "axis" ? "tick" : "legend-item");
          const title = label || (role === "legend" ? this.ph("legend") : "");
          p.textContent = parts.length ? `${title}${title ? ": " : ""}${line(parts)}` : title;
          if (p.textContent) into.appendChild(p);
          continue;
        }
        // A title that repeats its chart's name or its group's says nothing new.
        if (role === "title" && (label === name || label === groupLabel)) { render(n.kids, into, caption, groupLabel); continue; }
        // The root (it's the chart: named on the element), and groups without a name: their
        // content belongs to what's around them.
        if ((n.item.depth === 0 && label === name) || ((role === "group" || role === "series") && !label.trim())) {
          render(n.kids, into, caption, groupLabel);
          continue;
        }
        // A series that is only its marks: their table, named by it (not a group around it too).
        if (n.kids.length && role === "series" && n.kids.every((k) => isData(k.item) && !k.kids.length)) {
          render(n.kids, into, label, groupLabel);
          continue;
        }
        if (n.kids.length && (role === "group" || role === "series")) {
          const g = document.createElement("div");
          g.setAttribute("role", "group");
          g.setAttribute("aria-label", label);
          into.appendChild(g);
          render(n.kids, g, label, label);
          continue;
        }
        if (n.item.actionable && (n.item.kind === "brush" || n.item.kind === "pan")) {
          const d = this.mirrorDrag(n.item);
          into.appendChild(d);
          if (n.item.path === focusedPath) queueMicrotask(() => d.focus());
          render(n.kids, into, caption, groupLabel);
          continue;
        }
        if (n.item.actionable) {
          // A control the engine draws (a toggle, a button): a native button.
          const b = this.mirrorButton(n.item);
          into.appendChild(b);
          if (n.item.path === focusedPath) queueMicrotask(() => b.focus());
          render(n.kids, into, caption, groupLabel);
          continue;
        }
        if (label.trim() && !(role === "series" && n.kids.length === 0 && label === caption)) {
          const p = document.createElement("p");
          p.textContent = role === "annotation" ? this.ph("note", { text: label }) : label;
          into.appendChild(p);
        }
        render(n.kids, into, label || caption, groupLabel);
      }
      flush();
      if (missing.length) {
        const p = document.createElement("p");
        const what = missing.every((n) => n.item.role === "region") ? "regions-missing" : "points-missing";
        p.textContent = this.ph(`${what}.${missing.length === 1 ? "one" : "other"}`, { n: missing.length });
        into.appendChild(p);
      }
    };
    render(top, this.mirror, "", name);
    // The pages of a mirror with more marks than it lists at once.
    const page = (from: number, key: string) => {
      const b = document.createElement("button");
      const to = Math.min(from + DatarsView.MIRROR_MAX, seen);
      const n = (v: number) => v.toLocaleString(this.mirror.lang || undefined);
      b.textContent = this.ph(key, { from: n(from + 1), to: n(to), total: n(seen) });
      b.dataset.ev = `rows:${from}`;
      b.addEventListener("click", () => {
        this.mirrorFrom = from;
        if (this.lastMirror) this.fillMirror(this.lastMirror[0], this.lastMirror[1]);
        this.live.textContent = this.ph("rows-shown", { from: n(from + 1), to: n(to), total: n(seen) }) + ".";
        queueMicrotask(() => this.mirror.querySelector<HTMLElement>("[data-nav]")?.focus());
      });
      this.mirror.appendChild(b);
    };
    if (left) page(this.mirrorFrom + DatarsView.MIRROR_MAX, "rows-next");
    if (before) page(Math.max(0, this.mirrorFrom - DatarsView.MIRROR_MAX), "rows-previous");
    // One stop for the keyboard among the data: where it was, else the first; arrows move on.
    const stops = [...this.mirror.querySelectorAll<HTMLElement>("[data-nav]")];
    const current = stops.find((x) => x.dataset.item === focusedItem && focusedItem) ?? stops[0];
    for (const x of stops) x.tabIndex = x === current ? 0 : -1;
    if (current && focusedItem && current.dataset.item === focusedItem) queueMicrotask(() => current.focus());
  }

  /** A native button for a mark or control a click acts on — a checkbox, a switch or a radio
   * button (`role` and `aria-checked`) when the control says it's one, so its state is announced
   * by the reader's own words for it. */
  private mirrorButton(item: MirrorItem): HTMLButtonElement {
    const b = document.createElement("button");
    b.textContent = item.label;
    if (item.kind) {
      b.setAttribute("role", item.kind);
      b.setAttribute("aria-checked", String(!!item.checked));
    }
    b.dataset.path = item.path;
    if (item.rect) b.dataset.rect = item.rect.join(",");
    b.addEventListener("click", () => { if (this.view?.activate(item.path)) { this.kick(); this.syncChrome(); this.noteSignals(); this.sayChange(); } });
    return b;
  }

  /** What a pointer drags — a brushable area, a pannable view — for the keyboard: one stop that
   * takes the keys the engine knows for it (`key`: Enter, the arrows, Escape; +, −, 0), named by
   * the part it's in and what it holds, its keys described. Screen readers pass the keys through
   * (`role="application"`), as they do for any widget with keys of its own. */
  private mirrorDrag(item: MirrorItem): HTMLElement {
    const d = document.createElement("div");
    d.setAttribute("role", "application");
    d.tabIndex = 0;
    d.setAttribute("aria-label", item.label);
    const hint = document.createElement("span");
    hint.id = `hint-${item.kind}-${Math.abs([...item.path].reduce((h, c) => (h * 31 + c.charCodeAt(0)) | 0, 7))}`;
    hint.textContent = this.ph(`${item.kind}-hint`);
    d.setAttribute("aria-describedby", hint.id);
    d.appendChild(hint);
    d.dataset.path = item.path;
    if (item.rect) d.dataset.rect = item.rect.join(",");
    d.addEventListener("keydown", (e) => {
      if (!this.view?.key || e.metaKey || e.ctrlKey || e.altKey) return;
      const key = (e.shiftKey && e.key.startsWith("Arrow") ? "Shift+" : "") + e.key;
      if (!this.view.key(item.path, key)) return;
      e.preventDefault();
      e.stopPropagation(); // not also a step of the story (Escape, the arrows)
      this.now();
      this.kick();
      this.syncChrome();
      this.noteSignals();
      this.sayChange();
    });
    return d;
  }

  /** What the reader's last click, activation or key changed, said by the live region — politely,
   * once (`take_announcement`): "Pears, hidden. Total: 69 kg. Now 2 data points…". The same words
   * twice in a row are said twice. */
  private sayChange() {
    const said: string | undefined = this.view?.take_announcement?.();
    if (!said) return;
    if (this.live.textContent === said) {
      this.live.textContent = "";
      requestAnimationFrame(() => { this.live.textContent = said; });
    } else this.live.textContent = said;
  }

  /** A run of data marks as a table: a row per mark, its name and — when its label says one
   * ("Housing: 10,240") — its value in a column of its own, so a screen reader's table commands
   * read down a column. Rows are keyboard stops (one tab stop; arrows move between them). */
  private dataTable(marks: MirrorItem[], caption: string, focusedPath?: string, focusedItem?: string): HTMLTableElement {
    // Marks that say the same in a row (a parliament's seats, each "V") are one row and a count,
    // outlined together on the chart.
    const run: MirrorItem[] = [];
    const counts: number[] = [];
    for (const m of marks) {
      const last = run[run.length - 1];
      if (last && !m.actionable && !last.actionable && last.label === m.label) {
        counts[counts.length - 1]++;
        if (last.rect && m.rect) {
          const [x, y, w, h] = last.rect, [a, b, c, d] = m.rect;
          const x0 = Math.min(x, a), y0 = Math.min(y, b);
          run[run.length - 1] = { ...last, rect: [x0, y0, Math.max(x + w, a + c) - x0, Math.max(y + h, b + d) - y0] };
        }
        continue;
      }
      run.push(m);
      counts.push(1);
    }
    const table = document.createElement("table");
    // A long run listed a page at a time: the rows numbered of the whole ("row 201 of 365").
    const total = run.find((i) => i.total !== undefined)?.total;
    if (total !== undefined) table.setAttribute("aria-rowcount", String(total));
    const cap = document.createElement("caption");
    cap.textContent = caption || this.ph("data");
    table.appendChild(cap);
    const split = (l: string) => {
      const i = l.lastIndexOf(": ");
      return i > 0 ? [l.slice(0, i), l.slice(i + 2)] : [l, ""];
    };
    const pairs = run.map((i) => split(i.label));
    // (Buttons keep their whole label: it's what pressing one acts on.)
    const twoColumns = (pairs.some((p) => p[1]) || counts.some((c) => c > 1)) && !run.some((i) => i.actionable);
    const body = document.createElement("tbody");
    run.forEach((item, k) => {
      const tr = document.createElement("tr");
      if (item.index !== undefined) tr.setAttribute("aria-rowindex", String(item.index + 1));
      const th = document.createElement("th");
      th.scope = "row";
      const [label, value] = counts[k] > 1 ? [item.label, `× ${counts[k]}`] : twoColumns ? pairs[k] : [item.label, ""];
      let stop: HTMLElement = th;
      if (item.actionable) {
        const b = this.mirrorButton(item);
        b.textContent = item.label;
        th.appendChild(b);
        stop = b;
      } else {
        th.textContent = label;
      }
      stop.dataset.nav = "";
      stop.dataset.item = `${caption}\u0000${item.label}\u0000${item.path}`;
      stop.dataset.label = item.label;
      if (item.rect) stop.dataset.rect = item.rect.join(",");
      tr.appendChild(th);
      if (twoColumns) {
        const td = document.createElement("td");
        td.textContent = item.actionable ? "" : value;
        tr.appendChild(td);
      }
      body.appendChild(tr);
      if (item.actionable && item.path === focusedPath && !focusedItem) queueMicrotask(() => stop.focus());
    });
    table.appendChild(body);
    return table;
  }

  /** A `<select>` for an engine-drawn one: the options it offers, the chosen one selected, a
   * choice setting its signal. */
  private nativeSelect(c: { signal: string; current: unknown; options: { value: unknown; label: string }[] }): HTMLSelectElement {
    const sel = document.createElement("select");
    sel.dataset.signal = c.signal;
    // (Its options are part of what it is: a select with others is another select.)
    sel.dataset.options = JSON.stringify(c.options.map((o) => o.value));
    c.options.forEach((o, i) => {
      const opt = document.createElement("option");
      opt.value = String(i);
      opt.textContent = o.label;
      if (JSON.stringify(o.value) === JSON.stringify(c.current)) opt.selected = true;
      sel.appendChild(opt);
    });
    sel.addEventListener("change", () => {
      const o = c.options[Number(sel.value)];
      if (!o) return;
      this.now();
      this.view?.set_signal(c.signal, JSON.stringify(o.value));
      this.kick();
      this.syncChrome();
      this.noteSignals();
    });
    return sel;
  }

  private pickerKey = "";
  /** On touch screens, the platform's own picker for each engine-drawn select: a native `<select>`
   * laid invisibly over the box, so a tap opens the phone's list (a wheel, a menu, a dialog)
   * instead of the chart's. With a mouse the chart's own list stays. */
  private placePickers(controls: { kind?: string; signal: string; label: string; current: unknown; rect?: number[]; options?: { value: unknown; label: string }[] }[]) {
    const layer = this.root.querySelector<HTMLElement>(".pickers");
    if (!layer) return;
    const touch = matchMedia("(pointer: coarse)").matches;
    const selects = touch ? controls.filter((c) => c.kind === "select" && c.rect && c.options?.length) : [];
    const key = JSON.stringify(selects.map((c) => [c.signal, c.current, c.rect?.map(Math.round), c.options?.length]));
    if (key === this.pickerKey) return;
    this.pickerKey = key;
    layer.replaceChildren(...selects.map((c) => {
      const sel = this.nativeSelect(c as { signal: string; current: unknown; options: { value: unknown; label: string }[] });
      // The accessibility mirror has the same select: this one is for the finger only.
      sel.setAttribute("aria-hidden", "true");
      sel.tabIndex = -1;
      const [x, y, w, h] = c.rect as number[];
      Object.assign(sel.style, { left: `${x}px`, top: `${y}px`, width: `${w}px`, height: `${h}px` });
      return sel;
    }));
  }

  private cardSpot = "";
  private linkLayer!: HTMLElement;
  private linkKey = "";

  /** Real links over the elements that link somewhere (a map's OpenStreetMap credit): clickable,
   * focusable, announced — the canvas can only draw the text. */
  private placeLinks() {
    const links = (this.view?.links?.() ?? []) as { href: string; label: string; bounds: [number, number, number, number] }[];
    const key = JSON.stringify(links.map((l) => [l.href, l.bounds.map(Math.round)]));
    if (key === this.linkKey) return;
    this.linkKey = key;
    this.linkLayer.replaceChildren(...links.filter((l) => /^https?:/.test(l.href)).map((l) => {
      const a = document.createElement("a");
      Object.assign(a, { href: l.href, target: "_blank", rel: "noopener" });
      a.setAttribute("aria-label", l.label || l.href);
      const [x, y, w, h] = l.bounds;
      Object.assign(a.style, { left: `${x - 2}px`, top: `${y - 2}px`, width: `${w + 4}px`, height: `${h + 4}px` });
      return a;
    }));
  }

  private placeCard(moving = false) {
    // Every frame: only the narration, never `status()` (the whole semantics tree — for big data,
    // thousands of marks, re-cut for the current camera).
    const n = this.view?.narration() as { title?: string; text?: string; anchor?: string } | null;
    if (!n?.anchor) {
      // Placed once per state and size, on the settled chart (not every frame of a transition).
      const key = `${this.view?.index()}|${this.size.w}x${this.size.h}|${n?.title ?? ""}|${n?.text ?? ""}`;
      if (moving || key === this.cardSpot) return;
      this.cardSpot = key;
      // The corner where the caption covers the least: data marks and text found by sampling
      // what's drawn under a grid over each candidate spot (the title is usually top-left).
      const w = this.card.offsetWidth || 280, h = this.card.offsetHeight || 60, m = 12;
      const spots: [number, number][] = [[this.size.w - w - m, m], [m, this.size.h - h - m], [this.size.w - w - m, this.size.h - h - m], [m, m]];
      // All the spots' points in one call: each hit test over a map of thousands of counties
      // walked the whole frame.
      const pts: number[] = [];
      for (const [x, y] of spots) for (let i = 0; i <= 4; i++) for (let j = 0; j <= 2; j++) pts.push(x + (w * i) / 4, y + (h * j) / 2);
      const occupied: ArrayLike<number> = this.view.occupied
        ? this.view.occupied(new Float64Array(pts))
        : Array.from({ length: pts.length / 2 }, (_, k) => (this.view.hit_test(pts[2 * k], pts[2 * k + 1]) as { role?: string; kind: string }[]).some((t) => t.kind === "text" || t.role === "datum" || t.role === "region") ? 1 : 0);
      let best = spots[0], least = Infinity;
      spots.forEach(([x, y], s) => {
        let covered = 0;
        for (let k = 0; k < 15; k++) covered += occupied[s * 15 + k];
        if (covered < least) { least = covered; best = [x, y]; }
      });
      this.card.style.left = `${Math.max(0, best[0])}px`; this.card.style.top = `${Math.max(0, best[1])}px`;
      return;
    }
    const a = (this.view.anchors() as { path: string; x: number; y: number }[]).find((x) => x.path.includes(`"${n.anchor}"`));
    if (a) { this.card.style.left = `${Math.min(a.x + 12, this.size.w - 290)}px`; this.card.style.top = `${Math.max(a.y - 40, 8)}px`; }
  }
}

if (typeof customElements !== "undefined" && !customElements.get("datars-view")) customElements.define("datars-view", DatarsView);
