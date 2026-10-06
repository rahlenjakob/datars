// annotate: a callout — text offset from a point, with a connector and a dot — placed with the
// plot's scales. Keyed, one note carries across the steps from point to point.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, line, annotate } from "@datars/std";

const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const high = [10, 12, 16, 18, 22, 28, 32, 31, 26, 19, 13, 10];
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const frame = { data: "madrid", x: "month", y: "high", xType: "point", yDomain: [0, 45], title: "Madrid, average daily high (°C)" } as const;
const july = { x: e('scale.x("Jul")'), y: e("scale.y(32)"), text: "The hottest month: 32 °C" };

export default doc({
  title: "Madrid's average daily high, annotated",
  description: "A line of Madrid's monthly highs with a callout on the hottest month, then an elbow connector with an arrowhead, then the same note moved to the coldest month.",
  size: [640, 320],
  data: { madrid: data.values({ month: months, high }, { key: "month" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ ...frame, children: [line(), annotate(july, { key: "note" })] }, at("default")),
      plot({ ...frame, children: [line(), annotate({ ...july, dx: 60, connector: "elbow", head: true, dot: false }, { key: "note" })] }, at("connector")),
      plot({ ...frame, children: [line(), annotate({ x: e('scale.x("Jan")'), y: e("scale.y(10)"), text: "The coldest: 10 °C", dx: 50, dy: 30 }, { key: "note" })] }, at("moved")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "annotate({ x, y, text })", text: "A note up and to the right of July's point, with a dot and a line to it." }),
      step("connector", { title: "connector: \"elbow\", head: true", text: "An elbow connector ending in an arrowhead, no dot." }),
      step("moved", { title: "{ key: \"note\" }", text: "Keyed, the same note travels to another point; dy: 30 puts it below." }),
    ],
  }),
});
