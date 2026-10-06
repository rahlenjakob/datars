// segmented: one of a few options side by side — here the city a temperature chart shows. Click an
// option, or step: the highlight slides and the line morphs to the new city.
import { doc, data, e, group, op, signal, story, step } from "@datars/sdk";
import { plot, line, segmented } from "@datars/std";

const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const temp = { oslo: [-4, -4, 0, 5, 11, 15, 17, 16, 11, 6, 1, -3], madrid: [6, 8, 11, 13, 17, 23, 26, 26, 21, 15, 10, 7], cairo: [14, 15, 18, 22, 25, 28, 29, 29, 27, 24, 19, 15] };

export default doc({
  title: "Average temperature (°C)",
  description: "A segmented control picks the city; the line morphs to it.",
  size: [640, 320],
  data: { t: data.values({ month: months, oslo: temp.oslo, madrid: temp.madrid, cairo: temp.cairo }, { key: "month" }) },
  signals: { city: signal.str("oslo") },
  // The chosen city's column, as `temp`.
  tables: { shown: { from: "t", ops: [op.derive("temp", e('city == "oslo" ? d.oslo : city == "madrid" ? d.madrid : d.cairo'))] } },
  scene: group({ key: "root", layout: { type: "rows", gap: 10, padding: [16, 20, 12, 12] }, children: [
    segmented({ signal: "city", options: ["oslo", "madrid", "cairo"], labels: ["Oslo", "Madrid", "Cairo"] }, { key: "city", size: { w: 300, h: 32 } }),
    plot({ data: "shown", x: "month", y: "temp", yDomain: [-10, 32], children: [line({ points: true })] }, { key: "chart" }),
  ] }),
  program: story({ steps: [
    step("oslo", { set: { city: "oslo" }, title: "city = oslo" }),
    step("madrid", { set: { city: "madrid" }, title: "city = madrid" }),
    step("cairo", { set: { city: "cairo" }, title: "city = cairo" }),
  ] }),
});
