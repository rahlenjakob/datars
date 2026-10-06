//! A theme specimen board (`datars theme --specimen`, or a theme editor's preview): standard charts,
//! palettes, ramps and inks as one document, every colour through the theme's tokens.

use serde_json::{json, Value};

fn use_(recipe: &str, params: Value, key: &str) -> Value {
    json!({ "kind": "use", "recipe": format!("@datars/std/{recipe}"), "params": params, "key": key })
}

/// A specimen board for a theme: standard charts (bars, lines, stacked bars, a donut, a scatter)
/// and the palettes, ramps and ink tokens, as one document — every colour through the theme's
/// tokens, so the board is the theme. `theme` is an inline definition (or none: a built-in `name`).
pub fn specimen_doc(name: &str, theme: Option<&Value>) -> Value {
    let cats = ["North", "South", "East", "West", "Central"];
    let x: Vec<i32> = (1..=8).collect();
    let (mut series, mut xs, mut ys) = (Vec::new(), Vec::new(), Vec::new());
    for (si, sname) in ["Alpha", "Beta", "Gamma"].iter().enumerate() {
        for &xv in &x {
            series.push(*sname);
            xs.push(xv);
            ys.push(20 + si as i32 * 9 + (xv * (3 + si as i32)) % 17);
        }
    }
    // A loose cloud with a trend (a fixed-seed LCG: the board is the same every time).
    let (mut sx, mut sy, mut sv) = (Vec::new(), Vec::new(), Vec::new());
    let mut seed: u64 = 42;
    let mut rnd = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) as f64 / (1u64 << 31) as f64
    };
    for _ in 0..80 {
        let x = rnd() * 100.0;
        let y = 20.0 + 0.6 * x + (rnd() - 0.5) * 40.0;
        sx.push((x * 10.0).round() / 10.0);
        sy.push((y * 10.0).round() / 10.0);
        sv.push((y / 10.0).round());
    }
    let tokens = ["ink", "ink-2", "muted", "accent", "positive", "negative", "highlight", "surface"];
    let swatch = |i: usize, fill: Value, w: f64| json!({ "kind": "shape", "key": format!("s{i}"), "geom": { "type": "rect", "x": i as f64 * (w + 4.0), "y": 0, "w": w, "h": 22, "r": 3 }, "fill": fill });
    let strip = |key: &str, y: f64, label: &str, items: Vec<Value>| json!({ "kind": "group", "key": key, "transform": { "translate": [0, y] }, "children": [
        { "kind": "text", "key": "label", "text": label, "at": [0, 0], "style": { "size": "$size.small", "ink": "$muted", "baseline": "top" } },
        { "kind": "group", "key": "chips", "transform": { "translate": [0, 14] }, "children": items },
    ] });
    let palettes = json!({ "kind": "group", "key": "palettes",
        "scales": { "seq": { "type": "sequential", "domain": [0, 6], "range": "$sequential" }, "div": { "type": "diverging", "domain": [-3, 3], "range": "$diverging", "mid": 0 } },
        "children": [
            strip("categorical", 0.0, "categorical", (0..10).map(|i| swatch(i, json!(format!("$categorical[{i}]")), 22.0)).collect()),
            strip("sequential", 48.0, "sequential", (0..7).map(|i| swatch(i, json!(format!("=scale.seq({i})")), 30.0)).collect()),
            strip("diverging", 96.0, "diverging", (0..7).map(|i| swatch(i, json!(format!("=scale.div({})", i as i32 - 3)), 30.0)).collect()),
            strip("inks", 144.0, "inks", tokens.iter().enumerate().map(|(i, t)| json!({ "kind": "group", "key": t, "children": [
                swatch(i, json!(format!("${t}")), 30.0),
                { "kind": "text", "key": "name", "text": t, "at": [i as f64 * 34.0 + 15.0, 36], "style": { "size": 8, "ink": "$muted", "align": "middle", "baseline": "top" } },
            ] })).collect()),
        ] });
    let mut doc = json!({
        "datars": 1, "id": "specimen", "title": format!("Theme specimen — {name}"),
        "size": { "width": 1000, "height": 640 },
        "theme": { "use": name },
        "data": {
            "cats": { "values": { "region": cats, "sales": [132, 98, 143, 87, 64] }, "key": ["region"] },
            "series": { "values": { "series": series, "x": xs, "y": ys } },
            "dots": { "values": { "x": sx, "y": sy, "v": sv } },
        },
        "scene": { "kind": "group", "key": "root", "layout": { "type": "rows", "gap": 12, "padding": [16, 20, 16, 20] }, "children": [
            { "kind": "text", "key": "title", "text": format!("{name}"), "at": [0, 0], "size": { "h": "auto" }, "style": { "font": "font.title", "size": "$size.title", "ink": "$ink", "baseline": "top" } },
            { "kind": "group", "key": "grid", "layout": { "type": "grid", "columns": 3, "gap": 20 }, "children": [
                use_("plot", json!({ "data": "cats", "x": "region", "y": "sales", "color": "region", "title": "Bars", "children": [use_("bar", json!({ "labels": true }), "bars")] }), "bars"),
                use_("plot", json!({ "data": "series", "x": "x", "y": "y", "xType": "linear", "color": "series", "title": "Lines", "zero": false, "children": [use_("line", json!({ "labels": true }), "lines")] }), "lines"),
                use_("plot", json!({ "data": "series", "x": "x", "y": "y", "color": "series", "title": "Stacked", "legend": true, "children": [use_("stacked", json!({ "series": "series" }), "stacked")] }), "stacked"),
                { "kind": "group", "key": "donut-cell", "layout": { "type": "rows", "gap": 6 }, "children": [
                    { "kind": "text", "key": "title", "text": "Parts", "at": [0, 0], "size": { "h": "auto" }, "style": { "font": "font.title", "size": "$size.title", "ink": "$ink", "baseline": "top" } },
                    use_("pie", json!({ "data": "cats", "value": "sales", "category": "region", "inner": 0.58 }), "donut"),
                ] },
                use_("plot", json!({ "data": "dots", "x": "x", "y": "y", "xType": "linear", "color": "v", "colorType": "sequential", "title": "Points", "zero": false, "children": [use_("point", json!({ "r": 4 }), "points")] }), "points"),
                palettes,
            ] },
        ] },
    });
    if let Some(t) = theme {
        doc["theme"]["themes"] = json!([t]);
    }
    doc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_specimen_board_resolves_cleanly_in_every_mode() {
        let theme = serde_json::json!({ "name": "test/brand", "extends": "datars/neutral", "tokens": { "accent": "#7a1fa2" },
            "modes": { "dark": { "accent": "#c58af0" }, "high-contrast": { "accent": "#5a0f80" } } });
        let doc = specimen_doc("test/brand", Some(&theme));
        let mut e = datars_engine::Engine::new();
        let d = e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
        assert!(d.is_empty(), "{d:?}");
        for mode in [datars_theme::Mode::Light, datars_theme::Mode::Dark, datars_theme::Mode::HighContrast] {
            e.set_mode(mode);
            let s = e.scene();
            assert!(e.diagnostics().is_empty(), "{:?}", e.diagnostics());
            let snap = s.snapshot();
            for part in ["(\"bars\",)", "(\"lines\",)", "(\"stacked\",)", "(\"donut\",)", "(\"points\",)", "(\"palettes\",)"] {
                assert!(snap.contains(part), "{part} missing");
            }
        }
    }
}
