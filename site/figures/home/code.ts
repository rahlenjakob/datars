// The home page's "write it once" moment: the reader edits a small doc.ts on the page and this chart
// follows. Its rows are a data slot (the page hands it the reader's numbers with `provideData`), and
// the recipe on screen is a signal (`setSignal("shape", …)`): the four recipes the code can name are
// all here, keyed alike, so a change of recipe or of a number is a keyed morph. The sample is the
// getting-started guide's.
import { data, doc, e, group, interactive, motion, signal, text } from "@datars/sdk";
import { bar, pie, plot, treemap, waffle } from "@datars/std";

const when = (s: string) => e(`shape == ${JSON.stringify(s)}`);

export default doc({
  id: "home-code",
  title: "Sales by region",
  description: "Sales in four regions as bars, a donut, a treemap or a waffle of 100 squares — the numbers and the shape are the reader's, edited in the code beside it.",
  size: [560, 400],
  data: { sales: data.slot("sales", { key: "region", sample: { region: ["North", "South", "East", "West"], sales: [120, 98, 143, 87] } }) },
  signals: { shape: signal.str("bar") },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 6, padding: [14, 16, 10, 10] },
    children: [
      text("Sales by region", [0, 0], { key: "title", size: { h: 26 }, style: { font: "font.title", size: "$size.title", ink: "$ink", baseline: "top" }, semantics: { role: "title" } }),
      group({
        key: "stage", size: { h: "fill" },
        children: [
          plot({ data: "sales", x: "region", y: "sales", color: "region", children: [bar({ labels: true, format: ",.0f", radius: 3 })] }, { key: "chart", when: when("bar") }),
          pie({ data: "sales", value: "sales", category: "region", inner: 0.55, format: ",.0f" }, { key: "chart", when: when("pie") }),
          treemap({ data: "sales", value: "sales", category: "region", format: ",.0f" }, { key: "chart", when: when("treemap") }),
          waffle({ data: "sales", value: "sales", category: "region", gap: 3 }, { key: "chart", when: when("waffle") }),
        ],
      }),
    ],
  }),
  motion: motion({ select: { role: "datum" }, matcher: "by-key", duration: 0.9 }),
  program: interactive(),
});
