// timeline: events on a time axis, their labels packed into lanes above and below it so none
// overlap (re-packed for a phone), with eras as shaded spans — eras inside eras stacked in rows,
// under the axis or over it. Milestones of spaceflight.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { timeline } from "@datars/std";

const events = [
  { year: 1957, event: "Sputnik 1 in orbit", kind: "Robotic" },
  { year: 1961, event: "Gagarin orbits Earth", kind: "Crewed" },
  { year: 1969, event: "Apollo 11 on the Moon", kind: "Crewed" },
  { year: 1971, event: "First space station", kind: "Crewed" },
  { year: 1977, event: "Voyagers launched", kind: "Robotic" },
  { year: 1981, event: "First Shuttle flight", kind: "Crewed" },
  { year: 1990, event: "Hubble launched", kind: "Robotic" },
  { year: 1998, event: "ISS assembly begins", kind: "Crewed" },
  { year: 2012, event: "Curiosity lands on Mars", kind: "Robotic" },
  { year: 2021, event: "Webb telescope launched", kind: "Robotic" },
];
const eras = [
  { start: 1955, end: 1975, label: "Space Race" },
  { start: 1981, end: 2011, label: "Space Shuttle" },
];
// Programmes inside the eras: each lands a row past the era it falls in.
const programmes = [
  ...eras,
  { start: 1961, end: 1972, label: "Apollo" },
  { start: 1998, end: 2011, label: "ISS assembly" },
];
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const common = { data: "events", date: "year", label: "event", xType: "linear" as const };

export default doc({
  title: "Milestones of spaceflight",
  description: "Ten milestones of spaceflight from Sputnik to the Webb telescope on a line of years, labels stacked above and below it where they would collide; then with the Space Race and the Space Shuttle era shaded; then with the Apollo programme and ISS assembly in a row under the eras they fall in; then with the eras over the line; then with dots coloured by crewed and robotic missions.",
  size: [640, 300],
  data: {
    events: data.values(events, { key: "year" }),
    eras: data.values(eras, { key: "label" }),
    programmes: data.values(programmes, { key: "label" }),
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      timeline(common, at("default")),
      timeline({ ...common, eras: "eras" }, at("eras")),
      timeline({ ...common, eras: "programmes" }, at("nested")),
      timeline({ ...common, eras: "programmes", eraSide: "above" }, at("above")),
      timeline({ ...common, eras: "programmes", eraSide: "above", color: "kind" }, at("color")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "timeline({ date, label })", text: "A dot per event on a line of years; labels that would collide move to lanes above and below." }),
      step("eras", { title: "eras: \"eras\"", text: "Spans shaded along the line, with their names." }),
      step("nested", { title: "eras inside eras", text: "An era inside another stacks a row past it: Apollo in the Space Race, ISS assembly in the Shuttle era. Hover one for its span." }),
      step("above", { title: "eraSide: \"above\"", text: "The eras over the line; the labels above start past them." }),
      step("color", { title: "color: \"kind\"", text: "Dots coloured by crewed or robotic mission." }),
    ],
  }),
});
