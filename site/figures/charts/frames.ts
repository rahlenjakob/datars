// The "Frames and layout" family on one table: three cities' monthly temperatures. A plot builds
// the scales, axes and gridlines; title, legend and grid are recipes of their own; facet repeats
// the chart per city; a card narrates over it, placed where it covers the least data. Every line
// is keyed by its city, so it moves from the plot into its own panel and back.
import { doc, data, e, group, motion, story, step } from "@datars/sdk";
import { plot, line, title as heading, legend, grid, facet, card } from "@datars/std";

const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const normals: Record<string, number[]> = {
  Oslo: [-4.3, -4, -0.2, 4.5, 10.8, 15.2, 16.4, 15.2, 10.8, 6.3, 0.7, -3.1],
  Madrid: [6.3, 7.9, 11.2, 12.9, 16.7, 22.4, 25.6, 25.1, 20.9, 15.1, 9.9, 6.9],
  Cairo: [14, 15.3, 17.7, 21.5, 25, 27.4, 28.3, 28.3, 26.4, 23.6, 19.4, 15.6],
};
const rows = Object.entries(normals).flatMap(([city, t]) => t.map((c, i) => ({ city, month: months[i], c })));
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const chart = { data: "temps", x: "month", y: "c", xType: "point", color: "city", suffix: "°" } as const;
const titled = (state: string, children: unknown[]) => group({ key: "chart", when: e(`state == "${state}"`), layout: { type: "rows", gap: 8 }, children: [
  heading({ text: "Average temperature through the year", subtitle: "Monthly means, °C", source: "Source: climate normals, rounded" }, { key: "heading", size: { h: "auto" } }),
  ...(children as never[]),
] });

export default doc({
  title: "Three cities' temperatures, framed five ways",
  description: "Monthly temperatures in Oslo, Madrid and Cairo as a bare plot, then with a title, subtitle and source and a legend, then with vertical gridlines, then as small multiples, then with a narration card.",
  size: [640, 400],
  data: { temps: data.values(rows, { key: ["city", "month"] }) },
  motion: motion({ select: { role: "datum" }, matcher: "by-key", duration: 1.1 }),
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [14, 18, 10, 10] },
    // The cities' colours, for the legend (the plots make the same scale from their colour field).
    scales: { color: { type: "categorical", domain: { data: "temps", field: "city" }, range: "$categorical" } },
    children: [
      plot({ ...chart, children: [line()] }, at("plot")),
      titled("title", [
        plot({ ...chart, children: [line()] }, { key: "plot" }),
        legend({ scale: "color" }, { key: "legend", size: { h: "auto" } }),
      ]),
      titled("grid", [
        plot({ ...chart, yLabel: "°C", children: [grid({ scale: "x", orient: "vertical" }), line({ labels: true, points: true })] }, { key: "plot" }),
      ]),
      titled("facet", [
        facet({ data: "temps", by: "city", columns: 3, chart: plot({ x: "month", y: "c", xType: "point", color: "city", suffix: "°", children: [line()] }) }, { key: "plot" }),
      ]),
      group({ key: "chart", when: e('state == "card"'), children: [
        plot({ ...chart, title: "Average temperature (°C)", children: [line({ labels: true })] }, { key: "plot" }),
        card({ kicker: "Cairo", title: "Never colder than Oslo's July", text: "Cairo's coolest month averages 14 °C; Oslo's warmest, 16 °C." }, { key: "card" }),
      ] }),
    ],
  }),
  program: story({
    steps: [
      step("plot", { title: "plot()", text: "Scales from the columns, axes sized to their labels, gridlines." }),
      step("title", { title: "title() · legend()", text: "A title, subtitle and source above; a legend below." }),
      step("grid", { title: "grid()", text: "Gridlines the other way, points, names at the line ends." }),
      step("facet", { title: "facet()", text: "The chart once per city, on shared scales." }),
      step("card", { title: "card()", text: "Narration over the chart, where it covers the least." }),
    ],
  }),
});
