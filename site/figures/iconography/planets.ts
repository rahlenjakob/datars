// The iconography page's pictures: NASA's photographs of the eight planets, each cut to a circle
// (Saturn kept whole, its rings over the page through the WebP's alpha) — to scale by size, on a
// log scale by distance from the Sun, and as bars of their moons with the photo at the end. Every
// photo is one keyed `image()` node, so it flies from layout to layout, its crop with it. Figures
// from NASA's Planetary Fact Sheet (18 March 2025); the photographs' credits are in `credits`.
import { data, doc, e, geom, group, image, repeat, shape, story, step, text } from "@datars/sdk";

type Planet = { id: string; name: string; d: number; au: number; moons: number; file: string; by: string; credit: string; nasa: string };
const P: Planet[] = [
  { id: "mercury", name: "Mercury", d: 4879, au: 0.387, moons: 0, file: "mercury.jpg", by: "MESSENGER", credit: "NASA/Johns Hopkins University Applied Physics Laboratory/Carnegie Institution of Washington", nasa: "PIA15160" },
  { id: "venus", name: "Venus", d: 12104, au: 0.723, moons: 0, file: "venus.jpg", by: "Mariner 10", credit: "NASA/JPL-Caltech", nasa: "PIA23791" },
  { id: "earth", name: "Earth", d: 12756, au: 1, moons: 1, file: "earth.jpg", by: "the crew of Apollo 17", credit: "NASA", nasa: "as17-148-22727" },
  { id: "mars", name: "Mars", d: 6792, au: 1.524, moons: 2, file: "mars.jpg", by: "the Viking orbiters", credit: "NASA/JPL/USGS", nasa: "PIA00407" },
  { id: "jupiter", name: "Jupiter", d: 142984, au: 5.204, moons: 95, file: "jupiter.jpg", by: "Cassini", credit: "NASA/JPL/University of Arizona", nasa: "PIA02873" },
  { id: "saturn", name: "Saturn", d: 120536, au: 9.573, moons: 274, file: "saturn.webp", by: "Cassini", credit: "NASA/JPL/Space Science Institute", nasa: "PIA11141" },
  { id: "uranus", name: "Uranus", d: 51118, au: 19.165, moons: 28, file: "uranus.jpg", by: "Voyager 2", credit: "NASA/JPL-Caltech", nasa: "PIA18182" },
  { id: "neptune", name: "Neptune", d: 49528, au: 30.04, moons: 16, file: "neptune.jpg", by: "Voyager 2", credit: "NASA/JPL", nasa: "PIA01492" },
];
// Saturn's photograph is its rings' width and a little more; its globe is 0.415 of that, and the
// photo 237 px tall to its 560.
const RINGS = 1 / 0.4154, SAT_H = 237 / 560;
const width = (p: Planet) => (p.id === "saturn" ? p.d * RINGS : p.d);
let cum = 0;
const rows = P.map((p, i) => { const r = { ...p, i, w: width(p), cum }; cum += width(p); return r; });
const TOTAL = cum;
const col = <K extends keyof (typeof rows)[number]>(k: K) => rows.map((r) => r[k]);

const at = (s: string) => `state == "${s}"`;
const by3 = (size: string, dist: string, moons: string) => `(${at("size")} ? ${size} : ${at("distance")} ? ${dist} : ${moons})`;
// Narrow for its text: a phone, or a reader's big text in a tablet's width.
const narrow = "box.w < 640 * max(1, textScale)";
// A label is a name and a value, two lines at the reader's text size; they sit in eight evenly
// spaced slots, in two rows by turns (even planets one row, odd the other), each tied to its planet
// by a leader — the planets themselves where the data puts them.
const L1 = 'token("size.label") * 1.25', L2 = 'token("size.small") * 1.3';
const LB = `(${L1} + ${L2})`;
const odd = "(d.i % 2 == 1)";
// To scale: one km per k px, side by side with a gap between, after the Sun's edge; as big as the
// width allows and the label rows leave room for.
const left = "(box.w * 0.125)";
const gap = "max(8, box.w * 0.03)";
const zone = `(box.h - (${narrow} ? ${L1} * 4 : ${LB} * 2) - 34)`;
const k = `min((box.w - ${left} - 12 - ${gap} * 7) / ${TOTAL}, ${zone} / 142984)`;
const cyS = "(box.h / 2)";
const sx = `(${left} + d.cum * ${k} + d.i * ${gap} + d.w * ${k} / 2)`;
const sw = `(d.w * ${k})`;
// Wide, each label sits over or under its own planet; on a phone, in eight even slots.
const slotS = `(${narrow} ? ${left} + (box.w - ${left}) * (d.i + 0.5) / 8 : ${sx})`;
// On a phone the to-scale labels are names only, in four rows (two above, two below), so a row's
// names are four slots apart.
const deep = `(${narrow} && d.i % 4 >= 2 ? ${L1} : 0)`;
const topS = `(${odd} ? 4 + ${deep} : box.h - (${narrow} ? ${L1} : ${LB}) - 4 - ${deep})`;
const edgeS = `(${odd} ? (${narrow} ? ${L1} : ${LB}) + 10 + ${deep} : box.h - (${narrow} ? ${L1} : ${LB}) - 10 - ${deep})`;
// By distance: a log axis from 0.3 to 40 AU; every planet drawn big enough to see (√ of its size);
// the labels in two rows under them, the axis under those.
const lx = `(28 + (log10(d.au) - log10(0.3)) / (log10(40) - log10(0.3)) * (box.w - 56))`;
const dh = "min(box.h, box.w * 0.44)";
const dd = `(${dh} * 0.04 + sqrt(d.d / 142984) * ${dh} * 0.105)`;
const dw = `(d.id == "saturn" ? ${dd} * ${RINGS} : ${dd})`;
// The block — planets, two label rows, the axis and its title — centred in the box.
const maxHalf = `(${dh} * 0.0725)`;
const blockD = `(${maxHalf} * 2 + 12 + ${LB} * 2 + 4 + 16 + ${L2} * 2 + 8)`;
const cyD = `((box.h - ${blockD}) / 2 + ${maxHalf})`;
const rowD = `(${cyD} + ${maxHalf} + 12)`;
const topD = `(${rowD} + (${odd} ? ${LB} + 4 : 0))`;
const slotD = `(28 + (box.w - 56) * (d.i + 0.5) / 8)`;
const axisY = `(${rowD} + ${LB} * 2 + 4 + 16)`;
// By moons: a bar a planet, the photo at its end.
const rowH = "((box.h - 20) / 8)";
const my = `(10 + ${rowH} * (d.i + 0.5))`;
const labelW = `min(box.w * 0.3, max(measure("Mercury", token("size.label"), 600), measure("Neptune", token("size.label"), 600)) + 4)`;
const ph = `(${rowH} * 0.84)`;
const bar = `((box.w - ${labelW} - 10 - ${ph} * ${RINGS} - measure("274", token("size.small"), 400) - 14) * d.moons / 274)`;
const mx = `(${labelW} + 10 + ${bar} + ${ph} * (d.id == "saturn" ? ${RINGS} : 1) / 2)`;
const mw = `(d.id == "saturn" ? ${ph} * ${RINGS} : ${ph})`;
// The photo's rect: its centre and width in each layout (Saturn's height is its photo's aspect).
const cx = by3(sx, lx, mx), cy = by3(cyS, cyD, my), w = by3(sw, dw, mw);
const h = `(d.id == "saturn" ? ${w} * ${SAT_H} : ${w})`;
// The globe's own half height, for leaders.
const half = `(d.id == "saturn" ? ${w} / ${RINGS} / 2 : ${w} / 2)`;
const value = `(${at("size")} ? format(d.d, ",.0f") + (${narrow} ? "" : " km") : ${at("distance")} ? format(d.au, ",.2~f") + (${narrow} ? "" : " AU") : format(d.moons, ",.0f"))`;
// A slot's label kept inside the box: half its widest line from either edge.
const lw = `(max(measure(d.name, token("size.label"), 600), measure(${value}, token("size.small"), 400)) / 2 + 2)`;
const keep = (x: string) => `clamp(${x}, ${lw}, box.w - ${lw})`;

export default doc({
  id: "iconography-planets",
  title: "The planets, photographed",
  description: "NASA's photographs of the eight planets: to scale by size, where Jupiter is 29 times as wide as Mercury; by distance from the Sun on a logarithmic scale, from 0.39 astronomical units for Mercury to 30 for Neptune; and by their moons, from none for Mercury and Venus to 274 for Saturn.",
  size: [720, 320],
  data: { planets: data.values({ id: col("id"), name: col("name"), d: col("d"), au: col("au"), moons: col("moons"), file: col("file"), by: col("by"), i: col("i"), w: col("w"), cum: col("cum") }, { key: "id" }) },
  credits: Object.fromEntries(P.map((p) => [`assets/planets/${p.file}`, { licence: "public-domain", credit: p.credit, source: p.nasa.startsWith("PIA") ? `https://photojournal.jpl.nasa.gov/catalog/${p.nasa}` : `https://images.nasa.gov/details/${p.nasa}` }])),
  scene: group({
    key: "root",
    children: [
      // The Sun's edge, to the same scale (its 1.39 million km would be the page and more).
      shape(geom.circle({ cx: e(`${left} - box.h * 1.5 - 34`), cy: e(cyS), r: e("box.h * 1.5") }), { key: "sun", when: e(at("size")), fill: "$highlight", opacity: 0.9, semantics: { role: "decoration" } }),
      text("The Sun's edge", [e(`${left} - 42`), e(cyS)], { key: "sun-label", when: e(`${at("size")} && !(${narrow})`), rotate: -90, style: { size: "$size.small", weight: 600, ink: "on($highlight)", align: "middle", baseline: "middle" } }),
      // The distance axis.
      group({ key: "axis", when: e(at("distance")), children: [
        shape(geom.segment({ x1: 28, y1: e(axisY), x2: e("box.w - 28"), y2: e(axisY) }), { key: "line", stroke: { paint: "$rule", width: 1 } }),
        ...[0.5, 1, 2, 5, 10, 20, 30].map((au) => {
          const x = `(28 + (log10(${au}) - log10(0.3)) / (log10(40) - log10(0.3)) * (box.w - 56))`;
          return group({ key: `t${au}`, children: [
            shape(geom.segment({ x1: e(x), y1: e(`${axisY} - 4`), x2: e(x), y2: e(`${axisY} + 4`) }), { key: "tick", stroke: { paint: "$rule", width: 1 } }),
            text(au === 1 ? "1 AU" : String(au), [e(x), e(`${axisY} + 6`)], { key: "n", style: { size: "$size.small", ink: "$muted", align: "middle", baseline: "top" } }),
          ] });
        }),
        text(e(`${narrow} ? "AU from the Sun, log scale" : "Distance from the Sun, astronomical units (Earth = 1), log scale"`), [e("box.w / 2"), e(`${axisY} + 8 + ${L2}`)], { key: "title", style: { size: "$size.small", ink: "$ink-2", align: "middle", baseline: "top" } }),
      ] }),
      repeat("planets", group({
        key: e("d.id"),
        semantics: { role: "datum", label: e('d.name + ": " + format(d.d, ",.0f") + " km across, " + format(d.au, ",.2~f") + " AU from the Sun, " + format(d.moons, ",.0f") + (d.moons == 1 ? " moon" : " moons")'), value: e("d.d") },
        children: [
          shape(geom.rect({ x: e(`${labelW} + 10`), y: e(`${my} - ${rowH} * 0.16`), w: e(bar), h: e(`${rowH} * 0.32`), r: 3 }), { key: "bar", when: e(at("moons")), fill: "$accent", opacity: 0.85, semantics: { role: "decoration" } }),
          // Leaders from each planet to its label's slot (to scale and by distance).
          shape(geom.segment({ x1: e(sx), y1: e(`${cyS} + (${odd} ? -1 : 1) * (${half} + 3)`), x2: e(keep(slotS)), y2: e(edgeS) }), { key: "leader", when: e(at("size")), stroke: { paint: "$rule", width: 1 }, semantics: { role: "decoration" } }),
          shape(geom.segment({ x1: e(lx), y1: e(`${cyD} + ${half} + 3`), x2: e(keep(slotD)), y2: e(`${topD} - 3`) }), { key: "leader-d", when: e(at("distance")), stroke: { paint: "$rule", width: 1 }, semantics: { role: "decoration" } }),
          image(e('"assets/planets/" + d.file'), [e(`${cx} - ${w} / 2`), e(`${cy} - ${h} / 2`), e(w), e(h)], {
            key: "photo", fit: "cover", crop: e('d.id == "saturn" ? "rect" : "circle"'), alt: e('d.name + ", photographed by " + d.by'),
          }),
          text(e("d.name"), [e(by3(keep(slotS), keep(slotD), labelW)), e(by3(topS, topD, my))], {
            key: "name", style: { size: "$size.label", weight: 600, ink: "$ink", align: e(`${at("moons")} ? "end" : "middle"`) as never, baseline: e(`${at("moons")} ? "middle" : "top"`) as never },
          }),
          text(e(value), [e(by3(keep(slotS), keep(slotD), `${mx} + ${mw} / 2 + 8`)), e(by3(`${topS} + ${L1}`, `${topD} + ${L1}`, my))], {
            key: "value", when: e(`!(${at("size")} && ${narrow})`), style: { size: "$size.small", ink: "$ink-2", align: e(`${at("moons")} ? "start" : "middle"`) as never, baseline: e(`${at("moons")} ? "middle" : "top"`) as never },
          }),
        ],
      })),
    ],
  }),
  program: story({
    steps: [
      step("size", { title: "To scale", text: "Jupiter is 29 times as wide as Mercury; Earth is a dot beside it." }),
      step("distance", { title: "By distance", text: "On a log scale: Neptune is 78 times as far from the Sun as Mercury." }),
      step("moons", { title: "By moons", text: "Saturn has 274 known moons; Mercury and Venus have none." }),
    ],
  }),
});
