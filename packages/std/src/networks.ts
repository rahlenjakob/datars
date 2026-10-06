// Networks and hierarchies: links between things (network, arcDiagram, chord, matrix) and things
// inside things (tree, dendrogram, sunburst, icicle). The layouts are table ops run by the engine
// (datars-algo: a deterministic force simulation, Louvain communities, chords, tidy trees and
// partitions); these recipes only draw the rows they add.

import { e, group, op, recipe, repeat, shape, geom, t, text, Cx, Prop, Template } from "@datars/sdk";

const J = JSON.stringify;
/** A column of the current row, as expression source. */
const col = (name: string) => (/^[A-Za-z_$][A-Za-z0-9_$]*$/.test(name) ? `d.${name}` : `d[${J(name)}]`);

// Helper signatures use one-line `type`s, never inline object types: `datars eject` copies each
// helper by its first brace block and each `type` by its line.
type GraphNames = { id?: string; source?: string; target?: string; label?: string };

/** Node/link column names with their defaults. */
function graphNames(p: GraphNames) {
  const id = p.id ?? "id";
  return { id, source: p.source ?? "source", target: p.target ?? "target", label: p.label ? col(p.label) : `key.name(${col(id)})` };
}

/** Opacity of a mark under a keyset selection: strong when nothing is selected or `hit` holds. */
function selectedOpacity(sel: string | undefined, hit: string, on: number, off: number): Prop {
  return sel ? e(`${sel}.isEmpty() || (${hit}) ? ${on} : ${off}`) : on;
}

/** A node order for a line or a matrix: `input`, `cluster`, `degree` or a nodes column. */
function orderOps(order: string) {
  if (order === "input") return [];
  if (order === "cluster") return [op.sort("order")];
  if (order === "degree") return [op.sort(["degree", "desc"], "order")];
  return [op.sort(order, "order")];
}

// ---- network (force-directed node-link diagram) ----------------------------------------------------

export interface NetworkParams {
  nodes: string; links: string; id: string; source: string; target: string; weight: string; size: string; color: string; label: string;
  minRadius: number; maxRadius: number; distance: number; charge: number; iterations: number; seed: number; labels: number; selected: string; format: string;
}

export const network = recipe<NetworkParams>({
  id: "@datars/std/network",
  doc: "A node-link diagram laid out by a deterministic force simulation — seeded starts, a fixed number of steps, computed once per data and size, never per frame: links pull, nodes repel, never overlap and stay inside the box. Nodes sized by a field (default: their links), the largest named; nodes and links keyed, so a data change moves them.",
  params: {
    nodes: t.table("One row per node."),
    links: t.table("One row per link: the ids of its two nodes (and optionally a weight)."),
    id: t.field("The nodes' id column, which links' source and target name (default `id`)."),
    source: t.field("The links' source column (default `source`)."),
    target: t.field("The links' target column (default `target`)."),
    weight: t.field("Link weight: heavier links are thicker and pull harder (default: all equal)."),
    size: t.field("Node size field (default `degree`: how many links a node has, or their summed weight)."),
    color: t.field("Colour nodes by this field (categorical); `community` colours the clusters found in the links. Default: one colour."),
    label: t.field("Node name column (default: the id, through the document's key names)."),
    minRadius: t.number(3, "Radius of the smallest node (px)."),
    maxRadius: t.number(14, "Radius of the largest node (px); areas between are ∝ size."),
    distance: t.number(0, "Link rest length in px (0: from the box and the number of nodes)."),
    charge: t.number(0, "Many-body strength: negative repels, positive attracts (0: from the link length)."),
    iterations: t.number(300, "Simulation steps — a fixed number, so the layout is the same on every device."),
    seed: t.number(1, "Seed of the starting positions: another seed, another (equally valid) layout."),
    labels: t.number(8, "How many nodes carry a name label, largest first (0: none; every node names itself on hover). Labels that would collide are left out."),
    selected: t.string(undefined, "A keyset signal of node ids: those nodes and their links stay strong, the rest recede."),
    format: t.string(",.0f", "Number format of sizes and weights in tooltips."),
  },
  tokens: ["mark", "categorical", "paper", "rule", "grid", "ink", "muted", "size.label"],
  expand(p, cx) {
    const g = graphNames(p);
    const links = { links: p.links, source: g.source, target: g.target, id: g.id, weight: p.weight };
    const stats = cx.table("stats", p.nodes, op.communities(links));
    const S = J(stats);
    const size = p.size ?? "degree";
    const count = `max(1, table.count(${S}))`;
    const radius = `${p.minRadius} + ${Math.max(0, p.maxRadius - p.minRadius)} * sqrt(max(0, ${col(size)}) / max(1e-9, table.max(${S}, ${J(size)})))`;
    // Rest length from the room each node gets; repulsion ∝ its square (the layout's scale).
    const dist = p.distance ? String(p.distance) : `clamp(sqrt(box.w * box.h / ${count}) * 0.45, 12, 100)`;
    const charge = p.charge ? String(p.charge) : `-(${dist}) * (${dist}) / 20`;
    const nodes = cx.table("nodes", stats,
      op.derive("r", e(radius)),
      op.force({ ...links, radius: e("d.r"), width: e("box.w"), height: e("box.h"), distance: e(dist), charge: e(charge), iterations: p.iterations, seed: p.seed }),
      op.window("rank", size, "rank", { order: `-${size}` }));
    const ends = cx.table("links", p.links,
      op.lookup({ from: nodes, key: g.id, field: g.source, values: ["x", "y"], as: ["x1", "y1"] }),
      op.lookup({ from: nodes, key: g.id, field: g.target, values: ["x", "y"], as: ["x2", "y2"] }),
      op.filter(e("isFinite(d.x1) && isFinite(d.x2)")));
    // Labels largest first, so decluttering keeps the big nodes' names.
    const named = cx.table("named", nodes, op.filter(e(`d.rank <= ${Math.max(0, Math.floor(p.labels))}`)), op.sort("rank"));
    const sel = p.selected;
    // A node that recedes turns grey rather than translucent (links would show through it).
    const strong = sel ? `(${sel}.isEmpty() || ${sel}.has(${col(g.id)}))` : "true";
    const colour = p.color ? `scale.color(${col(p.color)})` : `"$mark"`;
    const fill = sel ? e(`${strong} ? ${colour} : "$grid"`) : p.color ? e(colour) : "$mark";
    const w = p.weight ? `1 + 2.2 * sqrt(max(0, ${col(p.weight)}) / max(1e-9, table.max(${J(p.links)}, ${J(p.weight)})))` : "1.2";
    const f = J(p.format);
    const nodeLabel = p.size
      ? `\`\${${g.label}}: \${format(${col(size)}, ${f})}\``
      : p.weight ? `\`\${${g.label}}: \${format(d.degree, ${f})} ${p.weight}\`` : `\`\${${g.label}}: \${d.degree} link\${d.degree == 1 ? "" : "s"}\``;
    const linkLabel = `\`\${key.name(${col(g.source)})} – \${key.name(${col(g.target)})}${p.weight ? `: \${format(${col(p.weight)}, ${f})}` : ""}\``;
    return group({
      key: "network",
      scales: p.color ? { color: { type: "categorical", domain: { data: stats, field: p.color }, range: "$categorical" } } : undefined,
      semantics: { role: "group", label: "Network" },
      children: [
        group({ key: "links", children: [repeat(ends, shape(geom.segment({ x1: e("d.x1"), y1: e("d.y1"), x2: e("d.x2"), y2: e("d.y2") }), {
          key: [e(col(g.source)), e(col(g.target))],
          stroke: { paint: sel ? e(`!${sel}.isEmpty() && (${sel}.has(${col(g.source)}) || ${sel}.has(${col(g.target)})) ? "$ink-2" : "$rule"`) : "$rule", width: e(w), cap: "round" },
          opacity: selectedOpacity(sel, `${sel}.has(${col(g.source)}) || ${sel}.has(${col(g.target)})`, 0.6, 0.12),
          semantics: { role: "datum", label: e(linkLabel) },
          pickable: true,
        }))] }),
        group({ key: "nodes", children: [repeat(nodes, shape(geom.circle({ cx: e("d.x"), cy: e("d.y"), r: e("d.r") }), {
          key: e(col(g.id)),
          fill,
          stroke: { paint: "$paper", width: 1.5 },
          semantics: { role: "datum", label: e(nodeLabel), value: e(col(size)) },
          pickable: true,
        }))] }),
        p.labels > 0 ? group({ key: "labels", declutter: true, children: [repeat(named, text(e(g.label), [e("d.x + d.r + 3"), e("d.y")], {
          key: e(col(g.id)),
          style: { size: "$size.label", ink: sel ? e(`${strong} ? "$ink" : "$muted"`) : "$ink", baseline: "middle", contain: true },
          halo: ["$paper", 3],
        }))] }) : null,
      ],
    });
  },
});

// ---- arcDiagram --------------------------------------------------------------------------------------

export interface ArcDiagramParams {
  nodes: string; links: string; id: string; source: string; target: string; weight: string; size: string; color: string; label: string;
  order: string; maxRadius: number; labels: boolean; selected: string; format: string;
}

export const arcDiagram = recipe<ArcDiagramParams>({
  id: "@datars/std/arcDiagram",
  doc: "Nodes on a line and each link as an arc above it, as tall as it is long. Ordered so clusters sit together (or by degree, a field, or input order); names under the line, turned when they crowd. Links between nodes of one colour take it.",
  params: {
    nodes: t.table("One row per node."),
    links: t.table("One row per link: the ids of its two nodes (and optionally a weight)."),
    id: t.field("The nodes' id column (default `id`)."),
    source: t.field("The links' source column (default `source`)."),
    target: t.field("The links' target column (default `target`)."),
    weight: t.field("Link weight: heavier links are thicker (default: all equal)."),
    size: t.field("Node size field (default `degree`, the node's links)."),
    color: t.field("Colour nodes (and the arcs within one colour) by this field; `community` colours clusters. Default: one colour."),
    label: t.field("Node name column (default: the id)."),
    order: t.string("cluster", "Order along the line: `cluster` (communities together, best connected first), `degree`, `input`, or a column of the nodes table."),
    maxRadius: t.number(7, "Radius of the largest node (px)."),
    labels: t.bool(true, "Node names under the line (turned upright when they don't fit across)."),
    selected: t.string(undefined, "A keyset signal of node ids: their arcs stay strong, the rest recede."),
    format: t.string(",.0f", "Number format of sizes and weights in tooltips."),
  },
  tokens: ["mark", "categorical", "paper", "rule", "ink-2", "size.label"],
  expand(p, cx) {
    const g = graphNames(p);
    const links = { links: p.links, source: g.source, target: g.target, id: g.id, weight: p.weight };
    const stats = cx.table("stats", p.nodes, op.communities(links));
    const nodes = cx.table("nodes", stats, ...orderOps(p.order ?? "cluster"), op.derive("lw", e(`measure(${g.label}, token("size.label"))`)));
    const N = J(nodes);
    const size = p.size ?? "degree";
    const withColor = p.color
      ? [op.lookup({ from: stats, key: g.id, field: g.source, values: [p.color], as: ["c1"] }), op.lookup({ from: stats, key: g.id, field: g.target, values: [p.color], as: ["c2"] })]
      : [];
    const arcs = cx.table("arcs", p.links, ...withColor, op.filter(e(`scale.nx(${col(g.source)}) != null && scale.nx(${col(g.target)}) != null`)));
    const R = Math.max(1, p.maxRadius);
    // Names across when each fits its step, else turned upright; the line sits above them.
    const turned = `(scale.nx.step() < table.max(${N}, "lw") + 6)`;
    const below = p.labels ? `(${turned} ? table.max(${N}, "lw") + 6 : token("size.label") + 6)` : "0";
    // Arcs as half-ellipses, as tall as half their span, flattened so the longest possible fits
    // above; in a box taller than that needs (a phone), the whole diagram centred up and down.
    const half = "max(1, (scale.nx.max() - scale.nx.min()) / 2)";
    const under = `(${below} + ${R + 3})`;
    const base = `(box.h - ${under} - max(0, (box.h - ${under} - ${half} - ${R + 2}) / 2))`;
    const k = `min(1, (${base} - ${R + 2}) / ${half})`;
    const x1 = `scale.nx(${col(g.source)})`, x2 = `scale.nx(${col(g.target)})`;
    const c = `(abs(${x2} - ${x1}) / 2 * ${k} * 4 / 3)`;
    const path = `\`M \${${x1}} \${${base}} C \${${x1}} \${${base} - ${c}} \${${x2}} \${${base} - ${c}} \${${x2}} \${${base}}\``;
    const r = `(2.5 + ${R - 2.5} * sqrt(max(0, ${col(size)}) / max(1e-9, table.max(${N}, ${J(size)}))))`;
    const nodeFill = p.color ? e(`scale.color(${col(p.color)})`) : "$mark";
    const arcInk = p.color ? e(`d.c1 == d.c2 ? scale.color(d.c1) : "$rule"`) : "$mark";
    const w = p.weight ? `1 + 2.5 * sqrt(max(0, ${col(p.weight)}) / max(1e-9, table.max(${J(p.links)}, ${J(p.weight)})))` : "1.4";
    const f = J(p.format);
    const sel = p.selected;
    return group({
      key: "arc-diagram",
      scales: {
        nx: { type: "point", domain: { data: nodes, field: g.id }, range: "width", padding: 1 },
        ...(p.color ? { color: { type: "categorical", domain: { data: stats, field: p.color }, range: "$categorical" } } : {}),
      },
      semantics: { role: "group", label: "Arc diagram" },
      children: [
        group({ key: "arcs", children: [repeat(arcs, shape(geom.path(e(path)), {
          key: [e(col(g.source)), e(col(g.target))],
          stroke: { paint: arcInk, width: e(w), cap: "round" },
          opacity: selectedOpacity(sel, `${sel}.has(${col(g.source)}) || ${sel}.has(${col(g.target)})`, 0.55, 0.1),
          semantics: { role: "datum", label: e(`\`\${key.name(${col(g.source)})} – \${key.name(${col(g.target)})}${p.weight ? `: \${format(${col(p.weight)}, ${f})}` : ""}\``) },
          pickable: true,
        }))] }),
        group({ key: "nodes", children: [repeat(nodes, shape(geom.circle({ cx: e(`scale.nx(${col(g.id)})`), cy: e(base), r: e(r) }), {
          key: e(col(g.id)),
          fill: nodeFill,
          stroke: { paint: "$paper", width: 1.5 },
          semantics: { role: "datum", label: e(`\`\${${g.label}}: \${format(${col(size)}, ${f})}${p.size ? "" : " links"}\``), value: e(col(size)) },
          pickable: true,
        }))] }),
        p.labels ? group({ key: "labels", children: [repeat(nodes, text(e(g.label), [e(`scale.nx(${col(g.id)})`), e(`${base} + ${R + 5}`)], {
          key: e(col(g.id)),
          rotate: e(`${turned} ? -90 : 0`),
          style: { size: "$size.label", ink: "$ink-2", align: e(`${turned} ? "end" : "middle"`), baseline: e(`${turned} ? "middle" : "top"`), contain: true },
        }))] }) : null,
      ],
    });
  },
});

// ---- chord -------------------------------------------------------------------------------------------

export interface ChordParams { data: string; source: string; target: string; value: string; pad: number; thickness: number; labels: boolean; format: string; selected: string }

export const chord = recipe<ChordParams>({
  id: "@datars/std/chord",
  doc: "A chord diagram: groups around a circle, each as long as the flows touching it, and a ribbon per flow between its two groups, as wide as the flow at both ends. Ribbons sharing a group never cross; each takes its source's colour.",
  params: {
    data: t.table("One row per flow: source, target and value."),
    source: t.field("The flows' source column (default `source`)."),
    target: t.field("The flows' target column (default `target`)."),
    value: t.field("The flows' size (default: each flow counts 1)."),
    pad: t.number(0.04, "Gap between groups (radians)."),
    thickness: t.number(0.07, "The group arcs' thickness, as a share of the radius."),
    labels: t.bool(true, "Group names outside the circle, where their arc has room."),
    format: t.string(",.0f", "Number format of values in tooltips."),
    selected: t.string(undefined, "A keyset signal of group names: flows touching them stay strong, the rest recede."),
  },
  tokens: ["categorical", "paper", "ink-2", "size.label"],
  expand(p, cx) {
    const src = p.source ?? "source", dst = p.target ?? "target";
    const common = { source: src, target: dst, value: p.value, pad: p.pad };
    const groups = cx.table("groups", p.data, op.chordGroups(common), op.derive("lw", e(`measure(key.name(d.name), token("size.label"))`)));
    const G = J(groups);
    // The outer radius leaves room for the widest name beside the circle (or a margin without names).
    const outer = p.labels ? `max(12, min(box.h / 2 - token("size.label") - 4, box.w / 2 - table.max(${G}, "lw") - 16))` : "max(12, min(box.w, box.h) / 2 - 4)";
    const inner = `(${outer}) * ${1 - p.thickness}`;
    const ribbons = cx.table("ribbons", p.data, op.chordRibbons({ ...common, cx: e("box.w / 2"), cy: e("box.h / 2"), r: e(`${inner} - 1.5`) }));
    const mid = "(d.a0 + d.a1) / 2";
    const f = J(p.format);
    const sel = p.selected;
    const v = p.value ? col(p.value) : "1";
    return group({
      key: "chord",
      scales: { color: { type: "categorical", domain: { data: groups, field: "name" }, range: "$categorical" } },
      semantics: { role: "group", label: "Chord diagram" },
      children: [
        group({ key: "ribbons", children: [repeat(ribbons, shape(geom.path(e("d.path")), {
          key: [e(col(src)), e(col(dst))],
          fill: e(`scale.color(${col(src)})`),
          opacity: selectedOpacity(sel, `${sel}.has(${col(src)}) || ${sel}.has(${col(dst)})`, 0.68, 0.12),
          stroke: { paint: "$paper", width: 0.5 },
          semantics: { role: "datum", label: e(`\`\${key.name(${col(src)})} → \${key.name(${col(dst)})}: \${format(${v}, ${f})}\``), value: e(v) },
          pickable: true,
        }))] }),
        group({ key: "groups", children: [repeat(groups, shape(geom.arc({ cx: e("box.w / 2"), cy: e("box.h / 2"), r0: e(inner), r1: e(outer), a0: e("d.a0"), a1: e("d.a1") }), {
          key: e("d.name"),
          fill: e("scale.color(d.name)"),
          semantics: { role: "datum", label: e(`\`\${key.name(d.name)}: \${format(d.value, ${f})}\``), value: e("d.value") },
          pickable: true,
        }))] }),
        p.labels ? group({ key: "labels", declutter: true, children: [repeat(groups, text(e("key.name(d.name)"), [e(`box.w / 2 + (${outer} + 6) * sin(${mid})`), e(`box.h / 2 - (${outer} + 6) * cos(${mid})`)], {
          key: e("d.name"),
          when: e(`(d.a1 - d.a0) * (${outer}) >= token("size.label") * 0.8`),
          style: { size: "$size.label", ink: "$ink-2", align: e(`sin(${mid}) >= 0 ? "start" : "end"`), baseline: e(`abs(cos(${mid})) > 0.94 ? (cos(${mid}) > 0 ? "alphabetic" : "hanging") : "middle"`) },
        }))] }) : null,
      ],
    });
  },
});

// ---- matrix (adjacency matrix) --------------------------------------------------------------------------

export interface MatrixParams {
  nodes: string; links: string; id: string; source: string; target: string; weight: string; color: string; label: string;
  order: string; directed: boolean; labels: boolean; format: string;
}

export const matrix = recipe<MatrixParams>({
  id: "@datars/std/matrix",
  doc: "An adjacency matrix: a row and a column per node, a cell where two are linked — shaded by weight, coloured where both ends share a colour. Ordered so clusters show as blocks along the diagonal (or by degree, a field, or input order).",
  params: {
    nodes: t.table("One row per node."),
    links: t.table("One row per link: the ids of its two nodes (and optionally a weight)."),
    id: t.field("The nodes' id column (default `id`)."),
    source: t.field("The links' source column (default `source`)."),
    target: t.field("The links' target column (default `target`)."),
    weight: t.field("Link weight: heavier links are darker cells (default: all equal)."),
    color: t.field("Colour cells whose two nodes share this field's value by it (`community`: the clusters); the others stay neutral. Default: one colour."),
    label: t.field("Node name column (default: the id)."),
    order: t.string("cluster", "Order of the rows and columns: `cluster` (communities together, best connected first), `degree`, `input`, or a column of the nodes table."),
    directed: t.bool(false, "Links go one way: a link fills only its source's row. Otherwise each fills both cells, so the matrix is symmetric."),
    labels: t.bool(true, "Node names left of the rows and above the columns, where the rows are tall enough."),
    format: t.string(",.0f", "Number format of weights in tooltips."),
  },
  tokens: ["mark", "categorical", "muted", "surface", "ink-2", "size.label"],
  expand(p, cx) {
    const g = graphNames(p);
    const links = { links: p.links, source: g.source, target: g.target, id: g.id, weight: p.weight };
    const stats = cx.table("stats", p.nodes, op.communities(links));
    const nodes = cx.table("nodes", stats, ...orderOps(p.order ?? "cluster"), op.derive("lw", e(`measure(${g.label}, token("size.label"))`)));
    const N = J(nodes);
    const withColor = p.color
      ? [op.lookup({ from: stats, key: g.id, field: g.source, values: [p.color], as: ["c1"] }), op.lookup({ from: stats, key: g.id, field: g.target, values: [p.color], as: ["c2"] })]
      : [];
    const cells = cx.table("cells", p.links, ...withColor, op.filter(e(`scale.m(${col(g.source)}) != null && scale.m(${col(g.target)}) != null`)));
    // Names need rows at least ~7 px tall; the matrix is square, names in a gutter left and above.
    const gutter = p.labels ? `(box.h / max(1, table.count(${N})) >= 7 ? min(table.max(${N}, "lw"), box.w * 0.3) + 6 : 0)` : "0";
    const side = `max(0, min(box.w, box.h) - ${gutter})`;
    const x0 = `((box.w - ${side} - ${gutter}) / 2 + ${gutter})`;
    const fill = p.color ? e(`d.c1 == d.c2 ? scale.color(d.c1) : "$muted"`) : "$mark";
    const shade = p.weight ? e(`0.3 + 0.7 * sqrt(max(0, ${col(p.weight)}) / max(1e-9, table.max(${J(p.links)}, ${J(p.weight)})))`) : 1;
    const f = J(p.format);
    const label = `\`\${key.name(${col(g.source)})} – \${key.name(${col(g.target)})}${p.weight ? `: \${format(${col(p.weight)}, ${f})}` : ""}\``;
    const cell = (row: string, column: string, key: Prop) => shape(geom.rect({ x: e(`scale.m(${column})`), y: e(`scale.m(${row})`), w: e("scale.m.bandwidth()"), h: e("scale.m.bandwidth()"), r: e("min(2, scale.m.bandwidth() / 5)") }), {
      key, fill, opacity: shade, semantics: { role: "datum", label: e(label), value: p.weight ? e(col(p.weight)) : undefined }, pickable: true,
    });
    const size = `min(token("size.label"), scale.m.bandwidth() * 0.92)`;
    const named = `scale.m.bandwidth() >= 6`;
    return group({
      key: "matrix",
      transform: { translate: [e(x0), e(gutter)] },
      scales: {
        m: { type: "band", domain: { data: nodes, field: g.id }, range: [0, `=${side}`], padding: 0.08 },
        ...(p.color ? { color: { type: "categorical", domain: { data: stats, field: p.color }, range: "$categorical" } } : {}),
      },
      semantics: { role: "group", label: "Adjacency matrix" },
      children: [
        shape(geom.rect({ x: 0, y: 0, w: e(side), h: e(side), r: 3 }), { key: "ground", fill: "$surface", semantics: { role: "decoration" } }),
        group({ key: "cells", children: [repeat(cells, cell(col(g.source), col(g.target), [e(col(g.source)), e(col(g.target))]))] }),
        p.directed ? null : group({ key: "mirror", children: [repeat(cells, cell(col(g.target), col(g.source), [e(col(g.target)), e(col(g.source))]))] }),
        p.labels ? group({ key: "rows", children: [repeat(nodes, text(e(g.label), [-5, e(`scale.m(${col(g.id)}) + scale.m.bandwidth() / 2`)], {
          key: e(col(g.id)), when: e(named), style: { size: e(size), ink: "$ink-2", align: "end", baseline: "middle", maxWidth: e(`${gutter} - 6`) },
        }))] }) : null,
        p.labels ? group({ key: "columns", children: [repeat(nodes, text(e(g.label), [e(`scale.m(${col(g.id)}) + scale.m.bandwidth() / 2`), -5], {
          key: e(col(g.id)), when: e(named), rotate: -90, style: { size: e(size), ink: "$ink-2", align: "start", baseline: "middle", maxWidth: e(`${gutter} - 6`) },
        }))] }) : null,
      ],
    });
  },
});

// ---- tree and dendrogram ---------------------------------------------------------------------------------

type TreeParams = { data: string; id: string; parent: string; label: string; orientation: "horizontal" | "vertical" | "radial"; color: string; link: "curve" | "elbow" | "line"; r: number; labels: boolean };

function treeParams(link: "curve" | "elbow") {
  return {
    data: t.table("One row per node: its id and its parent's id (none for the root)."),
    id: t.field("The nodes' id column (default `id`)."),
    parent: t.field("The column holding each node's parent id (default `parent`); empty for the root."),
    label: t.field("Node name column (default: the id)."),
    orientation: t.oneOf(["horizontal", "vertical", "radial"] as const, "horizontal", "Root at the left (names read across), at the top, or in the middle with the leaves around it."),
    color: t.field("Colour nodes by this field (categorical). Default: branches dark, leaves light."),
    link: t.oneOf(["curve", "elbow", "line"] as const, link, "Links as smooth curves, right-angled elbows or straight lines."),
    r: t.number(3.5, "Node radius (px)."),
    labels: t.bool(true, "Node names: leaves outside, branches beside their node."),
  };
}

/** A node-link hierarchy: the tidy tree (`tidy`) or the dendrogram (`cluster`), in any orientation. */
function hierarchyTree(p: TreeParams, cx: Cx, method: "tidy" | "cluster"): Template {
  const id = p.id ?? "id", parent = p.parent ?? "parent";
  const label = p.label ? col(p.label) : `key.name(${col(id)})`;
  const orient = p.orientation ?? "horizontal";
  const R = Math.max(0, p.r);
  // Measure names on a probe layout first: the room around the tree depends on which are leaves.
  const probe = cx.table("probe", p.data, op.tree({ id, parent, method, width: 1, height: 1 }),
    op.derive("lw", e(`measure(${label}, token("size.label"))`)),
    op.derive("leafw", e("d.leaf ? d.lw : 0")), op.derive("rootw", e("d.depth == 0 ? d.lw : 0")), op.derive("isleaf", e("d.leaf ? 1 : 0")));
  const P = J(probe);
  const leafW = p.labels ? `table.max(${P}, "leafw")` : "0", rootW = p.labels ? `table.max(${P}, "rootw")` : "0";
  const gap = R + 4;
  const fs = `token("size.label")`;
  let width: string, height: string, X: string, Y: string, PX: string, PY: string;
  const crowded = `(box.w / max(1, table.sum(${P}, "isleaf")) < ${leafW} + 6)`;
  if (orient === "vertical") {
    const top = p.labels ? `(max(${R}, ${fs} / 2) + 3)` : `${R + 2}`;
    const bottom = p.labels ? `(${crowded} ? ${leafW} + ${gap} + 2 : ${fs} + ${gap} + 2)` : `${R + 2}`;
    width = `box.w - ${2 * R + 12}`; height = `box.h - ${top} - ${bottom}`;
    X = `(${R + 6} + d.x)`; Y = `(${top} + d.y)`; PX = `(${R + 6} + d.px)`; PY = `(${top} + d.py)`;
  } else if (orient === "radial") {
    const rad = `max(10, min(box.w, box.h) / 2 - ${leafW} - ${gap} - 2)`;
    width = String(2 * Math.PI); height = rad;
    X = "(box.w / 2 + d.y * sin(d.x))"; Y = "(box.h / 2 - d.y * cos(d.x))"; PX = "(box.w / 2 + d.py * sin(d.px))"; PY = "(box.h / 2 - d.py * cos(d.px))";
  } else {
    const left = p.labels ? `(${rootW} + ${gap} + 2)` : `${R + 2}`;
    const right = p.labels ? `(${leafW} + ${gap} + 2)` : `${R + 2}`;
    const pad = `(${fs} / 2 + 2)`;
    width = `box.h - 2 * ${pad}`; height = `box.w - ${left} - ${right}`;
    X = `(${left} + d.y)`; Y = `(${pad} + d.x)`; PX = `(${left} + d.py)`; PY = `(${pad} + d.px)`;
  }
  const laid = cx.table("tree", p.data, op.tree({ id, parent, label: p.label, method, width: e(width), height: e(height) }));
  const linksT = cx.table("links", laid, op.filter(e("isFinite(d.px)")));
  const link = p.link ?? (method === "cluster" ? "elbow" : "curve");
  const polar = (a: string, r: string) => [`box.w / 2 + (${r}) * sin(${a})`, `box.h / 2 - (${r}) * cos(${a})`];
  let path: string;
  if (link === "line") {
    path = `\`M \${${PX}} \${${PY}} L \${${X}} \${${Y}}\``;
  } else if (orient === "radial") {
    if (link === "curve") {
      const m = "(d.py + d.y) / 2";
      const [c1x, c1y] = polar("d.px", m), [c2x, c2y] = polar("d.x", m);
      path = `\`M \${${PX}} \${${PY}} C \${${c1x}} \${${c1y}} \${${c2x}} \${${c2y}} \${${X}} \${${Y}}\``;
    } else {
      // Along the parent's circle to the child's angle (one cubic arc), then straight out.
      const k = "(4 / 3 * tan((d.x - d.px) / 4) * d.py)";
      const [qx, qy] = polar("d.x", "d.py");
      path = `\`M \${${PX}} \${${PY}} C \${${PX} + ${k} * cos(d.px)} \${${PY} + ${k} * sin(d.px)} \${${qx} - ${k} * cos(d.x)} \${${qy} - ${k} * sin(d.x)} \${${qx}} \${${qy}} L \${${X}} \${${Y}}\``;
    }
  } else if (orient === "vertical") {
    path = link === "curve"
      ? `\`M \${${PX}} \${${PY}} C \${${PX}} \${(${PY} + ${Y}) / 2} \${${X}} \${(${PY} + ${Y}) / 2} \${${X}} \${${Y}}\``
      : `\`M \${${PX}} \${${PY}} H \${${X}} V \${${Y}}\``;
  } else {
    path = link === "curve"
      ? `\`M \${${PX}} \${${PY}} C \${(${PX} + ${X}) / 2} \${${PY}} \${(${PX} + ${X}) / 2} \${${Y}} \${${X}} \${${Y}}\``
      : `\`M \${${PX}} \${${PY}} V \${${Y}} H \${${X}}\``;
  }
  // Names: leaves outside, after their node (turned along the radius in a radial tree, and under
  // crowded leaves of a vertical one); branches before their node across, beside it down, above it
  // in a radial tree — where, crowded near the middle, those that would collide are left out.
  let leafAt: [Prop, Prop], leafRotate: Prop, leafAlign: Prop, leafBaseline: Prop;
  let branchAt: [Prop, Prop], branchAlign: Prop, branchBaseline: Prop;
  if (orient === "radial") {
    const rr = `(d.y + ${gap})`;
    leafAt = [e(`box.w / 2 + ${rr} * sin(d.x)`), e(`box.h / 2 - ${rr} * cos(d.x)`)];
    leafRotate = e(`d.x * ${180 / Math.PI} + (d.x < ${Math.PI} ? -90 : 90)`);
    leafAlign = e(`d.x < ${Math.PI} ? "start" : "end"`);
    leafBaseline = "middle";
    branchAt = [e(X), e(`${Y} - ${gap}`)];
    branchAlign = "middle";
    branchBaseline = "alphabetic";
  } else if (orient === "vertical") {
    leafAt = [e(X), e(`${Y} + ${gap}`)];
    leafRotate = e(`${crowded} ? 90 : 0`);
    leafAlign = e(`${crowded} ? "start" : "middle"`);
    leafBaseline = e(`${crowded} ? "middle" : "top"`);
    branchAt = [e(`${X} + ${gap}`), e(Y)];
    branchAlign = "start";
    branchBaseline = "middle";
  } else {
    leafAt = [e(`${X} + ${gap}`), e(Y)];
    leafRotate = 0;
    leafAlign = "start";
    leafBaseline = "middle";
    branchAt = [e(`${X} - ${gap}`), e(Y)];
    branchAlign = "end";
    branchBaseline = "middle";
  }
  return group({
    key: method === "cluster" ? "dendrogram" : "tree",
    scales: p.color ? { color: { type: "categorical", domain: { data: p.data, field: p.color }, range: "$categorical" } } : undefined,
    semantics: { role: "group", label: method === "cluster" ? "Dendrogram" : "Tree" },
    children: [
      group({ key: "links", children: [repeat(linksT, shape(geom.path(e(path)), {
        key: e(col(id)), stroke: { paint: "$rule", width: 1.2, join: "round" }, opacity: 0.8, semantics: { role: "decoration" },
      }))] }),
      group({ key: "nodes", children: [repeat(laid, shape(geom.circle({ cx: e(X), cy: e(Y), r: R }), {
        key: e(col(id)),
        fill: p.color ? e(`scale.color(${col(p.color)})`) : e(`d.leaf ? "$muted" : "$ink-2"`),
        stroke: { paint: "$paper", width: 1 },
        semantics: { role: "datum", label: e("d.path") },
        pickable: true,
      }))] }),
      p.labels ? group({ key: "leaf-labels", children: [repeat(laid, text(e(label), leafAt, {
        key: e(col(id)), when: e("d.leaf"), rotate: leafRotate,
        style: { size: "$size.label", ink: "$ink-2", align: leafAlign, baseline: leafBaseline, contain: true },
      }))] }) : null,
      p.labels ? group({ key: "branch-labels", declutter: orient === "radial", children: [repeat(laid, text(e(label), branchAt, {
        key: e(col(id)), when: e("!d.leaf"),
        style: { size: "$size.label", ink: "$ink", align: branchAlign, baseline: branchBaseline, contain: true },
        halo: ["$paper", 3],
      }))] }) : null,
    ],
  });
}

export const tree = recipe<TreeParams>({
  id: "@datars/std/tree",
  doc: "A tidy tree (Reingold–Tilford) of a parent-child table: parents centred over their children, subtrees packed as close as they go, identical subtrees drawn alike. Horizontal, vertical or radial; nodes keyed by id, so the tree re-lays itself when rows change.",
  params: treeParams("curve"),
  tokens: ["rule", "ink", "ink-2", "muted", "paper", "size.label"],
  expand(p, cx) {
    return hierarchyTree(p, cx, "tidy");
  },
});

export const dendrogram = recipe<TreeParams>({
  id: "@datars/std/dendrogram",
  doc: "A dendrogram (cluster layout) of a parent-child table: every leaf on one line, each parent centred over its children at its height, joined by elbows. Horizontal, vertical or radial; nodes keyed by id.",
  params: treeParams("elbow"),
  tokens: ["rule", "ink", "ink-2", "muted", "paper", "size.label"],
  expand(p, cx) {
    return hierarchyTree(p, cx, "cluster");
  },
});

// ---- sunburst and icicle ------------------------------------------------------------------------------------

type PartitionParams = { data: string; id: string; parent: string; value: string; label: string; color: string; sort: boolean; labels: boolean; format: string };

function partitionParams() {
  return {
    data: t.table("One row per node: its id and its parent's id (none for the root)."),
    id: t.field("The nodes' id column (default `id`)."),
    parent: t.field("The column holding each node's parent id (default `parent`); empty for the root."),
    value: t.field("Leaf size (a parent is the sum of its leaves; values on parent rows are ignored). Default: every leaf counts 1."),
    label: t.field("Node name column (default: the id)."),
    color: t.field("Colour by this field (default `branch`: each top-level branch its own colour, lighter deeper down)."),
    sort: t.bool(false, "Siblings largest first. Off, they keep row order — so when the values change, every piece grows or shrinks in place instead of changing places."),
    labels: t.bool(true, "Names inside the pieces that have room for them."),
    format: t.string(",.0f", "Number format of values in labels and tooltips."),
  };
}

/** A partition's fill, its opacity (lighter with depth) and the ink that reads on it. */
function partitionInk(p: PartitionParams, cx: Cx, laid: string, depth0: string) {
  const by = p.color ?? "branch";
  // Colours in order of the pieces drawn (a lone root, the hole or the neutral first band, has none).
  const drawn = cx.table("coloured", laid, op.filter(e(`d.depth >= ${depth0}`)));
  const fade = `max(0.35, 1 - 0.2 * (d.depth - ${depth0}))`;
  return {
    fill: e(`scale.color(${col(by)})`),
    opacity: e(fade),
    ink: e(`${fade} > 0.75 ? "on(" + scale.color(${col(by)}) + ")" : "$ink"`),
    scale: { type: "categorical", domain: { data: drawn, field: by }, range: "$categorical" },
  };
}

const partitionLabel = (f: string) => e(`\`\${d.path}: \${format(d.sum, ${f})} (\${format(d.share, ".0%")})\``);

type SunburstParams = PartitionParams & { total: boolean };

export const sunburst = recipe<SunburstParams>({
  id: "@datars/std/sunburst",
  doc: "A sunburst: a hierarchy as rings, the root in the middle and each node an arc spanning its share of its parent. A single root becomes the hole, with the total in it; names follow the arcs where they fit.",
  params: { ...partitionParams(), total: t.bool(true, "The root's name and total in the middle (with a single root).") },
  tokens: ["categorical", "paper", "ink", "ink-2", "size.label", "size.title", "font.title"],
  expand(p, cx) {
    const id = p.id ?? "id", parent = p.parent ?? "parent";
    const label = p.label ? col(p.label) : `key.name(${col(id)})`;
    const R = "(min(box.w, box.h) / 2 - 2)";
    const laid = cx.table("partition", p.data, op.partition({ id, parent, label: p.label, value: p.value, width: 2 * Math.PI, height: e(R), sort: p.sort }),
      op.derive("lw", e(`measure(${label}, token("size.label"))`)), op.derive("isroot", e("d.depth == 0 ? 1 : 0")));
    const L = J(laid);
    const single = `table.sum(${L}, "isroot") == 1`;
    const depth0 = `(${single} ? 1 : 0)`;
    const ink = partitionInk(p, cx, laid, depth0);
    const f = J(p.format);
    // Names along the arc where they fit across it, else along the radius, else none.
    const a = "((d.x0 + d.x1) / 2)", rm = "((d.y0 + d.y1) / 2)";
    const along = `(d.lw + 8 <= (d.x1 - d.x0) * ${rm} && token("size.label") + 4 <= d.y1 - d.y0)`;
    const across = `(d.lw + 8 <= d.y1 - d.y0 && token("size.label") + 2 <= (d.x1 - d.x0) * ${rm})`;
    const deg = `${a} * ${180 / Math.PI}`;
    const rot = `${along} ? ${deg} - (${a} > ${Math.PI / 2} && ${a} < ${1.5 * Math.PI} ? 180 : 0) : ${deg} + (${a} < ${Math.PI} ? -90 : 90)`;
    return group({
      key: "sunburst",
      scales: { color: ink.scale as never },
      semantics: { role: "group", label: "Sunburst" },
      children: [
        group({ key: "arcs", children: [repeat(laid, shape(geom.arc({ cx: e("box.w / 2"), cy: e("box.h / 2"), r0: e("d.y0"), r1: e("d.y1"), a0: e("d.x0"), a1: e("d.x1") }), {
          key: e(col(id)),
          when: e(`d.depth >= ${depth0}`),
          fill: ink.fill, opacity: ink.opacity,
          stroke: { paint: "$paper", width: 1 },
          semantics: { role: "datum", label: partitionLabel(f), value: e("d.sum") },
          pickable: true,
        }))] }),
        p.labels ? group({ key: "labels", children: [repeat(laid, text(e(label), [e(`box.w / 2 + ${rm} * sin(${a})`), e(`box.h / 2 - ${rm} * cos(${a})`)], {
          key: e(col(id)),
          when: e(`d.depth >= ${depth0} && (${along} || ${across})`),
          rotate: e(rot),
          style: { size: "$size.label", ink: ink.ink, align: "middle", baseline: "middle" },
        }))] }) : null,
        p.total ? group({ key: "total", when: e(single), children: [repeat(cx.table("root", laid, op.filter(e("d.depth == 0"))), group({ children: [
          text(e(label), [e("box.w / 2"), e("box.h / 2 - 4")], { key: "name", when: e(`d.y1 >= 34 && d.lw + 8 <= 2 * d.y1`), style: { size: "$size.label", ink: "$ink-2", align: "middle", baseline: "alphabetic" } }),
          text("", [e("box.w / 2"), e("box.h / 2")], { key: "value", when: e("d.y1 >= 22"), number: { value: e("d.sum"), format: p.format }, style: { font: "font.title", size: e(`min(token("size.title"), d.y1 * 0.5)`), ink: "$ink", align: "middle", baseline: e(`d.y1 >= 34 && d.lw + 8 <= 2 * d.y1 ? "hanging" : "middle"`) } }),
        ] }))] }) : null,
      ],
    });
  },
});

type IcicleParams = PartitionParams & { orientation: "horizontal" | "vertical" };

export const icicle = recipe<IcicleParams>({
  id: "@datars/std/icicle",
  doc: "An icicle: a hierarchy as bands, the root first and each node spanning its share of its parent, its name and value inside where they fit. Horizontal (the root at the left) or vertical (on top).",
  params: { ...partitionParams(), orientation: t.oneOf(["horizontal", "vertical"] as const, "horizontal", "The root at the left, its children in the next column (names read across), or on top.") },
  tokens: ["categorical", "paper", "muted", "ink", "size.label", "size.small"],
  expand(p, cx) {
    const id = p.id ?? "id", parent = p.parent ?? "parent";
    const label = p.label ? col(p.label) : `key.name(${col(id)})`;
    const across = (p.orientation ?? "horizontal") === "horizontal";
    const f = J(p.format);
    const laid = cx.table("partition", p.data, op.partition({ id, parent, label: p.label, value: p.value, width: e(across ? "box.h" : "box.w"), height: e(across ? "box.w" : "box.h"), sort: p.sort }),
      op.derive("lw", e(`measure(${label}, token("size.label"), 600)`)),
      op.derive("vw", e(`measure(format(d.sum, ${f}), token("size.small"))`)),
      op.derive("rootw", e("d.depth == 0 ? max(d.lw, d.vw) : 0")));
    const ink = partitionInk(p, cx, laid, "1");
    // The roots' band only needs room for a name and its value (across: as wide as they are; down:
    // two lines tall), narrower than the others, which share the rest.
    const L = J(laid);
    const H = across ? "box.w" : "box.h";
    const band = `(${H} / (table.max(${L}, "depth") + 1))`;
    const thin = across ? `min(${band}, table.max(${L}, "rootw") + 14)` : `min(${band}, token("size.label") + token("size.small") + 22)`;
    const depthAt = (v: string) => `(${v} <= ${band} ? ${v} * ${thin} / ${band} : ${thin} + (${v} - ${band}) * (${H} - ${thin}) / max(1, ${H} - ${band}))`;
    const [d0, d1] = [depthAt("d.y0"), depthAt("d.y1")];
    const [x, y, w, h] = across ? [d0, "d.x0", `(${d1} - ${d0})`, "(d.x1 - d.x0)"] : ["d.x0", d0, "(d.x1 - d.x0)", `(${d1} - ${d0})`];
    // A root is neutral (it spans everything); its children take the colours.
    const fill = e(`d.depth == 0 ? "$muted" : scale.color(${col(p.color ?? "branch")})`);
    const opacity = e(`d.depth == 0 ? 0.3 : ${ink.opacity.expr}`);
    const textInk = e(`d.depth == 0 ? "$ink" : ${ink.ink.expr}`);
    const nameFits = `${w} >= d.lw + 12 && ${h} >= token("size.label") + 8`;
    const valueFits = `${w} >= max(d.lw, d.vw) + 12 && ${h} >= token("size.label") + token("size.small") + 14`;
    return group({
      key: "icicle",
      scales: { color: ink.scale as never },
      semantics: { role: "group", label: "Icicle" },
      children: [
        group({ key: "cells", children: [repeat(laid, shape(geom.rect({ x: e(x), y: e(y), w: e(w), h: e(h) }), {
          key: e(col(id)), fill, opacity, stroke: { paint: "$paper", width: 1 },
          semantics: { role: "datum", label: partitionLabel(f), value: e("d.sum") }, pickable: true,
        }))] }),
        p.labels ? group({ key: "labels", children: [repeat(laid, group({ children: [
          text(e(label), [e(`${x} + 6`), e(`${y} + 6`)], { key: "name", when: e(nameFits), style: { size: "$size.label", weight: 600, ink: textInk, baseline: "top" } }),
          text(e(`format(d.sum, ${f})`), [e(`${x} + 6`), e(`${y} + 8 + token("size.label")`)], { key: "value", when: e(valueFits), style: { size: "$size.small", ink: textInk, baseline: "top" } }),
        ] }))] }) : null,
      ],
    });
  },
});
