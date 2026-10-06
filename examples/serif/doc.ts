// A newsroom look in a serif: Newsreader (OFL) from font files in assets/fonts. The theme's font
// tokens say where each face comes from (`font.file`, or `font.google` for a Google Fonts family);
// the build step subsets them to the chart's text and ships them in the bundle, so every runtime
// draws the same glyphs — none supplies fonts of its own.
// River lengths are approximate (sources differ by a few per cent).
import { doc, data, e, font, group, motion, signal, story, step, theme } from "@datars/sdk";
import { plot, bar } from "@datars/std";

const serif = (weight: number) => font.file("Newsreader", `../../assets/fonts/Newsreader-${weight >= 600 ? "SemiBold" : "Regular"}.ttf`, { weight });

const river = ["Volga", "Danube", "Ural", "Dnieper", "Don", "Pechora", "Kama", "Oka", "Dniester", "Rhine"];
const km = [3530, 2850, 2428, 2290, 1870, 1809, 1805, 1500, 1352, 1233];
const mi = km.map((k) => Math.round(k * 0.621371));

export default doc({
  id: "serif",
  title: "Europe’s longest rivers",
  size: [720, 440],
  theme: theme({
    name: "newsroom",
    extends: "datars/neutral",
    tokens: {
      paper: "#fbf8f1",
      ink: "#1f1c17",
      accent: "#8c2f1b",
      "font.body": serif(400),
      "font.number": serif(400),
      "font.strong": serif(600),
      "font.title": serif(600),
      "size.title": 20,
      "size.label": 12,
    },
  }),
  data: { rivers: data.values({ river, km, mi }, { key: "river" }) },
  signals: { unit: signal.str("km") },
  motion: motion({ select: { role: "datum" }, matcher: "by-key" }),
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ data: "rivers", x: "km", y: "river", xType: "linear", yType: "band", title: "Europe’s longest rivers — length in kilometres", format: ",.0f",
        children: [bar({ labels: true, format: ",.0f", fill: "$accent" })] }, { key: "chart", when: e('unit == "km"') }),
      plot({ data: "rivers", x: "mi", y: "river", xType: "linear", yType: "band", title: "…and in miles, for the flow of transatlantic readers", format: ",.0f",
        children: [bar({ labels: true, format: ",.0f", fill: "$accent" })] }, { key: "chart", when: e('unit == "mi"') }),
    ],
  }),
  program: story({
    steps: [
      step("km", { set: { unit: "km" }, text: "The Volga is Europe’s longest river." }),
      step("miles", { set: { unit: "mi" }, text: "The same lengths in miles." }),
    ],
  }),
});
