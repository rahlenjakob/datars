// The accessibility page (site/pages/features/accessibility.html): the lab — one chart, and what
// each kind of reader gets from it.
//
// Everything through <datars-view>'s public API, nothing from inside it:
// - Screen reader: `status.semantics` — the tree the element's hidden list is built from (each
//   item's role, label, depth, box and whether a click acts on it) — walked item by item with its
//   box outlined over the chart; the `state` event's narration is what the live region speaks.
// - Keyboard: `keydown` on the element, and the `signal` and `state` events for what a key did.
// - Vision: an SVG colour matrix over the drawn chart (the engine's own CVD matrices), and the
//   `mode` attribute for high contrast.
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

/** The items a screen reader walks, by the rules the element's list follows (packages/web): no
 * unnamed groups, no drawn control that isn't clickable (its native input stands for it), at most
 * 500 marks. */
function readable(semantics) {
  return (semantics ?? []).filter((i) => !(i.role === "control" && !i.actionable) && (i.actionable || i.label)).slice(0, 500);
}

function lab(section) {
  const slot = section.querySelector('.chart[data-chart="accessibility-lab"]');
  const viewBox = section.querySelector(".al-view");
  const ring = section.querySelector(".al-reading");
  const tree = section.querySelector(".al-tree");
  const pos = section.querySelector(".al-pos");
  const said = section.querySelector(".al-said");
  const figname = section.querySelector(".al-figname");
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
  function describe() {
    if (!view) return;
    const s = view.status;
    if (!s) return;
    const next = readable(s.semantics);
    const key = (list) => list.map((i) => `${i.role}|${i.label}|${i.actionable}`).join("\n");
    figname.textContent = view.getAttribute("aria-label") || next[0]?.label || "Chart";
    if (key(next) === key(items)) {
      items = next; // the boxes may have moved
      return place();
    }
    const label = at >= 0 ? items[at]?.label : null;
    items = next;
    tree.replaceChildren(...items.map((it, i) => {
      const li = document.createElement("li");
      li.style.setProperty("--depth", String(it.depth));
      const b = document.createElement("button");
      b.type = "button";
      b.className = "al-item";
      b.dataset.i = String(i);
      const role = document.createElement("span");
      role.className = `al-role${it.actionable ? " al-act" : ""}`;
      role.textContent = it.actionable ? "button" : it.role;
      const text = document.createElement("span");
      text.className = "al-label";
      text.textContent = it.label;
      b.append(role, text);
      b.addEventListener("click", () => read(i));
      li.appendChild(b);
      return li;
    }));
    // Keep reading where we were, if that item's still there (a label that follows the state).
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
  whenReady(slot).then((v) => {
    view = v;
    if (rm.checked !== reducedQuery.matches) v.setAttribute("reduced-motion", rm.checked ? "reduce" : "no-preference");
    // The element fills its list in idle time once a change settles: read it a moment after.
    let t = 0;
    const soon = (ms = 350) => { clearTimeout(t); t = setTimeout(describe, ms); };
    v.addEventListener("state", (e) => {
      announce(e.detail?.narration);
      if (pendingKey) key(pendingKey, `step: ${e.detail?.state ?? ""}`);
      pendingKey = null;
      soon(900);
    });
    v.addEventListener("signal", (e) => {
      if (!e.detail.changed.includes("coalition")) return;
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
    new MutationObserver(() => describe()).observe(v, { attributes: true, attributeFilter: ["aria-label", "mode"] });
    const n = v.status?.narration;
    if (n) announce(n);
    soon(600);
  });
}

const el = document.getElementById("lab");
if (el) lab(el);
