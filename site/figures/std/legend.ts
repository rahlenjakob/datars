// legend: a swatch and a name per entry of a colour scale, flowing in rows. A plot draws one
// below itself with `legend: true`; anywhere else, give the colour scale to a group around both.
import { doc, data, e, group, motion, story, step } from "@datars/sdk";
import { legend, plot, stacked, title } from "@datars/std";

const days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const stations = { Harbour: [42, 45, 44, 48, 55, 80, 76], "Old Town": [30, 32, 31, 35, 41, 62, 58], University: [58, 61, 60, 57, 44, 18, 15] };
const rows = Object.entries(stations).flatMap(([station, n]) => n.map((rides, i) => ({ station, day: days[i], rides })));
const colors = { color: { type: "categorical" as const, domain: { data: "rides", field: "station" }, range: "$categorical" } };

export default doc({
  title: "Bike rentals by station",
  description: "A plot's own legend under stacked bars, then the same legend recipe, with a title, placed under the chart title on a colour scale shared with the plot.",
  size: [640, 320],
  data: { rides: data.values(rows, { key: ["station", "day"] }) },
  motion: motion({ select: { role: "datum" }, matcher: "by-key" }), // bars pair across the two layouts
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ data: "rides", x: "day", y: "rides", color: "station", title: "Bike rentals per day", legend: true,
        children: [stacked()] }, { key: "chart", when: e('state == "default"') }),
      group({ key: "chart", when: e('state == "placed"'), scales: colors, layout: { type: "rows", gap: 8 }, children: [
        title({ text: "Bike rentals per day" }, { key: "title", size: { h: "auto" } }),
        legend({ scale: "color", title: "Station" }, { key: "legend", size: { h: "auto" } }),
        plot({ data: "rides", x: "day", y: "rides", children: [stacked({ color: "station" })] }, { key: "plot" }),
      ] }),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "plot({ legend: true })", text: "The plot draws legend() for its colour field, below the chart." }),
      step("placed", { title: "legend({ scale: \"color\", title })", text: "Placed yourself: here under the title, with a title of its own, on a colour scale it shares with the plot." }),
    ],
  }),
});
