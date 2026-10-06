//! The texts a frame shows, where they are drawn — for hosts that lay real text over the canvas
//! (PDF.js-style), so readers can select, copy and find a chart's words and crawlers can read
//! them. Exact to the pixels: the same origin, scale and rotation `datars_render::flatten` draws a
//! text node's runs with, and the lines `datars_text` laid it out in.

use crate::bounds::transform_rect;
use crate::pick::{seen, INVISIBLE};
use datars_math::{Affine, Rect, Vec2};
use datars_scene::{Clip, KeyPath, Node, NodeKind, Role, Scene, TextNode};
use datars_text::FontDb;

/// A text as the frame draws it.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct TextOut {
    /// Key path (as `hit_test`, `explain` and the semantics spell it): the same while the element
    /// stays on screen, so a host can keep what a reader selected across frames.
    pub path: String,
    /// The whole text, with the line breaks it has itself (not its wraps).
    pub text: String,
    /// The semantic role of the text or of its nearest ancestor with one (`title`, `annotation`,
    /// `tick`, `legend-item`, …); none when nothing says.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// The authored family and weight, and the faces the glyphs came from (primary first): for
    /// pages that match the drawn font.
    pub family: String,
    pub weight: u16,
    pub faces: Vec<String>,
    /// Font size in CSS px, as drawn (a camera's zoom included).
    pub size: f64,
    /// Clockwise rotation in radians, about the text's origin (every line turns with it).
    pub rotate: f64,
    /// Opacity as drawn, with its ancestors'.
    pub opacity: f64,
    /// A place name a map's vector tiles put there (not authored text).
    pub place: bool,
    /// Pressing here starts a drag or a click the chart handles (a pan, a brush, a slider, a
    /// checkbox's label): a host leaves the pointer to the chart there rather than start a text
    /// selection.
    pub drag: bool,
    /// Axis-aligned bounds of the lines as drawn, `[x, y, w, h]` in CSS px.
    pub bounds: [f64; 4],
    /// One box per line, in order.
    pub lines: Vec<LineOut>,
}

/// One line of a [`TextOut`]: a box a host sizes its own text to.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct LineOut {
    pub text: String,
    /// The line box's top-left corner in CSS px (the rotation, about the text's origin, applied).
    pub x: f64,
    pub y: f64,
    /// The box before rotation: the drawn advance by the face's ascent + descent.
    pub w: f64,
    pub h: f64,
    /// The baseline, down from the box's top.
    pub baseline: f64,
    /// A line break in the text follows this line (a wrap doesn't).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub br: bool,
    /// Its paragraph reads right to left (the base direction its runs were ordered in).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub rtl: bool,
}

/// Every text `scene` draws readably, in tree order — the order recipes compose a chart in
/// (a title before its axes, an axis's ticks together), which is the reading order a copy of
/// several texts keeps. Texts clipped away or off the canvas aren't there.
pub fn texts(scene: &Scene, fonts: &FontDb) -> Vec<TextOut> {
    let mut out = Vec::new();
    let canvas = Rect::new(0.0, 0.0, scene.width, scene.height);
    walk(&scene.root, &KeyPath::default(), Affine::IDENTITY, 1.0, canvas, None, fonts, &mut out);
    out
}

/// The role a node's semantics give it, if they say more than "a group".
fn role_of(n: &Node) -> Option<String> {
    let s = n.semantics.as_ref().filter(|s| s.role != Role::Group)?;
    serde_json::to_value(s.role).ok()?.as_str().map(String::from)
}

#[allow(clippy::too_many_arguments)]
fn walk(n: &Node, path: &KeyPath, parent: Affine, acc: f64, clip: Rect, role: Option<&str>, fonts: &FontDb, out: &mut Vec<TextOut>) {
    let acc = seen(n, acc);
    if acc <= INVISIBLE {
        return;
    }
    let xf = n.common.placed(parent);
    let here = path.push(&n.key);
    let own = role_of(n);
    let role = own.as_deref().or(role);
    let clip = match &n.common.clip {
        Some(Clip::Rect { rect }) => transform_rect(*rect, &xf).intersect(&clip),
        Some(Clip::Path { path }) => transform_rect(path.bounds(), &xf).intersect(&clip),
        None => Some(clip),
    };
    let Some(clip) = clip else { return };
    match &n.kind {
        NodeKind::Group { children } => children.iter().for_each(|c| walk(c, &here, xf, acc, clip, role, fonts, out)),
        NodeKind::View { viewport, camera, clip: clips, children } => {
            let clip = if *clips { transform_rect(*viewport, &xf).intersect(&clip) } else { Some(clip) };
            let Some(clip) = clip else { return };
            let cam = camera.map(|c| c.transform(*viewport)).unwrap_or(Affine::translate(viewport.x, viewport.y));
            children.iter().for_each(|c| walk(c, &here, xf.mul(cam), acc, clip, role, fonts, out));
        }
        NodeKind::Text(t) if !t.runs.is_empty() && !t.text.trim().is_empty() => {
            if let Some(o) = text_out(t, &xf, fonts) {
                let b = Rect::new(o.bounds[0], o.bounds[1], o.bounds[2], o.bounds[3]);
                if b.intersects(&clip) {
                    out.push(TextOut { path: here.to_string(), role: role.map(String::from), opacity: acc, ..o });
                }
            }
        }
        _ => {}
    }
}

/// Thousandths of a CSS px: exact enough for any screen, and a host comparing two frames' layers
/// isn't fooled by float noise.
fn r3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0 + 0.0
}

/// A text node drawn through `xf`, as `datars_render::flatten` draws it: glyphs at
/// `xf(origin) + offset·scale`, scaled by `scale`, turned by the node's own rotation.
fn text_out(t: &TextNode, xf: &Affine, fonts: &FontDb) -> Option<TextOut> {
    let scale = if t.screen_size { 1.0 } else { xf.scale_factor() };
    let origin = xf.apply(t.origin) + t.offset * scale;
    let turn = Affine::rotate(t.rotate);
    let mut bounds = Rect::empty();
    let mut lines: Vec<LineOut> = Vec::new();
    for l in datars_text::lines(fonts, &t.text, &t.style) {
        if l.text.trim().is_empty() {
            // A blank line breaks lines but has nothing to select.
            if let Some(prev) = lines.last_mut() {
                prev.br |= l.hard_break;
            }
            continue;
        }
        let top_left = origin + turn.apply_vec(Vec2::new(l.x, l.baseline - l.ascent) * scale);
        let (w, h) = (l.width * scale, (l.ascent + l.descent) * scale);
        for corner in [Vec2::ZERO, Vec2::new(w, 0.0), Vec2::new(0.0, h), Vec2::new(w, h)] {
            bounds = bounds.include(top_left + turn.apply_vec(corner));
        }
        lines.push(LineOut { text: l.text, x: r3(top_left.x), y: r3(top_left.y), w: r3(w), h: r3(h), baseline: r3(l.ascent * scale), br: l.hard_break, rtl: l.rtl });
    }
    if lines.is_empty() || bounds.is_empty() {
        return None;
    }
    let mut faces: Vec<String> = Vec::new();
    for r in &t.runs {
        if !faces.iter().any(|f| **f == *r.font) {
            faces.push(r.font.to_string());
        }
    }
    Some(TextOut {
        path: String::new(),
        text: t.text.clone(),
        role: None,
        family: t.style.family.to_string(),
        weight: t.style.weight,
        faces,
        size: r3(t.style.size * scale),
        rotate: t.rotate,
        opacity: 1.0,
        place: false,
        drag: false,
        bounds: [r3(bounds.x), r3(bounds.y), r3(bounds.w), r3(bounds.h)],
        lines,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use datars_scene::text::TextStyle;
    use datars_scene::Key;

    fn shaped(db: &FontDb, s: &str, at: Vec2) -> TextNode {
        let mut t = TextNode::new(s, at, TextStyle { size: 10.0, ..TextStyle::default() });
        datars_text::layout(db, &mut t);
        t
    }

    /// Text that zooms with its transform (not screen-sized) is drawn — and laid over — at the
    /// transform's scale; a clip hides what's outside it.
    #[test]
    fn scaled_text_scales_and_clipped_text_goes() {
        let db = FontDb::with_bundled();
        let mut zoomed = shaped(&db, "Zoomed", Vec2::new(10.0, 20.0));
        zoomed.screen_size = false;
        let plain = shaped(&db, "Zoomed", Vec2::new(10.0, 20.0));
        let hidden = shaped(&db, "Clipped", Vec2::new(150.0, 20.0));
        let mut clipped = Node::group(Key::name("clipped"), vec![Node::text(Key::name("hidden"), hidden)]);
        clipped.common.clip = Some(Clip::Rect { rect: Rect::new(0.0, 0.0, 100.0, 100.0) });
        let root = Node::group(Key::name("root"), vec![Node::text(Key::name("zoomed"), zoomed).transform(Affine::scale(2.0, 2.0)), Node::text(Key::name("plain"), plain), clipped]);
        let out = texts(&Scene::new(400.0, 200.0, root), &db);
        assert_eq!(out.len(), 2, "{out:?}");
        let (z, p) = (&out[0], &out[1]);
        assert_eq!((z.size, p.size), (20.0, 10.0));
        let (zl, pl) = (&z.lines[0], &p.lines[0]);
        assert!((zl.w - 2.0 * pl.w).abs() < 1e-2 && (zl.h - 2.0 * pl.h).abs() < 1e-2, "{zl:?} {pl:?}");
        assert!((zl.x - 20.0).abs() < 1e-9 && (zl.y + zl.baseline - 40.0).abs() < 1e-2, "its origin goes through the transform: {zl:?}");
    }

    /// A right-to-left text's lines say so (the page lays them in that direction), a left-to-right
    /// one's don't (and the JSON leaves the flag out).
    #[test]
    fn lines_carry_their_paragraphs_direction() {
        let db = FontDb::with_bundled();
        let root = Node::group(Key::name("root"), vec![Node::text(Key::name("he"), shaped(&db, "ירושלים 2024", Vec2::new(10.0, 20.0))), Node::text(Key::name("en"), shaped(&db, "Oslo 2024", Vec2::new(10.0, 60.0)))]);
        let out = texts(&Scene::new(400.0, 200.0, root), &db);
        assert_eq!(out.iter().map(|t| t.lines[0].rtl).collect::<Vec<_>>(), [true, false]);
        let json = serde_json::to_string(&out).unwrap();
        assert_eq!(json.matches("\"rtl\":true").count(), 1, "{json}");
        assert!(!json.contains("\"rtl\":false"));
    }
}
