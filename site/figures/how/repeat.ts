// Under the hood, §4 (resolving): one `repeat` template times a table's rows gives one keyed node
// per row, laid out by scales that resolve in the node's layout box. The last step narrows the
// box: the same template and rows resolve again, every node keeps its key, and motion pairs them.
import { data, doc, e, geom, group, repeat, shape, signal, step, story, text, type Template } from "@datars/sdk";
import { box, label, line, narration, PHONE, SIZE, WIDE } from "./_kit";

const party = ["S", "SD", "M", "V", "C", "KD", "MP", "L"];
const share = [30.3, 20.5, 19.1, 6.8, 6.7, 5.3, 5.1, 4.6];

const CODE = [
  'repeat("votes", shape(',
  "  geom.rect({ x: 0,",
  "    y: scale.y(d.party),",
  "    w: scale.x(d.share),",
  "    h: scale.y.bandwidth() }),",
  "  { key: d.party }))",
];

const head = (key: string, s: string) => label(key, s, [0, 0], { size: SIZE.small, ink: "$muted", baseline: "top", strong: true });

/** The template, as code in a card. */
const template = (h: number, scales: boolean): Template => group({
  key: "template",
  children: [
    head("head", "TEMPLATE"),
    box("card", 0, 20, e("box.w"), h, { on: "phase == 0" }),
    ...CODE.map((c, i) => label(`l${i}`, c, [12, 40 + i * 18], { size: SIZE.small, ink: "$ink-2" })),
    // The scales in scope, resolved in the plot's box: its width is the x range.
    ...(scales ? [
      label("s-head", "SCALES", [0, h + 40], { size: SIZE.small, ink: "$muted", baseline: "top", strong: true }),
      box("s-card", 0, h + 60, e("box.w"), 62, { on: "phase == 3" }),
      label("s-y", "y: band over party", [12, h + 80], { size: SIZE.small, ink: "$ink-2" }),
      label("s-x", "x: linear, [0, 35] → [0, box.w − 34]", [12, h + 100], { size: SIZE.small, ink: "$ink-2" }),
    ] : []),
  ],
});

// The table and the plot share one band scale, so a row and its node sit on one line.
const rowsTop = 20;
const bandScale = { type: "band" as const, domain: party, range: [rowsTop + 20, e(`box.h - 4`)], padding: 0.25 };

const table = (): Template => group({
  key: "table",
  scales: { y: bandScale },
  children: [
    head("head", "TABLE votes"),
    group({ key: "rows", opacity: e("phase >= 1 ? 1 : 0"), children: [
      label("h-party", "key", [0, rowsTop + 8], { size: SIZE.small, ink: "$muted" }),
      label("h-share", "share", [e("box.w - 8"), rowsTop + 8], { size: SIZE.small, ink: "$muted", align: "end" }),
      repeat("votes", group({ key: e("d.party"), children: [
        label("party", e("d.party"), [0, e("scale.y(d.party) + scale.y.bandwidth() / 2")], { strong: true }),
        label("share", e("format(d.share, '.1f')"), [e("box.w - 8"), e("scale.y(d.party) + scale.y.bandwidth() / 2")], { ink: "$ink-2", align: "end" }),
      ] })),
    ] }),
  ],
});

const plot = (): Template => group({
  key: "scene",
  children: [
    head("head", e("phase >= 3 ? 'NODES, NARROWER BOX' : 'NODES, ONE PER ROW'")),
    group({
      key: "area",
      layout: { type: "columns" },
      children: [group({
        key: "boxed",
        size: { w: e("phase >= 3 ? box.w * 0.58 : box.w") },
        scales: { y: bandScale, x: { type: "linear", domain: [0, 35], range: [0, e("box.w - 34")] } },
        children: [
          box("layout-box", 0, rowsTop, e("box.w"), e(`box.h - ${rowsTop}`), { dashed: true, fill: "transparent", r: 4 }),
          label("box-w", e("'box.w = ' + format(box.w, '.0f')"), [e("box.w - 6"), rowsTop + 8], { size: SIZE.small, ink: "$muted", align: "end" }),
          repeat("votes", shape(geom.rect({ x: 0, y: e("scale.y(d.party)"), w: e("phase >= 2 ? scale.x(d.share) : 0"), h: e("scale.y.bandwidth()"), r: 2 }), {
            key: e("d.party"), fill: e("key.color(d.party)"), opacity: e("phase >= 2 ? 1 : 0"),
            semantics: { role: "datum", label: e("key.name(d.party) + ': ' + format(d.share, '.1f')") },
          })),
          repeat("votes", text("", [e("(phase >= 2 ? scale.x(d.share) : 0) + 5"), e("scale.y(d.party) + scale.y.bandwidth() / 2")], {
            key: e("d.party + '-w'"), opacity: e("phase >= 2 ? 1 : 0"),
            number: { value: e("phase >= 2 ? scale.x(d.share) : 0"), format: ".0f" },
            style: { size: SIZE.small, ink: "$muted", baseline: "middle" },
          })),
        ],
      })],
    }),
  ],
});

// Links from each row to its node: the join, keyed by the row.
const links = (x0: string, x1: string): Template => group({
  key: "links",
  scales: { y: bandScale },
  opacity: e("phase >= 2 ? 1 : 0"),
  children: [repeat("votes", line(e("d.party"), e(x0), e("scale.y(d.party) + scale.y.bandwidth() / 2"), e(x1), e("scale.y(d.party) + scale.y.bandwidth() / 2"), { ink: "$grid" }))],
});

const wide = group({
  key: "layout",
  when: e(WIDE),
  layout: { type: "rows", gap: 14, padding: [14, 16, 10, 16] },
  children: [
    group({ key: "content", layout: { type: "columns", gap: 0 }, children: [
      group({ key: "c1", size: { w: 214 }, children: [template(128, true)] }),
      group({ key: "gap", size: { w: 22 } }),
      group({ key: "c2", size: { w: 96 }, children: [table()] }),
      group({ key: "c3", size: { w: 26 }, children: [links("2", "box.w - 4")] }),
      group({ key: "c4", children: [plot()] }),
    ] }),
    narration(2),
  ],
});

const phone = group({
  key: "layout",
  when: e(PHONE),
  layout: { type: "rows", gap: 12, padding: [12, 12, 8, 12] },
  children: [
    group({ key: "top", size: { h: 150 }, children: [template(124, false)] }),
    group({ key: "content", layout: { type: "columns", gap: 0 }, children: [
      group({ key: "c2", size: { w: 84 }, children: [table()] }),
      group({ key: "c3", size: { w: 14 }, children: [links("1", "box.w - 2")] }),
      group({ key: "c4", children: [plot()] }),
    ] }),
    narration(3),
  ],
});

export default doc({
  id: "how-repeat",
  title: "A template times a table",
  description: "A repeat template, the votes table's eight rows, the eight keyed nodes they make, and the same nodes resolved again in a narrower box.",
  size: [680, 340],
  data: { votes: data.values({ party, share }, { key: "party" }) },
  keys: {
    S: { name: "Social Democrats", color: "#e8112d" }, SD: { name: "Sweden Democrats", color: "#dddd00" },
    M: { name: "Moderates", color: "#1b49dd" }, V: { name: "Left", color: "#a01313" }, C: { name: "Centre", color: "#009933" },
    KD: { name: "Christian Democrats", color: "#005ea8" }, MP: { name: "Greens", color: "#83cf39" }, L: { name: "Liberals", color: "#3a8fd6" },
  },
  signals: { phase: signal.num(0) },
  scene: group({ key: "root", children: [wide, phone] }),
  program: story({ steps: [
    step("template", { set: { phase: 0 }, text: "A recipe returns templates: a repeat over a table, with expressions for the properties. No rows yet." }),
    step("rows", { set: { phase: 1 }, text: "The table is typed, columnar and keyed: votes is keyed by party." }),
    step("nodes", { set: { phase: 2 }, text: "Evaluated per row, in the layout box: one node per row, keyed by the row's key, its width from scale.x." }),
    step("narrower", { set: { phase: 3 }, text: "In a narrower box the scale resolves again: same template, same rows, same keys, new widths." }),
  ] }),
});
