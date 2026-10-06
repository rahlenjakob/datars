// sunburst: a hierarchy as rings. What fills a laptop's disk, folder by folder; the pieces are keyed
// by folder, so they reshape when the value changes from gigabytes to files.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { sunburst } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
// [folder, parent, GB, files (thousands)] — leaves carry the numbers; folders add them up.
const disk: [string, string | null, number, number][] = [
  ["Disk", null, 0, 0],
  ["Photos", "Disk", 0, 0], ["2023", "Photos", 38, 9], ["2024", "Photos", 52, 12], ["2025", "Photos", 61, 14],
  ["Music", "Disk", 0, 0], ["Albums", "Music", 34, 6], ["Podcasts", "Music", 12, 1],
  ["Documents", "Disk", 0, 0], ["Work", "Documents", 14, 22], ["Taxes", "Documents", 2, 3], ["Letters", "Documents", 1, 4],
  ["Apps", "Disk", 0, 0], ["Editors", "Apps", 18, 40], ["Games", "Apps", 44, 18], ["Tools", "Apps", 6, 25],
  ["System", "Disk", 0, 0], ["Caches", "System", 21, 60], ["Logs", "System", 3, 12],
];

export default doc({
  title: "What fills a laptop's disk",
  description: "A laptop's folders as a sunburst, each ring a level deeper: sized by gigabytes, then by number of files, then without names.",
  size: [640, 380],
  data: {
    disk: data.values({ folder: disk.map((d) => d[0]), parent: disk.map((d) => d[1]), gb: disk.map((d) => d[2]), files: disk.map((d) => d[3]) }, { key: "folder" }),
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [12, 16, 12, 12] },
    children: [
      sunburst({ data: "disk", id: "folder", value: "gb" }, at("default")),
      sunburst({ data: "disk", id: "folder", value: "files" }, at("files")),
      sunburst({ data: "disk", id: "folder", value: "files", labels: false }, at("plain")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "sunburst()", text: "Each folder spans its share of the one around it; the total of the disk (GB) sits in the hole." }),
      step("files", { title: "value: \"files\"", text: "Sized by thousands of files instead: caches and apps grow, photos shrink." }),
      step("plain", { title: "labels: false", text: "No names: every piece still names itself on hover." }),
    ],
  }),
});
