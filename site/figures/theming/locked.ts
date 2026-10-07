// Locks: a fictional bank's house theme, compiled into the document, locks its brand colour, its
// paper and its title face. A host app that embeds the chart may restyle the rest with
// `setTokens()`; an override of a locked token is ignored and reported. The page plays the host.
// Illustrative figures.
import { doc, data, font, group, interactive, theme } from "@datars/sdk";
import { line, plot } from "@datars/std";

const skerry = theme({
  name: "skerry-bank",
  extends: "datars/neutral",
  tokens: {
    brand: "#1f3a5f",
    accent: "$brand",
    paper: "#f7f5ef",
    ink: "#14213d",
    categorical: ["$brand", "#d9a441", "#3c9a78"],
    "font.title": font.file("Fraunces", "../../fonts/Fraunces-Bold.ttf", { weight: 700 }),
    "size.title": 18,
    "stroke.line": 2,
    "point.radius": 3.5,
  },
  modes: { dark: { paper: "#0f1724", ink: "#e8e4da", brand: "#8fb3e0", categorical: ["$brand", "#e3b04f", "#3fae86"] } },
  locked: ["brand", "accent", "paper", "font.title"],
});

const year = [2016, 2017, 2018, 2019, 2020, 2021, 2022, 2023, 2024, 2025];
const rows = [["Savings", [3.1, 3.4, 3.6, 3.9, 4.8, 5.2, 5.0, 5.6, 6.1, 6.6]], ["Loans", [4.2, 4.4, 4.7, 4.9, 4.8, 5.1, 5.6, 5.8, 5.9, 6.2]], ["Funds", [1.0, 1.3, 1.5, 1.9, 2.1, 2.8, 2.6, 3.1, 3.7, 4.3]]] as const;

export default doc({
  id: "theming-locked",
  title: "A bank's charts, in its house style",
  description: "Savings, loans and funds held by a fictional bank's customers over ten years, as lines, in a house theme that locks its brand colour, paper and title face.",
  size: [720, 380],
  theme: skerry,
  data: { book: data.values(rows.flatMap(([kind, v]) => v.map((bn, i) => ({ kind, year: year[i], bn }))), { key: ["kind", "year"] }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 18, 12, 12] },
    children: [
      plot({ data: "book", x: "year", y: "bn", xType: "linear", xFormat: "d", color: "kind", title: "What our customers hold (bn)", labelSpace: 72,
        children: [line({ labels: true, points: true })] }, { key: "chart" }),
    ],
  }),
  program: interactive(),
});
