// select: a dropdown for one of many options — here which country the bars highlight. Click it to
// open the list over the chart; choose one, or click anywhere else to close it. Eight options
// in columns of four fit the chart on a phone.
import { doc, data, e, group, signal, story, step } from "@datars/sdk";
import { plot, bar, select } from "@datars/std";

const codes = ["NO", "SE", "FI", "DK", "IS", "EE", "LV", "LT"];
const names = ["Norway", "Sweden", "Finland", "Denmark", "Iceland", "Estonia", "Latvia", "Lithuania"];

export default doc({
  title: "Share of electricity from renewables (%)",
  description: "A select picks a country; its bar is highlighted. The list floats above the chart while it's open.",
  size: [640, 340],
  data: { r: data.values({ code: codes, share: [98, 69, 52, 81, 100, 38, 64, 71] }, { key: "code" }) },
  keys: Object.fromEntries(codes.map((c, i) => [c, { name: names[i] }])),
  signals: { country: signal.str("SE") },
  scene: group({ key: "root", layout: { type: "rows", gap: 10, padding: [16, 20, 12, 12] }, children: [
    select({ signal: "country", options: codes, label: "Highlight", rows: 4 }, { key: "country", size: { w: 220, h: 54 } }),
    plot({ data: "r", x: "code", y: "share", yDomain: [0, 110], children: [bar({ labels: true, fill: e('d.code == country ? "$accent" : "$muted@0.4"') })] }, { key: "chart" }),
  ] }),
  program: story({ steps: [
    step("sweden", { set: { country: "SE" }, title: 'country = "SE"' }),
    step("estonia", { set: { country: "EE" }, title: 'country = "EE"' }),
  ] }),
});
