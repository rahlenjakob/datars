//! Icons and pictures as chart material: where they come from (the default set, files, theme
//! tokens, inline), how they're painted (inks, never the file's colours), how many of one draw
//! (instances), what assistive technology hears (decorative unless labelled), and text along a
//! path.

use datars_engine::{Engine, Request};
use datars_scene::{Geom, IconInk, Node, NodeKind, Paint, Proto, Role};
use datars_theme::Ink;

const LOGO: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 10"><rect width="20" height="10" fill="#e30613"/><circle cx="10" cy="5" r="3" fill="#ffffff"/></svg>"##;

fn load(scene: serde_json::Value) -> Engine {
    let doc = serde_json::json!({ "datars": 1, "size": { "width": 400, "height": 200 },
        "data": { "t": { "values": { "k": ["a", "b", "c"], "x": [20, 60, 100] }, "key": ["k"] } },
        "theme": { "use": "datars/neutral", "tokens": { "icon.up": "lucide:arrow-up" } },
        "scene": scene });
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    e
}

/// Answer icon requests as a host does: the default set from disk, `logo.svg` from memory.
fn serve(e: &mut Engine) {
    for r in e.requests() {
        let Request::Source { name, url } = r else { continue };
        let bytes = if url == "logo.svg" {
            Some(LOGO.as_bytes().to_vec())
        } else {
            datars_icon::set::bundled_svg(&url).map(String::into_bytes)
        };
        match bytes {
            Some(b) => e.provide(&name, &b).unwrap(),
            None => e.asset_failed(&name, "not found"),
        }
    }
}

fn find<'a>(n: &'a Node, key: &str) -> Option<&'a Node> {
    if n.key.to_string().contains(key) {
        return Some(n);
    }
    n.children().iter().find_map(|c| find(c, key))
}

#[test]
fn an_icon_is_asked_for_then_drawn_in_its_ink() {
    let mut e = load(serde_json::json!({ "kind": "group", "children": [
        { "kind": "icon", "key": "h", "icon": "heart", "at": [50, 50], "icon_size": 24, "ink": "$accent" } ] }));
    // Named in the document: asked for as soon as it loads, with the default set's URL.
    let asks: Vec<(String, String)> = e.requests().into_iter().filter_map(|r| if let Request::Source { name, url } = r { Some((name, url)) } else { None }).collect();
    assert_eq!(asks, vec![("icon:lucide:heart".to_string(), "datars:icons/lucide/heart.json".to_string())]);
    // Not here yet: nothing drawn (no stand-in to jump from).
    assert!(find(&e.scene().root, "h").is_none());
    serve(&mut e);
    let scene = e.scene();
    let icon = find(&scene.root, "h").expect("drawn once it arrives");
    let NodeKind::Group { children } = &icon.kind else { panic!("{:?}", icon.kind) };
    let NodeKind::Shape { stroke: Some(s), fill, .. } = &children[0].kind else { panic!() };
    assert_eq!(s.paint, Paint::Solid(Ink::token("accent")), "currentColor is the node's ink");
    assert!(fill.is_none(), "Lucide strokes, it doesn't fill");
    assert!((s.width - 2.0).abs() < 1e-9, "2 of 24 units at 24 px");
    // Placed at its point: the box centred on (50, 50).
    let b = datars_engine::node_bounds(icon);
    assert!((b.center().x - 50.0).abs() < 1.0 && (b.center().y - 50.0).abs() < 2.0, "{b:?}");
    assert!(e.requests().is_empty());
}

#[test]
fn a_logo_draws_its_colours_in_the_inks_passed_never_its_hex() {
    let mut e = load(serde_json::json!({ "kind": "group", "children": [
        { "kind": "icon", "key": "a", "icon": "logo.svg", "at": [50, 50], "icon_size": 40, "inks": ["$negative"] } ] }));
    serve(&mut e);
    let scene = e.scene();
    let NodeKind::Group { children } = &find(&scene.root, "a").unwrap().kind else { panic!() };
    let fills: Vec<Paint> = children.iter().filter_map(|c| if let NodeKind::Shape { fill: Some(f), .. } = &c.kind { Some(f.clone()) } else { None }).collect();
    // Slot 0 (the file's red) as passed; slot 1 (its white) from the palette.
    assert_eq!(fills, vec![Paint::Solid(Ink::token("negative")), Paint::Solid(Ink::palette("categorical", 1))]);
    // Its box keeps the file's 2:1 aspect, the longer side 40.
    let b = datars_engine::node_bounds(find(&scene.root, "a").unwrap());
    assert!((b.w - 40.0).abs() < 0.5 && (b.h - 20.0).abs() < 0.5, "{b:?}");
}

#[test]
fn theme_tokens_inline_paths_and_markup_name_icons_too() {
    let mut e = load(serde_json::json!({ "kind": "group", "children": [
        { "kind": "icon", "key": "tok", "icon": "$icon.up", "at": [20, 20] },
        { "kind": "icon", "key": "path", "icon": { "d": "M0 0L10 5L0 10Z" }, "at": [60, 20], "ink": "$mark" },
        { "kind": "icon", "key": "svg", "icon": "<svg viewBox='0 0 4 4'><rect width='4' height='4'/></svg>", "at": [100, 20] } ] }));
    let names: Vec<String> = e.requests().into_iter().filter_map(|r| if let Request::Source { name, .. } = r { Some(name) } else { None }).collect();
    assert_eq!(names, vec!["icon:lucide:arrow-up".to_string()], "the token names its icon; inline ones need nothing");
    serve(&mut e);
    let scene = e.scene();
    for k in ["tok", "path", "svg"] {
        assert!(find(&scene.root, k).is_some(), "{k}");
    }
}

#[test]
fn icons_are_decorative_unless_labelled() {
    let mut e = load(serde_json::json!({ "kind": "group", "children": [
        { "kind": "icon", "key": "plain", "icon": "heart", "at": [20, 20] },
        { "kind": "icon", "key": "said", "icon": "arrow-up", "at": [60, 20], "semantics": { "role": "label", "label": "Rising" } } ] }));
    serve(&mut e);
    let scene = e.scene();
    assert_eq!(find(&scene.root, "plain").unwrap().semantics.as_ref().map(|s| s.role), Some(Role::Decoration));
    let said: Vec<String> = e.semantics().into_iter().map(|(_, l, _)| l).collect();
    assert!(said.contains(&"Rising".to_string()) && said.iter().all(|l| l.is_empty() || l == "Rising"), "{said:?}");
    let marks = e.icon_marks(0);
    assert_eq!(marks.iter().map(|m| m.label.clone()).collect::<Vec<_>>(), vec![None, Some("Rising".to_string())]);
}

#[test]
fn a_symbol_can_be_an_icon_and_built_in_names_stay_symbols() {
    let mut e = load(serde_json::json!({ "kind": "group", "children": [
        { "kind": "shape", "key": "star", "geom": { "type": "symbol", "symbol": "star", "x": 20, "y": 20, "size": 8 }, "fill": "$mark" },
        { "kind": "shape", "key": "lstar", "geom": { "type": "symbol", "symbol": "lucide:star", "x": 60, "y": 20, "size": 8 }, "fill": "$mark" } ] }));
    serve(&mut e);
    let scene = e.scene();
    assert!(matches!(&find(&scene.root, "\"star\"").unwrap().kind, NodeKind::Shape { geom: Geom::Symbol { .. }, .. }));
    let NodeKind::Group { children } = &find(&scene.root, "lstar").unwrap().kind else { panic!() };
    assert!(matches!(&children[0].kind, NodeKind::Shape { stroke: Some(s), .. } if s.paint == Paint::Solid(Ink::token("mark"))), "in the shape's fill");
}

#[test]
fn many_of_one_icon_are_one_instances_node() {
    let mut e = load(serde_json::json!({ "kind": "instances", "key": "people", "from": "t", "proto": "icon", "icon": "user", "x": "=d.x", "y": 50, "r": 10, "fill": "$mark", "label": "=d.k" }));
    serve(&mut e);
    let scene = e.scene();
    let NodeKind::Instances(i) = &scene.root.kind else { panic!("{:?}", scene.root.kind) };
    let Proto::Icon { icon } = &i.proto else { panic!() };
    assert_eq!((i.len(), icon.name.as_ref()), (3, "lucide:user"));
    assert!(icon.layers.iter().all(|l| l.stroke.as_ref().is_some_and(|s| s.ink == IconInk::Current)));
    // The pointer finds an instance by its box.
    assert!(e.hit_test(60.0, 50.0).iter().any(|h| h.label.as_deref() == Some("b")), "{:?}", e.hit_test(60.0, 50.0));
    // Every backend draws its layers: the display list's ops for it.
    let list = e.display_list(&scene);
    let datars_render::Op::Instances(op) = &list.ops[0] else { panic!() };
    assert_eq!(op.icon_ops().len(), 3 * icon.layers.len());
}

#[test]
fn an_icon_marker_rides_the_end_of_a_line() {
    let mut e = load(serde_json::json!({ "kind": "shape", "key": "l", "geom": { "type": "segment", "x1": 0, "y1": 0, "x2": 100, "y2": 0 },
        "stroke": { "paint": "$ink", "width": 1 }, "markers": { "end": { "type": "icon", "icon": "lucide:plane", "size": 20 } } }));
    serve(&mut e);
    let scene = e.scene();
    let NodeKind::Shape { markers: Some(m), .. } = &scene.root.kind else { panic!() };
    assert!(matches!(&m.end, Some(datars_scene::Marker::Icon { size, .. }) if *size == 20.0));
    let b = datars_render::bounds(&e.display_list(&scene));
    assert!(b.x1() > 105.0, "the plane is centred on the end, past it: {b:?}");
}

#[test]
fn an_icon_that_cant_be_found_says_so_once() {
    let mut e = load(serde_json::json!({ "kind": "icon", "icon": "no-such-icon-anywhere", "at": [10, 10] }));
    serve(&mut e);
    let _ = e.scene();
    assert!(e.diagnostics().iter().any(|d| d.message.contains("no-such-icon-anywhere")), "{:?}", e.diagnostics());
    assert!(e.requests().is_empty(), "not asked for again");
}

#[test]
fn a_host_can_answer_on_the_spot() {
    let mut e = load(serde_json::json!({ "kind": "group", "children": [
        { "kind": "icon", "key": "h", "icon": "heart", "at": [20, 20] } ] }));
    e.set_asset_fetch(Box::new(|_, url| datars_icon::set::bundled_svg(url).map(String::into_bytes)));
    assert!(e.requests().is_empty());
    assert!(find(&e.scene().root, "h").is_some());
}

#[test]
fn a_document_may_carry_its_icons() {
    let icon = datars_icon::Icon::from_path("M0 0H10V10H0Z", None, None).unwrap();
    let doc = serde_json::json!({ "datars": 1, "icons": { "lucide:heart": serde_json::from_str::<serde_json::Value>(&icon.to_json()).unwrap() },
        "scene": { "kind": "icon", "key": "h", "icon": "heart", "at": [20, 20] } });
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    assert!(e.requests().is_empty(), "nothing to ask for");
    assert!(matches!(e.scene().root.kind, NodeKind::Group { .. }));
}

#[test]
fn text_follows_an_arc_and_a_text_too_long_for_it_is_left_out() {
    let mut e = load(serde_json::json!({ "kind": "group", "children": [
        { "kind": "text", "key": "on", "text": "Neutral", "at": [0, 0], "style": { "size": 12, "align": "middle", "baseline": "middle" }, "path": { "arc": [200, 150, 100, -1.6, 1.6] } },
        { "kind": "text", "key": "long", "text": "A name far longer than its little arc", "at": [0, 0], "style": { "size": 12 }, "path": { "arc": [200, 150, 100, -0.1, 0.1] } } ] }));
    let scene = e.scene();
    let NodeKind::Text(t) = &find(&scene.root, "on").unwrap().kind else { panic!() };
    let along = t.along.as_ref().unwrap();
    assert_eq!(along.len(), 7);
    // Centred on the top of the arc, turned with it.
    let mid = along[3];
    assert!((mid[0] as f64 - 200.0).abs() < 8.0 && (mid[1] as f64 - 50.0).abs() < 3.0, "{mid:?}");
    assert!(along[0][2] < -0.05 && along[6][2] > 0.05);
    let NodeKind::Text(long) = &find(&scene.root, "long").unwrap().kind else { panic!() };
    assert!(long.along.as_ref().unwrap().is_empty() && long.runs.is_empty());
    // A glyph at a time in the display list, each turned.
    let list = e.display_list(&scene);
    let turned = list.ops.iter().filter(|o| matches!(o, datars_render::Op::Glyphs { rotate, .. } if rotate.abs() > 0.05)).count();
    assert!(turned >= 4, "{turned}");
}

#[test]
fn a_picture_fits_its_rect_and_is_decorative_without_alt() {
    let mut png = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut png, 4, 2);
        enc.set_color(png::ColorType::Rgba);
        enc.write_header().unwrap().write_image_data(&[200u8; 32]).unwrap();
    }
    let mut e = load(serde_json::json!({ "kind": "group", "children": [
        { "kind": "image", "key": "c", "asset": "p.png", "rect": [0, 0, 100, 100], "fit": "contain" },
        { "kind": "image", "key": "v", "asset": "p.png", "rect": [0, 0, 100, 100], "fit": "cover", "semantics": { "role": "label", "label": "A grey card" } } ] }));
    assert!(e.requests().iter().any(|r| matches!(r, Request::Source { name, url } if name == "image:p.png" && url == "p.png")));
    e.provide("image:p.png", &png).unwrap();
    let scene = e.scene();
    let c = find(&scene.root, "\"c\"").unwrap();
    let NodeKind::Image { rect, .. } = &c.kind else { panic!() };
    assert_eq!((rect.w, rect.h, rect.y), (100.0, 50.0, 25.0), "contain: all of it, centred");
    assert_eq!(c.semantics.as_ref().map(|s| s.role), Some(Role::Decoration));
    let v = find(&scene.root, "\"v\"").unwrap();
    let NodeKind::Image { rect, .. } = &v.kind else { panic!() };
    assert_eq!((rect.w, rect.h), (200.0, 100.0), "cover: fills the rect…");
    assert!(v.common.clip.is_some(), "…cut to it");
    assert!(e.semantics().iter().any(|(_, l, _)| l == "A grey card"));
    let list = e.display_list(&scene);
    assert!(list.ops.iter().any(|o| matches!(o, datars_render::Op::Image { image: Some(i), .. } if i.width == 4)));
}

#[test]
fn a_picture_this_build_cant_decode_is_the_hosts_to_decode() {
    // A WebP: no Rust decoder here reads it, as none in the web runtime reads any picture.
    let file = b"RIFF\x10\0\0\0WEBPVP8 \0\0\0\0".to_vec();
    let mut e = load(serde_json::json!({ "kind": "image", "key": "w", "asset": "p.webp", "rect": [0, 0, 100, 100], "fit": "fill" }));
    e.provide("image:p.webp", &file).unwrap();
    // The file is held, and the host asked to decode it; nothing drawn meanwhile (no stand-in).
    assert!(e.requests().contains(&Request::Decode { name: "image:p.webp".into() }));
    assert_eq!(e.pictures_decoding(), 1);
    assert_eq!(e.picture_file("image:p.webp"), Some(&file[..]));
    assert!(find(&e.scene().root, "w").is_none());
    // The host's decoder hands back straight RGBA: drawn, premultiplied, named by its file.
    e.provide_pixels("image:p.webp", 2, 1, &[255, 0, 0, 255, 0, 0, 255, 128]).unwrap();
    assert_eq!(e.pictures_decoding(), 0);
    assert!(!e.requests().iter().any(|r| matches!(r, Request::Decode { .. })));
    let scene = e.scene();
    assert!(find(&scene.root, "w").is_some());
    let list = e.display_list(&scene);
    let img = list.ops.iter().find_map(|o| if let datars_render::Op::Image { image: Some(i), .. } = o { Some(i.clone()) } else { None }).unwrap();
    assert_eq!((img.width, img.texel(1, 0)), (2, [0, 0, 128, 128]));
    assert_eq!(&img.encoded[..], &file[..], "exports embed the file as shipped");
    // A decoder that can't read it says why, once.
    let mut e = load(serde_json::json!({ "kind": "image", "key": "w", "asset": "q.webp", "rect": [0, 0, 10, 10] }));
    e.provide("image:q.webp", &file).unwrap();
    e.asset_failed("image:q.webp", "the platform's decoder couldn't read the picture");
    assert!(e.requests().is_empty());
    let _ = e.scene();
    assert!(e.diagnostics().iter().any(|d| d.message.contains("couldn't read the picture")));
}

#[test]
fn an_icon_file_may_be_its_compact_form() {
    // What the web runtime fetches for the default set (`icons/lucide/heart.json`): read, not SVG.
    let mut e = load(serde_json::json!({ "kind": "icon", "key": "h", "icon": "heart", "at": [20, 20] }));
    let svg = datars_icon::set::bundled_svg("datars:icons/lucide/heart.json").unwrap();
    let compact = datars_icon::parse_svg(&svg).unwrap().to_json();
    e.provide("icon:lucide:heart", compact.as_bytes()).unwrap();
    assert!(e.requests().is_empty());
    let drawn = e.icons_drawn();
    assert_eq!(drawn.len(), 1);
    assert_eq!(*drawn[0].1, datars_icon::parse_svg(&svg).unwrap(), "the same drawing as the SVG read");
}

#[test]
fn inline_markup_ships_read_as_a_drawn_icon() {
    // A bundle (or `datars dev`) hands the web runtime every drawing read, inline ones included.
    let mut e = load(serde_json::json!({ "kind": "icon", "key": "i", "icon": LOGO, "at": [20, 20] }));
    let _ = e.scene();
    let drawn = e.icons_drawn();
    assert_eq!(drawn.len(), 1);
    assert!(drawn[0].0.starts_with("inline:"), "{}", drawn[0].0);
    // Given back as the document's `icons`, it draws without reading markup.
    let doc = serde_json::json!({ "datars": 1, "icons": { drawn[0].0.clone(): serde_json::from_str::<serde_json::Value>(&drawn[0].1.to_json()).unwrap() },
        "scene": { "kind": "icon", "key": "i", "icon": LOGO, "at": [20, 20] } });
    let mut again = Engine::new();
    again.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    assert!(find(&again.scene().root, "i").is_some());
    assert_eq!(again.icons_drawn(), drawn);
}
