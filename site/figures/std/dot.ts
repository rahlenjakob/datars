// dot: one dot per category at its value — a dot plot, lighter than bars when zero isn't the point.
// Hours of sunshine a year in eight European capitals (rounded climate normals).
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, dot } from "@datars/std";

const cities = ["Reykjavik", "Oslo", "Stockholm", "Berlin", "Paris", "Rome", "Madrid", "Athens"];
const hours = [1270, 1670, 1820, 1630, 1660, 2470, 2770, 2770];
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const title = "Hours of sunshine a year";
const zero = false; // a dot needs no baseline: the axis fits the values

export default doc({
  title: "Hours of sunshine in eight European capitals",
  description: "Eight capitals' yearly hours of sunshine as dots, then horizontal, then coloured by region.",
  size: [640, 320],
  data: {
    sun: data.values({ city: cities, hours, region: cities.map((_, i) => (i < 3 ? "North" : i < 5 ? "West" : "South")) }, { key: "city" }),
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ data: "sun", x: "city", y: "hours", title, zero, children: [dot()] }, at("default")),
      plot({ data: "sun", x: "hours", y: "city", xType: "linear", yType: "band", title, zero, children: [dot()] }, at("horizontal")),
      plot({ data: "sun", x: "hours", y: "city", xType: "linear", yType: "band", color: "region", legend: true, title, zero, children: [dot({ r: 8 })] }, at("color")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "dot()", text: "A dot per city at its value, on a band x axis." }),
      step("horizontal", { title: "yType: \"band\"", text: "Categories on the y axis: the usual shape of a dot plot." }),
      step("color", { title: "color: \"region\", r: 8", text: "The plot's colour field colours the dots; r sets their radius." }),
    ],
  }),
});
