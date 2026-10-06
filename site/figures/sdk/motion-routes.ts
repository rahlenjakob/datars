// `route.*`: the path an element travels. In every panel four dots swap sides (and order) when you
// step; each panel's motion rule picks a different route. `spiral` and `explode` swirl around and
// burst from the centre of the whole scene, so their dots leave their panel on the way.
import { data, doc, e, geom, group, motion, repeat, shape, signal, step, story, text, route, type Route, type Rule } from "@datars/sdk";

const ROUTES: [string, Route][] = [
  ["straight()", route.straight()], ["arc(0.45)", route.arc(0.45)], ["elbow()", route.elbow()], ["spiral(1)", route.spiral(1)],
  ["explode()", route.explode()], ["hop(24)", route.hop(24)], ["drift(1, 30)", route.drift(1, 30)], ["drop(0.3)", route.drop(0.3)],
];
const key = (label: string) => label.slice(0, label.indexOf("("));

const x = e("side ? box.w - 22 : 22");
const y = e("side ? 38 + (3 - d.i) * (box.h - 52) / 3 : 38 + d.i * (box.h - 52) / 3");

const panel = ([label]: [string, Route]) => group({
  key: key(label),
  children: [
    shape(geom.rect({ x: 0, y: 0, w: e("box.w"), h: e("box.h"), r: 6 }), { key: "bg", fill: "$surface" }),
    text(`route.${label}`, [8, 8], { key: "label", style: { font: "font.strong", size: "$size.small", ink: "$ink", baseline: "top" } }),
    repeat("dots", shape(geom.circle({ cx: x, cy: y, r: 6 }), { key: e("d.i"), fill: e("'$categorical[' + d.i + ']'") })),
  ],
});

const grid = (columns: number, when: string) => group({ key: "root", when: e(when), layout: { type: "grid", columns, gap: 10, padding: [10, 12, 10, 12] }, children: ROUTES.map(panel) });

export default doc({
  id: "sdk-motion-routes",
  title: "Routes",
  description: "Eight panels of four dots swapping sides, each travelling a different route: straight, an arc, an elbow, a spiral, an explosion, a hop, a drift and a drop.",
  size: [640, 300],
  data: { dots: data.values({ i: [0, 1, 2, 3] }, { key: "i" }) },
  signals: { side: signal.bool(false) },
  scene: group({ key: "figure", children: [grid(4, 'sizeClass != "phone"'), grid(2, 'sizeClass == "phone"')] }),
  motion: motion(
    { duration: 1.8, easing: "cubic-in-out" },
    ...ROUTES.map(([label, r]): Rule => ({ select: { key: `figure/root/${key(label)}` }, route: r })),
  ),
  program: story({ steps: [step("left", { set: { side: false } }), step("right", { set: { side: true } })] }),
});
