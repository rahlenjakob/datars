// The home page's "Drag" moment: an engine-drawn slider through the year, and the hours of daylight
// in twelve places from Svalbard to Tierra del Fuego, ranked. Nothing is stored but each place's
// latitude: the table computes every day length from the slider's signal with the standard sunrise
// equation (the sun's declination, and its centre 0.833° below the horizon for refraction and its
// disc), so the engine recomputes and re-ranks the bars as the reader drags — keyed by place, so
// each bar slides to its new rank. No host code.
import { data, doc, e, group, interactive, op, signal, text } from "@datars/sdk";
import { bar, plot, slider } from "@datars/std";

const place = ["Longyearbyen", "Tromsø", "Reykjavík", "Stockholm", "London", "New York", "Cairo", "Mumbai", "Singapore", "Johannesburg", "Sydney", "Ushuaia"];
const lat = [78.22, 69.65, 64.15, 59.33, 51.51, 40.71, 30.04, 19.08, 1.35, -26.2, -33.87, -54.8];
const R = "0.017453292519943295"; // degrees to radians
// 1 January 2026, as days since 1970-01-01 (what formatDate reads), less one: day 1 is 1 January.
const EPOCH = 20453;

export default doc({
  id: "home-daylight",
  title: "Hours of daylight through the year",
  description: "Twelve places from Longyearbyen in Svalbard to Ushuaia in Tierra del Fuego, ranked by the hours between sunrise and sunset on the day the slider picks. Computed from each place's latitude with the standard sunrise equation.",
  size: [720, 440],
  data: { places: data.values({ place, lat }, { key: "place" }) },
  signals: { day: signal.num(172) },
  tables: {
    hours: {
      from: "places",
      ops: [
        op.derive("dec", e(`-23.44 * ${R} * cos(2 * 3.141592653589793 * (day + 10) / 365)`)),
        op.derive("c", e(`(sin(-0.833 * ${R}) - sin(d.lat * ${R}) * sin(d.dec)) / (cos(d.lat * ${R}) * cos(d.dec))`)),
        op.derive("hours", e(`round(2 * acos(clamp(d.c, -1, 1)) / ${R} / 15, 1)`)),
        op.sort(["hours", "desc"], ["lat", "desc"]),
      ],
    },
  },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 6, padding: [14, 20, 10, 12] },
    children: [
      text(e(`"Hours of daylight, " + formatDate(${EPOCH} + day, "%-d %B")`), [0, 0], { key: "title", size: { h: 26 }, style: { font: "font.title", size: "$size.title", ink: "$ink", baseline: "top" }, semantics: { role: "title" } }),
      text(e('sizeClass == "phone" ? "Sunrise to sunset, ranked" : "Sunrise to sunset, ranked. Drag through the year: the north and the south trade places."'), [0, 0], { key: "sub", size: { h: 20 }, style: { size: "$size.label", ink: "$muted", baseline: "top" } }),
      slider({ signal: "day", min: 1, max: 365, step: 1, label: "Day of the year, 2026", format: ",.0f" }, { key: "slider", size: { h: 44 } }),
      plot({ data: "hours", x: "hours", y: "place", xType: "linear", yType: "band", xDomain: [0, 24], padding: 0.22, format: ".0f",
        children: [bar({ labels: true, format: ".1f", suffix: " h", fill: e('d.lat > 0 ? "$categorical[0]" : "$categorical[1]"') })] }, { key: "chart" }),
    ],
  }),
  program: interactive(),
});
