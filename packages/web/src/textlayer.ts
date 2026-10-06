// The text layer: a chart's words as real text laid over the canvas (as PDF.js does over a page),
// so readers can select and copy a title or a label, the browser's find-in-page finds them, and
// crawlers read them. What decides pixels stays in the engine: it says where every line of
// every text is drawn (`View.text_layer()`); this lays one transparent span per line exactly
// there — the chart's own font when the page can have it, stretched to the drawn width in any
// case — so a selection's highlight sits on the drawn glyphs.
//
// Accessibility: the layer is `aria-hidden`. The element's accessible version is its semantics
// mirror (roles, values, actions, in the chart's logical order); the same words again, positioned
// for the eye, would have screen readers read every title and label twice. Forced colours leave
// it alone (they'd paint the transparent text over the drawn one).

/** One line of a text, as the engine draws it: its box's top-left corner (the text's rotation
 * applied), the box before rotation, the baseline down from its top; `br`: a line break in the
 * text follows (not a wrap); `rtl`: its paragraph reads right to left. CSS px. */
export interface TextLine { text: string; x: number; y: number; w: number; h: number; baseline: number; br?: boolean; rtl?: boolean }

/** A text the chart draws (see `View.text_layer` in datars-host-web). */
export interface TextItem {
  path: string;
  text: string;
  role?: string;
  family: string;
  weight: number;
  faces: string[];
  size: number;
  rotate: number;
  opacity: number;
  place: boolean;
  drag: boolean;
  bounds: [number, number, number, number];
  lines: TextLine[];
}

export const TEXT_CSS = `
.stage { -webkit-user-select: none; user-select: none; }
.texts { position: absolute; inset: 0; overflow: hidden; pointer-events: none; white-space: pre; font-size: 0; line-height: 0; forced-color-adjust: none; }
.texts.moving { visibility: hidden; }
.texts span { position: absolute; left: 0; top: 0; color: transparent; transform-origin: 0 0; unicode-bidi: isolate; font-synthesis: none;
  font-kerning: normal; pointer-events: auto; -webkit-user-select: text; user-select: text; cursor: text; }
.texts span.pass { pointer-events: none; }
.texts span.sep { position: static; font-size: 1px; line-height: 0; pointer-events: none; }
.texts span::selection { color: transparent; background: rgba(66,105,208,.3); background: color-mix(in srgb, var(--accent, #4269d0) 32%, transparent); }
.texts span::-moz-selection { color: transparent; background: rgba(66,105,208,.3); }
`;

/** Font files registered with the page, by face and content: one `FontFace` however many charts
 * use it. */
const pageFaces = new Map<string, { family: string; loaded: Promise<unknown> }>();

/** A cheap fingerprint of a font file (a subset of the same face in another bundle differs). */
function fingerprint(b: Uint8Array): string {
  let h = 2166136261;
  const step = Math.max(1, Math.floor(b.length / 4096));
  for (let i = 0; i < b.length; i += step) h = Math.imul(h ^ b[i], 16777619);
  return `${b.length}:${(h >>> 0).toString(36)}`;
}

let measurer: CanvasRenderingContext2D | null = null;
function measure(font: string, text: string): number {
  measurer ??= document.createElement("canvas").getContext("2d");
  if (!measurer) return 0;
  measurer.font = font;
  return measurer.measureText(text).width;
}

/** What a line's span shows: its text and line, and the font it measures in. */
interface Placed { item: TextItem; line: TextLine; font: string }

/** One end of a selection: in which line's span, where, in what text. */
interface Mark { key: string; offset: number; text: string }

/** Whether the page has a selection that isn't just a caret, kept up to date as it changes (a
 * `selectionchange` only fires when the reader selects): cheaper than asking on every move. */
let pageSelection = false;
if (typeof document !== "undefined") document.addEventListener("selectionchange", () => { pageSelection = document.getSelection()?.type === "Range"; });

export class TextLayer {
  /** The layer on screen, as the engine described it (a new description equal to it is a no-op). */
  private json = "";
  private spans: HTMLSpanElement[] = [];
  private placed = new WeakMap<HTMLSpanElement, Placed>();
  /** The page font family registered for each engine face (null: none could be). */
  private faceNames = new Map<string, string | null>();

  constructor(private root: ShadowRoot, readonly el: HTMLDivElement, private faceBytes: (id: string) => Uint8Array | undefined) {}

  /** Forget everything (the element left the page, or shows another document). */
  reset() {
    this.json = "";
    this.spans = [];
    this.faceNames.clear();
    this.el.replaceChildren();
    this.el.classList.remove("moving");
  }

  /** Hide the layer while the chart moves (its text is on its way somewhere else) — unless the
   * reader has a selection there, which stays until the chart settles and the layer is rebuilt. */
  hide() {
    // A selection in this layer keeps it (the reader is copying): asked only when the page has
    // one — reading the document's selection forces a layout of the page, tens of ms on a long
    // article, at the start of every transition.
    if (!this.el.classList.contains("moving") && !(pageSelection && this.selectionRange())) this.el.classList.add("moving");
  }

  /** The engine's description of the settled chart: rebuild what changed, show the layer. */
  update(json: string) {
    if (json !== this.json) {
      this.json = json;
      let items: TextItem[] = [];
      try { items = JSON.parse(json); } catch { /* an engine without a text layer */ }
      this.build(items);
    }
    this.el.classList.remove("moving");
  }

  /** A selection being dragged where there is no chart text under the pointer. Over the canvas
   * (which can't be selected) the browser leaves the selection where the pointer last crossed
   * text — a fast drag past a title's end drops its last letters; outside the chart it tries to
   * carry the selection out of the element's shadow tree, which browsers do badly. So for those
   * moves the host calls this (and keeps the page's own text unselectable meanwhile): as in a
   * paragraph, the selection runs to the end (or start) of the text beside the pointer on its
   * line, to the first text above the chart and to the last below it — and stays where it is
   * anywhere else. */
  extendTo(clientX: number, clientY: number) {
    const sel = document.getSelection();
    if (!sel || !this.selectionRange()) return;
    const box = this.el.getBoundingClientRect();
    const x = clientX - box.left, y = clientY - box.top;
    const texts = this.spans.map((s) => s.firstChild as Text | null).filter((t): t is Text => !!t);
    let best: [Text, number] | null = null, gap = Infinity;
    if (y < 0 && texts.length) best = [texts[0], 0];
    else if (y > box.height && texts.length) best = [texts[texts.length - 1], texts[texts.length - 1].length];
    else {
      for (const s of this.spans) {
        const p = this.placed.get(s);
        const t = s.firstChild as Text | null;
        if (!p || !t || p.item.rotate || p.item.drag || y < p.line.y || y > p.line.y + p.line.h) continue;
        const d = x > p.line.x + p.line.w ? x - p.line.x - p.line.w : p.line.x - x;
        if (d >= 0 && d < gap) {
          gap = d;
          best = [t, x > p.line.x ? t.length : 0];
        }
      }
    }
    if (!best) return;
    try {
      sel.extend(best[0], best[1]);
    } catch {
      /* a browser that won't extend into a shadow tree: the selection stays */
    }
  }

  /** The selection, when it lies in this layer: a live range over it. */
  selectionRange(): Range | null {
    const sel = document.getSelection();
    if (!sel || sel.rangeCount === 0) return null;
    let r: AbstractRange | null = null;
    const composed = (sel as any).getComposedRanges;
    if (typeof composed === "function") {
      try { r = composed.call(sel, { shadowRoots: [this.root] })[0] ?? null; } catch {
        try { r = composed.call(sel, this.root)[0] ?? null; } catch { r = null; }
      }
    }
    if (!r) {
      // Before `getComposedRanges`: Chromium's shadow-root selection, else the document's (which
      // Firefox leaves inside shadow trees).
      const own = (this.root as any).getSelection?.() as Selection | null | undefined;
      r = own && own.rangeCount ? own.getRangeAt(0) : sel.getRangeAt(0);
    }
    if (!r || r.collapsed || !this.el.contains(r.startContainer) || !this.el.contains(r.endContainer)) return null;
    const live = document.createRange();
    try {
      live.setStart(r.startContainer, r.startOffset);
      live.setEnd(r.endContainer, r.endOffset);
    } catch {
      return null;
    }
    return live;
  }

  /** The selected text, a line per text in reading order, a wrapped text's lines joined by
   * spaces; null when the selection isn't in this layer. */
  selectedText(): string | null {
    const r = this.selectionRange();
    if (!r) return null;
    let out = "";
    let prev: Placed | null = null;
    for (const s of this.spans) {
      const t = s.firstChild as Text | null;
      const p = this.placed.get(s);
      if (!t || !p || !r.intersectsNode(t)) continue;
      const a = r.startContainer === t ? r.startOffset : 0;
      const b = r.endContainer === t ? r.endOffset : t.length;
      if (b <= a) continue;
      if (prev) out += prev.item === p.item ? (prev.line.br ? "\n" : " ") : "\n";
      out += t.data.slice(a, b);
      prev = p;
    }
    return out || null;
  }

  /** Stretch each span to its line's drawn width (and turn it with its text): the page's font,
   * even the chart's own, never advances exactly like the engine's layout. Again when fonts load. */
  fit() {
    for (const s of this.spans) {
      const p = this.placed.get(s);
      if (!p) continue;
      const natural = measure(p.font, p.line.text);
      const k = natural > 0 ? p.line.w / natural : 1;
      const turn = p.item.rotate ? `rotate(${p.item.rotate}rad) ` : "";
      s.style.transform = `${turn}scaleX(${Math.round(k * 1e4) / 1e4})`;
    }
  }

  private build(items: TextItem[]) {
    const kept = this.saveSelection();
    const nodes: Node[] = [];
    const spans: HTMLSpanElement[] = [];
    for (const item of items) {
      const families = [...item.faces.map((f) => this.pageFace(f)).filter((f): f is string => !!f), JSON.stringify(item.family), "system-ui", "sans-serif"].join(", ");
      item.lines.forEach((line, index) => {
        const s = document.createElement("span");
        s.textContent = line.text;
        // The paragraph's direction, as the engine ordered its runs (a line of a right-to-left
        // paragraph may start with a number or a Latin word).
        s.dir = line.rtl ? "rtl" : "ltr";
        s.dataset.key = `${item.path}#${index}`;
        if (item.role) s.dataset.role = item.role;
        if (item.drag) s.className = "pass";
        const font = `${item.weight} ${item.size}px ${families}`;
        // The line box exactly: its height is the drawn face's ascent + descent, which with the
        // same face in the page puts the page's baseline on the drawn one.
        s.style.cssText = `left:${line.x}px;top:${line.y}px;height:${line.h}px;font:${item.weight} ${item.size}px/${line.h}px ${families}`;
        this.placed.set(s, { item, line, font });
        spans.push(s);
        // A separator after it, in the layer's (sizeless) text flow: a wrap reads as a space, a new
        // text or a line break as a new line — to find-in-page (a search doesn't match across two
        // labels) and in copies the browser makes. (Bare text nodes there aren't laid out at all.)
        const sep = document.createElement("span");
        sep.className = "sep";
        sep.textContent = index === item.lines.length - 1 || line.br ? "\n" : " ";
        nodes.push(s, sep);
      });
    }
    this.el.replaceChildren(...nodes);
    this.spans = spans;
    this.fit();
    if (kept) this.restoreSelection(kept);
  }

  /** The page font family for an engine face: its file registered once per page under a name of
   * our own (a page's own "Inter" is left alone — a chart's subset of it lacks most glyphs). */
  private pageFace(id: string): string | null {
    if (this.faceNames.has(id)) return this.faceNames.get(id)!;
    let family: string | null = null;
    try {
      const bytes = typeof FontFace !== "undefined" ? this.faceBytes(id) : undefined;
      if (bytes && bytes.length) {
        const key = `${id}|${fingerprint(bytes)}`;
        let reg = pageFaces.get(key);
        if (!reg) {
          const name = `datars-face-${pageFaces.size + 1}`;
          // Any weight asked for is this face: never a synthesized bold wider than the drawn one.
          const face = new FontFace(name, bytes as Uint8Array<ArrayBuffer>, { weight: "1 1000", style: "normal" });
          document.fonts.add(face);
          reg = { family: name, loaded: face.loaded };
          pageFaces.set(key, reg);
        }
        family = JSON.stringify(reg.family);
        reg.loaded.then(() => this.fit(), () => undefined);
      }
    } catch {
      family = null;
    }
    this.faceNames.set(id, family);
    return family;
  }

  /** The selection's ends as span keys, offsets and the texts they're in, to put back over
   * rebuilt spans. */
  private saveSelection(): Mark[] | null {
    const r = this.selectionRange();
    if (!r) return null;
    const at = (n: Node, offset: number): Mark | null => {
      const key = n.nodeType === Node.TEXT_NODE ? (n.parentElement as HTMLElement | null)?.dataset?.key : undefined;
      return key ? { key, offset, text: (n as Text).data } : null;
    };
    const a = at(r.startContainer, r.startOffset), b = at(r.endContainer, r.endOffset);
    return a && b ? [a, b] : null;
  }

  /** The selection again, where the same texts are still drawn (moved, maybe); gone if they
   * changed — half a word of a new title isn't what the reader selected. */
  private restoreSelection([a, b]: Mark[]) {
    const find = (m: Mark) => {
      const t = this.spans.find((s) => s.dataset.key === m.key)?.firstChild as Text | undefined;
      return t && t.data === m.text ? t : undefined;
    };
    const ta = find(a), tb = find(b);
    try {
      if (ta && tb) document.getSelection()?.setBaseAndExtent(ta, a.offset, tb, b.offset);
      else document.getSelection()?.removeAllRanges();
    } catch {
      /* the selection goes */
    }
  }
}
