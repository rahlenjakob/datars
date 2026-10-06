// The frame profiler: what each frame of a transition cost, split into the engine (resolve, plan,
// interpolate, flatten) and the raster (GPU encode/submit, or CPU raster + blit), and the frame
// rate the page actually got. On with the `perf` attribute, `?datars-perf` in the page URL, or
// `localStorage["datars:perf"] = "1"`: a corner readout while things move, one console line per
// transition, a `perf` event on the element, and every summary in `window.__datarsPerf` (for
// scripted runs).

export interface PerfSummary {
  /** `from → to` state names (or `frame` for motion outside a transition: exploring, a clock). */
  label: string;
  renderer: string;
  frames: number;
  /** Wall time from the first moving frame to the last, ms. */
  ms: number;
  /** Frames per second over the run. */
  fps: number;
  engine: Spread;
  raster: Spread;
  /** Time between frames (what the reader sees): 16.7 ms at 60 Hz. */
  interval: Spread;
  /** Frames that took longer than a 60 Hz frame from one to the next. */
  long: number;
  /** 60 Hz frames missed in all (a 50 ms gap misses two). */
  dropped: number;
  /** The longest gap between two frames, ms, and the frame rate it amounts to: the stutter. */
  worst: number;
  minFps: number;
  /** From the input (a step, a click, a signal) to the first frame on screen, ms: the stall
   * before anything moves. `input` is the part spent handling the input itself (the new state
   * resolved and the transition planned), `first` the first frame's engine + raster. */
  stall: number | null;
  input: number | null;
  first: number;
  /** The most display-list ops a frame drew. */
  ops: number;
  /** Instances rebuilt per frame (built and uploaded again, not kept from an earlier frame): work
   * a desktop hides in a millisecond and a phone misses frames over. */
  rebuilt: Spread;
  /** Between frames: data arriving (tile ranges and sources decoded by the engine), the page's own
   * work after each frame (cards, links, the accessibility mirror), and what's left of each gap
   * that none of it explains (the GPU finishing, garbage collection, the browser's compositor). */
  data: Spread;
  page: Spread;
  other: Spread;
  /** Streaming: frames drawn while tiles were still downloading (drawn from coarser ones meanwhile),
   * the most missing at once, how many were still missing when the motion ended, and how long
   * after that the view was complete (ms; null while waiting, or if nothing was missing). */
  pendingFrames: number;
  pendingMax: number;
  pendingAtEnd: number;
  complete: number | null;
  /** The transition's slowest frame (engine + raster, if over 12 ms) and what it did: which frame,
   * display ops, meshes tessellated, instances rebuilt, tiles still downloading — what a spike is
   * made of, measured where a profiler (which shifts the timing) might not catch it. */
  slowest: { frame: number; engine: number; raster: number; ops: number; tessellated: number; rebuilt: number; pending: number } | null;
}

export interface Spread {
  p50: number;
  p95: number;
  max: number;
}

const FRAME_60 = 1000 / 60;

function spread(xs: number[]): Spread {
  if (!xs.length) return { p50: 0, p95: 0, max: 0 };
  const s = [...xs].sort((a, b) => a - b);
  const at = (q: number) => s[Math.min(s.length - 1, Math.floor(q * s.length))];
  const r = (v: number) => Math.round(v * 10) / 10;
  return { p50: r(at(0.5)), p95: r(at(0.95)), max: r(s[s.length - 1]) };
}

export function perfRequested(el: HTMLElement): boolean {
  if (el.hasAttribute("perf")) return true;
  if (/[?&#]datars-perf\b/.test(location.search + location.hash)) return true;
  try {
    return localStorage.getItem("datars:perf") === "1";
  } catch {
    return false;
  }
}

declare global {
  interface Window {
    __datarsPerf?: PerfSummary[];
  }
}

/** What the panel shows besides frame times (read when it redraws). */
export interface PanelInfo {
  /** `view.nerd_stats()`: renderer and caches, timing, the transition in flight. */
  stats: Record<string, any> | null;
  state: string;
  index: number;
  states: number;
  size: { w: number; h: number };
  src: string;
  /** Bytes of wasm memory (every view on the page shares it). */
  memory: number | null;
}

const HISTORY = 180;

function bytes(n: number): string {
  return n >= 1 << 20 ? `${(n / (1 << 20)).toFixed(1)} MB` : n >= 1024 ? `${Math.round(n / 1024)} KB` : `${n} B`;
}

function int(n: number): string {
  return Math.round(n).toLocaleString("en").replace(/,/g, " ");
}

/** The frame profiler and its "stats for nerds" panel. */
export class FrameProfiler {
  private engine: number[] = [];
  private raster: number[] = [];
  private rebuilt: number[] = [];
  private slowest: PerfSummary["slowest"] = null;
  private interval: number[] = [];
  private dataMs: number[] = [];
  private pageMs: number[] = [];
  private otherMs: number[] = [];
  private dataAcc = 0;
  private lastWork = 0;
  private pendingFrames = 0;
  private pendingMax = 0;
  private pendingLast = 0;
  /** A finished run still waiting for tiles: when its motion ended. */
  private awaiting: { summary: PerfSummary; since: number } | null = null;
  private ops = 0;
  private first = 0;
  private last = 0;
  private from = "";
  private shown = 0;
  private firstCost = 0;
  private stall: number | null = null;
  private inputMs: number | null = null;
  private pending: { at: number; ms: number; from: string } | null = null;
  private lastSummary: PerfSummary | null = null;
  /** The last HISTORY frames across runs: [engine, raster, interval (0 for a run's first)]. */
  private history: [number, number, number][] = [];
  private text: HTMLPreElement;
  private graph: HTMLCanvasElement;

  constructor(private panel: HTMLElement, private info: () => PanelInfo, private emit: (s: PerfSummary) => void) {
    panel.replaceChildren();
    const close = document.createElement("button");
    close.textContent = "×";
    close.setAttribute("aria-label", "Close stats");
    close.addEventListener("click", () => (panel.hidden = true));
    this.text = document.createElement("pre");
    this.graph = document.createElement("canvas");
    this.graph.width = 480;
    this.graph.height = 72;
    panel.append(close, this.text, this.graph);
    // Shown by the element (`showStats`); a tool reading `record()` keeps it hidden.
    panel.hidden = true;
  }

  /** The last few seconds of frames — `[engine ms, raster ms, gap before it ms]`, the gap 0 for a
   * run's first frame — and whether frames are coming now: what a tool draws as a live chart. */
  record(): { frames: [number, number, number][]; moving: boolean } {
    return { frames: this.history.slice(), moving: this.first > 0 };
  }

  get visible(): boolean {
    return !this.panel.hidden;
  }
  set visible(v: boolean) {
    this.panel.hidden = !v;
    if (v) this.draw(performance.now());
  }

  /** Data the engine took in between frames (a tile range, a source): ms spent. */
  data(ms: number) {
    this.dataAcc += ms;
  }

  /** The page's own work after a frame (placing cards and links, syncing the mirror). */
  page(ms: number) {
    if (this.pageMs.length < this.engine.length) this.pageMs.push(ms);
    this.lastWork += ms;
  }

  /** An input that may start a transition: when it came (`performance.now()`), how long the
   * engine took to handle it, and the state it left. */
  input(at: number, ms: number, from: string) {
    this.pending = { at, ms, from };
    // An input mid-run (a step during a transition) starts a new run from here.
    if (this.first) this.end(from, true);
  }

  /** A frame the page drew at `now`: `timing` is the engine's `[engine ms, raster ms, ops]`. */
  frame(now: number, timing: number[] | undefined, moving: boolean, state: string) {
    const [engine = 0, raster = 0, ops = 0, pending = 0, rebuilt = 0, tessellated = 0] = timing ?? [];
    // A finished run waiting for its tiles: done when a frame has them all.
    if (this.awaiting && pending === 0) {
      const a = this.awaiting;
      this.awaiting = null;
      a.summary.complete = Math.round(now - a.since);
      console.info(`datars perf ${a.summary.label}: view complete ${a.summary.complete} ms after the motion ended (${a.summary.pendingAtEnd} tiles were still downloading)`);
      this.draw(performance.now());
    }
    if (pending > 0) {
      this.pendingFrames++;
      this.pendingMax = Math.max(this.pendingMax, pending);
    }
    this.pendingLast = pending;
    let gap = 0;
    if (!this.first) {
      this.first = now;
      this.from = this.from || state;
      this.firstCost = engine + raster;
      // The input that started this, if it was recent: how long until something was on screen.
      const p = this.pending;
      this.pending = null;
      const end = performance.now();
      if (p && end - p.at < 2000) {
        this.stall = end - p.at;
        this.inputMs = p.ms;
        this.from = p.from;
      } else {
        this.stall = this.inputMs = null;
      }
    } else {
      gap = now - this.last;
      this.interval.push(gap);
      this.dataMs.push(this.dataAcc);
      this.otherMs.push(Math.max(0, gap - this.lastWork - this.dataAcc));
    }
    this.dataAcc = 0;
    this.lastWork = engine + raster;
    this.last = now;
    if (engine + raster > Math.max(12, this.slowest ? this.slowest.engine + this.slowest.raster : 0)) {
      const r = (v: number) => Math.round(v * 10) / 10;
      this.slowest = { frame: this.engine.length, engine: r(engine), raster: r(raster), ops, tessellated, rebuilt, pending };
    }
    this.engine.push(engine);
    this.raster.push(raster);
    this.rebuilt.push(rebuilt);
    this.ops = Math.max(this.ops, ops);
    this.history.push([engine, raster, gap]);
    if (this.history.length > HISTORY) this.history.shift();
    if (!moving) this.end(state);
    else if (now - this.shown > 250) this.draw(now);
  }

  /** Motion stopped (or a new input interrupted it): summarise the run. */
  private end(state: string, interrupted = false) {
    if (this.engine.length > 1) {
      const ms = this.last - this.first;
      const s: PerfSummary = {
        label: (this.from && this.from !== state ? `${this.from} → ${state}` : "frame") + (interrupted ? " (interrupted)" : ""),
        renderer: String(this.info().stats?.renderer ?? "?"),
        frames: this.engine.length,
        ms: Math.round(ms),
        fps: ms > 0 ? Math.round(((this.engine.length - 1) * 1000) / ms) : 0,
        engine: spread(this.engine),
        raster: spread(this.raster),
        interval: spread(this.interval),
        long: this.interval.filter((i) => i > FRAME_60 * 1.5).length,
        dropped: this.interval.reduce((n, i) => n + Math.max(0, Math.round(i / FRAME_60) - 1), 0),
        worst: Math.round(Math.max(0, ...this.interval)),
        minFps: 0,
        stall: this.stall === null ? null : Math.round(this.stall),
        input: this.inputMs === null ? null : Math.round(this.inputMs * 10) / 10,
        first: Math.round(this.firstCost * 10) / 10,
        ops: this.ops,
        // After the first frame (which builds what the transition brings in).
        rebuilt: spread(this.rebuilt.slice(1)),
        data: spread(this.dataMs),
        page: spread(this.pageMs),
        other: spread(this.otherMs),
        pendingFrames: this.pendingFrames,
        pendingMax: this.pendingMax,
        pendingAtEnd: this.pendingLast,
        complete: this.pendingLast > 0 ? null : this.pendingFrames > 0 ? 0 : null,
        slowest: this.slowest,
      };
      if (this.pendingLast > 0) this.awaiting = { summary: s, since: this.last };
      s.minFps = s.worst > 0 ? Math.round(1000 / s.worst) : s.fps;
      (window.__datarsPerf ??= []).push(s);
      const stall = s.stall === null ? "" : `stall ${s.stall} ms (input ${s.input} + first frame ${s.first}); `;
      console.info(
        `datars perf ${s.label}: ${stall}${s.fps} fps, min ${s.minFps} (worst gap ${s.worst} ms, ${s.dropped} frames dropped) over ${s.frames} frames (${s.renderer}); engine p50 ${s.engine.p50} / p95 ${s.engine.p95} / max ${s.engine.max} ms; raster p50 ${s.raster.p50} / p95 ${s.raster.p95} / max ${s.raster.max} ms; instances rebuilt per frame p50 ${int(s.rebuilt.p50)} / max ${int(s.rebuilt.max)}; between frames: data max ${s.data.max}, page max ${s.page.max}, unexplained p95 ${s.other.p95} / max ${s.other.max} ms; ${s.ops} ops${s.slowest ? `; slowest frame ${s.slowest.frame}: engine ${s.slowest.engine} + raster ${s.slowest.raster} ms, ${s.slowest.tessellated} meshes tessellated, ${int(s.slowest.rebuilt)} instances rebuilt${s.slowest.pending ? `, ${s.slowest.pending} tiles downloading` : ""}` : ""}`,
      );
      this.lastSummary = s;
      this.emit(s);
    }
    this.engine = [];
    this.raster = [];
    this.rebuilt = [];
    this.slowest = null;
    this.interval = [];
    this.dataMs = [];
    this.pageMs = [];
    this.otherMs = [];
    this.pendingFrames = 0;
    this.pendingMax = 0;
    this.ops = 0;
    this.first = 0;
    this.from = state;
    this.draw(performance.now());
  }

  /** Redraw the panel: the text, then the frame graph. */
  private draw(now: number) {
    this.shown = now;
    if (this.panel.hidden) return;
    const i = this.info();
    const st = i.stats ?? {};
    const g = st.gpu;
    const [eng = 0, ras = 0, ops = 0, pend = 0, reb = 0] = (st.timing as number[] | undefined) ?? [];
    const recent = this.history.slice(-30).map((h) => h[2]).filter((v) => v > 0);
    const fps = recent.length ? Math.round(1000 / (recent.reduce((a, b) => a + b, 0) / recent.length)) : 0;
    const worst = recent.length ? Math.max(...recent) : 0;
    const renderer = g ? `gpu ${g.backend} ${g.samples}×MSAA` : String(st.renderer ?? "?");
    const physical = g ? `${g.size[0]}×${g.size[1]} px` : `${Math.round(i.size.w * (st.dpr ?? 1))}×${Math.round(i.size.h * (st.dpr ?? 1))} px`;
    const lines = [
      `datars   ${renderer} · ${i.size.w}×${i.size.h} @${st.dpr ?? "?"} (${physical})${st.tier ? ` · ${st.tier}` : ""}`,
      `chart    ${i.src}`,
      `state    ${i.state} (${i.index + 1}/${i.states})${st.transition ? ` · moving ${Math.round(st.transition.progress * 100)}%` : ""}`,
      `fps      ${this.first ? fps : "idle"}${this.first && worst ? ` · min ${Math.round(1000 / worst)} (worst gap ${Math.round(worst)} ms)` : ""}`,
      `frame    engine ${eng.toFixed(1)} ms · raster ${ras.toFixed(1)} ms · ${int(ops)} ops · ${int(reb)} instances rebuilt${pend ? ` · ${pend} tiles downloading` : ""}`,
    ];
    if (g) {
      lines.push(`draw     ${int(g.draws)} draws · ${int(g.instances)} instances (${int(g.instance_sets_kept ?? 0)} big sets kept) · ${g.passes} passes · arena ${int(g.arena_meshes)} meshes (${bytes(g.arena_bytes)})`);
      lines.push(`cache    ${int(g.meshes)} meshes (${bytes(g.mesh_bytes)}) · ${int(g.tessellations)} tessellated · ${int(g.cache_hits)} hits · ${int(g.evictions)} evicted`);
    }
    if (st.transition) {
      const t = st.transition;
      const tracks = t.pairs + t.enters + t.exits;
      lines.push(`motion   ${int(tracks)} tracks (${t.pairs} pairs, ${t.enters} in, ${t.exits} out) · ${int(t.morphs)} morphs · ${t.splits} splits · ${int(t.instances)} instanced · ${t.crossfades} crossfades`);
    }
    const s = this.lastSummary;
    if (s) {
      lines.push(`last     ${s.label}: ${s.stall === null ? "" : `stall ${s.stall} ms (input ${s.input} + 1st ${s.first}) · `}${s.fps} fps, min ${s.minFps} · ${s.dropped} dropped`);
      lines.push(`         engine p50 ${s.engine.p50} / p95 ${s.engine.p95} · raster p50 ${s.raster.p50} / p95 ${s.raster.p95} ms · rebuilt p50 ${int(s.rebuilt.p50)} / max ${int(s.rebuilt.max)} instances a frame`);
      lines.push(`         between frames: data max ${s.data.max} · page max ${s.page.max} · unexplained p95 ${s.other.p95} / max ${s.other.max} ms`);
      if (s.pendingFrames) lines.push(`         streaming: ${s.pendingFrames} frames with tiles missing (≤ ${s.pendingMax}) · ${s.pendingAtEnd ? (s.complete === null ? `${s.pendingAtEnd} still downloading…` : `complete ${s.complete} ms after the motion`) : "complete when the motion ended"}`);
    }
    if (i.memory) lines.push(`memory   wasm ${bytes(i.memory)} (shared by the page's charts)`);
    this.text.textContent = lines.join("\n");
    this.plot();
  }

  /** Each frame a bar: engine (blue) under raster (orange), height in ms, the 60 Hz line at
   * 16.7 ms; a red tick where the gap before a frame missed a refresh. */
  private plot() {
    const c = this.graph.getContext("2d");
    if (!c) return;
    const { width: w, height: h } = this.graph;
    const top = 50; // ms at the top of the graph
    const y = (ms: number) => h - Math.min(ms, top) * (h / top);
    c.clearRect(0, 0, w, h);
    const bw = w / HISTORY;
    this.history.forEach(([e, r, gap], k) => {
      const x = k * bw;
      c.fillStyle = "#5b9bd5";
      c.fillRect(x, y(e), Math.max(1, bw - 1), h - y(e));
      c.fillStyle = "#ed7d31";
      c.fillRect(x, y(e + r), Math.max(1, bw - 1), y(e) - y(e + r));
      if (gap > FRAME_60 * 1.5) {
        c.fillStyle = "#ff4d4d";
        c.fillRect(x, 0, Math.max(1, bw - 1), 4);
      }
    });
    c.strokeStyle = "rgba(255,255,255,.6)";
    c.setLineDash([3, 3]);
    c.beginPath();
    c.moveTo(0, y(FRAME_60));
    c.lineTo(w, y(FRAME_60));
    c.stroke();
    c.setLineDash([]);
    c.fillStyle = "rgba(255,255,255,.7)";
    c.font = "10px ui-monospace, Menlo, monospace";
    c.fillText("16.7 ms", 4, y(FRAME_60) - 3);
  }
}
