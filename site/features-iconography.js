// The iconography page (/features/iconography/): published charts driven through the public
// `<datars-view>` API — `setSignal()` for the pictogram's shape and set, the dial's value and
// looks; `send("goto:…")` for the forecast's icon set and the day ring; `setTokens()` and `mode`
// for the brands and the logos' inks; the `text-scale` attribute for the reader's text size. The
// line under each control says the call the page just made.

const $ = (sel, el = document) => el.querySelector(sel);
const $$ = (sel, el = document) => [...el.querySelectorAll(sel)];
const press = (buttons, on) => buttons.forEach((b) => b.setAttribute("aria-pressed", String(b === on)));
const say = (id, text) => { const el = document.getElementById(id); if (el) el.textContent = text; };

/** The slot's <datars-view> once its engine runs (the `chartmount` event, or already mounted). */
function view(alias) {
  const slot = $(`.chart[data-chart="${alias}"]`);
  if (!slot) return new Promise(() => {});
  return new Promise((resolve) => {
    const ready = (v) => (v.status ? resolve(v) : v.addEventListener("state", () => resolve(v), { once: true }));
    const v = $("datars-view", slot);
    if (v) return ready(v);
    slot.addEventListener("chartmount", (e) => ready(e.detail.view), { once: true });
  });
}

// The page steps its charts with its own controls: the element's arrows and card stay hidden.
document.addEventListener("chartmount", (e) => e.detail.view.setAttribute("no-controls", ""));

// The sections whose charts this page styles (the reader's own look leaves them alone): mount now.
for (const el of $$("[data-own-look]")) el.dataset.ownLook = "ready";
document.dispatchEvent(new Event("ownlookready"));

// ---- the pictogram lab -------------------------------------------------------------------------

const LAB_INKS = {
  theme: null,
  sea: ["#2a6f97", "#2c9c94", "#56b38a", "#8cc084", "#61a5c2"],
  mono: ["$ink"],
};
view("iconography-lab").then((v) => {
  const shapes = $$("[data-lab-shape]"), sets = $$("[data-lab-set]"), inks = $$("[data-lab-inks]");
  shapes.forEach((b) => b.addEventListener("click", () => { press(shapes, b); v.setSignal("shape", b.dataset.labShape); say("lab-code", `view.setSignal("shape", "${b.dataset.labShape}");`); }));
  sets.forEach((b) => b.addEventListener("click", () => { press(sets, b); v.setSignal("set", b.dataset.labSet); say("lab-code", `view.setSignal("set", "${b.dataset.labSet}");`); }));
  inks.forEach((b) => b.addEventListener("click", () => {
    press(inks, b);
    const c = LAB_INKS[b.dataset.labInks];
    v.setTokens(c ? { categorical: c } : {});
    say("lab-code", c ? `view.setTokens({ categorical: ${JSON.stringify(c)} });` : "view.setTokens({}); // the theme's own palette");
  }));
});

// ---- your icons, our inks: the forecast in brands and modes ------------------------------------

/** Brands for the forecast: paper, ink and accent (the built-in theme derives its greys and
 * surfaces from them) and the sun's `highlight`; `mode` for a dark-first one. Fictional brands. */
const BRANDS = {
  light: { name: "datars, light", mode: "light", paper: "#ffffff", tokens: {} },
  dark: { name: "datars, dark", mode: "dark", paper: "#111318", tokens: {} },
  larkspur: { name: "Larkspur Health", mode: "light", paper: "#f1f6f5", tokens: { paper: "#f1f6f5", ink: "#12343a", accent: "#0f766e", highlight: "#c9861f" } },
  quill: { name: "The Daily Quill", mode: "light", paper: "#f4eee2", tokens: { paper: "#f4eee2", ink: "#1b1916", accent: "#1d3f6e", highlight: "#b3261e" } },
  neonfall: { name: "Neonfall", mode: "dark", paper: "#0b0620", tokens: { paper: "#0b0620", ink: "#f3ecff", accent: "#22e5ff", highlight: "#ffd23f" } },
};
view("iconography-forecast").then((v) => {
  const sets = $$("[data-fc-set]"), brands = $$("[data-fc-brand]");
  const frame = document.getElementById("fc-frame");
  let set = "lucide", brand = "light";
  const code = () => {
    const b = BRANDS[brand];
    const t = JSON.stringify(b.tokens).replace(/,"/g, ', "').replace(/":/g, '": ').replace(/^\{"/, '{ "').replace(/\}$/, " }").replace("{ }", "{}");
    say("fc-code", `// ${b.name}${set === "ours" ? ", your SVG files" : ""}\nview.setAttribute("mode", "${b.mode}");\nview.setTokens(${Object.keys(b.tokens).length ? t : "{}"});`);
  };
  const apply = (btn) => {
    press(brands, btn);
    brand = btn.dataset.fcBrand;
    const b = BRANDS[brand];
    v.setAttribute("mode", b.mode);
    v.setTokens(b.tokens);
    // The frame wears the chart's paper, so the chart reads as one surface.
    frame?.style.setProperty("--ic-paper", b.paper);
    code();
  };
  sets.forEach((b) => b.addEventListener("click", () => { press(sets, b); set = b.dataset.fcSet; v.send(`goto:${set}`); code(); }));
  brands.forEach((btn) => btn.addEventListener("click", () => apply(btn)));
  // It opens in the page's own mode.
  apply(brands.find((b) => b.dataset.fcBrand === (document.documentElement.dataset.theme === "dark" ? "dark" : "light")) ?? brands[0]);
});

// ---- bring an SVG: the parties' symbols in their colours, a house style, one ink ----------------

const PARTIES = ["labour", "greens", "first", "centre"];
const LOOKS = {
  own: () => ({}),
  house: () => Object.fromEntries(PARTIES.flatMap((p, i) => [[`party.${p}`, `$categorical[${i}]`], [`party.${p}.mark`, "$paper"], [`party.${p}.detail`, "$paper@0.55"]])),
  print: () => Object.fromEntries(PARTIES.flatMap((p) => [[`party.${p}`, "$ink"], [`party.${p}.mark`, "$paper"], [`party.${p}.detail`, "$ink-2"]])),
};
view("iconography-parties").then((v) => {
  const looks = $$("[data-party-look]");
  looks.forEach((b) => b.addEventListener("click", () => { press(looks, b); v.setTokens(LOOKS[b.dataset.partyLook]()); }));
});

// ---- the dial ----------------------------------------------------------------------------------

view("iconography-dial").then((v) => {
  const range = document.getElementById("dial-mood"), out = document.getElementById("dial-out");
  range?.addEventListener("input", () => {
    if (out) out.textContent = range.value;
    v.setSignal("mood", Number(range.value));
    say("dial-code", `view.setSignal("mood", ${range.value});`);
  });
  for (const k of ["needle", "names"]) {
    const buttons = $$(`[data-dial-${k}]`);
    buttons.forEach((b) => b.addEventListener("click", () => {
      press(buttons, b);
      const value = b.getAttribute(`data-dial-${k}`);
      v.setSignal(k, value);
      say("dial-code", `view.setSignal("${k}", "${value}");`);
    }));
  }
});

// ---- curved text: the day ring -----------------------------------------------------------------

view("iconography-day").then((v) => {
  const days = $$("[data-day]");
  days.forEach((b) => b.addEventListener("click", () => { press(days, b); v.send(`goto:${b.dataset.day}`); }));
});

// ---- accessible: the reader's text size --------------------------------------------------------

view("iconography-a11y").then((v) => {
  const scales = $$("[data-scale]");
  scales.forEach((b) => b.addEventListener("click", () => { press(scales, b); v.setAttribute("text-scale", b.dataset.scale); }));
});

// ---- pictures: the planets ---------------------------------------------------------------------

const PLANET_NOTES = {
  size: "Jupiter is 29 times as wide as Mercury; Earth is a dot beside it.",
  distance: "On a log scale: Neptune is 78 times as far from the Sun as Mercury.",
  moons: "Saturn has 274 known moons; Mercury and Venus have none.",
};
view("iconography-planets").then((v) => {
  const steps = $$("[data-planets]");
  steps.forEach((b) => b.addEventListener("click", () => {
    press(steps, b);
    v.send(`goto:${b.dataset.planets}`);
    say("planets-note", PLANET_NOTES[b.dataset.planets]);
  }));
});
