"use client";
// @datars/react — datars charts in React apps: <DatarsView chart={sales} />.
//
// The component renders a box at the chart's aspect (from the document's authored size) on the
// server and on the client alike, so nothing moves when the chart arrives; after hydration it loads
// the runtime (@datars/web, a lazy chunk) and puts a <datars-view> in that box — near the viewport
// only, by default, and taken out again far away (every chart on a page shares one engine memory).
// State, signals, theme tokens and data are props; the element's API is on the ref.
import { forwardRef, useCallback, useEffect, useImperativeHandle, useLayoutEffect, useRef, useState, useSyncExternalStore, type CSSProperties, type HTMLAttributes, type RefObject } from "react";
import type { DatarsView as DatarsViewElement } from "@datars/web";

/** A compiled chart, as the bundler plugins (`@datars/vite`, `@datars/next`) import chart files. */
export interface DatarsChart {
  readonly kind: "datars-chart";
  /** The chart file, relative to the project: HMR updates find their views by it. */
  readonly id?: string;
  /** The document's authored size, [width, height] — the box's aspect before anything loads. */
  readonly size?: readonly [number, number];
  readonly title?: string | null;
  /** The program's steps, by name: a stepper can be drawn before the chart loads. */
  readonly states?: readonly string[];
  /** The document (JSON IR): played by the full engine. */
  readonly doc?: object;
  /** A published bundle's manifest URL: played by the smaller core engine. */
  readonly src?: string;
}

/** The chart's state, as the element reports it (`state` events). */
export interface DatarsStatus {
  /** The current step's index and name, and every step's name (a story's program). */
  index: number;
  state: string;
  states: string[];
  narration: { title?: string; text?: string; anchor?: string } | null;
  [key: string]: unknown;
}

/** The element's API, on the component's ref. Calls before the chart has loaded are kept where
 * the element keeps them (signals, tokens, data) or dropped (events). */
export interface DatarsViewHandle {
  /** The `<datars-view>` element (null until mounted: after hydration, near the viewport). */
  readonly element: DatarsViewElement | null;
  /** The last reported state (null until the chart has drawn). */
  readonly status: DatarsStatus | null;
  /** Fire a program event: `next`, `prev`, `back`, `goto:<state>`. */
  send(event: string): void;
  /** Go to a step, by name or index. */
  goto(state: string | number): void;
  next(): void;
  prev(): void;
  /** Set a signal (filters, sliders, app state). */
  setSignal(name: string, value: unknown): void;
  /** Override theme tokens (brand colours, fonts); the theme's locks hold. */
  setTokens(tokens: Record<string, unknown>): void;
  /** Provide a data slot (CSV/JSON text or bytes, rows, or columns). */
  setData(name: string, data: string | Uint8Array | object): void;
  /** Listen to state changes (what `useDatarsState` uses). Returns the unsubscribe. */
  subscribe(listener: (status: DatarsStatus) => void): () => void;
}

export interface DatarsViewProps extends Omit<HTMLAttributes<HTMLDivElement>, "children"> {
  /** A chart file's import (`import sales from "./sales.chart.ts"`), or a document object. */
  chart?: DatarsChart | object | null;
  /** A document (JSON IR, object or text) held by the app; a new one morphs from what's shown. */
  document?: object | string | null;
  /** A published bundle's manifest URL (`datars publish … --to site/` → `/c/<alias>`). */
  src?: string;
  /** A raw document's URL (`doc.json`). */
  doc?: string;
  /** Go to this step (name or index) when it changes; also where the chart opens. */
  state?: string | number;
  /** Colour mode (default: follows `prefers-color-scheme`). */
  mode?: "light" | "dark" | "high-contrast";
  /** The box's height (px or any CSS length). Default: the chart's aspect across the width. */
  height?: number | string;
  /** Width / height, when the chart doesn't say (a `src` bundle before it loads). */
  aspectRatio?: number;
  /** Signals to set (applied as they change; the chart opens with them). */
  signals?: Record<string, unknown>;
  /** Theme token overrides (applied as they change). */
  tokens?: Record<string, unknown>;
  /** Data slots (applied as they change). */
  data?: Record<string, string | Uint8Array | object>;
  /** Mount only near the viewport, and unmount far away (default `true`: within 150% of the
   * viewport's height). `false`: mount after hydration and stay. A string: the IntersectionObserver
   * rootMargin. The step a reader was on is kept across unmounts. */
  lazy?: boolean | string;
  /** Where the runtime's engines, fonts and atlas are served from (the plugins set this). */
  runtime?: string;
  /** The chart's accessible name (default: the element's own, from the chart's semantics). */
  label?: string;
  /** Other `<datars-view>` attributes: `scrub`, `steps`, `cpu`, `no-script`, `publishers`, `perf`… */
  attributes?: Record<string, string | boolean | number | undefined | null>;
  /** The chart moved to another step (and when it first draws). */
  onStateChange?: (status: DatarsStatus) => void;
  /** The chart drew its first frame. */
  onReady?: (element: DatarsViewElement) => void;
  /** The runtime failed to load. */
  onError?: (error: unknown) => void;
}

/** The environment's runtime folder: Next.js inlines `process.env.DATARS_RUNTIME` (withDatars). */
declare const process: { env: Record<string, string | undefined> };
function envRuntime(): string | undefined {
  try {
    return process.env.DATARS_RUNTIME || undefined;
  } catch {
    return undefined;
  }
}

type WebModule = typeof import("@datars/web");
let webModule: Promise<WebModule> | null = null;
/** Load the runtime once (a lazy chunk: pages without charts never pay for it). */
function loadRuntime(runtime?: string): Promise<WebModule> {
  webModule ??= import("@datars/web");
  return webModule.then((m) => {
    const base = runtime ?? envRuntime();
    if (base) m.setRuntimeBase(base);
    return m;
  });
}

const useIsoLayoutEffect = typeof window === "undefined" ? useEffect : useLayoutEffect;

/** A chart prop as a chart: a plugin import as is, a bare document wrapped. */
function asChart(x: DatarsChart | object | null | undefined): DatarsChart | null {
  if (!x || typeof x !== "object") return null;
  const o = x as Record<string, unknown>;
  if (o.kind === "datars-chart") return x as DatarsChart;
  if ("scene" in o || "datars" in o) {
    const states = ((o.program as { states?: { name?: string }[] } | undefined)?.states ?? []).map((s) => s.name ?? "");
    return { kind: "datars-chart", doc: x, size: docSize(o) ?? undefined, title: typeof o.title === "string" ? o.title : null, states };
  }
  return null;
}

function docSize(doc: unknown): [number, number] | null {
  if (!doc || typeof doc !== "object") return null;
  const s = (doc as { size?: unknown }).size as { width?: number; height?: number } | number[] | undefined;
  if (Array.isArray(s) && s[0] > 0 && s[1] > 0) return [s[0], s[1]];
  if (s && !Array.isArray(s) && s.width && s.height) return [s.width, s.height];
  return null;
}

function toStatus(detail: unknown): DatarsStatus {
  const d = (detail ?? {}) as Record<string, unknown>;
  const states = Array.isArray(d.states) ? (d.states as string[]) : [];
  const index = typeof d.index === "number" ? d.index : 0;
  return { ...d, index, state: typeof d.state === "string" ? d.state : (states[index] ?? ""), states, narration: (d.narration as DatarsStatus["narration"]) ?? null };
}

function setAttr(el: Element, name: string, value: string | number | boolean | null | undefined) {
  if (value === undefined || value === null || value === false) el.removeAttribute(name);
  else el.setAttribute(name, value === true ? "" : String(value));
}

/** Apply a record prop's changes (signals, tokens) — by value, so an inline object each render
 * doesn't re-send anything. */
function changed(prev: Record<string, unknown> | undefined, next: Record<string, unknown> | undefined): [string, unknown][] {
  const out: [string, unknown][] = [];
  for (const [k, v] of Object.entries(next ?? {})) if (!prev || !(k in prev) || JSON.stringify(prev[k]) !== JSON.stringify(v)) out.push([k, v]);
  return out;
}

export const DatarsView = forwardRef<DatarsViewHandle, DatarsViewProps>(function DatarsView(props, ref) {
  const { chart: chartProp, document: documentProp, src: srcProp, doc: docUrl, state, mode, height, aspectRatio, signals, tokens, data, lazy = true, runtime, label, attributes, onStateChange, onReady, onError, style, ...rest } = props;

  // A chart file edited in dev: the plugin's module hands the new version to views showing it.
  const [update, setUpdate] = useState<{ for: unknown; chart: DatarsChart } | null>(null);
  const baseChart = asChart(chartProp);
  const chart = update && update.for === chartProp ? update.chart : baseChart;
  const src = srcProp ?? chart?.src;
  const document = documentProp ?? chart?.doc ?? null;
  const size = chart?.size ?? docSize(typeof documentProp === "string" ? null : documentProp);
  /** What the element plays: a change of kind or URL is a new element, a new document a morph. */
  const sourceKey = src ? `src:${src}` : docUrl ? `doc:${docUrl}` : document ? "document" : "";

  const box = useRef<HTMLDivElement>(null);
  const el = useRef<DatarsViewElement | null>(null);
  const status = useRef<DatarsStatus | null>(null);
  const listeners = useRef(new Set<(s: DatarsStatus) => void>());
  /** The step the reader was on, kept across unmounts (lazy) and element swaps. */
  const lastIndex = useRef<number | null>(null);
  const applied = useRef<{ document: unknown; signals?: Record<string, unknown>; tokens?: Record<string, unknown>; data?: Record<string, unknown> }>({ document: null });
  const latest = useRef(props);
  latest.current = props;
  const current = useRef({ src, docUrl, document, sourceKey });
  current.current = { src, docUrl, document, sourceKey };
  const [near, setNear] = useState(false);

  useImperativeHandle(ref, () => ({
    get element() { return el.current; },
    get status() { return status.current; },
    send: (ev) => el.current?.send(ev),
    goto: (s) => {
      const name = typeof s === "number" ? status.current?.states[s] : s;
      if (name !== undefined) el.current?.send(`goto:${name}`);
      else if (typeof s === "number") el.current?.setAttribute("state", String(s));
    },
    next: () => el.current?.send("next"),
    prev: () => el.current?.send("prev"),
    setSignal: (n, v) => el.current?.setSignal(n, v),
    setTokens: (t) => el.current?.setTokens(t),
    setData: (n, d) => el.current?.provideData(n, d),
    subscribe: (l) => {
      listeners.current.add(l);
      if (status.current) l(status.current);
      return () => void listeners.current.delete(l);
    },
  }), []);

  // Near the viewport? (Only on the client, after hydration: the server and the first client
  // render agree on an empty box.)
  useEffect(() => {
    const host = box.current;
    if (!host) return;
    if (lazy === false || typeof IntersectionObserver === "undefined") {
      setNear(true);
      return;
    }
    const io = new IntersectionObserver((es) => setNear(es.some((e) => e.isIntersecting)), { rootMargin: typeof lazy === "string" ? lazy : "150% 0px" });
    io.observe(host);
    return () => io.disconnect();
  }, [lazy]);

  // HMR (dev): the plugin's chart module announces its new version.
  useEffect(() => {
    const id = baseChart?.id;
    if (!id || typeof addEventListener !== "function") return;
    const on = (e: Event) => {
      const next = (e as CustomEvent).detail as DatarsChart | undefined;
      if (next?.id === id) setUpdate({ for: chartProp, chart: next });
    };
    addEventListener("datars:update", on);
    return () => removeEventListener("datars:update", on);
  }, [baseChart?.id, chartProp]);

  /** A new element for the current source, set up before it's on the page (it starts when it
   * connects), then put in the box — over the one it replaces, which goes once the new one draws. */
  const mountElement = async (cancelled: () => boolean) => {
    const host = box.current;
    const now = current.current;
    if (!host || !now.sourceKey) return;
    await loadRuntime(latest.current.runtime);
    if (cancelled() || !box.current) return;
    const p = latest.current;
    const v = window.document.createElement("datars-view") as DatarsViewElement;
    v.style.cssText = "position:absolute;left:0;top:0;width:100%;";
    v.setAttribute("no-controls", "");
    setAttr(v, "src", now.src);
    setAttr(v, "doc", now.src ? undefined : now.docUrl);
    setAttr(v, "mode", p.mode);
    setAttr(v, "aria-label", p.label);
    setAttr(v, "height", Math.round(host.clientHeight) || undefined);
    // Where the reader was (back near the viewport, or a new version of the chart), else the prop.
    setAttr(v, "state", lastIndex.current ?? p.state ?? undefined);
    for (const [k, val] of Object.entries(p.attributes ?? {})) setAttr(v, k, val);
    if (!now.src && !now.docUrl && now.document) void v.setDocument(now.document);
    applied.current.document = now.document;
    for (const [k, val] of Object.entries(p.signals ?? {})) v.setSignal(k, val);
    if (p.tokens) v.setTokens(p.tokens);
    for (const [k, val] of Object.entries(p.data ?? {})) v.provideData(k, val);
    applied.current.signals = p.signals;
    applied.current.tokens = p.tokens;
    applied.current.data = p.data;
    let first = true;
    v.addEventListener("state", (e) => {
      if (el.current !== v) return;
      const s = toStatus((e as CustomEvent).detail);
      status.current = s;
      lastIndex.current = s.index;
      if (first) {
        first = false;
        host.dataset.datars = "ready";
        latest.current.onReady?.(v);
      }
      latest.current.onStateChange?.(s);
      listeners.current.forEach((l) => l(s));
    });
    const old = el.current;
    el.current = v;
    host.dataset.datars = "loading";
    host.appendChild(v);
    if (old) {
      // The old chart stays until the new one has drawn (a swap never shows an empty box).
      const drop = () => old.remove();
      v.addEventListener("state", drop, { once: true });
      setTimeout(drop, 4000);
    }
  };

  // Mount near the viewport; unmount far away (and on unmount).
  useEffect(() => {
    if (!near || !sourceKey) return;
    let done = false;
    mountElement(() => done).catch((e) => (latest.current.onError ? latest.current.onError(e) : console.error("datars:", e)));
    return () => {
      done = true;
      const host = box.current;
      el.current = null;
      status.current = null;
      if (host) {
        host.querySelectorAll("datars-view").forEach((n) => n.remove());
        delete host.dataset.datars;
      }
    };
    // A new source (another bundle URL or document URL) is a new element: the effect below.
  }, [near, !!sourceKey]); // eslint-disable-line react-hooks/exhaustive-deps

  // Another source while mounted: a new element, swapped in once it draws.
  const mountedKey = useRef(sourceKey);
  useEffect(() => {
    if (mountedKey.current === sourceKey) return;
    mountedKey.current = sourceKey;
    if (!el.current || !sourceKey) return;
    let done = false;
    mountElement(() => done).catch((e) => (latest.current.onError ? latest.current.onError(e) : console.error("datars:", e)));
    return () => {
      done = true;
    };
  }, [sourceKey]); // eslint-disable-line react-hooks/exhaustive-deps

  // A new document for the same element: it morphs from what's shown, keeping its step.
  useEffect(() => {
    const v = el.current;
    if (!v || sourceKey !== "document" || !document || applied.current.document === document) return;
    applied.current.document = document;
    void v.setDocument(document);
  }, [document, sourceKey]);

  // Attributes that follow props.
  useEffect(() => {
    if (el.current) setAttr(el.current, "mode", mode);
  }, [mode]);
  useEffect(() => {
    if (el.current) setAttr(el.current, "aria-label", label);
  }, [label]);
  const lastState = useRef(state);
  useEffect(() => {
    if (lastState.current === state) return;
    lastState.current = state;
    if (el.current && state !== undefined) el.current.setAttribute("state", String(state));
  }, [state]);
  useEffect(() => {
    const v = el.current;
    if (!v) return;
    for (const [k, val] of changed(applied.current.signals, signals)) v.setSignal(k, val);
    applied.current.signals = signals;
  }, [signals]);
  useEffect(() => {
    const v = el.current;
    if (!v || !tokens || JSON.stringify(tokens) === JSON.stringify(applied.current.tokens)) return;
    v.setTokens(tokens);
    applied.current.tokens = tokens;
  }, [tokens]);
  useEffect(() => {
    const v = el.current;
    if (!v) return;
    for (const [k, val] of Object.entries(data ?? {})) if (applied.current.data?.[k] !== val) v.provideData(k, val);
    applied.current.data = data;
  }, [data]);

  // The box decides the chart's height: the element follows it (never the other way round).
  useIsoLayoutEffect(() => {
    const host = box.current;
    if (!host || typeof ResizeObserver === "undefined") return;
    let h = -1;
    const fit = () => {
      const next = Math.round(host.clientHeight);
      if (next === h || next <= 0) return;
      h = next;
      host.querySelectorAll("datars-view").forEach((v) => v.setAttribute("height", String(next)));
    };
    const ro = new ResizeObserver(fit);
    ro.observe(host);
    fit();
    return () => ro.disconnect();
  }, [near]);

  const boxStyle: CSSProperties = {
    position: "relative",
    display: "block",
    width: "100%",
    ...(height !== undefined ? { height } : { aspectRatio: aspectRatio ? String(aspectRatio) : size ? `${size[0]} / ${size[1]}` : "5 / 3" }),
    ...style,
  };
  return <div {...rest} ref={box} style={boxStyle} data-datars-view="" />;
});

/** The chart's state, re-rendering when it changes: `const s = useDatarsState(ref)` →
 * `s?.state`, `s?.index`, `s?.states` (null until the chart has drawn). */
export function useDatarsState(ref: RefObject<DatarsViewHandle | null>): DatarsStatus | null {
  const snapshot = useRef<DatarsStatus | null>(null);
  const subscribe = useCallback((notify: () => void) => {
    const h = ref.current;
    if (!h) return () => {};
    return h.subscribe((s) => {
      snapshot.current = s;
      notify();
    });
  }, [ref]);
  return useSyncExternalStore(subscribe, () => snapshot.current, () => null);
}

export type { DatarsViewElement };
