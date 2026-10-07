// Maps: the built-in world atlas shaded by a value, and a camera that flies to the countries a
// signal holds — out to travel, in to land. Population in millions, 2024, rounded. Illustrative.
import { data, doc, e, group, motion, signal, step, story } from "@datars/sdk";
import { map } from "@datars/std";
import { PAD, SIZE } from "./_kit";

const C = "IND 1441 CHN 1425 USA 342 IDN 279 PAK 245 NGA 229 BRA 217 BGD 174 RUS 144 MEX 130 ETH 129 JPN 123 PHL 119 EGY 114 COD 106 VNM 100 IRN 90 TUR 87 DEU 84 THA 72 GBR 68 FRA 65 ITA 59 ZAF 61 ESP 48 POL 38 SWE 10 NOR 6 FIN 6 DNK 6 KOR 52 ARG 46 COL 52 CAN 39 AUS 27".split(" ");
const id = C.filter((_, i) => i % 2 === 0), pop = C.filter((_, i) => i % 2 === 1).map(Number);

export default doc({
  id: "overview-maps",
  title: "A camera over the world",
  description: "Countries shaded by population on a world map; the camera flies to Europe, then to East Asia, then back out.",
  size: SIZE,
  data: { world: data.atlas("countries"), people: data.values({ id, pop }, { key: "id" }) },
  signals: { focus: signal.keyset(id) },
  scene: group({ key: "root", layout: { type: "stack", padding: PAD }, children: [
    map({ source: "world", data: "people", key: "id", value: "pop", format: ",.0f", colorType: "piecewise", stops: "#c9dbf7 20 · #8db0ec 60 · #4f7fdc 150 · #2350b8 400 · #0f2f7a 1450",
      camera: { fit: { keys: "=focus" }, padding: 8 } }, { key: "chart" }),
  ] }),
  motion: motion({ duration: 1.8, easing: "cubic-in-out" }),
  program: story({ steps: [
    step("world", { set: { focus: id } }),
    step("europe", { set: { focus: ["GBR", "FRA", "DEU", "ESP", "ITA", "POL", "SWE", "NOR", "FIN"] } }),
    step("asia", { set: { focus: ["JPN", "KOR", "CHN", "VNM", "PHL", "THA"] } }),
  ] }),
});
