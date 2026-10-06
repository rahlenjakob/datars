// area: a filled area from a baseline to y — from zero, or from a second field (`y0`) for a band.
// Here Madrid's average daily high and low temperature, month by month.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, area, line } from "@datars/std";

const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const high = [10, 12, 16, 18, 22, 28, 32, 31, 26, 19, 13, 10];
const low = [3, 4, 6, 8, 12, 17, 19, 19, 16, 11, 6, 3];
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const frame = { data: "madrid", x: "month", y: "high", xType: "point", title: "Madrid, average daily temperature (°C)" } as const;

export default doc({
  title: "Madrid's daily high and low through the year",
  description: "Madrid's average daily high as a filled area, then faint under a line, then as a band from the low to the high.",
  size: [640, 320],
  data: { madrid: data.values({ month: months, high, low }, { key: "month" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ ...frame, children: [area()] }, at("default")),
      plot({ ...frame, children: [area({ opacity: 0.2 }), line({}, { key: "high" })] }, at("opacity")),
      plot({ ...frame, children: [area({ y0: "low", opacity: 0.2 }), line({}, { key: "high" }), line({ y: "low" }, { key: "low" })] }, at("band")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "area()", text: "The daily high, filled down to zero." }),
      step("opacity", { title: "opacity: 0.2", text: "A faint area under a line of the same values." }),
      step("band", { title: "y0: \"low\"", text: "From the low to the high: the day's range as a band." }),
    ],
  }),
});
