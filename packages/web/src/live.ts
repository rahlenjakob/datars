// Live feeds for a chart's live source (docs/06 §Live data): the transport is the host's — a
// WebSocket, server-sent events, a poll — and the engine only sees versions. `liveFeed` is the
// part every page would otherwise write again:
//
// - **Pace.** Rows are buffered and handed to the chart as one version at most every `pace` ms
//   (a trade feed sends dozens a second; the chart moves once per beat, and the engine caps that
//   move to the beat, so motion never queues behind the feed).
// - **Back off, recover.** A connection that fails or drops is retried after 1 s, 2 s, 4 s … up
//   to a minute (with jitter, so a thousand readers don't reconnect in step); one that works resets
//   the delay.
// - **Pause.** While the tab is hidden or the chart is off the screen the connection is closed
//   (after `linger` ms, so scrolling past doesn't churn it) and opened again when it's back —
//   a reader's quota and battery aren't spent on a chart nobody sees. `connect` runs again then,
//   and can fetch what it missed.
// - **Remounts.** Pages free charts far from the screen and mount them again near it: a new
//   element gets every row kept so far (`keep`), then the new ones.
//
//   const feed = liveFeed({ view: () => slot.querySelector("datars-view"), watch: slot,
//     source: "prices", connect: socket("wss://…", { open: (ws) => ws.send(…), message: (m) => rows }) });
//
// The chart's source declares how rows merge (`live: { mode: "upsert", span: … }`); the feed only
// delivers them.

/** What a feed is doing, for the page to say so ("Live", "Reconnecting in 8 s", "Paused"). */
export interface FeedStatus {
  state: "connecting" | "live" | "paused" | "retrying" | "stopped";
  /** While retrying: ms until the next attempt. */
  retryIn?: number;
  /** The last failure, said. */
  error?: string;
  /** Failed attempts since the last connection that worked. */
  failures: number;
  /** When the last rows arrived (ms since the epoch), if any. */
  lastRows?: number;
}

/** What `connect` gets: where rows go, and how to say the connection works or failed. */
export interface FeedContext<Row> {
  /** Rows that arrived (buffered; delivered at the feed's pace). */
  push(rows: Row[]): void;
  /** The connection works: the backoff resets, the status is `live`. */
  ok(): void;
  /** The connection failed or dropped: it is closed and tried again after the backoff. */
  fail(reason?: unknown): void;
  /** Aborted when the feed pauses, retries or stops: close sockets and cancel fetches on it. */
  signal: AbortSignal;
}

export interface LiveFeedOptions<Row> {
  /** The chart's source the rows go to. */
  source: string;
  /** The `<datars-view>`, or a function returning the one on the page now (charts mount near the
   * screen and are freed far from it). */
  view: ViewLike | (() => ViewLike | null | undefined);
  /** The element whose visibility pauses the feed (default: the view, when one is given). */
  watch?: Element | null;
  /** Open the connection. Called again after each pause or failure. May return a cleanup. */
  connect(cx: FeedContext<Row>): void | (() => void) | Promise<void | (() => void)>;
  /** Least ms between versions handed to the chart (default 1000). */
  pace?: number;
  /** Backoff in ms: the first retry and the longest (default 1000 and 60000). */
  backoff?: { first?: number; max?: number };
  /** Ms hidden or off-screen before the connection closes (default 2000). */
  linger?: number;
  /** Rows kept for a newly mounted view (default 20000; the oldest go first). */
  keep?: number;
  onStatus?(s: FeedStatus): void;
  /** For tests: the clock and timers (default: the global ones). */
  timers?: { now(): number; setTimeout(f: () => void, ms: number): unknown; clearTimeout(id: unknown): void; random?(): number };
}

/** The part of `<datars-view>` a feed uses. */
export interface ViewLike {
  provideData(name: string, data: unknown): void;
  /** Set once the chart is running (its renderer): rows handed over before then wait. */
  dataset?: DOMStringMap;
}

export interface LiveFeed<Row> {
  readonly status: FeedStatus;
  /** Every row kept, oldest first (what a new view would get). */
  rows(): Row[];
  /** Hand buffered rows to the chart now. */
  flush(): void;
  /** Close the connection for good. */
  stop(): void;
}

export function liveFeed<Row>(o: LiveFeedOptions<Row>): LiveFeed<Row> {
  const T = o.timers ?? { now: () => Date.now(), setTimeout: (f: () => void, ms: number) => setTimeout(f, ms), clearTimeout: (id: unknown) => clearTimeout(id as ReturnType<typeof setTimeout>), random: Math.random };
  const random = T.random ?? Math.random;
  const pace = o.pace ?? 1000;
  const first = o.backoff?.first ?? 1000;
  const max = o.backoff?.max ?? 60000;
  const linger = o.linger ?? 2000;
  const keep = o.keep ?? 20000;
  const viewNow = () => (typeof o.view === "function" ? o.view() : o.view) ?? null;
  const status: FeedStatus = { state: "connecting", failures: 0 };
  const kept: Row[] = [];
  let pending: Row[] = [];
  let delivered: ViewLike | null = null;
  let abort: AbortController | null = null;
  let cleanup: (() => void) | null = null;
  let retry: unknown = null;
  let tick: unknown = null;
  let lingerTimer: unknown = null;
  let lastFlush = -Infinity;
  let stopped = false;
  let hidden = typeof document !== "undefined" && document.visibilityState === "hidden";
  let offscreen = false;

  const say = (patch: Partial<FeedStatus>) => {
    Object.assign(status, patch);
    if (patch.state !== "retrying") delete status.retryIn;
    o.onStatus?.({ ...status });
  };

  /** A running view (its renderer set) takes rows directly; a new one gets everything kept. */
  function deliver() {
    const v = viewNow();
    if (!v || (v.dataset && !v.dataset.renderer)) return false;
    const rows = v === delivered ? pending : kept;
    if (!rows.length) return true;
    try {
      v.provideData(o.source, rows);
    } catch (e) {
      console.warn("datars: live feed", o.source, e);
    }
    delivered = v;
    pending = [];
    lastFlush = T.now();
    return true;
  }

  function flush() {
    if (tick !== null) T.clearTimeout(tick);
    tick = null;
    if (!deliver() && !stopped) tick = T.setTimeout(flush, Math.min(pace, 250)); // the view isn't running yet
  }

  function push(rows: Row[]) {
    if (stopped || !rows?.length) return;
    kept.push(...rows);
    if (kept.length > keep) kept.splice(0, kept.length - keep);
    pending.push(...rows);
    say({ lastRows: T.now() });
    if (tick !== null) return;
    const wait = Math.max(0, lastFlush + pace - T.now());
    tick = T.setTimeout(flush, wait);
  }

  function close() {
    abort?.abort();
    abort = null;
    try { cleanup?.(); } catch { /* the connection is going anyway */ }
    cleanup = null;
  }

  function open() {
    if (stopped || abort) return;
    const ac = new AbortController();
    abort = ac;
    say({ state: "connecting" });
    const cx: FeedContext<Row> = {
      push: (rows) => { if (abort === ac) push(rows); },
      ok: () => { if (abort === ac) say({ state: "live", failures: 0, error: undefined }); },
      fail: (reason) => { if (abort === ac) failed(reason); },
      signal: ac.signal,
    };
    try {
      const r = o.connect(cx);
      if (r && typeof (r as Promise<unknown>).then === "function") {
        (r as Promise<void | (() => void)>).then((c) => { if (abort === ac) cleanup = c || null; else c?.(); }, (e) => cx.fail(e));
      } else if (typeof r === "function") cleanup = r;
    } catch (e) {
      cx.fail(e);
    }
  }

  function failed(reason: unknown) {
    close();
    const failures = status.failures + 1;
    // 1, 2, 4 … s, at most `max`; ± 20 % jitter so readers don't come back in step.
    const delay = Math.round(Math.min(max, first * 2 ** (failures - 1)) * (0.8 + 0.4 * random()));
    say({ state: "retrying", failures, retryIn: delay, error: reason instanceof Error ? reason.message : reason === undefined ? undefined : String(reason) });
    if (retry !== null) T.clearTimeout(retry);
    retry = T.setTimeout(() => { retry = null; if (!paused()) open(); }, delay);
  }

  const paused = () => hidden || offscreen;
  function visibility() {
    if (stopped) return;
    if (lingerTimer !== null) T.clearTimeout(lingerTimer);
    lingerTimer = null;
    if (paused()) {
      lingerTimer = T.setTimeout(() => {
        lingerTimer = null;
        if (!paused() || stopped) return;
        close();
        if (retry !== null) T.clearTimeout(retry);
        retry = null;
        flush();
        say({ state: "paused" });
      }, linger);
    } else if (!abort && retry === null) {
      open();
    }
  }

  const onVisibility = () => { hidden = document.visibilityState === "hidden"; visibility(); };
  if (typeof document !== "undefined") document.addEventListener("visibilitychange", onVisibility);
  const watched = o.watch ?? (typeof o.view === "function" ? null : (o.view as unknown as Element));
  let io: IntersectionObserver | null = null;
  if (watched && typeof IntersectionObserver !== "undefined") {
    io = new IntersectionObserver((es) => { offscreen = !es.some((e) => e.isIntersecting); visibility(); });
    io.observe(watched);
  }

  if (paused()) say({ state: "paused" });
  else open();

  return {
    get status() { return { ...status }; },
    rows: () => kept.slice(),
    flush,
    stop() {
      if (stopped) return;
      flush();
      stopped = true;
      close();
      for (const t of [retry, tick, lingerTimer]) if (t !== null) T.clearTimeout(t);
      io?.disconnect();
      if (typeof document !== "undefined") document.removeEventListener("visibilitychange", onVisibility);
      say({ state: "stopped" });
    },
  };
}

/** A WebSocket transport for `liveFeed`: `open` subscribes once connected, `message` turns each
 * message into rows (or nothing: a heartbeat). A close or error is a failure, retried. */
export function socket<Row>(url: string | (() => string), o: { open?(ws: WebSocket): void; message(data: unknown, ws: WebSocket): Row[] | null | undefined | void; protocols?: string | string[] }) {
  return (cx: FeedContext<Row>) => {
    const ws = new WebSocket(typeof url === "function" ? url() : url, o.protocols);
    ws.onopen = () => { cx.ok(); o.open?.(ws); };
    ws.onmessage = (m) => {
      let rows: Row[] | null | undefined | void;
      try { rows = o.message(typeof m.data === "string" ? safeJson(m.data) : m.data, ws); } catch (e) { console.warn("datars: live message", e); }
      if (rows && rows.length) cx.push(rows);
    };
    ws.onerror = () => cx.fail(new Error("connection error"));
    ws.onclose = (e) => cx.fail(new Error(e.reason || `closed (${e.code})`));
    return () => { ws.onclose = ws.onerror = ws.onmessage = null; ws.close(); };
  };
}

/** Server-sent events: each event's data, parsed as JSON when it is, turned into rows. */
export function events<Row>(url: string | (() => string), message: (data: unknown) => Row[] | null | undefined | void) {
  return (cx: FeedContext<Row>) => {
    const es = new EventSource(typeof url === "function" ? url() : url);
    es.onopen = () => cx.ok();
    es.onmessage = (m) => { const rows = message(safeJson(m.data)); if (rows && rows.length) cx.push(rows); };
    es.onerror = () => cx.fail(new Error("event stream error"));
    return () => es.close();
  };
}

/** Polling: `fetchRows` every `every` ms while the feed runs (a REST quote endpoint). A throw is a
 * failure, retried with the feed's backoff. */
export function poll<Row>(fetchRows: (signal: AbortSignal) => Promise<Row[] | null | undefined>, every: number) {
  return (cx: FeedContext<Row>) => {
    let timer: ReturnType<typeof setTimeout> | null = null;
    const once = async () => {
      timer = null;
      try {
        const rows = await fetchRows(cx.signal);
        if (cx.signal.aborted) return;
        cx.ok();
        if (rows && rows.length) cx.push(rows);
        timer = setTimeout(once, every);
      } catch (e) {
        if (!cx.signal.aborted) cx.fail(e);
      }
    };
    once();
    return () => { if (timer) clearTimeout(timer); };
  };
}

function safeJson(s: string): unknown {
  try { return JSON.parse(s); } catch { return s; }
}
