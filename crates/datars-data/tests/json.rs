use datars_data::date::date_from_ymd;
use datars_data::{read_json, Column, ColumnType, DataError, Table, Value};

fn strs(t: &Table, c: &str) -> Vec<Option<String>> {
    t.str(c).unwrap().iter().map(|s| s.as_deref().map(str::to_string)).collect()
}

#[test]
fn array_of_records() {
    let t = read_json(br#"[{"b": "x", "a": 1}, {"a": 2.5, "c": true}, {"b": "z"}]"#).unwrap();
    // Columns in order of first appearance (not alphabetical).
    assert_eq!(t.column_names(), vec!["b", "a", "c"]);
    assert_eq!(strs(&t, "b"), vec![Some("x".into()), None, Some("z".into())]);
    let a = t.num("a").unwrap();
    assert_eq!(a[..2], [1.0, 2.5]);
    assert!(a[2].is_nan());
    // Booleans with gaps become text (Bool columns have no null).
    assert_eq!(strs(&t, "c"), vec![None, Some("true".into()), None]);
}

#[test]
fn object_of_columns_keeps_order_and_types() {
    let t = read_json(br#"{"zeta": [3, 1, 2], "alpha": ["a", "b", null], "ok": [true, false, true], "d": ["2024-01-31", null, "2024-03"]}"#).unwrap();
    assert_eq!(t.column_names(), vec!["zeta", "alpha", "ok", "d"]);
    assert_eq!(t.num("zeta").unwrap(), &[3.0, 1.0, 2.0]);
    assert_eq!(t.bool("ok").unwrap(), &[true, false, true]);
    assert_eq!(t.date("d").unwrap(), &[date_from_ymd(2024, 1, 31), None, date_from_ymd(2024, 3, 1)]);
    assert_eq!(t.column("alpha").unwrap().ty(), ColumnType::Str);
}

#[test]
fn mixed_columns_become_text() {
    let t = read_json(br#"[{"v": 1}, {"v": "two"}, {"v": 3.5}, {"v": false}]"#).unwrap();
    assert_eq!(strs(&t, "v"), vec![Some("1".into()), Some("two".into()), Some("3.5".into()), Some("false".into())]);
    // Strings that merely look like dates in part stay text.
    let t = read_json(br#"{"d": ["2024-01-31", "soon"]}"#).unwrap();
    assert_eq!(t.column("d").unwrap().ty(), ColumnType::Str);
}

#[test]
fn errors_say_where() {
    let e = read_json(b"[{\"a\": 1},\n {\"a\": }]").unwrap_err();
    assert_eq!(e.to_string(), "line 2, column 8: expected value");
    match e {
        DataError::Parse { line: 2, column, .. } => assert!(column > 0),
        other => panic!("{other:?}"),
    }
    let e = read_json(br#"[{"a": {"nested": 1}}]"#).unwrap_err();
    assert_eq!(e.to_string(), "record 0, field \"a\": nested arrays and objects aren't supported (flatten them first)");
    let e = read_json(br#"[1, 2]"#).unwrap_err();
    assert!(e.to_string().contains("record 0 is not an object"), "{e}");
    let e = read_json(br#"{"a": [1, 2], "b": [1]}"#).unwrap_err();
    assert_eq!(e.to_string(), "column \"b\" has 1 values, column \"a\" has 2");
    let e = read_json(br#"{"a": 1}"#).unwrap_err();
    assert!(e.to_string().contains("column \"a\" is not an array"), "{e}");
    let e = read_json(b"42").unwrap_err();
    assert!(e.to_string().contains("expected an array of records or an object of columns"), "{e}");
}

#[test]
fn empty_inputs() {
    let t = read_json(b"[]").unwrap();
    assert_eq!((t.len(), t.width()), (0, 0));
    let t = read_json(b"{}").unwrap();
    assert_eq!((t.len(), t.width()), (0, 0));
}

#[test]
fn table_serde_form_round_trips_through_read_json() {
    let t = Table::from_columns(
        "votes",
        vec![("party", Column::strs(&["S", "M"])), ("share", Column::Num(vec![30.3, f64::NAN])), ("day", Column::Date(vec![Some(19_000), None]))],
    )
    .unwrap()
    .with_key(&["party"])
    .unwrap();
    let back = read_json(t.to_json().as_bytes()).unwrap();
    assert_eq!(back, t);
    assert_eq!(back.key, vec!["party"]);
    assert_eq!(back.get("day", 0), Value::Date(19_000));
    // The serde form is validated too.
    let e = read_json(br#"{"name": "t", "columns": [{"name": "a", "type": "num", "values": [1]}, {"name": "b", "type": "num", "values": [1, 2]}]}"#).unwrap_err();
    assert!(e.to_string().contains("column \"b\" has 2 values"), "{e}");
    let e = read_json(br#"{"columns": [{"name": "a", "type": "date", "values": ["nope"]}]}"#).unwrap_err();
    assert!(e.to_string().contains("expected a date"), "{e}");
}

#[test]
fn bom_is_skipped() {
    let t = read_json(b"\xEF\xBB\xBF[{\"a\": 1}]").unwrap();
    assert_eq!(t.num("a").unwrap(), &[1.0]);
}
