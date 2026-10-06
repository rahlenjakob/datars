// The playground's countries as a choreographed story, for the page's scroll to scrub (and the
// reduced-motion comparison to replay): each transition has its own rule, scoped with `when` —
// bars arc to the map largest first, the map spirals into the sunflower rippling from the centre,
// the sunflower packs into tiles in a wave from the left.
import { doc, motion, signal, step, story, choreo, route } from "@datars/sdk";
import { labelRule, peopleData, peopleScene } from "./_people";

const STEPS = ["bars", "map", "spiral", "grid"];

export default doc({
  id: "motion-scroll",
  title: "Twenty-four countries, choreographed",
  description: "The 24 most populous countries as ranked bars, then as circles where they are on a map, then as a sunflower spiral with the largest at the centre, then as tiles grouped by continent.",
  size: [720, 420],
  data: peopleData,
  signals: { layout: signal.str("bars") },
  scene: peopleScene,
  motion: motion(
    { duration: 1.6, easing: "cubic-in-out" },
    { when: { from: "bars", to: "map" }, select: { role: "datum" }, route: route.arc(0.35), choreo: choreo.stagger("value", 0.5) },
    { when: { from: "map", to: "spiral" }, select: { role: "datum" }, route: route.spiral(0.5), choreo: choreo.ripple(0.55) },
    { when: { from: "spiral", to: "grid" }, select: { role: "datum" }, choreo: choreo.wave(0.6, 0) },
    labelRule,
  ),
  // No narration: the chart's own title says where it is (a narration card would sit on the chart).
  program: story({ steps: STEPS.map((name) => step(name, { set: { layout: name } })) }),
});
