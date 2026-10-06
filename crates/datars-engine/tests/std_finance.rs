//! The standard library's financial charts through the engine: trading days on a band scale
//! (calendar ticks, no weekend gaps), candles and volume panes, indicators computed over a longer
//! history, indexed comparisons — and the generic pieces under them (window ops, `group.first`).

use datars_engine::Engine;
use serde_json::{json, Value};

/// Ten sessions over a weekend and a month boundary: Mon 27 January to Fri 7 February 2025.
const DAYS: [&str; 10] = ["2025-01-27", "2025-01-28", "2025-01-29", "2025-01-30", "2025-01-31", "2025-02-03", "2025-02-04", "2025-02-05", "2025-02-06", "2025-02-07"];

fn ohlcv() -> Value {
    let open: Vec<f64> = (0..10).map(|i| 100.0 + i as f64).collect();
    let close: Vec<f64> = (0..10).map(|i| 100.0 + i as f64 + if i % 2 == 0 { 1.0 } else { -1.0 }).collect();
    let high: Vec<f64> = open.iter().zip(&close).map(|(o, c)| o.max(*c) + 2.0).collect();
    let low: Vec<f64> = open.iter().zip(&close).map(|(o, c)| o.min(*c) - 2.0).collect();
    let volume: Vec<f64> = (0..10).map(|i| 1e6 * (1 + i % 3) as f64).collect();
    json!({ "values": { "date": DAYS, "open": open, "high": high, "low": low, "close": close, "volume": volume }, "key": ["date"] })
}

fn snapshot(doc: Value) -> String {
    let mut e = Engine::new();
    let diags = e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    assert!(diags.is_empty(), "{diags:?}");
    let s = e.scene().snapshot();
    assert!(e.diagnostics().is_empty(), "{:?}\n{s}", e.diagnostics());
    s
}

fn plot(data: Value, params: Value, children: Value) -> Value {
    let mut p = json!({ "data": "p", "x": "date", "y": "close", "xType": "band", "children": children });
    p.as_object_mut().unwrap().extend(params.as_object().unwrap().clone());
    json!({ "datars": 1, "size": { "width": 420, "height": 300 }, "data": { "p": data }, "scene": { "kind": "use", "recipe": "@datars/std/plot", "params": p } })
}

fn use_(recipe: &str, params: Value) -> Value {
    json!({ "kind": "use", "recipe": format!("@datars/std/{recipe}"), "params": params })
}

/// The lines of a snapshot inside the group with `key` (by indentation).
fn section<'a>(snap: &'a str, key: &str) -> Vec<&'a str> {
    let lines: Vec<&str> = snap.lines().collect();
    let Some(i) = lines.iter().position(|l| l.contains(&format!("group (\"{key}\",)"))) else { return Vec::new() };
    let depth = lines[i].len() - lines[i].trim_start().len();
    lines[i + 1..].iter().take_while(|l| l.len() - l.trim_start().len() > depth).copied().collect()
}

fn num_after(line: &str, key: &str) -> f64 {
    line.split(key).nth(1).and_then(|r| r.split([' ', ',', ')']).next()).and_then(|x| x.parse().ok()).unwrap_or(f64::NAN)
}

#[test]
fn candles_sit_on_trading_days_with_volume_in_a_pane_below() {
    let s = snapshot(plot(ohlcv(), json!({}), json!([use_("candlestick", json!({})), use_("volume", json!({}))])));
    let bodies: Vec<&str> = section(&s, "bodies").into_iter().filter(|l| l.contains(" rect ")).collect();
    assert_eq!(bodies.len(), 10, "{s}");
    let xs: Vec<f64> = bodies.iter().map(|l| num_after(l, "x=")).collect();
    let steps: Vec<f64> = xs.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(steps.iter().all(|d| (d - steps[0]).abs() < 0.02 && *d > 0.0), "Friday to Monday is one slot, like any other day: {xs:?}");
    // Up days (close ≥ open) in $up, the others in $down; hovering reads the day and its prices.
    assert!(bodies[0].contains("fill=$up") && bodies[1].contains("fill=$down"), "{}", bodies[1]);
    assert!(bodies[5].contains("\"3 Feb 2025 · O 105.00 H 107.00 L 102.00 C 104.00 (−0.95%)\""), "{}", bodies[5]);
    // The axis labels calendar weeks at their first session, not every day.
    let ticks: Vec<&str> = section(&s, "axis-x").into_iter().filter(|l| l.contains(" text ")).collect();
    assert_eq!(ticks.len(), 2, "{ticks:?}");
    assert!(ticks[0].contains("\"27 Jan\"") && ticks[1].contains("\"3 Feb\""), "{ticks:?}");
    // Prices leave out zero: the axis spans the lows and highs.
    let price_axis = section(&s, "axis-y").join("\n");
    assert!(!price_axis.contains("text \"0\""), "{price_axis}");
    assert!(price_axis.contains("text \"96\"") || price_axis.contains("text \"95\""), "down to the lowest low (98): {price_axis}");
    // Volume in its own pane under the prices, on its own axis.
    let pane = section(&s, "lower");
    assert_eq!(pane.iter().filter(|l| l.contains(" rect ") && l.contains("role=datum")).count(), 10);
    assert!(section(&s, "axis-lower").iter().any(|l| l.contains(" text ") && l.contains("M\" at")), "{s}");
}

#[test]
fn prices_on_a_log_axis_fit_their_range() {
    let s = snapshot(plot(ohlcv(), json!({ "yType": "log" }), json!([use_("candlestick", json!({}))])));
    let ticks: Vec<String> = section(&s, "axis-y").iter().filter(|l| l.contains(" text ")).map(|l| l.split('"').nth(3).unwrap_or("").to_string()).collect();
    // 98…111 stays 98…111 (not 10…1000), labelled like a price axis.
    assert!(ticks.len() >= 3 && ticks.iter().all(|t| t.parse::<f64>().is_ok_and(|v| (98.0..=111.0).contains(&v))), "{ticks:?}");
    assert!(section(&s, "bodies").iter().all(|l| !l.contains("NaN")));
}

#[test]
fn ohlc_bars_and_hollow_candles() {
    let s = snapshot(plot(ohlcv(), json!({}), json!([use_("ohlc", json!({}))])));
    assert_eq!(section(&s, "ohlc").iter().filter(|l| l.contains(" path ") && l.contains("role=datum")).count(), 10, "{s}");
    let h = snapshot(plot(ohlcv(), json!({}), json!([use_("candlestick", json!({ "hollow": true }))])));
    let bodies: Vec<&str> = section(&h, "bodies").into_iter().filter(|l| l.contains(" rect ")).collect();
    assert!(bodies[0].contains("fill=$paper") && bodies[0].contains("stroke=$up"), "rising: outlined: {}", bodies[0]);
    assert!(bodies[1].contains("fill=$down"), "falling: filled: {}", bodies[1]);
}

/// Sixty sessions of a steady climb; the chart shows the last ten.
fn history() -> (Value, Value) {
    let mut days = Vec::new();
    let mut d = datars_data::date::date_from_ymd(2025, 1, 1).unwrap();
    while days.len() < 60 {
        if (d + 3).rem_euclid(7) < 5 {
            days.push(d);
        }
        d += 1;
    }
    let iso: Vec<String> = days.iter().map(|&d| { let (y, m, day) = datars_data::date::ymd_from_date(d); format!("{y:04}-{m:02}-{day:02}") }).collect();
    let close: Vec<f64> = (0..60).map(|i| 50.0 + i as f64).collect();
    let src = json!({ "values": { "date": iso, "close": close }, "key": ["date"] });
    let shown = json!({ "from": "h", "ops": [{ "op": "window", "fn": "rank", "field": "date", "as": "age", "order": "-date" }, { "op": "filter", "expr": "=d.age <= 10" }] });
    (src, shown)
}

fn history_doc(children: Value) -> Value {
    let (src, shown) = history();
    json!({ "datars": 1, "size": { "width": 420, "height": 300 }, "data": { "h": src }, "tables": { "p": shown },
        "scene": { "kind": "use", "recipe": "@datars/std/plot", "params": { "data": "p", "x": "date", "y": "close", "xType": "band", "children": children } } })
}

fn polyline_points(snap: &str, key: &str) -> Vec<usize> {
    section(snap, key).iter().filter(|l| l.contains(" polyline ")).map(|l| num_after(l, "n=") as usize).collect()
}

#[test]
fn indicators_compute_over_the_history_and_show_the_rows_on_show() {
    // Over the history, every row on show has its full 5-day window.
    let s = snapshot(history_doc(json!([use_("line", json!({})), use_("movingAverage", json!({ "window": 5, "source": "h" }))])));
    assert_eq!(polyline_points(&s, "ma-sma_5"), vec![10], "{s}");
    // Over the rows on show only, the first four have no full window: no line there.
    let s = snapshot(history_doc(json!([use_("movingAverage", json!({ "window": 5 }))])));
    assert_eq!(polyline_points(&s, "ma-sma_5"), vec![6]);
    // Partial windows when asked; an EMA from the first row.
    let s = snapshot(history_doc(json!([use_("movingAverage", json!({ "window": 5, "full": false })), use_("movingAverage", json!({ "window": 3, "kind": "ema" }))])));
    assert_eq!(polyline_points(&s, "ma-sma_5"), vec![10]);
    assert_eq!(polyline_points(&s, "ma-ema_3"), vec![8], "an EMA waits for its span too");
    // Bollinger bands: a band and its edges and middle, named in the plot's key with the averages.
    let s = snapshot(history_doc(json!([use_("bollinger", json!({ "window": 5, "source": "h" })), use_("movingAverage", json!({ "window": 5, "source": "h" }))])));
    let bands = section(&s, "bollinger-5-2");
    assert_eq!(bands.iter().filter(|l| l.contains(" area ")).count(), 1, "{s}");
    assert_eq!(bands.iter().filter(|l| l.contains(" polyline ")).count(), 3);
    let key = section(&s, "indicators").join("\n");
    assert!(key.contains("\"BB 5, 2\"") && key.contains("\"SMA 5\""), "{key}");
}

#[test]
fn hidden_indicators_leave_the_axis_to_the_prices() {
    let with = |when: bool| {
        let bb = json!({ "kind": "use", "recipe": "@datars/std/bollinger", "params": { "window": 5, "k": 40, "source": "h" }, "when": when });
        let s = snapshot(history_doc(json!([use_("line", json!({})), bb])));
        section(&s, "axis-y").iter().filter(|l| l.contains(" text ")).map(|l| l.split('"').nth(3).unwrap_or("").replace('−', "-").replace(',', "").parse::<f64>().unwrap_or_else(|_| panic!("{l}"))).collect::<Vec<_>>()
    };
    // σ of five steady steps is √2: ±40σ reaches far past the prices (100…109) — only while shown.
    let (shown, hidden) = (with(true), with(false));
    assert!(shown.first().is_some_and(|t| *t < 50.0), "{shown:?}");
    assert!(hidden.first().is_some_and(|t| *t >= 90.0), "{hidden:?}");
}

#[test]
fn indexed_lines_start_at_the_base_and_say_their_change() {
    let v = json!({ "values": { "t": ["A", "A", "A", "B", "B", "B"], "date": ["2025-03-03", "2025-03-04", "2025-03-05", "2025-03-03", "2025-03-04", "2025-03-05"], "close": [10, 11, 12, 200, 190, 180] }, "key": ["t", "date"] });
    let s = snapshot(plot(v.clone(), json!({ "color": "t" }), json!([use_("indexed", json!({}))])));
    let labels: Vec<&str> = section(&s, "end-labels").into_iter().filter(|l| l.contains(" text ")).collect();
    assert!(labels.iter().any(|l| l.contains("\"A +20.0%\"")) && labels.iter().any(|l| l.contains("\"B −10.0%\"")), "{labels:?}");
    let ticks = section(&s, "axis-y").join("\n");
    assert!(ticks.contains("text \"100\"") && !ticks.contains("text \"0\""), "an index around 100, not prices: {ticks}");
    assert!(s.contains("group (\"rule-y-100\",)"), "a baseline at the base");
    // Drawdowns: from each series' running peak, labelled at the end.
    let s = snapshot(plot(v, json!({ "color": "t", "format": ".0%" }), json!([use_("drawdown", json!({}))])));
    let labels: Vec<&str> = section(&s, "end-labels").into_iter().filter(|l| l.contains(" text ")).collect();
    assert!(labels.iter().any(|l| l.contains("\"A +0.0%\"")) && labels.iter().any(|l| l.contains("\"B −10.0%\"")), "{labels:?}");
}

#[test]
fn sparklines_colour_by_their_change() {
    let v = json!({ "values": { "t": ["A", "A", "A", "B", "B", "B"], "i": [1, 2, 3, 1, 2, 3], "v": [10, 11, 12, 20, 19, 18] } });
    let doc = json!({ "datars": 1, "size": { "width": 200, "height": 80 }, "data": { "s": v },
        "scene": { "kind": "group", "key": "root", "layout": { "type": "rows", "gap": 4 }, "children": [
            { "kind": "repeat", "from": { "groups": "s", "by": "t" }, "template": { "kind": "use", "recipe": "@datars/std/sparkline", "params": { "data": "@group", "x": "i", "y": "v", "name": "v" } } }] } });
    let s = snapshot(doc);
    let lines: Vec<&str> = s.lines().filter(|l| l.contains(" polyline ")).collect();
    assert_eq!(lines.len(), 2, "{s}");
    assert!(lines[0].contains("stroke=$up") && lines[0].contains("\"v: 12.00 (+20.0%)\""), "{}", lines[0]);
    assert!(lines[1].contains("stroke=$down") && lines[1].contains("\"v: 18.00 (−10.0%)\""), "{}", lines[1]);
}

#[test]
fn window_ops_and_group_ends_in_documents() {
    // Rolling σ, an EMA, running peaks, the partition's first value and a minimum window, as table
    // ops; `group.first/last` in expressions.
    let ops = json!([
        { "op": "window", "fn": "rolling_std", "field": "v", "as": "sd", "k": 2, "partition": ["g"], "order": "i" },
        { "op": "window", "fn": "ema", "field": "v", "as": "ema", "k": 3, "partition": ["g"], "order": "i" },
        { "op": "window", "fn": "cummax", "field": "v", "as": "peak", "partition": ["g"], "order": "i" },
        { "op": "window", "fn": "first", "field": "v", "as": "first", "partition": ["g"], "order": "i" },
        { "op": "window", "fn": "rolling_max", "field": "v", "as": "max2", "k": 2, "min": 2, "partition": ["g"], "order": "i" },
    ]);
    let row = "=d.g + d.i + ':' + format(d.sd, '.1f') + '/' + format(d.ema, '.2f') + '/' + d.peak + '/' + d.first + '/' + (d.max2 > 0 ? d.max2 : '-')";
    let doc = json!({ "datars": 1, "size": { "width": 300, "height": 200 },
        "data": { "s": { "values": { "g": ["a", "a", "a", "b", "b"], "i": [1, 2, 3, 1, 2], "v": [4, 8, 6, 10, 2] } } },
        "tables": { "w": { "from": "s", "ops": ops } },
        "scene": { "kind": "group", "key": "root", "children": [
            { "kind": "repeat", "from": "w", "template": { "kind": "text", "text": row, "at": [0, "=d.i * 10"] } },
            { "kind": "repeat", "from": { "groups": "s", "by": "g" }, "template": { "kind": "text", "text": "=d.g + ' ' + group.first('v') + '→' + group.last('v')", "at": [100, 0] } }] } });
    let s = snapshot(doc);
    for want in ["a1:0.0/4.00/4/4/-", "a2:2.0/6.00/8/4/8", "a3:1.0/6.00/8/4/8", "b1:0.0/10.00/10/10/-", "b2:4.0/6.00/10/10/10", "a 4→6", "b 10→2"] {
        assert!(s.contains(&format!("\"{want}\"")), "{want}\n{s}");
    }
}
