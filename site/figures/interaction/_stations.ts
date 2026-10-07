// The interaction page's made-up bike-share stations — trips per hour of a weekday at eight
// stations — shared by its scrubber (hours.ts) and its drill-down (drill.ts). Not a figure itself:
// the site build registers only files named like `name.ts`, and this one starts with `_`.

/** Trips around a time of day, falling off with `width` hours. */
const peak = (h: number, at: number, width: number) => Math.exp(-((h - at) ** 2) / (2 * width * width));

// Commuter stations peak at 8 and 17; the park and the beach in the afternoon; the harbour late.
export const STATIONS: { id: string; name: string; trips: (h: number) => number }[] = [
  { id: "central", name: "Central Station", trips: (h) => 70 * peak(h, 8, 1.3) + 80 * peak(h, 17, 1.6) + 12 * peak(h, 13, 4) },
  { id: "tech", name: "Tech Park", trips: (h) => 64 * peak(h, 8.5, 1.1) + 58 * peak(h, 17.5, 1.3) },
  { id: "oldtown", name: "Old Town", trips: (h) => 26 * peak(h, 12, 3) + 30 * peak(h, 19, 2.5) },
  { id: "uni", name: "University", trips: (h) => 40 * peak(h, 9.5, 1.6) + 30 * peak(h, 15, 2.2) },
  { id: "harbour", name: "Harbour", trips: (h) => 22 * peak(h, 16, 3.5) + 30 * peak(h, 21.5, 1.8) },
  { id: "park", name: "City Park", trips: (h) => 46 * peak(h, 14.5, 3) },
  { id: "beach", name: "Beach", trips: (h) => 52 * peak(h, 15.5, 2.4) },
  { id: "hospital", name: "Hospital", trips: (h) => 24 * peak(h, 7, 1.2) + 22 * peak(h, 15, 1.2) + 18 * peak(h, 23, 1.2) },
];

/** Every station's trips in every hour: columns `station` (its id), `hour`, `trips`. */
export function hourly() {
  const station: string[] = [], hour: number[] = [], trips: number[] = [];
  for (const s of STATIONS) for (let h = 0; h < 24; h++) { station.push(s.id); hour.push(h); trips.push(Math.round(s.trips(h) + 2)); }
  return { station, hour, trips };
}

/** The stations as document keys: their names (and no colours of their own). */
export const stationKeys = Object.fromEntries(STATIONS.map((s) => [s.id, { name: s.name }]));
