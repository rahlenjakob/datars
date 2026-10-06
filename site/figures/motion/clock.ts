// Ambient motion from a clock: a signal declared with a rate (`signal.clock(1)`) counts up while the
// settled scene reads it, so the planets go round with no animation code — each at its own period
// (Mercury's year is a quarter of ours). Step to "sizes" and they line up by size: the clock pauses
// for the transition (a morph never jumps), and that state doesn't read it, so the chart sleeps.
// Off screen, and for readers who prefer reduced motion, it pauses too; renders see its default.
// Orbits and sizes are not to scale with each other.
import { data, doc, e, geom, group, motion, repeat, shape, signal, step, story, text, choreo, route } from "@datars/sdk";

const name = ["Mercury", "Venus", "Earth", "Mars", "Jupiter", "Saturn"];
const period = [0.241, 0.615, 1, 1.881, 11.86, 29.46]; // years
const km = [2440, 6052, 6371, 3390, 69911, 58232]; // radius
const keys = { Mercury: { color: "#a8a29e" }, Venus: { color: "#e9c46a" }, Earth: { color: "#4f8fdc" }, Mars: { color: "#d8613c" }, Jupiter: { color: "#d4a373" }, Saturn: { color: "#e6c88f" } };

const cx = "box.w / 2", cy = "box.h / 2";
const R = "(min(box.w / 2, box.h / 2) - 14)";
const orbit = `${R} * (0.2 + 0.8 * d.i / 5)`;
// A tenth of a year a second; each planet starts somewhere along its orbit.
const angle = "6.2832 * (t * 0.1 / d.period + d.i * 0.137)";
// Planet sizes: √ of the radius, so Mercury stays visible next to Jupiter.
const size = (k: number) => `${k} * sqrt(d.km / 69911)`;

export default doc({
  id: "motion-clock",
  title: "Planets on a clock",
  description: "Six planets going round the Sun, each at its own period, driven by a clock; then lined up from smallest to largest.",
  size: [640, 340],
  data: { planets: data.values({ name, period, km, i: name.map((_, i) => i), rank: name.map((n) => [...km].sort((a, b) => a - b).indexOf(km[name.indexOf(n)])) }, { key: "name" }) },
  keys,
  signals: { t: signal.clock(1) },
  scene: group({
    key: "root",
    children: [
      group({ key: "orbits", when: e('state == "orbits"'), children: [
        repeat("planets", shape(geom.circle({ cx: e(cx), cy: e(cy), r: e(orbit) }), { key: e("d.name + '-orbit'"), stroke: { paint: "$grid", width: 1 }, semantics: { role: "decoration" } })),
        shape(geom.circle({ cx: e(cx), cy: e(cy), r: e(`${R} * 0.09`) }), { key: "sun", fill: "#f4a261", semantics: { role: "decoration" } }),
      ] }),
      group({ key: "planets", children: [
        repeat("planets", shape(geom.circle({
          cx: e(`state == "orbits" ? ${cx} + ${orbit} * cos(${angle}) : 20 + (d.rank + 0.5) * (box.w - 40) / 6`),
          cy: e(`state == "orbits" ? ${cy} + ${orbit} * sin(${angle}) : box.h / 2`),
          r: e(`state == "orbits" ? ${size(14)} + 2 : min(box.w / 13, box.h / 3) * sqrt(d.km / 69911) * 1.2`),
        }), { key: e("d.name"), fill: e("key.color(d.name)"), semantics: { role: "datum", label: e("d.name"), value: e("d.km") } })),
        repeat("planets", text(e("d.name"), [e("20 + (d.rank + 0.5) * (box.w - 40) / 6"), e("box.h / 2 + min(box.w / 13, box.h / 3) * 1.25 + 14")], {
          key: e("d.name + '-name'"), when: e('state == "sizes"'), style: { size: "$size.small", ink: "$muted", align: "middle", baseline: "top" },
        })),
      ] }),
    ],
  }),
  motion: motion(
    { duration: 1.4, easing: "cubic-in-out" },
    { select: { role: "datum" }, route: route.arc(0.3), choreo: choreo.stagger("data", 0.4) },
  ),
  program: story({ steps: [step("orbits", { title: "On a clock", text: "Each planet at its own period." }), step("sizes", { title: "By size", text: "Smallest to largest; the clock rests." })] }),
});
