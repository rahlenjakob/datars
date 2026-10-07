// The features overview (/features/): twelve published charts, each driven by this page through
// the public `<datars-view>` API — `send("goto:…")` for the stories, `provideData()` for the live
// feed, `setTokens()` for the brands, `setSignal()` for selection and focus. A conductor plays one
// card at a time among those on screen (and the one under the pointer at once), so the page never
// animates more than a chart or two in a frame: a chart draws only while it changes.

const $ = (sel, el = document) => el.querySelector(sel);
const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
const SITE = new URL(".", import.meta.url);
const cards = [...document.querySelectorAll(".fx-card")];
const live = (card, text) => { const c = $(".fx-live code", card); if (c.textContent !== text) c.textContent = text; };

// ---- colour easing for the brands card (the engine snaps colour changes; see /features/theming/) --

function rgb(hex) { const n = parseInt(hex.slice(1, 7), 16); return [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((v) => v / 255); }
const lin = (c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
const gam = (c) => (c <= 0.0031308 ? c * 12.92 : 1.055 * c ** (1 / 2.4) - 0.055);
function lab(hex) {
  const [r, g, b] = rgb(hex).map(lin);
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b), m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b), s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return [0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s, 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s, 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s];
}
function unlab([L, a, b]) {
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3, m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3, s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return `#${[4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s, -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s, -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s]
    .map((v) => Math.round(Math.min(1, Math.max(0, gam(v))) * 255).toString(16).padStart(2, "0")).join("")}`;
}
const isHex = (v) => typeof v === "string" && /^#[0-9a-f]{6}$/i.test(v);
const mix = (a, b, t) => { const [p, q] = [lab(a), lab(b)]; return unlab(p.map((v, i) => v + (q[i] - v) * t)); };
function inks(from, to, t) {
  const out = {};
  for (const [k, b] of Object.entries(to)) {
    const a = from[k];
    if (isHex(a) && isHex(b) && a !== b) out[k] = mix(a, b, t);
    else if (Array.isArray(a) && Array.isArray(b) && b.length && b.every(isHex) && a.every(isHex)) {
      const colors = b.map((c, i) => mix(a[i % a.length], c, t));
      out[k] = k === "categorical" ? colors : { kind: k === "diverging" ? "diverging" : "sequential", colors };
    }
  }
  return out;
}

const face = (family, weight, file) => ({ family: [family], weight, src: new URL(`fonts/${file}.ttf`, SITE).href });
const faces = new Map(), got = new Map();
function loadFace(url) {
  if (!faces.has(url)) {
    const p = fetch(url).then((r) => (r.ok ? r.arrayBuffer() : Promise.reject(new Error(String(r.status))))).then((b) => new Uint8Array(b));
    p.then((b) => got.set(url, b), () => faces.delete(url));
    faces.set(url, p);
  }
  return faces.get(url);
}
const isFont = (v) => v && typeof v === "object" && !Array.isArray(v) && "family" in v;
const type = (f) => ({ "font.title": f(700), "font.strong": f(700), "font.body": f(400), "font.number": f(400) });
const manrope = (w) => face("Manrope", w, w >= 600 ? "Manrope-Bold" : "Manrope-Regular");
const news = (w) => face("Newsreader", w, w >= 600 ? "Newsreader-SemiBold" : "Newsreader-Regular");
const mono = (w) => face("Space Mono", w, w >= 600 ? "SpaceMono-Bold" : "SpaceMono-Regular");
/** Fictional brands (the theming page has the whole set), light and dark. */
const BRANDS = [
  { name: "datars", light: {}, dark: {} },
  { name: "Larkspur Health", shape: { ...type(manrope), "radius.bar": 7, "size.title": 16 },
    light: { paper: "#f1f6f5", ink: "#12343a", categorical: ["#0f766e", "#6cb8d0", "#e8a33d", "#8497a4"] },
    dark: { paper: "#0f1e21", ink: "#e4f0ee", categorical: ["#4fd1c1", "#7cc7e3", "#f0b457", "#9fb2be"] } },
  { name: "The Daily Quill", shape: { ...type(news), "radius.bar": 0, "size.title": 19, "band.padding": 0.12 },
    light: { paper: "#f4eee2", ink: "#1b1916", categorical: ["#b3261e", "#1d3f6e", "#c9922e", "#4d6b3c"] },
    dark: { paper: "#191714", ink: "#ece6d8", categorical: ["#e2574c", "#7fa6d9", "#e3b14f", "#8fb074"] } },
  { name: "Neonfall", night: true, shape: { ...type(mono), "radius.bar": 0, "size.title": 15 },
    dark: { paper: "#0b0620", ink: "#f3ecff", categorical: ["#ff2bd6", "#22e5ff", "#a3ff3d", "#ffd23f"] } },
];
const pageMode = () => (document.documentElement.dataset.theme === "dark" ? "dark" : "light");
function brandLayer(b) {
  const m = pageMode();
  return { mode: b.night ? "dark" : m, tokens: { ...(b.shape ?? {}), ...((b.night || m === "dark") ? b.dark : b.light) } };
}

// ---- per card: what one beat does --------------------------------------------------------------

/** Story cards: the next state, by `send("goto:…")`. */
function story(text) {
  return {
    init(card, view) {
      view.addEventListener("state", (e) => { card.states = e.detail?.states ?? card.states; card.index = e.detail?.index ?? 0; });
    },
    beat(card, view) {
      const s = card.states ?? [];
      if (s.length < 2) return;
      const next = s[((card.index ?? 0) + 1) % s.length];
      view.send(`goto:${next}`);
      live(card, text ? text(next) : `view.send("goto:${next}")`);
    },
  };
}
const ROWS = (n) => Array.from({ length: 20 }, (_, i) => { const t = n - 19 + i; return { t, v: Math.round(60 + 18 * Math.sin(t / 3) + 6 * Math.sin(t * 1.7)) }; });
const DEV = { bar: "bar()", line: "line({ points: true })", area: "area()" };

const ACTS = {
  charts: story((s) => ({ bars: "plot({ children: [bar()] })", donut: "pie({ inner: 0.55 })", treemap: "treemap()" })[s]),
  data: {
    beat(card, view) {
      card.n = (card.n ?? 20) + 1;
      view.provideData("feed", ROWS(card.n));
      live(card, `view.provideData("feed", rows) // reading ${card.n}`);
    },
  },
  theming: {
    init(card, view) {
      card.brand = 0;
      new MutationObserver(() => this.apply(card, view, false)).observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    },
    beat(card, view) {
      card.brand = (card.brand + 1) % BRANDS.length;
      this.apply(card, view, true);
    },
    async apply(card, view, animate) {
      const b = BRANDS[card.brand], { mode, tokens } = brandLayer(b);
      await Promise.all(Object.values(tokens).filter(isFont).map((v) => loadFace(v.src).catch(() => {})));
      const from = animate && !reduced ? view.status?.tokens?.tokens : null;
      view.setAttribute("mode", mode);
      view.setTokens(tokens);
      for (const v of Object.values(tokens)) {
        if (!isFont(v) || !got.has(v.src) || card.has?.has(v.src)) continue;
        try { view.provideData(`font:${v.family[0]}-${v.weight}`, got.get(v.src)); } catch { /* not asked for */ }
        (card.has ??= new Set()).add(v.src);
      }
      live(card, `view.setTokens({ … }) // ${b.name}, ${Object.keys(tokens).length} tokens`);
      const to = from && view.status?.tokens?.tokens;
      if (!from || !to) return;
      // Ease the colours in OKLab, one token layer a frame (the first in this task).
      const run = (card.run = (card.run ?? 0) + 1);
      view.setTokens({ ...tokens, ...inks(from, to, 0) });
      const t0 = performance.now();
      const step = (now) => {
        if (card.run !== run) return;
        const t = Math.min(1, (now - t0) / 450), e = t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2;
        view.setTokens(t >= 1 ? tokens : { ...tokens, ...inks(from, to, e) });
        if (t < 1) requestAnimationFrame(step);
      };
      requestAnimationFrame(step);
    },
  },
  "big-data": story((s) => `view.send("goto:${s}") // 60,000 rows, 9,000 drawn`),
  platforms: story((s) => `view.send("goto:${s}") // same document, re-laid out`),
  maps: story((s) => `view.send("goto:${s}") // the camera flies`),
  animation: story((s) => `view.send("goto:${s}") // keyed (party, seat)`),
  interaction: {
    beat(card, view) {
      const order = [["Oslo"], ["Lisbon"], ["Cairo"], []];
      card.i = ((card.i ?? -1) + 1) % order.length;
      view.setSignal("pick", order[card.i]);
      live(card, `view.setSignal("pick", ${JSON.stringify(order[card.i])})`);
    },
  },
  accessibility: {
    beat(card, view) {
      const days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
      card.i = ((card.i ?? -1) + 1) % (days.length + 1);
      const day = days[card.i];
      view.setSignal("focus", day ? [day] : []);
      // What a screen reader says for the focused bar: the label the engine gave the mark.
      const items = day ? view.status?.semantics ?? [] : [];
      const label = day && findLabel(items, day);
      live(card, day ? `Screen reader: “${label ?? day}”` : "Screen reader: “Bikes rented by weekday, bar chart”");
    },
  },
  extensibility: story((s) => (s === "rose" ? "repeat(…, shape(geom.arc({ … })))" : "plot({ children: [bar()] })")),
  delivery: story((s) => (s === "v2" ? "republished: every embed shows v2" : "datars publish doc.ts --alias count")),
  developers: story((s) => `plot({ …, children: [${DEV[s] ?? s}] })`),
};
/** The first semantics item labelled for `day` (depth first). */
function findLabel(items, day) {
  for (const it of items) {
    if (typeof it?.label === "string" && it.label.startsWith(`${day}`) && it.role === "datum") return it.label;
    const inner = it?.children && findLabel(it.children, day);
    if (inner) return inner;
  }
  return null;
}

// ---- the conductor -----------------------------------------------------------------------------

const BEAT = 2400;
const visible = new Set();
let last = -1, lastAt = 0;
const io = new IntersectionObserver((entries) => {
  for (const e of entries) e.isIntersecting ? visible.add(e.target) : visible.delete(e.target);
}, { threshold: 0.55 });

function ready(card) { return card.view?.dataset.renderer && card.inited; }
function play(card) {
  const act = ACTS[card.dataset.fx];
  if (!act || !ready(card)) return;
  card.playedAt = performance.now();
  act.beat(card, card.view);
  for (const c of cards) c.classList.toggle("playing", c === card);
  clearTimeout(card.glow);
  card.glow = setTimeout(() => card.classList.remove("playing"), 1600);
}

for (const card of cards) {
  io.observe(card);
  const slot = $(".chart", card);
  slot.addEventListener("chartmount", (e) => {
    const view = e.detail.view;
    card.view = view;
    view.setAttribute("no-controls", "");
    const act = ACTS[card.dataset.fx];
    act?.init?.(card, view);
    card.inited = true;
  });
  // The card under the pointer plays now (and the conductor moves on from it).
  card.addEventListener("pointerenter", () => {
    if (performance.now() - (card.playedAt ?? 0) < 1300) return;
    play(card);
    last = cards.indexOf(card);
    lastAt = performance.now();
  });
  card.addEventListener("focusin", () => { if (performance.now() - (card.playedAt ?? 0) > 1300) play(card); });
}

// The brands card is this page's to style (the reader's own look leaves it alone): it may mount now.
for (const el of document.querySelectorAll(".fx [data-own-look]")) el.dataset.ownLook = "ready";
document.dispatchEvent(new Event("ownlookready"));

// One card at a time, in reading order among those on screen, never under reduced motion.
if (!reduced) {
  setInterval(() => {
    if (document.hidden || performance.now() - lastAt < BEAT - 50) return;
    const on = cards.filter((c) => visible.has(c) && ready(c));
    if (!on.length) return;
    const next = on.find((c) => cards.indexOf(c) > last) ?? on[0];
    last = cards.indexOf(next);
    lastAt = performance.now();
    play(next);
  }, 200);
}
