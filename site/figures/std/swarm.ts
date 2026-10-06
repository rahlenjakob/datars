// swarm: a beeswarm — every row a dot along one value axis, packed so none overlap. Inside a plot,
// it reads the plot's x scale (and colour scale, with `color`).
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, swarm } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
// 160 finishing times of a 10 km fun run (minutes), from a fixed hash (the same numbers every build).
const u = (i: number, k: number) => Math.abs(Math.sin(i * 12.9898 + k * 78.233) * 43758.5453) % 1;
const runner = Array.from({ length: 160 }, (_, i) => `#${i + 1}`);
const club = runner.map((_, i) => (i % 3 === 0 ? "Club runners" : "First timers"));
const minutes = runner.map((_, i) => Math.round(((i % 3 === 0 ? 49 : 58) + 9 * (u(i, 1) + u(i, 2) + u(i, 3) - 1.5)) * 10) / 10);

export default doc({
  title: "Finishing times at a village fun run",
  description: "160 finishing times as a beeswarm, then coloured by whether the runner belongs to a club, then with bigger dots.",
  size: [640, 320],
  data: { times: data.values({ runner, minutes, club }, { key: "runner" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ data: "times", x: "minutes", xType: "linear", title: "10 km finishing times (minutes)", children: [swarm()] }, at("default")),
      plot({ data: "times", x: "minutes", xType: "linear", color: "club", legend: true, title: "10 km finishing times (minutes)", children: [swarm()] }, at("color")),
      plot({ data: "times", x: "minutes", xType: "linear", color: "club", legend: true, title: "10 km finishing times (minutes)", children: [swarm({ r: 5 })] }, at("bigger")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "swarm()", text: "One dot per runner, pushed up and down until none overlap." }),
      step("color", { title: "color: \"club\"", text: "Dots take the plot's colour scale." }),
      step("bigger", { title: "r: 5", text: "Bigger dots need more room: the swarm grows taller." }),
    ],
  }),
});
