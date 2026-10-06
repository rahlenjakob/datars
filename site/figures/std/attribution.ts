// attribution: the credit OpenStreetMap's licence asks for, in the bottom-right corner of its box and
// linked to the licence page. Placed outside the map's view, it stays put while the camera moves.
import { doc, data, e, group, motion, signal, story, step, view } from "@datars/sdk";
import { attribution, basemap } from "@datars/std";

const sweden = [12.5, 56.3, 19.5, 60.4], stockholm = [18.02, 59.308, 18.12, 59.343];

export default doc({
  title: "A basemap and its credit",
  description: "A basemap of southern Sweden with its credit in the corner, which stays put as the camera flies to Stockholm, then with a text of its own.",
  size: [640, 320],
  data: { tiles: data.tiles("../../../assets/tiles/descent.pmtiles") },
  signals: { bounds: signal.keyset(sweden.map(String)) },
  motion: motion({ select: { kind: "view" }, duration: 2.4, easing: "cubic-in-out" }),
  scene: group({
    key: "root",
    children: [
      view({
        key: "map",
        coord: { type: "geo", projection: "web-mercator", fit: { bbox: [-180, -85.0511, 180, 85.0511] }, padding: 0 },
        camera: { fit: { geo: "=bounds" }, padding: 0 },
        children: [basemap({ source: "tiles" })],
      }),
      attribution({}, { key: "credit", when: e('state != "text"') }),
      attribution({ text: "Map data © OpenStreetMap contributors" }, { key: "credit", when: e('state == "text"') }),
    ],
  }),
  program: story({
    steps: [
      step("default", { set: { bounds: sweden }, title: "attribution({})", text: "© OpenStreetMap contributors, Natural Earth — legible on the map, linked to the licence." }),
      step("camera", { set: { bounds: stockholm }, title: "Outside the view", text: "A sibling of the view, not a child: the camera flies, the credit stays." }),
      step("text", { set: { bounds: stockholm }, title: "text: \"Map data © …\"", text: "Your own wording, for archives with other sources." }),
    ],
  }),
});
