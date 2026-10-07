// The maps page (site/pages/features/maps.html): the map playground.
//
// One published chart (site/figures/maps/playground.ts) with a state per place and a signal per
// layer, over an automatic basemap the build cut for exactly those places. The page flies it with
// `send("goto:<place>")`, toggles layers with `setSignal`, restyles the basemap with `setTokens`,
// and reads what the engine fetched with `stats` (archive bytes and range requests so far) and
// `tileViews()` (the zoom on screen) — the public API of <datars-view>, nothing else. The code
// panel shows those calls for the state on screen.
import { highlight } from "./site.js";

/** The document's places (its states), as in playground.ts: id, name, kind, [w, s, e, n]. */
const PLACES = [
  ["world", "The world", "world", [-165, -48, 165, 72]],
  ["iceland", "Iceland", "country", [-24.6, 63.2, -13.3, 66.6]],
  ["sweden", "Sweden", "country", [10.5, 55.2, 24.5, 69.2]],
  ["stockholm", "Stockholm", "city", [17.86, 59.25, 18.24, 59.4]],
  ["gamla-stan", "Gamla stan, Stockholm", "street", [18.062, 59.3205, 18.081, 59.3283]],
  ["denmark", "Denmark", "country", [7.8, 54.5, 13, 57.8]],
  ["copenhagen", "Copenhagen", "city", [12.3, 55.57, 12.8, 55.78]],
  ["portugal", "Portugal", "country", [-10, 36.8, -6, 42.2]],
  ["lisbon", "Lisbon", "city", [-9.48, 38.62, -8.95, 38.86]],
  ["kenya", "Kenya", "country", [33.8, -4.8, 42, 5.2]],
  ["india", "India", "country", [68, 6.5, 97.5, 35.7]],
  ["japan", "Japan", "country", [129, 30.5, 146, 45.7]],
  ["new-zealand", "New Zealand", "country", [166, -47.5, 179, -34]],
  ["usa", "United States", "country", [-125, 24.5, -66.9, 49.5]],
  ["brazil", "Brazil", "country", [-62, -33.5, -33, 3]],
  ["rio", "Rio de Janeiro", "city", [-43.55, -23.08, -43.08, -22.82]],
  ["copacabana", "Copacabana, Rio de Janeiro", "street", [-43.198, -22.99, -43.152, -22.944]],
];
const KINDS = { street: "Street corner", city: "City", country: "Country", world: "World" };
const TOUR = ["brazil", "rio", "copacabana", "world"];
/** The layer signals and their defaults (the document's). */
const LAYERS = { streets: true, labels: true, symbols: true, flows: false, choropleth: false };

/** Fixed basemap palettes, as theme tokens. "theme" sets none: the built-in theme's, per mode. */
const STYLES = {
  theme: {},
  atlas: { "map.land": "#f1e8d6", "map.water": "#a9c9d3", "map.park": "#d5dfb4", "map.building": "#e3d5bc", "map.road": "#fffaf0", "map.road-major": "#e9b872", "map.border": "#9b8a70", "map.label": "#4a3d2a", "map.label-halo": "#f7f1e3" },
  ink: { "map.land": "#f4f4f2", "map.water": "#ffffff", "map.park": "#e6e6e3", "map.building": "#d4d4d0", "map.road": "#ffffff", "map.road-major": "#9a9a96", "map.border": "#6f6f6b", "map.label": "#1f1f1d", "map.label-halo": "#ffffff" },
  blueprint: { "map.land": "#123a66", "map.water": "#0b2747", "map.park": "#164576", "map.building": "#1d5189", "map.road": "#2f6aa6", "map.road-major": "#8cc2f2", "map.border": "#7fb2e5", "map.label": "#e3f0ff", "map.label-halo": "#0b2747" },
  night: { "map.land": "#171d29", "map.water": "#0a0f19", "map.park": "#1a2a25", "map.building": "#252e3f", "map.road": "#2b3548", "map.road-major": "#d39a46", "map.border": "#4f5a6e", "map.label": "#cfd6e4", "map.label-halo": "#0a0f19" },
};

const section = document.getElementById("playground");
if (section) playground(section);

/** The slot's <datars-view> once its engine is running. */
function whenReady(slot) {
  return new Promise((resolve) => {
    const look = () => {
      const v = slot.querySelector("datars-view");
      if (!v) return false;
      if (v.status) resolve(v);
      else v.addEventListener("state", () => resolve(v), { once: true });
      return true;
    };
    if (look()) return;
    const mo = new MutationObserver(() => { if (look()) mo.disconnect(); });
    mo.observe(slot, { childList: true });
  });
}

const fold = (s) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
const kb = (n) => (n < 1048576 ? `${Math.max(n ? 1 : 0, Math.round(n / 1024))} KB` : `${(n / 1048576).toFixed(2)} MB`);
/** Write text only when it changes: a readout, not an animation (no layout work per frame). */
const put = (el, t) => { if (el && el.textContent !== t) el.textContent = t; };

function playground(section) {
  const slot = section.querySelector('.chart[data-chart="maps-playground"]');
  const list = section.querySelector(".mx-places");
  const q = section.querySelector("#mx-q");
  const codeEl = section.querySelector("#mx-code");
  const placeEl = section.querySelector(".mx-place");
  const kindEl = section.querySelector(".mx-kind");
  const zoomEl = section.querySelector(".mx-zoom");
  const bytesEl = section.querySelector(".mx-bytes");
  const reqsEl = section.querySelector(".mx-reqs");
  const fileEl = section.querySelector(".mx-file");
  const bars = [...section.querySelectorAll(".mx-bars span")];
  const tourBtn = section.querySelector("#mx-tour");

  let view = null;
  let at = "world";
  let style = "theme";
  const layers = { ...LAYERS };
  let tokens = {}; // what setTokens was last given
  let pending = null; // a place picked before the chart was running
  let tour = null; // the tour's timer, while it runs

  // ---- places (their buttons are in the page: nothing moves when this script arrives) ----
  const buttons = new Map([...list.querySelectorAll("button[data-place]")].map((b) => [b.dataset.place, b]));
  list.addEventListener("click", (e) => {
    const b = e.target.closest("button[data-place]");
    if (!b) return;
    stopTour();
    fly(b.dataset.place);
  });
  q.addEventListener("input", () => {
    const s = fold(q.value.trim());
    let shown = 0;
    for (const [id, name] of PLACES) {
      const hit = !s || fold(name).includes(s) || fold(KINDS[PLACES.find((p) => p[0] === id)[2]]).includes(s);
      buttons.get(id).hidden = !hit;
      shown += hit ? 1 : 0;
    }
    for (const g of list.querySelectorAll(".mx-group")) g.hidden = ![...g.querySelectorAll("button")].some((b) => !b.hidden);
    list.dataset.empty = shown ? "" : "Not in this archive";
  });
  q.addEventListener("keydown", (e) => {
    if (e.key !== "Enter") return;
    const first = [...buttons.values()].find((b) => !b.hidden);
    if (first) { stopTour(); fly(first.dataset.place); }
  });

  function fly(id) {
    at = id;
    for (const [pid, b] of buttons) b.setAttribute("aria-pressed", String(pid === id));
    const p = PLACES.find((x) => x[0] === id);
    put(placeEl, p[1]);
    put(kindEl, KINDS[p[2]]);
    render();
    if (!view) return void (pending = id);
    view.send(`goto:${id}`);
    watch(4000);
  }

  // ---- the tour ----
  function stopTour() {
    if (!tour) return;
    clearTimeout(tour);
    tour = null;
    tourBtn.textContent = "Fly the tour";
  }
  tourBtn.addEventListener("click", () => {
    if (tour) return stopTour();
    let i = 0;
    tourBtn.textContent = "Stop the tour";
    const next = () => {
      fly(TOUR[i]);
      i += 1;
      tour = i < TOUR.length ? setTimeout(next, 5200) : null;
      if (!tour) tourBtn.textContent = "Fly the tour";
    };
    next();
  });

  // ---- layers ----
  for (const box of section.querySelectorAll("input[data-layer]")) {
    box.checked = layers[box.dataset.layer];
    box.addEventListener("change", () => {
      layers[box.dataset.layer] = box.checked;
      view?.setSignal(box.dataset.layer, box.checked);
      render();
      watch(1500);
    });
  }

  // ---- style ----
  const inks = [...section.querySelectorAll(".mx-inks input[data-token]")];
  const resolved = () => (view?.status?.tokens?.tokens ?? {});
  /** The colour inputs show what the map is drawn with now: the overrides, else the theme's. */
  function showInks() {
    const r = resolved();
    for (const input of inks) {
      const v = tokens[input.dataset.token] ?? r[input.dataset.token];
      const hex = typeof v === "string" && /^#[0-9a-f]{6}/i.test(v) ? v.slice(0, 7).toLowerCase() : null;
      if (hex && input.value !== hex) input.value = hex;
    }
  }
  function applyTokens() {
    view?.setTokens(tokens);
    render();
    requestAnimationFrame(showInks);
  }
  section.querySelector(".mx-styles").addEventListener("click", (e) => {
    const b = e.target.closest("button[data-style]");
    if (!b) return;
    style = b.dataset.style;
    for (const o of section.querySelectorAll(".mx-styles button")) o.setAttribute("aria-pressed", String(o === b));
    tokens = { ...STYLES[style] };
    applyTokens();
  });
  for (const input of inks) {
    input.addEventListener("input", () => {
      tokens = { ...tokens, [input.dataset.token]: input.value };
      // One token edited: the label halo follows the water for legibility on dark palettes.
      view?.setTokens(tokens);
      render();
    });
  }
  // The page's light/dark toggle changes the theme's colours: show them.
  new MutationObserver(() => requestAnimationFrame(showInks)).observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });

  // ---- the code ----
  function render() {
    const p = PLACES.find((x) => x[0] === at);
    const lines = ['const map = document.querySelector("datars-view");', ""];
    lines.push(`map.send("goto:${at}");`, `// the document's step: step("${at}", { set: { bounds: [${p[3].join(", ")}] } })`);
    const changed = Object.entries(layers).filter(([k, v]) => v !== LAYERS[k]);
    if (changed.length) {
      lines.push("");
      for (const [k, v] of changed) lines.push(`map.setSignal("${k}", ${v});`);
    }
    const entries = Object.entries(tokens);
    if (entries.length) {
      lines.push("", "map.setTokens({");
      for (const [k, v] of entries) lines.push(`  "${k}": "${v}",`);
      lines.push("});");
    }
    lines.push("", "// the readout:", "map.stats;        // { bytes, requests, tiles, … } fetched so far", "map.tileViews();  // [{ source, bbox, zoom, … }] on screen now");
    const text = lines.join("\n");
    if (codeEl.textContent !== text) {
      codeEl.textContent = text;
      highlight(codeEl);
    }
  }

  // ---- the readout ----
  // Bytes fetched since the chart opened, attributed to the whole zoom on screen when they arrived
  // (requests are asked for at that zoom and land a moment later: close, not exact). Polled a few
  // times a second while something moves, and only written when it changes.
  const perZoom = new Array(16).fill(0);
  let lastBytes = 0, watchUntil = 0, timer = 0, visible = false;
  function sample() {
    timer = 0;
    if (!view) return;
    const s = view.stats;
    const tv = view.tileViews?.() ?? [];
    const z = tv.length ? tv[0].zoom : null;
    if (z != null) put(zoomEl, `zoom ${Math.max(0, z).toFixed(1)}`);
    if (s) {
      const d = s.bytes - lastBytes;
      if (d > 0 && z != null) perZoom[Math.max(0, Math.min(15, Math.floor(z)))] += d;
      lastBytes = s.bytes;
      put(bytesEl, kb(s.bytes));
      put(reqsEl, String(s.requests));
      const max = Math.max(...perZoom, 1);
      bars.forEach((b, i) => {
        const h = `${perZoom[i] ? Math.max(4, Math.round((perZoom[i] / max) * 100)) : 0}%`;
        if (b.style.height !== h) b.style.height = h;
        const t = perZoom[i] ? `zoom ${i}: ${kb(perZoom[i])}` : "";
        if (b.title !== t) b.title = t;
      });
    }
    if (!fileEl.dataset.found) {
      const hit = performance.getEntriesByType("resource").find((r) => /\.pmtiles(\?|$)/.test(r.name));
      if (hit) {
        fileEl.dataset.found = "1";
        put(fileEl, decodeURIComponent(new URL(hit.name).pathname.split("/").pop()));
      }
    }
    if (visible && performance.now() < watchUntil) timer = setTimeout(sample, 200);
  }
  function watch(ms) {
    watchUntil = Math.max(watchUntil, performance.now() + ms);
    if (!timer) timer = setTimeout(sample, 120);
  }

  render();
  whenReady(slot).then((v) => {
    view = v;
    new IntersectionObserver(([e]) => { visible = e.isIntersecting; if (visible) watch(1500); }, { threshold: 0 }).observe(v);
    for (const [k, on] of Object.entries(layers)) if (on !== LAYERS[k]) v.setSignal(k, on);
    if (Object.keys(tokens).length) v.setTokens(tokens);
    showInks();
    if (pending) fly(pending);
    watch(3000);
  });
}
