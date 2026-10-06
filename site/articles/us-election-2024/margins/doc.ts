// Margins.
// Every state, shaded by its margin: deeper red for bigger Trump wins, deeper blue for Harris. The
// scale is pinned at zero. The "blue wall" of Pennsylvania, Michigan and Wisconsin, which Joe Biden
// won back in 2020, went to Trump — each by under two points. The Sun Belt: Arizona, Nevada, Georgia
// and North Carolina, all to Trump. Where Harris won: the West Coast, the Northeast, and a band
// through Illinois, Minnesota, Colorado and New Mexico — twenty states and DC.
// Sources and rounding: the article's notes on the data.
import { doc, data, e, group, motion, signal, step, story } from "@datars/sdk";
import { annotate, map, title } from "@datars/std";
import { civic } from "../theme";

/** A lon/lat box [west, north, east, south] as a camera fits it: its projected corners. */
const geoBox = ([w, n, east, s]: number[]) => [
  e(`geo.x(${w}, ${n})`), e(`geo.y(${w}, ${n})`), e(`geo.x(${east}, ${s})`), e(`geo.y(${east}, ${s})`),
];

const margin = {
  label: [
    "US-AL", "US-AK", "US-AZ", "US-AR", "US-CA", "US-CO", "US-CT", "US-DE", "US-DC", "US-FL", "US-GA", "US-HI", "US-ID",
    "US-IL", "US-IN", "US-IA", "US-KS", "US-KY", "US-LA", "US-ME", "US-MD", "US-MA", "US-MI", "US-MN", "US-MS", "US-MO",
    "US-MT", "US-NE", "US-NV", "US-NH", "US-NJ", "US-NM", "US-NY", "US-NC", "US-ND", "US-OH", "US-OK", "US-OR", "US-PA",
    "US-RI", "US-SC", "US-SD", "US-TN", "US-TX", "US-UT", "US-VT", "US-VA", "US-WA", "US-WV", "US-WI", "US-WY",
  ],
  value: [
    30.6, 13.8, 5.5, 30.6, -20.1, -11, -14.5, -14.7, -83.8, 13.1, 2.2, -23.1, 36.5, -11, 19, 13.3, 16.1, 30.6, 22, -6.9,
    -28.8, -25.3, 1.4, -4.3, 22.9, 18.4, 19.9, 20.6, 3.1, -2.8, -5.9, -6, -12.7, 3.2, 36.8, 11.2, 34.3, -14.4, 1.7,
    -13.9, 17.9, 29.2, 29.7, 13.7, 21.6, -31.8, -5.8, -18.3, 41.9, 0.9, 46.2,
  ],
};

const charts = [
  { scene: "all", title: "Margin by state, 2024 (Trump − Harris, points)", box: [-125, 50, -66, 24], notes: [] },
  {
    scene: "blue-wall",
    title: "The 'blue wall' falls",
    box: [-93, 48, -69.5, 38],
    notes: [
      { text: "Pennsylvania +1.7", admin1: "US-PA" },
      { text: "Michigan +1.4", admin1: "US-MI" },
      { text: "Wisconsin +0.9", admin1: "US-WI" },
    ],
    selected: "focus",
  },
  {
    scene: "sun-belt",
    title: "The Sun Belt",
    box: [-121, 39.5, -74, 29],
    notes: [
      { text: "Arizona +5.5", admin1: "US-AZ" },
      { text: "Nevada +3.1", admin1: "US-NV" },
      { text: "Georgia +2.2", admin1: "US-GA" },
      { text: "North Carolina +3.2", admin1: "US-NC" },
    ],
    selected: "focus",
  },
  { scene: "coasts", title: "Where Harris won", box: [-125, 50, -66, 24], notes: [], selected: "focus" },
];

export default doc({
  id: "us-election-2024/margins",
  title: "Margins",
  size: [1000, 620],
  theme: civic,
  data: {
    "geo:admin1": data.url("admin1.geojson", { id: "iso_3166_2" }),
    "geo:countries": data.atlas("countries"),
    margin: data.values(margin, { key: "label" }),
  },
  signals: { focus: signal.keyset(), scene: signal.str("all") },
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
      ...charts.map((s) => group({
        key: "chart",
        when: e(`scene == "${s.scene}"`),
        layout: { type: "rows", gap: 8 },
        children: [
          title({ text: s.title }, { key: "title", size: { h: "auto" } }),
          map({
            source: "geo:admin1",
            data: "margin",
            key: "label",
            value: "value",
            colorType: "piecewise",
            backdrop: false,
            camera: { fit: { bbox: geoBox(s.box) }, padding: 0 },
            format: ",.1f",
            label: e(
              'd.value == null ? `${(d.name ?? d.id)}` : (`${(d.name ?? d.id)}: ${"" + format(d.value, ",.1f") + ""} pts (Trump − Harris)`)',
            ),
            legend: true,
            padding: 0,
            projection: "web-mercator",
            stops: "#1e3a8a -30 · #93c5fd -5 · #f5f5f4 0 · #fca5a5 5 · #7f1d1d 30",
            under: "geo:countries",
            selected: s.selected,
            children: [
              ...s.notes.map((n) => annotate({
                x: e(`geo.cx("geo:admin1", "${n.admin1}")`),
                y: e(`geo.cy("geo:admin1", "${n.admin1}")`),
                text: n.text,
                connector: "none",
                dot: true,
                dx: 0,
                dy: -10,
                head: false,
                pin: true,
              })),
            ],
          }, { key: "body" }),
        ],
      })),
    ],
  }),
  motion: motion(
    { select: { role: "datum" }, matcher: "by-key" },
    { select: { role: "region" }, matcher: "by-key" },
    { select: { kind: "instance" }, matcher: "by-key" },
  ),
  program: story({
    steps: [
      step("all", { set: { focus: [], scene: "all" } }),
      step("blue-wall", { set: { focus: ["US-PA", "US-MI", "US-WI"], scene: "blue-wall" } }),
      step("sun-belt", { set: { focus: ["US-GA", "US-NC", "US-AZ", "US-NV"], scene: "sun-belt" } }),
      step("coasts", {
        set: {
          focus: [
            "US-CA", "US-WA", "US-OR", "US-NY", "US-MA", "US-IL", "US-MD", "US-NJ", "US-VA", "US-CO", "US-NM", "US-MN",
            "US-NH", "US-ME", "US-VT", "US-CT", "US-RI", "US-DE", "US-DC", "US-HI",
          ],
          scene: "coasts",
        },
      }),
    ],
    drivers: [{ scroll: "trigger" }],
  }),
});
