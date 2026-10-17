// The accessibility page (site/pages/features/accessibility.html): the lab — one chart, and what
// each kind of reader gets from it.
//
// Everything through <datars-view>'s public API, nothing from inside it:
// - Screen reader: `readout` — the order a screen reader walks (the chart's name and summary,
//   headings, axes, each mark's name and value, checkboxes and switches with their state), each
//   with its box, outlined over the chart; the `state` event's narration is what the live region
//   speaks.
// - Keyboard: `keydown` on the element, and the `signal` and `state` events for what a key did.
// - Text size: the `textScale` property (the `text-scale` attribute).
// - Vision: an SVG colour matrix over the drawn chart (the engine's own CVD matrices), the `mode`
//   attribute for high contrast, and `setTokens` to show what forced colours do.
// - Motion: the `reduced-motion` attribute, and `send` to play the steps.
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

const reducedQuery = matchMedia("(prefers-reduced-motion: reduce)");
const NAMES = { S: "Social Democrats", SD: "Sweden Democrats", M: "Moderates", V: "Left", C: "Centre", KD: "Christian Democrats", MP: "Greens", L: "Liberals" };

function lab(section) {
  const slot = section.querySelector('.chart[data-chart="accessibility-lab"]');
  const viewBox = section.querySelector(".al-view");
  const ring = section.querySelector(".al-reading");
  const tree = section.querySelector(".al-tree");
  const pos = section.querySelector(".al-pos");
  const said = section.querySelector(".al-said");
  const figname = section.querySelector(".al-figname");
  const summary = section.querySelector(".al-summary");
  const keylog = section.querySelector(".al-keylog");
  let view = null;
  let items = [];
  let at = -1; // the item being read
  let tab = "sr";

  // ---- tabs ----
  const tabs = [...section.querySelectorAll('.al-tabs [role="tab"]')];
  function select(t, focus = false) {
    for (const b of tabs) {
      const on = b === t;
      b.setAttribute("aria-selected", String(on));
      b.setAttribute("aria-pressed", String(on));
      b.tabIndex = on ? 0 : -1;
      section.querySelector(`#${b.getAttribute("aria-controls")}`).hidden = !on;
    }
    tab = t.id.replace("al-tab-", "");
    if (focus) t.focus();
    place();
  }
  for (const t of tabs) {
    t.addEventListener("click", () => select(t));
    t.addEventListener("keydown", (e) => {
      const i = tabs.indexOf(t);
      const to = e.key === "ArrowRight" ? tabs[(i + 1) % tabs.length] : e.key === "ArrowLeft" ? tabs[(i + tabs.length - 1) % tabs.length] : e.key === "Home" ? tabs[0] : e.key === "End" ? tabs[tabs.length - 1] : null;
      if (to) { e.preventDefault(); select(to, true); }
    });
  }

  // ---- the screen reader's list ----
  /** What the kind of each stop is called, as a reader hears it. */
  const KIND = { chart: "figure", heading: "heading", text: "text", datum: "data", button: "button", checkbox: "checkbox", switch: "switch", radio: "radio" };
  function describe() {
    if (!view) return;
    const r = view.readout;
    if (!r) return;
    const [chart, ...next] = r;
    figname.textContent = chart?.label ?? "Chart";
    summary.textContent = chart?.value || "";
    const key = (list) => list.map((i) => `${i.kind}|${i.label}|${i.value}|${i.checked}`).join("\n");
    if (key(next) === key(items)) {
      items = next; // the boxes may have moved
      return place();
    }
    const label = at >= 0 ? items[at]?.label : null;
    items = next;
    tree.replaceChildren(...items.map((it, i) => {
      const li = document.createElement("li");
      li.style.setProperty("--depth", it.kind === "heading" ? "0" : "1");
      const b = document.createElement("button");
      b.type = "button";
      b.className = "al-item";
      b.dataset.i = String(i);
      const role = document.createElement("span");
      const acts = ["button", "checkbox", "switch", "radio"].includes(it.kind);
      role.className = `al-role${acts ? " al-act" : ""}`;
      role.textContent = KIND[it.kind] ?? it.kind;
      const text = document.createElement("span");
      text.className = "al-label";
      const state = it.checked === undefined ? "" : it.kind === "switch" ? (it.checked ? ", on" : ", off") : it.checked ? ", checked" : ", not checked";
      text.textContent = `${it.label}${it.value ? `, ${it.value}` : ""}${state}`;
      b.append(role, text);
      b.addEventListener("click", () => read(i));
      li.appendChild(b);
      return li;
    }));
    // Keep reading where we were, if that item's still there.
    const again = label ? items.findIndex((i) => i.label === label) : -1;
    read(again >= 0 ? again : Math.min(at, items.length - 1), false);
  }
  function read(i, scroll = true) {
    at = i;
    for (const b of tree.querySelectorAll(".al-item")) b.setAttribute("aria-current", String(Number(b.dataset.i) === i));
    pos.textContent = i >= 0 ? `Item ${i + 1} of ${items.length}` : `${items.length} items`;
    const cur = tree.querySelector(`.al-item[data-i="${i}"]`);
    // Into view inside the list only (scrollIntoView would scroll the page too).
    if (cur && scroll) {
      const top = cur.offsetTop, bottom = top + cur.offsetHeight;
      if (top < tree.scrollTop) tree.scrollTop = top - 4;
      else if (bottom > tree.scrollTop + tree.clientHeight) tree.scrollTop = bottom - tree.clientHeight + 4;
    }
    place();
  }
  /** The item being read, outlined where it's drawn (its `rect`, CSS px from the chart's corner). */
  function place() {
    const it = items[at];
    const v = slot.querySelector("datars-view");
    if (tab !== "sr" || !it || !v || !(it.rect?.[2] > 0 && it.rect?.[3] > 0)) return void (ring.hidden = true);
    const a = v.getBoundingClientRect(), b = viewBox.getBoundingClientRect();
    const [x, y, w, h] = it.rect;
    Object.assign(ring.style, { left: `${a.left - b.left + x}px`, top: `${a.top - b.top + y}px`, width: `${w}px`, height: `${h}px` });
    ring.hidden = false;
  }
  for (const b of section.querySelectorAll("[data-walk]")) {
    b.addEventListener("click", () => {
      if (!items.length) return;
      read(Math.max(0, Math.min(items.length - 1, at + Number(b.dataset.walk))));
    });
  }
  new ResizeObserver(() => place()).observe(viewBox);

  // ---- what the live region said ----
  let lastSaid = "";
  function announce(n) {
    const text = [n?.title, n?.text].filter(Boolean).join(". ");
    if (!text || text === lastSaid) return;
    lastSaid = text;
    said.querySelector(".al-quiet")?.remove();
    const li = document.createElement("li");
    li.textContent = n?.text ?? text; // the live region holds the narration's text
    said.prepend(li);
    while (said.children.length > 3) said.lastElementChild.remove();
  }

  // ---- keys ----
  function key(k, did) {
    keylog.querySelector(".al-quiet")?.remove();
    const li = document.createElement("li");
    const kbd = document.createElement("kbd");
    kbd.textContent = k;
    const t = document.createElement("span");
    t.textContent = did;
    li.append(kbd, t);
    keylog.prepend(li);
    while (keylog.children.length > 6) keylog.lastElementChild.remove();
  }
  let pendingKey = null;
  const KEYNAMES = { " ": "Space", ArrowRight: "→", ArrowLeft: "←", Enter: "Enter", Escape: "Esc", Tab: "Tab" };

  // ---- vision ----
  for (const b of section.querySelectorAll(".al-cvd button")) {
    b.addEventListener("click", () => {
      for (const o of section.querySelectorAll(".al-cvd button")) o.setAttribute("aria-pressed", String(o === b));
      slot.style.filter = b.dataset.cvd ? `url(#al-${b.dataset.cvd})` : "";
    });
  }
  const hc = section.querySelector("input[data-hc]");
  hc.addEventListener("change", () => {
    if (!view) return;
    // Back to the page's own light or dark when it's off (the slot says which).
    view.setAttribute("mode", hc.checked ? "high-contrast" : document.documentElement.dataset.theme === "dark" ? "dark" : "light");
  });

  // ---- text size ----
  const ts = section.querySelector("input[data-ts]");
  const tsOut = section.querySelector(".al-ts-out");
  const tsCode = section.querySelector("#al-ts-code");
  const yours = section.querySelector(".al-ts-yours");
  function textCode() {
    const v = Number(ts.value);
    tsOut.textContent = `${v}×`;
    tsCode.textContent = `<datars-view src="c/riksdag" text-scale="${v}"></datars-view>`;
    highlight(tsCode);
  }
  ts.addEventListener("input", () => {
    textCode();
    if (view) view.textScale = Number(ts.value);
    soonDescribe(1200);
  });
  textCode();

  // ---- forced colours, as the element paints them (simulated with the public setTokens) ----
  const fc = section.querySelector("input[data-fc]");
  fc.addEventListener("change", () => {
    if (!view) return;
    const dark = document.documentElement.dataset.theme === "dark";
    const [ink, paper, accent] = dark ? ["#ffffff", "#000000", "#1aebff"] : ["#000000", "#ffffff", "#0066cc"];
    view.setTokens(fc.checked ? { ink, paper, "ink-2": ink, muted: ink, rule: ink, grid: `mix(${ink}, ${paper}, 0.7)`, surface: paper, "surface-ink": ink, accent, highlight: accent } : {});
  });

  // ---- motion ----
  const rm = section.querySelector("input[data-rm]");
  const rmCode = section.querySelector("#al-rm-code");
  section.querySelector(".al-rm-system").hidden = !reducedQuery.matches;
  rm.checked = reducedQuery.matches;
  function motionCode() {
    const attr = rm.checked === reducedQuery.matches ? "" : rm.checked ? ' reduced-motion="reduce"' : ' reduced-motion="no-preference"';
    rmCode.textContent = `<datars-view src="c/riksdag"${attr}></datars-view>${attr ? "" : "  <!-- follows the reader's setting -->"}`;
    highlight(rmCode);
  }
  rm.addEventListener("change", () => {
    if (view) {
      if (rm.checked === reducedQuery.matches) view.removeAttribute("reduced-motion");
      else view.setAttribute("reduced-motion", rm.checked ? "reduce" : "no-preference");
    }
    motionCode();
  });
  motionCode();
  let playing = 0;
  section.querySelector("[data-play]").addEventListener("click", () => {
    if (!view || playing) return;
    const states = view.status?.states ?? [];
    let i = 0;
    const go = () => {
      view.send(`goto:${states[i]}`);
      i++;
      playing = i < states.length ? setTimeout(go, 1800) : 0;
    };
    go();
  });

  // ---- the chart ----
  let t = 0;
  function soonDescribe(ms = 350) { clearTimeout(t); t = setTimeout(describe, ms); }
  whenReady(slot).then((v) => {
    view = v;
    yours.textContent = `${v.textScale}×`;
    if (rm.checked !== reducedQuery.matches) v.setAttribute("reduced-motion", rm.checked ? "reduce" : "no-preference");
    // Read the order a moment after a change settles.
    const soon = soonDescribe;
    v.addEventListener("state", (e) => {
      announce(e.detail?.narration);
      if (pendingKey) key(pendingKey, `step: ${e.detail?.state ?? ""}`);
      pendingKey = null;
      soon(900);
    });
    v.addEventListener("signal", (e) => {
      if (!e.detail.changed.includes("coalition")) return soon(500);
      const c = e.detail.signals.coalition ?? [];
      if (pendingKey) key(pendingKey, `coalition: ${c.length ? c.map((k) => NAMES[k] ?? k).join(", ") : "nobody"}`);
      pendingKey = null;
      soon(500);
    });
    v.addEventListener("keydown", (e) => {
      const k = KEYNAMES[e.key] ?? (e.key.length === 1 ? e.key.toUpperCase() : e.key);
      if (e.key === "Tab") return void setTimeout(() => key(e.shiftKey ? "Shift+Tab" : "Tab", "moves focus"), 0);
      pendingKey = k;
      // A key that did nothing the page can hear still shows.
      setTimeout(() => { if (pendingKey === k) { key(k, "nothing here"); pendingKey = null; } }, 400);
    });
    v.addEventListener("pointerup", () => soon(500));
    new MutationObserver(() => describe()).observe(v, { attributes: true, attributeFilter: ["aria-label", "mode", "text-scale"] });
    const n = v.status?.narration;
    if (n) announce(n);
    soon(600);
  });
}

const el = document.getElementById("lab");
if (el) lab(el);
