// datars-render-wgpu. One uniform block per draw (dynamic offset); every position ends in device
// pixels of the current target. Colours arrive as straight-alpha sRGB and leave premultiplied;
// the target is a non-sRGB format so blending happens in sRGB space like the CPU reference.

struct Draw {
    // Local → device px: x' = m.x·x + m.z·y + t.x, y' = m.y·x + m.w·y + t.y. The translation
    // already includes the mesh centre (added in f64 on the CPU, for precision).
    m: vec4<f32>,
    // t.z: stroke half-width in local units (0 for fills).
    t: vec4<f32>,
    // Solid paint colour, or the instances' outline colour. Straight alpha.
    color: vec4<f32>,
    // Gradient geometry relative to the mesh's reference point: linear (p0, p1); radial (c, r, 0).
    grad: vec4<f32>,
    // Vertex space → gradient space, 2×2 column-major (identity for meshes in local units; the
    // inverse linear transform for strokes built in device space).
    gm: vec4<f32>,
    // Opacity, paint mode (0 solid, 1 linear, 2 radial), ramp row, 1 if the target is sRGB.
    misc: vec4<f32>,
    // Instances: outline width px, px per size unit, symbol extent factor, polygon vertex count
    // (0 = analytic circle).
    inst: vec4<f32>,
    // Target size in device px.
    vp: vec4<f32>,
    // Symbol polygon (unit size), two vertices per vec4.
    poly: array<vec4<f32>, 6>,
};

@group(0) @binding(0) var<uniform> d: Draw;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

const RAMP_W: f32 = 256.0;

fn xform(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(d.m.x * p.x + d.m.z * p.y, d.m.y * p.x + d.m.w * p.y) + d.t.xy;
}

fn to_clip(px: vec2<f32>) -> vec4<f32> {
    return vec4<f32>(px.x / d.vp.x * 2.0 - 1.0, 1.0 - px.y / d.vp.y * 2.0, 0.0, 1.0);
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

// Straight → premultiplied (and linear, only when the platform forced an sRGB target).
fn finish(c: vec4<f32>) -> vec4<f32> {
    var rgb = c.rgb;
    if (d.misc.w > 0.5) {
        rgb = srgb_to_linear(rgb);
    }
    return vec4<f32>(rgb * c.a, c.a);
}

// ---- paths: fills, strokes, glyphs, clips -----------------------------------------------------

struct MeshOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>,
};

@vertex
fn vs_fill(@location(0) p: vec2<f32>) -> MeshOut {
    var o: MeshOut;
    o.pos = to_clip(xform(p));
    o.local = p;
    return o;
}

// Strokes extrude here, so one mesh serves every width in its bucket exactly.
@vertex
fn vs_stroke(@location(0) p: vec2<f32>, @location(1) n: vec2<f32>) -> MeshOut {
    let q = p + n * d.t.z;
    var o: MeshOut;
    o.pos = to_clip(xform(q));
    o.local = q;
    return o;
}

@fragment
fn fs_paint(i: MeshOut) -> @location(0) vec4<f32> {
    let mode = d.misc.y;
    let gp = vec2<f32>(d.gm.x * i.local.x + d.gm.z * i.local.y, d.gm.y * i.local.x + d.gm.w * i.local.y);
    var t = 0.0;
    if (mode < 1.5) {
        let ab = d.grad.zw - d.grad.xy;
        let l2 = dot(ab, ab);
        t = select(1.0, dot(gp - d.grad.xy, ab) / l2, l2 > 0.0);
    } else {
        t = length(gp - d.grad.xy) / max(d.grad.z, 1e-30);
    }
    let rows = f32(textureDimensions(tex).y);
    let uv = vec2<f32>((clamp(t, 0.0, 1.0) * (RAMP_W - 1.0) + 0.5) / RAMP_W, (d.misc.z + 0.5) / rows);
    // Sampled unconditionally (explicit LOD: legal outside uniform control flow, and cheap).
    let g = textureSampleLevel(tex, samp, uv, 0.0);
    var c = select(d.color, g, mode > 0.5);
    c.a = c.a * d.misc.x;
    return finish(c);
}

// ---- instances: SDF symbols and rects ---------------------------------------------------------

struct InstOut {
    @builtin(position) pos: vec4<f32>,
    // Offset from the shape centre in device px, along the shape's axes.
    @location(0) coord: vec2<f32>,
    // Symbols: (radius px, radius px). Rects: half extents px.
    @location(1) half: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) op: f32,
};

fn corner(vi: u32) -> vec2<f32> {
    var c = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, 1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
    );
    return c[vi];
}

@vertex
fn vs_symbol(
    @builtin(vertex_index) vi: u32,
    @location(0) p: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) op: f32,
) -> InstOut {
    let r = size.x * d.inst.y;
    // Room for the outline and one pixel of anti-aliasing.
    let ext = r * d.inst.z + d.inst.x * 0.5 + 1.0;
    let k = corner(vi);
    var o: InstOut;
    o.pos = to_clip(xform(p) + k * ext);
    o.coord = k * ext;
    o.half = vec2<f32>(r, r);
    o.color = color;
    o.op = op * d.misc.x;
    return o;
}

// Rects are geometry: the whole affine applies (a rotated view rotates them).
@vertex
fn vs_rect(
    @builtin(vertex_index) vi: u32,
    @location(0) p: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) op: f32,
) -> InstOut {
    let axes = vec2<f32>(length(d.m.xy), length(d.m.zw));
    let half = abs(size) * 0.5;
    let c = p + size * 0.5;
    let ext_px = half * axes + vec2<f32>(d.inst.x * 0.5 + 1.0);
    let k = corner(vi);
    var o: InstOut;
    o.pos = to_clip(xform(c + k * ext_px / axes));
    o.coord = k * ext_px;
    o.half = half * axes;
    o.color = color;
    o.op = op * d.misc.x;
    return o;
}

fn sd_box(p: vec2<f32>, b: vec2<f32>) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0);
}

fn poly_vertex(i: u32) -> vec2<f32> {
    let v = d.poly[i / 2u];
    return select(v.xy, v.zw, (i & 1u) == 1u);
}

// Signed distance to the symbol's polygon (scaled by r), negative inside (after Inigo Quilez).
fn sd_polygon(p: vec2<f32>, r: f32) -> f32 {
    let n = u32(d.inst.w);
    var dist = dot(p - poly_vertex(0u) * r, p - poly_vertex(0u) * r);
    var s = 1.0;
    var j = n - 1u;
    for (var i = 0u; i < n; i = i + 1u) {
        let vi = poly_vertex(i) * r;
        let vj = poly_vertex(j) * r;
        let e = vj - vi;
        let w = p - vi;
        let b = w - e * clamp(dot(w, e) / max(dot(e, e), 1e-30), 0.0, 1.0);
        dist = min(dist, dot(b, b));
        let c1 = p.y >= vi.y;
        let c2 = p.y < vj.y;
        let c3 = e.x * w.y > e.y * w.x;
        if ((c1 && c2 && c3) || (!c1 && !c2 && !c3)) {
            s = -s;
        }
        j = i;
    }
    return s * sqrt(dist);
}

// Fill under a centred outline, anti-aliased analytically from the signed distance (px).
fn shade(dist: f32, fill: vec4<f32>, op: f32) -> vec4<f32> {
    let w = d.inst.x;
    let f = finish(vec4<f32>(fill.rgb, fill.a * op)) * clamp(0.5 - dist, 0.0, 1.0);
    var c = f;
    if (w > 0.0) {
        let outer = clamp(0.5 - (dist - w * 0.5), 0.0, 1.0);
        let inner = clamp(0.5 - (dist + w * 0.5), 0.0, 1.0);
        let s = finish(vec4<f32>(d.color.rgb, d.color.a * op)) * max(outer - inner, 0.0);
        c = s + f * (1.0 - s.a);
    }
    if (c.a <= 0.0) {
        discard;
    }
    return c;
}

@fragment
fn fs_symbol(i: InstOut) -> @location(0) vec4<f32> {
    var dist = length(i.coord) - i.half.x;
    if (d.inst.w > 0.5) {
        dist = sd_polygon(i.coord, i.half.x);
    }
    return shade(dist, i.color, i.op);
}

@fragment
fn fs_rect(i: InstOut) -> @location(0) vec4<f32> {
    return shade(sd_box(i.coord, i.half), i.color, i.op);
}

// ---- layers -----------------------------------------------------------------------------------

// A full-screen triangle.
@vertex
fn vs_full(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    let x = f32((vi << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(vi & 2u) * 2.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

// The layer (premultiplied) scaled by its opacity; blend state does the compositing.
@fragment
fn fs_composite(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(tex, vec2<i32>(pos.xy), 0) * d.misc.x;
}

// Multiply, first half: dst · (Cs + 1 − αs) via a (Zero, Src) blend.
@fragment
fn fs_composite_mul(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let c = textureLoad(tex, vec2<i32>(pos.xy), 0) * d.misc.x;
    return vec4<f32>(c.rgb + vec3<f32>(1.0 - c.a), c.a);
}
