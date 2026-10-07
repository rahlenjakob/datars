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

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Wasm = any;

const engines = new Map<string, Promise<Wasm>>();
/** How long a chart's first frame waits for the faces the page's tokens name (see `themeFaces`). */
const THEME_FACE_WAIT = 1500;
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
.controls { display: flex; gap: 8px; justify-content: center; margin-top: 8px; }
.controls[hidden] { display: none; }
.controls button { font: inherit; padding: 4px 12px; border-radius: 6px; border: 1px solid var(--rule, #999); background: var(--paper, #fff); color: var(--ink, #111); cursor: pointer; }
.perf { position: absolute; left: 6px; top: 6px; z-index: 2; pointer-events: none; max-width: calc(100% - 12px); padding: 6px 8px; border-radius: 6px; background: rgba(0,0,0,.8); color: #fff; font: 11px/1.35 ui-monospace, Menlo, monospace; }
.perf[hidden] { display: none; }
.perf pre { margin: 0; font: inherit; white-space: pre-wrap; }
.perf canvas { display: block; width: 240px; max-width: 100%; height: 36px; margin-top: 6px; }
.perf button { float: right; margin: -4px -4px 0 8px; padding: 0 4px; pointer-events: auto; background: none; border: 0; color: #fff; font: 14px/1 system-ui, sans-serif; cursor: pointer; }
.sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); white-space: nowrap; }
${TEXT_CSS}`;

/** `HTMLElement`, or (on a server, where there is none) a stand-in so the module still loads. */
const ElementBase: typeof HTMLElement = typeof HTMLElement === "undefined" ? (class {} as typeof HTMLElement) : HTMLElement;

export class DatarsView extends ElementBase {
  static observedAttributes = ["src", "doc", "state", "mode", "height", "no-controls", "reduced-motion"];
  /** Runtime files the page supplies (see `runtimeAssets`). */
  static assets = runtimeAssets;
  private root: ShadowRoot;
  private stage!: HTMLDivElement;
  private canvas!: HTMLCanvasElement;
  private poster!: HTMLDivElement;
  private card!: HTMLDivElement;
  private tip!: HTMLDivElement;
  private mirror!: HTMLUListElement;
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
      <div class="stage" part="stage"><div class="poster" part="poster"></div><canvas hidden part="canvas"></canvas>
      <div class="texts" part="texts" aria-hidden="true"></div><div class="card" part="card"></div><div class="links" part="links"></div><div class="pickers"></div><div class="tip" hidden part="tooltip"></div><div class="focus" hidden part="focus"></div><div class="perf" hidden part="perf"></div></div>
      <div class="controls" part="controls" hidden><button data-ev="prev" aria-label="Previous">←</button><button data-ev="next" aria-label="Next">→</button></div>
      <ul class="sr" role="list" aria-label="Chart content"></ul><div class="sr" aria-live="polite"></div>`;
    this.stage = this.root.querySelector(".stage")!;
    // The height the page asked for, from the first frame the element is on the page: laid out a
    // frame later, the chart would grow into its space and push the page down.
    this.reserveHeight();
    this.canvas = this.root.querySelector("canvas")!;
    this.poster = this.root.querySelector(".poster")!;
    this.card = this.root.querySelector(".card")!;
    this.linkLayer = this.root.querySelector(".links")!;
    this.tip = this.root.querySelector(".tip")!;
    this.mirror = this.root.querySelector("ul")!;
    this.live = this.root.querySelector("[aria-live]")!;
    // A sighted keyboard user sees where the hidden list's focus is: the item's box on the chart.
    const ring = this.root.querySelector<HTMLDivElement>(".focus")!;
    this.mirror.addEventListener("focusin", (e) => {
      const r = (e.target as HTMLElement).dataset?.rect?.split(",").map(Number);
      if (!r || r.length !== 4 || !(r[2] > 0 && r[3] > 0)) return void (ring.hidden = true);
      Object.assign(ring.style, { left: `${r[0]}px`, top: `${r[1]}px`, width: `${r[2]}px`, height: `${r[3]}px` });
      ring.hidden = false;
    });
    this.mirror.addEventListener("focusout", (e) => {
      // Moving to another of the list's items: its own focusin places the ring.
      if (!this.mirror.contains(e.relatedTarget as Node | null)) ring.hidden = true;
    });
    this.tabIndex = 0;
    this.setAttribute("role", "figure");
    this.root.querySelectorAll("button").forEach((b) => b.addEventListener("click", () => this.send((b as HTMLElement).dataset.ev!)));
    if (!this.wired) {
      // The host element's own listeners, once: it may be taken off the page and put back.
      this.wired = true;
      this.addEventListener("keydown", (e) => {
        if (e.key === "ArrowRight" || e.key === " ") { this.send("next"); e.preventDefault(); }
        if (e.key === "ArrowLeft") { this.send("prev"); e.preventDefault(); }
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
    }
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
      const actions = this.view.chrome?.().actions;
      if (actions && actionsKey(actions) !== this.mirrorActions) this.syncChrome();
    }, ms);
  }

  /** Taken off the page, the chart lets go of its engine at once — its scene, tables, GPU device
   * and canvas — rather than when the garbage collector gets round to it: every view on a page
   * shares one wasm memory, so a long article that unmounts charts far off screen stays small.
   * Put back, it starts again (`connectedCallback`). */
  disconnectedCallback() {
    this.generation++; // a start still loading gives up
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
    this.view?.set_tokens(JSON.stringify(tokens));
    this.kick();
    this.syncChrome();
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

  private async start() {
    const gen = ++this.generation;
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
        // The poster downloads alongside the engine (not after it).
        void this.fetchPoster(m, this.base.replace(/[?#].*$/, "").replace(/\/c\/[^/]+$/, "").replace(/\/manifest\.json$/, ""), live);
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
    if (this.hostTokens) {
      try { view.set_tokens?.(JSON.stringify(this.hostTokens)); } catch (e) { console.warn("datars: tokens", e); }
      await this.themeFaces(view, live);
      if (!live()) return abandon();
    }
    for (const [name, value] of this.hostSignals) {
      try { view.set_signal?.(name, JSON.stringify(value)); } catch (e) { console.warn("datars: signal", name, e); }
    }
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
    if (this.hostTokens && Object.keys(this.hostTokens).length) return;
    const want = this.getAttribute("mode") ?? (matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light");
    const drawn = /data-mode="(\w+)"/.exec(svg.slice(0, 300))?.[1];
    if (drawn && drawn !== want) return;
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
    const m = this.getAttribute("mode") ?? (matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light");
    this.view?.set_mode(m);
    this.syncChrome();
    this.kick();
  }

  private layout() {
    const w = this.clientWidth || 800;
    const aspect = this.view ? 0 : 0.6;
    const h = Number(this.getAttribute("height")) || Math.round(w * (aspect || 0.6));
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
    if (spread > 0 && this.view.wheel(mid[0], mid[1], -Math.log(spread / this.pinch) / 0.0015)) {
      this.tip.hidden = true;
      this.kick();
      this.noteSignals();
    }
    this.pinch = spread || this.pinch;
    return true;
  }

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
        .then((b) => { if (this.view !== view) return; view.provide_source(r.name, b); this.kick(); })
        .catch((e) => console.warn("datars: data", r.name, e))
        .finally(() => this.fetching.delete(r.name));
    }
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
    if (s.semantics) this.describedGen = this.chromeGen;
    this.live.textContent = n?.text ?? "";
  }

  /** The whole semantics tree into the mirror, in idle time and only while no transition runs:
   * the brief status that keeps every step cheap lists only what a click acts on, and a screen
   * reader needs every labelled mark — a bar's party and value, the title, the axes. */
  private describeSoon() {
    if (this.describedGen === this.chromeGen || this.describeIdle || !this.view?.semantics) return;
    const ric: (f: () => void, o?: { timeout: number }) => number = (globalThis as any).requestIdleCallback ?? ((f) => window.setTimeout(f, 60));
    this.describeIdle = ric(() => {
      this.describeIdle = 0;
      const view = this.view;
      if (!view || this.describedGen === this.chromeGen) return;
      if (view.transitioning?.()) return; // the frame loop asks again when it settles
      this.describedGen = this.chromeGen;
      this.fillMirror(view.chrome?.() ?? view.status(), view.semantics());
    }, { timeout: 1000 });
  }

  /** Most marks the mirror lists: past that a list helps no one (the chart's text alternative has
   * the data), and building it would cost the page. */
  private static MIRROR_MAX = 500;

  /** The hidden list screen readers and keyboards use: engine-drawn controls as native ones,
   * clickable marks as buttons, and every other labelled item (`items`: the semantics tree, or
   * only what a click acts on) as text, indented by depth. */
  private fillMirror(s: Record<string, any>, items: { role: string; label: string; depth: number; path: string; actionable: boolean; rect?: number[] }[]) {
    // Where the keyboard is, read before the mirror is rebuilt: rebuilding mustn't lose it.
    const focused = (this.root.activeElement as HTMLInputElement | null)?.dataset?.signal;
    const focusedPath = (this.root.activeElement as HTMLElement | null)?.dataset?.path;
    this.mirror.innerHTML = "";
    // Engine-drawn controls as native ones: keyboards and screen readers can operate them.
    for (const c of s.controls ?? []) {
      const li = document.createElement("li");
      if (c.kind === "select") {
        // A select as a real one: its options, the chosen one, the same signal.
        const sel = this.nativeSelect(c);
        sel.setAttribute("aria-label", c.label);
        if (c.rect) sel.dataset.rect = c.rect.join(",");
        li.appendChild(sel);
        this.mirror.appendChild(li);
        if (c.signal === focused) sel.focus();
        continue;
      }
      const input = document.createElement("input");
      Object.assign(input, { type: "range", min: String(c.min), max: String(c.max), step: c.step > 0 ? String(c.step) : "any", value: String(c.value) });
      input.setAttribute("aria-label", c.label);
      input.dataset.signal = c.signal;
      if (c.rect) input.dataset.rect = c.rect.join(",");
      input.addEventListener("input", () => { this.view?.set_signal(c.signal, input.value); this.kick(); this.noteSignals(); });
      input.addEventListener("change", () => this.syncChrome());
      li.appendChild(input);
      this.mirror.appendChild(li);
      if (c.signal === focused) input.focus(); // rebuilding mustn't steal the keyboard's place
    }
    // A select is its native select above, not its drawn box; a dragged control (a slider's
    // thumb) is its range input.
    const selectLabels = new Set((s.controls ?? []).filter((c: { kind?: string }) => c.kind === "select").map((c: { label: string }) => c.label));
    let listed = 0, left = 0;
    for (const item of items) {
      if (item.role === "control" && (!item.actionable || selectLabels.has(item.label))) continue;
      // A group with no name of its own (a plot's frame) says nothing: its marks follow.
      if (!item.actionable && !item.label) continue;
      if (listed >= DatarsView.MIRROR_MAX && !item.actionable) { left++; continue; }
      listed++;
      const li = document.createElement("li");
      li.style.paddingLeft = `${item.depth}em`;
      if (item.actionable) {
        // Click interactions (select, filter, drill) as buttons: keyboards and screen readers too.
        const b = document.createElement("button");
        b.textContent = item.label;
        b.dataset.path = item.path;
        if (item.rect) b.dataset.rect = item.rect.join(",");
        b.addEventListener("click", () => { if (this.view?.activate(item.path)) { this.kick(); this.syncChrome(); this.noteSignals(); } });
        li.appendChild(b);
        this.mirror.appendChild(li);
        if (item.path === focusedPath) b.focus();
        continue;
      }
      li.textContent = `${item.role}: ${item.label}`;
      this.mirror.appendChild(li);
    }
    if (left) {
      const li = document.createElement("li");
      li.textContent = `${left} more`;
      this.mirror.appendChild(li);
    }
    // The chart's name: its root's label (the title), else what the page called it.
    const name = items[0]?.depth === 0 && !items[0].actionable ? items[0].label : "";
    if (name) this.setAttribute("aria-label", name);
    else if (!this.getAttribute("aria-label")) this.setAttribute("aria-label", "Chart");
  }

  /** A `<select>` for an engine-drawn one: the options it offers, the chosen one selected, a
   * choice setting its signal. */
  private nativeSelect(c: { signal: string; current: unknown; options: { value: unknown; label: string }[] }): HTMLSelectElement {
    const sel = document.createElement("select");
    sel.dataset.signal = c.signal;
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
