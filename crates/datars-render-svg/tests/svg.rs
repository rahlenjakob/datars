//! The SVG writer: expected elements for every op kind, well-formed XML, resolvable ids,
//! compact numbers, escaping, determinism.

use datars_color::Color;
use datars_math::{Affine, FillRule, PathData, Rect, Vec2};
use datars_render::{DPaint, DisplayList, GlyphSource, InstancesOp, NoGlyphs, Op, SharedPath, StrokeStyle};
use datars_render_svg::to_svg;
use datars_scene::{Blend, Cap, GlyphPos, Join, Proto, SymbolKind};
use std::collections::BTreeSet;
use std::sync::Arc;

const RED: Color = Color::rgba(1.0, 0.0, 0.0, 1.0);

struct Squares;
impl GlyphSource for Squares {
    fn outline(&self, _font: &str, glyph: u16, size: f64) -> Option<PathData> {
        (glyph != 0).then(|| PathData::rect(Rect::new(0.1 * size, -0.5 * size, 0.5 * size, 0.5 * size)))
    }
}

fn instances(proto: Proto, xf: Affine) -> InstancesOp {
    InstancesOp {
        proto,
        xf,
        x: vec![10.0, 20.0, 30.0].into(),
        y: vec![10.0, 12.0, 14.0].into(),
        size: vec![2.0, 3.0, 4.0].into(),
        w: Some(vec![4.0, -5.0, 6.0].into()),
        h: Some(vec![3.0, 3.0, 3.0].into()),
        fill: vec![RED, Color::rgb8(0, 128, 255), Color::BLACK.with_alpha(0.5)].into(),
        opacity: vec![1.0, 0.5, 1.0].into(),
        alpha: 1.0,
        stroke: Some((Color::WHITE, 0.5)),
        size_scale: 1.0,
    }
}

fn scene() -> DisplayList {
    let mut star = PathData::new();
    star.move_to(Vec2::new(50.0, 5.0)).line_to(Vec2::new(60.0, 40.0)).line_to(Vec2::new(30.0, 18.0)).line_to(Vec2::new(70.0, 18.0)).line_to(Vec2::new(40.0, 40.0)).close();
    let mut curve = PathData::new();
    curve.move_to(Vec2::new(0.0, 60.0)).cubic_to(Vec2::new(20.0, 20.0), Vec2::new(40.0, 100.0), Vec2::new(60.0, 60.0));
    let linear = DPaint::Linear { p0: Vec2::new(0.0, 0.0), p1: Vec2::new(10.0, 0.0), stops: vec![(0.0, RED), (0.5, Color::WHITE.with_alpha(0.25)), (1.0, Color::BLACK)] };
    let radial = DPaint::Radial { c: Vec2::new(5.0, 5.0), r: 5.0, stops: vec![(0.0, Color::WHITE), (1.0, RED)] };
    let dashed = StrokeStyle { width_px: 1.5, cap: Cap::Round, join: Join::Bevel, miter_limit: 4.0, dash: Some(vec![4.0, 2.0]) };
    let glyphs: Arc<[GlyphPos]> = vec![GlyphPos { id: 1, x: 0.0, y: 0.0 }, GlyphPos { id: 0, x: 8.0, y: 0.0 }, GlyphPos { id: 2, x: 16.0, y: 0.0 }].into();
    let ops = vec![
        Op::Fill { path: SharedPath::new(star), xf: Affine::IDENTITY, paint: DPaint::Solid(RED), rule: FillRule::EvenOdd, opacity: 1.0 },
        Op::Fill { path: SharedPath::new(PathData::rect(Rect::new(0.0, 0.0, 10.0, 10.0))), xf: Affine::translate(100.0, 10.0).mul(Affine::scale(3.0, 2.0)), paint: linear.clone(), rule: FillRule::NonZero, opacity: 0.8 },
        Op::Fill { path: SharedPath::new(PathData::circle(Vec2::new(5.0, 5.0), 5.0)), xf: Affine::translate(140.0, 10.0), paint: radial, rule: FillRule::NonZero, opacity: 1.0 },
        Op::Stroke { path: SharedPath::new(curve.clone()), xf: Affine::translate(0.5, 0.5), style: dashed, paint: DPaint::Solid(Color::rgb8(29, 31, 36)), opacity: 1.0 },
        Op::Stroke { path: SharedPath::new(curve), xf: Affine::scale(2.0, 1.0), style: StrokeStyle { width_px: 3.0, cap: Cap::Square, join: Join::Miter, miter_limit: 10.0, dash: None }, paint: linear, opacity: 0.5 },
        Op::PushClip { path: SharedPath::new(PathData::rect(Rect::new(0.0, 0.0, 80.0, 80.0))), xf: Affine::IDENTITY, rule: FillRule::NonZero },
        Op::PushClip { path: SharedPath::new(PathData::circle(Vec2::new(40.0, 40.0), 30.0)), xf: Affine::IDENTITY, rule: FillRule::EvenOdd },
        Op::PushLayer { opacity: 0.5, blend: Blend::Multiply },
        Op::Fill { path: SharedPath::new(PathData::rect(Rect::new(0.0, 0.0, 200.0, 200.0))), xf: Affine::IDENTITY, paint: DPaint::Solid(Color::rgb8(60, 169, 81)), rule: FillRule::NonZero, opacity: 1.0 },
        Op::PopLayer,
        Op::PopClip,
        Op::PopClip,
        Op::Instances(instances(Proto::Symbol { symbol: SymbolKind::Circle }, Affine::translate(0.0, 100.0))),
        Op::Instances(instances(Proto::Symbol { symbol: SymbolKind::Square }, Affine::translate(40.0, 100.0))),
        Op::Instances(instances(Proto::Symbol { symbol: SymbolKind::Diamond }, Affine::translate(80.0, 100.0))),
        Op::Instances(instances(Proto::Rect, Affine::translate(120.0, 100.0).mul(Affine::scale(1.5, 1.0)))),
        Op::Instances(instances(Proto::Rect, Affine::rotate(0.3))),
        Op::Glyphs { font: Arc::from("A&B <\"x\">"), size: 10.0, glyphs, origin: Vec2::new(20.0, 150.0), scale: 1.0, rotate: 0.0, color: RED, halo: Some((Color::WHITE, 2.0)), text: Some(Arc::from("A & <b>")) },
        Op::Image { asset: Arc::from("photo.png"), rect: Rect::new(0.0, 0.0, 10.0, 10.0), xf: Affine::IDENTITY, opacity: 1.0 },
    ];
    DisplayList { width: 200.0, height: 160.5, background: Color::rgb8(251, 248, 241), ops }
}

/// A minimal XML well-formedness check: one root element, properly nested tags, quoted and
/// unique attributes, no raw `<` or bare `&` in values or text.
fn well_formed(s: &str) -> Result<(), String> {
    fn entities_ok(t: &str) -> bool {
        let mut rest = t;
        while let Some(i) = rest.find('&') {
            let after = &rest[i + 1..];
            let Some(j) = after.find(';') else { return false };
            let ent = &after[..j];
            let ok = matches!(ent, "amp" | "lt" | "gt" | "quot" | "apos") || (ent.starts_with('#') && ent.len() > 1);
            if !ok {
                return false;
            }
            rest = &after[j + 1..];
        }
        true
    }
    let mut stack: Vec<&str> = Vec::new();
    let mut roots = 0;
    let mut i = 0;
    while i < s.len() {
        let rest = &s[i..];
        if let Some(r) = rest.strip_prefix('<') {
            let end = r.find('>').ok_or("unterminated tag")?;
            let tag = &r[..end];
            i += end + 2;
            if let Some(name) = tag.strip_prefix('/') {
                let open = stack.pop().ok_or(format!("unexpected </{name}>"))?;
                if open != name.trim() {
                    return Err(format!("</{name}> closes <{open}>"));
                }
                continue;
            }
            let self_closing = tag.ends_with('/');
            let body = tag.trim_end_matches('/');
            let name_end = body.find(|c: char| c.is_whitespace()).unwrap_or(body.len());
            let name = &body[..name_end];
            if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == ':' || c == '-' || c == '_') {
                return Err(format!("bad tag name {name:?}"));
            }
            let mut attrs = &body[name_end..];
            let mut seen = BTreeSet::new();
            loop {
                attrs = attrs.trim_start();
                if attrs.is_empty() {
                    break;
                }
                let eq = attrs.find('=').ok_or(format!("attribute without value in <{name}>"))?;
                let an = &attrs[..eq];
                if an.is_empty() || !an.chars().all(|c| c.is_ascii_alphanumeric() || c == ':' || c == '-' || c == '_') {
                    return Err(format!("bad attribute name {an:?}"));
                }
                if !seen.insert(an) {
                    return Err(format!("duplicate attribute {an} in <{name}>"));
                }
                let v = attrs[eq + 1..].strip_prefix('"').ok_or(format!("unquoted {an}"))?;
                let close = v.find('"').ok_or(format!("unterminated {an}"))?;
                let value = &v[..close];
                if value.contains('<') || !entities_ok(value) {
                    return Err(format!("bad value for {an}: {value:?}"));
                }
                attrs = &v[close + 1..];
            }
            if stack.is_empty() {
                roots += 1;
            }
            if !self_closing {
                stack.push(name);
            }
        } else {
            let next = rest.find('<').unwrap_or(rest.len());
            let text = &rest[..next];
            if !entities_ok(text) || (stack.is_empty() && !text.trim().is_empty()) {
                return Err(format!("bad text {text:?}"));
            }
            i += next;
        }
    }
    if !stack.is_empty() {
        return Err(format!("unclosed {stack:?}"));
    }
    if roots != 1 {
        return Err(format!("{roots} root elements"));
    }
    Ok(())
}

fn attr_values<'a>(svg: &'a str, attr: &str) -> Vec<&'a str> {
    let key = format!(" {attr}=\"");
    svg.match_indices(&key).map(|(i, _)| {
        let v = &svg[i + key.len()..];
        &v[..v.find('"').unwrap()]
    }).collect()
}

#[test]
fn document_is_well_formed_with_the_expected_elements() {
    let svg = to_svg(&scene(), &Squares);
    well_formed(&svg).unwrap_or_else(|e| panic!("{e}\n{svg}"));
    assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" version=\"1.1\" width=\"200\" height=\"160.5\" viewBox=\"0 0 200 160.5\">"), "{svg}");
    assert!(svg.contains("<rect width=\"200\" height=\"160.5\" fill=\"#fbf8f1\"/>"), "background");
    assert!(svg.contains("fill-rule=\"evenodd\""));
    assert!(svg.contains("transform=\"matrix(3 0 0 2 100 10)\""), "fill transform");
    assert_eq!(svg.matches("<linearGradient").count(), 2);
    assert_eq!(svg.matches("<radialGradient").count(), 1);
    assert!(svg.contains("gradientUnits=\"userSpaceOnUse\""));
    assert!(svg.contains("gradientTransform=\"matrix(2 0 0 1 0 0)\""), "stroke gradients carry the op transform");
    assert!(svg.contains("stop-opacity=\"0.25\""));
    assert!(svg.contains("fill-opacity=\"0.8\""));
    // Strokes: geometry pre-transformed, attributes for non-defaults only.
    assert!(svg.contains("<path d=\"M0.5 60.5C20.5 20.5 40.5 100.5 60.5 60.5\" fill=\"none\" stroke=\"#1d1f24\" stroke-width=\"1.5\" stroke-linecap=\"round\" stroke-linejoin=\"bevel\" stroke-dasharray=\"4 2\"/>"), "{svg}");
    assert!(svg.contains("stroke-linecap=\"square\" stroke-miterlimit=\"10\""));
    assert!(svg.contains("stroke-opacity=\"0.5\""));
    // Clips and layers nest.
    assert_eq!(svg.matches("<clipPath").count(), 2);
    assert!(svg.contains("clip-rule=\"evenodd\""));
    assert!(svg.contains("<g opacity=\"0.5\" style=\"mix-blend-mode:multiply\">"));
    // Instances.
    assert_eq!(svg.matches("<circle").count(), 3);
    assert!(svg.contains("<circle cx=\"10\" cy=\"110\" r=\"2\" fill=\"#ff0000\"/>"), "{svg}");
    assert!(svg.contains("<circle cx=\"20\" cy=\"112\" r=\"3\" fill=\"#0080ff\" fill-opacity=\"0.5\" stroke-opacity=\"0.5\"/>"), "{svg}");
    assert!(svg.contains("<rect x=\"48\" y=\"108\" width=\"4\" height=\"4\""), "square symbols are rects: {svg}");
    // Rect instance 2 has w = −5: local x 15..20, ×1.5 + 120 → 142.5..150.
    assert!(svg.contains("<rect x=\"142.5\" y=\"112\" width=\"7.5\" height=\"3\""), "negative widths normalise: {svg}");
    assert_eq!(svg.matches("<rect").count(), 1 + 3 + 3, "background, squares, axis-aligned rects");
    assert!(svg.contains("<g stroke=\"#ffffff\" stroke-width=\"0.5\">"));
    // Glyphs: a halo run and a fill run, two outlined glyphs each (glyph 0 has no outline). Each
    // outline is defined once and placed by <use>; the fill run carries the text as its label.
    assert_eq!(svg.matches("<use xlink:href=\"#").count(), 4);
    assert_eq!(svg.matches("<path id=\"").count(), 2, "one def per distinct outline: {svg}");
    // Outlines in integer units at 1000/em, relative after the first point (h/v for axis-aligned
    // edges); the run group scales em units to px (size 10 → 0.01) and positions are in em units
    // (16 px → 1600).
    assert!(svg.contains("d=\"M350-500h250v500h-500v-500z\"/>"), "{svg}");
    assert!(svg.contains(" x=\"1600\"/>"), "{svg}");
    assert!(svg.contains("<g transform=\"matrix(0.01 0 0 0.01 20 150)\""), "{svg}");
    assert!(svg.contains("aria-hidden=\"true\" fill=\"none\" stroke=\"#ffffff\" stroke-width=\"200\" stroke-linejoin=\"round\" stroke-linecap=\"round\">"), "{svg}");
    assert_eq!(svg.matches("role=\"img\" aria-label=\"A &amp; &lt;b&gt;\"").count(), 1, "{svg}");
    assert!(svg.contains("<g data-image=\"photo.png\"/>"));
}

#[test]
fn every_reference_resolves_to_a_unique_id() {
    let svg = to_svg(&scene(), &Squares);
    let ids = attr_values(&svg, "id");
    let unique: BTreeSet<&str> = ids.iter().copied().collect();
    assert_eq!(unique.len(), ids.len(), "ids are unique");
    let mut refs = 0;
    for (i, _) in svg.match_indices("url(#") {
        let r = &svg[i + 5..];
        let r = &r[..r.find(')').unwrap()];
        assert!(unique.contains(r), "dangling reference #{r}");
        refs += 1;
    }
    assert_eq!(refs, 3 + 2, "three gradient uses and two clips");
    // Definitions live in <defs>, before the body.
    let defs_end = svg.find("</defs>").unwrap();
    for id in &ids {
        assert!(svg.find(&format!("id=\"{id}\"")).unwrap() < defs_end);
    }
}

#[test]
fn numbers_have_at_most_three_decimals_outside_matrices() {
    let mut list = scene();
    list.ops.push(Op::Fill {
        path: SharedPath::new(PathData::polygon(&[Vec2::new(1.0 / 3.0, 2.0 / 3.0), Vec2::new(10.123456, 5.0), Vec2::new(-0.00001, 7.77777)])),
        xf: Affine::IDENTITY,
        paint: DPaint::Solid(RED),
        rule: FillRule::NonZero,
        opacity: 1.0,
    });
    let svg = to_svg(&list, &Squares);
    assert!(svg.contains("M0.333 0.667L10.123 5L0 7.778Z"), "{svg}");
    for d in attr_values(&svg, "d") {
        for num in d.split(|c: char| !(c.is_ascii_digit() || c == '.')) {
            if let Some((_, frac)) = num.split_once('.') {
                assert!(frac.len() <= 3 && !frac.ends_with('0'), "{num} in {d}");
            }
        }
    }
    assert!(!svg.contains("-0 ") && !svg.contains("-0\""), "no negative zero");
}

#[test]
fn output_is_deterministic_and_ids_depend_on_content() {
    let a = to_svg(&scene(), &Squares);
    assert_eq!(a, to_svg(&scene(), &Squares));
    let mut other = scene();
    other.width = 201.0;
    let b = to_svg(&other, &Squares);
    let id = |s: &str| attr_values(s, "id")[0].to_string();
    assert_ne!(id(&a), id(&b), "two documents inlined in one page don't clash");
}

#[test]
fn transparent_background_and_empty_list() {
    let list = DisplayList { width: 10.0, height: 10.0, background: Color::TRANSPARENT, ops: vec![] };
    let svg = to_svg(&list, &NoGlyphs);
    well_formed(&svg).unwrap();
    assert!(!svg.contains("<rect"));
    assert!(!svg.contains("<defs>"));
}

#[test]
fn unbalanced_pops_and_pushes_still_close_every_group() {
    let clip = Op::PushClip { path: SharedPath::new(PathData::rect(Rect::new(0.0, 0.0, 5.0, 5.0))), xf: Affine::IDENTITY, rule: FillRule::NonZero };
    let ops = vec![Op::PopLayer, Op::PopClip, clip.clone(), Op::PushLayer { opacity: 1.0, blend: Blend::Screen }, clip, Op::PopLayer];
    let svg = to_svg(&DisplayList { width: 10.0, height: 10.0, background: RED, ops }, &NoGlyphs);
    well_formed(&svg).unwrap_or_else(|e| panic!("{e}\n{svg}"));
    assert!(svg.contains("mix-blend-mode:screen"));
}

#[test]
fn well_formedness_checker_rejects_broken_xml() {
    assert!(well_formed("<a><b></a></b>").is_err());
    assert!(well_formed("<a x=\"1\" x=\"2\"/>").is_err());
    assert!(well_formed("<a x=\"<\"/>").is_err());
    assert!(well_formed("<a>&nope;</a>").is_err());
    assert!(well_formed("<a/><b/>").is_err());
    assert!(well_formed("<a><b/>").is_err());
    assert!(well_formed("<a x=\"1 &amp; 2\"><b/>text</a>").is_ok());
}
