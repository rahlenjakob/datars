// slope: each series' first and last value joined, named at both ends. A fictional election's
// swing: seven parties' shares last time and this time; labels that would collide are pushed apart.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, slope } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const party = ["Greens", "Labour", "Liberals", "Centre", "Conservatives", "Farmers", "Pirates"];
const last = [8.1, 29.4, 6.2, 5.9, 31.8, 4.4, 2.1];
const now = [11.6, 27.0, 4.1, 6.3, 28.9, 5.0, 4.4];
const rows = party.flatMap((p, i) => [{ party: p, election: "2021", share: last[i] }, { party: p, election: "2025", share: now[i] }]);
const frame = { data: "votes", x: "election", y: "share", xType: "point", color: "party", axes: "x", grid: false, padding: 0, title: "Share of the vote (%)" } as const;

export default doc({
  title: "A fictional election's swing",
  description: "Seven parties' vote shares in two elections as a slope chart labelled at both ends, then with two parties picked out, then named at the end only.",
  size: [640, 360],
  data: { votes: data.values(rows, { key: ["party", "election"] }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ ...frame, children: [slope()] }, at("default")),
      plot({ ...frame, children: [slope({ highlight: ["Greens", "Pirates"] })] }, at("highlight")),
      plot({ ...frame, children: [slope({ labels: "end", stroke: e('d.end > d.start ? "$positive" : "$negative"') })] }, at("end")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "slope()", text: "A line per party from 2021 to 2025, named with its share at both ends." }),
      step("highlight", { title: "highlight: [\"Greens\", \"Pirates\"]", text: "The two biggest gainers keep their colour; the rest turn grey." }),
      step("end", { title: "labels: \"end\", stroke: …", text: "Names at the end only, lines coloured by whether the party gained." }),
    ],
  }),
});
