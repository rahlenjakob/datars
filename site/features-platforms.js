// The platforms page (site/pages/features/platforms.html): one chart, every target, laid over each
// other.
//
// The stage holds the chart live (a <datars-view> laid out at the iPhone 16 Pro's chart box,
// 402 × 812, and scaled to fit) under two layers, the left and the right target. A target is the
// live view or a file: the iOS sample app's and the desktop viewer's captures (site/img), and the
// PNG, SVG and MP4 this site's build made with `datars render` and `datars video` at the same box.
// Swipe shows the left target left of the divider and the right one right of it; Difference lays
// one over the other with `mix-blend-mode: difference` (black where they agree — it works over the
// live canvas, which the page can't read back); Heatmap compares two files pixel by pixel in the
// browser, the way `datars gpu` compares the GPU with the CPU reference (crates/datars-test/src/
// gpu.rs: ΔE as OKLab distance × 100, "visible" over 10), and draws its heatmap.
//
// Steps go to every target at once: `send("goto:<state>")` to the live views, the step's file to
// the others, and the film is sought to the step's hold (from its WebVTT captions).
import { highlight } from "./site.js";

const W = 402, H = 812, DPR = 3;
const section = document.getElementById("same");
const NAMES = { web: "Web", ios: "iOS", android: "Android", mac: "macOS", png: "PNG", svg: "SVG", mp4: "Video", pdf: "PDF" };
const SUBS = { web: "live", ios: "Metal", android: "GLES", mac: "Metal", png: "CPU reference", svg: "vector", mp4: "H.264" };

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

/** Scale a fixed 402 × 812 box to its frame's width (`--k`), before anything inside is shown. */
function scaleTo(frame) {
  const set = () => {
    const k = frame.clientWidth / W;
    if (k > 0) {
      frame.style.setProperty("--k", k.toFixed(5));
      frame.classList.add("pf-sized");
    }
  };
  set();
  new ResizeObserver(set).observe(frame);
}

// ---- OKLab ΔE, as datars-color's delta_e ------------------------------------------------------
const LIN = new Float32Array(256).map((_, i) => { const c = i / 255; return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4; });
function oklab(r, g, b, out) {
  const R = LIN[r], G = LIN[g], B = LIN[b];
  const l = Math.cbrt(0.4122214708 * R + 0.5363325363 * G + 0.0514459929 * B);
  const m = Math.cbrt(0.2119034982 * R + 0.6806995451 * G + 0.1073969566 * B);
  const s = Math.cbrt(0.0883024619 * R + 0.2817188376 * G + 0.6299787005 * B);
  out[0] = 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s;
  out[1] = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s;
  out[2] = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s;
}

/** Pixels of a file target at the comparison size (3×), drawn by the browser: a PNG as it is, an
 * SVG rasterized at that size, a video's current frame. */
function pixels(media) {
  const c = new OffscreenCanvas(W * DPR, H * DPR);
  const g = c.getContext("2d", { willReadFrequently: true });
  g.fillStyle = "#fff";
  g.fillRect(0, 0, c.width, c.height);
  g.drawImage(media, 0, 0, c.width, c.height);
  return g.getImageData(0, 0, c.width, c.height);
}

/** Compare two images of the same size: mean ΔE, share visibly different (ΔE > 10), share not
 * bit-identical, and a heatmap (grey context, red where they differ — as `datars gpu` draws it).
 * In slices, yielding between them, so the page keeps its frames. */
async function compare(a, b, heat, alive) {
  const n = a.width * a.height, A = a.data, B = b.data, P = heat.data;
  const x = [0, 0, 0], y = [0, 0, 0];
  let sum = 0, visible = 0, changed = 0;
  const rows = 160;
  for (let r0 = 0; r0 < a.height; r0 += rows) {
    const end = Math.min(a.height, r0 + rows) * a.width;
    for (let p = r0 * a.width; p < end; p++) {
      const i = p * 4;
      const gr = (A[i] + A[i + 1] + A[i + 2]) / 3;
      let de = 0;
      if (A[i] !== B[i] || A[i + 1] !== B[i + 1] || A[i + 2] !== B[i + 2]) {
        oklab(A[i], A[i + 1], A[i + 2], x);
        oklab(B[i], B[i + 1], B[i + 2], y);
        de = Math.hypot(x[0] - y[0], x[1] - y[1], x[2] - y[2]) * 100;
        changed++;
        sum += de;
        if (de > 10) visible++;
      }
      const k = Math.min(1, de / 20);
      const base = gr * 0.3 + 255 * 0.7;
      P[i] = base * (1 - k) + 255 * k;
      P[i + 1] = P[i + 2] = base * (1 - k);
      P[i + 3] = 255;
    }
    await new Promise((r) => setTimeout(r, 0));
    if (!alive()) return null;
  }
  return { mean: sum / n, visible: visible / n, changed: changed / n };
}

function platforms() {
  const stage = section.querySelector(".pf-stage");
  const canvas = stage.querySelector(".pf-canvas");
  const live = stage.querySelector(".pf-livelayer");
  const layers = { a: stage.querySelector(".pf-a"), b: stage.querySelector(".pf-b") };
  const heatCanvas = stage.querySelector(".pf-heat");
  const tags = { a: stage.querySelector(".pf-tag-a"), b: stage.querySelector(".pf-tag-b") };
  const swipe = section.querySelector(".pf-swipe");
  const measure = section.querySelector(".pf-measure-text");
  const codeEl = section.querySelector("#pf-code");
  const facts = section.querySelector(".pf-facts");
  const steps = [...section.querySelectorAll(".pf-steps button")];
  const stateOf = (i) => steps[i].dataset.state;

  // The files the build and the captures made: files[target][step] → { url, size }.
  const files = {};
  for (const li of section.querySelectorAll(".pf-files li")) {
    const t = li.dataset.target;
    if (!li.dataset.src) continue; // a film without ffmpeg: no film
    const f = { url: new URL(li.dataset.src, location.href).href, size: li.dataset.size };
    if (t === "mp4") files.mp4 = f;
    else (files[t] ??= [])[Number(li.dataset.step)] = f;
  }
  const chartUrl = new URL(section.querySelector(".pf-livelayer .chart").dataset.src, location.href).href;

  const S = { step: 0, a: "web", b: "ios", mode: "swipe", split: 50 };
  const views = [];
  let renderer = "";
  let job = 0;

  scaleTo(stage);
  for (const shot of section.querySelectorAll(".pf-shot")) scaleTo(shot);

  // ---- the film: each step's hold ----
  // From its captions when its steps have narration; else from the film's length, as `datars video`
  // lays a program out: each state held `--hold` seconds (2.5 by default), the transitions between.
  let holds = null;
  const HOLD = 2.5;
  const film = files.mp4 ? fetch(files.mp4.url.replace(/\.mp4$/, ".vtt")).then((r) => r.text()).then((vtt) => {
    const t = (s) => s.split(":").reduce((n, x) => n * 60 + Number(x), 0);
    const cues = [...vtt.matchAll(/(\d+:\d+:[\d.]+) --> (\d+:\d+:[\d.]+)/g)];
    if (cues.length === steps.length) holds = cues.map((m) => Math.max(t(m[1]), t(m[2]) - 0.3));
  }).catch(() => {}) : Promise.resolve();
  const holdAt = (v, step) => {
    if (holds) return holds[step];
    const seg = (v.duration - HOLD) / Math.max(1, steps.length - 1);
    return Math.max(0, HOLD + step * seg - 0.3);
  };
  /** Put a video at the step's hold; resolves when that frame is there. */
  async function seekVideo(v, step) {
    if (!files.mp4) return;
    if (!v.src) { v.preload = "auto"; v.src = files.mp4.url; }
    await film;
    await new Promise((resolve) => {
      const done = () => { v.removeEventListener("seeked", done); resolve(); };
      const go = () => {
        const t = holdAt(v, step);
        if (Math.abs(v.currentTime - t) < 0.01 && v.readyState >= 2) return resolve();
        v.addEventListener("seeked", done);
        v.currentTime = t;
      };
      if (v.readyState >= 1) go(); else v.addEventListener("loadedmetadata", go, { once: true });
    });
  }

  // ---- media: one <img> and one <video> per layer, swapped only once the new frame is decoded ----
  function media(layer) {
    if (!layer._img) {
      layer._img = Object.assign(document.createElement("img"), { alt: "", decoding: "async", width: W, height: H });
      layer._video = Object.assign(document.createElement("video"), { muted: true, playsInline: true, preload: "none", width: W, height: H });
      layer._img.hidden = layer._video.hidden = true;
      layer.append(layer._img, layer._video);
    }
    return layer;
  }
  async function show(layer, target, step) {
    const m = media(layer);
    if (target === "web") { m._img.hidden = m._video.hidden = true; return null; }
    if (target === "mp4") {
      await seekVideo(m._video, step);
      m._img.hidden = true; m._video.hidden = false;
      return m._video;
    }
    const url = files[target]?.[step]?.url;
    if (!url) return null;
    if (m._img.src !== url) {
      const next = new Image();
      next.src = url;
      try { await next.decode(); } catch { /* shown as soon as it loads */ }
      m._img.src = url;
    }
    m._video.hidden = true; m._img.hidden = false;
    return m._img;
  }

  // ---- what the panel says ----
  const command = (t, step) => {
    const st = stateOf(step);
    return {
      web: [`<!-- ${NAMES.web}: this page -->`, `<datars-view src="${chartUrl}"></datars-view>`, `view.send("goto:${st}")`],
      ios: [`// ${NAMES.ios}: the SwiftUI sample app`, `DatarsChart(source: .url(chartURL), state: ${step})`],
      android: [`// ${NAMES.android}: the sample app (Kotlin)`, `chart.load(chartURL)`, step ? `chart.send("next")  // ×${step}: a tap each` : "// the first step, as it opens"],
      mac: [`# ${NAMES.mac}: the desktop viewer, offscreen`, `datars-view ${chartUrl} \\`, `  --screenshot mac.png --state ${step} --dpr 3`],
      png: [`# ${NAMES.png}: the CPU reference`, `datars render everywhere.ts --state ${step} \\`, `  --size 402x812 --dpr 3 --out everywhere.png`],
      svg: [`# ${NAMES.svg}: text stays text`, `datars render everywhere.ts --state ${step} \\`, `  --size 402x812 --out everywhere.svg`],
      mp4: [`# ${NAMES.mp4}: every step, with captions`, `datars video everywhere.ts \\`, `  --size 402x812 --dpr 3 --out everywhere.mp4`],
    }[t];
  };
  function describe() {
    const lines = [...command(S.a, S.step), "", ...command(S.b, S.step)];
    codeEl.textContent = lines.join("\n");
    highlight(codeEl);
    const r = renderer ? ` Your browser draws the live chart ${renderer === "gpu" ? "on its GPU (WebGPU, or WebGL2 where there's none)" : "with the CPU renderer (no GPU for the page)"}.` : "";
    facts.textContent = `Left: ${NAMES[S.a]} (${SUBS[S.a]}). Right: ${NAMES[S.b]} (${SUBS[S.b]}). All at 402 × 812, the chart box of the iPhone 16 Pro under its status bar (the Android emulator’s display is set to give its app the same).${r}${S.a === "ios" || S.b === "ios" ? " The bar at the foot of the iOS capture is the system's home indicator, drawn over the app." : ""}`;
    tags.a.textContent = `${NAMES[S.a]} · ${SUBS[S.a]}`;
    tags.b.textContent = `${NAMES[S.b]} · ${SUBS[S.b]}`;
  }

  // ---- the stage ----
  async function render() {
    const my = ++job;
    stage.dataset.mode = S.mode;
    for (const g of section.querySelectorAll(".pf-chips")) {
      for (const b of g.querySelectorAll("button[data-v]")) b.setAttribute("aria-pressed", String(b.dataset.v === S[g.dataset.side]));
    }
    for (const b of section.querySelectorAll(".pf-modes button")) b.setAttribute("aria-pressed", String(b.dataset.mode === S.mode));
    for (const b of steps) b.setAttribute("aria-pressed", String(Number(b.dataset.step) === S.step));
    for (const c of section.querySelectorAll(".pf-card")) c.classList.toggle("pf-picked", c.dataset.v === S.a || c.dataset.v === S.b);
    describe();
    const [ma, mb] = await Promise.all([show(layers.a, S.a, S.step), show(layers.b, S.b, S.step)]);
    if (my !== job) return;
    // Which layer goes on top in Difference: the right one, unless it's the live chart (the base).
    layers.a.classList.toggle("pf-top", S.b === "web");
    layers.b.classList.toggle("pf-top", S.b !== "web");
    stage.style.setProperty("--split", `${S.split}%`);
    heatCanvas.hidden = true;
    if (S.a === "web" || S.b === "web") {
      measure.textContent = S.mode === "heatmap"
        ? "A heatmap needs two files: the page can't read the live canvas back. Difference works with it — black is where the two agree."
        : `The page can't read the live canvas back, so there is no number for ${NAMES.web} — but Difference shows it: black is where the two agree. Edges can glow faintly where your screen's pixel density isn't the files' 3×.`;
      return;
    }
    if (S.a === S.b) { measure.textContent = "The same file on both sides."; return; }
    measure.textContent = "Comparing every pixel…";
    const pa = pixels(ma), pb = pixels(mb);
    const heat = new ImageData(pa.width, pa.height);
    const r = await compare(pa, pb, heat, () => my === job);
    if (!r || my !== job) return;
    const pct = (x) => (x === 0 ? "0" : x < 0.0001 ? "under 0.01" : (x * 100).toFixed(x < 0.01 ? 2 : 1));
    measure.innerHTML = r.changed === 0
      ? `<b>Identical</b>: every pixel the same, at 3× (${pa.width} × ${pa.height}).`
      : `<b>Mean ΔE ${r.mean.toFixed(3)}</b> · ${pct(r.visible)} % of pixels visibly different · ${pct(r.changed)} % not bit-identical — measured in your browser, ${pa.width} × ${pa.height} px, the way <code>datars gpu</code> measures (ΔE is OKLab distance × 100; 1–2 is about what you'd notice).`;
    if (S.mode === "heatmap") {
      if (heatCanvas.width !== heat.width) { heatCanvas.width = heat.width; heatCanvas.height = heat.height; }
      heatCanvas.getContext("2d").putImageData(heat, 0, 0);
      heatCanvas.hidden = false;
    }
  }

  // ---- the strip: every target at this step ----
  let stripNear = false;
  async function strip() {
    for (const a of section.querySelectorAll(".pf-exports a[data-get]")) {
      const f = a.dataset.get === "mp4" ? files.mp4 : files[a.dataset.get]?.[S.step];
      if (!f) continue;
      a.href = f.url;
      a.querySelector("i").textContent = f.size;
    }
    if (!stripNear) return;
    for (const card of section.querySelectorAll(".pf-card")) {
      const t = card.dataset.v;
      const el = card.querySelector(".pf-file");
      if (!el) continue;
      if (t === "mp4") seekVideo(el, S.step);
      else {
        const url = files[t]?.[S.step]?.url;
        if (url && el.src !== url) {
          const next = new Image();
          next.src = url;
          next.decode().catch(() => {}).then(() => { if (files[t][S.step].url === url) el.src = url; });
        }
      }
    }
  }

  function goStep(i) {
    S.step = i;
    for (const v of views) v.send(`goto:${stateOf(i)}`);
    render();
    strip();
  }
  for (const b of steps) b.addEventListener("click", () => goStep(Number(b.dataset.step)));
  section.querySelector(".pf-panel").addEventListener("click", (e) => {
    const b = e.target.closest("button");
    if (!b || b.disabled) return;
    const side = b.closest(".pf-chips")?.dataset.side;
    if (side) {
      const other = side === "a" ? "b" : "a";
      if (S[other] === b.dataset.v) S[other] = S[side]; // picking the other side's target swaps them
      S[side] = b.dataset.v;
    } else if (b.dataset.mode) S.mode = b.dataset.mode;
    else return;
    render();
  });
  for (const card of section.querySelectorAll(".pf-card")) {
    card.querySelector(".pf-pick")?.addEventListener("click", () => {
      const t = card.dataset.v;
      if (t === S.b) return;
      if (t === S.a) S.a = S.b;
      S.b = t;
      render();
      stage.scrollIntoView({ block: "nearest", behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "instant" : "smooth" });
    });
  }
  // The divider: the range under the stage, or a drag across the stage itself.
  const setSplit = (v) => {
    S.split = Math.max(0, Math.min(100, v));
    swipe.value = String(S.split);
    stage.style.setProperty("--split", `${S.split}%`);
  };
  swipe.addEventListener("input", () => setSplit(Number(swipe.value)));
  let dragging = false;
  const at = (e) => { const r = canvas.getBoundingClientRect(); setSplit(((e.clientX - r.left) / r.width) * 100); };
  stage.addEventListener("pointerdown", (e) => { if (S.mode !== "swipe") return; dragging = true; stage.setPointerCapture(e.pointerId); at(e); });
  stage.addEventListener("pointermove", (e) => { if (dragging) at(e); });
  stage.addEventListener("pointerup", () => { dragging = false; });
  stage.addEventListener("pointercancel", () => { dragging = false; });

  for (const slot of section.querySelectorAll(".pf-livelayer .chart, .pf-live .chart")) {
    whenReady(slot).then((v) => {
      views.push(v);
      if (S.step) v.send(`goto:${stateOf(S.step)}`);
      const r = () => { if (slot.closest(".pf-livelayer") && v.dataset.renderer && v.dataset.renderer !== renderer) { renderer = v.dataset.renderer; describe(); } };
      r();
      setTimeout(r, 1000);
    });
  }
  render();
  // The strip loads its files (and the film) once it comes near.
  new IntersectionObserver(([e], io) => {
    if (!e.isIntersecting) return;
    io.disconnect();
    stripNear = true;
    strip();
  }, { rootMargin: "50% 0px" }).observe(section.querySelector(".pf-strip"));
}

if (section) platforms();
