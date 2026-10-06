// ridgeline: a density curve per row of a band axis, rising into the row above — a year of daily
// temperatures, month by month, in the space of one chart.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, ridgeline } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
// Daily highs in a northern city: Oslo's monthly normals plus a fixed hash (the same every build).
const u = (i: number, k: number) => Math.abs(Math.sin(i * 12.9898 + k * 78.233) * 43758.5453) % 1;
const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const normal = [-1.5, -0.8, 3.5, 9.4, 15.6, 19.7, 21.5, 20.1, 15.3, 9.1, 3.6, 0.2];
const day = Array.from({ length: 360 }, (_, i) => i + 1);
const month = day.map((d) => months[Math.floor((d - 1) / 30)]);
const high = day.map((d, i) => Math.round((normal[Math.floor((d - 1) / 30)] + 5.5 * (u(i, 1) + u(i, 2) + u(i, 3) - 1.5)) * 10) / 10);
const frame = { data: "days", x: "high", y: "month", xType: "linear", yType: "band", title: "Daily high temperature by month (°C)", padding: 0 } as const;

export default doc({
  title: "A year of daily highs, month by month",
  description: "360 daily high temperatures as a ridgeline per month, then with ridges overlapping less, then smoothed with a wider kernel.",
  size: [640, 400],
  data: { days: data.values({ day, month, high }, { key: "day" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ ...frame, children: [ridgeline()] }, at("default")),
      plot({ ...frame, children: [ridgeline({ overlap: 0.9 })] }, at("overlap")),
      plot({ ...frame, children: [ridgeline({ bandwidth: 2.5 })] }, at("smooth")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "ridgeline()", text: "One curve per month on a shared temperature axis: summer's are narrow, spring's wide." }),
      step("overlap", { title: "overlap: 0.9", text: "The tallest ridge now stays inside its own row." }),
      step("smooth", { title: "bandwidth: 2.5", text: "A wider kernel smooths each month into a gentler hill." }),
    ],
  }),
});
