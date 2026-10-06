// The motion playground (site/pages/features/animation.html): 24 countries, five layouts — bars,
// a map of where they are, a sunflower spiral, a grid by continent and the top eight. Every mark is
// keyed by its country, so each change of layout is one planned transition; the page swaps in the
// reader's motion rule (`setDocument`) and replays it. A custom scene, no recipes: what the engine
// does with it is all the motion rule's doing.
import { doc, motion, signal, step, story } from "@datars/sdk";
import { TITLES, labelRule, peopleData, peopleScene } from "./_people";

export default doc({
  id: "motion-playground",
  title: "Twenty-four countries, five layouts",
  description: "The 24 most populous countries as ranked bars, as circles where they are on a map, as a sunflower spiral with the largest at the centre, as tiles grouped by continent, and as the top eight with names and values.",
  size: [720, 420],
  data: peopleData,
  signals: { layout: signal.str("bars") },
  scene: peopleScene,
  // The page replaces the first rule with the reader's; the labels' rule stays.
  motion: motion({ select: { role: "datum" }, duration: 1.2, easing: "cubic-in-out" }, labelRule),
  program: story({ steps: Object.keys(TITLES).map((name) => step(name, { set: { layout: name } })) }),
});
