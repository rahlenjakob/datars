// The distributions half of "Distributions and time" on one table: 240 commutes by mode, each row
// keyed by person. Every row as a dot, then summarised per mode — box, violin, ridge — binned, and
// as a mean with an interval. (The numbers are made up, from a seeded formula: the same on every run.)
import { doc, data, e, group, motion, story, step } from "@datars/sdk";
import { plot, swarm, boxplot, violin, ridgeline, histogram, errorBars } from "@datars/std";

const u = (i: number, k: number) => Math.abs(Math.sin(i * 12.9898 + k * 78.233) * 43758.5453) % 1;
const modes = ["Bike", "Car", "Bus", "Train"];
const centre: Record<string, number> = { Bike: 17, Car: 26, Bus: 34, Train: 44 };
const person = Array.from({ length: 240 }, (_, i) => `#${i + 1}`);
const mode = person.map((_, i) => modes[i % 4]);
const minutes = person.map((_, i) => Math.max(4, Math.round(centre[mode[i]] + (mode[i] === "Bike" ? 6 : 12) * (u(i, 1) + u(i, 2) + u(i, 3) - 1.5))));
// Each mode's mean and its 95 % interval (mean ± 1.96 standard errors), for the error bars.
const stats = modes.map((m) => {
  const v = minutes.filter((_, i) => mode[i] === m);
  const mean = v.reduce((s, x) => s + x, 0) / v.length;
  const sd = Math.sqrt(v.reduce((s, x) => s + (x - mean) ** 2, 0) / (v.length - 1));
  const half = (1.96 * sd) / Math.sqrt(v.length);
  return { mode: m, mean: Math.round(mean * 10) / 10, lo: Math.round((mean - half) * 10) / 10, hi: Math.round((mean + half) * 10) / 10 };
});

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const title = "Time to get to work (minutes)";
const across = { data: "commutes", x: "minutes", y: "mode", xType: "linear", yType: "band", color: "mode", title } as const;

export default doc({
  title: "240 commutes, six ways",
  description: "Commuting times of 240 people by bike, car, bus and train: every person as a dot, then a box plot, violins, ridgelines and a histogram per mode, then each mode's mean with its 95 % interval.",
  size: [640, 400],
  data: {
    commutes: data.values({ person, mode, minutes }, { key: "person" }),
    means: data.values(stats, { key: "mode" }),
  },
  motion: motion(
    { select: { role: "datum" }, matcher: "by-key", duration: 1.1 },
    // A mode's box, violin and ridge are one shape each but keyed inside their recipes: they pair
    // by where they are instead.
    { when: { from: "boxplot", to: "violin" }, select: { role: "datum" }, matcher: "nearest" },
    { when: { from: "violin", to: "ridgeline" }, select: { role: "datum" }, matcher: "nearest" },
  ),
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [14, 18, 10, 10] },
    children: [
      plot({ data: "commutes", x: "minutes", xType: "linear", color: "mode", legend: true, title, children: [swarm({ r: 4 })] }, at("swarm")),
      plot({ ...across, children: [boxplot()] }, at("boxplot")),
      plot({ ...across, children: [violin()] }, at("violin")),
      plot({ ...across, padding: 0, children: [ridgeline()] }, at("ridgeline")),
      plot({ data: "commutes", x: "minutes", xType: "linear", color: "mode", legend: true, title, yLabel: "People", children: [histogram({ step: 4 })] }, at("histogram")),
      plot({ data: "means", x: "mean", y: "mode", xType: "linear", yType: "band", color: "mode", title: "Mean time to work, with its 95 % interval (minutes)",
        children: [errorBars({ lo: "lo", hi: "hi", r: 6 })] }, at("intervals")),
    ],
  }),
  program: story({
    steps: [
      step("swarm", { title: "swarm()", text: "Every commute a dot, packed so none overlap." }),
      step("boxplot", { title: "boxplot()", text: "Each mode's middle half, median and whiskers." }),
      step("violin", { title: "violin()", text: "The shape of each mode's times." }),
      step("ridgeline", { title: "ridgeline()", text: "The same shapes as overlapping ridges." }),
      step("histogram", { title: "histogram()", text: "Counted into four-minute bins." }),
      step("intervals", { title: "errorBars()", text: "Each mode's mean, and how sure we are of it." }),
    ],
  }),
});
