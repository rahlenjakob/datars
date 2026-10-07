// The delivery page's republishing demo: one chart in three versions, as a newsroom would publish
// it — v1 goes out with a typo in its title, v2 fixes the typo, v3 adds a step with the seats.
// The site build publishes each (delivery-v1, -v2, -v3) beside the others with `datars publish`,
// so their chunks share one content-addressed folder: what a republish re-downloads is exactly the
// chunks whose bytes changed. Votes: Swedish Riksdag election 2022, Valmyndigheten, rounded (as the
// votes example); seats: the 349 seats as allocated.
import { doc, data, e, group, motion, signal, step, story, text } from "@datars/sdk";
import { plot, bar, pie } from "@datars/std";

const party = ["S", "SD", "M", "V", "C", "KD", "MP", "L"];
const share = [30.3, 20.5, 19.1, 6.8, 6.7, 5.3, 5.1, 4.6];
const seats = [107, 73, 68, 24, 24, 19, 18, 16];
const keys = {
  S: { name: "Social Democrats", color: "#e8112d" }, SD: { name: "Sweden Democrats", color: "#dddd00" },
  M: { name: "Moderates", color: "#1b49dd" }, V: { name: "Left", color: "#a01313" }, C: { name: "Centre", color: "#009933" },
  KD: { name: "Christian Democrats", color: "#005ea8" }, MP: { name: "Greens", color: "#83cf39" }, L: { name: "Liberals", color: "#3a8fd6" },
};

export function votes(version: 1 | 2 | 3) {
  const title = version === 1 ? "Vote share by pary, Sweden 2022" : "Vote share by party, Sweden 2022";
  const when = (s: string) => e(`shape == ${JSON.stringify(s)}`);
  return doc({
    id: `delivery-v${version}`,
    title: "Vote share by party, Sweden 2022",
    description: `The Swedish general election of 2022: vote share by party, ranked, then as a donut${version === 3 ? ", then the 349 seats" : ""}. Version ${version} of a chart republished on this page.`,
    size: [640, 400],
    data: { votes: data.values(version === 3 ? { party, share, seats } : { party, share }, { key: "party" }) },
    keys,
    tables: { ranked: { from: "votes", ops: [{ op: "sort", by: [["share", "desc"]] }] } },
    signals: { shape: signal.str("ranked") },
    motion: motion({ select: { role: "datum" }, matcher: "by-key" }),
    scene: group({
      key: "root",
      layout: { type: "rows", gap: 6, padding: [14, 16, 10, 10] },
      children: [
        text(version === 3 ? e(`shape == "seats" ? "Seats by party, Riksdag 2022" : ${JSON.stringify(title)}`) : title, [0, 0], { key: "title", size: { h: 24 }, style: { font: "font.title", size: e("box.w < 330 ? 14 : 17"), ink: "$ink", baseline: "top" }, semantics: { role: "title" } }),
        group({
          key: "stage", size: { h: "fill" },
          children: [
            plot({ data: "ranked", x: "share", y: "party", xType: "linear", yType: "band", color: "party", suffix: " %", children: [bar({ labels: true, suffix: " %" })] }, { key: "chart", when: when("ranked") }),
            pie({ data: "votes", value: "share", category: "party", inner: 0.58, total: true }, { key: "chart", when: when("donut") }),
            version === 3 ? plot({ data: "ranked", x: "seats", y: "party", xType: "linear", yType: "band", color: "party", children: [bar({ labels: true, format: "d" })] }, { key: "chart", when: when("seats") }) : null,
          ].filter(Boolean),
        }),
      ],
    }),
    program: story({
      steps: [
        step("ranked", { set: { shape: "ranked" } }),
        step("donut", { set: { shape: "donut" } }),
        ...(version === 3 ? [step("seats", { set: { shape: "seats" } })] : []),
      ],
    }),
  });
}
