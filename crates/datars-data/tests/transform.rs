use datars_data::date::{date_from_ymd, ymd_from_date};
use datars_data::num::pairwise_sum;
use datars_data::transform::*;
use datars_data::{Column, ColumnType, DataError, Table, Value};

fn d(y: i32, m: u32, day: u32) -> i32 {
    date_from_ymd(y, m, day).unwrap()
}

fn strs(t: &Table, c: &str) -> Vec<Option<String>> {
    t.str(c).unwrap().iter().map(|s| s.as_deref().map(str::to_string)).collect()
}

fn s(v: &str) -> Option<String> {
    Some(v.to_string())
}

/// Checks numbers with NaN standing for null.
fn nums(t: &Table, c: &str) -> Vec<f64> {
    t.num(c).unwrap_or_else(|| panic!("no numeric column {c}")).to_vec()
}

fn assert_nums(got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{got:?} vs {want:?}");
    for (g, w) in got.iter().zip(want) {
        assert!((g.is_nan() && w.is_nan()) || (g - w).abs() < 1e-12, "{got:?} vs {want:?}");
    }
}

fn sales() -> Table {
    Table::from_columns(
        "sales",
        vec![
            ("region", Column::strs(&["N", "S", "N", "E", "S", "N"])),
            ("product", Column::strs(&["a", "a", "b", "a", "b", "a"])),
            ("amount", Column::Num(vec![10.0, 20.0, 5.0, 7.0, f64::NAN, 3.0])),
            ("day", Column::Date(vec![Some(d(2024, 1, 3)), Some(d(2024, 1, 1)), None, Some(d(2024, 2, 1)), Some(d(2024, 1, 2)), Some(d(2023, 12, 31))])),
        ],
    )
    .unwrap()
}

#[test]
fn filter_and_predicates() {
    let t = sales();
    let f = filter(&t, &[true, false, true, false, false, true]).unwrap();
    assert_eq!(strs(&f, "region"), vec![s("N"), s("N"), s("N")]);
    assert!(matches!(filter(&t, &[true]), Err(DataError::LengthMismatch { .. })));
    assert_eq!(filter_where(&t, "amount", &Pred::Gt(6.0)).unwrap().len(), 3);
    assert_eq!(filter_where(&t, "amount", &Pred::IsNull).unwrap().len(), 1);
    assert_eq!(filter_where(&t, "region", &Pred::In(vec![Value::str("E"), Value::str("S")])).unwrap().len(), 3);
    assert_eq!(filter_where(&t, "region", &Pred::Ne(Value::str("N"))).unwrap().len(), 3);
    assert_eq!(filter_where(&t, "product", &Pred::Contains("A".into())).unwrap().len(), 4);
    assert_eq!(filter_where(&t, "day", &Pred::Between(d(2024, 1, 1) as f64, d(2024, 1, 3) as f64)).unwrap().len(), 3);
    assert!(filter_where(&t, "nope", &Pred::IsNull).is_err());
    // Filtering keeps the key.
    let k = t.clone().with_column("id", Column::Num((0..6).map(f64::from).collect())).unwrap().with_key(&["id"]).unwrap();
    assert_eq!(filter(&k, &[true; 6]).unwrap().key, vec!["id"]);
}

#[test]
fn derive_adds_or_replaces() {
    let t = sales();
    let x = derive(&t, "double", Column::Num(nums(&t, "amount").iter().map(|v| v * 2.0).collect())).unwrap();
    assert_eq!(x.num("double").unwrap()[0], 20.0);
    assert!(derive(&t, "bad", Column::Num(vec![1.0])).is_err());
}

#[test]
fn aggregate_every_op() {
    let t = sales();
    let a = aggregate(
        &t,
        &["region"],
        &[
            Agg::count("n"),
            Agg::new(AggOp::Count, "amount", "n_amount"),
            Agg::sum("amount", "sum"),
            Agg::mean("amount", "mean"),
            Agg::new(AggOp::Median, "amount", "median"),
            Agg::new(AggOp::Quantile(0.25), "amount", "q1"),
            Agg::new(AggOp::Min, "amount", "min"),
            Agg::new(AggOp::Max, "day", "last_day"),
            Agg::new(AggOp::Distinct, "product", "products"),
            Agg::new(AggOp::First, "product", "first"),
            Agg::new(AggOp::Last, "product", "last"),
        ],
    )
    .unwrap();
    // Groups in order of first appearance, keyed by the group column.
    assert_eq!(strs(&a, "region"), vec![s("N"), s("S"), s("E")]);
    assert_eq!(a.key, vec!["region"]);
    assert!(a.validate_keys().is_ok());
    assert_nums(&nums(&a, "n"), &[3.0, 2.0, 1.0]);
    assert_nums(&nums(&a, "n_amount"), &[3.0, 1.0, 1.0]);
    assert_nums(&nums(&a, "sum"), &[18.0, 20.0, 7.0]);
    assert_nums(&nums(&a, "mean"), &[6.0, 20.0, 7.0]);
    assert_nums(&nums(&a, "median"), &[5.0, 20.0, 7.0]);
    assert_nums(&nums(&a, "q1"), &[4.0, 20.0, 7.0]);
    assert_nums(&nums(&a, "min"), &[3.0, 20.0, 7.0]);
    assert_eq!(a.date("last_day").unwrap(), &[Some(d(2024, 1, 3)), Some(d(2024, 1, 2)), Some(d(2024, 2, 1))]);
    assert_nums(&nums(&a, "products"), &[2.0, 2.0, 1.0]);
    assert_eq!(strs(&a, "first"), vec![s("a"), s("a"), s("a")]);
    assert_eq!(strs(&a, "last"), vec![s("a"), s("b"), s("a")]);
}

#[test]
fn aggregate_edge_cases() {
    let t = sales();
    // No group columns: one row for the whole table.
    let all = aggregate(&t, &[], &[Agg::count("n"), Agg::sum("amount", "total")]).unwrap();
    assert_eq!(all.len(), 1);
    assert_nums(&nums(&all, "total"), &[45.0]);
    assert!(all.key.is_empty());
    // …even for an empty table.
    let empty = t.take_rows(&[]);
    let e = aggregate(&empty, &[], &[Agg::count("n"), Agg::mean("amount", "m")]).unwrap();
    assert_nums(&nums(&e, "n"), &[0.0]);
    assert!(nums(&e, "m")[0].is_nan());
    assert_eq!(aggregate(&empty, &["region"], &[Agg::count("n")]).unwrap().len(), 0);
    // Multi-column groups.
    let g = aggregate(&t, &["region", "product"], &[Agg::sum("amount", "s")]).unwrap();
    assert_eq!(g.len(), 5);
    assert_eq!(g.key, vec!["region", "product"]);
    // Mistakes.
    assert!(matches!(aggregate(&t, &["region"], &[Agg::sum("region", "x")]), Err(DataError::TypeMismatch { .. })));
    assert!(aggregate(&t, &["region"], &[Agg { op: AggOp::Sum, column: None, as_name: "x".into() }]).is_err());
    assert!(aggregate(&t, &["region"], &[Agg::count("region")]).is_err());
    assert!(aggregate(&t, &["region"], &[Agg::new(AggOp::Quantile(1.5), "amount", "q")]).is_err());
    assert!(aggregate(&t, &["nope"], &[]).is_err());
}

#[test]
fn sums_are_pairwise_and_bit_identical() {
    // Values whose naive running sum loses the small terms.
    let mut v = vec![1e16];
    v.extend(std::iter::repeat_n(1.0, 1000));
    v.push(-1e16);
    let n = v.len();
    let t = Table::from_columns("s", vec![("g", Column::strs(&vec!["x"; n])), ("v", Column::Num(v.clone()))]).unwrap();
    let a = aggregate(&t, &["g"], &[Agg::sum("v", "s")]).unwrap();
    let got = nums(&a, "s")[0];
    assert_eq!(got.to_bits(), pairwise_sum(&v).to_bits(), "the fixed pairwise tree");
    let naive: f64 = v.iter().sum();
    assert_ne!(got, naive, "pairwise keeps what a running sum drops");
    assert!((got - 1000.0).abs() <= 2.0, "{got}");
    for _ in 0..3 {
        assert_eq!(nums(&aggregate(&t, &["g"], &[Agg::sum("v", "s")]).unwrap(), "s")[0].to_bits(), got.to_bits());
    }
}

#[test]
fn sort_is_stable_with_nulls_last() {
    let t = sales();
    let s1 = sort(&t, &[("amount", false)]).unwrap();
    assert_nums(&nums(&s1, "amount"), &[3.0, 5.0, 7.0, 10.0, 20.0, f64::NAN]);
    let s2 = sort(&t, &[("amount", true)]).unwrap();
    assert_nums(&nums(&s2, "amount"), &[20.0, 10.0, 7.0, 5.0, 3.0, f64::NAN]);
    // Ties keep input order (stable): N rows stay 10, 5, 3.
    let s3 = sort(&t, &[("region", false)]).unwrap();
    assert_eq!(strs(&s3, "region"), vec![s("E"), s("N"), s("N"), s("N"), s("S"), s("S")]);
    assert_nums(&nums(&s3, "amount")[1..4], &[10.0, 5.0, 3.0]);
    // Multi-key, mixed directions.
    let s4 = sort(&t, &[("product", true), ("amount", false)]).unwrap();
    assert_eq!(strs(&s4, "product"), vec![s("b"), s("b"), s("a"), s("a"), s("a"), s("a")]);
    assert_nums(&nums(&s4, "amount"), &[5.0, f64::NAN, 3.0, 7.0, 10.0, 20.0]);
    // Dates, nulls last even descending.
    let s5 = sort(&t, &[("day", true)]).unwrap();
    assert_eq!(s5.date("day").unwrap()[0], Some(d(2024, 2, 1)));
    assert_eq!(s5.date("day").unwrap()[5], None);
    assert!(sort(&t, &[("nope", false)]).is_err());
}

#[test]
fn top_n_folds_the_rest_into_other() {
    let t = Table::from_columns(
        "t",
        vec![("party", Column::strs(&["A", "B", "C", "D", "E"])), ("votes", Column::Num(vec![5.0, 40.0, 30.0, 15.0, 10.0])), ("seats", Column::Num(vec![1.0, 9.0, 7.0, 3.0, 2.0]))],
    )
    .unwrap()
    .with_key(&["party"])
    .unwrap();
    let top = top_n(&t, "votes", 3, Some("Other")).unwrap();
    assert_eq!(strs(&top, "party"), vec![s("B"), s("C"), s("D"), s("Other")]);
    assert_nums(&nums(&top, "votes"), &[40.0, 30.0, 15.0, 15.0]);
    assert_nums(&nums(&top, "seats"), &[9.0, 7.0, 3.0, 3.0]);
    assert!(top.validate_keys().is_ok());
    let plain = top_n(&t, "votes", 2, None).unwrap();
    assert_eq!(strs(&plain, "party"), vec![s("B"), s("C")]);
    // Nothing left over: no Other row.
    assert_eq!(top_n(&t, "votes", 5, Some("Other")).unwrap().len(), 5);
    assert_eq!(top_n(&t, "votes", 9, Some("Other")).unwrap().len(), 5);
}

#[test]
fn numeric_bins() {
    let t = Table::from_columns("t", vec![("age", Column::Num(vec![0.3, 19.0, 20.0, 29.9, -1.0, f64::NAN, 45.0]))]).unwrap();
    let b = bin_num(&t, "age", BinSpec::Step(10.0), "bin").unwrap();
    assert_nums(&nums(&b, "bin"), &[0.0, 10.0, 20.0, 20.0, -10.0, f64::NAN, 40.0]);
    assert_nums(&nums(&b, "bin_end"), &[10.0, 20.0, 30.0, 30.0, 0.0, f64::NAN, 50.0]);
    // Decimal steps are exact: 0.3 lands in [0.3, 0.4), not [0.2, 0.3).
    let fine = bin_num(&t, "age", BinSpec::Step(0.1), "b").unwrap();
    assert_eq!(nums(&fine, "b")[0], 0.3);
    assert_eq!(nums(&fine, "b_end")[0], 0.4);
    // Count: equal widths over the extent, maximum in the last bin.
    let c = bin_num(&t, "age", BinSpec::Count(2), "c").unwrap();
    assert_nums(&nums(&c, "c"), &[-1.0, -1.0, -1.0, 22.0, -1.0, f64::NAN, 22.0]);
    assert_nums(&nums(&c, "c_end"), &[22.0, 22.0, 22.0, 45.0, 22.0, f64::NAN, 45.0]);
    // Nice: about 5 bins of a 1/2/5 width over [-1, 45] → width 10.
    let n = bin_num(&t, "age", BinSpec::Nice(5), "n").unwrap();
    assert_nums(&nums(&n, "n"), &[0.0, 10.0, 20.0, 20.0, -10.0, f64::NAN, 40.0]);
    assert!(bin_num(&t, "age", BinSpec::Step(0.0), "x").is_err());
    assert!(bin_num(&t, "age", BinSpec::Count(0), "x").is_err());
}

#[test]
fn time_bins_across_year_boundaries() {
    let days = [d(2023, 12, 30), d(2023, 12, 31), d(2024, 1, 1), d(2024, 1, 2), d(2024, 4, 15)];
    let t = Table::from_columns("t", vec![("day", Column::Date(days.iter().map(|&x| Some(x)).chain([None]).collect()))]).unwrap();
    let bin = |u| bin_time(&t, "day", u, "b").unwrap();
    let dates = |t: &Table| t.date("b").unwrap().iter().map(|x| x.map(ymd_from_date)).collect::<Vec<_>>();
    assert_eq!(dates(&bin(TimeBin::Year)), vec![Some((2023, 1, 1)), Some((2023, 1, 1)), Some((2024, 1, 1)), Some((2024, 1, 1)), Some((2024, 1, 1)), None]);
    assert_eq!(dates(&bin(TimeBin::Quarter)), vec![Some((2023, 10, 1)), Some((2023, 10, 1)), Some((2024, 1, 1)), Some((2024, 1, 1)), Some((2024, 4, 1)), None]);
    assert_eq!(dates(&bin(TimeBin::Month)), vec![Some((2023, 12, 1)), Some((2023, 12, 1)), Some((2024, 1, 1)), Some((2024, 1, 1)), Some((2024, 4, 1)), None]);
    // ISO weeks start on Monday: Sat 30 Dec and Sun 31 Dec 2023 are in the week of Mon 25 Dec.
    assert_eq!(dates(&bin(TimeBin::Week)), vec![Some((2023, 12, 25)), Some((2023, 12, 25)), Some((2024, 1, 1)), Some((2024, 1, 1)), Some((2024, 4, 15)), None]);
    assert_eq!(dates(&bin(TimeBin::Day))[1], Some((2023, 12, 31)));
    assert_nums(&nums(&bin(TimeBin::Weekday), "b"), &[5.0, 6.0, 0.0, 1.0, 0.0, f64::NAN]);
    assert_nums(&nums(&bin(TimeBin::MonthOfYear), "b"), &[12.0, 12.0, 1.0, 1.0, 4.0, f64::NAN]);
    assert!(matches!(bin_time(&sales(), "amount", TimeBin::Year, "b"), Err(DataError::TypeMismatch { .. })));
}

#[test]
fn text_bins() {
    let t = Table::from_columns("t", vec![("v", Column::strs_opt(&[Some("ann@example.se"), Some("Uppsala, Sweden"), Some("  bo  svensson "), Some("örjan"), Some("x@"), None]))]).unwrap();
    let b = |p| strs(&bin_text(&t, "v", p, "p").unwrap(), "p");
    assert_eq!(b(TextPart::After("@".into())), vec![s("example.se"), s("Uppsala, Sweden"), s("bo  svensson"), s("örjan"), None, None]);
    assert_eq!(b(TextPart::Before(",".into())), vec![s("ann@example.se"), s("Uppsala"), s("bo  svensson"), s("örjan"), s("x@"), None]);
    assert_eq!(b(TextPart::FirstWord), vec![s("ann@example.se"), s("Uppsala,"), s("bo"), s("örjan"), s("x@"), None]);
    assert_eq!(b(TextPart::FirstLetter), vec![s("A"), s("U"), s("B"), s("Ö"), s("X"), None]);
    assert!(bin_text(&sales(), "amount", TextPart::FirstWord, "p").is_err());
}

fn series() -> Table {
    // Two partitions, rows deliberately out of time order.
    Table::from_columns(
        "s",
        vec![
            ("k", Column::strs(&["a", "b", "a", "b", "a", "b"])),
            ("t", Column::Num(vec![2.0, 1.0, 1.0, 2.0, 3.0, 3.0])),
            ("v", Column::Num(vec![20.0, 5.0, 10.0, 5.0, 40.0, 10.0])),
        ],
    )
    .unwrap()
}

#[test]
fn window_ops_with_partitions() {
    let t = series();
    let w = |op| nums(&window(&t, op, "v", &["k"], &[("t", false)], "w").unwrap(), "w");
    // Rows: a@2, b@1, a@1, b@2, a@3, b@3.
    assert_nums(&w(WindowOp::Cumsum), &[30.0, 5.0, 10.0, 10.0, 70.0, 20.0]);
    assert_nums(&w(WindowOp::Lag(1)), &[10.0, f64::NAN, f64::NAN, 5.0, 20.0, 5.0]);
    assert_nums(&w(WindowOp::Lead(1)), &[40.0, 5.0, 20.0, 10.0, f64::NAN, f64::NAN]);
    assert_nums(&w(WindowOp::RollingMean(2)), &[15.0, 5.0, 10.0, 5.0, 30.0, 7.5]);
    assert_nums(&w(WindowOp::ShareOfTotal), &[20.0 / 70.0, 0.25, 10.0 / 70.0, 0.25, 40.0 / 70.0, 0.5]);
    assert_nums(&w(WindowOp::PctChange), &[1.0, f64::NAN, f64::NAN, 0.0, 1.0, 1.0]);
    // Rank within each partition by the order keys (time here).
    assert_nums(&w(WindowOp::Rank), &[2.0, 1.0, 1.0, 2.0, 3.0, 3.0]);
    // Output rows stay in input order.
    let out = window(&t, WindowOp::Cumsum, "v", &["k"], &[("t", false)], "w").unwrap();
    assert_eq!(out.num("t").unwrap(), t.num("t").unwrap());
    assert!(window(&t, WindowOp::RollingMean(0), "v", &[], &[], "w").is_err());
}

#[test]
fn rolling_windows_ema_running_extremes_and_first_last() {
    let t = series();
    let w = |op| nums(&window(&t, op, "v", &["k"], &[("t", false)], "w").unwrap(), "w");
    // In time order: a = 10, 20, 40 (rows 2, 0, 4); b = 5, 5, 10 (rows 1, 3, 5).
    assert_nums(&w(WindowOp::RollingSum(2)), &[30.0, 5.0, 10.0, 10.0, 60.0, 15.0]);
    assert_nums(&w(WindowOp::RollingStd(2)), &[5.0, 0.0, 0.0, 0.0, 10.0, 2.5]);
    assert_nums(&w(WindowOp::RollingMin(2)), &[10.0, 5.0, 10.0, 5.0, 20.0, 5.0]);
    assert_nums(&w(WindowOp::RollingMax(2)), &[20.0, 5.0, 10.0, 5.0, 40.0, 10.0]);
    // Span 3: α = 0.5, seeded with the first value.
    assert_nums(&w(WindowOp::Ema(3)), &[15.0, 5.0, 10.0, 5.0, 27.5, 7.5]);
    assert_nums(&w(WindowOp::Cummax), &[20.0, 5.0, 10.0, 5.0, 40.0, 10.0]);
    assert_nums(&w(WindowOp::Cummin), &[10.0, 5.0, 10.0, 5.0, 10.0, 5.0]);
    assert_nums(&w(WindowOp::First), &[10.0, 5.0, 10.0, 5.0, 10.0, 5.0]);
    assert_nums(&w(WindowOp::Last), &[40.0, 10.0, 40.0, 10.0, 40.0, 10.0]);
    for op in [WindowOp::RollingSum(0), WindowOp::RollingStd(0), WindowOp::RollingMin(0), WindowOp::RollingMax(0), WindowOp::Ema(0)] {
        assert!(window(&t, op, "v", &[], &[], "w").is_err(), "{op:?}");
    }
}

#[test]
fn windows_can_refuse_partial_starts() {
    let t = series();
    let strict = |op| nums(&window_with(&t, op, "v", &["k"], &[("t", false)], "w", WindowOpts { min_periods: 2 }).unwrap(), "w");
    // Each partition's first row has one value: null instead of a one-value "average".
    assert_nums(&strict(WindowOp::RollingMean(2)), &[15.0, f64::NAN, f64::NAN, 5.0, 30.0, 7.5]);
    assert_nums(&strict(WindowOp::Ema(3)), &[15.0, f64::NAN, f64::NAN, 5.0, 27.5, 7.5]);
    assert_nums(&strict(WindowOp::RollingStd(3)), &[5.0, f64::NAN, f64::NAN, 0.0, (1400.0f64 / 9.0).sqrt(), (50.0f64 / 9.0).sqrt()]);
}

#[test]
fn window_nulls_are_skipped_not_zero() {
    let t = Table::from_columns("n", vec![("v", Column::Num(vec![f64::NAN, 1.0, f64::NAN, 3.0]))]).unwrap();
    let w = |op| nums(&window(&t, op, "v", &[], &[], "w").unwrap(), "w");
    assert_nums(&w(WindowOp::Cummax), &[f64::NAN, 1.0, 1.0, 3.0]);
    // A null row repeats the running average.
    assert_nums(&w(WindowOp::Ema(3)), &[f64::NAN, 1.0, 1.0, 2.0]);
    assert_nums(&w(WindowOp::First), &[1.0, 1.0, 1.0, 1.0]);
    assert_nums(&w(WindowOp::RollingMin(2)), &[f64::NAN, 1.0, 1.0, 3.0]);
    // First / last keep the column's type.
    let s = Table::from_columns("s", vec![("x", Column::strs(&["a", "b", "c"]))]).unwrap();
    assert_eq!(strs(&window(&s, WindowOp::Last, "x", &[], &[], "l").unwrap(), "l"), vec![s_("c"), s_("c"), s_("c")]);
}

#[test]
fn ranks_with_ties() {
    let t = Table::from_columns("r", vec![("v", Column::Num(vec![10.0, 30.0, 20.0, 30.0, f64::NAN, 10.0]))]).unwrap();
    // No order keys: rank by the column, largest first; nulls last.
    let r = nums(&window(&t, WindowOp::Rank, "v", &[], &[], "r").unwrap(), "r");
    assert_nums(&r, &[4.0, 1.0, 3.0, 1.0, 6.0, 4.0]);
    let dr = nums(&window(&t, WindowOp::DenseRank, "v", &[], &[], "r").unwrap(), "r");
    assert_nums(&dr, &[3.0, 1.0, 2.0, 1.0, 4.0, 3.0]);
    // Ascending by explicit order keys.
    let asc = nums(&window(&t, WindowOp::Rank, "v", &[], &[("v", false)], "r").unwrap(), "r");
    assert_nums(&asc, &[1.0, 4.0, 3.0, 4.0, 6.0, 1.0]);
    // Lag keeps the column's type.
    let s = Table::from_columns("s", vec![("x", Column::strs(&["a", "b", "c"]))]).unwrap();
    let l = window(&s, WindowOp::Lag(2), "x", &[], &[], "l").unwrap();
    assert_eq!(strs(&l, "l"), vec![None, None, s_("a")]);
}

fn s_(v: &str) -> Option<String> {
    s(v)
}

#[test]
fn joins() {
    let people = Table::from_columns("people", vec![("id", Column::Num(vec![1.0, 2.0, 3.0, 4.0])), ("name", Column::strs(&["Ann", "Bo", "Cy", "Di"])), ("city", Column::strs(&["U", "S", "X", "S"]))])
        .unwrap()
        .with_key(&["id"])
        .unwrap();
    let cities = Table::from_columns("cities", vec![("city", Column::strs(&["S", "U"])), ("name", Column::strs(&["Stockholm", "Uppsala"])), ("pop", Column::Num(vec![980.0, 230.0]))])
        .unwrap()
        .with_key(&["city"])
        .unwrap();
    let inner = join(&people, &cities, &["city"], JoinKind::Inner).unwrap();
    assert_eq!(inner.column_names(), vec!["id", "name", "city", "name_cities", "pop"]);
    assert_nums(&nums(&inner, "id"), &[1.0, 2.0, 4.0]);
    assert_eq!(strs(&inner, "name_cities"), vec![s("Uppsala"), s("Stockholm"), s("Stockholm")]);
    assert_eq!(inner.key, vec!["id"], "a lookup join keeps the left key");
    let left = join(&people, &cities, &["city"], JoinKind::Left).unwrap();
    assert_eq!(left.len(), 4);
    assert!(nums(&left, "pop")[2].is_nan());
    assert_eq!(strs(&left, "name_cities")[2], None);
    // One-to-many expands left rows (in right order) and extends the key.
    let visits = Table::from_columns("visits", vec![("vid", Column::Num(vec![10.0, 11.0, 12.0])), ("id", Column::Num(vec![2.0, 1.0, 2.0]))]).unwrap().with_key(&["vid"]).unwrap();
    let pv = join(&people, &visits, &["id"], JoinKind::Inner).unwrap();
    assert_nums(&nums(&pv, "id"), &[1.0, 2.0, 2.0]);
    assert_nums(&nums(&pv, "vid"), &[11.0, 10.0, 12.0]);
    assert_eq!(pv.key, vec!["id", "vid"]);
    // A text code never silently fails to match a number.
    let codes = Table::from_columns("c", vec![("id", Column::strs(&["1"]))]).unwrap();
    assert!(matches!(join(&people, &codes, &["id"], JoinKind::Inner), Err(DataError::TypeMismatch { .. })));
    // Null keys never match.
    let nully = Table::from_columns("n", vec![("city", Column::strs_opt(&[None::<&str>, Some("S")])), ("z", Column::Num(vec![1.0, 2.0]))]).unwrap();
    let nn = join(&nully, &Table::from_columns("m", vec![("city", Column::strs_opt(&[None::<&str>])), ("w", Column::Num(vec![9.0]))]).unwrap(), &["city"], JoinKind::Left).unwrap();
    assert!(nums(&nn, "w").iter().all(|v| v.is_nan()));
}

#[test]
fn joins_with_an_empty_table_match_nothing() {
    // A table filtered to nothing whose key column came out numeric (a `derive` over zero rows
    // can't tell) joined against text codes: nothing matches, which is not a type error.
    let regions = Table::from_columns("regions", vec![("id", Column::strs(&["2081", "2080"])), ("name", Column::strs(&["Borlänge", "Falun"]))]).unwrap();
    let none = Table::from_columns("none", vec![("id", Column::Num(vec![])), ("v", Column::Num(vec![]))]).unwrap();
    let inner = join(&regions, &none, &["id"], JoinKind::Inner).unwrap();
    assert_eq!(inner.len(), 0, "an inner join with an empty table keeps no row");
    assert_eq!(inner.column_names(), vec!["id", "name", "v"]);
    let left = join(&regions, &none, &["id"], JoinKind::Left).unwrap();
    assert_eq!(strs(&left, "name"), vec![s("Borlänge"), s("Falun")]);
    assert!(nums(&left, "v").iter().all(|v| v.is_nan()));
    // And from the other side.
    assert_eq!(join(&none, &regions, &["id"], JoinKind::Left).unwrap().len(), 0);
}

#[test]
fn pivot_round_trip() {
    let long = Table::from_columns(
        "long",
        vec![("country", Column::strs(&["SE", "SE", "NO", "NO", "DK"])), ("year", Column::strs(&["2020", "2021", "2020", "2021", "2021"])), ("v", Column::Num(vec![1.0, 2.0, 3.0, 4.0, 5.0]))],
    )
    .unwrap();
    let wide = pivot(&long, &["country"], "year", "v").unwrap();
    assert_eq!(wide.column_names(), vec!["country", "2020", "2021"]);
    assert_eq!(wide.key, vec!["country"]);
    assert_nums(&nums(&wide, "2020"), &[1.0, 3.0, f64::NAN]);
    assert_nums(&nums(&wide, "2021"), &[2.0, 4.0, 5.0]);
    let back = unpivot(&wide, &["country"], &[], "year", "v").unwrap();
    assert_eq!(back.key, vec!["country", "year"]);
    // Missing combinations come back as nulls; drop them and it's the original table.
    let back = filter_where(&back, "v", &Pred::NotNull).unwrap();
    let mut orig = long.clone();
    orig.key = back.key.clone();
    assert_eq!(back, orig);
    // A repeated (index, name) pair is ambiguous.
    let dup = Table::from_columns("d", vec![("c", Column::strs(&["SE", "SE"])), ("y", Column::strs(&["2020", "2020"])), ("v", Column::Num(vec![1.0, 2.0]))]).unwrap();
    let e = pivot(&dup, &["c"], "y", "v").unwrap_err();
    assert!(e.to_string().contains("rows 0 and 1"), "{e}");
    // Numeric names become column names by their text.
    let numeric = Table::from_columns("n", vec![("c", Column::strs(&["SE"])), ("y", Column::Num(vec![2020.0])), ("v", Column::Num(vec![1.0]))]).unwrap();
    assert_eq!(pivot(&numeric, &["c"], "y", "v").unwrap().column_names(), vec!["c", "2020"]);
    // Unpivot needs value columns of one type.
    let mixed = Table::from_columns("m", vec![("id", Column::strs(&["a"])), ("x", Column::Num(vec![1.0])), ("y", Column::strs(&["s"]))]).unwrap();
    assert!(unpivot(&mixed, &["id"], &[], "k", "v").is_err());
}

#[test]
fn union_matches_columns_by_name() {
    let a = Table::from_columns("a", vec![("k", Column::strs(&["x"])), ("v", Column::Num(vec![1.0]))]).unwrap();
    let b = Table::from_columns("b", vec![("v", Column::Num(vec![2.0])), ("k", Column::strs(&["y"])), ("extra", Column::Bool(vec![true]))]).unwrap();
    let u = union(&[&a, &b]).unwrap();
    assert_eq!(u.column_names(), vec!["k", "v", "extra"]);
    assert_eq!(strs(&u, "k"), vec![s("x"), s("y")]);
    assert_eq!(u.bool("extra").unwrap(), &[false, true]);
    let c = Table::from_columns("c", vec![("v", Column::strs(&["oops"]))]).unwrap();
    assert!(matches!(union(&[&a, &c]), Err(DataError::TypeMismatch { .. })));
    assert_eq!(union(&[]).unwrap().len(), 0);
}

#[test]
fn seeded_sampling() {
    let t = Table::from_columns("t", vec![("i", Column::Num((0..100).map(f64::from).collect()))]).unwrap();
    let a = sample(&t, 10, 42);
    let b = sample(&t, 10, 42);
    assert_eq!(a, b, "same seed, same rows");
    assert_eq!(a.len(), 10);
    let v = nums(&a, "i");
    assert!(v.windows(2).all(|w| w[0] < w[1]), "original order kept: {v:?}");
    assert_ne!(sample(&t, 10, 7), a, "another seed, other rows");
    assert_eq!(sample(&t, 1000, 1).len(), 100);
    // Pinned so the sequence can't drift between targets or releases.
    assert_eq!(nums(&sample(&t, 3, 1), "i"), PINNED_SAMPLE.to_vec(), "{:?}", nums(&sample(&t, 3, 1), "i"));
}

const PINNED_SAMPLE: [f64; 3] = [48.0, 57.0, 74.0];

#[test]
fn interpolate_at_a_data_time() {
    let t = Table::from_columns(
        "race",
        vec![
            ("name", Column::strs(&["A", "B", "A", "B", "A", "C"])),
            ("year", Column::Num(vec![2000.0, 2000.0, 2010.0, 2010.0, 2020.0, 2015.0])),
            ("value", Column::Num(vec![0.0, 100.0, 10.0, 50.0, 30.0, 7.0])),
            ("group", Column::strs(&["x", "y", "x2", "y", "x3", "z"])),
        ],
    )
    .unwrap();
    let at = |time: f64| interpolate_at(&t, "name", "year", "value", time).unwrap();
    let r = at(2005.0);
    assert_eq!(strs(&r, "name"), vec![s("A"), s("B"), s("C")]);
    assert_eq!(r.key, vec!["name"]);
    assert_nums(&nums(&r, "value"), &[5.0, 75.0, 7.0]);
    assert_nums(&nums(&r, "year"), &[2005.0, 2005.0, 2005.0]);
    // Other columns come from the latest observation at or before t (or the first).
    assert_eq!(strs(&r, "group"), vec![s("x"), s("y"), s("z")]);
    assert_nums(&nums(&at(2015.0), "value"), &[20.0, 50.0, 7.0]);
    assert_eq!(strs(&at(2015.0), "group"), vec![s("x2"), s("y"), s("z")]);
    // Clamped at both ends; exact at observations.
    assert_nums(&nums(&at(1990.0), "value"), &[0.0, 100.0, 7.0]);
    assert_nums(&nums(&at(2030.0), "value"), &[30.0, 50.0, 7.0]);
    assert_nums(&nums(&at(2010.0), "value"), &[10.0, 50.0, 7.0]);
    assert_eq!(strs(&at(2030.0), "group"), vec![s("x3"), s("y"), s("z")]);
    // Date times interpolate by day.
    let dt = Table::from_columns(
        "d",
        vec![("k", Column::strs(&["a", "a"])), ("day", Column::Date(vec![Some(d(2024, 1, 1)), Some(d(2024, 1, 11))])), ("v", Column::Num(vec![0.0, 10.0]))],
    )
    .unwrap();
    let r = interpolate_at(&dt, "k", "day", "v", d(2024, 1, 4) as f64).unwrap();
    assert_nums(&nums(&r, "v"), &[3.0]);
    assert_eq!(r.column("day").unwrap().ty(), ColumnType::Num);
    assert!(interpolate_at(&t, "name", "group", "value", 0.0).is_err());
}
