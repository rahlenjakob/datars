// bar: one bar per category, from zero to its value. The steps change one parameter at a time;
// every plot has the key "chart" and every bar its fruit, so each bar morphs into its next shape.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, bar } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });

export default doc({
  title: "Fruit sold at a market stall",
  description: "Five fruits as vertical bars, then with value labels, then as horizontal bars.",
  size: [640, 320],
  data: { sales: data.values({ fruit: ["Apples", "Pears", "Plums", "Cherries", "Figs"], kg: [42, 31, 18, 27, 9] }, { key: "fruit" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ data: "sales", x: "fruit", y: "kg", title: "Fruit sold on Saturday (kg)", children: [bar()] }, at("default")),
      plot({ data: "sales", x: "fruit", y: "kg", title: "Fruit sold on Saturday (kg)", children: [bar({ labels: true })] }, at("labels")),
      plot({ data: "sales", x: "kg", y: "fruit", xType: "linear", yType: "band", title: "Fruit sold on Saturday (kg)",
        children: [bar({ labels: true })] }, at("horizontal")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "bar()", text: "Vertical bars: the plot's x is a band scale." }),
      step("labels", { title: "labels: true", text: "Each bar carries its value." }),
      step("horizontal", { title: "yType: \"band\"", text: "Categories on the y axis make the bars horizontal." }),
    ],
  }),
});
