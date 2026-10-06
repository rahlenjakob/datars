// The theme studio (/themes/): a theme editor over ten published charts. The page holds a theme as
// an author writes it — `{ name, scheme, tokens, modes, locked }`, extending datars/neutral — and
// hands each chart the layer of tokens for the mode on show with `setTokens()` and `mode`, exactly
// as a host app restyles charts it downloaded. Nothing is re-rendered but the charts' next frames.
//
// What the engine does, the page doesn't redo: a hidden `<datars-view>` (the resolver) resolves the
// theme in each mode — colour expressions, generated palettes, the cascade of base and mode tokens —
// and the editor shows, checks and animates what it returns. The checks are the engine's own
// (datars-theme `validate`: WCAG contrast, ΔE in OKLab under simulated colour-vision deficiencies,
// monotone ramps), recomputed here on those resolved colours with the same formulas.

import { BRANDS, highlight, wearTheme, wornTheme } from "./site.js";

const MODES = ["light", "dark", "high-contrast"];
const data = JSON.parse(document.getElementById("studio-data").textContent);
const NEUTRAL = data.authored;
/** The site's root (this file sits there): faces the site serves are under `fonts/`. */
const SITE = new URL(".", import.meta.url);
const STORE = "datars-studio";
const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
const $ = (sel, el = document) => el.querySelector(sel);
const $$ = (sel, el = document) => [...el.querySelectorAll(sel)];
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
const clone = (v) => JSON.parse(JSON.stringify(v));

// ---- colour maths (datars-color's, for checks, swatches and the in-between frames) -------------

function rgba(hex) {
  let h = String(hex).replace("#", "");
  if (h.length <= 4) h = [...h].map((c) => c + c).join("");
  const n = parseInt(h.slice(0, 6), 16);
  return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255, h.length >= 8 ? parseInt(h.slice(6, 8), 16) / 255 : 1];
}
const byte = (v) => Math.round(Math.min(1, Math.max(0, v)) * 255).toString(16).padStart(2, "0");
const hexOf = ([r, g, b, a = 1]) => `#${byte(r)}${byte(g)}${byte(b)}${a < 0.999 ? byte(a) : ""}`;
const toLin = (c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
const toSrgb = (c) => (c <= 0.0031308 ? c * 12.92 : 1.055 * c ** (1 / 2.4) - 0.055);
function oklab([r, g, b]) {
  [r, g, b] = [toLin(r), toLin(g), toLin(b)];
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return [0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s, 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s, 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s];
}
function fromOklab([L, a, b]) {
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3, m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3, s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [toSrgb(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s), toSrgb(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s), toSrgb(-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s)];
}
const lch = (hex) => { const [L, a, b] = oklab(rgba(hex)); return [L, Math.hypot(a, b), Math.atan2(b, a)]; };
const fromLch = (L, C, H) => hexOf(fromOklab([L, C * Math.cos(H), C * Math.sin(H)]));
/** `mix(a, b, t)` in OKLab, as the engine interpolates colours. */
function mix(a, b, t) {
  const [x, y] = [rgba(a), rgba(b)];
  const [p, q] = [oklab(x), oklab(y)];
  return hexOf([...fromOklab(p.map((v, i) => v + (q[i] - v) * t)), x[3] + (y[3] - x[3]) * t]);
}
const luminance = (c) => { const [r, g, b] = c.map(toLin); return 0.2126 * r + 0.7152 * g + 0.0722 * b; };
/** WCAG contrast of `fg` over `bg` (a translucent foreground composited on its background first). */
function contrast(fg, bg) {
  const [f, b] = [rgba(fg), rgba(bg)];
  const over = [0, 1, 2].map((i) => f[i] * f[3] + b[i] * (1 - f[3]));
  const [x, y] = [luminance(over), luminance(b)];
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}
const CVD = {
  protanopia: [[0.152286, 1.052583, -0.204868], [0.114503, 0.786281, 0.099216], [-0.003882, -0.048116, 1.051998]],
  deuteranopia: [[0.367322, 0.860646, -0.227968], [0.280085, 0.672501, 0.047413], [-0.01182, 0.04294, 0.968881]],
  tritanopia: [[1.255528, -0.076749, -0.178779], [-0.078411, 0.930809, 0.147602], [0.004733, 0.691367, 0.3039]],
};
function simulate(hex, kind) {
  if (!kind) return hex;
  const c = rgba(hex), v = c.slice(0, 3).map(toLin);
  return hexOf([...CVD[kind].map((row) => toSrgb(Math.min(1, Math.max(0, row[0] * v[0] + row[1] * v[1] + row[2] * v[2])))), c[3]]);
}
const deltaE = (a, b) => { const [p, q] = [oklab(rgba(a)), oklab(rgba(b))]; return Math.hypot(p[0] - q[0], p[1] - q[1], p[2] - q[2]) * 100; };
/** The smallest ΔE between any two colours, and which two. */
function closest(colors) {
  let worst = [Infinity, 0, 0];
  for (let i = 0; i < colors.length; i++) for (let j = i + 1; j < colors.length; j++) {
    const d = deltaE(colors[i], colors[j]);
    if (d < worst[0]) worst = [d, i, j];
  }
  return worst;
}
const isHex = (v) => typeof v === "string" && /^#[0-9a-f]{3,8}$/i.test(v);

// ---- the theme -------------------------------------------------------------------------------

const isFont = (v) => !!v && typeof v === "object" && !Array.isArray(v) && ("family" in v || "google" in v);
const isGen = (v) => !!v && typeof v === "object" && !Array.isArray(v) && "generate" in v;
const PALETTES = ["categorical", "sequential", "diverging"];
/** Tokens a mode may set differently (colours, palettes, how far marks dim); type and shapes are
 * the same in every mode. */
const perMode = (token) => token === "dim" || PALETTES.includes(token) || isHex(data.resolved.light[token]);

const blank = () => ({ name: "my-theme", scheme: "light", tokens: {}, modes: { dark: {}, "high-contrast": {} }, locked: [], from: "default" });

/** A face of the site's presets (`{ family, weight, src }`) as an author writes it: a Google
 * Fonts family (all of the site's faces are). */
const googleFace = (f) => ({ google: f.family[0], weight: f.weight });
function presetTheme(id) {
  if (id === "default") return blank();
  if (id === "ledger") return LEDGER();
  const b = BRANDS[id];
  const conv = (o) => Object.fromEntries(Object.entries(o ?? {}).map(([k, v]) => [k, isFont(v) ? googleFace(v) : clone(v)]));
  const t = blank();
  t.name = id;
  t.from = id;
  if (!b.light) {
    // Dark everywhere: its night palette in light mode too.
    t.scheme = "dark";
    t.tokens = { ...conv(b.shape), ...conv(b.dark) };
    t.modes["high-contrast"] = { paper: "#000000", ink: "#ffffff", grid: "mix($ink, $paper, 0.65)", muted: "mix($ink, $paper, 0.2)" };
  } else {
    t.tokens = { ...conv(b.shape), ...conv(b.light) };
    t.modes.dark = conv(b.dark);
    // High contrast: the brand's type and shapes, black on white, an accent that reads as text.
    t.modes["high-contrast"] = { paper: "#ffffff", ink: "#000000", accent: readable(b.light.accent, "#ffffff", 4.5, -1), card: "#ffffff", "card-line": "#000000" };
  }
  return t;
}
/** A preset of the studio's own: a quiet financial look — navy and brass, Plex type, flat bars. */
function LEDGER() {
  const t = blank();
  Object.assign(t, { name: "ledger", from: "ledger" });
  t.tokens = {
    paper: "#f7f5ef", ink: "#14213d", accent: "#1f3a5f", highlight: "#b08d57",
    // Six colours that pass the distinctness check for colour-blind readers too (as do dark's).
    categorical: ["#1f3a5f", "#d9a441", "#3c9a78", "#c2414c", "#8e7cc3", "#7cc0e6"],
    sequential: { kind: "sequential", colors: ["#eef1f5", "#c3cddb", "#8a9cb6", "#4f6788", "#1f3a5f"] },
    "font.title": { google: "IBM Plex Sans", weight: 600 }, "font.strong": { google: "IBM Plex Sans", weight: 600 },
    "font.body": { google: "IBM Plex Sans", weight: 400 }, "font.number": { google: "IBM Plex Mono", weight: 400 },
    "size.title": 16, "radius.bar": 1, "radius.card": 4, "stroke.line": 1.6, "point.radius": 3, "band.padding": 0.3,
    "map.water": "#e3e8ee", "map.land": "#fbfaf6", card: "#ffffff", "card-line": "#d9d4c7",
  };
  t.modes.dark = {
    paper: "#0f1724", ink: "#e8e4da", accent: "#8fb3e0", highlight: "#d6b47e",
    categorical: ["#8fb3e0", "#e3b04f", "#3fae86", "#f0707e", "#c4a6ff", "#eef0f4"],
    sequential: { kind: "sequential", colors: ["#1b2a40", "#2c4466", "#46658f", "#7395c0", "#b9d0ec"] },
    "map.water": "#0b111b", "map.land": "#182233", card: "#162131", "card-line": "#2c3a4f",
  };
  t.modes["high-contrast"] = { paper: "#ffffff", ink: "#000000", accent: "#14213d" };
  return t;
}

/** A colour moved along OKLCH lightness (`dir` +1 lighter, −1 darker) until it reaches `min`
 * contrast on `bg`: the `d` of `lighten(c, d)` / `darken(c, d)`, 0 when it reads already. */
function lightnessNeeded(hex, bg, min, dir) {
  const [L, C, H] = lch(hex);
  for (let d = 0; d <= 0.6; d += 0.02) if (contrast(fromLch(Math.min(1, Math.max(0, L + dir * d)), C, H), bg) >= min) return Math.round(d * 100) / 100;
  return 0.6;
}
const readable = (hex, bg, min, dir) => { const d = lightnessNeeded(hex, bg, min, dir); const [L, C, H] = lch(hex); return d ? fromLch(L + dir * d, C, H) : hex; };
/** The opposite hue at the same lightness, chroma reduced until it's a real (sRGB) colour. */
function opposite(hex) {
  const [L, C, H] = lch(hex);
  for (let c = C; c > 0; c -= 0.01) {
    const out = fromOklab([L, c * Math.cos(H + Math.PI), c * Math.sin(H + Math.PI)]);
    if (out.every((v) => v >= -0.001 && v <= 1.001)) return hexOf(out);
  }
  return fromLch(L, 0, H);
}

/** One colour → a theme: the brand as accent, palettes the engine generates from it, and lighter
 * or darker accents where the brand wouldn't read — written as expressions over `$brand`. */
function brandTheme(hex, paper) {
  const t = blank();
  t.name = "my-brand";
  t.from = "brand";
  // The categorical palette follows the accent (so it leads with the lighter accent in dark mode);
  // the ramps follow the brand.
  t.tokens = {
    brand: hex,
    accent: "$brand",
    categorical: { generate: "categorical", from: "$accent", n: 8 },
    sequential: { generate: "sequential", from: "$brand", n: 5 },
    diverging: { generate: "diverging", from: opposite(hex), to: "$brand", n: 7 },
  };
  const dark = paper === "dark";
  const nightPaper = dark ? mix("#0d0f14", hex, 0.1) : "#111318";
  const lift = lightnessNeeded(hex, nightPaper, 3, 1);
  const sink = lightnessNeeded(hex, "#ffffff", 4.5, -1);
  // On dark paper a ramp runs from the paper up to light (as the built-in theme's dark one does):
  // the brand's hue at rising lightness.
  const [, C, H] = lch(hex);
  const nightRamp = { kind: "sequential", colors: [0.3, 0.42, 0.56, 0.72, 0.88].map((L, i) => fromLch(L, Math.min(C, 0.16) * (0.55 + 0.1 * i), H)) };
  if (dark) {
    t.scheme = "dark";
    Object.assign(t.tokens, { paper: "mix(#0d0f14, $brand, 0.1)", ink: "#eef0f4", sequential: nightRamp, ...(lift ? { accent: `lighten($brand, ${lift})` } : {}) });
    t.modes["high-contrast"] = { paper: "#000000", ink: "#ffffff", accent: lift ? `lighten($brand, ${Math.min(0.6, lift + 0.1)})` : "$brand" };
  } else {
    if (paper === "tinted") {
      t.tokens.paper = "mix(#ffffff, $brand, 0.05)";
      t.tokens.card = "mix($paper, $brand, 0.06)";
      t.modes.dark.paper = "mix(#111318, $brand, 0.1)";
    }
    t.modes.dark.sequential = nightRamp;
    if (lift) t.modes.dark.accent = `lighten($brand, ${lift})`;
    if (sink) t.modes["high-contrast"].accent = `darken($brand, ${sink})`;
  }
  return t;
}

function restore() {
  const hash = new URLSearchParams(location.hash.slice(1)).get("theme");
  if (hash) {
    try {
      const json = new TextDecoder().decode(Uint8Array.from(atob(hash.replace(/-/g, "+").replace(/_/g, "/")), (c) => c.charCodeAt(0)));
      return normalise(JSON.parse(json));
    } catch { /* a broken link: start from what's saved */ }
  }
  try { return normalise(JSON.parse(localStorage.getItem(STORE) ?? "null")); } catch { return null; }
}
function normalise(t) {
  if (!t || typeof t !== "object" || typeof t.tokens !== "object") return null;
  return { ...blank(), ...t, modes: { dark: {}, "high-contrast": {}, ...(t.modes ?? {}) }, locked: Array.isArray(t.locked) ? t.locked : [] };
}

let theme = restore() ?? blank();
const root = document.documentElement;
let mode = root.dataset.theme === "dark" ? "dark" : "light";

/** The view mode and the token layer a chart gets for page mode `m`: base tokens, then the mode's
 * own (a dark-first theme plays dark in light mode, from its base tokens). */
function layer(m) {
  const viewMode = theme.scheme === "dark" && m === "light" ? "dark" : m;
  const own = m === "light" ? {} : theme.modes[m] ?? {};
  return { viewMode, tokens: { ...theme.tokens, ...own } };
}
/** Where an edit of `token` in the mode on show goes: type and shapes (and everything in a dark-
 * first theme) to the base, colours in dark or high contrast to that mode's block. */
const editLayer = (token) => (!perMode(token) || mode === "light" || theme.scheme === "dark" ? theme.tokens : theme.modes[mode]);
/** The value `token` has in mode `m` as written, and who wrote it — this theme's mode block, its
 * base, or datars/neutral. */
function written(token, m = mode) {
  const own = m === "light" ? null : theme.modes[m];
  if (own && token in own) return { value: own[token], by: m };
  if (token in theme.tokens) return { value: theme.tokens[token], by: "base" };
  const vm = layer(m).viewMode;
  const nm = vm === "light" ? null : NEUTRAL.modes?.[vm];
  return { value: nm && token in nm ? nm[token] : NEUTRAL.tokens[token], by: "neutral" };
}

// ---- fonts -----------------------------------------------------------------------------------

const FAMILIES = new Map(data.fonts.flatMap(([, names]) => names.map(([n, w]) => [n, { weights: w, id: n.toLowerCase().replace(/\s+/g, "-") }])));
/** Where this page previews a face from: the site's own file, else Fontsource's copy of Google
 * Fonts (latin subset, static instances). A build ships the real face from Google instead. */
function faceUrl(family, weight) {
  const local = data.local[family]?.[weight];
  if (local) return new URL(`fonts/${local}.ttf`, SITE).href;
  const id = FAMILIES.get(family)?.id ?? family.toLowerCase().replace(/\s+/g, "-");
  return `https://cdn.jsdelivr.net/fontsource/fonts/${id}@latest/latin-${weight}-normal.ttf`;
}
/** A font token as this page hands it to a chart: the family, weight and a file to fetch. */
const previewFont = (v) => {
  const family = v.google ?? (Array.isArray(v.family) ? v.family[0] : v.family);
  return { family: [family], weight: v.weight ?? 400, src: faceUrl(family, v.weight ?? 400) };
};
const bytes = new Map(), loaded = new Map();
/** A face's bytes, fetched once for every chart on the page (`loaded` once they're in). One the
 * reader is waiting for goes ahead of the charts' own downloads; one fetched ahead goes after. */
function loadFace(url, priority = "high") {
  if (!bytes.has(url)) {
    const p = fetch(url, { priority }).then((r) => (r.ok ? r.arrayBuffer() : Promise.reject(new Error(`${r.status}`)))).then((b) => new Uint8Array(b));
    p.then((b) => loaded.set(url, b), () => bytes.delete(url));
    bytes.set(url, p);
  }
  return bytes.get(url);
}
const facesOf = (tokens) => Object.values(tokens).filter((v) => isFont(v) && v.src).map((v) => v.src);

// ---- the resolver: the engine resolves the theme, per mode ----------------------------------

const resolver = document.createElement("datars-view");
resolver.setAttribute("cpu", "");
resolver.setAttribute("height", "4");
resolver.setAttribute("no-controls", "");
$("#sp-resolver").appendChild(resolver);
const resolverReady = customElements.whenDefined("datars-view").then(async () => {
  await resolver.setDocument({ datars: 1, size: { width: 4, height: 4 }, scene: { kind: "group", key: "root", children: [] } });
  for (let i = 0; i < 400 && !resolver.status; i++) await sleep(25);
});
/** Generated palettes by what they're generated from: generating a categorical palette is the
 * engine's costliest resolve (a search for the most distinct colours), so each is made once. */
const generated = new Map();

/** The theme resolved in mode `m`: every token's value (`resolved`), the layer a chart gets with
 * generated palettes as their colours (`literal`; a generator would cost each chart that search
 * on every change), and the resolver's complaints about tokens that don't resolve. */
function resolve(m) {
  const { viewMode, tokens } = layer(m);
  const lite = {}, gens = [];
  for (const [k, v] of Object.entries(tokens)) {
    if (isFont(v)) continue; // faces don't change colours, and the resolver needn't fetch them
    if (isGen(v)) {
      gens.push([k, v]);
      lite[`studio-from-${k}`] = v.from;
      if (v.to) lite[`studio-to-${k}`] = v.to;
    } else lite[k] = v;
  }
  if (resolver.getAttribute("mode") !== viewMode) resolver.setAttribute("mode", viewMode);
  let seen = resolver.status?.diagnostics?.length ?? 0;
  const pass = (t) => { resolver.setTokens(t); return resolver.status?.tokens?.tokens ?? {}; };
  /** What the resolver said about the tokens of the last pass. */
  const said = () => {
    const all = resolver.status?.diagnostics ?? [];
    const out = all.slice(seen).map((d) => /^theme token `([^`]+)`: (.*)$/.exec(d)).filter(Boolean);
    seen = all.length;
    return out;
  };
  let r = pass(lite);
  // A token that doesn't resolve (half-typed, a typo, a missing reference) is left out, so the
  // charts keep the inherited value rather than drawing the engine's magenta for unknown inks.
  const errors = new Map();
  for (const [, token, message] of said()) {
    const k = token.replace(/^studio-(from|to)-/, "");
    if (k in tokens) errors.set(k, message);
  }
  for (const k of Object.keys(lite)) {
    if (k.startsWith("studio-")) continue;
    const want = data.resolved.light[k];
    const ok = Array.isArray(want) ? Array.isArray(r[k]) && r[k].every(isHex) : isHex(want) ? isHex(r[k]) : typeof want === "number" ? typeof r[k] === "number" : true;
    if (!ok && !errors.has(k)) errors.set(k, Array.isArray(want) ? "not a list of colours" : isHex(want) ? "not a colour" : "not a number");
  }
  for (const k of errors.keys()) {
    delete lite[k];
    delete lite[`studio-from-${k}`];
    delete lite[`studio-to-${k}`];
  }
  if (errors.size) { r = pass(lite); said(); }
  for (let i = gens.length - 1; i >= 0; i--) if (errors.has(gens[i][0])) gens.splice(i, 1);
  if (gens.length) {
    for (const [k, v] of gens) {
      const from = r[`studio-from-${k}`], to = v.to ? r[`studio-to-${k}`] : undefined;
      delete lite[`studio-from-${k}`];
      delete lite[`studio-to-${k}`];
      if (!isHex(from)) { errors.set(k, "nothing to generate from"); continue; }
      const key = `${v.generate}|${from}|${to ?? ""}|${v.n ?? 8}`;
      if (!generated.has(key)) generated.set(key, pass({ ...lite, "studio-gen": { generate: v.generate, from, ...(isHex(to) ? { to } : {}), n: v.n ?? 8 } })["studio-gen"]);
      const colors = generated.get(key);
      if (Array.isArray(colors)) lite[k] = v.generate === "categorical" ? colors : { kind: v.generate, colors };
    }
    r = pass(lite);
    said();
  }
  const resolved = Object.fromEntries(Object.entries(r).filter(([k]) => !k.startsWith("studio-")));
  return { viewMode, resolved, literal: lite, errors };
}
/** The tokens a chart gets in mode `m`: the literal layer and the faces to fetch. */
function hostLayer(m, res) {
  const t = { ...res.literal };
  for (const [k, v] of Object.entries(layer(m).tokens)) if (isFont(v)) t[k] = previewFont(v);
  return t;
}

// ---- the charts ------------------------------------------------------------------------------

const studio = $("#studio");
const wall = $("#wall");
/** Each chart: its slot and view, whether it's on screen, the faces it holds, and what it got. */
const tiles = [];
/** What the charts show: `{ m, viewMode, host, resolved }`. Until the resolver is up, the theme
 * as written (charts generate its palettes themselves) — so a chart that mounts first opens in
 * the reader's theme, and the resolved layer that follows draws the same. */
let current = (() => {
  const { viewMode, tokens } = layer(mode);
  const host = Object.fromEntries(Object.entries(tokens).map(([k, v]) => [k, isFont(v) ? previewFont(v) : v]));
  facesOf(host).forEach((u) => loadFace(u));
  return { m: mode, viewMode, host, resolved: null };
})();

// Every chart in the studio plays the studio's mode, not the page's (site.js leaves slots with a
// mode of their own alone).
for (const slot of $$(".chart", wall)) slot.dataset.mode = layer(mode).viewMode;
/** The papers this theme resolved to last time, by mode (for the frames, before the resolver is up). */
const signature = () => JSON.stringify([theme.scheme, theme.tokens, theme.modes]);
function papers() {
  try {
    const p = JSON.parse(localStorage.getItem(`${STORE}-paper`) ?? "null");
    return p?.sig === signature() ? p.papers : {};
  } catch { return {}; }
}
if (isHex(papers()[mode])) wall.style.setProperty("--tile-paper", papers()[mode]);

const seen = new IntersectionObserver((entries) => {
  for (const e of entries) {
    const t = tiles.find((x) => x.view === e.target);
    if (!t) continue;
    t.seen = e.isIntersecting;
    if (t.seen && t.stale) deliver(t, null, false);
  }
});
// Charts well off screen take a change when they come near (a slider dragged re-lays out only
// the charts around the reader).
const near = new IntersectionObserver((entries) => {
  for (const e of entries) {
    const t = tiles.find((x) => x.view === e.target);
    if (!t) continue;
    t.near = e.isIntersecting;
    if (t.near && t.stale) deliver(t, null, false);
  }
}, { rootMargin: "60% 0px" });

studio.addEventListener("chartmount", (e) => {
  const view = e.detail.view, slot = e.target;
  const tile = { slot, view, seen: false, near: true, stale: false, faces: new Set(), got: null };
  tiles.push(tile);
  // A starting chart asks for the faces its tokens name: answered from what this page fetched.
  view.addEventListener("datarequest", (ev) => {
    if (!ev.detail.name.startsWith("font:")) return;
    const got = bytes.get(ev.detail.url);
    if (!got) return;
    ev.preventDefault();
    ev.detail.respond(got);
    tile.faces.add(ev.detail.url);
  });
  if (current) {
    view.setAttribute("mode", current.viewMode);
    view.setTokens(current.host);
    tile.got = current.host;
    // A new chart waits for its faces before its first frame (the runtime's own wait), then has them.
    for (const u of facesOf(current.host)) tile.faces.add(u);
  }
  // Watched once it's on the page (the event comes just before): Safari reports nothing at all
  // for an element it started watching before that.
  queueMicrotask(() => { seen.observe(view); near.observe(view); });
});
// The charts may mount now (site.js held back any that came near before this script ran).
studio.dataset.ownLook = "ready";
document.dispatchEvent(new Event("ownlookready"));

/** Hand a chart the current layer. */
function deliver(tile, from, animate) {
  const want = current;
  tile.stale = false;
  tile.view.setAttribute("mode", want.viewMode);
  if (animate && from?.resolved && tile.seen && !reduced) return blend(tile, from, want);
  put(tile, want.host);
}
/** Tokens to a chart, and with them the bytes of any face it hasn't got — in the same task, before
 * its next frame: the engine names a face `font:<family>-<weight>`, and one handed over at once is
 * never drawn in a fallback first (a face that arrives later would be, for a frame, then snap). */
function put(tile, tokens) {
  const { view } = tile;
  view.setTokens(tokens);
  tile.got = tokens;
  if (!view.dataset.renderer) return; // still starting: it fetches its faces before its first frame
  for (const v of Object.values(tokens)) {
    if (!isFont(v) || tile.faces.has(v.src) || !loaded.has(v.src)) continue;
    try { view.provideData(`font:${v.family[0]}-${v.weight}`, loaded.get(v.src)); } catch { /* not named any more */ }
    tile.faces.add(v.src);
  }
}

/** In-between frames for a change of look or mode: every colour token (and palette) eased from
 * what the chart showed to what it will, in OKLab, handed over 26 times or so. The engine re-inks
 * each frame from tokens; nothing else is redone. */
const blends = new Map();
function blend(tile, from, to) {
  blends.set(tile, { from, to });
  tile.view.setAttribute("mode", to.viewMode);
  if (blends.size === 1) requestAnimationFrame(step);
}
const BLEND_MS = 450;
let blendStart = 0;
function step(now) {
  if (!blendStart) blendStart = now;
  const t = Math.min(1, (now - blendStart) / BLEND_MS);
  const e = t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2; // cubic-in-out, the engine's default
  for (const [tile, { from, to }] of blends) {
    if (current !== to) { blends.delete(tile); continue; }
    if (t >= 1) {
      put(tile, to.host);
      continue;
    }
    const inks = {};
    for (const [k, b] of Object.entries(to.resolved)) {
      const a = from.resolved[k];
      if (isHex(a) && isHex(b)) inks[k] = mix(a, b, e);
      else if (Array.isArray(a) && Array.isArray(b) && a.length === b.length && b.every(isHex)) {
        const colors = b.map((c, i) => mix(a[i], c, e));
        inks[k] = k === "categorical" ? colors : { kind: k === "diverging" ? "diverging" : "sequential", colors };
      }
    }
    put(tile, { ...to.host, ...inks });
  }
  if (t >= 1) { blends.clear(); blendStart = 0; return; }
  if (blends.size) requestAnimationFrame(step);
  else blendStart = 0;
}

let applying = 0, staleTimer = 0;
/** Resolve the theme for the mode on show and hand it to every chart (and the editor). */
async function apply({ animate = false, touched = null } = {}) {
  const run = ++applying;
  await resolverReady;
  if (run !== applying) return;
  const res = resolve(mode);
  const host = hostLayer(mode, res);
  // The editor shows the change at once; the charts take it when its faces are in (a chart never
  // draws a face it hasn't got, so they wait rather than show a fallback).
  render(res, touched);
  const faces = facesOf(host);
  const missing = Object.values(host).filter((v) => isFont(v) && !loaded.has(v.src));
  if (missing.length) status(`Fetching ${[...new Set(missing.map((v) => v.family[0]))].join(" and ")}…`);
  try {
    await Promise.all(faces.map((u) => loadFace(u)));
  } catch {
    if (run === applying) fontFailed(faces);
    return;
  }
  if (run !== applying) return;
  const from = current;
  current = { m: mode, viewMode: res.viewMode, host, resolved: res.resolved };
  for (const slot of $$(".chart", wall)) slot.dataset.mode = res.viewMode;
  paper(res.resolved.paper);
  const t0 = performance.now();
  let n = 0;
  // The charts on screen take it now; the others when the reader stops (a slider being dragged
  // re-lays out only what's seen) or when they come near.
  for (const tile of tiles) {
    if (!tile.view.isConnected) continue;
    if (!tile.seen && !animate) { tile.stale = true; continue; }
    if (tile.seen) n++;
    deliver(tile, from, animate);
  }
  clearTimeout(staleTimer);
  staleTimer = setTimeout(() => { for (const t of tiles) if (t.stale && t.near && t.view.isConnected) deliver(t, null, false); }, 300);
  const ms = performance.now() - t0;
  const charts = `<b>${n} chart${n === 1 ? "" : "s"}</b> on screen`;
  if (!n || !from.resolved) status(null); // nothing seen changing (or the page opening)
  else if (animate && !reduced && from?.resolved) status(`${charts} easing to the new look: every frame's inks handed over by <code>setTokens()</code>, nothing else re-rendered.`);
  else status(`${charts} took the change in ${ms < 10 ? ms.toFixed(1) : Math.round(ms)} ms — <code>setTokens()</code> on each, nothing else re-rendered.`);
  later();
}
/** The charts' frames take their paper, so a chart reads as one surface to its edge (and it's kept
 * for the next visit: the frames open in it). */
function paper(hex) {
  if (!isHex(hex)) return;
  wall.style.setProperty("--tile-paper", hex);
  try { localStorage.setItem(`${STORE}-paper`, JSON.stringify({ sig: signature(), papers: { ...papers(), [mode]: hex } })); } catch { /* private mode */ }
}
/** The faces the presets use, fetched while the reader looks (or hovers there), so picking one
 * rarely waits for its type. */
function prefetchPresetFaces() {
  for (const id of ["newsprint", "nordic", "neon", "sunset", "ledger"]) for (const v of Object.values(presetTheme(id).tokens)) if (isFont(v)) loadFace(previewFont(v).src, "low");
}
$(".studio-presets").addEventListener("pointerenter", prefetchPresetFaces, { once: true });
$(".studio-presets").addEventListener("focusin", prefetchPresetFaces, { once: true });
(window.requestIdleCallback ?? ((f) => setTimeout(f, 3000)))(prefetchPresetFaces, { timeout: 6000 });
let rafApply = 0, pending = null;
/** Apply at the next frame (a slider or a colour being dragged sends many inputs per frame). */
function soon(touched) {
  pending = touched;
  if (rafApply) return;
  rafApply = requestAnimationFrame(() => { rafApply = 0; apply({ touched: pending }); });
}

const statusEl = $("#wall-status"), OPENING = statusEl.innerHTML;
/** The line over the wall: what the last change cost, or what's being waited for. Nothing to say
 * keeps the last line (or the opening one, after a wait). */
function status(html) {
  if (html) statusEl.innerHTML = html;
  else if (statusEl.dataset.waiting) statusEl.innerHTML = OPENING;
  statusEl.dataset.waiting = html?.startsWith("Fetching") ? "1" : "";
}

// ---- editing ---------------------------------------------------------------------------------

const undo = [];
let editing = false;
/** Note the theme before a change (one entry per gesture: a drag is one change). */
function remember() {
  if (editing) return;
  undo.push(JSON.stringify(theme));
  if (undo.length > 100) undo.shift();
  $("[data-studio-undo]").disabled = false;
}
const settle = () => { editing = false; };
function set(token, value, { gesture = false } = {}) {
  remember();
  editing = gesture;
  const target = editLayer(token);
  if (value === undefined) delete target[token];
  else target[token] = value;
  theme.from = theme.from && `${theme.from.replace(/\*$/, "")}*`; // started from it, then changed
  hit(token);
  save();
}
function replace(next, opts = {}) {
  remember();
  theme = next;
  save();
  apply({ animate: true, ...opts });
}

/** The chips of the charts that read a token light up as it changes. */
function hit(token) {
  for (const c of $$(`.tile-tokens code[data-tok="${CSS.escape(token)}"]`)) {
    c.classList.remove("hit");
    void c.offsetWidth;
    c.classList.add("hit");
  }
}

let saveTimer = 0;
function save() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    try { localStorage.setItem(STORE, JSON.stringify(theme)); } catch { /* private mode */ }
    history.replaceState(null, "", `#theme=${shareCode()}`);
  }, 300);
}
function shareCode() {
  const { name, scheme, tokens, modes, locked } = theme;
  const json = JSON.stringify({ name, scheme, tokens, modes, locked, from: theme.from });
  return btoa(String.fromCharCode(...new TextEncoder().encode(json))).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

// Colour tokens: the picker sets a colour; the text field takes a colour or an expression.
for (const row of $$('.tok[data-kind="color"]')) {
  const token = row.dataset.token;
  const pick = $('input[type="color"]', row), text = $(".tok-val", row);
  pick.addEventListener("input", () => { set(token, pick.value, { gesture: true }); soon(token); });
  pick.addEventListener("change", settle);
  text.addEventListener("input", () => {
    const v = text.value.trim();
    if (!v) return;
    set(token, v, { gesture: true });
    soon(token);
  });
  text.addEventListener("change", () => {
    settle();
    if (!text.value.trim()) set(token, undefined);
    apply({ touched: token }); // and say whether it resolved, now that it's written
  });
  text.addEventListener("keydown", (e) => { if (e.key === "Enter") text.blur(); });
}
// Numbers.
for (const row of $$('.tok[data-kind="number"]')) {
  const token = row.dataset.token, range = $('input[type="range"]', row);
  range.addEventListener("input", () => { set(token, Number(range.value), { gesture: true }); soon(token); });
  range.addEventListener("change", settle);
}
// Resets.
for (const btn of $$(".tok .tok-reset")) {
  btn.addEventListener("click", () => {
    const token = btn.closest(".tok").dataset.token;
    set(token, undefined);
    apply({ animate: true, touched: token });
  });
}

// Fonts: a family from the list, or any Google Fonts family by name.
let googleList = null;
async function googleFamilies() {
  googleList ??= fetch("https://api.fontsource.org/v1/fonts?type=google").then((r) => r.json()).then((list) => {
    const dl = $("#google-families");
    dl.replaceChildren(...list.map((f) => Object.assign(document.createElement("option"), { value: f.family })));
    for (const f of list) if (!FAMILIES.has(f.family)) FAMILIES.set(f.family, { weights: f.weights, id: f.id });
    return list;
  }).catch(() => { googleList = null; return []; });
  return googleList;
}
const nearestWeight = (weights, w) => weights.reduce((a, b) => (Math.abs(b - w) < Math.abs(a - w) ? b : a), weights[0] ?? 400);
for (const row of $$('.tok[data-kind="font"]')) {
  const token = row.dataset.token;
  const fam = $(".font-family", row), weight = $(".font-weight", row), err = $(".tok-err", row);
  let last = fam.value;
  const choose = (family, w) => {
    const info = FAMILIES.get(family);
    const wt = nearestWeight(info?.weights ?? [400], w);
    const neutral = NEUTRAL.tokens[token];
    // The built-in face as the built-in theme has it: no token at all.
    if (family === "Inter" && wt === neutral.weight) set(token, undefined);
    else set(token, { google: family, weight: wt });
    err.textContent = "";
    apply({ touched: token });
  };
  fam.addEventListener("change", async () => {
    if (fam.value !== "*") { last = fam.value; return choose(fam.value, Number(weight.value)); }
    fam.value = last;
    let box = $(".font-search", row);
    if (!box) {
      box = document.createElement("div");
      box.className = "font-search";
      box.innerHTML = `<input type="text" list="google-families" placeholder="A Google Fonts family, e.g. Rubik" spellcheck="false" autocomplete="off" autocapitalize="off" enterkeyhint="go" aria-label="Google Fonts family"><button type="button" class="btn btn-ghost small">Use</button>`;
      $(".font-pick", row).after(box);
      const input = $("input", box);
      const use = async () => {
        const list = await googleFamilies();
        const f = list.find((x) => x.family.toLowerCase() === input.value.trim().toLowerCase());
        if (!f) { err.textContent = list.length ? `No Google Fonts family is called “${input.value.trim()}”.` : "The font list couldn't be loaded."; return; }
        if (![...fam.options].some((o) => o.value === f.family)) {
          let picked = $('optgroup[label="Picked"]', fam);
          if (!picked) { picked = document.createElement("optgroup"); picked.label = "Picked"; fam.insertBefore(picked, fam.lastElementChild); }
          picked.appendChild(new Option(f.family, f.family));
        }
        fam.value = last = f.family;
        box.remove();
        choose(f.family, Number(weight.value));
      };
      input.addEventListener("focus", googleFamilies, { once: true });
      input.addEventListener("keydown", (e) => { if (e.key === "Enter") use(); });
      $("button", box).addEventListener("click", use);
    }
    $("input", box).focus();
  });
  weight.addEventListener("change", () => choose(fam.value, Number(weight.value)));
}
function fontFailed(urls) {
  for (const row of $$('.tok[data-kind="font"]')) {
    const v = written(row.dataset.token).value;
    if (isFont(v) && urls.includes(previewFont(v).src)) $(".tok-err", row).textContent = `${v.google ?? v.family} ${v.weight} couldn't be fetched for the preview.`;
  }
  status("A face couldn't be fetched; the charts keep their type.");
  render(resolve(mode), null);
}

// Palettes: presets, generators, and any colour of the list.
const PRESET_PALETTES = {
  "categorical:neutral": undefined,
  "categorical:generate": { generate: "categorical", from: "$accent", n: 8 },
  "categorical:okabe": ["#0072b2", "#e69f00", "#009e73", "#cc79a7", "#56b4e9", "#d55e00", "#f0e442"],
  "categorical:tableau": ["#4e79a7", "#f28e2b", "#e15759", "#76b7b2", "#59a14f", "#edc948", "#b07aa1", "#ff9da7", "#9c755f", "#bab0ac"],
  // Expressions in a palette: they re-derive in every mode, like any colour token.
  "categorical:shades": ["$accent", "mix($accent, $paper, 0.45)", "mix($accent, $ink, 0.4)", "mix($accent, $paper, 0.7)", "mix($accent, $ink, 0.65)", "mix($accent, $paper, 0.85)"],
  "sequential:neutral": undefined,
  "sequential:generate": { generate: "sequential", from: "$accent", n: 5 },
  "sequential:viridis": { kind: "sequential", colors: ["#440154", "#3b528b", "#21918c", "#5ec962", "#fde725"] },
  "sequential:magma": { kind: "sequential", colors: ["#fcfdbf", "#fc8961", "#b73779", "#51127c", "#000004"] },
  "sequential:greens": { kind: "sequential", colors: ["#edf8e9", "#bae4b3", "#74c476", "#31a354", "#006d2c"] },
  "diverging:neutral": undefined,
  "diverging:generate": null, // from the accent to its opposite: made when picked
  "diverging:purple-green": { kind: "diverging", colors: ["#762a83", "#af8dc3", "#f7f7f7", "#7fbf7b", "#1b7837"] },
  "diverging:teal-brown": { kind: "diverging", colors: ["#01665e", "#5ab4ac", "#f5f5f5", "#d8b365", "#8c510a"] },
};
for (const b of $$("[data-pal-preset]")) {
  b.addEventListener("click", () => {
    const key = b.dataset.palPreset, token = key.split(":")[0];
    let v = clone(PRESET_PALETTES[key] ?? null) ?? undefined;
    if (key === "diverging:generate") v = { generate: "diverging", from: opposite(current?.resolved.accent ?? "#4269d0"), to: "$accent", n: 7 };
    set(token, v);
    apply({ animate: true, touched: token });
  });
}
/** A palette's colours as a list this editor can change one by one (a generator or expressions
 * become the colours they resolve to now). */
function editablePalette(token) {
  const colors = [...(current?.resolved[token] ?? [])];
  return token === "categorical" ? colors : { kind: token, colors };
}
function setPaletteColour(token, i, hex) {
  const p = editablePalette(token);
  const list = Array.isArray(p) ? p : p.colors;
  list[i] = hex;
  set(token, p, { gesture: true });
  soon(token);
}
/** One more categorical colour: the last one's lightness and chroma, a golden angle round. */
function addColour() {
  const p = editablePalette("categorical");
  const last = p[p.length - 1] ?? "#4269d0";
  const [L, C, H] = lch(last);
  p.push(fromLch(L, Math.max(C, 0.1), H + 2.39996));
  set("categorical", p);
  apply({ touched: "categorical" });
}
function dropColour() {
  const p = editablePalette("categorical");
  if (p.length <= 2) return;
  p.pop();
  set("categorical", p);
  apply({ touched: "categorical" });
}

// Presets and a brand colour.
for (const b of $$("[data-studio-preset]")) b.addEventListener("click", () => replace({ ...presetTheme(b.dataset.studioPreset), locked: theme.locked }));
let brandPaper = "white";
const brandPick = $("#brand-colour"), brandHex = $("#brand-hex");
const showBrand = (hex) => { brandPick.value = hex; brandHex.value = hex; brandPick.parentElement.style.setProperty("--c", hex); };
brandPick.addEventListener("input", () => showBrand(brandPick.value));
brandHex.addEventListener("input", () => { if (/^#[0-9a-f]{6}$/i.test(brandHex.value.trim())) showBrand(brandHex.value.trim().toLowerCase()); });
$("#brand-go").addEventListener("click", () => replace({ ...brandTheme(brandPick.value, brandPaper), locked: theme.locked }));
for (const b of $$("[data-brand-paper]")) {
  b.addEventListener("click", () => {
    brandPaper = b.dataset.brandPaper;
    for (const o of $$("[data-brand-paper]")) o.setAttribute("aria-pressed", String(o === b));
    if (theme.from === "brand") replace({ ...brandTheme(brandPick.value, brandPaper), name: theme.name, locked: theme.locked });
  });
}
$("#theme-name").addEventListener("input", (e) => {
  theme.name = e.target.value.trim().replace(/\s+/g, "-") || "my-theme";
  save();
  renderExport();
});
$("[data-studio-reset]").addEventListener("click", () => replace(blank()));
$("[data-studio-undo]").addEventListener("click", () => {
  const prev = undo.pop();
  if (!prev) return;
  theme = JSON.parse(prev);
  editing = false;
  $("[data-studio-undo]").disabled = !undo.length;
  save();
  apply({ animate: true });
});
document.addEventListener("keydown", (e) => {
  if ((e.metaKey || e.ctrlKey) && !e.shiftKey && e.key === "z" && !e.target.closest?.("input, select, textarea")) {
    e.preventDefault();
    $("[data-studio-undo]").click();
  }
});

// Modes.
for (const b of $$("[data-studio-mode]")) {
  b.addEventListener("click", () => {
    if (mode === b.dataset.studioMode) return;
    mode = b.dataset.studioMode;
    apply({ animate: true });
  });
}

// Colour vision, for the whole wall: the engine's matrices as an SVG filter.
for (const b of $$("[data-vision]")) {
  b.addEventListener("click", () => {
    wall.style.filter = b.dataset.vision ? `url(#cvd-${b.dataset.vision})` : "";
    for (const o of $$("[data-vision]")) o.setAttribute("aria-pressed", String(o === b));
  });
}

// Tabs (and, on a phone, folding the editor away).
const tabs = $$('.sp-tabs [role="tab"]');
const panel = $("#studio-panel");
function showTab(tab) {
  for (const t of tabs) {
    const on = t === tab;
    t.setAttribute("aria-selected", String(on));
    t.tabIndex = on ? 0 : -1;
    $(`#${t.getAttribute("aria-controls")}`).hidden = !on;
  }
  $("#sp-body").scrollTop = 0;
  setFold(false);
  showScope();
  if (tab.id === "tab-checks") checksSoon(0);
  if (tab.id === "tab-export") renderExport();
}
for (const t of tabs) {
  t.addEventListener("click", () => showTab(t));
  t.addEventListener("keydown", (e) => {
    const i = tabs.indexOf(t);
    const j = e.key === "ArrowRight" ? (i + 1) % tabs.length : e.key === "ArrowLeft" ? (i + tabs.length - 1) % tabs.length : -1;
    if (j < 0) return;
    e.preventDefault();
    tabs[j].focus();
    showTab(tabs[j]);
  });
}
const fold = $("[data-studio-fold]");
function setFold(on) {
  panel.classList.toggle("folded", on);
  fold.setAttribute("aria-expanded", String(!on));
  fold.setAttribute("aria-label", on ? "Show the editor" : "Hide the editor");
  fold.title = on ? "Show the editor" : "Hide the editor";
}
fold.addEventListener("click", () => setFold(!panel.classList.contains("folded")));

// ---- showing the theme in the editor ---------------------------------------------------------

const valueText = (v) => (typeof v === "string" ? v : v === undefined ? "" : JSON.stringify(v));
const SCOPE = {
  light: null,
  dark: "Dark mode: the colours you change here are for dark mode only.",
  "high-contrast": "High contrast: the colours you change here are for high contrast only.",
};

/** Which layer an edit goes to, said where colours are edited (the other tabs edit every mode). */
function showScope() {
  const scope = $("#sp-scope");
  const tab = tabs.find((t) => t.getAttribute("aria-selected") === "true")?.id;
  const note = theme.scheme === "dark" && mode !== "high-contrast" ? "Dark everywhere: light mode shows it dark too; colours go to its base." : SCOPE[mode];
  const show = !!note && ["tab-colours", "tab-palettes", "tab-maps"].includes(tab);
  scope.hidden = !show;
  scope.textContent = show ? note : "";
}

/** The choices that need no resolving — mode, preset, name, locks: shown as the page opens, so a
 * returning reader's theme is what the editor says from the first paint. */
function showChoices() {
  for (const b of $$("[data-studio-mode]")) b.setAttribute("aria-pressed", String(b.dataset.studioMode === mode));
  // The preset this theme started from (an edited one stays marked).
  for (const b of $$("[data-studio-preset]")) b.setAttribute("aria-pressed", String(theme.from?.replace(/\*$/, "") === b.dataset.studioPreset));
  const name = $("#theme-name");
  if (document.activeElement !== name) name.value = theme.name;
  for (const c of $$("[data-lock]")) c.checked = LOCKS[c.dataset.lock].every((t) => theme.locked.includes(t));
}

function render(res, touched) {
  const r = res.resolved;
  showChoices();
  showScope();
  const bad = res.errors;
  for (const row of $$(".tok[data-token]")) {
    const token = row.dataset.token, kind = row.dataset.kind;
    const own = editLayer(token);
    const isSet = token in own;
    const w = written(token);
    row.classList.toggle("set", isSet);
    $(".tok-reset", row).disabled = !isSet;
    row.title = isSet ? "" : `Inherited from ${w.by === "neutral" ? "datars/neutral" : w.by === "base" ? "your light (base) value" : `your ${w.by} value`}`;
    if (kind === "color") {
      const hex = r[token];
      const ok = isHex(hex) && !bad.has(token);
      if (ok) {
        $(".tok-sw", row).style.setProperty("--c", hex);
        const pick = $('input[type="color"]', row);
        if (document.activeElement !== pick) pick.value = hex.slice(0, 7);
      }
      const text = $(".tok-val", row);
      if (document.activeElement !== text) text.value = isSet ? valueText(own[token]) : "";
      text.placeholder = valueText(w.value);
      // Not while it's being typed: a half-written value is no mistake yet.
      const err = isSet && document.activeElement !== text ? bad.get(token) ?? "" : "";
      row.classList.toggle("bad", !!err);
      $(".tok-err", row).textContent = err ? `Doesn't resolve (${err}): the charts keep the inherited value.` : "";
    } else if (kind === "number") {
      const v = r[token];
      const range = $('input[type="range"]', row);
      if (typeof v === "number") {
        if (document.activeElement !== range || !editing) range.value = String(v);
        $("output", row).textContent = String(Math.round(v * 100) / 100);
      }
    } else if (kind === "font") {
      const v = w.value;
      const family = v.google ?? (Array.isArray(v.family) ? v.family[0] : v.family) ?? "Inter";
      const fam = $(".font-family", row), weight = $(".font-weight", row);
      if (![...fam.options].some((o) => o.value === family)) {
        let picked = $('optgroup[label="Picked"]', fam);
        if (!picked) { picked = document.createElement("optgroup"); picked.label = "Picked"; fam.insertBefore(picked, fam.lastElementChild); }
        picked.appendChild(new Option(family, family));
      }
      fam.value = family;
      const weights = FAMILIES.get(family)?.weights ?? [v.weight ?? 400];
      const wanted = weights.map(String).join();
      if ([...weight.options].map((o) => o.value).join() !== wanted) weight.replaceChildren(...weights.map((x) => new Option(String(x), String(x))));
      weight.value = String(v.weight ?? 400);
    }
  }
  renderPalettes(r);
  if (!$("#sp-export").hidden) renderExport();
  if (touched) hit(touched);
}

function renderPalettes(r) {
  // Categorical: a swatch per colour (each a colour picker), kept across renders so a picker that's
  // open stays open; then the add and remove buttons.
  const strip = $('[data-strip="categorical"]');
  const cat = (r.categorical ?? []).slice(0, 16);
  let sws = $$(".pal-sw", strip);
  while (sws.length > cat.length) sws.pop().remove();
  while (sws.length < cat.length) {
    const i = sws.length;
    const l = document.createElement("label");
    l.className = "pal-sw";
    l.innerHTML = `<input type="color" aria-label="Categorical colour ${i + 1}">`;
    const input = $("input", l);
    input.addEventListener("input", () => setPaletteColour("categorical", i, input.value));
    input.addEventListener("change", settle);
    strip.insertBefore(l, $(".pal-add", strip));
    sws.push(l);
  }
  cat.forEach((c, i) => {
    sws[i].style.setProperty("--c", c);
    const input = $("input", sws[i]);
    if (document.activeElement !== input) input.value = c.slice(0, 7);
  });
  if (!$(".pal-add", strip)) {
    const add = Object.assign(document.createElement("button"), { type: "button", className: "pal-add", textContent: "+", title: "Add a colour" });
    add.setAttribute("aria-label", "Add a colour");
    add.addEventListener("click", addColour);
    const drop = Object.assign(document.createElement("button"), { type: "button", className: "pal-add", textContent: "−", title: "Remove the last colour" });
    drop.setAttribute("aria-label", "Remove the last colour");
    drop.addEventListener("click", dropColour);
    strip.append(add, drop);
  }
  // The first eight as colour-blind readers see them, and the closest pair's ΔE.
  const eight = cat.slice(0, 8);
  for (const row of $$(".cvd-row")) {
    const kind = row.dataset.cvd;
    const seen = eight.map((c) => simulate(c, kind));
    $(".cvd-sw", row).style.background = `linear-gradient(90deg, ${seen.map((c, i) => `${c} ${(i * 100) / seen.length}% ${((i + 1) * 100) / seen.length}%`).join(", ")})`;
    const [d] = closest(seen);
    const bar = kind ? 4 : 8;
    const b = $("b", row);
    b.textContent = Number.isFinite(d) ? `ΔE ${d.toFixed(1)}` : "";
    b.className = d >= bar ? "pass" : "warn";
  }
  for (const k of ["sequential", "diverging"]) {
    const cs = r[k] ?? [];
    $(`[data-strip="${k}"]`).style.background = cs.length ? `linear-gradient(90deg, ${cs.map((c, i) => `${c} ${(i * 100) / cs.length}% ${((i + 1) * 100) / cs.length}%`).join(", ")})` : "";
  }
  for (const b of $$("[data-pal-preset]")) {
    const [token] = b.dataset.palPreset.split(":");
    const w = written(token);
    const v = PRESET_PALETTES[b.dataset.palPreset];
    const mine = w.by === "neutral" ? undefined : w.value;
    const on = b.dataset.palPreset.endsWith(":generate") ? isGen(mine) : JSON.stringify(mine) === JSON.stringify(v);
    b.setAttribute("aria-pressed", String(on));
  }
}

// ---- checks ----------------------------------------------------------------------------------

let checksTimer = 0;
function checksSoon(ms = 250) {
  clearTimeout(checksTimer);
  checksTimer = setTimeout(runChecks, ms);
}
const later = () => checksSoon();
/** The theme's checks in all three modes, on what the engine resolves (datars-theme `validate`). */
function runChecks() {
  if (!resolver.status) return checksSoon(300);
  const all = Object.fromEntries(MODES.map((m) => [m, (m === mode && current?.resolved) || resolve(m).resolved]));
  // The resolver ends on the mode on show, so its next quick resolve needn't switch.
  if (resolver.getAttribute("mode") !== layer(mode).viewMode) resolver.setAttribute("mode", layer(mode).viewMode);
  let problems = 0;
  NEUTRAL.checks.forEach((c, i) => {
    for (const m of MODES) {
      const r = all[m], cell = $(`tr[data-check="${i}"] td[data-mode="${m}"] .ck`);
      if (!cell) continue;
      let text = "—", cls = "", title = "";
      if (c.contrast) {
        const [fg, bg] = c.contrast.map((t) => r[t]);
        if (isHex(fg) && isHex(bg)) {
          const ratio = contrast(fg, bg);
          text = ratio.toFixed(1);
          cls = ratio >= c.min ? "pass" : ratio >= 3 ? "warn" : "fail";
          title = `${c.contrast[0]} ${fg} on ${c.contrast[1]} ${bg}: ${ratio.toFixed(2)}:1 (needs ${c.min}:1)`;
        }
      } else if (c.distinct) {
        const p = (r[c.distinct] ?? []).slice(0, c.first ?? Infinity);
        const views = [["normal vision", ""], ...(c.cvd ? [["deuteranopia", "deuteranopia"], ["protanopia", "protanopia"], ["tritanopia", "tritanopia"]] : [])];
        let worst = null;
        const lines = [];
        for (const [name, kind] of views) {
          const [d, a, b] = closest(p.map((x) => simulate(x, kind)));
          const bar = kind ? c.min / 2 : c.min;
          lines.push(`${name}: colours ${a + 1} and ${b + 1}, ΔE ${d.toFixed(1)} (want ≥ ${bar})`);
          if (!worst || d / bar < worst.d / worst.bar) worst = { d, bar, name };
        }
        if (worst && Number.isFinite(worst.d)) {
          text = worst.d.toFixed(1);
          cls = worst.d >= worst.bar ? "pass" : "warn";
          title = lines.join("\n");
        }
      } else if (c.monotone) {
        const ls = (r[c.monotone] ?? []).map((x) => oklab(rgba(x))[0]);
        const up = ls.every((l, j) => !j || l >= ls[j - 1]), down = ls.every((l, j) => !j || l <= ls[j - 1]);
        text = up || down ? "✓" : "✗";
        cls = up || down ? "pass" : "warn";
        title = up || down ? "Lightness only rises or falls along the ramp." : "Lightness goes up and down along the ramp: readers can't tell its order.";
      }
      cell.textContent = text;
      cell.className = `ck ${cls}`;
      cell.title = title;
      if (cls && cls !== "pass") problems++;
    }
  });
  const badge = $("#ck-badge");
  badge.hidden = !problems;
  badge.textContent = String(problems);
  badge.setAttribute("aria-label", `${problems} check${problems === 1 ? "" : "s"} not passing`);
}
// A failing contrast check jumps to its first token, in that mode.
for (const tr of $$("tr[data-check]")) {
  const c = NEUTRAL.checks[Number(tr.dataset.check)];
  if (!c.contrast) continue;
  for (const td of $$("td", tr)) {
    td.addEventListener("click", () => {
      if (td.dataset.mode !== mode) { mode = td.dataset.mode; apply({ animate: true }); }
      const tab = tabs.find((t) => $(`#${t.getAttribute("aria-controls")} [data-token="${c.contrast[0]}"]`));
      if (!tab) return;
      showTab(tab);
      const row = $(`#${tab.getAttribute("aria-controls")} [data-token="${c.contrast[0]}"]`);
      row.scrollIntoView({ block: "center" });
      $(".tok-val", row)?.focus({ preventScroll: true });
    });
  }
}

// ---- export ----------------------------------------------------------------------------------

const LOCKS = { colours: ["paper", "ink", "accent"], palettes: ["categorical", "sequential", "diverging"], fonts: ["font.title", "font.strong", "font.body", "font.number"] };
for (const c of $$("[data-lock]")) {
  c.addEventListener("change", () => {
    remember();
    const set = new Set(theme.locked);
    for (const t of LOCKS[c.dataset.lock]) c.checked ? set.add(t) : set.delete(t);
    theme.locked = [...set];
    save();
    renderExport();
  });
}
let format = "ts";
for (const b of $$("[data-export]")) {
  b.addEventListener("click", () => {
    format = b.dataset.export;
    for (const o of $$("[data-export]")) o.setAttribute("aria-pressed", String(o === b));
    renderExport();
  });
}
/** The theme as a theme file would hold it: what this studio set, nothing inherited. */
function themeJson() {
  const out = { name: theme.name, extends: "datars/neutral" };
  if (theme.scheme === "dark") out.scheme = "dark";
  out.tokens = theme.tokens;
  const modes = Object.fromEntries(Object.entries(theme.modes).filter(([, v]) => Object.keys(v).length));
  if (Object.keys(modes).length) out.modes = modes;
  if (theme.locked.length) out.locked = theme.locked;
  return out;
}
/** A value laid out as code: short lists and small objects on one line, long ones wrapped. `ts`:
 * JavaScript (bare keys, trailing commas) rather than JSON; `google`: font tokens as the SDK's
 * `font.google(…)`. */
function code(v, indent, { ts = false, google = false } = {}) {
  const pad = "  ".repeat(indent);
  const inner = (x) => code(x, indent + 1, { ts, google });
  if (google && isFont(v)) return `font.google(${JSON.stringify(v.google ?? v.family?.[0])}${v.weight && v.weight !== 400 ? `, { weight: ${v.weight} }` : ""})`;
  if (Array.isArray(v)) {
    const items = v.map(inner);
    if (items.join(", ").length < 70) return `[${items.join(", ")}]`;
    const rows = [];
    for (let i = 0; i < items.length; i += 4) rows.push(items.slice(i, i + 4).join(", "));
    return `[\n${rows.map((r) => `${pad}  ${r}`).join(",\n")}${ts ? "," : ""}\n${pad}]`;
  }
  if (v && typeof v === "object") {
    const key = (k) => (ts && /^[A-Za-z_$][\w$]*$/.test(k) ? k : JSON.stringify(k));
    const entries = Object.entries(v).map(([k, x]) => `${key(k)}: ${inner(x)}`);
    if (!entries.length) return "{}";
    const flat = `{ ${entries.join(", ")} }`;
    if (flat.length < 72 && !flat.includes("\n")) return flat;
    return `{\n${entries.map((e) => `${pad}  ${e}`).join(",\n")}${ts ? "," : ""}\n${pad}}`;
  }
  return JSON.stringify(v);
}
const camel = (s) => s.replace(/[^A-Za-z0-9]+(.)?/g, (_, c) => (c ? c.toUpperCase() : "")).replace(/^[^A-Za-z_$]/, "_$&") || "myTheme";
function exportText() {
  const t = themeJson();
  if (format === "json") return { lang: "js", text: code(t, 0) };
  if (format === "ts") {
    const fonts = JSON.stringify(t).includes('"google"');
    return { lang: "ts", text: `import { ${fonts ? "font, " : ""}theme } from "@datars/sdk";\n\nexport const ${camel(theme.name)} = theme(${code(t, 0, { ts: true, google: true })});\n\n// export default doc({ theme: ${camel(theme.name)}, … }) — or publish it for your apps.` };
  }
  const host = current?.host ?? {};
  return { lang: "js", text: `// What this page runs on every chart, in ${mode === "high-contrast" ? "high contrast" : `${mode} mode`}: a host layer\n// over any published chart (an app passes one per mode). Faces with a file to fetch.\nconst view = document.querySelector("datars-view");\nview.setAttribute("mode", ${JSON.stringify(current?.viewMode ?? mode)});\nview.setTokens(${code(host, 0, { ts: true })});` };
}
function renderExport() {
  const { lang, text } = exportText();
  const el = $("#export-code");
  el.dataset.lang = lang;
  el.textContent = text;
  highlight(el);
  const worn = wornTheme();
  const wear = $("[data-wear]");
  const on = !!worn && worn.name === theme.name && worn.code === shareCode();
  wear.textContent = on ? "Take it off" : worn ? "Wear this one instead" : "Wear it on every page";
  $("[data-wear-status]").textContent = on ? "You're wearing it." : worn ? `You're wearing “${worn.name}”.` : "";
}
async function copy(text, btn) {
  try {
    await navigator.clipboard.writeText(text);
    const was = btn.textContent;
    btn.textContent = "Copied";
    setTimeout(() => { btn.textContent = was; }, 1400);
  } catch { /* no clipboard: the text is on the page to select */ }
}
$('[data-copy="code"]').addEventListener("click", (e) => copy(exportText().text, e.currentTarget));
$('[data-copy="link"]').addEventListener("click", (e) => copy(`${location.origin}${location.pathname}#theme=${shareCode()}`, e.currentTarget));
$("[data-download]").addEventListener("click", () => {
  const a = document.createElement("a");
  a.href = URL.createObjectURL(new Blob([`${code(themeJson(), 0)}\n`], { type: "application/json" }));
  a.download = `${theme.name}.json`;
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 1000);
});
$("[data-wear]").addEventListener("click", () => {
  const worn = wornTheme();
  if (worn && worn.name === theme.name && worn.code === shareCode()) wearTheme(null);
  else {
    // Every page mode's layer and swatch, resolved now (the resolver ends on the mode on show).
    const modes = {}, swatch = {};
    for (const m of MODES) {
      const res = resolve(m);
      const tokens = hostLayer(m, res);
      // The site's own faces by path, as site.js fetches them.
      for (const [k, v] of Object.entries(tokens)) if (isFont(v) && v.src.startsWith(SITE.origin)) tokens[k] = { ...v, src: new URL(v.src).pathname };
      modes[m] = { mode: res.viewMode, tokens };
      swatch[m] = [res.resolved.accent, ...(res.resolved.categorical ?? []).slice(1, 3)];
    }
    resolve(mode);
    wearTheme({ name: theme.name, code: shareCode(), modes, swatch });
  }
  renderExport();
});

// ---- start -------------------------------------------------------------------------------------

showChoices();
apply();
