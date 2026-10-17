//! Intraday time: a time scale over millisecond timestamps (a trading session) ticks on hours and
//! minutes and labels them as times of day; a domain read from a table can be exact (09:30–16:00
//! stays 09:30–16:00); `table.first`/`table.last` read text too (a label from a one-row table).

use datars_engine::Engine;

const H: f64 = 3_600_000.0;
/// 2026-10-16 00:00 UTC.
const DAY: f64 = 1_792_108_800_000.0;

fn texts(doc: serde_json::Value) -> Vec<String> {
    let mut e = Engine::new();
    let diags = e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    assert!(diags.is_empty(), "{diags:?}");
    let s = e.scene();
    let mut out = Vec::new();
    fn walk(n: &datars_scene::Node, out: &mut Vec<String>) {
        if let datars_scene::NodeKind::Text(t) = &n.kind {
            out.push(t.text.clone());
        }
        for c in n.children() {
            walk(c, out);
        }
    }
    walk(&s.root, &mut out);
    out
}

fn session(exact: bool, locale: &str, x0: f64) -> serde_json::Value {
    let mut domain = serde_json::json!({ "data": "m", "fields": ["x0", "x1"] });
    if exact {
        domain["exact"] = serde_json::json!(true);
    }
    serde_json::json!({
        "datars": 1, "size": { "width": 800, "height": 100 }, "locale": locale,
        "data": {
            "m": { "values": [{ "sym": "AAPL", "name": "Apple", "x0": DAY + x0 * H, "x1": DAY + 16.0 * H }], "key": ["sym"] },
            "p": { "values": [{ "t": DAY + 10.0 * H, "v": 1 }], "key": ["t"] }
        },
        "scene": { "kind": "group", "key": "g",
            "scales": { "x": { "type": "time", "domain": domain, "range": [0, 650], "nice": true } },
            "children": [
                { "kind": "repeat", "from": { "ticks": "x", "count": 7 }, "template": { "kind": "text", "text": "=d.label", "at": ["=d.pos", 0] } },
                { "kind": "text", "key": "first", "text": "=format(scale.x(table.first('p', 't')), '.1f')", "at": [0, 20] },
                { "kind": "text", "key": "label", "text": "=scale.x.label(table.first('p', 't'))", "at": [0, 40] },
                { "kind": "text", "key": "name", "text": "=table.first('m', 'name') + '/' + table.last('m', 'sym')", "at": [0, 60] }
            ] }
    })
}

#[test]
fn a_session_ticks_on_the_hour_as_times_of_day() {
    let t = texts(session(true, "en-US", 9.5));
    assert!(t.iter().any(|s| s == "10 AM") && t.iter().any(|s| s == "12 PM") && t.iter().any(|s| s == "4 PM"), "{t:?}");
    let ticks = &t[..t.iter().position(|s| s == "50.0").unwrap()];
    assert!(!ticks.iter().any(|s| s.contains("1970") || s.contains("Oct")), "no dates inside one day: {ticks:?}");
    // 10:00 is half an hour into 09:30–16:00: 650 × 0.5 / 6.5.
    assert!(t.contains(&"50.0".to_string()), "{t:?}");
    assert!(t.contains(&"Oct 16 10:00 AM".to_string()), "a point's label: the day and time {t:?}");
    let sv = texts(session(true, "sv", 9.5));
    assert!(sv.iter().any(|s| s == "10:00") && sv.iter().any(|s| s == "16:00"), "24-hour clock: {sv:?}");
}

#[test]
fn an_exact_domain_from_data_is_not_rounded() {
    // From 09:45, rounded to nice steps the axis would start earlier and 10:00 would sit further in.
    let at10 = |exact| texts(session(exact, "en-US", 9.75))[..].iter().find(|s| s.parse::<f64>().is_ok()).cloned().unwrap();
    assert_eq!(at10(true), "26.0", "650 × 0.25 / 6.25 h");
    assert_ne!(at10(false), "26.0", "a nice domain moves it");
}

#[test]
fn first_and_last_read_text() {
    assert!(texts(session(true, "en-US", 9.5)).contains(&"Apple/AAPL".to_string()));
}

#[test]
fn dates_on_a_time_scale_are_still_days() {
    // Small numbers are day counts (dates hand themselves to expressions as days), not 1970's
    // first milliseconds.
    let doc = serde_json::json!({
        "datars": 1, "size": { "width": 800, "height": 100 }, "locale": "en-US",
        "scene": { "kind": "group", "key": "g",
            "scales": { "x": { "type": "time", "domain": [20_742, 20_772], "range": [0, 600] } },
            "children": [{ "kind": "repeat", "from": { "ticks": "x", "count": 4 }, "template": { "kind": "text", "text": "=d.label", "at": ["=d.pos", 0] } }] }
    });
    let t = texts(doc);
    assert!(t.iter().any(|s| s.starts_with("Oct") || s.starts_with("Nov")), "{t:?}");
}
