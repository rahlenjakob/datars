// The animation page (site/pages/features/animation.html): the motion playground and the
// reduced-motion comparison.
//
// The playground builds a motion rule from its controls, puts it in the chart's own document (the
// build publishes it at /play/<alias>.json) and hands that to the running view with `setDocument`:
// the engine replans, and the page plays the transition. Every frame is the engine's plan — the
// page draws nothing itself. The document it sends has two states, the layout on screen and the one
// picked, so a transition between any two layouts is the step from the first to the second.
//
// Two engine calls aren't on <datars-view>'s public API, so this reaches the element's running
// engine (`view.view`) for them: `seek(pos)` — what its `scrub` attribute calls, a frame of the plan
// at any t — and `set_reduced_motion(on)`, which the element only ever sets from the reader's media
// query. A public `seek()` and a `reduced-motion` attribute on the element would replace both.
import { highlight } from "./site.js";

const reducedQuery = matchMedia("(prefers-reduced-motion: reduce)");

/** The slot's <datars-view> once its engine is running. */
function whenReady(slot) {
  return new Promise((resolve) => {
    const look = () => {
      const v = slot.querySelector("datars-view");
      if (!v) return false;
      if (v.dataset.renderer && v.view) resolve(v);
      else v.addEventListener("state", () => resolve(v), { once: true });
      return true;
    };
    if (look()) return;
    const mo = new MutationObserver(() => { if (look()) mo.disconnect(); });
    mo.observe(slot, { childList: true });
  });
}

/** Show the plan of the step from state `floor(pos)` at fraction `pos % 1` — exactly, both ways. */
function seek(v, pos) {
  v.view?.seek(pos);
  v.kick?.(); // draw it: the element renders on demand
}

/** Reduced motion for one view: `true` or `false` whatever the reader's setting, `null` to follow it
 * again. Kept across the element's own updates (it re-applies the media query when the chart comes
 * into view), which call the engine's setter. */
function setReduced(v, on) {
  const engine = v.view;
  if (!engine) return;
  const set = Object.getPrototypeOf(engine).set_reduced_motion;
  if (on === null) {
    delete engine.set_reduced_motion;
    set.call(engine, reducedQuery.matches);
  } else {
    engine.set_reduced_motion = () => set.call(engine, on);
    set.call(engine, on);
  }
}

// ---- the playground ---------------------------------------------------------------------------

const DEFAULTS = {
  duration: 1.2, easing: "cubic-in-out", stiffness: 170, damping: 14, x1: 0.2, y1: 0.9, x2: 0.1, y2: 1, steps: 5,
  choreo: "together", order: "value", spread: 0.5, angle: 0, exit: 0.3, update: 0.4, enter: 0.3,
  route: "straight", arc: 0.45, turns: 1, hop: 30, drift: 40, bounce: 0.35, morph: "disc", ghost: "fade",
};
const PRESETS = {
  calm: {},
  springy: { duration: 1.4, easing: "spring", stiffness: 180, damping: 11, choreo: "stagger", order: "value", spread: 0.4, route: "arc", arc: 0.35, ghost: "pop" },
  cinematic: { duration: 2.4, easing: "bezier", x1: 0.7, y1: 0, x2: 0.2, y2: 1, choreo: "ripple", spread: 0.6, route: "spiral", turns: 0.5, ghost: "grow" },
  playful: { duration: 1.6, easing: "back-out", choreo: "wave", spread: 0.6, angle: 0, route: "hop", hop: 45, ghost: "slide" },
  mechanical: { duration: 1.8, easing: "steps", steps: 6, choreo: "stagger", order: "left", spread: 0.6, route: "elbow", morph: "crossfade" },
};
/** What the engine plays instead for a reader who asked for less motion (datars-engine). */
const CALM = '{ duration: 0.25, matcher: "none", easing: "linear" }';

const n = (x, d = 2) => Number(Number(x).toFixed(d));
const OUT = {
  duration: (v) => `${n(v, 1)} s`, stiffness: String, damping: String, steps: String,
  x1: (v) => n(v).toFixed(2), y1: (v) => n(v).toFixed(2), x2: (v) => n(v).toFixed(2), y2: (v) => n(v).toFixed(2),
  spread: (v) => n(v).toFixed(2), angle: (v) => `${v}°`, exit: (v) => n(v).toFixed(2), update: (v) => n(v).toFixed(2), enter: (v) => n(v).toFixed(2),
  arc: (v) => n(v).toFixed(2), turns: String, hop: (v) => `${v} px`, drift: (v) => `${v} px`, bounce: (v) => n(v).toFixed(2),
};

/** The rule's parts, each as [the document's JSON, the TypeScript that writes it] (as the SDK's
 * helpers in packages/sdk/src/motion.ts write them). */
function parts(S) {
  const easing = S.easing === "spring" ? `spring(${S.stiffness}, ${S.damping})`
    : S.easing === "bezier" ? `cubic-bezier(${n(S.x1)}, ${n(S.y1)}, ${n(S.x2)}, ${n(S.y2)})`
    : S.easing === "steps" ? `steps(${S.steps})` : S.easing;
  const rad = n((S.angle * Math.PI) / 180);
  const order = S.order === "random" ? { random: 7 } : S.order;
  const choreo = {
    together: null,
    stagger: [{ type: "stagger", order, spread: n(S.spread) }, `choreo.stagger(${S.order === "random" ? "{ random: 7 }" : JSON.stringify(S.order)}, ${n(S.spread)})`],
    wave: [{ type: "wave", spread: n(S.spread), angle: rad }, `choreo.wave(${n(S.spread)}, ${rad})`],
    ripple: [{ type: "ripple", spread: n(S.spread) }, `choreo.ripple(${n(S.spread)})`],
    phased: [{ type: "phased", exit: n(S.exit), update: n(S.update), enter: n(S.enter) }, `choreo.phased(${n(S.exit)}, ${n(S.update)}, ${n(S.enter)})`],
  }[S.choreo];
  const route = {
    straight: null,
    arc: [{ type: "arc", height: n(S.arc) }, `route.arc(${n(S.arc)})`],
    elbow: [{ type: "elbow" }, "route.elbow()"],
    spiral: [{ type: "spiral", turns: n(S.turns) }, `route.spiral(${n(S.turns)})`],
    explode: [{ type: "explode" }, "route.explode()"],
    hop: [{ type: "hop", height: Number(S.hop) }, `route.hop(${S.hop})`],
    drift: [{ type: "drift", seed: 1, amount: Number(S.drift) }, `route.drift(1, ${S.drift})`],
    drop: [{ type: "drop", bounce: n(S.bounce) }, `route.drop(${n(S.bounce)})`],
  }[S.route];
  const ghost = {
    fade: null,
    grow: [{ scale: 0, origin: "bottom" }, 'ghost.grow("bottom")'],
    pop: [{ opacity: 0, scale: 0.4, origin: "center" }, '{ opacity: 0, scale: 0.4, origin: "center" }'],
    slide: [{ opacity: 0, dx: 0, dy: 24 }, "ghost.slide(0, 24)"],
  }[S.ghost];
  const morph = S.morph === "disc" ? null : [S.morph, JSON.stringify(S.morph)];
  return { easing, choreo, route, ghost, morph };
}

/** The document's motion rules for the controls' settings — durations × `slow` for slow motion
 * (a plan is normalized in time, so the frames are the same, just further apart) — and the
 * TypeScript for them. `labels` is the figure's own rule for its labels, kept as it is. */
function rules(S, labels, slow = 1) {
  const p = parts(S);
  const datum = { select: { role: "datum" } };
  const ts = [];
  if (p.choreo) { datum.choreo = p.choreo[0]; ts.push(`choreo: ${p.choreo[1]}`); }
  if (p.route) { datum.route = p.route[0]; ts.push(`route: ${p.route[1]}`); }
  if (p.morph) { datum.morph = p.morph[0]; ts.push(`morph: ${p.morph[1]}`); }
  if (p.ghost) { datum.enter = p.ghost[0]; datum.exit = p.ghost[0]; ts.push(`enter: ${p.ghost[1]}`, `exit: ${p.ghost[1]}`); }
  const ir = [{ duration: n(S.duration * slow, 2), easing: p.easing }, ...(ts.length ? [datum] : []), ...(labels ? [labels] : [])];
  const code = [
    "motion(",
    `  { duration: ${n(S.duration, 1)}, easing: ${JSON.stringify(p.easing)} },`,
    ...(ts.length ? ["  {", '    select: { role: "datum" },', ...ts.map((t) => `    ${t},`), "  },"] : []),
    "  // the figure's labels: out first, in last",
    '  { select: { kind: "text", key: "root/chart" },',
    "    choreo: choreo.phased(0.3, 0.3, 0.4) },",
    ")",
  ].join("\n");
  return { ir, code };
}

function playground(section) {
  const slot = section.querySelector('.chart[data-chart="motion-playground"]');
  const pills = [...section.querySelectorAll(".mp-layouts button[data-state]")];
  const order = pills.map((p) => p.dataset.state);
  const replayBtn = section.querySelector(".mp-replay");
  const scrubber = section.querySelector(".mp-scrub");
  const what = section.querySelector(".mp-what");
  const fps = section.querySelector(".mp-fps");
  const codeEl = section.querySelector("#mp-code");
  const reducedBox = section.querySelector('input[data-mp="reduced"]');
  const reducedNote = section.querySelector(".mp-reduced-note");
  const label = (s) => pills.find((p) => p.dataset.state === s)?.textContent ?? s;

  const S = { ...DEFAULTS };
  let preset = "calm";
  let speed = 1;
  let view = null, base = null, labels = null;
  let at = order[0]; // the program state the engine is in
  let last = null; // the last transition, { from, to }: what Replay and the scrubber play
  let loaded = ""; // what the view's document was last built for
  let pending = null; // a layout picked before the chart was running
  let raf = 0;

  // The document as published, fetched beside the chart's bundle (`…/c/<alias>` → `…/play/<alias>.json`).
  const docUrl = new URL(slot.dataset.src.replace(/\/c\/([^/]+)$/, "/play/$1.json"), location.href);
  const docReady = fetch(docUrl).then((r) => r.json());

  reducedBox.checked = reducedQuery.matches;
  reducedNote.hidden = !reducedQuery.matches;

  // ---- the controls ----
  function render() {
    for (const group of section.querySelectorAll(".mp-panel [data-mp]:not(input)")) {
      const k = group.dataset.mp;
      const v = k === "preset" ? preset : S[k];
      for (const b of group.querySelectorAll("button[data-v]")) b.setAttribute("aria-pressed", String(b.dataset.v === v));
    }
    for (const input of section.querySelectorAll('.mp-panel input[type="range"][data-mp]')) {
      const k = input.dataset.mp;
      if (Number(input.value) !== Number(S[k])) input.value = String(S[k]);
      const out = section.querySelector(`output[for="${input.id}"]`);
      if (out) out.textContent = OUT[k](S[k]);
    }
    for (const [attr, k] of [["forEasing", "easing"], ["forChoreo", "choreo"], ["forRoute", "route"]]) {
      for (const el of section.querySelectorAll(`[data-${attr.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)}]`)) el.hidden = !el.dataset[attr].split(" ").includes(S[k]);
    }
    const { code } = rules(S, labels);
    codeEl.textContent = reducedBox.checked ? `// Reduced motion: the engine plays every change as\n// ${CALM} instead of:\n${code}` : code;
    highlight(codeEl);
  }

  section.querySelector(".mp-panel").addEventListener("click", (e) => {
    const b = e.target.closest("button[data-v]");
    const group = b?.closest("[data-mp]");
    if (!b || !group) return;
    if (group.dataset.mp === "preset") {
      preset = b.dataset.v;
      Object.assign(S, DEFAULTS, PRESETS[preset]);
    } else {
      S[group.dataset.mp] = b.dataset.v;
      preset = null;
    }
    render();
    again();
  });
  section.querySelector(".mp-panel").addEventListener("input", (e) => {
    const input = e.target;
    if (input.type !== "range" || !input.dataset.mp) return;
    S[input.dataset.mp] = Number(input.value);
    preset = null;
    render();
  });
  // A slider plays the transition again when it's let go (not at every pixel of the drag).
  section.querySelector(".mp-panel").addEventListener("change", (e) => {
    if (e.target.type === "range" && e.target.dataset.mp) again();
  });
  reducedBox.addEventListener("change", () => {
    if (view) setReduced(view, reducedBox.checked === reducedQuery.matches ? null : reducedBox.checked);
    loaded = ""; // the scrubber's plan is made again, the reduced way or not
    render();
    again();
  });

  // ---- the transport ----
  function documentFor(from, to) {
    const d = structuredClone(base);
    d.program.states = [from, to].map((s) => base.program.states.find((x) => x.name === s));
    d.motion = { rules: rules(S, labels, speed).ir };
    return d;
  }
  /** Hand the view the document for the step from `from` to `to` (unless it has it already). The
   * view morphs from what's on screen to the state of the same name — `from` — so a change while
   * something moves carries on from the frame on screen. */
  function load(from, to) {
    const k = `${from}>${to}>${speed}>${JSON.stringify(rules(S, labels, speed).ir)}`;
    if (k === loaded) return;
    view.setDocument(documentFor(from, to));
    loaded = k;
  }
  const seconds = () => (reducedBox.checked ? 0.25 : S.duration * speed);
  function press(state) {
    for (const p of pills) p.setAttribute("aria-pressed", String(p.dataset.state === state));
  }
  function say(text) {
    if (what.textContent !== text) what.textContent = text;
  }
  /** Follow a playing transition on the scrubber (by the clock: the engine plays it) and measure
   * the page's frame rate while it plays. */
  function follow() {
    cancelAnimationFrame(raf);
    const ms = seconds() * 1000;
    const t0 = performance.now();
    let prev = 0;
    const gaps = [];
    fps.textContent = "";
    const tick = (now) => {
      const f = Math.min(1, (now - t0) / ms);
      scrubber.value = String(Math.round(f * 1000));
      if (prev) gaps.push(now - prev);
      prev = now;
      if (f < 1) raf = requestAnimationFrame(tick);
      else {
        raf = 0;
        if (gaps.length >= 12) {
          const sorted = gaps.sort((a, b) => a - b);
          fps.textContent = `${Math.min(120, Math.round(1000 / sorted[sorted.length >> 1]))} fps`;
        }
      }
    };
    raf = requestAnimationFrame(tick);
  }
  function play(to) {
    if (!view) return void (pending = to);
    if (to === at && !raf) {
      // Back where a scrub started: morph there from the frame on screen.
      if (last && Number(scrubber.value) > 0 && Number(scrubber.value) < 1000) {
        loaded = "";
        load(at, at === last.to ? last.from : last.to);
        scrubber.value = "0";
      }
      return;
    }
    const from = at;
    if (from === to) return;
    load(from, to);
    last = { from, to };
    at = to;
    view.send("next");
    press(to);
    say(`${label(from)} → ${label(to)} · ${n(seconds(), 2)} s${speed > 1 ? " (slow motion)" : ""}`);
    follow();
  }
  function replay() {
    if (!view) return;
    if (!last) return play(order[(order.indexOf(at) + 1) % order.length]);
    load(last.from, last.to);
    seek(view, 0);
    at = last.to;
    view.send("next");
    press(last.to);
    say(`${label(last.from)} → ${label(last.to)} · ${n(seconds(), 2)} s${speed > 1 ? " (slow motion)" : ""}`);
    follow();
  }
  /** A setting changed: show it — the last transition again, or the first one. */
  function again() {
    if (view) replay();
  }

  for (const p of pills) p.addEventListener("click", () => play(p.dataset.state));
  replayBtn.addEventListener("click", replay);
  scrubber.addEventListener("input", () => {
    if (!view) return;
    cancelAnimationFrame(raf);
    raf = 0;
    fps.textContent = "";
    if (!last) last = { from: at, to: order[(order.indexOf(at) + 1) % order.length] };
    load(last.from, last.to);
    const f = Number(scrubber.value) / 1000;
    seek(view, f);
    at = f >= 1 ? last.to : last.from;
    press(f >= 0.5 ? last.to : last.from);
    say(`${label(last.from)} → ${label(last.to)} · t = ${f.toFixed(2)}`);
  });
  for (const b of section.querySelectorAll(".mp-speed button")) {
    b.addEventListener("click", () => {
      speed = Number(b.dataset.speed);
      for (const o of section.querySelectorAll(".mp-speed button")) o.setAttribute("aria-pressed", String(o === b));
      again();
    });
  }

  render();
  Promise.all([whenReady(slot), docReady]).then(([v, doc]) => {
    view = v;
    base = doc;
    labels = (doc.motion?.rules ?? []).find((r) => r.select?.kind === "text") ?? null;
    if (reducedBox.checked !== reducedQuery.matches) setReduced(view, reducedBox.checked);
    render(); // now with the figure's own labels rule
    // (The reader's look and the page's mode carry over to each new document: setDocument keeps
    // the view's tokens and mode.)
    if (pending) play(pending);
  });
}

// ---- reduced motion, side by side ---------------------------------------------------------------

function comparison(section) {
  const pills = [...section.querySelectorAll(".mp-rm-pills button[data-state]")];
  const order = pills.map((p) => p.dataset.state);
  const full = section.querySelector('.chart[data-rm="full"]');
  const calm = section.querySelector('.chart[data-rm="reduced"]');
  const note = section.querySelector(".mp-rm-note");
  const views = [];
  let current = order[0], auto = true, visible = false, forced = false;
  note.hidden = !reducedQuery.matches;

  const go = (state) => {
    current = state;
    for (const p of pills) p.setAttribute("aria-pressed", String(p.dataset.state === state));
    for (const v of views) v.send(`goto:${state}`);
  };
  for (const p of pills) p.addEventListener("click", () => { auto = false; go(p.dataset.state); });
  section.querySelector(".mp-rm-force")?.addEventListener("click", (e) => {
    forced = true;
    e.target.closest(".mp-rm-note").hidden = true;
    if (views[0]) setReduced(views[0], false);
  });
  whenReady(full).then((v) => { views.unshift(v); if (forced) setReduced(v, false); if (current !== order[0]) v.send(`goto:${current}`); });
  whenReady(calm).then((v) => { views.push(v); setReduced(v, true); if (current !== order[0]) v.send(`goto:${current}`); });
  // Both step on together while the pair is on screen, until the reader picks a step (and never
  // for a reader who asked for less motion, unless they asked to see it).
  new IntersectionObserver(([e]) => { visible = e.isIntersecting; }, { threshold: 0.5 }).observe(section.querySelector(".mp-rm"));
  setInterval(() => {
    if (auto && visible && !document.hidden && views.length === 2 && (!reducedQuery.matches || forced)) go(order[(order.indexOf(current) + 1) % order.length]);
  }, 3600);
}

const pg = document.getElementById("playground");
if (pg) playground(pg);
const rm = document.getElementById("reduced-motion");
if (rm) comparison(rm);
