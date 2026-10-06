// An axis rescales and the marks stay on their gridlines: world population from zero, then fitted
// to its range, then zoomed to this century. Gridlines and labels are keyed by their value, so a
// tick moves with the numbers it stands for; the line keeps its points as vertices (by x, as a
// series), and points cut off slide out through the plot's edge while the rest spreads out.
// World population, billions (UN World Population Prospects, rounded).
import { data, doc, e, group, motion, step, story } from "@datars/sdk";
import { line, plot } from "@datars/std";

const year = [1950, 1955, 1960, 1965, 1970, 1975, 1980, 1985, 1990, 1995, 2000, 2005, 2010, 2015, 2020, 2024];
const people = [2.5, 2.77, 3.02, 3.34, 3.7, 4.08, 4.45, 4.87, 5.33, 5.74, 6.15, 6.56, 6.99, 7.43, 7.84, 8.12];
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const chart = (state: string, o: Record<string, unknown>) => plot({
  data: "world", x: "year", xType: "linear", xFormat: "d", y: "people", title: "World population, billions",
  children: [line({ points: true })], ...o,
}, at(state));

export default doc({
  id: "motion-rescale",
  title: "An axis that rescales",
  description: "World population from 1950 to 2024 on an axis from zero, then fitted to the data's range, then zoomed to the years since 2000 — gridlines and points moving together.",
  size: [640, 360],
  data: { world: data.values({ year, people }, { key: "year" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [12, 16, 10, 12] },
    children: [
      chart("zero", { yDomain: [0, 9] }),
      chart("fitted", { zero: false }),
      chart("recent", { zero: false, xDomain: [2000, 2024], yDomain: [6, 8.5], clip: true }),
    ],
  }),
  motion: motion({ duration: 1.4, easing: "cubic-in-out" }),
  program: story({
    steps: [
      step("zero", { title: "From zero", text: "Population tripled since 1950." }),
      step("fitted", { title: "Fitted", text: "The axis rescales; every gridline travels with its value." }),
      step("recent", { title: "Since 2000", text: "Zoomed: earlier points slide out through the edge." }),
    ],
  }),
});
