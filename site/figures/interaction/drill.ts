// Drill down, and back: a bar enters a program chapter with its station — that station's day,
// hour by hour — and Esc (the `back` event) leaves it. One parameter on the bars, `chapter`, and a
// chapter in the program; no page code. Made-up trips at eight stations, not real data.
import { chapter, data, doc, e, group, op, signal, step, story } from "@datars/sdk";
import { area, bar, line, plot } from "@datars/std";
import { STATIONS, hourly, stationKeys } from "./_stations";

const rows = hourly();
const daily = { station: STATIONS.map((s) => s.id), trips: STATIONS.map((s) => rows.trips.filter((_, i) => rows.station[i] === s.id).reduce((a, b) => a + b, 0)) };

export default doc({
  id: "interaction-drill",
  title: "Stations, then a station's day",
  description: "Trips per weekday at eight bike-share stations; click a station to see its day hour by hour, Esc to go back. Made-up data.",
  size: [640, 420],
  data: { hourly: data.values(rows, { key: ["station", "hour"] }), daily: data.values(daily, { key: "station" }) },
  keys: stationKeys,
  signals: { focus: signal.str(""), scene: signal.str("stations") },
  tables: {
    ranked: { from: "daily", ops: [op.sort(["trips", "desc"])] },
    day: { from: "hourly", ops: [op.filter(e("d.station == focus"))] },
  },
  scene: group({
    key: "root",
    semantics: { role: "group", label: "Stations, then a station's day" },
    layout: { type: "stack", padding: [14, 18, 12, 12] },
    children: [
      plot({ data: "ranked", x: "trips", y: "station", xType: "linear", yType: "band", format: ".0f", title: "Trips on a weekday · click a station",
        children: [bar({ chapter: "station", labels: true, format: ",.0f", fill: "$accent", label: e("`${key.name(d.station)}: ${format(d.trips, ',.0f')} trips`") })] },
        { key: "chart", when: e('scene == "stations"') }),
      plot({ data: "day", x: "hour", y: "trips", xType: "linear", xDomain: [0, 23], yDomain: [0, 90], format: ".0f", xTicks: 8,
        title: e("`${key.name(focus)}, hour by hour · Esc to go back`"),
        children: [area({ opacity: 0.2 }), line({ width: 2, points: true })] },
        { key: "chart", when: e('scene == "day"') }),
    ],
  }),
  program: story({
    steps: [step("stations", { set: { scene: "stations", focus: "" } })],
    chapters: {
      station: chapter("s", [step("day", { set: { scene: "day", focus: "{s}" }, text: "Its trips through a weekday, hour by hour. Esc goes back to the stations." })]),
    },
  }),
});
