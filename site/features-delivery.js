// The delivery page (site/pages/features/delivery.html): republishing, watched.
//
// One chart in three versions (site/figures/delivery/_votes.ts), published by the site build side
// by side into one content-addressed chunk folder (`c/delivery-v1…3` + `chunks/`). Three embeds —
// an article, an app, a dashboard — play the version on screen. "Publish" points them at the next
// version, as moving an alias does: each loads that version's manifest and the chunks it lacks, as
// a returning reader's would. The ledger says what changed in bytes, from the manifests (each
// chunk's hash, kind and size), and what this browser actually fetched for it (the Resource Timing
// entries of the chunk requests: a transfer size of 0 is the HTTP cache).
//
// An embed swaps without a blank: the new version's view starts hidden over the old one and
// takes its place once it has drawn.
import "./runtime/datars.js";

const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
const root = document.documentElement;
const mode = () => (root.dataset.theme === "dark" ? "dark" : "light");
const kb = (n) => (n < 1024 ? `${n} B` : `${(n / 1024).toFixed(n < 10240 ? 1 : 0)} KB`);
const el = (tag, props = {}, ...kids) => {
  const n = Object.assign(document.createElement(tag), props);
  for (const k of kids) if (k != null) n.append(k);
  return n;
};
const KINDS = { poster: "poster", a11y: "accessible text", doc: "document", data: "data", font: "font subset", program: "program", scene: "baked scene", entry: "variant entry" };

/** The site's root, from this script's own URL (the site may live under a subpath). */
const site = new URL(".", import.meta.url);
const manifestUrl = (v) => new URL(`c/delivery-v${v}`, site).href;
const chunkUrl = (h) => new URL(`chunks/${h.replace(":", "_")}`, site).href;

const manifests = new Map();
const manifest = (v) => {
  if (!manifests.has(v)) manifests.set(v, fetch(manifestUrl(v), { cache: "no-cache" }).then((r) => r.json()));
  return manifests.get(v);
};
const entries = new Map();
const entry = (h) => {
  if (!entries.has(h)) entries.set(h, fetch(chunkUrl(h)).then((r) => r.json()));
  return entries.get(h);
};
/** The chunks a tier's first load needs — its entry and what it lists (lazy ones aside) — with
 * their kinds and sizes from the manifest. */
async function tierChunks(m, tier) {
  const variant = m.variants.find((x) => x.tier === tier);
  if (!variant) return null;
  const meta = Object.fromEntries(m.chunks.map((c) => [c.hash, c]));
  const e = await entry(variant.entry);
  const list = (e.chunks ?? []).filter((h) => !meta[h]?.lazy).map((h) => ({ hash: h, kind: meta[h]?.kind ?? "other", bytes: meta[h]?.bytes ?? 0, what: meta[h]?.meta?.state ?? meta[h]?.meta?.face ?? "" }));
  list.push({ hash: variant.entry, kind: "entry", bytes: meta[variant.entry]?.bytes ?? 0, what: tier });
  return { variant, list };
}

/** A <datars-view> for version `v` in a reserved box, in the page's mode, at the step `state`. */
function view(box, v, state) {
  const d = document.createElement("datars-view");
  d.setAttribute("src", manifestUrl(v));
  d.setAttribute("mode", mode());
  d.setAttribute("no-controls", "");
  d.setAttribute("aria-label", `Vote share by party, Sweden 2022 — version ${v}`);
  if (state) d.setAttribute("state", String(state));
  if (box.dataset.publishers) d.setAttribute("publishers", box.dataset.publishers);
  d.setAttribute("height", String(Math.round(box.clientHeight)));
  return d;
}
/** Resolves when a view has drawn its first frame (or, refused, settled on its poster). */
function drawn(d) {
  return new Promise((resolve) => {
    const t0 = performance.now();
    const look = () => (d.dataset.renderer || performance.now() - t0 > 8000 ? resolve() : setTimeout(look, 60));
    d.addEventListener("state", look, { once: true });
    setTimeout(() => { if (!d.status) look(); }, 2500);
  });
}
const views = new Set();
new MutationObserver(() => { for (const d of views) d.setAttribute("mode", mode()); }).observe(root, { attributes: true, attributeFilter: ["data-theme"] });
function fit(box) {
  new ResizeObserver(() => {
    const d = box.querySelector("datars-view:last-of-type");
    const h = String(Math.round(box.clientHeight));
    if (d && d.getAttribute("height") !== h) d.setAttribute("height", h);
  }).observe(box);
}

// ---- republishing ------------------------------------------------------------------------------

function republish(section) {
  const boxes = [...section.querySelectorAll(".dl-embeds .dl-box")];
  const vButtons = [...section.querySelectorAll(".dl-versions button[data-v]")];
  const next = section.querySelector(".dl-next");
  const pills = [...section.querySelectorAll(".dl-steps button")];
  const summary = section.querySelector(".dl-summary");
  const measured = section.querySelector(".dl-measured");
  const cmd = section.querySelector("#dl-cmd");
  const grid = section.querySelector(".dl-chunks");
  let current = 1, shown = null, state = "ranked", busy = false;
  const stepIndex = () => pills.findIndex((p) => p.dataset.state === state);

  function press() {
    for (const b of vButtons) b.setAttribute("aria-pressed", String(Number(b.dataset.v) === current));
    next.innerHTML = current < 3 ? `Publish v${current + 1} <span aria-hidden="true">→</span>` : `Roll back to v1 <span aria-hidden="true">↺</span>`;
    for (const p of pills) {
      p.disabled = p.dataset.state === "seats" && current < 3;
      p.setAttribute("aria-pressed", String(p.dataset.state === state));
    }
  }

  /** Every chunk the embeds have loaded on this page so far: a reader's HTTP cache. */
  const seen = new Set();
  /** The ledger: this version's chunks against the version shown before (and what this page has
   * loaded already, for a rollback). */
  async function ledger(from, to, fetched) {
    const [mb, ma] = await Promise.all([manifest(to), from ? manifest(from) : null]);
    const b = await tierChunks(mb, "T2");
    const a = ma ? await tierChunks(ma, "T2") : null;
    const before = new Set(a?.list.map((c) => c.hash) ?? []);
    const after = new Set(b.list.map((c) => c.hash));
    const tiles = [
      ...b.list.map((c) => ({ ...c, state: before.has(c.hash) || seen.has(c.hash) ? "kept" : "new" })),
      ...(a?.list.filter((c) => !after.has(c.hash)).map((c) => ({ ...c, state: "gone" })) ?? []),
    ];
    const total = Math.max(...tiles.map((t) => t.bytes));
    grid.replaceChildren(...tiles.map((t) => el("span", {
      className: `dl-chunk dl-${t.state}`,
      style: `--w:${Math.max(0.12, Math.sqrt(t.bytes / total)).toFixed(3)}`,
      title: `${KINDS[t.kind] ?? t.kind}${t.what ? ` · ${t.what}` : ""} · ${kb(t.bytes)} · ${t.hash.slice(0, 15)}…`,
    }, el("b", { textContent: KINDS[t.kind] ?? t.kind }), el("small", { textContent: `${t.what ? `${t.what} · ` : ""}${kb(t.bytes)}` }))));
    const sum = (xs) => xs.reduce((n, t) => n + t.bytes, 0);
    const changed = b.list.filter((c) => !before.has(c.hash));
    const fresh = changed.filter((c) => !seen.has(c.hash));
    const kept = b.list.filter((c) => before.has(c.hash));
    const kinds = (xs) => [...new Set(xs.map((t) => KINDS[t.kind] ?? t.kind))].join(", ");
    summary.innerHTML = !from
      ? `<b>v${to}</b> as first published: ${b.list.length} chunks, ${kb(sum(b.list))} before compression — all of them new to a reader. Publish the next version to see what a republish costs.`
      : fresh.length < changed.length
        ? `<b>v${from} → v${to}</b>: ${changed.length} of ${b.list.length} chunks differ from v${from}, but ${fresh.length === 0 ? "every one of them" : `${changed.length - fresh.length} of them`} came with a version this page showed before — so ${fresh.length === 0 ? "going back costs a reader only the manifest" : `only ${fresh.length} (${kb(sum(fresh))}) are new`}.`
        : `<b>v${from} → v${to}</b>: ${changed.length} of ${b.list.length} chunks are new (${kb(sum(changed))}: ${kinds(changed)}); ${kept.length} (${kb(sum(kept))}) are the same bytes under the same names, so a reader who saw v${from} already has them.`;
    for (const c of b.list) seen.add(c.hash);
    cmd.textContent = `datars publish election.ts --alias election --to site/\n# v${to}: the manifest c/election is rewritten;\n# ${changed.length} chunk${changed.length === 1 ? "" : "s"} written, ${kept.length} already there`;
    if (fetched) {
      const got = fetched.filter((e) => e.transferSize > 0);
      const cached = fetched.filter((e) => e.transferSize === 0);
      measured.innerHTML = `<b>Your browser, just now</b>: the three embeds asked for ${fetched.length} chunk${fetched.length === 1 ? "" : "s"}; ${got.length} came over the network (${kb(got.reduce((n, e) => n + e.transferSize, 0))} transferred, compressed, headers included)${cached.length ? ` and ${cached.length} from its cache` : ""} — plus the manifest, which hosts revalidate on every load.`;
    } else {
      measured.textContent = "Nothing fetched for a republish yet.";
    }
  }

  async function go(to) {
    if (busy || to === current) return;
    busy = true;
    const from = current;
    current = to;
    if (state === "seats" && to < 3) state = "ranked";
    press();
    const t0 = performance.now();
    const fresh = boxes.map((box) => {
      const old = box.querySelector("datars-view");
      const d = view(box, to, stepIndex());
      d.classList.add("dl-incoming");
      box.append(d);
      views.add(d);
      return [box, old, d];
    });
    await Promise.all(fresh.map(([, , d]) => drawn(d)));
    for (const [, old, d] of fresh) {
      d.classList.remove("dl-incoming");
      if (old) { views.delete(old); setTimeout(() => old.remove(), reduced ? 0 : 260); }
    }
    // What the three views fetched for it (the chunk folder only; every embed shares one fetch).
    const fetched = performance.getEntriesByType("resource").filter((e) => e.startTime >= t0 && e.name.includes("/chunks/"));
    const seen = new Map();
    for (const e of fetched) if (!seen.has(e.name) || e.transferSize > seen.get(e.name).transferSize) seen.set(e.name, e);
    await ledger(from, to, [...seen.values()]);
    busy = false;
  }

  for (const b of vButtons) b.addEventListener("click", () => go(Number(b.dataset.v)));
  next.addEventListener("click", () => go(current < 3 ? current + 1 : 1));
  for (const p of pills) {
    p.addEventListener("click", () => {
      state = p.dataset.state;
      press();
      for (const box of boxes) box.querySelector("datars-view:not(.dl-incoming)")?.send(`goto:${state}`);
    });
  }

  // First load: version 1 in every embed, once the section comes near.
  new IntersectionObserver(([e], io) => {
    if (!e.isIntersecting) return;
    io.disconnect();
    for (const box of boxes) {
      const d = view(box, 1, 0);
      box.append(d);
      views.add(d);
      fit(box);
    }
    ledger(null, 1, null);
  }, { rootMargin: "50% 0px" }).observe(section);
  press();
}

// ---- tiers -------------------------------------------------------------------------------------

function tiers(section) {
  const tabs = [...section.querySelectorAll(".dl-tier-tabs button")];
  const poster = section.querySelector(".dl-tier-poster");
  const live = section.querySelector(".dl-tier-live");
  const cap = section.querySelector(".dl-tier-cap");
  const what = section.querySelector(".dl-tier-what");
  const list = section.querySelector(".dl-tier-list");
  let liveView = null;
  const ABOUT = {
    T0: "A poster (SVG, text as text) and the chart's text alternative. Needs no runtime at all: email, RSS, strict CMSs, runtimes too old to play the rest — and anything that refuses the others.",
    T1: "Baked scenes, one per state, and the plans between them: stories, flights, scrubbing, tooltips. No data, no recipes.",
    T2: "The document with its recipes pre-expanded and its data by hash: filters, brushes, sliders, linked views, live data and app data slots all stay live. What the web runtime plays by default.",
    T3: "The source document, for charts whose structure depends on runtime data. Needs the recipe sandbox, pinned to the author's standard library by hash.",
  };
  async function show(tier) {
    for (const b of tabs) b.setAttribute("aria-pressed", String(b.dataset.tier === tier));
    const m = await manifest(3);
    const t = await tierChunks(m, tier);
    what.textContent = ABOUT[tier];
    const modules = t?.variant.requires?.modules?.length ? `<code>${t.variant.requires.modules.join(", ")}</code>` : "no runtime";
    list.replaceChildren(
      el("p", { className: "dl-tier-req", innerHTML: `Needs: ${modules} · first load ${kb(t.list.reduce((n, c) => n + c.bytes, 0))} in ${t.list.length} chunks, before compression` }),
      el("ul", {}, ...t.list.map((c) => el("li", {}, el("i", { className: `dl-k dl-k-${c.kind}` }), el("span", { textContent: `${KINDS[c.kind] ?? c.kind}${c.what ? ` · ${c.what}` : ""}` }), el("b", { textContent: kb(c.bytes) })))),
    );
    if (tier === "T0") {
      live.hidden = true;
      poster.hidden = false;
      if (!poster.childElementCount) {
        const ph = m.chunks.find((c) => c.kind === "poster")?.hash;
        const a = m.chunks.find((c) => c.kind === "a11y")?.hash;
        const [svg, text] = await Promise.all([ph ? fetch(chunkUrl(ph)).then((r) => r.text()) : "", a ? fetch(chunkUrl(a)).then((r) => r.json()) : null]);
        poster.replaceChildren(el("img", { alt: "", src: `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}` }));
        poster.dataset.alt = `${text?.title ?? ""}. ${text?.description ?? ""}`;
      }
      cap.replaceChildren("The poster chunk itself, drawn as an image; with it, the accessible-text chunk: ", el("i", { textContent: `“${poster.dataset.alt}”` }));
    } else {
      poster.hidden = true;
      live.hidden = false;
      if (!liveView) {
        liveView = view(live, 3, 0);
        live.append(liveView);
        views.add(liveView);
        fit(live);
      }
      const engine = () => liveView.dataset.engine;
      const say = () => {
        const plays = engine() === "full" ? "T3" : "T2";
        cap.textContent = tier === plays
          ? `This is the variant your browser is playing now (the ${engine()} runtime).`
          : tier === "T1"
            ? "T1 holds the same frames as baked scenes, for runtimes without the modules T2 needs (listed beside). A page can't pick a tier, so this is the T2 your browser chose."
            : `Your browser plays ${plays} here (the ${engine() ?? "core"} runtime): T3 is for runtimes with the recipe sandbox, and charts whose structure depends on their data.`;
      };
      if (engine()) say(); else { cap.textContent = ""; drawn(liveView).then(say); }
    }
  }
  for (const b of tabs) b.addEventListener("click", () => show(b.dataset.tier));
  new IntersectionObserver(([e], io) => { if (e.isIntersecting) { io.disconnect(); show("T0"); } }, { rootMargin: "50% 0px" }).observe(section);
}

// ---- signed --------------------------------------------------------------------------------------

function signed(section) {
  const box = section.querySelector(".dl-signed");
  const state = section.querySelector(".dl-signed-state");
  // The runtime's refusal arrives as a rejected promise from inside <datars-view> (it has no error
  // event): it's what this demo is for, so show it rather than leave it to the console.
  let refusal = "";
  addEventListener("unhandledrejection", (e) => {
    const m = String(e.reason?.message ?? e.reason ?? "");
    if (!/publisher|signature|unsigned/i.test(m)) return;
    e.preventDefault();
    refusal = m;
  });
  new IntersectionObserver(([e], io) => {
    if (!e.isIntersecting) return;
    io.disconnect();
    const d = view(box, 1, 0);
    box.append(d);
    views.add(d);
    fit(box);
    setTimeout(() => { state.textContent = d.status ? "(It played — the check didn't refuse it.)" : refusal ? `The runtime, in your browser just now: “${refusal}”.` : "Checked in your browser just now: the view never started its engine."; }, 3000);
  }, { rootMargin: "50% 0px" }).observe(box);
}

const r = document.getElementById("republish");
if (r) republish(r);
const t = document.getElementById("tiers");
if (t) tiers(t);
const s = document.getElementById("signed");
if (s) signed(s);
