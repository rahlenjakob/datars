// Point clouds: rows as dots at their own coordinates — any number of them (a scatter of millions,
// every star of a catalogue, every address of a city). The engine indexes the rows once into a
// pyramid (`instances` with `lod`) and each frame draws only what the camera shows: a sample that
// keeps the data's density, every row once zoomed in far enough. This recipe frames the rows in an
// explorable view — drag to pan, wheel or pinch to zoom from the whole down to single rows — and
// gives each dot a tooltip.

import { e, instances, recipe, t, view, Prop, Template } from "@datars/sdk";

export interface CloudParams {
  data: string; x: string; y: string; r: Prop; fill: Prop; opacity: Prop; label: Prop; name: string;
  bbox: unknown; padding: number; explore: string; maxZoom: number; points: number; budget: number;
  glow: Prop; glowOpacity: Prop; glowPoints: number;
  children: Template[];
}

export const cloud = recipe<CloudParams>({
  id: "@datars/std/cloud",
  doc: "A point cloud of any size (millions of rows): dots at their own x/y in an explorable view — a density-preserving sample at every zoom, every row up close, a tooltip on each.",
  params: {
    data: t.table("A table, or a point archive (a `tiles` source the publish compiler wrote)."),
    x: t.field(), y: t.field(),
    r: t.prop("Dot radius in screen px: a number, or an expression over the row."),
    fill: t.prop("Dot ink, or an expression over the row (default $mark)."),
    opacity: t.prop("Per-dot opacity: an expression over the row."),
    label: t.prop("Tooltip and accessible name per row (default `x, y`)."),
    name: t.string(undefined, "What the rows are, for the accessible description ('4,000,000 stars')."),
    bbox: t.json("The extent to frame, [x0, y0, x1, y1] in the rows' units. Default: the rows' extent — give it for millions of rows (finding it reads them all) and for point archives."),
    padding: t.number(12),
    explore: t.string("cloud", "The signals the camera explores with (`<explore>.x`, `.y`, `.zoom`)."),
    maxZoom: t.number(2048, "How far in the reader can zoom, × the whole."),
    points: t.number(150000, "The most dots a frame draws: a view holding more shows a uniform sample this size, one holding fewer shows every row."),
    budget: t.number(2048, "Rows per tile of the index at average density (smaller: lighter tiles, more of them)."),
    glow: t.prop("Radius (px) of a glow under the dots (a number or an expression over the row): a sample drawn large and faint, so density reads at a glance and, up close, stars bloom. None by default."),
    glowOpacity: t.prop("The glow's opacity per dot: a number (default 0.05) or an expression over the row (weight bright rows)."),
    glowPoints: t.number(6000, "How many dots the glow draws at most."),
    children: t.children("Drawn over the dots, in the same coordinates (annotations, marks)."),
  },
  tokens: ["mark"],
  expand(p) {
    const q = JSON.stringify;
    const bbox = p.bbox ?? [
      e(`table.min(${q(p.data)}, ${q(p.x)})`), e(`table.min(${q(p.data)}, ${q(p.y)})`),
      e(`table.max(${q(p.data)}, ${q(p.x)})`), e(`table.max(${q(p.data)}, ${q(p.y)})`),
    ];
    return view({
      key: "cloud",
      camera: { fit: { bbox } as never, padding: p.padding, explore: p.explore, maxZoom: p.maxZoom },
      children: [
        p.glow != null && p.glow !== 0 ? instances({
          key: "glow",
          from: p.data,
          x: e(`d.${p.x}`),
          y: e(`d.${p.y}`),
          r: p.glow,
          fill: p.fill ?? "$mark",
          opacity: p.glowOpacity ?? 0.05,
          screenSize: true,
          lod: { points: p.glowPoints, budget: p.budget },
          semantics: { role: "decoration" },
        }) : null,
        instances({
          key: "rows",
          from: p.data,
          x: e(`d.${p.x}`),
          y: e(`d.${p.y}`),
          r: p.r ?? 1.2,
          fill: p.fill ?? "$mark",
          opacity: p.opacity,
          label: p.label ?? e(`\`\${d.${p.x}}, \${d.${p.y}}\``),
          screenSize: true,
          lod: { points: p.points, budget: p.budget },
          semantics: { role: "series", label: p.name ?? `${p.data}: ${p.y} against ${p.x}` },
        }),
        ...(p.children ?? []),
      ].filter(Boolean) as Template[],
    });
  },
});
