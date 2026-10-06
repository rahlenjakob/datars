// map: a choropleth of the built-in countries atlas, joined to a table on the ISO code. The camera
// frames the keys in a signal, so the last step flies to the Baltic Sea.
import { doc, data, e, group, signal, story, step } from "@datars/sdk";
import { map } from "@datars/std";

// Forest cover, % of land area (rounded, about 2021).
const forest: Record<string, number> = {
  FIN: 74, SWE: 69, SVN: 61, MNE: 61, EST: 57, LVA: 55, AUT: 47, BLR: 43, BIH: 43, SVK: 40, MKD: 40, ESP: 37,
  PRT: 36, BGR: 36, LUX: 36, LTU: 35, CZE: 35, HRV: 34, NOR: 33, DEU: 33, FRA: 32, ITA: 32, CHE: 32, SRB: 32,
  POL: 31, ROU: 30, GRC: 30, ALB: 29, BEL: 23, HUN: 23, UKR: 17, DNK: 16, GBR: 13, MDA: 12, IRL: 11, NLD: 11,
};
const baltic = ["SWE", "FIN", "EST", "LVA", "LTU", "DNK"];
const chart = (legend: boolean, when: string) => map({
  source: "world", data: "forest", key: "id", value: "share", format: ".0f", legend,
  camera: { fit: { keys: "=focus" }, padding: 12 }, label: e("`${d.name}: ${d.share ?? 'no data'}% forest`"),
}, { key: "chart", when: e(when) });

export default doc({
  title: "Forest cover in Europe",
  description: "European countries shaded by forest cover, then with a colour legend, then with the camera flown to the countries around the Baltic Sea.",
  size: [640, 400],
  data: { world: data.atlas("countries"), forest: data.values({ id: Object.keys(forest), share: Object.values(forest) }, { key: "id" }) },
  signals: { focus: signal.keyset(Object.keys(forest)) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [12, 12, 12, 12] },
    children: [chart(false, 'state == "default"'), chart(true, 'state != "default"')],
  }),
  program: story({
    steps: [
      step("default", { set: { focus: Object.keys(forest) }, title: "map({ source, data, key, value })", text: "Regions coloured by value on the theme's sequential ramp." }),
      step("legend", { set: { focus: Object.keys(forest) }, title: "legend: true", text: "A colour ramp in the lower-left corner." }),
      step("camera", { set: { focus: baltic }, title: "camera: { fit: { keys } }", text: "The camera flies to the keys a signal holds." }),
    ],
  }),
});
