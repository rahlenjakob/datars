// lollipop: a stem from zero and a dot at the value — a bar chart with less ink. Library visits in
// eight towns; rows keyed by town, the keys bars use too.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, lollipop } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const town = ["Northby", "Eastwick", "Southam", "Westford", "Millbrook", "Harwich", "Ashdale", "Kingsley"];
const visits = [4.2, 3.1, 5.6, 2.4, 3.8, 6.3, 1.9, 4.7];
const frame = { data: "libraries", title: "Library visits per resident last year" } as const;

export default doc({
  title: "Library visits in eight towns",
  description: "Library visits per resident in eight towns as lollipops, then turned horizontal, then with value labels.",
  size: [640, 320],
  data: { libraries: data.values({ town, visits }, { key: "town" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ ...frame, x: "town", y: "visits", children: [lollipop()] }, at("default")),
      plot({ ...frame, x: "visits", y: "town", xType: "linear", yType: "band", children: [lollipop()] }, at("horizontal")),
      plot({ ...frame, x: "visits", y: "town", xType: "linear", yType: "band", grid: false, children: [lollipop({ labels: true })] }, at("labels")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "lollipop()", text: "A stem from zero to each town's value, a dot at its end." }),
      step("horizontal", { title: "yType: \"band\"", text: "Towns up the side: long names read without turning." }),
      step("labels", { title: "labels: true", text: "Each value past its dot; the gridlines can go." }),
    ],
  }),
});
