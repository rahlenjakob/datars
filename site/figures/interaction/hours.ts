// Scrub a chart with a chart: drag across the day below and the stations above re-rank for that
// hour — each bar keyed by its station, so it slides to its new place. The drag is one line,
// `on: { drag: scrub("hour", …) }` on a shape over the day's plot; a keyboard gets the same as a native range
// input in the accessibility mirror. Made-up trips per hour at eight stations, not real data.
import { data, doc, e, geom, group, interactive, op, scrub, shape, signal } from "@datars/sdk";
import { area, bar, line, plot, rule } from "@datars/std";
import { hourly, stationKeys } from "./_stations";

const { station, hour, trips } = hourly();
const byHour = Array.from({ length: 24 }, (_, h) => trips.filter((_, i) => hour[i] === h).reduce((a, b) => a + b, 0));

export default doc({
  id: "interaction-hours",
  title: "Busiest stations, hour by hour",
  description: "Eight bike-share stations ranked by trips in the hour picked; drag across the day below to pick it. Made-up data.",
  size: [640, 470],
  data: {
    trips: data.values({ station, hour, trips }, { key: ["station", "hour"] }),
    day: data.values({ hour: Array.from({ length: 24 }, (_, h) => h), trips: byHour }, { key: "hour" }),
  },
  keys: stationKeys,
  signals: { hour: signal.num(8) },
  tables: { now: { from: "trips", ops: [op.filter(e("d.hour == hour")), op.sort(["trips", "desc"])] } },
  scene: group({
    key: "root",
    semantics: { role: "group", label: "Busiest stations, hour by hour" },
    layout: { type: "rows", gap: 12, padding: [14, 18, 12, 12] },
    children: [
      plot({ data: "now", x: "trips", y: "station", xType: "linear", yType: "band", xDomain: [0, 90], format: ".0f",
        title: e("`Trips at ${hour < 10 ? '0' : ''}${hour}:00`"),
        children: [bar({ labels: true, format: ".0f", fill: "$accent", label: e("`${key.name(d.station)}: ${d.trips} trips at ${d.hour}:00`") })] }, { key: "ranked" }),
      plot({ data: "day", x: "hour", y: "trips", xType: "linear", xDomain: [0, 23], format: ".0f", title: "All stations · drag across the day", xTicks: 8,
        children: [
          area({ opacity: 0.2 }), line({ width: 1.5 }), rule({ axis: "x", value: e("hour"), dashed: false, ink: "$accent" }),
          // The scrubber: the plot's area, through its own x scale.
          shape(geom.rect({ x: e("scale.x.min()"), y: e("min(scale.y.min(), scale.y.max())"), w: e("scale.x.max() - scale.x.min()"), h: e("abs(scale.y.max() - scale.y.min())") }),
            { key: "scrub", fill: "$accent@0", pickable: true, on: { drag: scrub("hour", { step: 1, min: 0, max: 23 }) }, semantics: { role: "control", label: e("`Hour of the day: ${hour}:00`") } }),
        ] },
        { key: "day", size: { h: 130 } }),
    ],
  }),
  program: interactive(),
});
