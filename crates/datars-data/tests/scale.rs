use datars_color::Color;
use datars_data::date::{date_from_ymd, ymd_from_date};
use datars_data::scale::*;
use datars_data::{Column, Value};
use datars_theme::Ink;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9 * a.abs().max(b.abs()).max(1.0)
}

macro_rules! assert_close {
    ($a:expr, $b:expr) => {{
        let (a, b) = ($a, $b);
        assert!(close(a, b), "{a} != {b}");
    }};
}

fn day(y: i32, m: u32, d: u32) -> f64 {
    date_from_ymd(y, m, d).unwrap() as f64
}

fn vals(v: &[&str]) -> Vec<Value> {
    v.iter().map(|s| Value::str(s)).collect()
}

fn tick_nums(ts: &[Tick]) -> Vec<f64> {
    ts.iter().map(Tick::num).collect()
}

// ---- continuous ------------------------------------------------------------------------------

#[test]
fn linear_map_invert_clamp() {
    let s = Scale::linear([0.0, 100.0], [0.0, 500.0]);
    assert_eq!(s.map_num(50.0), 250.0);
    assert_eq!(s.map_f64(-10.0), -50.0);
    assert_eq!(s.invert(250.0), Some(Value::Num(50.0)));
    assert_eq!(s.invert_num(600.0), Some(120.0));
    let c = s.clone().with_clamp(true);
    assert_eq!(c.map_num(-10.0), 0.0);
    assert_eq!(c.invert_num(600.0), Some(100.0));
    // Screen y: the range runs backwards.
    let y = Scale::linear([0.0, 10.0], [300.0, 0.0]);
    assert_eq!(y.map_num(0.0), 300.0);
    assert_eq!(y.map_num(10.0), 0.0);
    assert_eq!(y.invert_num(150.0), Some(5.0));
    // Nulls and text don't map.
    assert!(s.map_num(Value::Null).is_nan());
    assert!(s.map_num("x").is_nan());
    assert_eq!(s.invert(f64::NAN), None);
    // A collapsed domain maps to the middle of the range.
    assert_eq!(Scale::linear([5.0, 5.0], [0.0, 10.0]).map_num(5.0), 5.0);
    // Dates map as days.
    assert_eq!(Scale::linear([0.0, 10.0], [0.0, 1.0]).map_num(Value::Date(5)), 0.5);
}

#[test]
fn linear_nice_ticks() {
    let s = Scale::linear([0.0, 97.0], [0.0, 1.0]).nice(5);
    assert_eq!(s.domain_extent(), Some([0.0, 100.0]));
    let t = s.ticks(5);
    assert_eq!(tick_nums(&t), vec![0.0, 20.0, 40.0, 60.0, 80.0, 100.0]);
    assert!(t.iter().all(|t| t.label_hint == LabelHint::Number { decimals: 0 }));
    // Without nice, ticks stay inside the domain.
    assert_eq!(tick_nums(&Scale::linear([0.0, 97.0], [0.0, 1.0]).ticks(5)), vec![0.0, 20.0, 40.0, 60.0, 80.0]);
    // Decimal steps are exact and carry their precision.
    let f = Scale::linear([0.0, 1.0], [0.0, 1.0]).ticks(10);
    assert_eq!(tick_nums(&f)[3], 0.3);
    assert_eq!(f[0].label_hint, LabelHint::Number { decimals: 1 });
    let g = Scale::linear([0.0, 0.1], [0.0, 1.0]).ticks(4);
    assert_eq!(tick_nums(&g), vec![0.0, 0.02, 0.04, 0.06, 0.08, 0.1]);
    assert_eq!(g[0].label_hint, LabelHint::Number { decimals: 2 });
    // Reversed domains tick in domain order.
    assert_eq!(tick_nums(&Scale::linear([10.0, 0.0], [0.0, 1.0]).ticks(2)), vec![10.0, 5.0, 0.0]);
    // Nice keeps direction.
    assert_eq!(Scale::linear([97.0, 3.0], [0.0, 1.0]).nice(5).domain_extent(), Some([100.0, 0.0]));
}

#[test]
fn log_scale() {
    let s = Scale::log(10.0, [1.0, 1000.0], [0.0, 300.0]);
    assert_close!(s.map_num(10.0), 100.0);
    assert_close!(s.map_num(100.0), 200.0);
    assert_close!(s.invert_num(100.0).unwrap(), 10.0);
    assert!(s.map_num(0.0).is_nan());
    assert!(s.map_num(-5.0).is_nan());
    let t = s.ticks(10);
    assert_eq!(t.len(), 28);
    let majors: Vec<f64> = t.iter().filter(|t| matches!(t.label_hint, LabelHint::Log { major: true, .. })).map(Tick::num).collect();
    assert_eq!(majors, vec![1.0, 10.0, 100.0, 1000.0]);
    // Nice rounds out to whole powers.
    let n = Scale::log(10.0, [3.0, 870.0], [0.0, 1.0]).nice(10);
    assert_eq!(n.domain_extent(), Some([1.0, 1000.0]));
    assert_eq!(Scale::log(10.0, [10.0, 1000.0], [0.0, 1.0]).nice(10).domain_extent(), Some([10.0, 1000.0]));
    // Small decimals.
    let d = Scale::log(10.0, [0.01, 1.0], [0.0, 1.0]).ticks(10);
    assert_eq!(d[0].num(), 0.01);
    assert_eq!(d[0].label_hint, LabelHint::Log { decimals: 2, major: true });
    // Negative domains mirror.
    let neg = Scale::log(10.0, [-1000.0, -1.0], [0.0, 300.0]);
    assert_close!(neg.map_num(-10.0), 200.0);
    assert_eq!(Scale::log(10.0, [-870.0, -3.0], [0.0, 1.0]).nice(10).domain_extent(), Some([-1000.0, -1.0]));
    // Base 2: powers when they are enough ticks, else linear ticks (as in d3).
    assert_eq!(tick_nums(&Scale::log(2.0, [1.0, 8.0], [0.0, 1.0]).ticks(5)), vec![1.0, 2.0, 4.0, 8.0]);
    assert_eq!(Scale::log(2.0, [1.0, 8.0], [0.0, 1.0]).ticks(10).len(), 15);
    // Within a decade, too few powers-and-multiples to read: linear ticks.
    let narrow = Scale::log(10.0, [9.0, 16.0], [0.0, 1.0]).ticks(2);
    assert_eq!(tick_nums(&narrow), vec![10.0, 12.0, 14.0, 16.0]);
    assert!(matches!(narrow[0].label_hint, LabelHint::Number { .. }));
}

#[test]
fn sqrt_pow_symlog() {
    let s = Scale::sqrt([0.0, 100.0], [0.0, 10.0]);
    assert_close!(s.map_num(25.0), 5.0);
    assert_close!(s.invert_num(5.0).unwrap(), 25.0);
    let p = Scale::pow(2.0, [0.0, 10.0], [0.0, 100.0]);
    assert_close!(p.map_num(5.0), 25.0);
    assert_close!(p.invert_num(25.0).unwrap(), 5.0);
    let y = Scale::symlog(1.0, [-100.0, 100.0], [-1.0, 1.0]);
    assert_close!(y.map_num(0.0), 0.0);
    assert_close!(y.map_num(100.0), 1.0);
    assert_close!(y.map_num(-100.0), -1.0);
    assert_close!(y.map_num(10.0), -y.map_num(-10.0));
    assert!(y.map_num(10.0) > 0.5, "log-like: 10 is past the middle of 0…100");
    assert_close!(y.invert_num(y.map_num(42.0)).unwrap(), 42.0);
    // Ticks for these are linear ticks of the domain.
    assert_eq!(tick_nums(&s.ticks(5)), vec![0.0, 20.0, 40.0, 60.0, 80.0, 100.0]);
    assert_eq!(p.clone().nice(5).domain_extent(), Some([0.0, 10.0]));
}

// ---- time ------------------------------------------------------------------------------------

fn ymd(t: &Tick) -> (i32, u32, u32) {
    match t.value {
        Value::Date(d) => ymd_from_date(d),
        ref v => panic!("not a date tick: {v:?}"),
    }
}

#[test]
fn time_ticks_over_three_years() {
    let s = Scale::time([day(2020, 1, 1), day(2023, 1, 1)], [0.0, 1096.0]);
    assert_close!(s.map_num(Value::Date(day(2021, 1, 1) as i32)), 366.0);
    let q = s.ticks(10);
    assert_eq!(q.len(), 13, "quarterly");
    assert_eq!(ymd(&q[0]), (2020, 1, 1));
    assert_eq!(ymd(&q[1]), (2020, 4, 1));
    assert_eq!(ymd(&q[12]), (2023, 1, 1));
    assert_eq!(q[0].label_hint, LabelHint::Time { interval: TimeInterval::Quarter, step: 1, boundary: TimeInterval::Year });
    assert_eq!(q[1].label_hint, LabelHint::Time { interval: TimeInterval::Quarter, step: 1, boundary: TimeInterval::Quarter });
    let y = s.ticks(4);
    assert_eq!(y.iter().map(ymd).collect::<Vec<_>>(), vec![(2020, 1, 1), (2021, 1, 1), (2022, 1, 1), (2023, 1, 1)]);
    assert!(matches!(y[0].label_hint, LabelHint::Time { interval: TimeInterval::Year, .. }));
}

#[test]
fn time_ticks_at_every_granularity() {
    let months = Scale::time([day(2024, 1, 1), day(2024, 7, 1)], [0.0, 1.0]).ticks(6);
    assert_eq!(months.len(), 7);
    assert_eq!(ymd(&months[1]), (2024, 2, 1));
    assert!(matches!(months[1].label_hint, LabelHint::Time { interval: TimeInterval::Month, boundary: TimeInterval::Month, .. }));
    let weeks = Scale::time([day(2024, 1, 3), day(2024, 3, 1)], [0.0, 1.0]).ticks(8);
    assert!(weeks.iter().all(|t| matches!(t.label_hint, LabelHint::Time { interval: TimeInterval::Week, .. })));
    assert_eq!(ymd(&weeks[0]), (2024, 1, 8), "first Monday inside the domain");
    assert!(weeks.iter().all(|t| datars_data::date::weekday(t.num() as i32) == 0));
    let days = Scale::time([day(2024, 1, 1), day(2024, 1, 8)], [0.0, 1.0]).ticks(7);
    assert_eq!(days.len(), 8);
    assert!(matches!(days[3].label_hint, LabelHint::Time { interval: TimeInterval::Day, boundary: TimeInterval::Day, .. }));
    let decades = Scale::time([day(1901, 6, 1), day(2019, 6, 1)], [0.0, 1.0]).ticks(10);
    assert_eq!(decades.iter().map(|t| ymd(t).0).collect::<Vec<_>>(), (1910..=2010).step_by(10).collect::<Vec<_>>());
    // Across a year boundary, the January tick says it starts a year.
    let winter = Scale::time([day(2023, 11, 1), day(2024, 3, 1)], [0.0, 1.0]).ticks(4);
    let jan = winter.iter().find(|t| ymd(t) == (2024, 1, 1)).expect("a January tick");
    assert!(matches!(jan.label_hint, LabelHint::Time { boundary: TimeInterval::Year, .. }));
    // Millisecond timestamps get clock ticks.
    let h = Scale::time_ms([0.0, 6.0 * 3_600_000.0], [0.0, 1.0]).ticks(6);
    assert_eq!(tick_nums(&h), (0..=6).map(|i| i as f64 * 3_600_000.0).collect::<Vec<_>>());
    assert!(matches!(h[1].label_hint, LabelHint::Time { interval: TimeInterval::Hour, boundary: TimeInterval::Hour, .. }));
    assert!(matches!(h[0].label_hint, LabelHint::Time { boundary: TimeInterval::Year, .. }));
    let m = Scale::time_ms([0.0, 60_000.0], [0.0, 1.0]).ticks(4);
    assert_eq!(tick_nums(&m), vec![0.0, 15_000.0, 30_000.0, 45_000.0, 60_000.0]);
}

#[test]
fn time_nice_and_invert() {
    let s = Scale::time([day(2020, 2, 10), day(2022, 11, 3)], [0.0, 100.0]).nice(4);
    assert_eq!(s.domain_extent(), Some([day(2020, 1, 1), day(2023, 1, 1)]));
    let m = Scale::time([day(2024, 1, 10), day(2024, 6, 20)], [0.0, 1.0]).nice(6);
    assert_eq!(m.domain_extent(), Some([day(2024, 1, 1), day(2024, 7, 1)]));
    let x = Scale::time([0.0, 10.0], [0.0, 100.0]);
    assert_eq!(x.invert_num(50.0), Some(5.0));
    // Date values map on a millisecond scale too.
    let ms = Scale::time_ms([0.0, 86_400_000.0 * 10.0], [0.0, 10.0]);
    assert_close!(ms.map_num(Value::Date(3)), 3.0);
}

// ---- discrete --------------------------------------------------------------------------------

#[test]
fn band_scale_with_padding() {
    let s = Scale::band(vals(&["a", "b", "c", "d"]), [0.0, 400.0]).with_padding(0.2, 0.1);
    assert_eq!(s.step(), 100.0);
    assert_eq!(s.bandwidth(), 80.0);
    assert_eq!(s.map_num("a"), 10.0);
    assert_eq!(s.map_num("c"), 210.0);
    assert!(s.map_num("zzz").is_nan());
    assert_eq!(s.invert(215.0), Some(Value::str("c")));
    assert_eq!(s.invert(105.0), Some(Value::str("b")), "the gap belongs to the nearer band");
    assert_eq!(s.invert(-50.0), None);
    assert_eq!(s.invert(450.0), None);
    let t = s.ticks(10);
    assert_eq!(t.len(), 4);
    assert_eq!(t[1], Tick { value: Value::str("b"), label_hint: LabelHint::Category });
    // No padding: bands tile the range.
    let plain = Scale::band(vals(&["a", "b"]), [0.0, 100.0]);
    assert_eq!((plain.step(), plain.bandwidth(), plain.map_num("b")), (50.0, 50.0, 50.0));
    // Reversed range: the first band is at the far end.
    let rev = Scale::band(vals(&["a", "b", "c", "d"]), [400.0, 0.0]).with_padding(0.2, 0.1);
    assert_eq!(rev.map_num("a"), 310.0);
    assert_eq!(rev.invert(315.0), Some(Value::str("a")));
    // Rounding snaps to whole pixels.
    let r = Scale::band(vals(&["a", "b", "c"]), [0.0, 100.0]).with_round(true);
    assert_eq!((r.step(), r.bandwidth(), r.map_num("a")), (33.0, 33.0, 1.0));
    // Alignment moves the leftover space.
    let left = Scale::band(vals(&["a", "b"]), [0.0, 100.0]).with_padding(0.0, 0.5).with_align(0.0);
    assert_eq!(left.map_num("a"), 0.0);
    // Numbers and dates work as categories.
    let years = Scale::band(vec![Value::Num(2020.0), Value::Num(2021.0)], [0.0, 10.0]);
    assert_eq!(years.map_num(2021.0), 5.0);
}

#[test]
fn dates_on_a_band_are_trading_days_with_calendar_ticks() {
    // Two tickers' sessions, sorted by ticker then date; the first lacks Friday 31 January.
    let d = |m: u32, day: u32| date_from_ymd(2025, m, day);
    let dates: Vec<Option<i32>> = [(1, 29), (1, 30), (2, 3), (2, 4), (1, 29), (1, 30), (1, 31), (2, 3), (2, 4)].iter().map(|&(m, day)| d(m, day)).collect();
    let s = Scale::from_data(&Column::Date(dates), ScaleKind::Band).with_range([0.0, 500.0]);
    let slots: Vec<(i32, u32, u32)> = s.domain_values().unwrap().iter().map(|v| if let Value::Date(x) = v { ymd_from_date(*x) } else { panic!("{v:?}") }).collect();
    assert_eq!(slots, vec![(2025, 1, 29), (2025, 1, 30), (2025, 1, 31), (2025, 2, 3), (2025, 2, 4)], "calendar order, not first appearance");
    // Evenly spaced whatever the weekend: Monday follows Friday by one slot.
    assert_eq!(s.map_num(Value::Date(d(2, 3).unwrap())) - s.map_num(Value::Date(d(1, 31).unwrap())), s.step());
    // Ticks are calendar ticks (the first session of February), not one per category.
    let t = s.ticks(2);
    assert_eq!(t.iter().map(ymd).collect::<Vec<_>>(), vec![(2025, 2, 3)]);
    assert!(matches!(t[0].label_hint, LabelHint::Time { interval: TimeInterval::Week, boundary: TimeInterval::Month, .. }), "a week tick that starts a month");
    // Other categories keep one tick each.
    assert!(Scale::band(vals(&["a", "b"]), [0.0, 1.0]).ticks(1).iter().all(|t| t.label_hint == LabelHint::Category));
}

#[test]
fn point_scale() {
    let s = Scale::point(vals(&["a", "b", "c"]), [0.0, 100.0]);
    assert_eq!((s.map_num("a"), s.map_num("b"), s.map_num("c")), (0.0, 50.0, 100.0));
    assert_eq!(s.bandwidth(), 0.0);
    assert_eq!(s.step(), 50.0);
    assert_eq!(s.invert(70.0), Some(Value::str("b")));
    let p = Scale::point(vals(&["a", "b", "c"]), [0.0, 100.0]).with_padding(0.0, 0.5);
    assert_close!(p.map_num("a"), 100.0 / 6.0);
    assert_close!(p.map_num("c"), 500.0 / 6.0);
    assert_eq!(Scale::point(vals(&["solo"]), [0.0, 100.0]).map_num("solo"), 50.0);
}

#[test]
fn ordinal_scale() {
    let s = Scale::ordinal(vals(&["lo", "mid", "hi"]), Outputs::Num(vec![1.0, 2.0]));
    assert_eq!((s.map_num("lo"), s.map_num("mid"), s.map_num("hi")), (1.0, 2.0, 1.0), "outputs cycle");
    assert!(s.map_num("other").is_nan());
    assert_eq!(s.invert(2.1), Some(Value::str("mid")));
    assert_eq!(s.try_map_ink("lo"), None, "numeric outputs aren't inks");
    let inks = Scale::ordinal(vals(&["a", "b"]), Outputs::Ink(vec![Ink::token("good"), Ink::token("bad")]));
    assert_eq!(inks.map_ink("b"), Ink::token("bad"));
    assert_eq!(inks.map_num("b"), 1.0);
    assert_eq!(inks.try_map_ink("zz"), None);
    assert_eq!(inks.map_ink("zz"), Ink::token("muted"));
    assert_eq!(inks.clone().with_unknown(Ink::token("rule")).map_ink("zz"), Ink::token("rule"));
    assert_eq!(Outputs::palette("categorical", 2), Outputs::Ink(vec![Ink::palette("categorical", 0), Ink::palette("categorical", 1)]));
}

#[test]
fn quantize_quantile_threshold() {
    let q = Scale::quantize([0.0, 100.0], Outputs::ramp("sequential", 4));
    assert_eq!(q.map_ink(10.0), Ink::ramp("sequential", 0.0));
    assert_eq!(q.map_ink(60.0), Ink::ramp("sequential", 2.0 / 3.0));
    assert_eq!(q.map_ink(100.0), Ink::ramp("sequential", 1.0));
    assert_eq!(q.map_num(60.0), 2.0);
    assert!(q.is_color());
    assert_eq!(tick_nums(&q.ticks(5)), vec![0.0, 20.0, 40.0, 60.0, 80.0, 100.0]);
    let qn = Scale::quantize([0.0, 100.0], Outputs::Num(vec![1.0, 2.0, 3.0, 4.0]));
    assert_eq!(qn.map_num(50.0), 3.0, "a value on a threshold goes above it");
    assert_eq!(qn.invert_num(3.0), Some(62.5));
    assert!(!qn.is_color());

    let ql = Scale::quantile(&[8.0, 1.0, 2.0, 3.0, f64::NAN, 4.0, 5.0, 6.0, 7.0], Outputs::Num(vec![0.0, 1.0, 2.0, 3.0]));
    assert_eq!(tick_nums(&ql.ticks(5)), vec![2.75, 4.5, 6.25]);
    assert_eq!((ql.map_num(3.0), ql.map_num(7.0), ql.map_num(-100.0)), (1.0, 3.0, 0.0));
    assert_eq!(ql.invert_num(0.0), Some(1.875));
    assert_eq!(ql.domain_extent(), Some([1.0, 8.0]));

    let th = Scale::threshold(vec![0.0, 50.0], Outputs::Ink(vec![Ink::token("negative"), Ink::token("low"), Ink::token("high")]));
    assert_eq!(th.map_ink(-1.0), Ink::token("negative"));
    assert_eq!(th.map_ink(0.0), Ink::token("low"));
    assert_eq!(th.map_ink(49.9), Ink::token("low"));
    assert_eq!(th.map_ink(50.0), Ink::token("high"));
    assert_eq!(th.try_map_ink(f64::NAN), None);
    let tn = Scale::threshold(vec![0.0, 50.0], Outputs::Num(vec![10.0, 20.0, 30.0]));
    assert_eq!(tn.invert_num(10.0), Some(0.0), "open-ended buckets invert to their finite edge");
    assert_eq!(tn.invert_num(21.0), Some(25.0));
}

// ---- colour ----------------------------------------------------------------------------------

#[test]
fn sequential_colour_scales() {
    let s = Scale::sequential([0.0, 100.0], Colors::palette("sequential"));
    assert_eq!(s.map_ink(25.0), Ink::Ramp { name: "sequential".into(), at: 0.25, alpha: 1.0 });
    assert_eq!(s.map_num(25.0), 0.25);
    assert_eq!(s.map_ink(150.0), Ink::ramp("sequential", 1.0), "clamped by default");
    assert_eq!(s.try_map_ink(Value::Null), None);
    assert_eq!(s.map_ink(Value::Null), Ink::token("muted"));
    assert_eq!(s.invert_num(0.5), Some(50.0));
    assert!(s.is_color());
    // Explicit stops blend in OKLab and give literal colours.
    let stops = Scale::sequential([0.0, 10.0], Colors::stops(&["#000000", "#ffffff"]));
    let black = Color::parse("#000000").unwrap();
    let white = Color::parse("#ffffff").unwrap();
    assert_eq!(stops.map_ink(5.0), Ink::Color(black.lerp_oklab(white, 0.5)));
    assert_eq!(stops.map_ink(0.0), Ink::Color(black));
    assert_eq!(stops.map_ink(10.0), Ink::Color(white));
    // A log colour ramp.
    let log = Scale::sequential([1.0, 1000.0], Colors::palette("sequential")).with_transform(Transform::Log { base: 10.0 });
    assert_close!(log.map_num(10.0), 1.0 / 3.0);
    assert_eq!(tick_nums(&log.ticks(3)), vec![1.0, 10.0, 100.0, 1000.0]);
    // "$name" is accepted for palettes.
    assert_eq!(Scale::sequential([0.0, 1.0], Colors::palette("$blues")).map_ink(1.0), Ink::ramp("blues", 1.0));
}

#[test]
fn diverging_colour_scale() {
    let s = Scale::diverging([-10.0, 0.0, 30.0], Colors::palette("diverging"));
    assert_eq!(s.map_num(0.0), 0.5);
    assert_eq!(s.map_num(-10.0), 0.0);
    assert_eq!(s.map_num(-5.0), 0.25);
    assert_eq!(s.map_num(15.0), 0.75);
    assert_eq!(s.map_num(99.0), 1.0);
    assert_eq!(s.map_ink(15.0), Ink::ramp("diverging", 0.75));
    assert_eq!(s.invert_num(0.75), Some(15.0));
    assert_eq!(s.invert_num(0.25), Some(-5.0));
    assert_eq!(s.domain_extent(), Some([-10.0, 30.0]));
    let n = s.clone().nice(5);
    assert!(matches!(n, Scale::Diverging(Diverging { domain: [-10.0, 0.0, 30.0], .. })));
}

#[test]
fn categorical_colour_scale() {
    let s = Scale::categorical(vals(&["S", "M", "SD"]), "categorical").with_override("M", Ink::parse("#1e6bb8").unwrap());
    assert_eq!(s.map_ink("S"), Ink::palette("categorical", 0));
    assert_eq!(s.map_ink("SD"), Ink::palette("categorical", 2), "keys keep their slot next to an override");
    assert_eq!(s.map_ink("M"), Ink::parse("#1e6bb8").unwrap());
    assert_eq!(s.map_ink("V"), Ink::token("muted"));
    assert_eq!(s.clone().with_unknown(Ink::token("rule")).map_ink("V"), Ink::token("rule"));
    assert_eq!(s.map_num("SD"), 2.0);
    assert_eq!(s.invert(1.0), Some(Value::str("M")));
    assert_eq!(s.invert(7.0), None);
    let col = Column::strs(&["SD", "V", "M", "S"]);
    assert_eq!(s.map_column_ink(&col), vec![Ink::palette("categorical", 2), Ink::token("muted"), Ink::parse("#1e6bb8").unwrap(), Ink::palette("categorical", 0)]);
    // Domains from data are in first-appearance order.
    let d = Scale::from_data(&Column::strs(&["b", "a", "b", "c"]), ScaleKind::Categorical);
    assert_eq!(d.domain_values(), Some(&vals(&["b", "a", "c"])[..]));
    assert_eq!(d.map_ink("c"), Ink::palette("categorical", 2));
    assert_eq!(d.ticks(3).len(), 3);
}

#[test]
fn piecewise_pinned_stops() {
    let p = Piecewise::parse("#22c55e 2 · #f5a524 4 · #f97362 6.5").unwrap();
    let s = Scale::Piecewise(p.clone());
    let (g, y, r) = (Color::parse("#22c55e").unwrap(), Color::parse("#f5a524").unwrap(), Color::parse("#f97362").unwrap());
    assert_eq!(s.map_ink(2.0), Ink::Color(g));
    assert_eq!(s.map_ink(1.0), Ink::Color(g), "clamped below");
    assert_eq!(s.map_ink(3.0), Ink::Color(g.lerp_oklab(y, 0.5)));
    assert_eq!(s.map_ink(4.0), Ink::Color(y));
    assert_eq!(s.map_ink(5.25), Ink::Color(y.lerp_oklab(r, 0.5)));
    assert_eq!(s.map_ink(6.5), Ink::Color(r));
    assert_eq!(s.map_ink(100.0), Ink::Color(r), "clamped above");
    assert_eq!(s.try_map_ink(f64::NAN), None);
    assert_eq!(s.map_num(4.25), 0.5);
    assert_eq!(tick_nums(&s.ticks(10)), vec![2.0, 4.0, 6.5]);
    assert_eq!(s.invert_num(0.5), Some(4.25));
    let stepped = s.clone().with_stepped(true);
    assert_eq!(stepped.map_ink(3.9), Ink::Color(g));
    assert_eq!(stepped.map_ink(4.0), Ink::Color(y));
    // Order-independent, theme references allowed; malformed specs are rejected.
    let q = Piecewise::parse("$negative 10, $positive -10").unwrap();
    assert_eq!(q.stops[0], (-10.0, Ink::token("positive")));
    assert_eq!(Scale::Piecewise(q.clone()).map_ink(-3.0), Ink::token("positive"), "tokens can't blend: nearer stop wins");
    assert_eq!(Scale::Piecewise(q).map_ink(3.0), Ink::token("negative"));
    assert!(Piecewise::parse("#22c55e 2 #f5a524").is_none());
    assert!(Piecewise::parse("nope 2").is_none());
    assert!(Piecewise::parse("#fff x").is_none());
    // Ramp positions on one palette blend by position.
    let ramp = Scale::piecewise(vec![(0.0, Ink::ramp("sequential", 0.0)), (10.0, Ink::ramp("sequential", 1.0))]);
    assert_eq!(ramp.map_ink(5.0), Ink::ramp("sequential", 0.5));
    // Rescaling the domain moves the stops proportionally.
    let moved = Scale::Piecewise(p).with_domain_extent([0.0, 9.0]);
    assert_eq!(moved.domain_extent(), Some([0.0, 9.0]));
    assert_eq!(moved.map_ink(4.0), Ink::Color(y));
}

// ---- interpolation ---------------------------------------------------------------------------

#[test]
fn lerp_for_animated_rescale() {
    let a = Scale::linear([0.0, 100.0], [0.0, 500.0]);
    let b = Scale::linear([0.0, 200.0], [0.0, 400.0]);
    assert_eq!(Scale::lerp(&a, &b, 0.0), a);
    assert_eq!(Scale::lerp(&a, &b, 1.0), b);
    let m = Scale::lerp(&a, &b, 0.5);
    assert_eq!(m.domain_extent(), Some([0.0, 150.0]));
    assert_eq!(m.range(), Some([0.0, 450.0]));
    // Log domains interpolate geometrically.
    let la = Scale::log(10.0, [1.0, 100.0], [0.0, 1.0]);
    let lb = Scale::log(10.0, [1.0, 10_000.0], [0.0, 1.0]);
    let lm = Scale::lerp(&la, &lb, 0.5).domain_extent().unwrap();
    assert_close!(lm[0], 1.0);
    assert_close!(lm[1], 1000.0);
    // Bands with the same domain move their range and paddings.
    let ba = Scale::band(vals(&["a", "b"]), [0.0, 100.0]);
    let bb = Scale::band(vals(&["a", "b"]), [0.0, 200.0]).with_padding(0.2, 0.0);
    let bm = Scale::lerp(&ba, &bb, 0.5);
    assert_eq!(bm.range(), Some([0.0, 150.0]));
    assert!(matches!(bm, Scale::Band(Band { padding_inner, .. }) if close(padding_inner, 0.1)));
    // Different domains / kinds switch at the midpoint.
    let bc = Scale::band(vals(&["a", "c"]), [0.0, 100.0]);
    assert_eq!(Scale::lerp(&ba, &bc, 0.4), ba);
    assert_eq!(Scale::lerp(&ba, &bc, 0.6), bc);
    assert_eq!(Scale::lerp(&a, &ba, 0.49), a);
    assert_eq!(Scale::lerp(&a, &ba, 0.51), ba);
    // Time, colour and piecewise scales interpolate their domains.
    let ta = Scale::time([0.0, 10.0], [0.0, 1.0]);
    let tb = Scale::time([10.0, 30.0], [0.0, 1.0]);
    assert_eq!(Scale::lerp(&ta, &tb, 0.5).domain_extent(), Some([5.0, 20.0]));
    let sa = Scale::sequential([0.0, 10.0], Colors::palette("sequential"));
    let sb = Scale::sequential([0.0, 30.0], Colors::palette("sequential"));
    assert_eq!(Scale::lerp(&sa, &sb, 0.5).domain_extent(), Some([0.0, 20.0]));
    let pa = Scale::Piecewise(Piecewise::parse("#000 0 #fff 10").unwrap());
    let pb = Scale::Piecewise(Piecewise::parse("#000 10 #fff 20").unwrap());
    assert_eq!(Scale::lerp(&pa, &pb, 0.5).domain_extent(), Some([5.0, 15.0]));
    let qa = Scale::quantize([0.0, 10.0], Outputs::Num(vec![1.0, 2.0]));
    let qb = Scale::quantize([10.0, 20.0], Outputs::Num(vec![1.0, 2.0]));
    assert_eq!(Scale::lerp(&qa, &qb, 0.25).domain_extent(), Some([2.5, 12.5]));
    let oa = Scale::ordinal(vals(&["x"]), Outputs::Num(vec![0.0]));
    let ob = Scale::ordinal(vals(&["x"]), Outputs::Num(vec![10.0]));
    assert_eq!(Scale::lerp(&oa, &ob, 0.3).map_num("x"), 3.0);
}

#[test]
fn a_mark_stays_on_its_gridline_through_a_rescale() {
    // Two-level interpolation: positions computed through the interpolated scale agree with
    // the interpolated scale's own ticks at every t.
    let a = Scale::linear([0.0, 100.0], [0.0, 500.0]);
    let b = Scale::linear([0.0, 400.0], [0.0, 500.0]);
    for i in 0..=10 {
        let s = Scale::lerp(&a, &b, i as f64 / 10.0);
        for t in s.ticks(5) {
            let px = s.map_num(t.value.clone());
            assert_close!(s.invert_num(px).unwrap(), t.num());
        }
    }
}

// ---- serde, from_data, columns ---------------------------------------------------------------

#[test]
fn scales_serialize_to_tagged_json() {
    let s = Scale::linear([0.0, 100.0], [0.0, 500.0]);
    assert_eq!(serde_json::to_string(&s).unwrap(), r#"{"type":"linear","domain":[0.0,100.0],"range":[0.0,500.0]}"#);
    let log: Scale = serde_json::from_str(r#"{"type": "log", "base": 10, "domain": [1, 1000], "range": [0, 1], "clamp": true}"#).unwrap();
    assert_eq!(log, Scale::log(10.0, [1.0, 1000.0], [0.0, 1.0]).with_clamp(true));
    let time: Scale = serde_json::from_str(r#"{"type": "time", "domain": [0, 10], "range": [0, 1]}"#).unwrap();
    assert_eq!(time, Scale::time([0.0, 10.0], [0.0, 1.0]));
    let all = vec![
        s,
        log,
        time,
        Scale::sqrt([0.0, 1.0], [0.0, 2.0]),
        Scale::pow(3.0, [0.0, 1.0], [0.0, 2.0]),
        Scale::symlog(2.0, [-1.0, 1.0], [0.0, 2.0]),
        Scale::time_ms([0.0, 1e9], [0.0, 1.0]),
        Scale::band(vals(&["a", "b"]), [0.0, 1.0]).with_padding(0.1, 0.2).with_round(true),
        Scale::point(vec![Value::Num(1.0), Value::Date(3)], [0.0, 1.0]),
        Scale::ordinal(vals(&["a"]), Outputs::Ink(vec![Ink::token("accent")])).with_unknown(Ink::token("muted")),
        Scale::quantize([0.0, 1.0], Outputs::Num(vec![1.0, 2.0])),
        // (Thresholds chosen to survive serde_json's default float parsing, which can be 1 ulp off for
        // 17-digit numbers.)
        Scale::quantile(&[1.0, 2.0, 3.0, 4.0], Outputs::ramp("sequential", 2)),
        Scale::threshold(vec![0.5], Outputs::Ink(vec![Ink::parse("#fff").unwrap(), Ink::parse("$ink@0.5").unwrap()])),
        Scale::sequential([0.0, 1.0], Colors::stops(&["#000", "#fff"])).with_transform(Transform::Sqrt),
        Scale::diverging([-1.0, 0.0, 1.0], Colors::palette("diverging")).with_clamp(false),
        Scale::categorical(vals(&["S", "M"]), "categorical").with_override("S", Ink::parse("#e8112d").unwrap()),
        Scale::Piecewise(Piecewise::parse("#22c55e 2 · #f5a524 4").unwrap()).with_stepped(true),
    ];
    for s in all {
        let j = serde_json::to_string(&s).unwrap();
        let back: Scale = serde_json::from_str(&j).unwrap_or_else(|e| panic!("{j}: {e}"));
        assert_eq!(back, s, "{j}");
    }
    let cat = serde_json::to_string(&Scale::categorical(vals(&["S"]), "categorical").with_override("S", Ink::parse("#e8112d").unwrap())).unwrap();
    assert_eq!(cat, r##"{"type":"categorical","domain":["S"],"palette":"categorical","overrides":[["S","#e8112d"]]}"##);
}

#[test]
fn domains_from_data() {
    let c = Column::Num(vec![3.0, 7.0, f64::NAN, 97.0]);
    assert_eq!(Scale::from_data(&c, ScaleKind::Linear).domain_extent(), Some([3.0, 97.0]));
    let opts = DomainOptions { zero: true, nice: Some(5), palette: None };
    assert_eq!(Scale::from_data_with(&c, ScaleKind::Linear, &opts).domain_extent(), Some([0.0, 100.0]));
    let neg = Column::Num(vec![-5.0, 20.0, 0.0]);
    assert_eq!(Scale::from_data(&neg, ScaleKind::Log).domain_extent(), Some([20.0, 20.0]));
    assert!(matches!(Scale::from_data(&neg, ScaleKind::Diverging), Scale::Diverging(Diverging { domain: [-5.0, 0.0, 20.0], .. })));
    assert!(matches!(Scale::from_data(&c, ScaleKind::Diverging), Scale::Diverging(Diverging { domain: [3.0, 50.0, 97.0], .. })));
    let dates = Column::Date(vec![Some(date_from_ymd(2021, 3, 1).unwrap()), None, Some(date_from_ymd(2020, 1, 1).unwrap())]);
    let t = Scale::from_data(&dates, ScaleKind::Time);
    assert!(matches!(t, Scale::Time { unit: TimeUnit::Days, .. }));
    assert_eq!(t.domain_extent(), Some([day(2020, 1, 1), day(2021, 3, 1)]));
    let b = Scale::from_data(&Column::strs(&["x", "y", "x"]), ScaleKind::Band).with_range([0.0, 100.0]);
    assert_eq!(b.domain_values().unwrap().len(), 2);
    assert_eq!(b.bandwidth(), 50.0);
    let seq = Scale::from_data_with(&c, ScaleKind::Sequential, &DomainOptions { palette: Some("blues".into()), ..DomainOptions::default() });
    assert_eq!(seq.map_ink(97.0), Ink::ramp("blues", 1.0));
    let qz = Scale::from_data(&c, ScaleKind::Quantize);
    assert_eq!(qz.map_ink(97.0), Ink::ramp("sequential", 1.0));
    let ql = Scale::from_data(&c, ScaleKind::Quantile);
    assert_eq!(ql.map_ink(3.0), Ink::ramp("sequential", 0.0));
    let ord = Scale::from_data(&Column::strs(&["lo", "hi"]), ScaleKind::Ordinal);
    assert_eq!(ord.map_num("hi"), 1.0);
    let empty = Scale::from_data(&Column::Num(vec![]), ScaleKind::Linear);
    assert_eq!(empty.domain_extent(), Some([0.0, 1.0]));
    for kind in [ScaleKind::Sqrt, ScaleKind::Symlog, ScaleKind::Point] {
        let _ = Scale::from_data(&c, kind);
    }
}

#[test]
fn map_column_agrees_with_map_num() {
    let col = Column::strs_opt(&[Some("b"), Some("zz"), None, Some("a")]);
    for s in [
        Scale::band(vals(&["a", "b"]), [0.0, 100.0]).with_padding(0.1, 0.1),
        Scale::point(vals(&["a", "b"]), [0.0, 100.0]),
        Scale::ordinal(vals(&["a", "b"]), Outputs::Num(vec![5.0, 6.0])),
        Scale::categorical(vals(&["a", "b"]), "categorical"),
    ] {
        let got = s.map_column(&col);
        for (i, g) in got.iter().enumerate() {
            let want = s.map_num(col.get(i));
            assert!((g.is_nan() && want.is_nan()) || g == &want, "{} row {i}: {g} vs {want}", s.kind_name());
        }
    }
    let nums = Column::Num(vec![0.0, 50.0, f64::NAN]);
    let lin = Scale::linear([0.0, 100.0], [0.0, 1.0]);
    let got = lin.map_column(&nums);
    assert_eq!(got[..2], [0.0, 0.5]);
    assert!(got[2].is_nan());
    let seq = Scale::sequential([0.0, 100.0], Colors::palette("sequential"));
    assert_eq!(seq.map_column_ink(&nums), vec![Ink::ramp("sequential", 0.0), Ink::ramp("sequential", 0.5), Ink::token("muted")]);
}

#[test]
fn accessors_and_setters() {
    let mut s = Scale::linear([0.0, 1.0], [0.0, 1.0]);
    s.set_domain_extent([2.0, 4.0]);
    s.set_range([10.0, 20.0]);
    assert_eq!((s.domain_extent(), s.range()), (Some([2.0, 4.0]), Some([10.0, 20.0])));
    assert_eq!(s.kind_name(), "linear");
    assert!(s.continuous().is_some());
    let mut b = Scale::band(vals(&["a"]), [0.0, 1.0]);
    b.set_domain_values(vals(&["x", "y"]));
    assert_eq!(b.domain_values().unwrap().len(), 2);
    assert_eq!(b.domain_extent(), None);
    let d = Scale::diverging([-1.0, 0.0, 1.0], Colors::palette("diverging")).with_domain_extent([-4.0, 2.0]);
    assert!(matches!(d, Scale::Diverging(Diverging { domain: [-4.0, 0.0, 2.0], .. })));
    assert_eq!(Scale::categorical(vec![], "c").range(), None);
}

#[test]
fn unified_map_returns_what_the_scale_is_for() {
    assert_eq!(Scale::linear([0.0, 1.0], [0.0, 10.0]).map(0.5), Mapped::Num(5.0));
    let c: ColorCat = Categorical { domain: vals(&["a"]), palette: "categorical".into(), overrides: vec![], unknown: None };
    assert_eq!(Scale::Categorical(c).map("a"), Mapped::Ink(Ink::palette("categorical", 0)));
    let q = Scale::quantize([0.0, 1.0], Outputs::Num(vec![1.0, 2.0]));
    assert_eq!(q.map(0.9).num(), Some(2.0));
    assert_eq!(Scale::sequential([0.0, 1.0], Colors::palette("sequential")).map(0.5).ink(), Some(&Ink::ramp("sequential", 0.5)));
}
