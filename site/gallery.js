// The gallery (site/pages/gallery.html): a wall of live charts the reader filters, and the lab — one
// chart, full screen, to step and scrub, restyle, remix and copy.
//
// Everything the lab does goes through <datars-view>'s public API, on a chart published like any
// other: `send("goto:…")` and `seek(position)` for the transport, the `mode` and `reduced-motion`
// attributes, `setTokens` for a brand, `setSignal` for the document's signals and `provideData` for
// its data (the edited columns replace the source; keyed marks morph to them). What the lab reads
// about a chart, it reads from what the site publishes: the bundle's manifest and its source
// document chunk (signals, data, steps), and the documents page (/gallery/documents/: the TypeScript,
// the tier sizes, the tokens it reads), fetched when the lab first opens.
import { BRANDS, highlight, lookTokens, wornTheme } from "./site.js";

const $ = (sel, el = document) => el.querySelector(sel);
const $$ = (sel, el = document) => [...el.querySelectorAll(sel)];
const root = document.documentElement;
/** The site's root (this file sits there). */
const SITE = new URL(".", import.meta.url);
const reducedQuery = matchMedia("(prefers-reduced-motion: reduce)");
const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
const cap = (s) => s.replace(/^./, (c) => c.toUpperCase());
const norm = (s) => String(s ?? "").toLowerCase().normalize("NFD").replace(/[̀-ͯ]/g, "");

// ---- the wall: search, what to show, families, features -----------------------------------------
// Two kinds of card share the wall: a chart type (a std recipe's reference figure, `.g-type`, written
// by the build's {{std:cards}}) and a story (a site chart). A family matches a chart type of that
// family, or a story that uses one; a feature only stories carry.

const cards = $$(".g-card");
const groups = $$(".g-group");
const secs = $$(".g-sec");
const query = $("#g-q");
const filter = { show: "", family: "", tags: new Set() };
const hay = new Map(cards.map((c) => [c, norm([c.id, c.dataset.words, c.dataset.tags, c.querySelector(".g-text")?.textContent].join(" "))]));
const isType = (c) => c.classList.contains("g-type");
const TOTAL = { types: cards.filter(isType).length, stories: cards.filter((c) => !isType(c)).length };

function applyFilter() {
  const words = norm(query?.value).split(/\s+/).filter(Boolean);
  const n = { types: 0, stories: 0 };
  for (const c of cards) {
    const kind = isType(c) ? "types" : "stories";
    const tags = (c.dataset.tags ?? "").split(" ");
    const ok = (!filter.show || filter.show === kind)
      && (!filter.family || (c.dataset.families ?? "").split(" ").includes(filter.family))
      && [...filter.tags].every((t) => tags.includes(t))
      && words.every((w) => hay.get(c).includes(w));
    if (c.hidden === ok) c.hidden = !ok;
    if (ok) n[kind]++;
  }
  for (const g of groups) g.hidden = !g.querySelector(".g-card:not([hidden])");
  for (const sec of secs) sec.hidden = !sec.querySelector(".g-card:not([hidden])");
  const count = $("#g-count");
  const part = (k, one, many) => {
    const label = n[k] === 1 && TOTAL[k] === 1 ? one : many;
    return n[k] === TOTAL[k] ? `<b>${n[k]}</b> ${label}` : `<b>${n[k]}</b> of ${TOTAL[k]} ${many}`;
  };
  if (count) count.innerHTML = [filter.show !== "stories" && part("types", "chart type", "chart types"), filter.show !== "types" && part("stories", "story", "stories")].filter(Boolean).join(" · ");
  $("#g-none").hidden = n.types + n.stories > 0;
}
let typing = 0;
query?.addEventListener("input", () => { clearTimeout(typing); typing = setTimeout(applyFilter, 90); });
/** After a choice in the bar, the wall starts at its top (the bar stays where it is). */
function toWall() {
  const bar = $("#g-bar");
  const body = $("#g-body");
  if (!bar || !body) return;
  const top = body.getBoundingClientRect().top + scrollY - (getComputedStyle(bar).position === "sticky" ? bar.offsetHeight + 60 : 0);
  if (scrollY > top) scrollTo({ top, behavior: "instant" });
}
$("#g-bar")?.addEventListener("click", (e) => {
  const b = e.target.closest("button");
  if (!b) return;
  const bar = $("#g-bar");
  if ("show" in b.dataset) {
    filter.show = b.dataset.show;
    for (const k of $$("[data-show]", bar)) k.setAttribute("aria-pressed", String(k === b));
  } else if ("family" in b.dataset) {
    filter.family = b.dataset.family;
    for (const k of $$("[data-family]", bar)) k.setAttribute("aria-pressed", String(k === b));
  } else if (b.dataset.tag) {
    const on = !filter.tags.has(b.dataset.tag);
    on ? filter.tags.add(b.dataset.tag) : filter.tags.delete(b.dataset.tag);
    b.setAttribute("aria-pressed", String(on));
  } else return;
  applyFilter();
  toWall();
});
document.addEventListener("click", (e) => {
  if (!e.target.closest("[data-g-clear]")) return;
  filter.show = filter.family = "";
  filter.tags.clear();
  if (query) query.value = "";
  for (const b of $$("#g-bar button")) b.setAttribute("aria-pressed", String(b.dataset.show === "" || b.dataset.family === ""));
  applyFilter();
});
// A search typed before this script ran (or restored by the browser on Back).
if (query?.value) applyFilter();

// ---- far away, a chart lets go -------------------------------------------------------------------
// A hundred-odd live charts on one page: a chart a few screens away gives its engine back (taken off
// the page, the element frees its scene, GPU device and WebGL context) and starts again, on the step
// it was on, when the reader comes back near. Its slot keeps the chart's height meanwhile, so the
// page doesn't move. (site.js mounts each slot the first time it comes near.)
const lab = $("#lab");
const parked = new WeakMap();
/** Park a chart that's far away: when the page is idle (freeing an engine mid-scroll would cost the
 * reader frames), and only if it's still far then. */
const away = new Set();
let parking = 0;
const idle = globalThis.requestIdleCallback ?? ((f) => setTimeout(f, 200));
function parkAway() {
  parking = 0;
  for (const slot of away) {
    const view = slot.querySelector(":scope > datars-view");
    if (!view) continue;
    // The slot's content box, as tall as the chart (it's a content box once the chart is gone).
    const cs = getComputedStyle(slot);
    const h = slot.clientHeight - parseFloat(cs.paddingTop) - parseFloat(cs.paddingBottom);
    if (slot.offsetParent && h > 0) slot.style.height = `${h}px`;
    const state = view.status?.state;
    if (state) view.setAttribute("state", state);
    parked.set(slot, view);
    view.remove();
  }
  away.clear();
}
const far = new IntersectionObserver((entries) => {
  if (lab?.open) return; // the wall is skipped while the lab is open: nothing is really far
  for (const e of entries) {
    const slot = e.target;
    if (!e.isIntersecting) {
      if (slot.querySelector(":scope > datars-view")) away.add(slot);
      continue;
    }
    away.delete(slot);
    if (!slot.querySelector(":scope > datars-view") && parked.has(slot)) {
      const v = parked.get(slot);
      parked.delete(slot);
      slot.append(v);
      slot.style.height = "";
    }
  }
  if (away.size && !parking) parking = idle(parkAway, { timeout: 3000 });
}, { rootMargin: "300% 0px" });
for (const c of cards) {
  const slot = c.querySelector(".chart[data-src]");
  if (slot) far.observe(slot);
}

// ---- what the lab knows about each chart ------------------------------------------------------

/** Every chart in the gallery, in page order: its card, slot and words. */
const charts = cards.map((card) => {
  const slot = card.querySelector(".chart[data-chart]");
  return {
    alias: card.id,
    card,
    slot,
    title: card.querySelector("h3")?.textContent ?? card.id,
    // A chart type: its recipe's name (the alias is lower-cased: `std-arcdiagram` is `arcDiagram`).
    recipe: card.classList.contains("g-type") ? card.querySelector("h3 code")?.textContent ?? null : null,
    kicker: card.querySelector(".g-kicker")?.textContent ?? "",
    states: $$(".caption .pills button[data-state]", card).map((b) => b.dataset.state),
  };
});
const byAlias = new Map(charts.map((c) => [c.alias, c]));

/** The chart's bundle, as published: its manifest and its source document (the T3 variant's
 * `doc` chunk: data, signals, steps — the document `datars publish` compiled). */
const sources = new Map();
function sourceOf(alias) {
  if (!sources.has(alias)) {
    const url = new URL(byAlias.get(alias).slot.dataset.src, location.href);
    const chunks = new URL(url.href.replace(/\/c\/[^/]+$/, "/chunks/"));
    sources.set(alias, (async () => {
      const manifest = await (await fetch(url)).json();
      const c = manifest.chunks.find((c) => c.kind === "doc" && !c.meta?.expanded) ?? manifest.chunks.find((c) => c.kind === "doc");
      const doc = c ? await (await fetch(new URL(c.hash.replace(":", "_"), chunks))).json() : null;
      return { manifest, doc, url: url.href };
    })().catch((e) => { sources.delete(alias); throw e; }));
  }
  return sources.get(alias);
}

/** The documents page: every chart's TypeScript, tier sizes and tokens, written by the build. */
const DOCS_URL = new URL("gallery/documents/", SITE);
let docsPage = null;
function documents() {
  docsPage ??= fetch(DOCS_URL).then((r) => (r.ok ? r.text() : Promise.reject(new Error(`${r.status}`)))).then((t) => new DOMParser().parseFromString(t, "text/html")).catch((e) => { docsPage = null; throw e; });
  return docsPage;
}
/** A chart type's reference page (/docs/std/<recipe>/): its figure's TypeScript and the theme
 * tokens the recipe reads — fetched for the one chart type the lab shows. */
const refPages = new Map();
const refUrl = (name) => new URL(`docs/std/${name}/`, SITE);
function reference(name) {
  if (!refPages.has(name)) {
    refPages.set(name, fetch(refUrl(name)).then((r) => (r.ok ? r.text() : Promise.reject(new Error(`${r.status}`)))).then((t) => new DOMParser().parseFromString(t, "text/html")).catch((e) => { refPages.delete(name); throw e; }));
  }
  return refPages.get(name);
}
/** What the lab reads about a chart lives on the documents page (stories) or its reference page. */
const aboutPage = (c) => (c.recipe ? reference(c.recipe) : documents());
/** A part of another page, adopted here: its links and media resolved against that page. */
function adopt(el, base = DOCS_URL) {
  const node = document.importNode(el, true);
  for (const a of $$("[href], [src], [poster]", node)) {
    for (const k of ["href", "src", "poster"]) {
      const v = a.getAttribute(k);
      if (v && !/^(#|[a-z]+:)/i.test(v)) a.setAttribute(k, new URL(v, base).href);
    }
  }
  return node;
}

/** What runs this chart elsewhere — the repository's own tests and measurements, nothing more. */
const IOS_PIXELS = ["votes", "riksdag", "business", "flows", "spending"];
const ANDROID_PIXELS = ["votes", "riksdag", "business", "flows", "inflation", "shapes", "warming", "dashboard"];
const NATIVE_TIMED = ["votes", "descent", "galaxy", "scatter"];
// The examples the wasm determinism test renders to the goldens (it skips documents that read an
// atlas or a data file: renewables, worlds, election).
const WASM_PIXELS = ["budget", "business", "dashboard", "descent", "flows", "galaxy", "hebrew", "inflation", "prices", "riksdag", "rio", "scatter", "serif", "shapes", "spending", "stocks", "votes", "warming"];
const IOS_BEHAVIOUR = {
  election: "the app answers its live source's requests (DatarsKit's tests)",
  spending: "an app's rows fill its data slot, and rows without its columns are refused (DatarsKit's tests)",
  budget: "its engine-drawn slider is described for VoiceOver and operable (DatarsKit's tests)",
  dashboard: "its bars can be activated without a pointer (DatarsKit's tests)",
};

// ---- the lab ------------------------------------------------------------------------------------

const stage = $("#lab-stage");
const frame = $("#lab-frame");
const pills = $("#lab-pills");
const say = $("#lab-say");
const scrub = $("#lab-scrub");
const at = $("#lab-at");
const playBtn = $("#lab-play");
const transport = $("#lab-transport");
const codeEl = $("#lab-code");
const GH = lab?.dataset.repo ?? "";

/** The lab's settings carry over from chart to chart; a chart's own remix doesn't. */
const L = {
  mode: null, // the reader's choice; null: the chart's own (dark-first charts) or the page's
  theme: null, // a key of themeChoices()
  reduced: reducedQuery.matches,
  tab: "remix",
  lang: "html",
  file: 0,
};
/** The chart on show and what the reader did to it. */
let S = null;
let opening = 0;

function modeFor(c) {
  return L.mode ?? c.slot.dataset.mode ?? (root.dataset.theme === "dark" ? "dark" : "light");
}

// The themes on offer: the reader's own look (if they made one), a theme from the studio they
// wear, the built-in theme and the site's preset brands.
function themeChoices() {
  const out = [];
  const look = ["light", "dark"].some((m) => Object.keys(lookTokens(m)).length);
  const worn = wornTheme();
  if (worn) out.push({ id: "studio", name: worn.name || "Studio theme", sw: worn.swatch?.[root.dataset.theme] ?? worn.swatch?.light ?? ["#888", "#aaa", "#ccc"] });
  else if (look) {
    const cs = getComputedStyle(root);
    out.push({ id: "look", name: "Your look", sw: ["--look-a", "--look-b", "--look-c"].map((v) => cs.getPropertyValue(v).trim() || "#888") });
  }
  out.push({ id: "datars", name: "datars", sw: ["#4269d0", "#efb118", "#ff725c"] });
  for (const id of ["newsprint", "nordic", "neon", "sunset"]) {
    const b = BRANDS[id];
    const c = b.light ?? b.dark;
    out.push({ id, name: b.name, sw: [c.accent, c.categorical[1], c.paper] });
  }
  return out;
}
/** The view mode and the token layer a theme gives a chart in `mode`. */
function themed(mode) {
  const id = L.theme;
  if (id === "studio") {
    const m = wornTheme()?.modes?.[mode];
    return { mode: m?.mode ?? mode, tokens: m?.tokens ?? {} };
  }
  if (id === "look") return { mode, tokens: lookTokens(mode) };
  const b = BRANDS[id];
  if (!b || id === "default") return { mode, tokens: {} };
  const colours = mode === "high-contrast" ? {} : mode === "dark" ? b.dark : (b.light ?? b.dark);
  return { mode, tokens: { ...b.shape, ...colours } };
}

/** The chart's box: its aspect ratio at the stage's width (its phone ratio on a narrow stage),
 * never under its minimum — and on a wide screen, no taller than leaves room for the transport. */
function boxFor(c) {
  const w = Math.max(200, stage.clientWidth || frame.clientWidth - 20 || 600);
  const d = c.slot.dataset;
  const ratio = (w < 600 && Number(d.phoneRatio)) || Number(d.aspect) || 0.6;
  let h = Math.max(Number(d.min) || 240, Math.round(w * ratio));
  if (matchMedia("(min-width: 961px)").matches) {
    const room = ($(".lab-left")?.clientHeight || innerHeight - 64) - 210;
    h = Math.min(h, Math.max(Number(d.min) || 240, room));
  } else {
    // A phone: the chart stays on screen above the controls, so it leaves them half the screen.
    h = Math.min(h, Math.max(Number(d.min) || 240, Math.round(innerHeight * 0.46)));
  }
  return h;
}

/** Fonts the themes name (`/fonts/…`) come from this site, wherever it is hosted. */
function answerFonts(view) {
  view.addEventListener("datarequest", (e) => {
    const { name, url } = e.detail;
    if (!name.startsWith("font:")) return;
    const u = new URL(url, location.href);
    const m = u.origin === location.origin && u.pathname.match(/\/fonts\/([^/]+\.(?:ttf|otf|woff2?))$/);
    if (!m) return;
    e.preventDefault();
    e.detail.respond(fetch(new URL(`fonts/${m[1]}`, SITE)).then((r) => r.arrayBuffer()).then((b) => new Uint8Array(b)));
  });
}
/** The election night's live source, replayed as the wall's card replays it (data-feed-for). */
function answerFeed(view, c) {
  const feed = c.card.querySelector("[data-feed-for]");
  if (!feed) return;
  const count = Number(feed.dataset.feedCount) || 1;
  let n = 0, held = 0;
  view.addEventListener("datarequest", (e) => {
    if (e.detail.name !== feed.dataset.feedSource) return;
    if (n >= count && ++held > 3) { n = 0; held = 0; }
    n = Math.min(n + 1, count);
    e.preventDefault();
    e.detail.respond(fetch(new URL(`${String(n).padStart(2, "0")}.json`, new URL(feed.dataset.src, location.href))).then((r) => r.text()));
  });
}

function makeView(c, state) {
  const v = document.createElement("datars-view");
  v.setAttribute("src", c.slot.dataset.src);
  const look = themed(modeFor(c));
  v.setAttribute("mode", look.mode);
  v.setAttribute("no-controls", "");
  v.setAttribute("aria-label", c.slot.dataset.label ?? c.title);
  if (state) v.setAttribute("state", state);
  if (L.reduced !== reducedQuery.matches) v.setAttribute("reduced-motion", L.reduced ? "reduce" : "no-preference");
  if (Object.keys(look.tokens).length) v.setTokens(look.tokens);
  answerFonts(v);
  answerFeed(v, c);
  v.setAttribute("height", String(boxFor(c)));
  return v;
}

/** Resolves at the view's first frame (its first `state` event), or after a while regardless. */
function firstFrame(v) {
  return new Promise((resolve) => {
    const t = setTimeout(resolve, 5000);
    v.addEventListener("state", () => { clearTimeout(t); resolve(); }, { once: true });
  });
}

// ---- opening, switching, closing ----

let pushed = false;
let lastFocus = null;

function visibleCharts() {
  return charts.filter((c) => !c.card.hidden);
}

function open(alias, { push = true } = {}) {
  const c = byAlias.get(alias);
  if (!c || !lab) return;
  if (!lab.open) {
    lastFocus = document.activeElement;
    lookUI();
    tab(L.tab);
    // The wall behind draws nothing while the lab is open, and keeps its height.
    const body = $("#g-body");
    if (body) {
      body.style.containIntrinsicSize = `auto ${body.offsetWidth}px auto ${body.offsetHeight}px`;
      body.classList.add("g-away");
    }
    root.style.overflow = "hidden";
    lab.showModal();
    if (push) {
      history.pushState({ lab: alias }, "", `#lab=${alias}`);
      pushed = true;
    }
  } else if (push) {
    history.replaceState({ lab: alias }, "", `#lab=${alias}`);
  }
  // Where the card's chart is now: the lab opens on the same step.
  const cardView = c.slot.querySelector("datars-view");
  const state = cardView?.status?.state ?? null;
  show(c, state);
}

function close() {
  if (!lab?.open) return;
  lab.close();
  teardown();
}
/** Put the wall back (after the close button, Escape or Back). */
function teardown() {
  if (lab?.open || !S) return;
  stopPlay();
  opening++;
  for (const v of $$("datars-view", stage)) v.remove();
  S = null;
  root.style.overflow = "";
  const body = $("#g-body");
  body?.classList.remove("g-away");
  if (body) body.style.containIntrinsicSize = "";
  if (pushed) {
    pushed = false;
    if (history.state?.lab) history.back();
  } else if (location.hash.startsWith("#lab=")) {
    history.replaceState(null, "", location.pathname + location.search);
  }
  lastFocus?.focus?.({ preventScroll: true });
}
lab?.addEventListener("close", teardown);
addEventListener("popstate", () => {
  const m = location.hash.match(/^#lab=([a-z0-9-]+)$/);
  if (m && byAlias.has(m[1])) {
    if (S?.c.alias !== m[1]) open(m[1], { push: false });
  } else if (lab?.open) {
    pushed = false; // Back took the lab's entry already
    close();
  }
});

async function show(c, state) {
  const gen = ++opening;
  stopPlay();
  const old = stage.querySelector("datars-view:not(.lab-next)");
  for (const v of $$("datars-view.lab-next", stage)) v.remove();
  S = { c, view: null, src: null, data: new Map(), signals: new Map(), status: null, state: state ?? c.states[0] ?? null, index: Math.max(0, c.states.indexOf(state)), seed: 1 };
  header(c);
  stepsUI(c.states, S.index);
  remixUI(null);
  codeUI();
  factsUI();
  const v = makeView(c, state && state !== c.states[0] ? state : null);
  wire(v);
  if (old) {
    v.classList.add("lab-next");
    stage.append(v);
  } else {
    stage.classList.add("pending");
    stage.append(v);
  }
  sourceOf(c.alias).then((src) => {
    if (gen !== opening) return;
    S.src = src;
    remixUI(src.doc);
    codeUI();
    factsUI();
  }).catch(() => { if (gen === opening) remixUI(false); });
  await firstFrame(v);
  if (gen !== opening) return;
  if (old) old.remove();
  v.classList.remove("lab-next");
  stage.classList.remove("pending");
  S.view = v;
  paper();
  // The chart's own controls (a slider's range) are known now: the signals can say so.
  if (S.src?.doc) signalsUI(S.src.doc);
  // A neighbour's bundle, ahead of the reader stepping to it.
  const list = visibleCharts();
  const i = list.indexOf(c);
  const next = list[(i + 1) % list.length];
  if (next) sourceOf(next.alias).catch(() => {});
}

function header(c) {
  $("#lab-title").textContent = c.title;
  $("#lab-kicker").textContent = c.kicker;
  const list = visibleCharts();
  const i = list.indexOf(c);
  $("#lab-pos").textContent = i >= 0 ? `${i + 1} / ${list.length}` : "";
  $$("[data-lab-go]").forEach((b) => { b.disabled = list.length < 2; });
}

$$("[data-lab-go]").forEach((b) => b.addEventListener("click", () => go(Number(b.dataset.labGo))));
function go(d) {
  if (!S) return;
  const list = visibleCharts();
  const i = list.indexOf(S.c);
  const next = list[(i + d + list.length) % list.length];
  if (next && next !== S.c) open(next.alias);
}
$("[data-lab-close]")?.addEventListener("click", close);
// Escape belongs to a focused chart (it goes back out of a chapter); elsewhere it closes the lab.
lab?.addEventListener("cancel", (e) => { if (document.activeElement?.closest?.("#lab-stage")) e.preventDefault(); });
lab?.addEventListener("keydown", (e) => {
  // Escape closes (the dialog's own); brackets step through the charts. The chart takes its own
  // keys (arrows step it) when it has the focus.
  if (e.target.closest("input, textarea, select")) return;
  if (e.key === "]") go(1);
  if (e.key === "[") go(-1);
});

// Open from a card (and fetch what the lab will show as soon as the reader points at it).
document.addEventListener("click", (e) => {
  const b = e.target.closest("[data-lab]");
  if (!b) return;
  e.preventDefault();
  open(b.dataset.lab);
});
const warm = (e) => {
  const b = e.target.closest?.("[data-lab]");
  if (!b) return;
  sourceOf(b.dataset.lab).catch(() => {});
  const c = byAlias.get(b.dataset.lab);
  if (c) aboutPage(c).catch(() => {});
};
document.addEventListener("pointerover", warm, { passive: true });
document.addEventListener("focusin", warm);
document.addEventListener("pointerdown", warm, { passive: true });

// ---- the transport: steps, play, scrub ----

function stepsUI(states, index) {
  const single = states.length < 2;
  transport.classList.toggle("single", single);
  scrub.max = String(Math.max(1, states.length - 1) * 1000);
  scrub.value = String(index * 1000);
  scrub.disabled = single;
  playBtn.disabled = single;
  const shown = single ? [] : states;
  if (pills.childElementCount !== shown.length || shown.some((s, i) => pills.children[i]?.dataset.state !== s)) {
    pills.replaceChildren(...shown.map((name) => {
      const b = document.createElement("button");
      b.type = "button";
      b.dataset.state = name;
      b.textContent = cap(name);
      return b;
    }));
  }
  [...pills.children].forEach((b, i) => b.setAttribute("aria-pressed", String(i === index)));
  at.textContent = single ? "Interactive: hover, click, drag" : `${index + 1} of ${states.length}`;
  if (single) say.replaceChildren();
}
pills.addEventListener("click", (e) => {
  const b = e.target.closest("button[data-state]");
  if (!b || !S?.view) return;
  stopPlay();
  S.view.send(`goto:${b.dataset.state}`);
});
scrub.addEventListener("input", () => {
  if (!S?.view) return;
  stopPlay();
  const pos = Number(scrub.value) / 1000;
  S.scrubbing = true;
  S.view.seek(pos);
  const n = S.c.states;
  const i = Math.min(n.length - 1, Math.floor(pos));
  const f = pos - i;
  at.textContent = f > 0.001 && i < n.length - 1 ? `${cap(n[i])} → ${cap(n[i + 1])} · ${f.toFixed(2)}` : `${i + 1} of ${n.length}`;
});
scrub.addEventListener("change", () => {
  if (!S?.view) return;
  S.scrubbing = false;
  // Let go between two steps: carry on to the nearer one.
  const pos = Number(scrub.value) / 1000;
  const to = S.c.states[Math.round(pos)];
  if (to) S.view.send(`goto:${to}`);
});

let playTimer = 0;
function stopPlay() {
  clearInterval(playTimer);
  playTimer = 0;
  playBtn.setAttribute("aria-pressed", "false");
  playBtn.setAttribute("aria-label", "Play the story");
}
playBtn.addEventListener("click", () => {
  if (playTimer) return stopPlay();
  if (!S?.view || S.c.states.length < 2) return;
  playBtn.setAttribute("aria-pressed", "true");
  playBtn.setAttribute("aria-label", "Pause");
  const step = () => {
    if (!S?.view || document.hidden) return;
    const n = S.c.states;
    S.view.send(`goto:${n[(S.index + 1) % n.length]}`);
  };
  step();
  playTimer = setInterval(step, 3600);
});

/** Follow the chart: its step, narration, the signals it set, its paper. */
function wire(v) {
  v.addEventListener("state", (e) => {
    if (!S || (S.view !== v && !v.classList.contains("lab-next"))) return;
    const d = e.detail ?? {};
    S.status = d;
    if (Array.isArray(d.states) && d.states.length && d.states.join() !== S.c.states.join()) S.c.states = d.states;
    S.index = d.index ?? 0;
    S.state = d.state ?? S.c.states[S.index] ?? null;
    if (!S.scrubbing) stepsUI(S.c.states, S.index);
    const n = d.narrationDrawn ? {} : d.narration ?? {};
    const text = [n.title, n.text].filter(Boolean).join(" — ");
    if (say.textContent !== text) {
      say.replaceChildren();
      if (n.title) say.appendChild(Object.assign(document.createElement("b"), { textContent: n.title }));
      if (n.text) say.appendChild(document.createTextNode(`${n.title ? " — " : ""}${n.text}`));
    }
    // A step sets its signals: what the reader set by hand gives way to it.
    const set = S.src?.doc?.program?.states?.find((s) => s.name === S.state)?.set ?? {};
    for (const k of Object.keys(set)) S.signals.delete(k);
    signalValues();
    codeUI();
  });
  // The engine's own controls (a slider, a select drawn on the canvas) change signals too.
  v.addEventListener("pointerup", () => setTimeout(() => { if (S?.view === v) { S.status = v.status ?? S.status; signalValues(); } }, 60));
}

/** The frame around the chart is the chart's paper, so a dark chart has no light rim. */
function paper() {
  const p = S?.view?.status?.tokens?.tokens?.paper;
  frame.style.setProperty("--lab-paper", typeof p === "string" ? p : "");
}

// ---- look: mode, theme, reduced motion ----

function restyle() {
  if (!S) return;
  const look = themed(modeFor(S.c));
  for (const v of $$("datars-view", stage)) {
    v.setAttribute("mode", look.mode);
    v.setTokens(look.tokens);
  }
  lookUI();
  codeUI();
  requestAnimationFrame(paper);
}
function lookUI() {
  const mode = S ? modeFor(S.c) : L.mode ?? root.dataset.theme;
  for (const b of $$("[data-lab-mode]")) b.setAttribute("aria-pressed", String(b.dataset.labMode === mode));
  const box = $("#lab-themes");
  const list = themeChoices();
  if (!list.some((t) => t.id === L.theme)) L.theme = list[0].id === "studio" || list[0].id === "look" ? list[0].id : "datars";
  const key = list.map((t) => t.id).join();
  if (box.dataset.key !== key) {
    box.dataset.key = key;
    box.innerHTML = list.map((t) => `<button type="button" data-lab-theme="${t.id}"><span class="sw" style="--a:${esc(t.sw[0])};--b:${esc(t.sw[1])};--c:${esc(t.sw[2])}"></span>${esc(t.name)}</button>`).join("");
  }
  for (const b of $$("[data-lab-theme]", box)) b.setAttribute("aria-pressed", String(b.dataset.labTheme === L.theme));
  $("#lab-reduced").checked = L.reduced;
}
$$("[data-lab-mode]").forEach((b) => b.addEventListener("click", () => { L.mode = b.dataset.labMode; restyle(); }));
$("#lab-themes")?.addEventListener("click", (e) => {
  const b = e.target.closest("[data-lab-theme]");
  if (!b) return;
  L.theme = b.dataset.labTheme;
  restyle();
});
$("#lab-reduced")?.addEventListener("change", (e) => {
  L.reduced = e.target.checked;
  for (const v of $$("datars-view", stage)) {
    if (L.reduced === reducedQuery.matches) v.removeAttribute("reduced-motion");
    else v.setAttribute("reduced-motion", L.reduced ? "reduce" : "no-preference");
  }
});

// Keep the chart's box as the window changes.
if (stage) {
  new ResizeObserver(() => {
    if (!S) return;
    const h = String(boxFor(S.c));
    for (const v of $$("datars-view", stage)) if (v.getAttribute("height") !== h) v.setAttribute("height", h);
  }).observe(stage);
}

// ---- tabs ----

const tabs = $$(".lab-tabs [role=tab]");
function tab(id) {
  L.tab = id;
  for (const t of tabs) {
    const on = t.id === `lt-${id}`;
    t.setAttribute("aria-selected", String(on));
    t.tabIndex = on ? 0 : -1;
    $(`#${t.getAttribute("aria-controls")}`).hidden = !on;
  }
  if (id === "code") codeUI();
  if (id === "facts") factsUI();
}
tabs.forEach((t, i) => {
  t.addEventListener("click", () => tab(t.id.slice(3)));
  t.addEventListener("keydown", (e) => {
    const d = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
    if (!d) return;
    const n = tabs[(i + d + tabs.length) % tabs.length];
    n.focus();
    tab(n.id.slice(3));
  });
});

// ---- remix: signals ----

/** Signals a reader can't sensibly set by hand: a map's camera bounds, the galaxy's view box. */
const HIDDEN_SIGNALS = { descent: ["bounds"], rio: ["bounds", "minLabel"], galaxy: ["bx0", "bx1", "by0", "by1"], renewables: ["focus"], worlds: ["spin"] };
/** Key sets whose possible keys are a column of the chart's data. */
const KEYS_FROM = { dashboard: { selected: ["sales", "region"] } };

function signalSpecs(doc, alias, status) {
  const out = [];
  const states = doc?.program?.states ?? [];
  for (const [name, s] of Object.entries(doc?.signals ?? {})) {
    if ((HIDDEN_SIGNALS[alias] ?? []).includes(name)) continue;
    const set = states.map((st) => st.set?.[name]).filter((v) => v !== undefined);
    const ctl = (status?.controls ?? []).find((c) => c.signal === name);
    if (s.type === "str") {
      const opts = [...new Set([s.default, ...set, ...(ctl?.options ?? []).map((o) => o.value)].filter((v) => typeof v === "string"))];
      if (opts.length > 1) out.push({ name, type: s.type, kind: "choice", opts, def: s.default });
    } else if (s.type === "num") {
      if (ctl?.kind === "slider") out.push({ name, type: s.type, kind: "range", min: ctl.min, max: ctl.max, step: ctl.step || (ctl.max - ctl.min) / 100, def: s.default });
      else {
        const vals = [s.default, ...set].filter((v) => typeof v === "number");
        const lo = Math.min(...vals), hi = Math.max(...vals);
        if (vals.length && hi > lo) out.push({ name, type: s.type, kind: "range", min: lo, max: hi, step: vals.every(Number.isInteger) ? 1 : (hi - lo) / 100, def: s.default });
      }
    } else if (s.type === "bool") {
      out.push({ name, type: s.type, kind: "bool", def: !!s.default });
    } else if (s.type === "keyset") {
      const from = KEYS_FROM[alias]?.[name];
      const col = from && (doc.data?.[from[0]]?.values?.[from[1]]);
      const keys = [...new Set([...(s.default ?? []), ...set.flat(), ...(col ?? [])])].filter((k) => typeof k === "string");
      if (keys.length > 1) out.push({ name, type: s.type, kind: "keys", keys, def: s.default ?? [] });
    }
  }
  return out;
}
/** A signal's value now: what the reader set here, else what the step set, else what the chart's
 * own control shows, else the document's default. */
function signalNow(spec) {
  if (S.signals.has(spec.name)) return S.signals.get(spec.name);
  const set = S.src?.doc?.program?.states?.find((s) => s.name === S.state)?.set;
  if (set && spec.name in set) return set[spec.name];
  const ctl = (S.view?.status?.controls ?? S.status?.controls ?? []).find((c) => c.signal === spec.name);
  if (ctl) return ctl.kind === "slider" ? ctl.value : ctl.current ?? spec.def;
  return spec.def;
}
const fmt = (v) => (typeof v === "number" ? (Math.abs(v) >= 1000 ? Math.round(v).toLocaleString("en") : String(Math.round(v * 100) / 100)) : String(v));

function signalsUI(doc) {
  const box = $("#rx-signals");
  if (!S || !doc) return void box.replaceChildren();
  const specs = signalSpecs(doc, S.c.alias, S.view?.status ?? S.status);
  S.specs = specs;
  if (!specs.length) return void box.replaceChildren();
  box.innerHTML = `<h3>Signals <small><code>view.setSignal()</code></small></h3>` + specs.map((s) => {
    const head = `<div class="rx-sig-h"><code>${esc(s.name)}</code><output data-out="${esc(s.name)}"></output></div>`;
    if (s.kind === "choice") return `<div class="rx-sig" data-sig="${esc(s.name)}">${head}<div class="rx-opts">${s.opts.map((o) => `<button type="button" data-v="${esc(o)}">${esc(o)}</button>`).join("")}</div></div>`;
    if (s.kind === "keys") return `<div class="rx-sig" data-sig="${esc(s.name)}">${head}<div class="rx-opts" data-multi>${s.keys.map((o) => `<button type="button" data-v="${esc(o)}">${esc(o)}</button>`).join("")}</div></div>`;
    if (s.kind === "bool") return `<div class="rx-sig" data-sig="${esc(s.name)}">${head}<div class="rx-opts"><button type="button" data-v="false">false</button><button type="button" data-v="true">true</button></div></div>`;
    return `<div class="rx-sig" data-sig="${esc(s.name)}">${head}<input type="range" min="${s.min}" max="${s.max}" step="${s.step}" aria-label="${esc(s.name)}"></div>`;
  }).join("") + `<p class="fine">The document's signals, as the page sets them. A step sets some of them itself; the chart's own controls set the others.</p>`;
  signalValues();
}
function signalValues() {
  const box = $("#rx-signals");
  if (!S?.specs) return;
  for (const s of S.specs) {
    const el = box.querySelector(`[data-sig="${CSS.escape(s.name)}"]`);
    if (!el) continue;
    const v = signalNow(s);
    const out = el.querySelector("output");
    const text = s.kind === "keys" ? `${(v ?? []).length} of ${s.keys.length}` : fmt(v);
    if (out.textContent !== text) out.textContent = text;
    if (s.kind === "range") {
      const r = el.querySelector("input");
      if (document.activeElement !== r && Number(r.value) !== Number(v)) r.value = String(v);
    } else {
      for (const b of el.querySelectorAll("button")) {
        const on = s.kind === "keys" ? (v ?? []).includes(b.dataset.v) : String(v) === b.dataset.v;
        b.setAttribute("aria-pressed", String(on));
      }
    }
  }
}
$("#rx-signals")?.addEventListener("click", (e) => {
  const b = e.target.closest("button[data-v]");
  const el = b?.closest("[data-sig]");
  if (!b || !el || !S?.view) return;
  const s = S.specs.find((x) => x.name === el.dataset.sig);
  let v = b.dataset.v;
  if (s.kind === "bool") v = v === "true";
  if (s.kind === "keys") {
    const now = new Set(signalNow(s) ?? []);
    now.has(v) ? now.delete(v) : now.add(v);
    v = s.keys.filter((k) => now.has(k));
  }
  setSignal(s.name, v);
});
$("#rx-signals")?.addEventListener("input", (e) => {
  const el = e.target.closest("[data-sig]");
  if (!el || e.target.type !== "range" || !S?.view) return;
  setSignal(el.dataset.sig, Number(e.target.value));
});
function setSignal(name, v) {
  S.signals.set(name, v);
  S.view.setSignal(name, v);
  signalValues();
  codeUI();
}

// ---- remix: data ----

/** Columns that place a row (time, position, identity) rather than measure it: kept as they are. */
const PLACES = /^(year|day|date|month|week|quarter|x|lat|lon|lng|id|index|g|series|ticker|side)$/i;
const TIMES = /^(year|day|date|month|week|x)$/i;

/** A source's rows as columns, from `values` (columns), `sample` or `rows` (records or columns). */
function columnsOf(s) {
  const v = s.values ?? s.sample ?? s.rows;
  if (!v) return null;
  if (Array.isArray(v)) {
    if (!v.length || typeof v[0] !== "object") return null;
    const cols = {};
    for (const k of Object.keys(v[0])) cols[k] = v.map((r) => r[k]);
    return cols;
  }
  return typeof v === "object" && Object.values(v).every(Array.isArray) ? v : null;
}
function dataSources(doc) {
  const out = [];
  for (const [name, s] of Object.entries(doc?.data ?? {})) {
    const cols = columnsOf(s);
    if (!cols) continue;
    const names = Object.keys(cols);
    const n = cols[names[0]]?.length ?? 0;
    if (!n) continue;
    const key = [s.key ?? []].flat();
    const values = names.filter((k) => !key.includes(k) && !PLACES.test(k) && cols[k].every((x) => typeof x === "number"));
    if (!values.length) continue;
    out.push({ name, key, names, n, values, slot: !!s.slot || ("sample" in s && !s.values), time: names.find((k) => TIMES.test(k)) ?? null, orig: structuredClone(cols), cols: structuredClone(cols), off: new Set() });
  }
  return out;
}
/** Accounts the data page hands the spending chart's slot (the build publishes them). */
const ACCOUNTS = { spending: [["Sample", null], ["This account", "data/spending/this.json"], ["Another account", "data/spending/other.json"]] };

function remixUI(doc) {
  const box = $("#rx-data");
  if (!S) return;
  if (doc === null) {
    $("#rx-signals").replaceChildren();
    box.innerHTML = `<p class="fine">Reading the chart's document…</p>`;
    return;
  }
  if (doc === false) {
    box.innerHTML = `<p class="fine">This chart's document couldn't be read.</p>`;
    return;
  }
  signalsUI(doc);
  S.sources = dataSources(doc);
  S.source = S.sources[0]?.name ?? null;
  dataUI();
}
function dataUI() {
  const box = $("#rx-data");
  const list = S.sources ?? [];
  if (!list.length) {
    const why = S.c.alias === "galaxy" ? "Its four million stars are an archive read by range, not rows in the document: zoom with the wheel or a pinch instead."
      : S.c.alias === "election" ? "Its rows arrive from a live source the page answers — here, the night's snapshots, one every two seconds."
      : "Its data isn't in the document as rows.";
    box.innerHTML = `<h3>Data</h3><p class="rx-none">${why}</p>`;
    return;
  }
  const src = list.find((s) => s.name === S.source) ?? list[0];
  const accounts = ACCOUNTS[S.c.alias];
  const max = 40;
  const shown = Math.min(src.n, src.n > max ? 12 : src.n);
  const head = `<tr><th scope="col" aria-label="In the data"></th>${src.names.map((k) => `<th scope="col"${src.values.includes(k) ? ' class="num"' : ""}>${esc(k)}</th>`).join("")}</tr>`;
  const rows = [];
  for (let i = 0; i < shown; i++) {
    rows.push(`<tr data-i="${i}"${src.off.has(i) ? ' class="off"' : ""}><td><input type="checkbox" data-row="${i}" ${src.off.has(i) ? "" : "checked"} aria-label="Row ${i + 1} in the data"${src.n > max ? " disabled" : ""}></td>${src.names.map((k) => src.values.includes(k)
      ? `<td class="num"><input class="rx-num${src.cols[k][i] !== src.orig[k][i] ? " changed" : ""}" type="text" inputmode="decimal" spellcheck="false" autocomplete="off" data-col="${esc(k)}" data-i="${i}" value="${src.cols[k][i]}" aria-label="${esc(k)}, row ${i + 1}"></td>`
      : `<td title="${esc(src.cols[k][i])}">${esc(src.cols[k][i])}</td>`).join("")}</tr>`);
  }
  if (src.n > shown) rows.push(`<tr class="rx-more"><td colspan="${src.names.length + 1}">and ${(src.n - shown).toLocaleString("en")} more rows — Shuffle changes them all</td></tr>`);
  const changed = src.off.size > 0 || src.values.some((k) => src.cols[k].some((v, i) => v !== src.orig[k][i]));
  box.innerHTML = `<h3>Data <small><code>view.provideData()</code></small></h3>
    ${list.length > 1 ? `<div class="seg seg-small rx-sources" role="group" aria-label="Source">${list.map((s) => `<button type="button" data-src="${esc(s.name)}" aria-pressed="${s === src}">${esc(s.name)}</button>`).join("")}</div>` : ""}
    ${accounts ? `<div class="rx-opts rx-sources" role="group" aria-label="Hand it an account">${accounts.map(([label, url], i) => `<button type="button" data-account="${i}" aria-pressed="${(S.account ?? 0) === i}">${esc(label)}</button>`).join("")}</div>` : ""}
    <div class="rx-tools">
      <button type="button" class="btn btn-primary small" data-rx="shuffle"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 7h3.5c4 0 6 10 10 10H21M3 17h3.5c1.6 0 2.8-1.6 3.9-3.6M13.6 9.6C14.7 7.6 15.9 7 17.5 7H21M18 4l3 3-3 3M18 14l3 3-3 3"/></svg>Shuffle</button>
      <button type="button" class="btn btn-ghost small" data-rx="sort"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 6h10M4 12h7M4 18h4M17 5v14M14 16l3 3 3-3"/></svg>Sort</button>
      <button type="button" class="btn btn-ghost small" data-rx="reset"${changed || S.account ? "" : " disabled"}>Reset</button>
    </div>
    <div class="rx-table-wrap"><table class="rx-table"><thead>${head}</thead><tbody>${rows.join("")}</tbody></table></div>
    <p class="fine">${src.slot ? "A data slot: the published chart ships a sample, and the app hands in each user's rows on the device." : `The <code>${esc(src.name)}</code> source as the document declares it. Change a number, untick a row, shuffle or sort: the page hands the chart the new rows and keyed marks morph to them.`}</p>`;
}
const rng = (seed) => () => { seed |= 0; seed = (seed + 0x6d2b79f5) | 0; let t = Math.imul(seed ^ (seed >>> 15), 1 | seed); t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
function provide(src) {
  const keep = [...Array(src.n).keys()].filter((i) => !src.off.has(i));
  const cols = {};
  for (const k of src.names) cols[k] = keep.map((i) => src.cols[k][i]);
  try {
    S.view?.provideData(src.name, cols);
    S.data.set(src.name, { cols, n: keep.length });
  } catch (e) {
    console.warn("datars: rows refused", e);
  }
  codeUI();
}
$("#rx-data")?.addEventListener("click", async (e) => {
  if (!S) return;
  const b = e.target.closest("button");
  const src = S.sources?.find((s) => s.name === S.source) ?? S.sources?.[0];
  if (!b || !src) return;
  if (b.dataset.src) { S.source = b.dataset.src; return dataUI(); }
  if (b.dataset.account) {
    const [, url] = ACCOUNTS[S.c.alias][Number(b.dataset.account)];
    S.account = Number(b.dataset.account);
    const rows = url ? await (await fetch(new URL(url, SITE))).json() : null;
    const cols = rows ? columnsOf({ rows }) : structuredClone(src.orig);
    if (!cols) return;
    src.cols = cols;
    src.n = cols[src.names[0]].length;
    src.off.clear();
    if (!url) src.orig = structuredClone(cols);
    provide(src);
    return dataUI();
  }
  const rx = b.dataset.rx;
  if (rx === "shuffle") {
    // A time series gets a new course: a smooth swell over its rows (one factor per row, so a
    // candle's open, high, low and close stay a candle), each column at the precision it was
    // written with. Anything else trades its measures between rows — the same values, the same
    // total and scale, each landing on another key.
    const r = rng(++S.seed * 7919);
    if (src.time) {
      const k = 1 + Math.floor(r() * 3), phase = r() * Math.PI * 2, amp = 0.25 + r() * 0.25;
      const places = Object.fromEntries(src.values.map((c) => [c, Math.min(4, Math.max(0, ...src.orig[c].slice(0, 200).map((v) => (String(v).split(".")[1] ?? "").length)))]));
      for (let i = 0; i < src.n; i++) {
        const f = 1 + amp * Math.sin((i / src.n) * Math.PI * 2 * k + phase) + (r() - 0.5) * 0.06;
        for (const c of src.values) {
          const p = 10 ** places[c];
          src.cols[c][i] = Math.round(src.orig[c][i] * f * p) / p;
        }
      }
    } else {
      const order = [...Array(src.n).keys()];
      for (let i = order.length - 1; i > 0; i--) {
        const j = Math.floor(r() * (i + 1));
        [order[i], order[j]] = [order[j], order[i]];
      }
      for (const c of src.values) src.cols[c] = order.map((i) => src.orig[c][i]);
    }
  } else if (rx === "sort") {
    // Rows in order of the first measure, largest first (a chart that draws in data order re-sorts).
    const c = src.values[0];
    const order = [...Array(src.n).keys()].sort((a, b2) => src.cols[c][b2] - src.cols[c][a]);
    for (const k of src.names) { const col = src.cols[k]; src.cols[k] = order.map((i) => col[i]); const o = src.orig[k]; src.orig[k] = order.map((i) => o[i]); }
    src.off = new Set([...src.off].map((i) => order.indexOf(i)));
  } else if (rx === "reset") {
    if (S.account) {
      S.account = 0;
      return $("#rx-data [data-account='0']")?.click();
    }
    src.cols = structuredClone(src.orig);
    src.off.clear();
  } else return;
  provide(src);
  dataUI();
});
let editTimer = 0;
$("#rx-data")?.addEventListener("input", (e) => {
  const t = e.target;
  const src = S?.sources?.find((s) => s.name === S.source) ?? S?.sources?.[0];
  if (!src) return;
  if (t.dataset.row !== undefined) {
    const i = Number(t.dataset.row);
    t.checked ? src.off.delete(i) : src.off.add(i);
    t.closest("tr").classList.toggle("off", !t.checked);
    provide(src);
    $("#rx-data [data-rx=reset]").disabled = false;
    return;
  }
  const num = Number(String(t.value).trim().replace(",", ".").replace(/\s/g, ""));
  if (t.dataset.col && t.value.trim() !== "" && Number.isFinite(num)) {
    src.cols[t.dataset.col][Number(t.dataset.i)] = num;
    t.classList.toggle("changed", num !== src.orig[t.dataset.col][Number(t.dataset.i)]);
    $("#rx-data [data-rx=reset]").disabled = false;
    clearTimeout(editTimer);
    editTimer = setTimeout(() => provide(src), 140);
  }
});

// ---- code: the same chart, as this page now shows it, for each platform ----

/** A value as a JavaScript literal, wrapped to the width of the panel. */
function js(v, ind = "") {
  if (Array.isArray(v)) {
    const parts = v.map((x) => js(x, `${ind}  `));
    const one = `[${parts.join(", ")}]`;
    if (one.length + ind.length <= 78 && !one.includes("\n")) return one;
    const lines = [];
    let line = "";
    for (const p of parts) {
      if (line && (line.length + p.length + 2 + ind.length > 76 || p.includes("\n"))) { lines.push(line); line = ""; }
      line += (line ? " " : "") + p + ",";
    }
    if (line) lines.push(line);
    return `[\n${lines.map((l) => `${ind}  ${l}`).join("\n")}\n${ind}]`;
  }
  if (v && typeof v === "object") {
    const ents = Object.entries(v).map(([k, x]) => `${/^[A-Za-z_$][\w$]*$/.test(k) ? k : JSON.stringify(k)}: ${js(x, `${ind}  `)}`);
    if (!ents.length) return "{}";
    const one = `{ ${ents.join(", ")} }`;
    if (one.length + ind.length <= 78 && !one.includes("\n")) return one;
    return `{\n${ents.map((e) => `${ind}  ${e},`).join("\n")}\n${ind}}`;
  }
  return JSON.stringify(v);
}
/** A value as a Swift literal. */
function swift(v, ind = "") {
  if (Array.isArray(v)) {
    const parts = v.map((x) => swift(x, `${ind}    `));
    const one = `[${parts.join(", ")}]`;
    return one.length + ind.length <= 78 && !one.includes("\n") ? one : `[\n${parts.map((p) => `${ind}    ${p},`).join("\n")}\n${ind}]`;
  }
  if (v && typeof v === "object") {
    const ents = Object.entries(v).map(([k, x]) => `${JSON.stringify(k)}: ${swift(x, `${ind}    `)}`);
    if (!ents.length) return "[:]";
    const one = `[${ents.join(", ")}]`;
    return one.length + ind.length <= 78 && !one.includes("\n") ? one : `[\n${ents.map((e) => `${ind}    ${e},`).join("\n")}\n${ind}]`;
  }
  return v === null ? "NSNull()" : JSON.stringify(v);
}
/** Font files the themes name, as addresses a page elsewhere can fetch. */
function absTokens(t) {
  const out = structuredClone(t);
  const walk = (o) => {
    if (Array.isArray(o)) o.forEach(walk);
    else if (o && typeof o === "object") {
      if (typeof o.src === "string" && o.src.startsWith("/fonts/")) o.src = new URL(o.src.slice(1), SITE).href;
      Object.values(o).forEach(walk);
    }
  };
  walk(out);
  return out;
}
/** What the code shows: the chart's address, its mode, step, theme, and what the reader changed. */
function session() {
  const c = S.c;
  const look = themed(modeFor(c));
  const tokens = Object.keys(look.tokens).length ? absTokens(look.tokens) : null;
  const theme = themeChoices().find((t) => t.id === L.theme)?.name ?? "";
  const size = S.src?.doc?.size;
  const height = Math.round(size?.height ?? 480);
  const state = S.index > 0 ? S.state : null;
  return { url: new URL(c.slot.dataset.src, location.href).href, runtime: new URL("runtime/datars.js", SITE).href, mode: look.mode, tokens, theme, height, state, index: S.index, signals: [...S.signals], data: [...S.data], alias: c.alias };
}
const BIG = 48;
const isBig = (cols) => Object.values(cols).some((a) => a.length > BIG);
const shape = (cols) => `{ ${Object.entries(cols).map(([k, a]) => `${k}: ${a.length} values`).join(", ")} }`;
const ident = (s) => s.replace(/[^A-Za-z0-9]+(.)?/g, (_, ch) => (ch ? ch.toUpperCase() : "")).replace(/^./, (ch) => ch.toLowerCase());

const CODE = {
  html(s) {
    const attrs = [`src="${s.url}"`, `height="${s.height}"`, `mode="${s.mode}"`, s.state && `state="${s.state}"`].filter(Boolean).join(" ");
    const calls = [];
    if (s.tokens) calls.push(`  view.setTokens(${js(s.tokens, "  ")}); // ${s.theme}`);
    for (const [k, v] of s.signals) calls.push(`  view.setSignal(${JSON.stringify(k)}, ${js(v, "  ")});`);
    for (const [k, d] of s.data) calls.push(isBig(d.cols) ? `  view.provideData(${JSON.stringify(k)}, ${ident(k)}); // your rows: ${shape(d.cols)}` : `  view.provideData(${JSON.stringify(k)}, ${js(d.cols, "  ")});`);
    return `<script type="module" src="${s.runtime}"></script>\n\n<datars-view id="chart" ${attrs}></datars-view>${calls.length ? `\n<script type="module">\n  const view = document.getElementById("chart");\n${calls.join("\n")}\n</script>` : ""}`;
  },
  react(s) {
    const name = cap(ident(s.alias));
    const props = [`src="${s.url}"`, `mode="${s.mode}"`, s.state && `state="${s.state}"`];
    if (s.tokens) props.push(`tokens={${js(s.tokens, "      ")}}`);
    if (s.signals.length) props.push(`signals={${js(Object.fromEntries(s.signals), "      ")}}`);
    if (s.data.length) props.push(`data={{ ${s.data.map(([k, d]) => `${/^[A-Za-z_$][\w$]*$/.test(k) ? k : JSON.stringify(k)}: ${isBig(d.cols) ? ident(k) : js(d.cols, "        ")}`).join(", ")} }}`);
    const big = s.data.filter(([, d]) => isBig(d.cols)).map(([k, d]) => `// ${ident(k)}: your rows, ${shape(d.cols)}\n`).join("");
    return `import { DatarsView } from "@datars/react";\n\n${big}export function ${name}() {\n  return (\n    <DatarsView\n${props.filter(Boolean).map((p) => `      ${p}`).join("\n")}\n    />\n  );\n}`;
  },
  swift(s) {
    const name = `${cap(ident(s.alias))}Chart`;
    const make = [];
    for (const [k, d] of s.data) make.push(isBig(d.cols) ? `        chart.data = [${JSON.stringify(k)}: ${ident(k)}Rows] // your rows as JSON or CSV Data: ${shape(d.cols)}` : `        chart.data = [${JSON.stringify(k)}: Data(#"${JSON.stringify(d.cols)}"#.utf8)]`);
    const upd = [`        chart.engine.setMode(.${s.mode === "high-contrast" ? "highContrast" : s.mode})`];
    if (s.tokens) upd.push(`        chart.engine.setTokens(${swift(s.tokens, "        ")}) // ${s.theme}`);
    for (const [k, v] of s.signals) upd.push(typeof v === "number" ? `        chart.engine.setSignal(${JSON.stringify(k)}, ${v})` : `        chart.engine.setSignal(${JSON.stringify(k)}, json: #"${JSON.stringify(v)}"#)`);
    upd.push("        chart.renderLoop()");
    return `import DatarsKit\nimport SwiftUI\n\nstruct ${name}: UIViewRepresentable {\n    func makeUIView(context: Context) -> DatarsChartView {\n        let chart = DatarsChartView()\n${make.length ? `${make.join("\n")}\n` : ""}        chart.load(.url(URL(string: "${s.url}")!))\n${s.index > 0 ? `        chart.targetState = ${s.index} // ${s.state}\n` : ""}        return chart\n    }\n\n    func updateUIView(_ chart: DatarsChartView, context: Context) {\n${upd.join("\n")}\n    }\n}`;
  },
  kotlin(s) {
    const lines = ["val chart = DatarsView(context)", `chart.load("${s.url}")`];
    if (s.mode !== "high-contrast") lines.push(`chart.setDarkMode(${s.mode === "dark"})`);
    if (s.index > 0) lines.push(`chart.goTo(${s.index}) // ${s.state}`);
    for (const [k, d] of s.data) lines.push(isBig(d.cols) ? `chart.provideData(${JSON.stringify(k)}, ${ident(k)}Json) // your rows: ${shape(d.cols)}` : `chart.provideData(${JSON.stringify(k)}, """${JSON.stringify(d.cols)}""")`);
    const missing = [s.tokens && "theme tokens", s.signals.length && "signals", s.mode === "high-contrast" && "high contrast"].filter(Boolean);
    if (missing.length) lines.push("", `// Not in the Android view's API yet: ${missing.join(", ")}.`, "// The web element, React and DatarsKit on iOS take them.");
    return lines.join("\n");
  },
};
const NOTES = {
  html: "The element and the calls this lab made, for any web page. The runtime loads once per page.",
  react: "<code>@datars/react</code>: the same element as a component, with the lab's settings as props.",
  swift: "DatarsKit on iOS: the engine and renderer run natively, with Metal.",
  kotlin: "The Android view: the engine runs natively, with Vulkan or OpenGL ES.",
  doc: "The document this chart was built from, as this site's build compiled it.",
};

async function codeUI() {
  if (!S || L.tab !== "code") return;
  for (const b of $$("[data-lang-tab]")) b.setAttribute("aria-pressed", String(b.dataset.langTab === L.lang));
  const note = $("#lab-code-note");
  const wrap = $(".lab-code-wrap");
  $(".lab-files")?.remove();
  if (L.lang !== "doc") {
    note.innerHTML = NOTES[L.lang];
    codeEl.dataset.lang = L.lang === "react" ? "ts" : L.lang === "html" ? "html" : "";
    codeEl.textContent = CODE[L.lang](session());
    if (codeEl.dataset.lang) highlight(codeEl);
    return;
  }
  const alias = S.c.alias;
  const recipe = S.c.recipe;
  note.innerHTML = NOTES.doc;
  let page;
  try {
    page = await aboutPage(S.c);
  } catch {
    codeEl.textContent = `// ${recipe ? "The reference page" : "The documents page"} couldn't be read.`;
    return;
  }
  if (S?.c.alias !== alias || L.lang !== "doc") return;
  if (recipe) {
    // The reference page shows the figure's document in full, under "Example".
    const pre = $("#example", page)?.nextElementSibling;
    const src = $("#example ~ .src-note a", page);
    note.innerHTML = `The figure's document, in full: <a href="${esc(src ? new URL(src.getAttribute("href"), refUrl(recipe)).href : "#")}">${esc(src?.textContent ?? `${recipe}.ts`)}</a>. The recipe itself: <a href="${refUrl(recipe).href}">its reference</a>, or edit it in the <a href="${new URL(`features/charts/#play=${recipe}`, SITE).href}">playground</a>.`;
    codeEl.dataset.lang = "ts";
    codeEl.innerHTML = pre?.querySelector("code")?.innerHTML ?? "";
    return;
  }
  const files = $$(`#doc-${alias} .g-doc-file`, page);
  if (!files.length) { codeEl.textContent = ""; return; }
  L.file = Math.min(L.file, files.length - 1);
  if (files.length > 1) {
    const bar = document.createElement("div");
    bar.className = "lab-files";
    bar.innerHTML = files.map((f, i) => `<button type="button" data-file="${i}" aria-pressed="${i === L.file}">${esc(f.dataset.file.split("/").pop())}</button>`).join("");
    wrap.before(bar);
  }
  const f = files[L.file];
  const title = f.querySelector(".code-title a");
  note.innerHTML = `${NOTES.doc} <a href="${esc(title?.getAttribute("href") ?? "#")}">${esc(f.dataset.file)}</a>`;
  codeEl.dataset.lang = "ts";
  codeEl.innerHTML = f.querySelector("pre code")?.innerHTML ?? "";
}
$(".lab-langs")?.addEventListener("click", (e) => {
  const b = e.target.closest("[data-lang-tab]");
  if (!b) return;
  L.lang = b.dataset.langTab;
  codeUI();
});
$("#lp-code")?.addEventListener("click", (e) => {
  const b = e.target.closest(".lab-files [data-file]");
  if (!b) return;
  L.file = Number(b.dataset.file);
  codeUI();
});
$("#lab-copy")?.addEventListener("click", async (e) => {
  const b = e.currentTarget;
  try {
    await navigator.clipboard.writeText(codeEl.textContent);
    b.textContent = "Copied";
  } catch {
    getSelection()?.selectAllChildren(codeEl);
    b.textContent = "Selected";
  }
  setTimeout(() => { b.textContent = "Copy"; }, 1600);
});

// ---- facts: what it weighs, where else it runs, what it's made of ----

const ICON = {
  web: '<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18"/></svg>',
  phone: '<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="6.5" y="2.5" width="11" height="19" rx="2.5"/><path d="M10.5 18.5h3"/></svg>',
  check: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m5 12.5 4.5 4.5L19 7.5"/></svg>',
  film: '<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="5" width="18" height="14" rx="2"/><path d="m10 9 5 3-5 3z"/></svg>',
  file: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5"/></svg>',
  gauge: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 18a8 8 0 1 1 16 0"/><path d="m12 18 4-6"/></svg>',
};
const gh = (path, label) => (GH ? `<a href="${esc(`${GH}/${path}`)}">${esc(label)}</a>` : esc(label));

function evidence(alias) {
  const li = (icon, html) => `<li>${ICON[icon]}<span>${html}</span></li>`;
  const out = [li("web", "<b>Web</b> — live on this page, from the same bundle any site would load.")];
  if (WASM_PIXELS.includes(alias)) out.push(li("check", `<b>The web engine's pixels</b> equal the reference goldens, every state — ${gh("packages/web/test/determinism.test.mjs", "the wasm determinism test")}.`));
  if (IOS_PIXELS.includes(alias)) out.push(li("phone", `<b>iOS</b> — every state renders to the same pixels as the goldens (${gh("apple/DatarsKit/Tests/DatarsKitTests/DatarsKitTests.swift", "DatarsKit's tests")}, on the iOS simulator).`));
  if (IOS_BEHAVIOUR[alias]) out.push(li("phone", `<b>iOS</b> — ${esc(IOS_BEHAVIOUR[alias])}.`));
  if (ANDROID_PIXELS.includes(alias)) out.push(li("phone", `<b>Android</b> — every state renders to the goldens' pixels through the C ABI (${gh("crates/datars-ffi/tests/goldens.rs", "goldens.rs")}, run on an emulator by ${gh("scripts/test-android.sh", "test-android.sh")}).`));
  if (NATIVE_TIMED.includes(alias)) out.push(li("gauge", `<b>iOS and Android</b> — its transitions frame-timed natively on the iOS simulator and the Android emulator: <a href="${new URL("performance/", SITE).href}">the numbers</a>.`));
  if (!IOS_PIXELS.includes(alias) && !ANDROID_PIXELS.includes(alias) && !NATIVE_TIMED.includes(alias) && !IOS_BEHAVIOUR[alias]) {
    out.push(li("phone", "<b>iOS and Android</b> — DatarsKit and the Android view load this same bundle by its address; this chart has no test of its own there yet."));
  }
  return out.join("");
}
function recipesOf(doc) {
  const set = new Set();
  const walk = (v) => {
    if (Array.isArray(v)) v.forEach(walk);
    else if (v && typeof v === "object") {
      if (v.kind === "use" && typeof v.recipe === "string") set.add(v.recipe);
      Object.values(v).forEach(walk);
    }
  };
  walk(doc?.scene);
  return [...set].sort();
}

async function factsUI() {
  const box = $("#lab-facts");
  if (!S || L.tab !== "facts") return;
  const c = S.c;
  const alias = c.alias;
  const alt = c.card.querySelector("details.alt");
  const recipesUsed = recipesOf(S.src?.doc);
  const recipe = c.recipe;
  const parts = [
    recipe ? `<p class="lab-ref"><a class="btn btn-ghost small" href="${refUrl(recipe).href}"><code>${esc(recipe)}</code> reference</a> <a class="btn btn-ghost small" href="${new URL(`features/charts/#play=${recipe}`, SITE).href}">Edit it in the playground</a></p>` : "",
    `<h3>What a reader downloads</h3><div data-part="tiers"><p class="fine">Measuring…</p></div><p class="fine">The runtime (one download for every chart on a page) plays the smallest variant it can — pre-expanded (T2) where there is one. A reader without it still gets T0: the poster and the text.</p>`,
    `<h3>Where else it runs</h3><ul class="lab-facts-list">${evidence(alias)}</ul><div data-part="film"></div>`,
    recipesUsed.length ? `<h3>Recipes it uses</h3><div class="lab-recipes">${recipesUsed.map((r) => {
      const std = r.match(/^@datars\/std\/(.+)$/);
      return std ? `<a href="${new URL(`docs/std/${std[1]}/`, SITE).href}">${esc(std[1])}</a>` : `<span title="The document's own recipe">${esc(r)}</span>`;
    }).join("")}</div>` : "",
    `<h3>Theme tokens it reads</h3><div data-part="tokens"></div>`,
    alt ? `<h3>Its text description</h3><div data-part="alt"></div>` : "",
  ];
  box.innerHTML = parts.join("");
  if (alt) {
    const a = alt.cloneNode(true);
    a.open = true;
    box.querySelector('[data-part="alt"]').append(a);
  }
  if (recipe) return recipeFacts(box, c);
  let page;
  try {
    page = await documents();
  } catch {
    box.querySelector('[data-part="tiers"]').innerHTML = `<p class="fine">The documents page couldn't be read.</p>`;
    return;
  }
  if (S?.c.alias !== alias || L.tab !== "facts") return;
  const doc = $(`#doc-${alias}`, page);
  const put = (part, sel) => {
    const el = doc && $(sel, doc);
    const at = box.querySelector(`[data-part="${part}"]`);
    if (el && at) at.replaceChildren(...[...el.childNodes].map((n) => adopt(n)));
  };
  put("tiers", ".g-doc-tiers");
  put("tokens", ".g-doc-tokens");
  const film = doc && $(".g-doc-film", doc);
  if (film) {
    const at = box.querySelector('[data-part="film"]');
    at.className = `lab-film${film.dataset.film?.endsWith("9x16") ? " v916" : ""}`;
    at.replaceChildren(...[...film.childNodes].map((n) => adopt(n)));
    at.insertAdjacentHTML("beforebegin", `<p class="fine">The same document as a film, rendered by <code>datars video</code> when this site was built:</p>`);
  }
  const exp = doc && $(".g-doc-exports", doc);
  if (exp) box.querySelector('[data-part="film"]').before(adopt(exp));
}

/** A chart type's facts: its tiers from its manifest (each variant's chunks, as published, before
 * the server compresses them) and its tokens from its reference page. */
async function recipeFacts(box, c) {
  const kb = (n) => (n < 1024 ? `${n} B` : `${(n / 1024).toFixed(n < 10240 ? 1 : 0)} KB`);
  const NAMES = { T0: "poster + text", T1: "baked scenes", T2: "pre-expanded", T3: "source" };
  try {
    const { manifest, url } = S.src ?? (await sourceOf(c.alias));
    const bytes = Object.fromEntries(manifest.chunks.map((k) => [k.hash, k.lazy ? 0 : k.bytes ?? 0]));
    const base = new URL(url.replace(/\/c\/[^/]+$/, "/chunks/"));
    const rows = await Promise.all(manifest.variants.map(async (v) => {
      const entry = await (await fetch(new URL(v.entry.replace(":", "_"), base))).json();
      return [v.tier, (entry.chunks ?? []).reduce((n, h) => n + (bytes[h] ?? 0), 0)];
    }));
    if (S?.c !== c) return;
    rows.sort((a, b) => a[0].localeCompare(b[0]));
    const max = Math.max(...rows.map((r) => r[1]));
    box.querySelector('[data-part="tiers"]').innerHTML = `<div class="hv">${rows.map(([t, n]) => `<div class="hv-row"><span class="hv-tier"><b>${t}</b> ${NAMES[t] ?? ""}</span><span class="hv-track" style="width:${((100 * n) / max).toFixed(1)}%"><i class="hv-doc" style="flex:1"></i></span><span class="hv-size">${kb(n)}</span></div>`).join("")}</div><p class="fine">Uncompressed, as the manifest lists each variant's chunks; a server's gzip makes them smaller.</p>`;
  } catch {
    box.querySelector('[data-part="tiers"]').innerHTML = `<p class="fine">The chart's manifest couldn't be read.</p>`;
  }
  try {
    const page = await reference(c.recipe);
    if (S?.c !== c) return;
    const p = $("#tokens", page)?.nextElementSibling;
    const at = box.querySelector('[data-part="tokens"]');
    if (p && at) {
      const codes = $$("code", p).map((x) => x.textContent);
      at.innerHTML = codes.length ? `<p class="tile-tokens">${codes.map((t) => `<code>${esc(t)}</code>`).join("")}</p>` : `<p class="fine">None: the recipe draws only what its parameters say.</p>`;
    }
  } catch { /* the tokens stay unsaid */ }
}

// ---- deep links ----
// `#lab=<alias>` opens the lab on that chart (and Back closes it).
{
  const m = location.hash.match(/^#lab=([a-z0-9-]+)$/);
  if (m && byAlias.has(m[1])) {
    history.replaceState(null, "", location.pathname + location.search);
    open(m[1]);
  }
}
lookUI();
