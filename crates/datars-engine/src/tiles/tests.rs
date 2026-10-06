//! The engine with tiles: sans-IO requests, synchronous fetching, filling settled scenes and
//! flight frames, fallbacks, batching, labels, other projections, determinism.

use super::testutil;
use crate::{Engine, Request};
use datars_scene::{Key, KeyPart, Node, NodeKind, Scene};
use std::rc::Rc;

fn doc(projection: &str, extra_layer: &str) -> String {
    format!(
        r#"{{"datars": 1, "size": {{"width": 512, "height": 512}},
        "data": {{"base": {{"tiles": "test.pmtiles"}}, "pts": {{"values": {{"id": ["a"], "lon": [18.07], "lat": [59.33]}}, "key": ["id"]}}}},
        "signals": {{"bounds": {{"type": "keyset", "default": [-180, -80, 180, 80]}}}},
        "scene": {{"kind": "view", "key": "map", "clip": true,
          "coord": {{"type": "geo", "projection": "{projection}", "fit": {{"bbox": [-180, -85.05, 180, 85.05]}}, "padding": 0}},
          "camera": {{"fit": {{"geo": "=bounds"}}}},
          "children": [
            {{"kind": "tiles", "key": "basemap", "source": "base", "layers": [
              {{"layer": "water", "template": {{"kind": "shape", "geom": {{"type": "feature"}}, "fill": "$map.water"}}}},
              {{"layer": "roads", "template": {{"kind": "shape", "geom": {{"type": "feature"}},
                 "stroke": {{"paint": "=d.kind == 'major' ? '$map.road-major' : '$map.road'", "width": "=tile.zoom >= 3 ? 2 : 1", "non_scaling": true}}}}}},
              {{"layer": "places", "labels": true, "priority": "=d.pop",
                 "template": {{"kind": "text", "key": "=d.name", "text": "=d.name", "at": ["=d.$x", "=d.$y"], "style": {{"size": 12}}}}}}
              {extra_layer}
            ]}},
            {{"kind": "instances", "key": "dots", "from": "pts", "x": "=geo.x(d.lon, d.lat)", "y": "=geo.y(d.lon, d.lat)", "r": 4, "screen_size": true}}
          ]}},
        "program": {{"states": [{{"name": "world"}}, {{"name": "sweden", "set": {{"bounds": [10, 55, 25, 66]}}}}]}}}}"#
    )
}

fn load(json: &str) -> Engine {
    let mut e = Engine::new();
    let d = e.load(datars_ir::Doc::from_json(json).unwrap());
    assert!(d.is_empty(), "{d:?}");
    e
}

fn with_fetch(json: &str, maxzoom: u8) -> Engine {
    let mut e = load(json);
    let bytes = Rc::new(testutil::archive(maxzoom));
    e.set_range_fetch(Box::new(move |url, off, len| {
        assert_eq!(url, "test.pmtiles");
        let end = ((off + len) as usize).min(bytes.len());
        Some(bytes[off as usize..end].to_vec())
    }));
    e
}

fn find<'a>(n: &'a Node, key: &Key) -> Option<&'a Node> {
    if &n.key == key {
        return Some(n);
    }
    n.children().iter().find_map(|c| find(c, key))
}

fn basemap(s: &Scene) -> &Node {
    find(&s.root, &Key::name("basemap")).expect("placeholder")
}

/// Tile groups under the placeholder (not the labels group).
fn tiles(s: &Scene) -> Vec<(i64, i64, i64)> {
    basemap(s)
        .children()
        .iter()
        .filter_map(|n| match n.key.parts() {
            [KeyPart::Int(z), KeyPart::Int(x), KeyPart::Int(y)] => Some((*z, *x, *y)),
            _ => None,
        })
        .collect()
}

fn labels(s: &Scene) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(l) = find(&basemap(s).clone(), &Key::name("labels")) {
        l.children().iter().flat_map(|g| g.children()).for_each(|n| {
            if let NodeKind::Text(t) = &n.kind {
                out.push(t.text.clone());
            }
        });
    }
    out
}

#[test]
fn ranges_are_requested_then_provided_sans_io() {
    let mut e = load(&doc("web-mercator", ""));
    let archive = testutil::archive(3);
    // Loading asks for the header right away.
    assert_eq!(e.requests(), vec![Request::Range { name: "base".into(), url: "test.pmtiles".into(), offset: 0, length: 16384 }]);
    let f = e.frame(0.0);
    assert_eq!(f.pending_tiles, 1, "no header yet: nothing to draw");
    assert!(basemap(&f.scene).children().is_empty());
    // Serve requests until none are left, like a host would.
    for _ in 0..6 {
        let reqs = e.requests();
        if reqs.is_empty() {
            break;
        }
        for r in reqs {
            let Request::Range { name, offset, length, .. } = r else { panic!("{r:?}") };
            let end = ((offset + length) as usize).min(archive.len());
            e.provide_range(&name, offset, &archive[offset as usize..end]).unwrap();
        }
        e.frame(0.0);
    }
    let f = e.frame(0.0);
    assert_eq!(f.pending_tiles, 0);
    assert_eq!(tiles(&f.scene), vec![(0, 0, 0)], "the world fits one z0 tile at 512 px");
    let st = e.tile_stats();
    assert!(st.decoded >= 1 && st.built >= 1 && st.requests >= 1 && st.bytes > 0, "{st:?}");
}

#[test]
fn settled_states_fill_at_their_zoom_with_batched_layers() {
    let mut e = with_fetch(&doc("web-mercator", ""), 3);
    let world = e.scene_for_state(0);
    assert_eq!(tiles(&world), vec![(0, 0, 0)]);
    let sweden = e.scene_for_state(1);
    let ts = tiles(&sweden);
    assert!(!ts.is_empty() && ts.iter().all(|t| t.0 == 3), "z3 is the archive's max: {ts:?}");
    // Each tile draws one group per layer; features of one style batch into one path.
    let t = &basemap(&sweden).children()[0];
    let roads = t.children().iter().find(|n| n.key == Key::name("roads"));
    if let Some(roads) = roads {
        assert!(roads.children().len() <= 2, "two road classes → at most two batches");
        assert!(roads.children().iter().all(|n| matches!(n.kind, NodeKind::Shape { .. })));
    }
    // Tiles sit in content space by a transform, clipped to their square.
    assert!(!t.common.transform.is_identity());
    assert!(matches!(t.common.clip, Some(datars_scene::Clip::Rect { .. })));
    // `tile.zoom` reached the templates: stroke width 2 at z≥3.
    let any_road = basemap(&sweden).children().iter().flat_map(|t| t.children()).find(|n| n.key == Key::name("roads")).expect("roads drawn");
    let NodeKind::Shape { stroke: Some(s), .. } = &any_road.children()[0].kind else { panic!() };
    assert_eq!(s.width, 2.0);
    assert!(e.diagnostics().is_empty(), "{:?}", e.diagnostics());
}

#[test]
fn labels_are_placed_by_priority_without_overlap() {
    let mut e = with_fetch(&doc("web-mercator", ""), 3);
    let sweden = e.scene_for_state(1);
    let l = labels(&sweden);
    // Stockholm (pop 1M) and Lidingö (40k) are a few px apart at z3: only Stockholm is placed.
    assert!(l.contains(&"Stockholm".to_string()), "{l:?}");
    assert!(!l.contains(&"Lidingö".to_string()), "{l:?}");
    assert!(l.contains(&"Göteborg".to_string()), "{l:?}");
    assert_eq!(l.iter().filter(|x| *x == "Stockholm").count(), 1, "buffered copies in neighbour tiles are dropped");
}

/// The text layer (the selectable text a web page lays over the canvas) has the place names tiles
/// put on the map, marked as such.
#[test]
fn the_text_layer_marks_place_names() {
    let mut e = with_fetch(&doc("web-mercator", ""), 3);
    let sweden = e.scene_for_state(1);
    let texts = e.text_layer(&sweden);
    let stockholm = texts.iter().find(|t| t.text == "Stockholm").expect("placed labels are in the layer");
    assert!(stockholm.place && stockholm.path.contains("(\"labels\",)"), "{stockholm:?}");
    assert!(!texts.iter().any(|t| t.text == "Lidingö"), "only what was placed");
    assert_eq!((stockholm.size, stockholm.lines.len()), (12.0, 1));
}

#[test]
fn flight_frames_fill_intermediate_zooms() {
    let mut e = with_fetch(&doc("web-mercator", ""), 3);
    let (sa, sb, plan) = e.plan_states(0, 1);
    assert_eq!(e.plan_at(&plan, 0.0), sa, "frames at the ends are the settled scenes");
    assert_eq!(e.plan_at(&plan, 1.0), sb);
    let zooms: Vec<i64> = (0..=10).map(|i| tiles(&e.plan_at(&plan, i as f64 / 10.0)).first().map(|t| t.0).unwrap_or(-1)).collect();
    assert_eq!(zooms[0], 0);
    assert_eq!(zooms[10], 3);
    assert!(zooms.iter().any(|z| *z == 1 || *z == 2), "intermediate zooms along the flight: {zooms:?}");
    assert!(zooms.windows(2).all(|w| w[1] >= w[0]), "zooming in only: {zooms:?}");
}

#[test]
fn pending_tiles_draw_a_cached_ancestor() {
    let mut e = load(&doc("web-mercator", ""));
    let archive = testutil::archive(3);
    let serve = |e: &mut Engine| {
        for r in e.requests() {
            let Request::Range { name, offset, length, .. } = r else { continue };
            let end = ((offset + length) as usize).min(archive.len());
            e.provide_range(&name, offset, &archive[offset as usize..end]).unwrap();
        }
    };
    // The world state: z0 arrives.
    for _ in 0..4 {
        e.frame(0.0);
        serve(&mut e);
    }
    assert_eq!(tiles(&e.frame(0.0).scene), vec![(0, 0, 0)]);
    // Jump to Sweden (the flight's first frame, then long after): z3 isn't there yet, so z0
    // stands in, clipped per square.
    e.goto(1);
    e.frame(1e6);
    let f = e.frame(2e6);
    assert!(f.pending_tiles > 0);
    let g = &basemap(&f.scene).children()[0];
    assert!(matches!(g.key.parts(), [KeyPart::Int(3), ..]), "keyed by the square it covers");
    assert_eq!(g.common.transform, datars_geo::Projection::web_mercator().fit_extent(datars_geo::Fit::Bbox(datars_geo::GeoBbox::new(-180.0, -85.05, 180.0, 85.05)), datars_math::Rect::new(0.0, 0.0, 512.0, 512.0)).tile_transform(datars_geo::TileId::new(0, 0, 0), 4096.0).unwrap(), "drawn with z0's data");
    serve(&mut e);
    let f = e.frame(1e6);
    assert_eq!(f.pending_tiles, 0);
}

/// Finer data arriving for a square fades in over what the square drew before (its ancestor's
/// data, overzoomed), keeping frames coming while it does — no pop. Then the stand-in goes.
#[test]
fn finer_data_fades_in_over_its_stand_in() {
    let mut e = load(&doc("web-mercator", ""));
    let archive = testutil::archive(3);
    let serve = |e: &mut Engine| {
        for r in e.requests() {
            let Request::Range { name, offset, length, .. } = r else { continue };
            let end = ((offset + length) as usize).min(archive.len());
            e.provide_range(&name, offset, &archive[offset as usize..end]).unwrap();
        }
    };
    for _ in 0..4 {
        e.frame(0.0);
        serve(&mut e);
    }
    e.goto(1);
    e.frame(10.0);
    let f = e.frame(20.0);
    assert!(f.pending_tiles > 0, "z0 stands in for z3");
    serve(&mut e);
    let under = |f: &crate::FrameOutput| basemap(&f.scene).children().iter().filter(|n| matches!(n.key.parts().first(), Some(KeyPart::Str(s)) if s.as_ref() == "under")).count();
    let opacity = |f: &crate::FrameOutput| basemap(&f.scene).children().iter().find(|n| matches!(n.key.parts(), [KeyPart::Int(3), ..])).map(|n| n.common.opacity);
    let f = e.frame(20.1);
    assert!(under(&f) > 0 && f.animating, "the z0 stand-in stays under the arriving z3 data");
    assert_eq!(opacity(&f), Some(0.0));
    let f = e.frame(20.1 + super::fill::FADE_S / 2.0);
    assert!(under(&f) > 0);
    assert!((opacity(&f).unwrap() - 0.5).abs() < 1e-9, "halfway (smoothstep)");
    let f = e.frame(20.2 + super::fill::FADE_S);
    assert_eq!(under(&f), 0, "faded in: the stand-in is gone");
    assert_eq!(opacity(&f), Some(1.0));
}

/// Squares new in view replace what was drawn there gradually: zooming in, finer data fades in
/// over what the ancestor square drew (which covers it whole); zooming out, the coarser square
/// draws at once and the finer squares it replaces fade out on top of it — so nothing ever shows
/// through. No level pops in or vanishes during a flight, and the landed view is whole and opaque.
#[test]
fn squares_new_after_a_zoom_replace_what_was_there_gradually() {
    let mut e = with_fetch(&doc("web-mercator", ""), 3);
    for _ in 0..3 {
        e.frame(0.0);
    }
    let tagged = |n: &Node, tag: &str| match n.key.parts() {
        [KeyPart::Str(s), KeyPart::Int(z), KeyPart::Int(x), KeyPart::Int(y)] if s.as_ref() == tag => Some(datars_geo::TileId::new(*z as u8, *x as u32, *y as u32)),
        _ => None,
    };
    let square = |n: &Node| match n.key.parts() {
        [KeyPart::Int(z), KeyPart::Int(x), KeyPart::Int(y)] => Some(datars_geo::TileId::new(*z as u8, *x as u32, *y as u32)),
        _ => None,
    };
    let within = |t: datars_geo::TileId, a: datars_geo::TileId| t.z >= a.z && t.ancestor(t.z - a.z) == a;
    let mut t = 0.0;
    for state in [1, 0] {
        e.set_clock(t);
        e.goto(state);
        let (mut fading, mut shared) = (0, 0);
        let end = t + 4.0;
        while t < end {
            t += 1.0 / 60.0;
            let f = e.frame(t);
            let kids = basemap(&f.scene).children();
            for (i, n) in kids.iter().enumerate() {
                // A square is faint only over data covering it whole: what was drawn for it or
                // for an ancestor, never finer squares that may leave gaps.
                if let (Some(sq), true) = (square(n), n.common.opacity < 1.0) {
                    assert!(kids[..i].iter().any(|c| tagged(c, "under").is_some_and(|u| within(sq, u))), "t {t:.2}: {sq:?} fades in with nothing whole under it");
                }
                if let Some(u) = tagged(n, "under") {
                    fading += 1;
                    // Under squares fading in over it; drawn for an ancestor, under all its
                    // squares at once (and only when they all fade over it).
                    let inside: Vec<&Node> = kids.iter().filter(|c| square(c).is_some_and(|sq| within(sq, u))).collect();
                    assert!(inside.iter().any(|c| c.common.opacity < 1.0), "t {t:.2}: something fades in over {u:?}");
                    if inside.len() > 1 {
                        shared += 1;
                        assert!(inside.iter().all(|c| c.common.opacity < 1.0), "t {t:.2}: {u:?} is shared only by squares all fading over it");
                    }
                }
                if let Some(o) = tagged(n, "over") {
                    fading += 1;
                    // Fading out: above the opaque new square that covers it.
                    let covered = kids[..i].iter().any(|c| square(c).is_some_and(|c_sq| c_sq.z < o.z && within(o, c_sq)) && c.common.opacity == 1.0);
                    assert!(covered, "t {t:.2}: {o:?} fades out over an opaque square covering it");
                }
            }
        }
        assert!(fading > 0, "the flight to state {state} crossfades");
        if state == 1 {
            assert!(shared > 0, "a level crossed draws the parent once under its children");
        }
        t += 1.0;
        let f = e.frame(t);
        let kids = basemap(&f.scene).children();
        assert!(!kids.iter().any(|n| tagged(n, "under").is_some() || tagged(n, "over").is_some()), "landed on state {state}: nothing left of before");
        assert!(kids.iter().filter(|n| square(n).is_some()).all(|n| n.common.opacity == 1.0));
    }
}

/// A frame that decodes a tile bigger than its decode budget doesn't also style one bigger than
/// its styling budget: the two take turns (each alone is the frame's one-tile allowance).
#[test]
fn big_tiles_are_decoded_and_styled_in_different_frames() {
    // Dense enough that the finest tiles (z3, ~900 roads, ~16 KB) are over both budgets below.
    let bytes = Rc::new(testutil::dense_archive(3, 340));
    let mut e = load(&doc("web-mercator", ""));
    let b = bytes.clone();
    e.set_range_fetch(Box::new(move |_, off, len| {
        let end = ((off + len) as usize).min(b.len());
        Some(b[off as usize..end].to_vec())
    }));
    // A slow device's share: every dense tile is over both budgets.
    e.set_work_scale(0.1);
    let (decode, style) = (((super::DECODE_PER_FRAME as f64 * 0.1) as u64).max(4096), ((super::BUILD_FEATURES_PER_FRAME as f64 * 0.1) as u64).max(500));
    let mut now = 0.0;
    let (mut heavy_decodes, mut heavy_styles) = (0, 0);
    for step in 0..2 {
        if step == 1 {
            e.set_clock(now);
            e.goto(1);
        }
        for _ in 0..240 {
            now += 1.0 / 60.0;
            let before = e.tile_stats();
            e.frame(now);
            let after = e.tile_stats();
            let decoded = after.decoded_bytes - before.decoded_bytes;
            let styled = after.built_features - before.built_features;
            // (The first tile of all has nothing to stand in for it: that frame does both.)
            if before.built == 0 {
                continue;
            }
            heavy_decodes += (decoded > decode) as u32;
            heavy_styles += (styled > style) as u32;
            assert!(!(decoded > decode && styled > style), "t {now:.2}: {decoded} bytes decoded and {styled} features styled in one frame");
        }
    }
    assert!(heavy_decodes > 0 && heavy_styles > 0, "the test meets big tiles ({heavy_decodes} decodes, {heavy_styles} stylings)");
}

/// Dense tiles: a flight styles its new tiles over several frames — never more features in one
/// than the budget (bar a single tile bigger than it) — and every frame still covers the view
/// (squares not styled yet show a nearby zoom's or an ancestor's nodes); it lands fully styled.
#[test]
fn styling_is_spread_over_frames_by_features_and_covers_the_view() {
    let bytes = Rc::new(testutil::dense_archive(3, 240));
    let engine = || {
        let mut e = load(&doc("web-mercator", ""));
        let bytes = bytes.clone();
        e.set_range_fetch(Box::new(move |_, off, len| {
            let end = ((off + len) as usize).min(bytes.len());
            Some(bytes[off as usize..end].to_vec())
        }));
        e
    };
    let mut e = engine();
    let mut now = 0.0;
    for _ in 0..60 {
        now += 1.0 / 60.0;
        e.frame(now);
    }
    // What the flight shows, from stills (no budget) in an engine of their own: styling them here
    // would leave nothing for the frames to style.
    let mut reference = engine();
    let (_, settled, plan) = reference.plan_states(0, 1);
    e.set_clock(now);
    e.goto(1);
    let (mut frames, mut styling_frames, mut deferred) = (0, 0, 0);
    loop {
        now += 1.0 / 60.0;
        let before = e.tile_stats();
        let f = e.frame(now);
        let after = e.tile_stats();
        let (tiles_styled, features) = (after.built - before.built, after.built_features - before.built_features);
        assert!(features <= super::BUILD_FEATURES_PER_FRAME as u64 || tiles_styled == 1, "frame {frames}: {tiles_styled} tiles, {features} features styled");
        styling_frames += (tiles_styled > 0) as u32;
        // The squares drawn are the view's squares at this point of the flight, whatever stands in.
        if let Some(a) = &e.active {
            let t = ((e.clock - a.start) / a.duration).clamp(0.0, 1.0);
            let still = reference.plan_at(&plan, t);
            assert_eq!(tiles(&f.scene), tiles(&still), "frame {frames} (t {t:.2}): the view is covered");
            deferred += (f.pending_tiles > 0 || after.last_fallback > 0) as u32;
        }
        frames += 1;
        if !f.animating || frames > 600 {
            break;
        }
    }
    assert!(styling_frames > 2, "the styling spread over frames ({styling_frames})");
    assert!(deferred > 0, "some squares waited, an ancestor or a nearby zoom standing in");
    now += 1.0 / 60.0;
    let f = e.frame(now);
    assert_eq!(tiles(&f.scene), tiles(&settled), "landed on the state's own tiles");
    assert_eq!(f.pending_tiles, 0);
}

#[test]
fn other_projections_reproject_once_per_tile() {
    let mut e = with_fetch(&doc("equal-earth", ""), 3);
    let s = e.scene_for_state(1);
    let t = &basemap(&s).children()[0];
    assert!(t.common.transform.is_identity(), "geometry already in content units");
    assert!(matches!(t.common.clip, Some(datars_scene::Clip::Path { .. })));
    assert!(!tiles(&s).is_empty());
}

#[test]
fn fills_are_deterministic_and_cached() {
    let mut a = with_fetch(&doc("web-mercator", ""), 3);
    let mut b = with_fetch(&doc("web-mercator", ""), 3);
    let s1 = a.scene_for_state(1);
    let built = a.tile_stats().built;
    let s2 = a.scene_for_state(1);
    assert_eq!(a.tile_stats().built, built, "built tiles are reused");
    assert_eq!(s1, s2);
    assert_eq!(s1.hash(), b.scene_for_state(1).hash());
    // A theme change rebuilds (sizes are theme numbers) and draws the same scene.
    a.set_mode(datars_theme::Mode::Dark);
    assert_eq!(a.scene_for_state(1), s1);
}

#[test]
fn whole_archives_and_filters() {
    let extra = r#",{"layer": "roads", "id": "majors", "filter": "=d.kind == 'major'", "minzoom": 2, "merge": false,
        "template": {"kind": "shape", "geom": {"type": "feature"}, "stroke": {"paint": "$ink", "width": 1}}}"#;
    let mut e = load(&doc("web-mercator", extra));
    e.provide("base", &testutil::archive(3)).unwrap();
    assert!(e.requests().is_empty(), "nothing to fetch: the archive is in memory");
    let s = e.scene_for_state(1);
    let majors: Vec<&Node> = basemap(&s).children().iter().flat_map(|t| t.children()).filter(|n| n.key == Key::name("majors")).collect();
    assert!(!majors.is_empty());
    // Unbatched: one node per feature, only the major roads.
    assert!(majors.iter().all(|m| m.children().len() <= 2));
    let world = e.scene_for_state(0);
    assert!(basemap(&world).children().iter().flat_map(|t| t.children()).all(|n| n.key != Key::name("majors")), "minzoom 2");
}

#[test]
fn unknown_tile_sources_are_diagnosed() {
    let bad = doc("web-mercator", "").replace(r#""source": "base""#, r#""source": "pts""#);
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&bad).unwrap());
    let _ = e.scene();
    assert!(e.diagnostics().iter().any(|d| d.message.contains("not a tiles source")), "{:?}", e.diagnostics());
}

#[test]
fn sessions_with_tile_ranges_replay_bit_for_bit() {
    let mut e = load(&doc("web-mercator", ""));
    e.start_recording();
    let archive = testutil::archive(3);
    let mut t = 0.0;
    for step in 0..2 {
        if step == 1 {
            e.set_clock(t);
            e.event("next");
        }
        for _ in 0..6 {
            t += 0.5;
            e.frame(t);
            for r in e.requests() {
                let Request::Range { name, offset, length, .. } = r else { continue };
                let end = ((offset + length) as usize).min(archive.len());
                e.provide_range(&name, offset, &archive[offset as usize..end]).unwrap();
            }
        }
    }
    let last = e.frame(t + 10.0).hash();
    let session = e.take_recording().unwrap();
    assert!(session.inputs.iter().any(|i| matches!(i, crate::session::Input::Range { .. })));
    assert!(session.inputs.iter().any(|i| matches!(i, crate::session::Input::Clock { .. })));
    let mut replayed = Engine::replay(&session, |_, _| {}).unwrap();
    assert_eq!(replayed.frame(t + 10.0).hash(), last);
}

/// Label opacities along a frame (text → opacity).
fn label_opacity(s: &Scene) -> Vec<(String, f64)> {
    let mut out = Vec::new();
    if let Some(l) = find(&basemap(s).clone(), &Key::name("labels")) {
        l.children().iter().flat_map(|g| g.children()).for_each(|n| {
            if let NodeKind::Text(t) = &n.kind {
                out.push((t.text.to_string(), n.common.opacity));
            }
        });
    }
    out
}

#[test]
fn labels_fade_during_flights_as_a_function_of_the_plan() {
    let mut e = with_fetch(&doc("web-mercator", ""), 6);
    let (sa, sb, plan) = e.plan_states(0, 1);
    for s in [&sa, &sb] {
        assert!(label_opacity(s).iter().all(|(_, o)| *o == 1.0), "settled scenes show labels fully");
    }
    let frames: Vec<Scene> = (1..40).map(|i| e.plan_at(&plan, i as f64 / 40.0)).collect();
    let partial = frames.iter().flat_map(label_opacity).filter(|(_, o)| *o > 0.0 && *o < 1.0).count();
    assert!(partial > 0, "some labels fade in or out on the way");
    // Opacity depends on the plan and t only: the same frame, however it's reached.
    let again = e.plan_at(&plan, 17.0 / 40.0);
    assert_eq!(again, frames[16]);
    let mut fresh = with_fetch(&doc("web-mercator", ""), 6);
    let (_, _, plan2) = fresh.plan_states(0, 1);
    assert_eq!(fresh.plan_at(&plan2, 17.0 / 40.0), frames[16], "no hidden frame history");
    // The last frames meet the settled scene: nothing left half-faded at the end.
    let end = e.plan_at(&plan, 0.999);
    assert!(label_opacity(&end).iter().all(|(_, o)| *o > 0.95), "{:?}", label_opacity(&end));
}

#[test]
fn tile_views_cover_states_and_the_flight_between_them() {
    let mut e = load(&doc("web-mercator", ""));
    let views = e.tile_views(15);
    assert!(views.iter().all(|v| v.source == "base" && !v.explore));
    let settled: Vec<&crate::TileView> = views.iter().filter(|v| !v.state.is_empty()).collect();
    assert_eq!(settled.iter().map(|v| v.state.as_str()).collect::<Vec<_>>(), vec!["world", "sweden"]);
    // The world fills the 512 px view at zoom 0; Sweden (15° × 11°) at about zoom 3.4.
    assert!(settled[0].zoom.abs() < 0.05, "{:?}", settled[0]);
    assert!((3.0..4.0).contains(&settled[1].zoom), "{:?}", settled[1]);
    let b = settled[1].bbox;
    assert!(b[0] <= 10.0 && b[2] >= 25.0 && b[1] <= 55.0 && b[3] >= 66.0, "the view shows what its camera fits: {b:?}");
    // The flight passes the zooms between, and evaluating cameras asks for no tiles.
    let flight: Vec<f64> = views.iter().filter(|v| v.state.is_empty()).map(|v| v.zoom).collect();
    assert_eq!(flight.len(), 15);
    assert!(flight.iter().any(|z| (1.0..3.0).contains(z)), "{flight:?}");
    assert_eq!(e.tile_stats().requests, 0);
    assert!(e.diagnostics().is_empty(), "{:?}", e.diagnostics());
}

#[test]
fn explorable_views_are_marked() {
    let json = doc("web-mercator", "").replace(r#""camera": {"fit": {"geo": "=bounds"}}"#, r#""camera": {"fit": {"geo": "=bounds"}, "explore": "pan"}"#);
    let mut e = load(&json);
    let views = e.tile_views(3);
    assert!(views.iter().filter(|v| !v.state.is_empty()).all(|v| v.explore), "{views:?}");
    // What's on screen now: the first state, at the host's viewport.
    let now = e.tile_views_now();
    e.frame(0.0);
    let now2 = e.tile_views_now();
    assert!(now.len() <= 1 && now2.len() == 1 && now2[0].explore && now2[0].state == "world", "{now2:?}");
}

#[test]
fn auto_sources_read_the_archive_next_to_the_document() {
    let json = doc("web-mercator", "").replace(r#""tiles": "test.pmtiles""#, r#""tiles": "auto""#);
    let e = load(&json);
    assert_eq!(e.requests(), vec![Request::Range { name: "base".into(), url: "base.auto.pmtiles".into(), offset: 0, length: 16384 }]);
}

#[test]
fn a_new_archive_url_replaces_what_was_read() {
    let mut e = load(&doc("web-mercator", ""));
    let (deep, shallow) = (Rc::new(testutil::archive(3)), Rc::new(testutil::archive(1)));
    e.set_range_fetch(Box::new(move |url, off, len| {
        let b = if url == "test.pmtiles" { &deep } else { &shallow };
        Some(b[off as usize..((off + len) as usize).min(b.len())].to_vec())
    }));
    assert!(tiles(&e.scene_for_state(1)).iter().all(|t| t.0 == 3));
    // A rebuilt archive (here: only to z1) under a new URL: read afresh, nothing kept from the old.
    e.set_tiles_url("base", "rebuilt.pmtiles").unwrap();
    let ts = tiles(&e.scene_for_state(1));
    assert!(!ts.is_empty() && ts.iter().all(|t| t.0 == 1), "{ts:?}");
    assert!(e.set_tiles_url("nope", "x.pmtiles").is_err());
}

#[test]
fn idle_time_fetches_the_tiles_around_the_view() {
    // Settled on Sweden with every tile it shows: preparing in idle time asks for more — the tiles
    // just past the view's edges and a level coarser — before anyone pans or zooms.
    let mut e = load(&doc("web-mercator", ""));
    let archive = testutil::archive(3);
    let serve = |e: &mut Engine| {
        for r in e.requests() {
            let Request::Range { name, offset, length, .. } = r else { continue };
            let end = ((offset + length) as usize).min(archive.len());
            e.provide_range(&name, offset, &archive[offset as usize..end]).unwrap();
        }
    };
    e.goto(1);
    for k in 0..20 {
        e.frame(k as f64 * 10.0);
        serve(&mut e);
    }
    let ranges = |e: &Engine| e.requests().iter().filter(|r| matches!(r, Request::Range { .. })).count();
    assert_eq!(ranges(&e), 0, "settled, everything in view is in");
    let mut calls = 0;
    while e.prepare() {
        calls += 1;
        assert!(calls < 30, "preparing finishes");
    }
    assert!(ranges(&e) > 0, "the surroundings are asked for");
    // Asked for, not decoded: nothing drawn changes until they're shown.
    let before = e.tile_stats();
    serve(&mut e);
    assert_eq!(e.tile_stats().decoded, before.decoded);
}

#[test]
fn idle_warming_waits_for_the_next_steps_tiles() {
    // Settled on the world, the next step (Sweden) is prepared in idle time and its display list
    // handed to the renderer to warm. Its tiles are still downloading at first: that round doesn't
    // count as done — the step waits (no spinning) until bytes arrive, then warms with them.
    let mut e = load(&doc("web-mercator", ""));
    let archive = testutil::archive(3);
    let serve = |e: &mut Engine| {
        for r in e.requests() {
            let Request::Range { name, offset, length, .. } = r else { continue };
            let end = ((offset + length) as usize).min(archive.len());
            e.provide_range(&name, offset, &archive[offset as usize..end]).unwrap();
        }
    };
    for k in 0..20 {
        e.frame(k as f64 * 10.0);
        serve(&mut e);
    }
    let mut calls = 0;
    while e.prepare() {
        calls += 1;
        assert!(calls < 30, "preparing finishes");
    }
    let first = e.prepared_to_warm().expect("the next step to warm");
    let decoded = e.tile_stats().decoded;
    assert!(e.prepared_to_warm().is_none(), "its tiles are downloading: nothing to warm until they arrive");
    let mut rounds = 1;
    let mut last = first.clone();
    for _ in 0..40 {
        serve(&mut e);
        match e.prepared_to_warm() {
            Some(list) => {
                rounds += 1;
                last = list;
            }
            None if e.requests().is_empty() => break,
            None => {}
        }
    }
    assert!(rounds > 1, "warmed again once the tiles came");
    assert!(e.prepared_to_warm().is_none(), "and then done");
    assert!(e.tile_stats().decoded > decoded, "the destination's own tiles decoded while warming");
    assert!(last.ops != first.ops, "and drawn in what's warmed (the first round had a coarser tile standing in)");
}

#[test]
fn a_smaller_work_share_spreads_styling_over_more_frames() {
    // A slow device's host hands out a quarter of the budgets: a frame styles a quarter of the
    // features (or one tile bigger than that), and the flight still lands fully styled. Sessions
    // record the share like any input.
    let bytes = Rc::new(testutil::dense_archive(3, 240));
    let run = |share: f64| {
        let mut e = load(&doc("web-mercator", ""));
        let b = bytes.clone();
        e.set_range_fetch(Box::new(move |_, off, len| Some(b[off as usize..((off + len) as usize).min(b.len())].to_vec())));
        e.start_recording();
        e.set_work_scale(share);
        let mut now = 0.0;
        for _ in 0..60 {
            now += 1.0 / 60.0;
            e.frame(now);
        }
        e.set_clock(now);
        e.goto(1);
        let (mut styling_frames, mut most) = (0, 0u64);
        for _ in 0..600 {
            now += 1.0 / 60.0;
            let before = e.tile_stats();
            let f = e.frame(now);
            let after = e.tile_stats();
            let (tiles, features) = (after.built - before.built, after.built_features - before.built_features);
            assert!(features as f64 <= super::BUILD_FEATURES_PER_FRAME as f64 * share || tiles == 1, "share {share}: {tiles} tiles, {features} features");
            styling_frames += (tiles > 0) as u32;
            most = most.max(features);
            if !f.animating {
                break;
            }
        }
        let session = e.take_recording().expect("recording");
        assert!(session.inputs.iter().any(|i| matches!(i, crate::session::Input::Work { scale } if *scale == share)));
        (styling_frames, e.frame(now + 1.0).pending_tiles)
    };
    let (full, pending_full) = run(1.0);
    let (quarter, pending_quarter) = run(0.25);
    assert!(quarter > full, "more frames at a quarter share: {quarter} vs {full}");
    assert_eq!((pending_full, pending_quarter), (0, 0), "both land with every tile in");
}
