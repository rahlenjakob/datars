// datars in notebooks: get the web runtime onto the page (once per page) and mount a chart.
//
// The runtime is `@datars/web` (the <datars-view> element + the wasm engine). It arrives one of
// two ways:
//   - by URL: `import(url)` of its `datars.js` (a CDN or any static host) — it finds its wasm,
//     fonts and atlases next to itself;
//   - as files: bytes handed over by the kernel (the widget) or embedded in a standalone page.
//     They become blob: URLs registered in the runtime's asset map (`DatarsView.assets`), since a
//     blob has no folder to find its neighbours in. Works offline, needs no server.
// Shared by the HTML outputs (inlined), standalone pages (inlined) and the anywidget module.

const NB = (globalThis.__datarsNotebook ??= { runtime: null, fromFiles: null });

const TEXT = new TextDecoder();

function bytesOf(v) {
  if (v instanceof Uint8Array) return v;
  if (v instanceof ArrayBuffer) return new Uint8Array(v);
  if (ArrayBuffer.isView(v)) return new Uint8Array(v.buffer, v.byteOffset, v.byteLength);
  if (typeof v === "string") { // base64
    const bin = atob(v);
    const out = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
    return out;
  }
  throw new Error("datars: unexpected runtime file");
}

function blobUrl(bytes, type) {
  return URL.createObjectURL(new Blob([bytes], { type }));
}

/** The runtime from its files: `{ "datars.js", "wasm/datars_host_web.js", "wasm/wasi_shim.js",
 * "wasm/datars_host_web_bg.wasm", "fonts/…", "atlas/…" }` → the runtime module. */
export async function runtimeFromFiles(files) {
  const f = {};
  for (const [k, v] of Object.entries(files)) f[k] = bytesOf(v);
  const js = (p) => blobUrl(f[p], "text/javascript");
  // The engine's glue imports its WASI shim by a relative path: point it at the shim's blob.
  const shim = js("wasm/wasi_shim.js");
  const glueSrc = TEXT.decode(f["wasm/datars_host_web.js"]).replaceAll('"./wasi_shim.js"', JSON.stringify(shim)).replaceAll("'./wasi_shim.js'", JSON.stringify(shim));
  const glue = blobUrl(glueSrc, "text/javascript");
  const wasm = blobUrl(f["wasm/datars_host_web_bg.wasm"], "application/wasm");
  const mod = await import(/* webpackIgnore: true */ js("datars.js"));
  const defined = customElements.get("datars-view");
  const assets = (defined && defined.assets) || mod.runtimeAssets;
  if (!assets) throw new Error("datars: this page already has an older <datars-view> without runtime assets");
  assets.set("wasm/datars_host_web.js", glue);
  assets.set("wasm/datars_host_web_bg.wasm", wasm);
  for (const [k, v] of Object.entries(f)) {
    if (k.startsWith("fonts/")) assets.set(k, blobUrl(v, "font/ttf"));
    else if (k.startsWith("atlas/")) assets.set(k, blobUrl(v, "application/json"));
  }
  return mod;
}

/** The runtime by URL (once per page). */
export function runtimeFromUrl(url) {
  if (!NB.runtime) NB.runtime = import(/* webpackIgnore: true */ url);
  return NB.runtime;
}

/** Files embedded in the page as `<script type="application/octet-stream" data-datars-file="…">`
 * (a standalone page made with the runtime inlined). */
export function runtimeFromPage() {
  if (!NB.runtime) {
    const files = {};
    for (const s of document.querySelectorAll("script[data-datars-file]")) files[s.dataset.datarsFile] = s.textContent.trim();
    NB.runtime = runtimeFromFiles(files);
  }
  return NB.runtime;
}

/** The notebook's colour scheme, when the frontend says: JupyterLab, VS Code, Colab. */
export function notebookMode() {
  const b = document.body;
  if (b?.dataset?.jpThemeLight === "false") return "dark";
  if (b?.dataset?.jpThemeLight === "true") return "light";
  if (b?.classList?.contains("vscode-dark") || b?.classList?.contains("vscode-high-contrast")) return "dark";
  if (b?.classList?.contains("vscode-light")) return "light";
  const colab = document.documentElement?.getAttribute("theme");
  if (colab === "dark" || colab === "light") return colab;
  return null; // the element follows prefers-color-scheme
}

/**
 * Mount a live chart in `host`: a `<datars-view>` over the static poster (if any), which fades in
 * once its first frame is drawn — no blank moment while the engine starts.
 *
 * opts: { doc (JSON text), width, height, mode ("auto" | "light" | "dark"), runtime (a promise of
 * the runtime module), onState(status), onPick(detail), onFail(error) }. Returns the element
 * (after the runtime loaded), or null when it couldn't.
 */
export async function mount(host, opts) {
  const poster = host.querySelector(".datars-poster");
  let view;
  try {
    await opts.runtime;
    view = document.createElement("datars-view");
  } catch (err) {
    fail(host, poster, err, opts);
    return null;
  }
  const w = opts.width || 720, h = opts.height || 440;
  const shown = Math.min(w, host.clientWidth || w);
  view.setAttribute("height", String(Math.round((shown * h) / w)));
  const mode = opts.mode && opts.mode !== "auto" ? opts.mode : notebookMode();
  if (mode) view.setAttribute("mode", mode);
  view.style.display = "block";
  view.style.width = "100%";
  if (poster) {
    view.style.position = "absolute";
    view.style.inset = "0 0 auto 0";
    view.style.opacity = "0";
    view.style.transition = "opacity .2s ease-out";
  }
  let first = true;
  view.addEventListener("state", (ev) => {
    if (first) {
      first = false;
      // Drawn: the live chart takes the poster's place.
      if (poster) {
        view.style.opacity = "1";
        setTimeout(() => { poster.remove(); view.style.position = ""; view.style.inset = ""; }, 220);
      }
      host.querySelector(".datars-note")?.remove();
      // A standalone page takes the chart's paper, so a dark chart doesn't sit on a white page.
      if (opts.page) {
        const cs = getComputedStyle(view);
        const paper = cs.getPropertyValue("--paper").trim(), ink = cs.getPropertyValue("--ink").trim();
        if (paper) document.body.style.background = paper;
        if (ink) document.body.style.color = ink;
      }
    }
    opts.onState?.(ev.detail);
  });
  if (opts.onPick) view.addEventListener("pick", (ev) => opts.onPick(ev.detail));
  view.setDocument(opts.doc);
  host.appendChild(view);
  return view;
}

function fail(host, poster, err, opts) {
  console.warn("datars:", err);
  opts.onFail?.(err);
  if (host.querySelector(".datars-note")) return;
  const note = document.createElement("div");
  note.className = "datars-note";
  note.style.cssText = "font: 12px/1.4 system-ui, sans-serif; color: #888; margin-top: 4px";
  note.textContent = poster
    ? "Static preview — the live chart's runtime didn't load (offline?). pip install \"datars[widget]\" for charts that work offline."
    : "The datars runtime didn't load, and there's no static preview (install the datars CLI for one).";
  host.appendChild(note);
}

/** A notebook HTML output: the document sits in a JSON script tag next to the poster. */
export function mountOutput(id, opts) {
  const tries = { n: 0 };
  const go = () => {
    const host = document.getElementById(id);
    if (!host) { if (tries.n++ < 100) requestAnimationFrame(go); return; } // not attached yet
    if (host.dataset.mounted) return;
    host.dataset.mounted = "1";
    const doc = host.querySelector("script.datars-doc")?.textContent;
    if (!doc) return;
    const runtime = opts.inline ? runtimeFromPage() : runtimeFromUrl(opts.url);
    mount(host, { ...opts, doc, runtime });
  };
  go();
}
