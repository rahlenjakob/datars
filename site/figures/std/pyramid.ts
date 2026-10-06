// pyramid: a population by age, one sex growing left and the other right from the ages up the
// middle — both on one scale. A fictional town of about 24,000; rows keyed (age, sex).
import { doc, data, e, group, story, step } from "@datars/sdk";
import { pyramid } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const ages = ["0–9", "10–19", "20–29", "30–39", "40–49", "50–59", "60–69", "70–79", "80+"];
const men = [1310, 1420, 1650, 1580, 1490, 1520, 1300, 950, 420];
const women = [1250, 1350, 1560, 1540, 1470, 1510, 1360, 1110, 720];
const rows = ages.flatMap((age, i) => [{ age, sex: "Men", people: men[i] }, { age, sex: "Women", people: women[i] }]);
const frame = { data: "town", y: "age", side: "sex", value: "people", title: "Residents by age" } as const;

export default doc({
  title: "A town's residents by age and sex",
  description: "A fictional town's residents in ten-year age groups as a population pyramid, then with the counts at the bars' ends, then with the sides swapped.",
  size: [640, 360],
  data: { town: data.values(rows, { key: ["age", "sex"] }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      pyramid({ ...frame }, at("default")),
      pyramid({ ...frame, labels: true, format: ",d" }, at("labels")),
      pyramid({ ...frame, labels: true, format: ",d", left: "Women" }, at("swap")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "pyramid()", text: "Men to the left, women to the right, the youngest at the bottom: past 70, women outnumber men." }),
      step("labels", { title: "labels: true, format: \",d\"", text: "Each group's count at the end of its bar." }),
      step("swap", { title: "left: \"Women\"", text: "Which side goes left is a parameter." }),
    ],
  }),
});
