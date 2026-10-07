// The extensibility page (site/pages/features/extensibility.html): the recipe workbench.
//
// The reader edits a recipe module (the ejected std/bar, or a recipe written from scratch) or a
// scene's JSON, and the chart beside it follows as they type. What runs is the real thing: the
// document carries the module as an inline package (`packages: [{ name, source }]`, exactly what
// the CLI embeds when it builds a document with local recipes), `view.setDocument()` hands it to
// the engine, and the engine's own QuickJS sandbox — compiled into its wasm — expands the recipe
// and morphs the chart to the result. Errors come back as the engine's diagnostics, with the
// module's line and column; then the last good document goes back on screen in the same task, so
// nothing broken is ever drawn.
//
// Each edit is registered under a new package name (`@local/bar-7`): the sandbox keeps a module
// it has evaluated under its name, so a new source under the same name wouldn't be picked up.
// The doc.ts shown uses the stable names a project would (`./recipes/bar`).
//
// Each tab starts from a figure the build checks like any other (site/figures/extensibility/*.ts,
// published as documents at /play/extensibility-<name>.json); the editor's starting text is in the
// page (`<script type="text/plain">`): the figure's recipe without its type annotations.
import { toTs } from "./features-charts.js";
import { highlight } from "./site.js";

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

/** The starting points: a figure, the module's name in a project, the recipe's export, the labels of
 * the two states, and small edits to try (each replaces text in the starting source). */
const TABS = {
  eject: {
    figure: "extensibility-eject", module: "bar", file: "recipes/bar.ts", states: ["Your copy", "std/bar"],
    tries: [
      { label: "Round the ends", find: /r: p\.radius \|\| "\$radius\.bar"/g, to: 'r: e(horizontal ? "scale.y.bandwidth() / 2" : "scale.x.bandwidth() / 2")' },
      { label: "Highlight the largest", find: "          fill,\n", to: '          fill: e(`d.${val} == table.max(${JSON.stringify(p.data)}, ${JSON.stringify(val)}) ? "$accent" : "$muted"`),\n' },
      {
        label: "A dot on each end", find: "        p.labels ? group(",
        to: "        // A marker of your own at the end of every bar.\n        repeat(p.data, shape(geom.circle({\n          cx: horizontal ? e(`scale.x(d.${val})`) : e(`scale.x(d.${cat}) + scale.x.bandwidth() / 2`),\n          cy: horizontal ? e(`scale.y(d.${cat}) + scale.y.bandwidth() / 2`) : e(`scale.y(d.${val})`),\n          r: 7,\n        }), { key: e(`\"dot-\" + d.${cat}`), fill: \"$ink\", stroke: { paint: \"$paper\", width: 2 }, semantics: { role: \"decoration\" } })),\n        p.labels ? group(",
      },
    ],
  },
  isotype: {
    figure: "extensibility-isotype", module: "isotype", file: "recipes/isotype.ts", states: ["Your isotype", "std/bar"],
    tries: [
      { label: "A gap every ten", find: "x: e(`112 + (d.i + 0.5) * ${step}`),", to: "x: e(`112 + (d.i + 0.5) * ${step} + floor(d.i / 10) * 5`)," },
      {
        label: "Count at the end", find: "        // Instanced marks",
        to: "        // The count after each row.\n        repeat(p.data, text(e(`String(d.${p.value})`), [e(`112 + d.${p.value} * ${step} + 8`), e(y)], {\n          key: e(`\"count-\" + d.${p.category}`),\n          style: { size: \"$size.label\", weight: 600, ink: \"$ink\", baseline: \"middle\" },\n        })),\n        // Instanced marks",
      },
      { label: "Bigger dots", find: 'size: t.number(12, "', to: 'size: t.number(20, "' },
    ],
  },
  scene: { figure: "extensibility-scene", module: null, file: "doc.json", states: ["Your scene", "std/pie"], tries: [] },
};
const STATES = ["yours", "std"];

function workbench(section) {
  const slot = section.querySelector(".chart[data-src]");
  const own = slot.closest("[data-own-look]");
  const tabs = [...section.querySelectorAll(".xe-tabs button[data-tab]")];
  const input = section.querySelector(".xe-input");
  const hl = section.querySelector(".xe-hl code");
  const hlPre = section.querySelector(".xe-hl");
  const gutter = section.querySelector(".xe-gutter");
  const file = section.querySelector(".xe-file");
  const tries = section.querySelector(".xe-tries");
  const resetBtn = section.querySelector(".xe-reset");
  const status = section.querySelector(".xe-status");
  const pills = [...section.querySelectorAll(".xe-states button[data-state]")];
  const codeEl = section.querySelector("#xe-doc");
  const root = new URL(slot.dataset.src.replace(/\/c\/[^/]+$/, "/"), location.href);

  const figures = new Map();
  const figure = (name) => {
    if (!figures.has(name)) figures.set(name, fetch(new URL(`play/${name}.json`, root)).then((r) => r.json()));
    return figures.get(name);
  };
  const texts = {}; // the reader's text per tab (kept when they switch tabs)
  const starts = {}; // each tab's starting text

  let tab = "eject";
  let base = null; // the tab's figure document
  let view = null;
  let good = null; // the last document that expanded without an error
  let state = "yours";
  let edits = 0; // package names used
  let typed = 0; // the reader's edits that reached the engine
  let errLine = 0;

  // ---- the editor ----
  function paint() {
    const text = input.value;
    hl.textContent = text.endsWith("\n") ? `${text} ` : text;
    highlight(hl);
    const n = text.split("\n").length;
    if (gutter.childElementCount !== n || gutter.dataset.err !== String(errLine)) {
      gutter.dataset.err = String(errLine);
      gutter.replaceChildren(...Array.from({ length: n }, (_, i) => {
        const s = document.createElement("span");
        s.textContent = String(i + 1);
        if (i + 1 === errLine) s.className = "err";
        return s;
      }));
    }
    sync();
  }
  function sync() {
    hlPre.scrollTop = input.scrollTop;
    hlPre.scrollLeft = input.scrollLeft;
    gutter.scrollTop = input.scrollTop;
  }
  input.addEventListener("scroll", sync);
  input.addEventListener("keydown", (e) => {
    // Tab indents (two spaces), as in a code editor; Escape then Tab leaves the editor.
    if (e.key === "Tab" && !e.shiftKey && !input.dataset.escaped) {
      e.preventDefault();
      // insertText keeps the browser's undo and fires `input`; where it's gone, by hand.
      if (!document.execCommand?.("insertText", false, "  ")) {
        input.setRangeText("  ", input.selectionStart, input.selectionEnd, "end");
        onEdit();
      }
    } else if (e.key === "Escape") input.dataset.escaped = "1";
  });
  input.addEventListener("blur", () => { delete input.dataset.escaped; });
  let timer = 0;
  function onEdit() {
    typed++;
    texts[tab] = input.value;
    paint();
    showTries();
    clearTimeout(timer);
    timer = setTimeout(run, 320);
  }
  input.addEventListener("input", onEdit);

  // ---- the tries ----
  function showTries() {
    const list = TABS[tab].tries;
    if (tries.childElementCount !== list.length || tries.dataset.tab !== tab) {
      tries.dataset.tab = tab;
      tries.replaceChildren(...list.map((t, i) => {
        const b = Object.assign(document.createElement("button"), { type: "button", textContent: t.label });
        b.dataset.i = String(i);
        return b;
      }));
    }
    list.forEach((t, i) => {
      const b = tries.children[i];
      const can = typeof t.find === "string" ? input.value.includes(t.find) : (t.find.lastIndex = 0, t.find.test(input.value));
      b.disabled = !can;
      b.title = can ? "Make this edit in the code" : "Done — or your edits changed the lines it replaces (Reset to start over)";
    });
    tries.closest(".xe-try-row").hidden = !list.length;
  }
  tries.addEventListener("click", (e) => {
    const b = e.target.closest("button[data-i]");
    if (!b || b.disabled) return;
    const t = TABS[tab].tries[Number(b.dataset.i)];
    const before = input.value;
    const after = typeof t.find === "string" ? before.replace(t.find, t.to) : before.replace(t.find, t.to);
    // Where the edit lands, so the reader sees what changed.
    let at = 0;
    while (at < before.length && before[at] === after[at]) at++;
    input.value = after;
    input.focus({ preventScroll: true });
    input.setSelectionRange(at, at);
    const line = after.slice(0, at).split("\n").length;
    input.scrollTop = Math.max(0, (line - 4) * parseFloat(getComputedStyle(input).lineHeight));
    onEdit();
  });
  resetBtn.addEventListener("click", () => {
    input.value = starts[tab];
    onEdit();
  });

  // ---- documents ----
  /** The tab's document with the reader's text in it: as sent to the engine (`live`, a fresh
   * package name), or as a project would write it. */
  function compose(text, live) {
    const d = structuredClone(base);
    const t = TABS[tab];
    if (t.module) {
      const from = `@local/${t.module}`;
      const name = live ? `@local/${t.module}-${++edits}` : from;
      const walk = (v) => {
        if (Array.isArray(v)) return v.forEach(walk);
        if (!v || typeof v !== "object") return;
        if (v.kind === "use" && typeof v.recipe === "string" && v.recipe.startsWith(`${from}/`)) v.recipe = name + v.recipe.slice(from.length);
        Object.values(v).forEach(walk);
      };
      walk(d.scene);
      d.packages = [{ name, source: text }];
    } else {
      // The scene tab: the reader's tables and the node for the first state.
      const part = JSON.parse(text);
      if (!part || typeof part !== "object" || !part.scene || typeof part.scene !== "object") throw new SyntaxError('Expected { "tables": { … }, "scene": { … } }');
      d.tables = { ...(part.tables ?? {}) };
      const i = d.scene.children.findIndex((n) => n.when?.expr?.includes('"yours"'));
      d.scene.children[i] = { ...part.scene, when: d.scene.children[i].when };
    }
    return d;
  }
  /** The starting text of the scene tab: its figure's own tables and node, as JSON. */
  function sceneText(d) {
    const node = structuredClone(d.scene.children.find((n) => n.when?.expr?.includes('"yours"')));
    delete node.when;
    return JSON.stringify({ tables: d.tables, scene: node }, null, 2);
  }

  /** Messages about the reader's code (not fonts on their way), and the line of the first error. */
  function problems(diags, name) {
    const mine = (diags ?? []).filter((d) => (name && d.includes(name)) || /^recipe `@local\//.test(d) || /expansion|template|unknown table|expression/.test(d));
    let line = 0;
    for (const d of mine) {
      const m = d.match(/@local\/[\w-]+:(\d+):(\d+)/);
      if (m) { line = Number(m[1]); break; }
    }
    return { mine, line, fatal: mine.some((d) => /Error|took too long|not a valid template|expansion/.test(d)) };
  }
  function say(kind, text) {
    status.dataset.kind = kind;
    status.textContent = text;
  }

  async function run() {
    if (!view || !base) return;
    let d;
    try {
      d = compose(input.value, true);
    } catch (e) {
      // Bad JSON: the engine never sees it.
      const pos = Number(String(e.message).match(/position (\d+)/)?.[1]);
      errLine = Number.isFinite(pos) ? input.value.slice(0, pos).split("\n").length : Number(String(e.message).match(/line (\d+)/)?.[1] ?? 0);
      paint();
      return say("error", `${errLine ? `Line ${errLine}: ` : ""}${e.message} — the chart keeps the last version that worked.`);
    }
    // A document the engine can't read at all (a node that isn't IR) is refused whole: the view
    // keeps what it had. Awaited, the answer is still here before the next frame.
    try {
      await view.setDocument(d);
    } catch (e) {
      errLine = 0;
      paint();
      return say("error", `${String(e?.message ?? e).split("\n")[0]} — the chart keeps the last version that worked.`);
    }
    const name = d.packages?.[0]?.name;
    const p = problems(view.status?.diagnostics, name);
    if (p.fatal && good) {
      // Back to the last good document before anything is drawn (same task, no frame between).
      view.setDocument(good);
      errLine = p.line;
      paint();
      const msg = p.mine[0].replace(/^recipe `@local\/[\w-]+\/(\w+)`: /, "$1: ").replace(/\n\s+at [\s\S]*$/, "").replace(/@local\/[\w-]+:(\d+):(\d+)/g, "line $1, column $2");
      return say("error", `${p.line ? `Line ${p.line}: ` : ""}${msg} — the chart keeps the last version that worked.`);
    }
    good = d;
    errLine = 0;
    paint();
    if (p.mine.length) say("note", p.mine.map((m) => m.replace(/^recipe `@local\/[\w-]+\/(\w+)`: /, "$1: ")).join(" · "));
    else say("ok", tab === "scene" ? "Your scene, drawn by the engine — the same nodes a recipe returns." : typed ? "Expanded again in the engine's sandbox, in this page, as you typed." : "Expanded by the engine's sandbox, in this page. Edit the code, or try an edit below it.");
    code();
  }
  function code() {
    let d;
    try { d = compose(input.value, false); } catch { return; }
    const t = TABS[tab];
    const header = t.module
      ? `// doc.ts — beside it, ${t.file}: the code on the left, with its types.\n`
      : "// doc.ts — the scene on the left is part of this document.\n";
    codeEl.textContent = toTs(d, { local: t.module ? { [`@local/${t.module}`]: `./recipes/${t.module}` } : {}, header });
    highlight(codeEl);
  }

  // ---- states ----
  function press(s) {
    state = s;
    for (const b of pills) b.setAttribute("aria-pressed", String(b.dataset.state === s));
  }
  for (const b of pills) b.addEventListener("click", () => {
    press(b.dataset.state);
    view?.send(`goto:${b.dataset.state}`);
  });

  // ---- tabs ----
  async function open(next) {
    tab = next;
    for (const b of tabs) b.setAttribute("aria-selected", String(b.dataset.tab === next));
    base = await figure(TABS[next].figure);
    if (tab !== next) return;
    if (!starts[next]) starts[next] = TABS[next].module ? document.getElementById(`xe-src-${next}`).textContent.replace(/^\n/, "") : sceneText(base);
    input.value = texts[next] ?? starts[next];
    typed = texts[next] && texts[next] !== starts[next] ? 1 : 0;
    input.scrollTop = 0;
    input.setAttribute("aria-label", `${TABS[next].file}: editable`);
    file.textContent = TABS[next].module ? `${TABS[next].file} (as JavaScript)` : "doc.json: your part of the document";
    pills.forEach((b, i) => { b.textContent = TABS[next].states[i]; });
    errLine = 0;
    good = null;
    paint();
    showTries();
    if (view) {
      run();
      // The new document opens in the state of the same name: say which.
      press(view.status?.state ?? state);
    }
  }
  for (const b of tabs) b.addEventListener("click", () => { if (b.dataset.tab !== tab) open(b.dataset.tab); });

  // The chart starts from the first document: the slot waits for it (`data-own-look`), so the view
  // asks for the engine build with the recipe sandbox.
  slot.addEventListener("chartmount", (e) => {
    const v = e.detail.view;
    try {
      good = compose(input.value, true);
      v.setDocument(good);
    } catch { /* the figure's own document plays */ }
    whenReady(slot).then((ready) => {
      view = ready;
      view.addEventListener("state", (ev) => { if (ev.detail?.state) press(ev.detail.state); });
      run();
    });
  });

  (async () => {
    await open("eject");
    own.dataset.ownLook = "ready";
    document.dispatchEvent(new Event("ownlookready"));
  })();
}

const wb = document.getElementById("workbench");
if (wb) workbench(wb);
