// The site's pages: mounts each chart when it scrolls near (a <datars-view> per slot, sized to the
// document's aspect ratio, in the space the page reserved for it), and wires the page's controls to
// the runtime's public API — `send("goto:<state>")`, the `state` event, `setAttribute("mode", …)`
// and `setTokens(…)` (the reader's look: "Make it yours"). Also the menus (disclosures that close
// on Escape and outside clicks). Charts inside `[data-own-look]` (the theme studio) are left to the
// page's own script: each slot fires `chartmount` with its view before the view starts.

const root = document.documentElement;
const views = [];

// ---- light / dark ----------------------------------------------------------------------------
function storedTheme() {
  try { return localStorage.getItem("datars-theme"); } catch { return null; }
}
root.dataset.theme = storedTheme() ?? (matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light");

function setPageTheme(mode) {
  root.dataset.theme = mode;
  try { localStorage.setItem("datars-theme", mode); } catch { /* private mode */ }
  for (const { slot, view } of views) {
    if (slot.dataset.mode) continue; // the hero always plays dark (the studio's charts: its mode)
    view.setAttribute("mode", viewModeOf(slot));
    if (!isPlain() && !ownLook(slot)) view.setTokens(lookTokens(mode)); // the look has a palette per mode
  }
  showLook();
}
document.getElementById("theme-toggle")?.addEventListener("click", () => setPageTheme(root.dataset.theme === "dark" ? "light" : "dark"));

// ---- menus -----------------------------------------------------------------------------------
// The Features dropdown, the phone menu and the docs' phone menu are <details>: they work without
// script; this closes them on Escape, on a click elsewhere and after following a link.
const menus = [...document.querySelectorAll("details.dd, details.burger")];
document.addEventListener("click", (e) => {
  for (const m of menus) if (m.open && !m.contains(e.target)) m.open = false;
});
document.addEventListener("keydown", (e) => {
  if (e.key !== "Escape") return;
  for (const m of menus) if (m.open) { m.open = false; m.querySelector("summary")?.focus(); }
});
for (const m of menus) {
  m.addEventListener("toggle", () => { if (m.open) for (const o of menus) if (o !== m) o.open = false; });
  for (const a of m.querySelectorAll("a")) a.addEventListener("click", () => { m.open = false; });
}

// ---- chart slots -----------------------------------------------------------------------------
function heightFor(slot, width) {
  // `data-phone-ratio`: charts that re-lay themselves out as rows on a narrow box need the height.
  const phone = width < 600 && Number(slot.dataset.phoneRatio);
  const aspect = phone || Number(slot.dataset.ratio || slot.dataset.aspect) || 0.6;
  const min = Number(slot.dataset.min) || 240;
  return Math.max(min, Math.round(width * aspect));
}

/** Slots a page styles itself, near before its script is there to style them: they wait for it
 * (`data-own-look="ready"` and an `ownlookready` event), so they never open in the wrong look. */
const unstyled = [];
document.addEventListener("ownlookready", () => unstyled.splice(0).forEach(mount));
function mount(slot) {
  const own = slot.closest("[data-own-look]");
  if (own && own.dataset.ownLook !== "ready") return void unstyled.push(slot);
  const view = document.createElement("datars-view");
  view.setAttribute("src", slot.dataset.src);
  if (slot.dataset.steps) view.setAttribute("steps", slot.dataset.steps);
  // `data-scrubbed`: the page's scroll scrubs the story — through the nearest `[data-scrub]` box's
  // passage through the viewport (the runtime's `scrub` attribute).
  if ("scrubbed" in slot.dataset) view.setAttribute("scrub", "");
  view.setAttribute("mode", viewModeOf(slot));
  lookAtMount(view, slot);
  const style = getComputedStyle(slot);
  const inner = slot.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight);
  view.setAttribute("height", String(heightFor(slot, inner)));
  view.setAttribute("aria-label", slot.dataset.label ?? slot.dataset.chart);
  slot.classList.add("pending");
  // The page's own script may style it first (the theme studio): before it starts, so its first
  // frame is already right.
  slot.dispatchEvent(new CustomEvent("chartmount", { bubbles: true, detail: { view } }));
  slot.appendChild(view);
  views.push({ slot, view });
  // Keep the aspect ratio as the column resizes (the view re-lays itself out on resize).
  new ResizeObserver(() => {
    const h = String(heightFor(slot, view.clientWidth));
    if (view.getAttribute("height") !== h) view.setAttribute("height", h);
  }).observe(view);
  // The poster shows at once; drop the shimmer when the engine has taken over the canvas.
  const ready = () => {
    if (view.dataset.renderer) slot.classList.remove("pending");
    else setTimeout(ready, 150);
  };
  ready();
  if ("caption" in slot.dataset) caption(slot, view);
  if (slot.dataset.autoplay) autoplay(slot, view, Number(slot.dataset.autoplay));
  const hook = hooks[slot.dataset.chart];
  if (hook) hook(view);
}

/** A story's steps as pills under its chart (click to go there), and the step's narration beside
 * them — instead of the view's own arrows and overlay card. The build writes the bar (the steps
 * and the first step's narration) next to the slot, so it takes its space before the chart mounts;
 * without one, it's made here. */
function caption(slot, view) {
  if (!slot || slot.classList.contains("captioned")) return;
  slot.classList.add("captioned");
  let bar = slot.nextElementSibling?.matches(".caption") ? slot.nextElementSibling : null;
  if (!bar) {
    bar = document.createElement("div");
    bar.className = "caption";
    bar.innerHTML = '<div class="pills small" role="group" aria-label="Steps"></div><p aria-hidden="true"></p>';
    slot.after(bar);
  }
  const pills = bar.querySelector(".pills");
  const text = bar.querySelector("p");
  const wire = (b, name) => b.addEventListener("click", () => view.send(`goto:${name}`));
  for (const b of pills.querySelectorAll("button[data-state]")) wire(b, b.dataset.state);
  view.addEventListener("state", (e) => {
    const s = e.detail ?? {};
    const states = s.states ?? [];
    if (states.length < 2) return;
    if (pills.childElementCount !== states.length) {
      pills.replaceChildren(...states.map((name) => {
        const b = document.createElement("button");
        b.type = "button";
        b.textContent = name.replace(/^./, (c) => c.toUpperCase());
        wire(b, name);
        return b;
      }));
    }
    [...pills.children].forEach((b, i) => b.setAttribute("aria-pressed", String(i === s.index)));
    // A chart that draws its narration itself (a std `card`) doesn't get it repeated underneath.
    const n = s.narrationDrawn ? {} : s.narration ?? {};
    const next = [n.title, n.text].filter(Boolean).join(" — ");
    if (text.textContent === next) return;
    text.replaceChildren();
    if (n.title) text.appendChild(Object.assign(document.createElement("b"), { textContent: n.title }));
    if (n.text) text.appendChild(document.createTextNode(`${n.title ? " — " : ""}${n.text}`));
  });
}

/** `data-autoplay="6000"`: the chart steps through its states by itself, one every so many ms,
 * while most of it is on screen — until the reader picks a step, touches the chart, or asked for
 * reduced motion. A step is held for the full time from when it starts or comes into view. */
function autoplay(slot, view, ms) {
  if (!ms || matchMedia("(prefers-reduced-motion: reduce)").matches) return;
  let states = [], index = 0, since = 0, visible = false, auto = true;
  const stop = () => { auto = false; };
  view.addEventListener("state", (e) => {
    states = e.detail?.states ?? states;
    index = e.detail?.index ?? index;
    since = performance.now();
  });
  view.addEventListener("pointerdown", stop);
  view.addEventListener("keydown", stop);
  slot.nextElementSibling?.matches(".caption") && slot.nextElementSibling.addEventListener("click", stop);
  new IntersectionObserver(([e]) => {
    visible = e.isIntersecting;
    if (visible) since = performance.now();
  }, { threshold: 0.6 }).observe(view);
  const timer = setInterval(() => {
    if (!auto) return clearInterval(timer);
    if (visible && !document.hidden && states.length > 1 && performance.now() - since >= ms) {
      since = performance.now();
      view.send(`goto:${states[(index + 1) % states.length]}`);
    }
  }, 250);
}

const near = new IntersectionObserver((entries) => {
  for (const e of entries) {
    if (!e.isIntersecting) continue;
    near.unobserve(e.target);
    mount(e.target);
  }
  // A screen and a half ahead (as @datars/react's <DatarsView>): a chart starts while it's still
  // off screen, so it's drawn by the time the reader gets there. 700 px was under a second ahead
  // at a reading scroll, less than a start on a slow connection.
}, { rootMargin: "150% 0px" });
document.querySelectorAll(".chart[data-src]").forEach((s) => near.observe(s));

// ---- per-chart wiring ------------------------------------------------------------------------
const hooks = {
  hero(view) {
    const pills = [...document.querySelectorAll("#hero-pills button")];
    if (!pills.length) return caption(view.closest(".chart"), view);
    view.addEventListener("state", (e) => {
      for (const p of pills) p.setAttribute("aria-pressed", String(p.dataset.state === e.detail?.state));
    });
    for (const p of pills) p.addEventListener("click", () => view.send(`goto:${p.dataset.state}`));
  },
  // Plays through its layouts while on screen, until the reader picks one.
  worlds(view) {
    const pills = [...document.querySelectorAll("#worlds-pills button")];
    if (!pills.length) return caption(view.closest(".chart"), view);
    const order = pills.map((p) => p.dataset.state);
    // Plays through its layouts while on screen, until the reader picks one (never for reduced motion).
    let current = order[0], auto = !matchMedia("(prefers-reduced-motion: reduce)").matches, visible = false;
    view.addEventListener("state", (e) => {
      current = e.detail?.state ?? current;
      for (const p of pills) p.setAttribute("aria-pressed", String(p.dataset.state === current));
    });
    for (const p of pills) p.addEventListener("click", () => { auto = false; view.send(`goto:${p.dataset.state}`); });
    new IntersectionObserver(([e]) => { visible = e.isIntersecting; }, { threshold: 0.6 }).observe(view);
    setInterval(() => {
      if (auto && visible && !document.hidden) view.send(`goto:${order[(order.indexOf(current) + 1) % order.length]}`);
    }, 5200);
  },
  // Four million stars: a tour by pills, and what the engine is doing, live — stars drawn this
  // frame, archive bytes fetched so far, and the page's frame rate while the reader explores or a
  // flight plays (the page's own animation frames, so it counts all the engine's work).
  galaxy(view) {
    const pills = [...document.querySelectorAll("#galaxy-pills button")];
    const text = document.getElementById("galaxy-text");
    if (!pills.length || !text) return caption(view.closest(".chart"), view);
    const drawn = document.getElementById("galaxy-drawn");
    const bytes = document.getElementById("galaxy-bytes");
    const fps = document.getElementById("galaxy-fps");
    let activeUntil = 0;
    const active = (ms) => { activeUntil = Math.max(activeUntil, performance.now() + ms); };
    view.addEventListener("state", (e) => {
      const s = e.detail ?? {};
      for (const p of pills) p.setAttribute("aria-pressed", String(p.dataset.state === s.state));
      const n = s.narration ?? {};
      text.replaceChildren();
      if (n.title) text.appendChild(Object.assign(document.createElement("b"), { textContent: n.title }));
      if (n.text) text.appendChild(document.createTextNode(`${n.title ? " — " : ""}${n.text}`));
    });
    for (const p of pills) p.addEventListener("click", () => { active(3200); view.send(`goto:${p.dataset.state}`); });
    view.addEventListener("pointerdown", () => active(1500));
    view.addEventListener("pointermove", (e) => { if (e.buttons) active(600); });
    view.addEventListener("wheel", () => active(600), { passive: true });
    const size = (n) => (n < 1048576 ? `${Math.max(1, Math.round(n / 1024))} KB` : `${(n / 1048576).toFixed(1)} MB`);
    let visible = false, last = 0, deltas = [], shown = 0;
    new IntersectionObserver(([e]) => { visible = e.isIntersecting; if (visible) requestAnimationFrame(tick); }, { threshold: 0 }).observe(view);
    // A readout, not an animation: written a few times a second and only when it changes. Text
    // replaced every frame (even with the same text) made the browser lay the bar out again on
    // every frame of a flight — dropped frames in Safari, on a phone most of all.
    const put = (el, t) => { if (el.textContent !== t) el.textContent = t; };
    function tick(now) {
      if (!visible) return;
      if (now < activeUntil && last) {
        deltas.push(now - last);
        if (deltas.length > 40) deltas.shift();
      }
      if (now - shown >= 250) {
        shown = now;
        const st = view.stats;
        if (st) {
          put(drawn, st.drawn.toLocaleString("en"));
          put(bytes, size(st.bytes));
        }
        if (now < activeUntil && deltas.length >= 10) {
          const sorted = [...deltas].sort((a, b) => a - b);
          put(fps, String(Math.min(120, Math.round(1000 / sorted[sorted.length >> 1]))));
        }
      }
      last = now;
      requestAnimationFrame(tick);
    }
  },
};

// The scroll story: the runtime moves the chart (`steps=".rstep"`); the page lights the step.
const stepIO = new IntersectionObserver((entries) => {
  for (const e of entries) e.target.classList.toggle("active", e.isIntersecting);
}, { rootMargin: "-45% 0px -45% 0px" });
document.querySelectorAll(".rstep").forEach((s) => stepIO.observe(s));

// ---- make it yours ------------------------------------------------------------------------------
// The reader's look for every chart on the site (articles keep theirs): a preset brand — its type,
// corners, strokes and a palette for light and for dark — and their own accent, palette, type,
// corners and lines on top. Kept in localStorage; applied with `setTokens` to each chart as it
// mounts (before its first frame) and to every chart on the page when it changes.
const face = (family, weight, file) => ({ family: [family], weight, src: `/fonts/${file}.ttf` });
const news = (w) => face("Newsreader", w, w >= 600 ? "Newsreader-SemiBold" : "Newsreader-Regular");
const manrope = (w) => face("Manrope", w, w >= 600 ? "Manrope-Bold" : "Manrope-Regular");
const mono = (w) => face("Space Mono", w, w >= 600 ? "SpaceMono-Bold" : "SpaceMono-Regular");
const inter = (w, file) => ({ family: ["Inter"], weight: w, src: `datars:fonts/${file}.ttf` });
const seq = (colors) => ({ kind: "sequential", colors });
const typeSet = (f) => ({ "font.title": f(700), "font.strong": f(700), "font.body": f(400), "font.number": f(400) });
export const BRANDS = {
  default: { name: "Default", shape: {}, light: {}, dark: {} },
  newsprint: {
    name: "Newsprint",
    shape: { "font.title": news(600), "font.strong": news(600), "font.body": news(400), "font.number": news(400),
      "size.title": 21, "size.label": 12.5, "size.body": 15, "stroke.line": 1.6, "point.radius": 2.6, "radius.bar": 0, "radius.card": 0 },
    light: { paper: "#f4eee2", ink: "#1b1916", accent: "#b3261e", categorical: ["#b3261e", "#1d3f6e", "#c9922e", "#4d6b3c", "#6e4a7e", "#2f7f7a"],
      sequential: seq(["#f1e2d0", "#e4b89d", "#d58566", "#bf5540", "#962a1f"]),
      "map.water": "#dcd6c8", "map.land": "#ebe4d5", "map.no-data": "#e6dfcf", "map.border": "#f4eee2", card: "#fbf7ee", "card-line": "#1b1916" },
    dark: { paper: "#191714", ink: "#ece6d8", accent: "#e2574c", categorical: ["#e2574c", "#7fa6d9", "#e3b14f", "#8fb074", "#b08cc2", "#5fb8b1"],
      sequential: seq(["#3a2420", "#6b3128", "#9c3f30", "#cf5a44", "#f08c6e"]),
      "map.water": "#211f1b", "map.land": "#2c2924", "map.no-data": "#2c2924", "map.border": "#191714", card: "#211e1a", "card-line": "#6d665a" },
  },
  nordic: {
    name: "Nordic",
    shape: { ...typeSet(manrope), "size.title": 18, "stroke.line": 2.8, "point.radius": 4.5, "radius.bar": 7, "radius.card": 18 },
    light: { paper: "#f1f5f4", ink: "#12343a", accent: "#0f766e", categorical: ["#0f766e", "#6cb8d0", "#e8a33d", "#8497a4", "#a6cf7d", "#d192be"],
      sequential: seq(["#dcefec", "#a5d6ce", "#5fb3a7", "#238478", "#0b534c"]),
      "map.water": "#d8e7ea", "map.land": "#fbfdfc", "map.no-data": "#e7eeed", "map.border": "#f1f5f4", card: "#ffffff", "card-line": "#d4e2df" },
    dark: { paper: "#0f1e21", ink: "#e4f0ee", accent: "#4fd1c1", categorical: ["#4fd1c1", "#7cc7e3", "#f0b457", "#9fb2be", "#b9df91", "#e0a6cf"],
      sequential: seq(["#15393a", "#1c5a57", "#23807a", "#3fb0a3", "#8ee3d6"]),
      "map.water": "#0b1719", "map.land": "#1a2d30", "map.no-data": "#1a2d30", "map.border": "#0f1e21", card: "#152a2d", "card-line": "#24433f" },
  },
  neon: {
    name: "Neon",
    shape: { ...typeSet(mono), "size.title": 17, "size.label": 10.5, "stroke.line": 3.2, "point.radius": 5, "radius.bar": 0, "radius.card": 2 },
    light: null, // dark-first: the night palette in every mode
    dark: { paper: "#0b0620", ink: "#f3ecff", accent: "#ff2bd6", categorical: ["#ff2bd6", "#22e5ff", "#a3ff3d", "#ffd23f", "#9d6bff", "#ff7a2f"],
      sequential: seq(["#24104a", "#4f1a86", "#9a1cb0", "#e7409f", "#ffc2ec"]),
      "map.water": "#0f0a2a", "map.land": "#1b1340", "map.no-data": "#1b1340", "map.border": "#3b2b74", card: "#140c34", "card-line": "#ff2bd6" },
  },
  sunset: {
    name: "Sunset",
    shape: { "font.title": face("Fraunces", 700, "Fraunces-Bold"), "size.title": 21, "stroke.line": 2.4, "point.radius": 3.5, "radius.bar": 4, "radius.card": 14 },
    light: { paper: "#fff4ea", ink: "#3a1f14", accent: "#e8590c", categorical: ["#e8590c", "#c2185b", "#6a3fb5", "#f2a541", "#2a9d8f", "#8d6e63"],
      sequential: seq(["#fde4cf", "#f8b889", "#f1874c", "#d6511d", "#992d0e"]),
      "map.water": "#fadfca", "map.land": "#fffaf5", "map.no-data": "#f5e6d8", "map.border": "#fff4ea", card: "#ffffff", "card-line": "#f0d0b8" },
    dark: { paper: "#1f1310", ink: "#fbe9dd", accent: "#ff8a3d", categorical: ["#ff8a3d", "#f06292", "#a78bfa", "#ffc36b", "#4fc3b3", "#bcaaa4"],
      sequential: seq(["#3d1d12", "#6e2e16", "#a6441c", "#e0692c", "#ffa66b"]),
      "map.water": "#29170f", "map.land": "#35221a", "map.no-data": "#35221a", "map.border": "#1f1310", card: "#2a1913", "card-line": "#4d3024" },
  },
};
/** The built-in theme's colours the reader's choices start from (datars/neutral). */
const NEUTRAL = { light: { paper: "#ffffff", ink: "#1d1f24", accent: "#4269d0" }, dark: { paper: "#111318", ink: "#eceef2", accent: "#7a9cf0" },
  categorical: ["#4269d0", "#efb118", "#ff725c", "#6cc5b0", "#3ca951", "#9c6b4e", "#a463f2", "#97bbf5", "#ff8ab7", "#9498a0"] };
const ACCENTS = ["#4269d0", "#0f766e", "#2f8f5b", "#e8a33d", "#e8590c", "#c8423b", "#c2185b", "#6a3fb5"];
const PALETTES = {
  preset: { name: "Preset's" },
  safe: { name: "Colour-blind safe", colors: ["#0072b2", "#e69f00", "#009e73", "#cc79a7", "#56b4e9", "#d55e00", "#f0e442"] },
  warm: { name: "Warm", colors: ["#e8590c", "#c2185b", "#f2a541", "#8d6e63", "#6a3fb5", "#2a9d8f"] },
  cool: { name: "Cool", colors: ["#0f766e", "#4269d0", "#6cb8d0", "#8497a4", "#a6cf7d", "#6a3fb5"] },
  shades: { name: "Shades of the accent" },
};
const TYPES = {
  inter: { name: "Inter", tokens: { "font.title": inter(700, "Inter-Bold"), "font.strong": inter(600, "Inter-SemiBold"), "font.body": inter(400, "Inter-Regular"), "font.number": inter(400, "Inter-Regular") } },
  newsreader: { name: "Newsreader", tokens: { "font.title": news(600), "font.strong": news(600), "font.body": news(400), "font.number": news(400) } },
  manrope: { name: "Manrope", tokens: typeSet(manrope) },
  mono: { name: "Space Mono", tokens: typeSet(mono) },
};
// `custom`: a whole theme from the theme studio (/themes/) instead of a preset and choices —
// `{ name, modes: { light|dark|high-contrast: { mode, tokens } }, swatch: { <mode>: [a, b, c] } }`,
// a view mode and token layer per page mode (a dark-first theme plays dark in light mode).
const PLAIN = { preset: "default", accent: null, palette: "preset", type: null, corners: null, lines: null, custom: null };
const LOOK_KEY = "datars-look";

function storedLook() {
  try {
    const l = JSON.parse(localStorage.getItem(LOOK_KEY) ?? "null");
    return l && typeof l === "object" ? { ...PLAIN, ...l, preset: BRANDS[l.preset] ? l.preset : "default" } : { ...PLAIN };
  } catch { return { ...PLAIN }; }
}
let look = storedLook();
const isPlain = () => Object.keys(PLAIN).every((k) => look[k] === PLAIN[k]);

// Colour arithmetic, just enough to keep a chosen accent readable in either mode.
const rgb = (hex) => { const n = parseInt(hex.slice(1), 16); return [(n >> 16) & 255, (n >> 8) & 255, n & 255]; };
const hexOf = ([r, g, b]) => `#${[r, g, b].map((v) => Math.round(v).toString(16).padStart(2, "0")).join("")}`;
const mix = (a, b, t) => hexOf(rgb(a).map((v, i) => v + (rgb(b)[i] - v) * t));
const lum = (hex) => { const [r, g, b] = rgb(hex).map((v) => { v /= 255; return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; }); return 0.2126 * r + 0.7152 * g + 0.0722 * b; };
const contrast = (a, b) => { const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p); return (x + 0.05) / (y + 0.05); };
/** The accent, lightened on a dark paper (darkened on a light one) until it stands out: 3 : 1. */
function readable(accent, paper) {
  const toward = lum(paper) < 0.2 ? "#ffffff" : "#000000";
  let c = accent;
  for (let i = 1; i <= 8 && contrast(c, paper) < 3; i++) c = mix(accent, toward, i * 0.1);
  return c;
}
const alike = (a, b) => rgb(a).reduce((d, v, i) => d + Math.abs(v - rgb(b)[i]), 0) < 90;

/** The tokens the reader's look sets for a chart in `mode`. */
function lookTokens(mode) {
  if (look.custom) return { ...(look.custom.modes?.[mode]?.tokens ?? {}) };
  const b = BRANDS[look.preset] ?? BRANDS.default;
  const colours = mode === "high-contrast" ? null : (mode === "dark" ? b.dark : b.light ?? b.dark);
  const t = { ...b.shape, ...(colours ?? {}) };
  if (colours) {
    const night = mode === "dark" || !b.light;
    const paper = colours.paper ?? NEUTRAL[night ? "dark" : "light"].paper;
    const accent = look.accent ? readable(look.accent, paper) : colours.accent ?? NEUTRAL[night ? "dark" : "light"].accent;
    if (look.accent) t.accent = accent;
    const base = colours.categorical ?? NEUTRAL.categorical;
    let pal = null;
    if (look.palette === "shades") {
      const ink = colours.ink ?? NEUTRAL[night ? "dark" : "light"].ink;
      pal = [accent, mix(accent, paper, 0.45), mix(accent, ink, 0.4), mix(accent, paper, 0.7), mix(accent, ink, 0.65), mix(accent, paper, 0.85)];
    } else if (PALETTES[look.palette]?.colors) pal = PALETTES[look.palette].colors.map((c) => (night ? readable(c, paper) : c));
    else if (look.accent) pal = [accent, ...base.slice(1).filter((c) => !alike(c, accent))]; // the preset's, led by the accent
    if (pal) t.categorical = pal;
  }
  if (look.type && TYPES[look.type]) Object.assign(t, TYPES[look.type].tokens);
  if (look.corners != null) Object.assign(t, { "radius.bar": look.corners, "radius.mark": Math.min(look.corners, 4), "radius.card": look.corners ? Math.max(6, Math.round(look.corners * 1.6)) : 0 });
  if (look.lines != null) Object.assign(t, { "stroke.line": look.lines, "point.radius": Math.round((1.6 + look.lines) * 10) / 10 });
  return t;
}

// The faces a look names, fetched once for the whole page: every chart asking for one gets these
// bytes (`datarequest`), and a change of type waits for them, so no chart shows a fallback first.
const faces = new Map();
function loadFace(path) {
  if (!faces.has(path)) faces.set(path, fetch(path).then((r) => (r.ok ? r.arrayBuffer() : Promise.reject(new Error(`${r.status} ${path}`)))));
  return faces.get(path);
}
function facesOf(tokens) {
  return Object.values(tokens).filter((v) => v && typeof v === "object" && typeof v.src === "string" && v.src.startsWith("/fonts/")).map((v) => v.src);
}
function answerFaces(view) {
  view.addEventListener("datarequest", (e) => {
    if (!e.detail.name.startsWith("font:")) return;
    const path = new URL(e.detail.url, location.href).pathname;
    if (!path.startsWith("/fonts/")) return;
    e.preventDefault();
    e.detail.respond(loadFace(path).then((b) => new Uint8Array(b)));
  });
}

const modeOf = (slot) => slot.dataset.mode ?? root.dataset.theme;
/** Charts the page styles itself (the theme studio's): the reader's look leaves them alone. */
const ownLook = (slot) => !!slot.closest("[data-own-look]");
/** The mode a chart plays in: its page's (or its own), unless the reader's theme is dark-first. */
const viewModeOf = (slot) => (!ownLook(slot) && look.custom?.modes?.[modeOf(slot)]?.mode) || modeOf(slot);
/** Style a chart about to mount: its tokens are there before its first frame. */
function lookAtMount(view, slot) {
  answerFaces(view);
  if (!isPlain() && !ownLook(slot)) view.setTokens(lookTokens(modeOf(slot)));
}
let applying = 0;
/** Apply the look to every chart on the page (after the faces it names are in). */
async function applyLook() {
  const run = ++applying;
  const modes = new Set(views.map(({ slot }) => modeOf(slot)).concat(root.dataset.theme));
  await Promise.all([...modes].flatMap((m) => facesOf(lookTokens(m))).map((p) => loadFace(p).catch(() => {})));
  if (run !== applying) return; // a newer choice came meanwhile
  for (const { slot, view } of views) {
    if (ownLook(slot)) continue;
    view.setAttribute("mode", viewModeOf(slot));
    view.setTokens(lookTokens(modeOf(slot)));
  }
  showLook();
}
function setLook(change) {
  look = { ...look, ...change };
  try { isPlain() ? localStorage.removeItem(LOOK_KEY) : localStorage.setItem(LOOK_KEY, JSON.stringify(look)); } catch { /* private mode */ }
  applyLook();
}

/** Show the look in every control that offers it (the header's menu, the editor on the home page),
 * the header's swatch, and the code the page ran. */
function showLook() {
  const t = lookTokens(root.dataset.theme);
  const b = BRANDS[look.preset] ?? BRANDS.default;
  // A studio theme's tokens may be expressions or generators: it brings its resolved colours.
  const swatch = look.custom?.swatch?.[root.dataset.theme];
  const pal = swatch ?? t.categorical ?? (root.dataset.theme === "dark" ? b.dark : b.light ?? b.dark)?.categorical ?? NEUTRAL.categorical;
  const accent = swatch?.[0] ?? t.accent ?? (root.dataset.theme === "dark" ? b.dark : b.light ?? b.dark)?.accent ?? NEUTRAL[root.dataset.theme === "dark" ? "dark" : "light"].accent;
  root.style.setProperty("--look-a", accent);
  root.style.setProperty("--look-b", pal[1] ?? accent);
  root.style.setProperty("--look-c", pal[2] ?? accent);
  const pressed = (sel, on) => { for (const el of document.querySelectorAll(sel)) el.setAttribute("aria-pressed", String(on(el))); };
  pressed("[data-look-preset]", (el) => !look.custom && el.dataset.lookPreset === look.preset);
  pressed("[data-look-accent]", (el) => (el.dataset.lookAccent || null) === look.accent);
  pressed("[data-look-palette]", (el) => el.dataset.lookPalette === look.palette);
  pressed("[data-look-type]", (el) => (el.dataset.lookType || null) === look.type);
  pressed("[data-look-mode]", (el) => el.dataset.lookMode === root.dataset.theme);
  pressed("[data-slot-mode]", (el) => el.dataset.slotMode === modeOf(document.querySelector(`.chart[data-chart="${el.dataset.for}"]`) ?? root));
  for (const el of document.querySelectorAll("input[data-look-custom]")) if (look.accent) el.value = look.accent;
  for (const el of document.querySelectorAll("input[data-look-range]")) {
    const k = el.dataset.lookRange;
    const v = look[k] ?? Number(el.dataset.preset ?? (k === "corners" ? (b.shape["radius.bar"] ?? 0) : (b.shape["stroke.line"] ?? 2)));
    el.value = String(v);
    const out = document.querySelector(`output[for="${el.id}"]`);
    if (out) out.textContent = look[k] == null ? "preset" : `${v} px`;
  }
  for (const el of document.querySelectorAll("[data-look-reset]")) el.disabled = isPlain();
  for (const el of document.querySelectorAll("[data-look-status]")) {
    el.textContent = isPlain() ? "The site's own look."
      : look.custom ? `Your theme “${look.custom.name}” — saved in this browser.`
      : `${b.name}${look.accent || look.type || look.palette !== "preset" || look.corners != null || look.lines != null ? ", your way" : ""} — saved in this browser.`;
  }
  const code = document.getElementById("look-code");
  if (code) {
    // The code the showcase chart ran (in its own mode, where the page gives it one).
    const mode = modeOf(document.querySelector('.chart[data-chart="brand"]') ?? root);
    const t = lookTokens(mode);
    const value = (v) => {
      if (Array.isArray(v)) {
        const rows = [];
        for (let i = 0; i < v.length; i += 3) rows.push(v.slice(i, i + 3).map((c) => JSON.stringify(c)).join(", "));
        return `[${rows.join(",\n    ")}]`;
      }
      if (v && typeof v === "object") return `{ ${Object.entries(v).map(([k, x]) => `${k}: ${k === "colors" ? value(x) : JSON.stringify(x)}`).join(", ")} }`;
      return JSON.stringify(v);
    };
    const body = Object.keys(t).length ? `{\n${Object.entries(t).map(([k, v]) => `  ${/^[a-z]+$/.test(k) ? k : JSON.stringify(k)}: ${value(v)},`).join("\n")}\n}` : "{}";
    code.textContent = `// every chart on the page\nview.setAttribute("mode", ${JSON.stringify(mode)});\nview.setTokens(${body});`;
    highlight(code);
  }
}

document.addEventListener("click", (e) => {
  const el = e.target.closest("[data-look-preset], [data-look-accent], [data-look-palette], [data-look-type], [data-look-mode], [data-look-reset], [data-slot-mode]");
  if (!el) return;
  // One chart's own mode (the theming page shows high contrast on its chart alone).
  if (el.dataset.slotMode) {
    const slot = document.querySelector(`.chart[data-chart="${el.dataset.for}"]`);
    if (!slot) return;
    slot.dataset.mode = el.dataset.slotMode;
    const view = slot.querySelector("datars-view");
    view?.setAttribute("mode", el.dataset.slotMode);
    view?.setTokens(lookTokens(el.dataset.slotMode));
    return showLook();
  }
  // A choice here replaces a theme from the studio (the choices build on a preset, not on it).
  if (el.dataset.lookPreset) setLook({ ...PLAIN, preset: el.dataset.lookPreset });
  else if ("lookAccent" in el.dataset) setLook({ accent: el.dataset.lookAccent || null, custom: null });
  else if (el.dataset.lookPalette) setLook({ palette: el.dataset.lookPalette, custom: null });
  else if ("lookType" in el.dataset) setLook({ type: el.dataset.lookType || null, custom: null });
  else if (el.dataset.lookMode) setPageTheme(el.dataset.lookMode);
  else if ("lookReset" in el.dataset) setLook({ ...PLAIN });
});
document.addEventListener("input", (e) => {
  const el = e.target;
  if (el.matches?.("input[data-look-custom]")) setLook({ accent: el.value, custom: null });
  else if (el.matches?.("input[data-look-range]")) setLook({ [el.dataset.lookRange]: Number(el.value), custom: null });
});

/** The theme studio's "Wear it on every page": its theme becomes the reader's look (`custom`
 * above), or none. */
export function wearTheme(custom) {
  setLook({ ...PLAIN, custom });
}
/** The studio theme the reader wears now, if any. */
export const wornTheme = () => look.custom;

// ---- replayed feeds ----------------------------------------------------------------------------
// A live chart on a static host: `data-feed-for="<alias>"` on an element with `data-src` (a folder of
// numbered snapshots, 01.json …), `data-feed-source` (the source's name) and `data-feed-count`
// answers the chart's requests for that source itself
// (`datarequest`) with the next snapshot, so each refresh animates in as a real feed would. With
// `data-feed-loop` it starts over after holding the last one; setting `dataset.next = "1"` replays.
// Each answer fires a `feed` event on the element: `{ n, count }`.
for (const el of document.querySelectorAll("[data-feed-for]")) {
  const slot = document.querySelector(`.chart[data-chart="${el.dataset.feedFor}"]`);
  const count = Number(el.dataset.feedCount) || 1, loop = el.hasAttribute("data-feed-loop");
  if (!slot) continue;
  el.dataset.next = "1";
  let held = 0;
  const wire = (view) => {
    if (view.dataset.fed) return;
    view.dataset.fed = "1";
    view.addEventListener("datarequest", (e) => {
      if (e.detail.name !== el.dataset.feedSource) return;
      const n = Math.min(Number(el.dataset.next) || 1, count);
      if (n === count && loop && ++held > 3) { held = 0; el.dataset.next = "1"; }
      else el.dataset.next = String(Math.min(n + 1, count));
      e.preventDefault();
      el.dispatchEvent(new CustomEvent("feed", { detail: { n, count } }));
      e.detail.respond(fetch(`${el.dataset.src}${String(n).padStart(2, "0")}.json`).then((r) => r.text()));
    });
  };
  const found = () => { const v = slot.querySelector("datars-view"); if (v) wire(v); };
  new MutationObserver(found).observe(slot, { childList: true, subtree: true });
  found();
}

// ---- code highlighting (tiny, just for the snippets on this page) ----------------------------
const esc = (s) => s.replace(/[&<>]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" })[c]);
// Per language: one regex whose capture groups are token kinds, and the class of each group.
const LANGS = {
  ts: {
    re: /(\/\/[^\n]*|\/\*[\s\S]*?\*\/)|("(?:[^"\\\n]|\\.)*"|'(?:[^'\\\n]|\\.)*'|`(?:[^`\\]|\\.)*`)|\b(import|from|export|default|const|let|return|true|false|null|new|function)\b|\b(\d[\d_]*(?:\.\d+)?)\b|([A-Za-z_$][\w$]*)(?=\s*\()|([A-Za-z_$][\w$]*)(?=\??:\s)/g,
    cls: ["c", "s", "k", "n", "f", "p"],
  },
  sh: { re: /(#[^\n]*)|("(?:[^"\\\n]|\\.)*"|'[^'\n]*')|\b(datars)\b|(--[\w-]+)/g, cls: ["c", "s", "f", "p"] },
  html: { re: /(<!--[\s\S]*?-->)|("[^"]*")|(<\/?[\w-]+|\/?>)|([\w-]+)(?==)/g, cls: ["c", "s", "t", "p"] },
};
LANGS.js = LANGS.ts;

export function highlight(el) {
  const lang = LANGS[el.dataset.lang];
  if (!lang) return;
  const text = el.textContent;
  let out = "", last = 0;
  lang.re.lastIndex = 0;
  for (let m; (m = lang.re.exec(text)); ) {
    const i = m.findIndex((g, j) => j > 0 && g !== undefined);
    out += esc(text.slice(last, m.index)) + `<span class="tk-${lang.cls[i - 1]}">${esc(m[0])}</span>`;
    last = m.index + m[0].length;
  }
  el.innerHTML = out + esc(text.slice(last));
}
document.querySelectorAll("pre code[data-lang]:not([data-hl])").forEach(highlight);
showLook(); // after the highlighter's tables above exist

// For page scripts (motion.js writes the playground's motion rule as the reader changes it).
export { highlight };
