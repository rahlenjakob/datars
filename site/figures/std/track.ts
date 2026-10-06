// track: a path over time, drawn up to a data time held in a signal, with a head at its tip. When
// the signal moves on, the path grows along itself. A fictional hurricane's positions every 12 hours.
import { doc, data, e, group, signal, story, step } from "@datars/sdk";
import { map, track } from "@datars/std";

const lon = [-24, -26, -28.2, -30.5, -33, -35.6, -38.3, -41, -43.8, -46.5, -49.2, -51.8, -54.3, -56.6, -58.7, -60.5, -62, -63.1, -63.8, -64, -63.6, -62.5, -60.6];
const lat = [12.5, 12.8, 13.1, 13.4, 13.8, 14.2, 14.7, 15.3, 15.9, 16.6, 17.4, 18.3, 19.3, 20.4, 21.7, 23.1, 24.7, 26.4, 28.2, 30.1, 32, 34, 36];
const storm = { data: "storm", time: "t", lon: "lon", lat: "lat", until: e("day") };
const chart = (future: boolean, when: string) =>
  map({ source: "world", fit: { bbox: [-82, 8, -18, 42] }, children: [track({ ...storm, future })] }, { key: "chart", when: e(when) });

export default doc({
  title: "A hurricane's track",
  description: "A storm's track drawn up to day 7, then with the rest of its path shown faintly, then run on to day 11.",
  size: [640, 360],
  data: {
    world: data.atlas("countries"),
    storm: data.values({ t: lon.map((_, i) => i / 2), lon, lat }, { key: "t" }),
  },
  signals: { day: signal.num(7) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [12, 12, 12, 12] },
    children: [chart(false, 'state == "default"'), chart(true, 'state != "default"')],
  }),
  program: story({
    steps: [
      step("default", { set: { day: 7 }, title: "track({ time, lon, lat, until: e(\"day\") })", text: "Drawn up to day 7 (the signal day), with a head at the tip." }),
      step("future", { set: { day: 7 }, title: "future: true", text: "The whole path faintly underneath: where it will go." }),
      step("later", { set: { day: 11 }, title: "day: 11", text: "The signal moves on and the path grows along itself." }),
    ],
  }),
});
