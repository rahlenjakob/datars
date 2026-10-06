// Camera flights: one map, a camera that fits the keys a signal holds. A step changes the keys and
// the camera flies — along the van Wijk path, zooming out to travel and in to land, so the reader
// keeps their bearings — from the world to Asia's giants, across to Europe, over to the Americas.
// The map is the playground's 24 countries, shaded by population (millions, 2024, rounded).
import { data, doc, e, group, motion, signal, step, story } from "@datars/sdk";
import { map } from "@datars/std";
import { PEOPLE } from "./_people";

const asia = ["IND", "CHN", "PAK", "BGD", "IDN"];
const europe = ["GBR", "FRA", "DEU"];
const americas = ["USA", "MEX", "BRA"];

export default doc({
  id: "motion-camera",
  title: "A camera that flies",
  description: "The 24 most populous countries shaded by population on a world map; the camera flies to the largest in Asia, then to Western Europe, then to the Americas, and back out to the world.",
  size: [640, 380],
  data: { world: data.atlas("countries"), people: data.values({ id: PEOPLE.code, name: PEOPLE.name, pop: PEOPLE.pop }, { key: "id" }) },
  signals: { focus: signal.keyset(PEOPLE.code) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [8, 8, 8, 8] },
    children: [
      map({
        // Stops spaced for a skewed range: two countries over a billion, most under 150 million.
        source: "world", data: "people", key: "id", value: "pop", format: ",.0f", colorType: "piecewise", stops: "#c9dbf7 60 · #8db0ec 110 · #4f7fdc 220 · #2350b8 400 · #0f2f7a 1450",
        camera: { fit: { keys: "=focus" }, padding: 16 }, label: e("`${d.name}: ${d.pop ?? 'not among the 24'}${d.pop ? ' million' : ''}`"),
      }, { key: "chart" }),
    ],
  }),
  motion: motion({ duration: 2.2, easing: "cubic-in-out" }),
  program: story({
    steps: [
      step("world", { set: { focus: PEOPLE.code }, title: "The world", text: "The 24 most populous countries." }),
      step("asia", { set: { focus: asia }, title: "Asia", text: "India and China: over a third of everyone." }),
      step("europe", { set: { focus: europe }, title: "Europe", text: "Germany, the UK and France." }),
      step("americas", { set: { focus: americas }, title: "The Americas", text: "The United States, Mexico and Brazil." }),
    ],
  }),
});
