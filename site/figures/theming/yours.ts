// "Make it yours in ten seconds": one chart for each kind of palette a brand colour generates —
// stacked bars on the categorical palette, a heatmap on the sequential ramp, stripes on the
// diverging one. The page hands it a theme built from the reader's one colour (`setTokens()`); the
// engine generates the palettes. Illustrative figures.
import { doc, data, e, group, interactive, text } from "@datars/sdk";
import { cell, plot, stacked, stripes } from "@datars/std";

const u = (i: number, k: number) => Math.abs(Math.sin(i * 12.9898 + k * 78.233) * 43758.5453) % 1; // a fixed hash

const teams = ["Search", "Social", "Email", "Video", "Partners", "Direct"];
const month = ["Jan", "Feb", "Mar", "Apr", "May", "Jun"];
const signups = month.flatMap((m, mi) => teams.map((t, ti) => ({ month: m, source: t, k: Math.round((14 - ti * 1.8) * (1 + mi * 0.09) + u(mi, ti) * 4) })));

const days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const hours = ["6", "9", "12", "15", "18", "21"];
const busy = days.flatMap((d, di) => hours.map((h, hi) => ({ day: d, hour: h, n: Math.round(20 + 60 * Math.sin(((hi + 0.5) / hours.length) * Math.PI) * (di >= 5 ? 0.6 : 1) + u(di, hi) * 18) })));

const week = Array.from({ length: 52 }, (_, i) => i + 1);
const change = week.map((_, i) => Math.round((Math.sin(i / 7) * 0.7 + (i - 26) / 40 + (u(i, 5) - 0.5) * 0.5) * 100) / 100);

const head = (s: string) => text(s, [0, 0], { key: "title", size: { h: "auto" }, style: { font: "font.title", size: "$size.title", ink: "$ink", baseline: "top" } });
const bars = () => plot({ data: "signups", x: "month", y: "k", color: "source", title: "Sign-ups by source (k)", legend: true, children: [stacked({ series: "source" })] }, { key: "bars" });
const heat = () => plot({ data: "busy", x: "hour", y: "day", yType: "band", color: "n", colorType: "sequential", padding: 0, grid: false, title: "Busiest hours", children: [cell({ gap: 2 })] }, { key: "heat" });
const warm = () => group({ key: "stripes", layout: { type: "rows", gap: 8 }, children: [head("Week on week"), stripes({ data: "change", x: "week", value: "change" }, { key: "s" })] });

export default doc({
  id: "theming-yours",
  title: "Your colour, every palette",
  description: "Sign-ups by source as stacked bars on the categorical palette, the busiest hours as a heatmap on the sequential ramp, and week-on-week change as stripes on the diverging ramp.",
  size: [880, 420],
  data: {
    signups: data.values(signups, { key: ["month", "source"] }),
    busy: data.values(busy, { key: ["day", "hour"] }),
    change: data.values({ week, change }, { key: "week" }),
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 18, 12, 12] },
    children: [
      group({ key: "panels", when: e("box.w >= 600"), layout: { type: "columns", gap: 28 }, children: [
        { ...bars(), size: { w: "54%" } },
        group({ key: "side", layout: { type: "rows", gap: 18 }, children: [{ ...heat(), size: { h: "64%" } }, warm()] }),
      ] }),
      group({ key: "panels", when: e("box.w < 600"), layout: { type: "rows", gap: 16 }, children: [
        { ...bars(), size: { h: "44%" } }, { ...heat(), size: { h: "34%" } }, warm(),
      ] }),
    ],
  }),
  program: interactive(),
});
