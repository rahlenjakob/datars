// Animation: a fictional assembly's seats gather into a bar per party and split into squares —
// keyed (party, seat), so the engine sees each bar as the parent of its units. Illustrative.
import { choreo, data, doc, group, motion, step, story } from "@datars/sdk";
import { bar, hemicycle, plot, waffle } from "@datars/std";
import { PAD, SIZE, at } from "./_kit";

export default doc({
  id: "overview-animation",
  title: "Seats into bars into squares",
  description: "A hundred seats in a fictional assembly as a hemicycle, then as a bar per party, then as a waffle of squares.",
  size: SIZE,
  data: { seats: data.values({ party: ["Greens", "Labour", "Centre", "Liberal", "Right"], n: [12, 31, 17, 14, 26] }, { key: "party" }) },
  scene: group({ key: "root", layout: { type: "stack", padding: PAD }, children: [
    hemicycle({ data: "seats", value: "n", category: "party", total: false }, at("seats")),
    plot({ data: "seats", x: "party", y: "n", color: "party", children: [bar()] }, at("bars")),
    waffle({ data: "seats", value: "n", category: "party", columns: 20, rows: 5 }, at("squares")),
  ] }),
  motion: motion({ select: { role: "datum" }, duration: 1.4, easing: "cubic-in-out", choreo: choreo.stagger("left", 0.4) }),
  program: story({ steps: [step("seats"), step("bars"), step("squares")] }),
});
