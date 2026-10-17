// `icon()`: where an icon comes from and how it's painted. Six cells: the default set by name, a
// theme icon token, a project SVG whose colours become ink slots, inline path data, a turned icon
// and an icon as a scatter's symbol. The second step repaints every one through its inks alone.
import { doc, e, geom, group, icon, shape, signal, story, step, text, type Template } from "@datars/sdk";

const warm = 'look == "warm"';
const ink = (a: string, b: string) => e(`${warm} ? "${b}" : "${a}"`);
// On a phone the captions shorten to what tells the cells apart.
const cap = (long: string, short: string) => e(`sizeClass == "phone" ? ${JSON.stringify(short)} : ${JSON.stringify(long)}`);
const cell = (key: string, caption: ReturnType<typeof e>, body: Template): Template => group({
  key,
  children: [
    shape(geom.rect({ x: 2, y: 2, w: e("box.w - 4"), h: e("box.h - 4"), r: 6 }), { key: "cell", fill: "$surface" }),
    body,
    text(caption, [e("box.w / 2"), e("box.h - 12")], { key: "caption", style: { size: "$size.small", ink: "$ink-2", align: "middle", baseline: "bottom", maxWidth: e("box.w - 12") } }),
  ],
});
const mid: [ReturnType<typeof e>, ReturnType<typeof e>] = [e("box.w / 2"), e("box.h / 2 - 8")];
const big = e("min(56, box.h * 0.42, box.w * 0.5)");

export default doc({
  id: "sdk-icon-sources",
  title: "Icons from six sources",
  description: "Six icons: a heart from the default set, the theme's warning icon token, a transit badge from a project SVG with three ink slots, a triangle from inline path data, an arrow turned 45 degrees, and stars as a scatter's symbols — then the same six repainted in other inks.",
  size: [640, 300],
  signals: { look: signal.str("plain") },
  scene: group({
    key: "root",
    layout: { type: "grid", columns: 3, gap: 6, padding: [6, 8, 6, 8] },
    children: [
      cell("set", cap('icon("heart")', "heart"), icon("heart", mid, { key: "icon", size: big, ink: ink("$accent", "$negative"), label: "Heart" })),
      cell("token", cap('icon("$icon.warning")', "$icon.warning"), icon("$icon.warning", mid, { key: "icon", size: big, ink: ink("$ink", "$highlight") })),
      cell("file", cap('icon("assets/badge.svg", …, { inks })', "badge.svg + inks"), icon("assets/badge.svg", mid, { key: "icon", size: big, inks: [ink("$ink", "$categorical[2]"), "$paper", ink("$highlight", "$categorical[0]")], label: "Fernmouth Transit" })),
      cell("path", cap("icon({ d: \"M0 0L10 5L0 10Z\" })", "path data"), icon({ d: "M0 0L10 5L0 10Z" }, mid, { key: "icon", size: big, ink: ink("$categorical[3]", "$categorical[4]") })),
      cell("rotate", cap("{ rotate: 45 }", "rotate: 45"), icon("arrow-up", mid, { key: "icon", size: big, rotate: e(`${warm} ? 135 : 45`), ink: ink("$positive", "$negative") })),
      cell("symbol", cap('geom.symbol({ symbol: "lucide:star" })', "as a symbol"), group({ key: "stars", children: [0, 1, 2, 3, 4].map((i) =>
        shape(geom.symbol({ symbol: "lucide:star", x: e(`box.w * ${0.2 + i * 0.15}`), y: e(`box.h * ${[0.5, 0.3, 0.42, 0.22, 0.34][i]}`), size: 9 }), { key: `s${i}`, fill: ink("$categorical[1]", "$categorical[6]") })) })),
    ],
  }),
  program: story({
    steps: [
      step("plain", { set: { look: "plain" }, title: "Six sources", text: "A set's name, a theme token, a project file, path data, turned, and as a symbol." }),
      step("warm", { set: { look: "warm" }, title: "Inks, not colours", text: "The same icons, repainted: every colour is an ink." }),
    ],
  }),
});
