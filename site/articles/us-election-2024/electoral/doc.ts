// Electoral votes.
// Electoral votes by state (circle area ∝ votes, colored by winner), then the national total and the
// 538-elector chamber.
// Sources and rounding: the article's notes on the data.
import { doc, data, e, group, motion, op, signal, step, story, table } from "@datars/sdk";
import { card, hemicycle, map, plot, stacked, symbols, title } from "@datars/std";
import { civic } from "../theme";

/** A lon/lat box [west, north, east, south] as a camera fits it: its projected corners. */
const geoBox = ([w, n, east, s]: number[]) => [
  e(`geo.x(${w}, ${n})`), e(`geo.y(${w}, ${n})`), e(`geo.x(${east}, ${s})`), e(`geo.y(${east}, ${s})`),
];

const ev = {
  label: [
    "US-AL", "US-AK", "US-AZ", "US-AR", "US-CA", "US-CO", "US-CT", "US-DE", "US-DC", "US-FL", "US-GA", "US-HI", "US-ID",
    "US-IL", "US-IN", "US-IA", "US-KS", "US-KY", "US-LA", "US-ME", "US-MD", "US-MA", "US-MI", "US-MN", "US-MS", "US-MO",
    "US-MT", "US-NE", "US-NV", "US-NH", "US-NJ", "US-NM", "US-NY", "US-NC", "US-ND", "US-OH", "US-OK", "US-OR", "US-PA",
    "US-RI", "US-SC", "US-SD", "US-TN", "US-TX", "US-UT", "US-VT", "US-VA", "US-WA", "US-WV", "US-WI", "US-WY",
  ],
  value: [
    9, 3, 11, 6, 54, 10, 7, 3, 3, 30, 16, 4, 4, 19, 11, 6, 6, 8, 8, 4, 10, 11, 15, 10, 6, 10, 4, 5, 6, 4, 14, 5, 28, 16,
    3, 17, 7, 8, 19, 4, 9, 3, 11, 40, 6, 3, 13, 12, 4, 10, 3,
  ],
};

const captions = [
  {
    scene: "circles",
    at: "bottom-left",
    caption: "Size is electoral votes, not land: California's 54 dwarf the empty West.",
    width: 260,
  },
  { scene: "stack", at: "top-right", caption: "electoral votes for Trump; 270 were needed.", width: 230, title: "312" },
  { scene: "chamber", at: "top-left", caption: "Every dot an elector.", width: 200 },
];

export default doc({
  id: "us-election-2024/electoral",
  title: "Electoral votes",
  size: [1000, 600],
  theme: civic,
  data: {
    ev: data.values(ev, { key: "label" }),
    ev_total: data.values({ label: ["Trump", "Harris"], value: [312, 226] }, { key: "label" }),
    "geo:admin1": data.url("admin1.geojson", { id: "iso_3166_2" }),
    "geo:countries": data.atlas("countries"),
  },
  tables: { stack_1: table("ev_total", op.derive("whole", e('""'))) },
  signals: { scene: signal.str("circles") },
  keys: {
    Democratic: { color: "#2563eb" },
    Harris: { color: "#2563eb" },
    Others: { color: "#9aa3ad" },
    Republican: { color: "#c0392b" },
    Trump: { color: "#c0392b" },
    "US-AK": { name: "Alaska", color: "#c0392b" },
    "US-AL": { name: "Alabama", color: "#c0392b" },
    "US-AR": { name: "Arkansas", color: "#c0392b" },
    "US-AZ": { name: "Arizona", color: "#c0392b" },
    "US-CA": { name: "California", color: "#2563eb" },
    "US-CO": { name: "Colorado", color: "#2563eb" },
    "US-CT": { name: "Connecticut", color: "#2563eb" },
    "US-DC": { name: "Washington", color: "#2563eb" },
    "US-DE": { name: "Delaware", color: "#2563eb" },
    "US-FL": { name: "Florida", color: "#c0392b" },
    "US-GA": { name: "Georgia", color: "#c0392b" },
    "US-HI": { name: "Hawaii", color: "#2563eb" },
    "US-IA": { name: "Iowa", color: "#c0392b" },
    "US-ID": { name: "Idaho", color: "#c0392b" },
    "US-IL": { name: "Illinois", color: "#2563eb" },
    "US-IN": { name: "Indiana", color: "#c0392b" },
    "US-KS": { name: "Kansas", color: "#c0392b" },
    "US-KY": { name: "Kentucky", color: "#c0392b" },
    "US-LA": { name: "Louisiana", color: "#c0392b" },
    "US-MA": { name: "Massachusetts", color: "#2563eb" },
    "US-MD": { name: "Maryland", color: "#2563eb" },
    "US-ME": { name: "Maine", color: "#2563eb" },
    "US-MI": { name: "Michigan", color: "#c0392b" },
    "US-MN": { name: "Minnesota", color: "#2563eb" },
    "US-MO": { name: "Missouri", color: "#c0392b" },
    "US-MS": { name: "Mississippi", color: "#c0392b" },
    "US-MT": { name: "Montana", color: "#c0392b" },
    "US-NC": { name: "North Carolina", color: "#c0392b" },
    "US-ND": { name: "North Dakota", color: "#c0392b" },
    "US-NE": { name: "Nebraska", color: "#c0392b" },
    "US-NH": { name: "New Hampshire", color: "#2563eb" },
    "US-NJ": { name: "New Jersey", color: "#2563eb" },
    "US-NM": { name: "New Mexico", color: "#2563eb" },
    "US-NV": { name: "Nevada", color: "#c0392b" },
    "US-NY": { name: "New York", color: "#2563eb" },
    "US-OH": { name: "Ohio", color: "#c0392b" },
    "US-OK": { name: "Oklahoma", color: "#c0392b" },
    "US-OR": { name: "Oregon", color: "#2563eb" },
    "US-PA": { name: "Pennsylvania", color: "#c0392b" },
    "US-RI": { name: "Rhode Island", color: "#2563eb" },
    "US-SC": { name: "South Carolina", color: "#c0392b" },
    "US-SD": { name: "South Dakota", color: "#c0392b" },
    "US-TN": { name: "Tennessee", color: "#c0392b" },
    "US-TX": { name: "Texas", color: "#c0392b" },
    "US-UT": { name: "Utah", color: "#c0392b" },
    "US-VA": { name: "Virginia", color: "#2563eb" },
    "US-VT": { name: "Vermont", color: "#2563eb" },
    "US-WA": { name: "Washington", color: "#2563eb" },
    "US-WI": { name: "Wisconsin", color: "#c0392b" },
    "US-WV": { name: "West Virginia", color: "#c0392b" },
    "US-WY": { name: "Wyoming", color: "#c0392b" },
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      group({
        key: "chart",
        when: e('scene == "circles"'),
        layout: { type: "rows", gap: 8 },
        children: [
          title({ text: "Electoral votes by state, 2024" }, { key: "title", size: { h: "auto" } }),
          map({
            source: "geo:admin1",
            camera: { fit: { bbox: geoBox([-125, 50, -66, 24]) }, padding: 0 },
            format: ",.0f",
            padding: 0,
            projection: "web-mercator",
            under: "geo:countries",
            children: [
              symbols({
                source: "geo:admin1",
                data: "ev",
                key: "label",
                value: "value",
                fill: e("key.color(d.label)"),
                format: ",.0f",
                label: e(
                  'd.value == null ? `${key.name(d.label)}` : (`${key.name(d.label)}: ${"" + format(d.value, ",.0f") + ""} electoral votes`)',
                ),
                max: 42,
              }),
            ],
          }, { key: "body" }),
        ],
      }),
      plot({
        data: "stack_1",
        x: "value",
        y: "whole",
        color: "label",
        xType: "linear",
        yType: "band",
        title: "312 to 226",
        axes: "none",
        grid: false,
        legend: true,
        padding: 0.2,
        xDomain: [0, 1],
        children: [stacked({ offset: "expand", segmentKey: e("d.label"), series: "label" })],
      }, { key: "chart", when: e('scene == "stack"') }),
      group({
        key: "chart",
        when: e('scene == "chamber"'),
        layout: { type: "rows", gap: 8 },
        children: [
          title({ text: "The Electoral College, 538 votes" }, { key: "title", size: { h: "auto" } }),
          hemicycle({ data: "ev_total", value: "value", category: "label" }, { key: "body" }),
        ],
      }),
      ...captions.map((s) => card({ text: s.caption, title: s.title, at: s.at, width: s.width }, {
        key: "caption",
        when: e(`scene == "${s.scene}"`),
      })),
    ],
  }),
  motion: motion(
    { select: { role: "datum" }, matcher: "by-key" },
    { select: { role: "region" }, matcher: "by-key" },
    { select: { kind: "instance" }, matcher: "by-key" },
    { when: { from: "stack", to: "chamber" }, select: { role: "datum" }, matcher: { hierarchy: { partition: "auto" } } },
    {
      when: { from: "stack", to: "chamber" },
      select: { kind: "instance" },
      matcher: { hierarchy: { partition: "auto" } },
    },
  ),
  program: story({
    steps: [
      step("circles", {
        set: { scene: "circles" },
        text: "Size is electoral votes, not land: [California](US-CA)'s 54 dwarf the empty West.",
      }),
      step("stack", { set: { scene: "stack" }, title: "312", text: "electoral votes for Trump; 270 were needed." }),
      step("chamber", { set: { scene: "chamber" }, text: "Every dot an elector." }),
    ],
  }),
});
