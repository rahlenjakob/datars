// Under the hood, §6 (maps): a camera over a tile pyramid. The view needs the squares that cover
// it at one zoom; zooming in, the next level's squares fade in over the coarser data drawn there
// before (crates/datars-engine/src/tiles/fill.rs, FADE_S = 0.3 s), a square whose tile hasn't
// arrived shows its ancestor overzoomed meanwhile, and zooming out the coarser squares draw at once
// while the finer ones fade out on top. Schematic: squares here are drawn about 100 px across (the
// engine picks the zoom at which a tile would span about 512 px), and the "map" is made up.
import { doc, e, geom, group, motion, shape, signal, step, story, text, view, type Rule, type Template } from "@datars/sdk";
import { label, narration, PHONE, SIZE } from "./_kit";

const WORLD = 1024;

// A made-up landmass: an octagon refined by seeded midpoint displacement. Level 2 keeps two
// refinements, level 3 four (the same outline, more detail), plus lakes that exist from level 3.
let seed = 11;
const rnd = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648) - 0.5;
function refine(pts: [number, number][], amp: number): [number, number][] {
  const out: [number, number][] = [];
  for (let i = 0; i < pts.length; i++) {
    const a = pts[i], b = pts[(i + 1) % pts.length];
    const [mx, my] = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2];
    const [dx, dy] = [b[1] - a[1], a[0] - b[0]]; // normal
    const k = rnd() * amp;
    out.push(a, [mx + dx * k, my + dy * k]);
  }
  return out;
}
const ring0: [number, number][] = [[330, 250], [560, 190], [760, 300], [830, 470], [760, 650], [560, 760], [360, 690], [250, 470]];
const levels: [number, number][][] = [ring0];
for (let i = 0; i < 4; i++) levels.push(refine(levels[i], 0.42 / (i + 1)));
const path = (pts: [number, number][]) => `M ${pts.map((p) => `${p[0].toFixed(1)} ${p[1].toFixed(1)}`).join(" L ")} Z`;
const LAND: Record<number, string> = { 2: path(levels[2]), 3: path(levels[4]) };
const ellipse = (cx: number, cy: number, rx: number, ry: number) => path(Array.from({ length: 24 }, (_, i) => [cx + rx * Math.cos((i / 24) * 2 * Math.PI), cy + ry * Math.sin((i / 24) * 2 * Math.PI)] as [number, number]));
const LAKES = [ellipse(690, 420, 30, 20), ellipse(640, 540, 20, 13), ellipse(735, 505, 12, 9)].join(" ");

// The camera's box in each step (world units, aspect 3:2), and the squares covering it.
const WIDE_BOX = [160, 224, 928, 736];
const NEAR_BOX = [576, 352, 960, 608];
const cover = (b: number[], z: number) => {
  const s = WORLD / 2 ** z, out: [number, number][] = [];
  for (let y = Math.floor(b[1] / s); y <= Math.ceil(b[3] / s) - 1; y++) for (let x = Math.floor(b[0] / s); x <= Math.ceil(b[2] / s) - 1; x++) out.push([x, y]);
  return out;
};
const Z2 = cover(WIDE_BOX, 2);
const Z3 = cover(NEAR_BOX, 3);
const WAITING = ["3/6/3", "3/5/4"]; // tiles still on their way in the "zoom in" step

/** One square of level z: its data (land, lakes from z3), clipped to the square, and its id. */
function square(z: number, x: number, y: number, when: string, labelWhen: string): Template {
  const s = WORLD / 2 ** z, id = `${z}/${x}/${y}`;
  // Only squares whose centre is in view get their id (half-shown ones would have it cut).
  const b = z === 2 ? WIDE_BOX : NEAR_BOX, [cx, cy] = [x * s + s / 2, y * s + s / 2];
  const inView = cx > b[0] && cx < b[2] && cy > b[1] && cy < b[3];
  return group({
    key: id,
    when: e(when),
    clip: [x * s, y * s, s, s],
    children: [
      shape(geom.rect({ x: x * s, y: y * s, w: s, h: s }), { key: "sea", fill: "$map.water", semantics: { role: "decoration" } }),
      shape(geom.path(LAND[z]), { key: "land", fill: "$map.land", stroke: { paint: "$map.border", width: 1, nonScaling: true }, semantics: { role: "decoration" } }),
      z >= 3 ? shape(geom.path(LAKES), { key: "lakes", fill: "$map.water", semantics: { role: "decoration" } }) : null,
      shape(geom.rect({ x: x * s, y: y * s, w: s, h: s }), { key: "edge", stroke: { paint: "$ink@0.28", width: 1, nonScaling: true }, semantics: { role: "decoration" } }),
      text(id, [x * s + s / 2, y * s + s / 2], { key: "id", pin: true, when: e(inView ? labelWhen : "false"), halo: ["$map.land", 2], style: { size: 10, ink: "$ink-2", align: "middle", baseline: "middle", font: "font.strong" } }),
    ],
  });
}

const screen = (): Template => group({
  key: "screen",
  size: { w: e("box.w") },
  children: [
    label("head", "THE VIEW", [0, 0], { size: SIZE.small, ink: "$muted", strong: true, baseline: "top" }),
    view({
      key: "view",
      transform: { translate: [0, 20] },
      size: { w: e("box.w"), h: e("box.w / 1.5") },
      camera: { fit: { bbox: [e("bx0"), e("by0"), e("bx1"), e("by1")] }, padding: 0 },
      children: [
        group({ key: "z2", children: Z2.map(([x, y]) => square(2, x, y, "true", "level == 2")) }),
        group({ key: "z3", children: Z3.map(([x, y]) => square(3, x, y, WAITING.includes(`3/${x}/${y}`) ? "level == 3 && arrived" : "level == 3", "level == 3")) }),
        // Squares whose tile is still on its way: the ancestor shows through, overzoomed.
        group({ key: "wait", children: Z3.filter(([x, y]) => WAITING.includes(`3/${x}/${y}`)).map(([x, y]) => {
          const s = WORLD / 8;
          return group({ key: `3/${x}/${y}`, when: e("level == 3 && !arrived"), children: [
            shape(geom.rect({ x: x * s + 3, y: y * s + 3, w: s - 6, h: s - 6 }), { key: "mark", stroke: { paint: "$accent", width: 1.5, dash: [5, 4], nonScaling: true }, semantics: { role: "decoration" } }),
            text(`3/${x}/${y}`, [x * s + s / 2, y * s + s / 2], { key: "id", pin: true, offset: [0, -7], halo: ["$map.land", 2], style: { size: 10, ink: "$accent", align: "middle", baseline: "middle", font: "font.strong" } }),
            text(`shows 2/${x >> 1}/${y >> 1}`, [x * s + s / 2, y * s + s / 2], { key: "from", pin: true, offset: [0, 7], halo: ["$map.land", 2], style: { size: 10, ink: "$accent", align: "middle", baseline: "middle" } }),
          ] });
        }) }),
      ],
    }),
  ],
});

// The whole world at one level, the camera's box on it and the squares that box needs.
const M = 132; // minimap size, px
const k = M / WORLD;
const minimap = (): Template => group({
  key: "mini",
  children: [
    shape(geom.rect({ x: 0, y: 0, w: M, h: M }), { key: "world", fill: "$surface", stroke: { paint: "$rule", width: 1 }, semantics: { role: "decoration" } }),
    group({ key: "need2", when: e("level == 2"), children: Z2.map(([x, y]) => shape(geom.rect({ x: x * M / 4, y: y * M / 4, w: M / 4, h: M / 4 }), { key: `${x}-${y}`, fill: "$accent@0.18", semantics: { role: "decoration" } })) }),
    group({ key: "need3", when: e("level == 3"), children: Z3.map(([x, y]) => shape(geom.rect({ x: x * M / 8, y: y * M / 8, w: M / 8, h: M / 8 }), { key: `${x}-${y}`, fill: "$accent@0.18", semantics: { role: "decoration" } })) }),
    group({ key: "grid2", when: e("level == 2"), children: [1, 2, 3].flatMap((i) => [
      shape(geom.segment({ x1: i * M / 4, y1: 0, x2: i * M / 4, y2: M }), { key: `v${i}`, stroke: { paint: "$grid", width: 1 }, semantics: { role: "decoration" } }),
      shape(geom.segment({ x1: 0, y1: i * M / 4, x2: M, y2: i * M / 4 }), { key: `h${i}`, stroke: { paint: "$grid", width: 1 }, semantics: { role: "decoration" } }),
    ]) }),
    group({ key: "grid3", when: e("level == 3"), children: [1, 2, 3, 4, 5, 6, 7].flatMap((i) => [
      shape(geom.segment({ x1: i * M / 8, y1: 0, x2: i * M / 8, y2: M }), { key: `v${i}`, stroke: { paint: "$grid", width: 1 }, semantics: { role: "decoration" } }),
      shape(geom.segment({ x1: 0, y1: i * M / 8, x2: M, y2: i * M / 8 }), { key: `h${i}`, stroke: { paint: "$grid", width: 1 }, semantics: { role: "decoration" } }),
    ]) }),
    shape(geom.rect({ x: e(`bx0 * ${k}`), y: e(`by0 * ${k}`), w: e(`(bx1 - bx0) * ${k}`), h: e(`(by1 - by0) * ${k}`) }), { key: "camera", stroke: { paint: "$accent", width: 1.5 }, semantics: { role: "decoration" } }),
  ],
});

const side = (): Template => group({
  key: "side",
  children: [
    label("head", "THE SQUARES IT NEEDS", [0, 0], { size: SIZE.small, ink: "$muted", strong: true, baseline: "top" }),
    group({ key: "mini-at", transform: { translate: [0, 20] }, children: [minimap()] }),
    group({ key: "legend", transform: { translate: [M + 16, 20] }, children: [
      label("zoom", e("'zoom ' + level + ': ' + (level == 2 ? '4 × 4' : '8 × 8') + ' squares'"), [0, 8], { strong: true }),
      label("need", e("'the view needs ' + (level == 2 ? " + Z2.length + " : " + Z3.length + ")"), [0, 28], { ink: "$ink-2" }),
      shape(geom.rect({ x: 0, y: 46, w: 14, h: 14, r: 2 }), { key: "k-need", fill: "$accent@0.18", semantics: { role: "decoration" } }),
      label("l-need", "needed", [20, 53], { size: SIZE.small, ink: "$ink-2" }),
      shape(geom.rect({ x: 0, y: 68, w: 14, h: 14 }), { key: "k-cam", stroke: { paint: "$accent", width: 1.5 }, semantics: { role: "decoration" } }),
      label("l-cam", "the camera's box", [20, 75], { size: SIZE.small, ink: "$ink-2" }),
      shape(geom.rect({ x: 0, y: 90, w: 14, h: 14 }), { key: "k-wait", stroke: { paint: "$accent", width: 1.5, dash: [3, 2] }, semantics: { role: "decoration" } }),
      label("l-wait", "an ancestor stands in", [20, 97], { size: SIZE.small, ink: "$ink-2" }),
    ] }),
  ],
});

const layout = (phone: boolean) => group({
  key: phone ? "phone" : "wide",
  when: e(phone ? PHONE : `!(${PHONE})`),
  layout: { type: "rows", gap: 14, padding: phone ? [12, 12, 8, 12] : [14, 18, 10, 18] },
  children: [
    group({ key: "main", layout: phone ? { type: "rows", gap: 16 } : { type: "columns", gap: 28 }, children: [
      group({ key: "screen-at", size: phone ? { h: e("box.w / 1.5 + 20") } : { w: e("min(box.w - 300, (box.h - 24) * 1.5)") }, children: [screen()] }),
      group({ key: "side-at", size: phone ? { h: M + 24 } : undefined, children: [side()] }),
    ] }),
    narration(2, 3),
  ],
});

const at = (l: string) => `root/${l}/main/screen-at/screen/view`;
const rules: Rule[] = [{ duration: 1.1, easing: "cubic-in-out" }];
for (const l of ["wide", "phone"]) {
  // Arriving detail fades in over 0.3 s once the camera is there; leaving detail fades out on top.
  rules.push(
    { when: { to: "zoom in" }, select: { key: `${at(l)}/z3` }, delay: 1.1, duration: 0.3, enter: { opacity: 0 } },
    { when: { to: "zoom in" }, select: { key: `${at(l)}/wait` }, delay: 1.1, duration: 0.3, enter: { opacity: 0 } },
    { when: { to: "arrived" }, select: { key: `${at(l)}/z3` }, duration: 0.3, enter: { opacity: 0 } },
    { when: { to: "arrived" }, select: { key: `${at(l)}/wait` }, duration: 0.3, exit: { opacity: 0 } },
    { when: { to: "zoom out" }, select: { key: `${at(l)}/z3` }, duration: 0.3, exit: { opacity: 0 } },
    { when: { to: "zoom out" }, select: { key: `${at(l)}/wait` }, duration: 0.3, exit: { opacity: 0 } },
  );
}

const box = (b: number[]) => ({ bx0: b[0], by0: b[1], bx1: b[2], by1: b[3] });

export default doc({
  id: "how-tiles",
  title: "A camera over a tile pyramid",
  description: "A view over a made-up map drawn from tiles: twelve zoom-2 squares, then zoomed in, twelve zoom-3 squares fading in over them with two still arriving and their ancestor standing in, then zoomed out again.",
  size: [680, 330],
  signals: { bx0: signal.num(WIDE_BOX[0]), by0: signal.num(WIDE_BOX[1]), bx1: signal.num(WIDE_BOX[2]), by1: signal.num(WIDE_BOX[3]), level: signal.num(2), arrived: signal.bool(true) },
  scene: group({ key: "root", children: [layout(false), layout(true)] }),
  motion: motion(...rules),
  program: story({ steps: [
    step("zoom 2", { set: { ...box(WIDE_BOX), level: 2, arrived: true }, text: "The view needs the squares that cover it at one zoom: twelve zoom-2 tiles, each fetched by range." }),
    step("zoom in", { set: { ...box(NEAR_BOX), level: 3, arrived: false }, text: "Zoomed in twice as far: zoom-3 squares fade in over the coarser data; two tiles are still arriving, so their ancestor shows there." }),
    step("arrived", { set: { ...box(NEAR_BOX), level: 3, arrived: true }, text: "The last two arrive and fade in over their stand-in in 0.3 s: detail sharpens into place instead of popping." }),
    step("zoom out", { set: { ...box(WIDE_BOX), level: 2, arrived: true }, text: "Zoomed out, the coarser squares draw at once and the finer ones fade out on top of them: nothing shows through." }),
  ] }),
});
