// The "Comparisons" family on one table: a fictional election, each party's share in two
// elections, every row keyed (party, election). The two dots of a dumbbell become a slope's ends,
// a bump chart's ranks, a pyramid's two sides and a marimekko's segments; then the swing per party
// as lollipops.
import { doc, data, e, group, motion, story, step } from "@datars/sdk";
import { plot, dumbbell, slope, bump, pyramid, marimekko, lollipop } from "@datars/std";

const party = ["Greens", "Labour", "Liberals", "Centre", "Conservatives", "Farmers", "Pirates"];
const last = [8.1, 29.4, 6.2, 5.9, 31.8, 4.4, 2.1];
const now = [11.6, 27.0, 4.1, 6.3, 28.9, 5.0, 4.4];
const rows = party.flatMap((p, i) => [{ party: p, election: "2021", share: last[i] }, { party: p, election: "2025", share: now[i] }]);
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const title = "Share of the vote (%)";

export default doc({
  title: "A fictional election, compared six ways",
  description: "Seven parties' shares in two elections as a dumbbell, a slope chart, a bump chart of ranks, a pyramid, a marimekko, then each party's swing as lollipops.",
  size: [640, 400],
  data: {
    votes: data.values(rows, { key: ["party", "election"] }),
    swing: data.values({ party, points: party.map((_, i) => Math.round((now[i] - last[i]) * 10) / 10) }, { key: "party" }),
  },
  motion: motion({ select: { role: "datum" }, matcher: "by-key", duration: 1.1 }),
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [14, 18, 10, 10] },
    children: [
      plot({ data: "votes", x: "share", y: "party", xType: "linear", yType: "band", color: "election", legend: true, title, children: [dumbbell({ labels: true })] }, at("dumbbell")),
      plot({ data: "votes", x: "election", y: "share", xType: "point", color: "party", axes: "x", grid: false, padding: 0, title, children: [slope()] }, at("slope")),
      bump({ data: "votes", x: "election", y: "share", series: "party", title: "Parties by rank" }, at("bump")),
      pyramid({ data: "votes", y: "party", side: "election", value: "share", labels: true, format: ".1f", title }, at("pyramid")),
      marimekko({ data: "votes", x: "election", series: "party", value: "share", title }, at("marimekko")),
      plot({ data: "swing", x: "points", y: "party", xType: "linear", yType: "band", title: "Swing, 2021 → 2025 (points)",
        children: [lollipop({ labels: true, format: ".1f", fill: e('d.points > 0 ? "$positive" : "$negative"') })] }, at("lollipop")),
    ],
  }),
  program: story({
    steps: [
      step("dumbbell", { title: "dumbbell()", text: "Two dots per party, joined: before and after." }),
      step("slope", { title: "slope()", text: "The same two values as a line from one election to the next." }),
      step("bump", { title: "bump()", text: "Ranks instead of values: who overtook whom." }),
      step("pyramid", { title: "pyramid()", text: "One election to the left, the other to the right." }),
      step("marimekko", { title: "marimekko()", text: "Each election as a column of shares." }),
      step("lollipop", { title: "lollipop()", text: "The swing itself, gains and losses." }),
    ],
  }),
});
