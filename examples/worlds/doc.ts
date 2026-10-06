// Eight billion, four ways — a scene no chart library ships: every country on a globe lifts off
// into orbits by continent, falls into a golden-angle spiral by population, and packs into blocks.
// One custom recipe (recipes/worlds.ts) draws all four; each country keeps its key, so its outline morphs from
// coastline to planet to block. Populations: rounded 2024 estimates (millions), approximate.
import { doc, data, e, group, text, signal, story, step, motion, choreo, route } from "@datars/sdk";
import { card } from "@datars/std";
import { worlds, CONTINENTS, type Layout } from "./recipes/worlds";

const ROWS = `
AUS 27 PNG 10 NZL 5.2 FJI 0.9 SLB 0.7 VUT 0.3
BRA 216 COL 52 ARG 46 PER 34 VEN 28 CHL 20 ECU 18 BOL 12 PRY 6.9 URY 3.4 GUY 0.8 SUR 0.6
USA 335 MEX 129 CAN 40 GTM 18 HTI 12 CUB 11 DOM 11 HND 10 NIC 7 SLV 6.3 CRI 5.2 PAN 4.4 PRI 3.2 JAM 2.8 TTO 1.5 BHS 0.4 BLZ 0.4
RUS 144 DEU 84 GBR 68 FRA 68 ITA 59 ESP 48 POL 37 UKR 37 ROU 19 NLD 18 BEL 12 CZE 10.9 SWE 10.5 GRC 10.4 PRT 10.4 HUN 9.6 BLR 9.2 AUT 9.1 CHE 8.9 SRB 6.6 BGR 6.4 DNK 5.9 FIN 5.6 NOR 5.5 SVK 5.4 IRL 5.2 HRV 3.9 BIH 3.2 LTU 2.9 ALB 2.8 MDA 2.5 SVN 2.1 LVA 1.9 MKD 1.8 KOS 1.8 EST 1.4 CYP 1.3 LUX 0.7 MNE 0.6 ISL 0.4
IND 1440 CHN 1410 IDN 280 PAK 245 BGD 173 JPN 124 PHL 117 VNM 100 IRN 90 TUR 86 THA 72 MMR 55 KOR 52 IRQ 46 AFG 42 SAU 37 UZB 36 MYS 35 YEM 35 NPL 31 PRK 26 TWN 23 SYR 23 LKA 22 KAZ 20 KHM 17 JOR 11 AZE 10 ARE 10 TJK 10 ISR 10 LAO 8 KGZ 7 TKM 7 SGP 6 LBN 5.5 PSE 5.4 OMN 5 KWT 4 GEO 4 MNG 3.5 ARM 3 QAT 3 BHR 1.5 TLS 1.4 BTN 0.8
NGA 225 ETH 127 EGY 113 COD 105 TZA 68 ZAF 63 KEN 56 UGA 49 SDN 49 DZA 46 MAR 38 AGO 37 GHA 34 MOZ 34 MDG 31 CIV 29 CMR 29 NER 27 BFA 23 MLI 23 MWI 21 ZMB 21 TCD 18 SOM 18 SEN 18 ZWE 17 GIN 14 RWA 14 BEN 14 BDI 13 TUN 12 SSD 11 TGO 9 SLE 9 LBY 7 CAF 5.7 COG 6 LBR 5.4 MRT 4.9 ERI 3.7 GMB 2.8 NAM 2.6 BWA 2.6 GAB 2.4 LSO 2.3 GNB 2.1 GNQ 1.7 SWZ 1.2 DJI 1.1`;
const id: string[] = [], continent: string[] = [], pop: number[] = [];
ROWS.trim().split("\n").forEach((line, c) => {
  const f = line.trim().split(/\s+/);
  for (let i = 0; i < f.length; i += 2) { id.push(f[i]); continent.push(CONTINENTS[c]); pop.push(Number(f[i + 1])); }
});

export default doc({
  id: "worlds",
  title: "Eight billion, four ways",
  size: [760, 620],
  theme: { use: "datars/neutral", tokens: { paper: "#0b1020", ink: "#eef2ff", "ink-2": "#c7d2fe", muted: "#8b93b8" } },
  data: {
    countries: data.values({ id, continent, pop }, { key: "id" }),
    world: data.atlas("countries"),
  },
  keys: {
    Asia: { color: "#f472b6" }, Africa: { color: "#fbbf24" }, Europe: { color: "#60a5fa" },
    "North America": { color: "#34d399" }, "South America": { color: "#a78bfa" }, Oceania: { color: "#22d3ee" },
  },
  // The globe turns eastward, a revolution every 45 s, while it's on screen.
  signals: { layout: signal.str("globe"), spin: signal.clock(8) },
  motion: motion(
    { duration: 2.2, easing: "cubic-in-out", matcher: "by-path" },
    { when: { from: "globe", to: "orbits" }, select: { role: "datum" }, route: route.spiral(0.35), choreo: choreo.ripple(0.55) },
    { when: { from: "orbits", to: "spiral" }, select: { role: "datum" }, route: route.arc(0.3), choreo: choreo.stagger("value", 0.5) },
    { when: { from: "spiral", to: "blocks" }, select: { role: "datum" }, choreo: choreo.wave(0.5, 0) },
  ),
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 16, 12, 16] },
    children: [
      text("Eight billion, four ways", [0, 0], { key: "title", style: { font: "font.title", size: 20, ink: "$ink", baseline: "top" } }),
      ...(["globe", "orbits", "spiral", "blocks"] as Layout[]).map((layout) => worlds({ data: "countries", source: "world", layout, turn: e("15 - spin") }, { key: "chart", when: e(`layout == "${layout}"`) })),
      card({ title: e("narration.title"), text: e("narration.text"), at: "bottom-left", width: 230 }, { key: "caption" }),
    ],
  }),
  program: story({
    steps: [
      step("globe", { set: { layout: "globe" }, title: "188 countries", text: "Coloured by continent, on a globe." }),
      step("orbits", { set: { layout: "orbits" }, title: "Planets", text: "Each country lifts off as a planet, as large as its population, orbiting in its continent's ring." }),
      step("spiral", { set: { layout: "spiral" }, title: "A golden spiral", text: "Smallest at the centre, the giants on the rim — each country turned by the golden angle from the last." }),
      step("blocks", { set: { layout: "blocks" }, title: "Area is people", text: "Asia alone is more than half of everyone." }),
    ],
  }),
});

