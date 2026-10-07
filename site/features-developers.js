// The developers page (site/pages/features/developers.html): the bench and the terminal.
//
// The bench is a live chart, started from its document (`setDocument`, so the runtime with the
// recipe sandbox plays it and every change is a morph), with three deliberate flaws (site/figures/developers/_bench.ts). Each
// fix the reader ticks picks one of the eight published combinations — its document, fetched from
// /play/developers-bench-<key><colour><credit>.json — and hands it to the running view with
// `setDocument`: the chart morphs to it. What lint says about it is `datars lint`'s own output for
// that file, captured when the site was built. Clicking a mark asks the running chart, through the
// public API, what's there (`hitTest(x, y)`) and why it looks so (`explain(path)`): its data row,
// its template and every expression with the value it has now.
//
// The terminal replays real CLI output, captured at build time ({{run:…}}): the command is typed,
// the output appears — at once for a reader who asked for reduced motion.
import { highlight } from "./site.js";
// The page mounts its bench itself (from a document, not a published bundle), so it brings the
// runtime along too.
import "./runtime/datars.js";

const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;

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

const el = (tag, props = {}, ...kids) => {
  const n = Object.assign(document.createElement(tag), props);
  for (const k of kids) if (k != null) n.append(k);
  return n;
};
const show = (v) => (typeof v === "string" ? JSON.stringify(v) : typeof v === "number" ? String(Math.round(v * 1000) / 1000) : JSON.stringify(v));

// ---- the bench ---------------------------------------------------------------------------------

/** Start the bench's <datars-view> from a document, in the slot's reserved box, in the page's
 * mode. Its poster (the build's render of the first state) stays under it until it has drawn. */
function mount(slot, doc) {
  const view = document.createElement("datars-view");
  view.setAttribute("aria-label", slot.dataset.label);
  view.setAttribute("no-controls", ""); // the page has its own step pills
  const root = document.documentElement;
  const mode = () => (root.dataset.theme === "dark" ? "dark" : "light");
  view.setAttribute("mode", mode());
  new MutationObserver(() => view.setAttribute("mode", mode())).observe(root, { attributes: true, attributeFilter: ["data-theme"] });
  const fit = () => {
    const h = String(Math.round(slot.clientHeight));
    if (view.getAttribute("height") !== h) view.setAttribute("height", h);
  };
  fit();
  new ResizeObserver(fit).observe(slot);
  view.setDocument(doc);
  slot.append(view);
  view.addEventListener("state", () => {
    const drawn = () => (view.dataset.renderer ? setTimeout(() => slot.classList.add("dv-live"), 400) : setTimeout(drawn, 100));
    drawn();
  }, { once: true });
  return view;
}

function bench(section) {
  const slot = section.querySelector(".dv-slot");
  const frame = section.querySelector(".dv-frame");
  const hoverBox = section.querySelector(".dv-hover");
  const pinBox = section.querySelector(".dv-pin");
  const status = section.querySelector(".dv-status");
  const lintOut = section.querySelector(".dv-lintout");
  const codeEl = section.querySelector("#dv-code");
  const explainOut = section.querySelector(".dv-explain");
  const fixes = [...section.querySelectorAll("input[data-fix]")];
  const tabs = [...section.querySelectorAll(".dv-tabs button")];
  const F = { key: false, colour: false, credit: false };
  const variant = () => `${+F.key}${+F.colour}${+F.credit}`;
  const docs = new Map();
  const docUrl = (v) => new URL(slot.dataset.src.replace(/developers-bench-\d+\.json$/, `developers-bench-${v}.json`), location.href);
  const fetchDoc = (v) => {
    if (!docs.has(v)) docs.set(v, fetch(docUrl(v)).then((r) => r.json()));
    return docs.get(v);
  };
  let view = null;
  let loaded = "000";
  let pinned = null; // the path explained

  // ---- tabs ----
  function tab(name) {
    for (const b of tabs) {
      const on = b.dataset.tab === name;
      b.setAttribute("aria-selected", String(on));
      b.setAttribute("aria-pressed", String(on));
    }
    for (const p of section.querySelectorAll(".dv-tab")) p.hidden = p.dataset.tab !== name;
  }
  for (const b of tabs) b.addEventListener("click", () => tab(b.dataset.tab));

  // ---- lint and fix ----
  function code() {
    const lines = [
      "data: {",
      `  countries: data.values({ name, continent, people }${F.key ? ', { key: "name" }' : ""}),${F.key ? "  // fixed" : ""}`,
      "},",
      "tables: {",
      '  ranked: { from: "countries", ops: [op.sort(["people", "desc"])] },',
      '  az: { from: "countries", ops: [op.sort("name")] },',
      "},",
      "// … each table as a plot of bars:",
      "plot({",
      `  data: table, x: "people", y: "name", color: ${F.colour ? '"continent"' : '"name"'},${F.colour ? "  // fixed" : ""}`,
      `  legend: ${F.colour}, children: [bar({ labels: true, format: ",.0f" })],`,
      "}),",
      'text("Rounded 2024 estimates (approximate)", [e("box.w"), 0], {',
      `  style: { size: 11, ink: "$muted", align: ${F.credit ? '"end"' : '"start"'} },${F.credit ? "  // fixed" : ""}`,
      "}),",
    ];
    codeEl.textContent = lines.join("\n");
    highlight(codeEl);
  }
  function lint() {
    const t = section.querySelector(`template.dv-lint[data-v="${variant()}"]`);
    lintOut.replaceChildren(t ? t.content.cloneNode(true) : el("p", { textContent: "—" }));
    const text = lintOut.textContent;
    const n = (text.match(/^warning|^error/gm) ?? []).length;
    status.textContent = n === 0
      ? "No lint findings. Step to “A–z”: each bar travels to its place."
      : F.key
        ? `${n} finding${n === 1 ? "" : "s"} left.`
        : "Step to “A–z”: without keys the bars don't move, they melt into their neighbours.";
  }
  async function apply() {
    const v = variant();
    code();
    lint();
    if (!view || loaded === v) return;
    loaded = v;
    const doc = await fetchDoc(v);
    if (variant() !== v) return; // ticked again meanwhile
    view.setDocument(doc);
    unpin();
  }
  for (const f of fixes) {
    f.addEventListener("change", () => {
      F[f.dataset.fix] = f.checked;
      apply();
    });
  }

  // ---- explain ----
  /** Where an element's bounds sit in the frame (bounds are CSS px from the view's top left). */
  function place(box, b) {
    const v = view.getBoundingClientRect(), f = frame.getBoundingClientRect();
    box.style.left = `${v.left - f.left + b[0]}px`;
    box.style.top = `${v.top - f.top + b[1]}px`;
    box.style.width = `${Math.max(2, b[2])}px`;
    box.style.height = `${Math.max(2, b[3])}px`;
    box.hidden = false;
  }
  function unpin() {
    pinned = null;
    pinBox.hidden = hoverBox.hidden = true;
  }
  const at = (e) => { const r = view.getBoundingClientRect(); return [e.clientX - r.left, e.clientY - r.top]; };
  /** The element a reader means at a point: the topmost datum or region, else the topmost thing. */
  const pick = (hits) => hits.find((h) => h.role === "datum" || h.role === "region") ?? hits.find((h) => h.label || h.text) ?? hits[0];

  function explainPath(path, hits) {
    const why = view.explain(path);
    const w = why?.[0];
    if (!w) return;
    pinned = path;
    if (w.bounds) place(pinBox, w.bounds);
    const rows = [];
    rows.push(el("h3", { textContent: "The element" }));
    rows.push(el("p", { className: "dv-path" }, el("code", { textContent: w.path })));
    rows.push(el("pre", { className: "dv-node" }, el("code", { textContent: w.node })));
    rows.push(el("h3", { textContent: "Recipes" }));
    rows.push(el("p", { className: "dv-small", textContent: w.recipes?.length ? w.recipes.join(" → ") : "None named by the running chart: what this page plays is the published, pre-expanded document. The CLI on the source names them — @datars/std/plot → @datars/std/bar for these bars (see the terminal below)." }));
    if (w.row) {
      rows.push(el("h3", { textContent: "Data row" }));
      const t = el("table", { className: "dv-row" });
      t.append(el("caption", { textContent: `${w.row.table} #${w.row.index}` }));
      for (const [k, v] of Object.entries(w.row.fields ?? {})) t.append(el("tr", {}, el("th", { textContent: k }), el("td", {}, el("code", { textContent: show(v) }))));
      rows.push(t);
    }
    if (w.values?.length) {
      rows.push(el("h3", { textContent: "Every expression, and its value now" }));
      const t = el("table", { className: "dv-values" });
      for (const x of w.values) t.append(el("tr", {}, el("th", { textContent: x.field }), el("td", {}, el("code", { className: "dv-expr", textContent: x.expr }), el("span", { className: "dv-arrow", textContent: " → " }), el("code", { className: "dv-val", textContent: show(x.value) }))));
      rows.push(t);
    }
    if (hits?.length > 1) {
      rows.push(el("h3", { textContent: "Everything under the pointer" }));
      const ul = el("ul", { className: "dv-hits" });
      for (const h of hits) {
        const b = el("button", { type: "button", textContent: `${h.kind}${h.role ? ` · ${h.role}` : ""}${h.label || h.text ? ` · “${h.label || h.text}”` : ""}` });
        if (h.path === path) b.setAttribute("aria-current", "true");
        b.addEventListener("click", () => explainPath(h.path, hits));
        ul.append(el("li", {}, b));
      }
      rows.push(ul);
    }
    explainOut.replaceChildren(...rows);
  }

  code();
  lint();
  // Steps: the pills under the chart.
  const pills = [...section.querySelectorAll(".dv-steps button")];
  for (const p of pills) p.addEventListener("click", () => view?.send(`goto:${p.dataset.state}`));
  const start = fetchDoc("000").then((doc) => mount(slot, doc));
  start.then((v) => whenReady(slot).then(() => v)).then((v) => {
    view = v;
    v.addEventListener("state", (e) => {
      for (const p of pills) p.setAttribute("aria-pressed", String(p.dataset.state === e.detail?.state));
    });
    // Hover: outline what a click would explain. Click: explain it.
    v.addEventListener("pointermove", (e) => {
      if (e.pointerType === "touch") return;
      const h = pick(v.hitTest(...at(e)));
      if (h?.bounds && h.path !== pinned) place(hoverBox, h.bounds);
      else hoverBox.hidden = true;
    });
    v.addEventListener("pointerleave", () => { hoverBox.hidden = true; });
    v.addEventListener("click", (e) => {
      const hits = v.hitTest(...at(e));
      const h = pick(hits);
      if (!h) return;
      hoverBox.hidden = true;
      tab("explain");
      explainPath(h.path, hits);
    });
    // A step moves everything: what was explained is somewhere else now.
    v.addEventListener("state", () => {
      if (!pinned) return;
      const w = v.explain(pinned)?.[0];
      if (w) explainPath(pinned, null);
      else unpin();
    });
    if (variant() !== "000") apply();
  });
}

// ---- the terminal ------------------------------------------------------------------------------

function terminal(section) {
  const screen = section.querySelector(".dv-screen");
  const code = screen.querySelector("code");
  const what = section.querySelector(".dv-what");
  const buttons = [...section.querySelectorAll(".dv-cmds button")];
  const order = buttons.map((b) => b.dataset.cmd);
  let run = 0, started = false;
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  const frame = () => new Promise((r) => requestAnimationFrame(r));
  /** A command's prompt line and output, as the build captured them. */
  function captured(cmd) {
    const t = section.querySelector(`template.dv-run[data-cmd="${cmd}"]`);
    const src = t.content.querySelector("code");
    const prompt = src.querySelector(".prompt")?.textContent ?? "";
    return { prompt, output: src.textContent.slice(prompt.length).replace(/^\n/, ""), what: t.dataset.what };
  }
  const press = (cmd) => { for (const b of buttons) b.setAttribute("aria-pressed", String(b.dataset.cmd === cmd)); };
  const bottom = () => { screen.scrollTop = screen.scrollHeight; };
  /** Type one command and print its output, after what's on screen. */
  async function one(cmd, my) {
    const c = captured(cmd);
    press(cmd);
    what.textContent = c.what;
    if (code.childNodes.length) code.append("\n\n");
    const p = el("span", { className: "prompt" });
    code.append(p);
    const out = document.createTextNode("");
    if (reduced) {
      p.textContent = c.prompt;
      code.append("\n", out);
      out.textContent = c.output;
      bottom();
      return true;
    }
    for (let i = 1; i <= c.prompt.length; i++) {
      if (my !== run) return false;
      p.textContent = c.prompt.slice(0, i);
      bottom();
      await wait(i < 9 ? 45 : 12);
    }
    await wait(280);
    if (my !== run) return false;
    code.append("\n", out);
    const lines = c.output.split("\n");
    for (let i = 0; i < lines.length; i += 3) {
      if (my !== run) return false;
      out.textContent = lines.slice(0, i + 3).join("\n");
      bottom();
      await frame();
    }
    return true;
  }
  /** The whole session from `from`, one command after another, while it's on screen. */
  async function session(from = 0) {
    const my = ++run;
    code.replaceChildren();
    for (let i = from; i < order.length; i++) {
      if (!(await one(order[i], my))) return;
      await wait(reduced ? 0 : 1800);
      while (!visible || document.hidden) { await wait(300); if (my !== run) return; }
    }
  }
  let visible = false;
  for (const b of buttons) b.addEventListener("click", () => { const my = ++run; code.replaceChildren(); one(b.dataset.cmd, my); });
  section.querySelector(".dv-replay").addEventListener("click", () => session(0));
  new IntersectionObserver(([e]) => {
    visible = e.isIntersecting;
    if (visible && !started) { started = true; session(0); }
  }, { threshold: 0.35 }).observe(section.querySelector(".dv-window"));
  // Before it's seen: the session's first commands as they printed (no blank window).
  for (const cmd of order.slice(0, 2)) {
    const c = captured(cmd);
    if (code.childNodes.length) code.append("\n\n");
    code.append(el("span", { className: "prompt", textContent: c.prompt }), `\n${c.output}`);
  }
  what.textContent = captured(order[0]).what;
}

const b = document.getElementById("bench");
if (b) bench(b);
const t = document.getElementById("terminal");
if (t) terminal(t);
