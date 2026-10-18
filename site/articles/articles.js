// The article pages: every chart is a <datars-view> mounted into its slot (`.chart[data-src]`, filled
// in by scripts/build-articles.mjs) when it comes near the screen and taken off again when it's far
// away — the charts on a page share one engine memory, and a long article shouldn't hold every map
// at once. A slot keeps its size either way, so the page never jumps. Scroll stories hand their
// steps to the view (`steps`); stepped figures get a row of step buttons.
import "../runtime/datars.js";

const dark = getComputedStyle(document.documentElement).colorScheme === "dark";
const phone = () => innerWidth < 700;

/** A figure's height for its width: the document's aspect, but never a sliver on a phone (charts
 * lay themselves out again at the size they're given) and never taller than the screen. */
function heightFor(slot, width) {
  const aspect = Number(slot.dataset.aspect) || 0.6;
  const vh = innerHeight;
  let h = width * aspect;
  // (articles.css reserves the same box for phones: keep the two in step.)
  if (phone()) h = Math.max(h, Math.min(width * (Number(getComputedStyle(slot).getPropertyValue("--phone-aspect")) || 1.05), vh * 0.72));
  return Math.round(Math.max(220, Math.min(h, vh * 0.86)));
}

function size(slot, view) {
  if (slot.closest(".scrolly-stage")) {
    view.setAttribute("height", String(Math.round(slot.clientHeight)));
    return;
  }
  const h = heightFor(slot, slot.clientWidth);
  slot.style.height = `${h}px`;
  slot.style.aspectRatio = "auto";
  if (view.getAttribute("height") !== String(h)) view.setAttribute("height", String(h));
}

function mount(slot) {
  if (slot.querySelector("datars-view")) return;
  const view = document.createElement("datars-view");
  view.setAttribute("src", slot.dataset.src);
  view.setAttribute("mode", dark ? "dark" : "light");
  if (slot.dataset.steps) view.setAttribute("steps", slot.dataset.steps);
  view.setAttribute("aria-label", slot.dataset.label ?? "Chart");
  slot.classList.add("pending");
  size(slot, view);
  slot.appendChild(view);
  const ready = () => {
    if (!view.isConnected) return;
    if (view.dataset.renderer) {
      slot.classList.remove("pending");
      dropPicture(slot); // drawn again: the picture kept for printing goes
    } else setTimeout(ready, 150);
  };
  ready();
  const stepper = slot.closest("figure")?.querySelector(".stepper");
  if (stepper) wireStepper(stepper, view);
}

function unmount(slot) {
  const view = slot.querySelector("datars-view");
  if (!view || printing) return; // (printing, every chart stays)
  // A picture of it for printing, which can't wait for it to load again: one PNG, shown only in
  // print (articles.css), gone when it mounts again.
  if (view.dataset.renderer && view.printPicture) {
    void view.printPicture().then((blob) => {
      if (!blob || slot.querySelector("datars-view")) return;
      dropPicture(slot);
      const img = document.createElement("img");
      img.className = "print-picture";
      img.alt = "";
      img.src = URL.createObjectURL(blob);
      slot.appendChild(img);
    });
  }
  view.remove(); // the runtime frees the engine
  slot.classList.remove("pending");
  // Its step buttons go with it (they'd drive a freed engine); a new view brings its own.
  slot.closest("figure")?.querySelector(".stepper")?.replaceChildren();
}

function dropPicture(slot) {
  slot.querySelectorAll("img.print-picture").forEach((img) => { URL.revokeObjectURL(img.src); img.remove(); });
}

/** Numbered step buttons and "next" under a figure whose story has steps. */
function wireStepper(bar, view) {
  bar.replaceChildren();
  view.addEventListener("state", (e) => {
    const s = e.detail ?? {};
    const states = s.states ?? [];
    if (bar.childElementCount !== states.length + 1) {
      bar.replaceChildren(...states.map((name, i) => {
        const b = document.createElement("button");
        b.type = "button";
        b.textContent = String(i + 1);
        b.setAttribute("aria-label", `Step ${i + 1} of ${states.length}`);
        b.addEventListener("click", () => view.send(`goto:${name}`));
        return b;
      }));
      const next = document.createElement("button");
      next.type = "button";
      next.className = "next";
      bar.appendChild(next);
    }
    const buttons = [...bar.querySelectorAll("button:not(.next)")];
    buttons.forEach((b, i) => b.setAttribute("aria-pressed", String(i === s.index)));
    // One handler, replaced on every step (a second listener would step twice).
    const last = s.index >= states.length - 1;
    const next = bar.querySelector(".next");
    next.textContent = last ? "Again" : "Next";
    next.onclick = () => view.send(last ? `goto:${states[0]}` : "next");
  });
}

const slots = [...document.querySelectorAll(".chart[data-src]")];
// Mount a screen and a half ahead; let go four screens behind.
const near = new IntersectionObserver((es) => es.forEach((e) => e.isIntersecting && mount(e.target)), { rootMargin: "150% 0px" });
const far = new IntersectionObserver((es) => es.forEach((e) => !e.isIntersecting && unmount(e.target)), { rootMargin: "400% 0px" });
for (const s of slots) {
  near.observe(s);
  far.observe(s);
}
// Printing: every chart on the page — the ones not near yet start now (for this print if they're
// quick, else the next), and none is let go until it's over.
let printing = false;
const printStart = () => { printing = true; slots.forEach(mount); };
addEventListener("beforeprint", printStart);
addEventListener("afterprint", () => { if (!matchMedia("print").matches) printing = false; });
matchMedia("print").addEventListener?.("change", (e) => (e.matches ? printStart() : (printing = false)));
// Keep each chart sized to its column (and a scroll story's to the screen).
let resizing = 0;
addEventListener("resize", () => {
  cancelAnimationFrame(resizing);
  resizing = requestAnimationFrame(() => slots.forEach((s) => { const v = s.querySelector("datars-view"); if (v) size(s, v); }));
});

// Light the step being read.
const lit = new IntersectionObserver((es) => es.forEach((e) => e.target.classList.toggle("active", e.isIntersecting)), { rootMargin: "-45% 0px -45% 0px" });
document.querySelectorAll(".step").forEach((s) => lit.observe(s));
