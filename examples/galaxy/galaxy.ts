// A synthetic barred spiral galaxy as a generated table: every row one star, drawn from the
// galaxy's parts by seeded random numbers. Nothing here is stored: the engine evaluates these
// expressions for each of the rows (the same stars on every platform), and the publish compiler
// ships the indexed result as a point archive instead of this source.
//
// Units are light-years from the galactic centre. The model is a sketch, not astrophysics: a bar
// and bulge of old stars, an exponential disk, two strong and two weak logarithmic arms of young
// stars, a sparse halo, and 160 globular clusters; masses from a power-law initial mass function,
// temperatures and luminosities from main-sequence relations, with red giants among the old.

import { data, e } from "@datars/sdk";

/** A uniform draw in [0, 1) and a normal draw: the `k`-th random number of this star. */
const u = (k: number) => `rand(d.i, ${k})`;
const n = (k: number) => `randn(d.i, ${k})`;

/** Where the galaxy's parts start in the unit interval of the first draw. */
const BULGE = 0.14, HALO = 0.16, CLUSTERS = 0.18, DISK = 0.36; // arms: the rest

export const ARM_PITCH = 0.3; // tan of the pitch angle (≈ 17°): how tightly the arms wind
export const ARM_START = 5200; // ly: where the arms leave the ends of the bar
export const RADIUS = 52_000; // ly: where the disk ends
export const CLUSTER_COUNT = 160;

/** A globular cluster's centre (ly): turned by the golden angle from the last, at a radius from
 * a low-discrepancy sequence, so they spread evenly through the halo. The engine computes the
 * same in `cf`/`cr` below; the document uses it to fly the camera to one. */
export function clusterCentre(c: number): [number, number] {
  const f = c * 0.7548776662 - Math.floor(c * 0.7548776662);
  const r = 6000 + 30000 * Math.pow(f, 1.2);
  return [r * Math.cos(c * 2.399963), r * Math.sin(c * 2.399963)];
}

/** Where each arm starts: the two strong ones at the ends of the bar, the weak ones between. */
export const ARM_ANGLES = [0, Math.PI, Math.PI / 2, (3 * Math.PI) / 2];

/** A point on arm `k` (0–3) at radius `r` (ly). */
export function armPoint(k: number, r: number): [number, number] {
  const t = Math.log(r / ARM_START) / ARM_PITCH + ARM_ANGLES[k];
  return [r * Math.cos(t), r * Math.sin(t)];
}

/** A radius from an exponential disk of scale `h`, truncated at the disk's edge (inverse CDF). */
const disk = (h: number, k: number) => `-${h} * log(1 - ${u(k)} * ${1 - Math.exp(-RADIUS / h)})`;

export function stars(count: number) {
  return data.generate(count, {
    // 0 bar and bulge, 1 halo, 2 globular clusters, 3 disk, 4 arms.
    part: e(`${u(1)} < ${BULGE} ? 0 : ${u(1)} < ${HALO} ? 1 : ${u(1)} < ${CLUSTERS} ? 2 : ${u(1)} < ${DISK} ? 3 : 4`),
    // Arms: two strong (0, 1) from the ends of the bar, and two weak (2, 3) between them.
    arm: e(`${u(2)} < 0.37 ? 0 : ${u(2)} < 0.74 ? 1 : ${u(2)} < 0.87 ? 2 : 3`),
    r: e(`d.part == 1 ? min(${RADIUS * 1.2}, 2500 / sqrt(max(${u(3)}, 0.0016))) : d.part == 3 ? 1800 + ${disk(9500, 3)} : ${ARM_START} + ${disk(11000, 3)}`),
    // Along a logarithmic spiral, spread across the arm (wider near the centre).
    theta: e(`d.part == 4 ? log(d.r / ${ARM_START}) / ${ARM_PITCH} + [${ARM_ANGLES.join(", ")}][d.arm] + ${n(4)} * (0.11 + 700 / d.r) : ${u(4)} * ${2 * Math.PI}`),
    cluster: e(`floor(${u(5)} * ${CLUSTER_COUNT})`),
    cf: e(`d.cluster * 0.7548776662 - floor(d.cluster * 0.7548776662)`),
    cr: e(`6000 + 30000 * pow(d.cf, 1.2)`),
    // The bar: an elongated cloud along the arms' starting points, around a round core.
    bx: e(`${u(6)} < 0.3 ? ${n(7)} * 1300 : ${n(7)} * 3300`),
    by: e(`${u(6)} < 0.3 ? ${n(8)} * 1200 : ${n(8)} * 950`),
    x: e(`round(d.part == 0 ? d.bx : d.part == 2 ? d.cr * cos(d.cluster * 2.399963) + ${n(9)} * (25 + 35 * rand(d.cluster, 93)) : d.r * cos(d.theta), 2)`),
    y: e(`round(d.part == 0 ? d.by : d.part == 2 ? d.cr * sin(d.cluster * 2.399963) + ${n(10)} * (25 + 35 * rand(d.cluster, 93)) : d.r * sin(d.theta), 2)`),
    // Young stars in the arms and a third of the disk; the bar, halo and clusters are old.
    young: e(`d.part == 4 || (d.part == 3 && ${u(11)} < 0.3)`),
    // Salpeter-like masses: 0.3 to 60 M☉ for the young (their faintest are lost in the glare of
    // the arms), 0.1 to 0.95 M☉ for the old (heavier ones have died).
    lo: e(`d.young ? 0.3 : 0.1`),
    mass: e(`d.lo * pow(1 - ${u(12)} * (1 - pow(d.lo / (d.young ? 60 : 0.95), 1.35)), -1 / 1.35)`),
    giant: e(`!d.young && ${u(13)} < 0.07`),
    temp: e(`round((d.giant ? 3500 + 1300 * ${u(14)} : d.mass >= 1 ? 5772 * pow(d.mass, 0.505) : 5772 * pow(d.mass, 0.38)) / 10) * 10`),
    // log10 of the luminosity in suns.
    lum: e(`round(d.giant ? 1.4 + 1.4 * ${u(15)} : clamp(3.5 * log10(d.mass), -3, 5.8), 1)`),
  }, { keep: ["x", "y", "temp", "lum"] });
}
