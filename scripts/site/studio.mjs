// The theme studio (/themes/): its editor's static markup, written by the build so the panel takes
// its full space before any script runs (site/studio.js only fills in values), and the data the
// page starts from — the built-in theme as authored and as the engine resolves it in each mode
// (`datars theme --json`), so the first paint already shows the real values.
//
// One catalogue below names every token the editor offers, with a few words on what reads it;
// the page's controls carry `data-token` and the script is driven by them.

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { escapeAttr, escapeHtml } from "./markdown.mjs";

const NEUTRAL = "crates/datars-theme/themes/neutral.json";

/** The editor's token groups: [tab, [[heading, [[token, what reads it, options]…]]…]]. A number's
 * options are its range; colours and fonts need none. */
export const STUDIO_GROUPS = {
  colours: [
    ["Surfaces and text", [
      ["paper", "the background"],
      ["ink", "text, axis lines"],
      ["ink-2", "secondary text, value labels"],
      ["muted", "tick labels, notes"],
      ["grid", "gridlines"],
      ["rule", "axis and baseline rules"],
      ["surface", "panels, tooltips"],
      ["surface-ink", "text on a surface"],
    ]],
    ["Accent and emphasis", [
      ["accent", "the highlight colour"],
      ["accent-ink", "text on the accent"],
      ["mark", "marks with no colour scale"],
      ["highlight", "emphasis, annotations"],
      ["dim", "how far unselected marks recede", { min: 0.05, max: 0.8, step: 0.01 }],
    ]],
    ["Meaning", [
      ["positive", "gains, rises"],
      ["negative", "losses, falls"],
      ["up", "prices up (candles, volume)"],
      ["down", "prices down"],
    ]],
  ],
  type: [
    ["Faces", [
      ["font.title", "titles, big numbers"],
      ["font.strong", "legends, emphasis"],
      ["font.body", "labels, ticks, text"],
      ["font.number", "figures"],
    ]],
    ["Sizes (px)", [
      ["size.title", "titles", { min: 12, max: 30, step: 0.5 }],
      ["size.body", "body text, cards", { min: 10, max: 20, step: 0.5 }],
      ["size.label", "labels, legends", { min: 8, max: 16, step: 0.5 }],
      ["size.small", "ticks, notes", { min: 7, max: 14, step: 0.5 }],
    ]],
  ],
  shape: [
    ["Corners", [
      ["radius.bar", "bar ends", { min: 0, max: 16, step: 1 }],
      ["radius.mark", "small marks, swatches", { min: 0, max: 8, step: 0.5 }],
    ]],
    ["Lines and dots", [
      ["stroke.line", "line series", { min: 0.5, max: 6, step: 0.1 }],
      ["stroke.grid", "gridlines", { min: 0, max: 3, step: 0.25 }],
      ["stroke.rule", "axis and baselines", { min: 0, max: 3, step: 0.25 }],
      ["point.radius", "dots, line points", { min: 1, max: 10, step: 0.25 }],
      ["band.padding", "gaps between bars", { min: 0, max: 0.7, step: 0.01 }],
    ]],
  ],
  maps: [
    ["Maps", [
      ["map.water", "sea, lakes, rivers"],
      ["map.land", "land, no-data backdrop"],
      ["map.park", "parks, forest"],
      ["map.building", "buildings"],
      ["map.road", "streets"],
      ["map.road-major", "major roads"],
      ["map.border", "borders"],
      ["map.label", "place names"],
      ["map.label-halo", "the halo behind names"],
      ["map.marker", "point markers"],
      ["map.no-data", "regions without data"],
    ]],
    ["Cards", [
      ["card", "card surface"],
      ["card-ink", "card text"],
      ["card-ink-2", "card secondary text"],
      ["card-line", "card outline, rules"],
      ["radius.card", "card corners", { min: 0, max: 28, step: 1 }],
    ]],
  ],
};

/** The type the editor offers: Google Fonts families by kind, with the weights each has (a reader
 * can also name any other Google Fonts family). */
export const FONT_FAMILIES = [
  ["Sans", [["Inter", [300, 400, 500, 600, 700, 800]], ["IBM Plex Sans", [300, 400, 500, 600, 700]], ["DM Sans", [300, 400, 500, 600, 700, 800]], ["Work Sans", [300, 400, 500, 600, 700, 800]],
    ["Manrope", [300, 400, 500, 600, 700, 800]], ["Space Grotesk", [300, 400, 500, 600, 700]], ["Archivo", [300, 400, 500, 600, 700, 800]], ["Barlow Condensed", [300, 400, 500, 600, 700, 800]],
    ["Source Sans 3", [300, 400, 500, 600, 700, 800]], ["Libre Franklin", [300, 400, 500, 600, 700, 800]]]],
  ["Serif", [["Newsreader", [300, 400, 500, 600, 700, 800]], ["Source Serif 4", [300, 400, 500, 600, 700, 800]], ["Lora", [400, 500, 600, 700]], ["Fraunces", [300, 400, 500, 600, 700, 800]],
    ["Playfair Display", [400, 500, 600, 700, 800]], ["Roboto Slab", [300, 400, 500, 600, 700, 800]], ["Instrument Serif", [400]]]],
  ["Mono", [["Space Mono", [400, 700]], ["IBM Plex Mono", [300, 400, 500, 600, 700]], ["JetBrains Mono", [300, 400, 500, 600, 700, 800]]]],
];

/** Faces this site serves itself (site/fonts, fonts/, assets/fonts — copied to /fonts/ by the
 * build): previewed from there; every other face comes from Fontsource's copy of Google Fonts. */
export const LOCAL_FACES = {
  Inter: { 400: "Inter-Regular", 600: "Inter-SemiBold", 700: "Inter-Bold" },
  Newsreader: { 400: "Newsreader-Regular", 600: "Newsreader-SemiBold" },
  Manrope: { 400: "Manrope-Regular", 700: "Manrope-Bold" },
  "Space Mono": { 400: "SpaceMono-Regular", 700: "SpaceMono-Bold" },
  Fraunces: { 700: "Fraunces-Bold" },
};

/** The built-in theme as authored and as the engine resolves it in each mode. */
export function studioData(root, cli) {
  const authored = JSON.parse(readFileSync(join(root, NEUTRAL), "utf8"));
  const resolved = {};
  for (const mode of ["light", "dark", "high-contrast"]) {
    const r = JSON.parse(execFileSync(cli, ["theme", join(root, NEUTRAL), "--json", "--mode", mode]).toString());
    resolved[mode] = r.tokens.tokens;
  }
  return { authored, resolved, fonts: FONT_FAMILIES, local: LOCAL_FACES };
}

/** The data as a JSON island the page script reads (`<` escaped: it can't close the element). */
export const studioJson = (data) => `<script type="application/json" id="studio-data">${JSON.stringify(data).replace(/</g, "\\u003c")}</script>`;

const id = (t) => `tok-${t.replace(/[^a-z0-9]+/g, "-")}`;

/** A colour token: its swatch (a colour picker), name, the value as written (a colour or an
 * expression — editable) and a reset. */
function colourRow(token, what, value) {
  return `<div class="tok" data-token="${token}" data-kind="color">
  <label class="tok-sw" style="--c:${escapeAttr(value)}"><input type="color" value="${escapeAttr(value.slice(0, 7))}" aria-label="${escapeAttr(`${token}: pick a colour`)}"></label>
  <label class="tok-name" for="${id(token)}"><code>${escapeHtml(token)}</code><small>${escapeHtml(what)}</small></label>
  <input class="tok-val" id="${id(token)}" type="text" spellcheck="false" autocomplete="off" autocapitalize="off" enterkeyhint="done" aria-describedby="${id(token)}-err">
  <button class="tok-reset" type="button" aria-label="${escapeAttr(`Reset ${token}`)}" title="Back to the inherited value" disabled><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 4l8 8M12 4l-8 8"/></svg></button>
  <p class="tok-err" id="${id(token)}-err" aria-live="polite"></p>
</div>`;
}

function numberRow(token, what, { min, max, step }, value) {
  return `<div class="tok num" data-token="${token}" data-kind="number">
  <label class="tok-name" for="${id(token)}"><code>${escapeHtml(token)}</code><small>${escapeHtml(what)}</small></label>
  <output for="${id(token)}">${value}</output>
  <button class="tok-reset" type="button" aria-label="${escapeAttr(`Reset ${token}`)}" title="Back to the inherited value" disabled><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 4l8 8M12 4l-8 8"/></svg></button>
  <input type="range" id="${id(token)}" min="${min}" max="${max}" step="${step}" value="${value}">
</div>`;
}

/** A font token: a family (the list below, or any Google Fonts family) and a weight. */
function fontRow(token, what, value, families) {
  const fam = value.family?.[0] ?? value.google ?? "Inter";
  const opts = families.map(([group, names]) => `<optgroup label="${escapeAttr(group)}">${names.map(([n]) => `<option${n === fam ? " selected" : ""}>${escapeHtml(n)}</option>`).join("")}</optgroup>`).join("");
  const weights = families.flatMap(([, names]) => names).find(([n]) => n === fam)?.[1] ?? [value.weight ?? 400];
  return `<div class="tok font" data-token="${token}" data-kind="font">
  <span class="tok-name"><code>${escapeHtml(token)}</code><small>${escapeHtml(what)}</small></span>
  <button class="tok-reset" type="button" aria-label="${escapeAttr(`Reset ${token}`)}" title="Back to the inherited value" disabled><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 4l8 8M12 4l-8 8"/></svg></button>
  <div class="font-pick">
    <select class="font-family" aria-label="${escapeAttr(`${token} family`)}">${opts}<optgroup label="Google Fonts"><option value="*">Any Google Fonts family…</option></optgroup></select>
    <select class="font-weight" aria-label="${escapeAttr(`${token} weight`)}">${weights.map((w) => `<option${w === (value.weight ?? 400) ? " selected" : ""}>${w}</option>`).join("")}</select>
  </div>
  <p class="tok-err" aria-live="polite"></p>
</div>`;
}

/** The editor's markup for one tab (`{{studio:colours}}` …), its values the built-in theme's in
 * light mode — what the page shows until a reader's own theme is restored. */
export function studioTab(tab, data) {
  const light = data.resolved.light;
  return STUDIO_GROUPS[tab].map(([h, rows]) => `<h3>${escapeHtml(h)}</h3><div class="toks">${rows.map(([token, what, range]) => {
    const v = light[token];
    if (token.startsWith("font.")) return fontRow(token, what, v, data.fonts);
    if (range) return numberRow(token, what, range, v);
    return colourRow(token, what, v);
  }).join("")}</div>`).join("");
}

/** The checks the built-in theme declares (the studio's theme inherits them), a row each, their
 * results filled in per mode by the page. */
export function studioChecks(data) {
  const label = (c) => {
    if (c.contrast) return [`<code>${c.contrast[0]}</code> on <code>${c.contrast[1]}</code>`, `contrast ≥ ${c.min}:1`];
    if (c.distinct) return [`<code>${c.distinct}</code> distinct`, `ΔE ≥ ${c.min}${c.cvd ? `, ≥ ${c.min / 2} for colour-blind readers` : ""}${c.first ? `, first ${c.first}` : ""}`];
    if (c.monotone) return [`<code>${c.monotone}</code> ramp`, "lightness only rises or falls"];
    return [escapeHtml(JSON.stringify(c)), ""];
  };
  const rows = data.authored.checks.map((c, i) => {
    const [what, rule] = label(c);
    return `<tr data-check="${i}"><th scope="row">${what}<small>${escapeHtml(rule)}</small></th>${["light", "dark", "high-contrast"].map((m) => `<td data-mode="${m}"><span class="ck">—</span></td>`).join("")}</tr>`;
  }).join("");
  return `<div class="table-wrap checks-wrap"><table class="checks"><thead><tr><th>Check</th><th>Light</th><th>Dark</th><th>High contrast</th></tr></thead><tbody>${rows}</tbody></table></div>`;
}

/** The tokens a chart reads, as chips under its tile: every recipe it uses declares its tokens
 * (`datars describe`), its own scene names more as inks (`"$card"`) and `token("…")`, and its
 * resolved first state (`datars inspect`, given as `drawn`) shows the inks the recipes drew with —
 * a colour scale's `$categorical[2]`, say, which no declaration lists. */
export function chartTokens(doc, std, drawn = "") {
  const tokens = new Set();
  for (const m of drawn.matchAll(/\$([a-z][a-z0-9.-]*[a-z0-9])/g)) tokens.add(m[1]);
  const walk = (v) => {
    if (Array.isArray(v)) v.forEach(walk);
    else if (v && typeof v === "object") {
      if (v.kind === "use" && typeof v.recipe === "string") for (const t of std[v.recipe]?.tokens ?? []) tokens.add(t);
      Object.values(v).forEach(walk);
    } else if (typeof v === "string") {
      for (const m of v.matchAll(/\$([a-z][a-z0-9.-]*[a-z0-9])/g)) tokens.add(m[1]);
      for (const m of v.matchAll(/token\("([^"]+)"\)/g)) tokens.add(m[1]);
    }
  };
  walk(doc.scene);
  return [...tokens].sort();
}
