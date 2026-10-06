use datars_data::{Column, ColumnType, DataError, Field, Table, Value};
use datars_scene::{Key, KeyPart};

fn t() -> Table {
    Table::from_columns(
        "t",
        vec![
            ("region", Column::strs(&["N", "N", "S", "S"])),
            ("year", Column::Num(vec![2020.0, 2021.0, 2020.0, 2021.0])),
            ("value", Column::Num(vec![1.5, 2.0, f64::NAN, 4.0])),
            ("day", Column::Date(vec![Some(0), Some(1), Some(2), None])),
            ("flag", Column::Bool(vec![true, false, true, false])),
        ],
    )
    .unwrap()
}

#[test]
fn row_keys_from_key_columns() {
    let k = t().with_key(&["region", "year"]).unwrap();
    assert!(k.has_stable_key());
    assert_eq!(k.row_key(0), Key::new(vec![KeyPart::str("N"), KeyPart::Int(2020)]));
    assert_eq!(k.key_of(3), Key::new(vec![KeyPart::str("S"), KeyPart::Int(2021)]));
    assert_eq!(k.keys().len(), 4);
    let d = t().select(&["day"]).unwrap();
    let d = d.take_rows(&[0, 1, 2]).with_key(&["day"]).unwrap();
    assert_eq!(d.row_key(2), Key::one(KeyPart::Int(2)), "dates key as whole days");
    let f = Table::from_columns("f", vec![("x", Column::Num(vec![0.5, -2.0]))]).unwrap().with_key(&["x"]).unwrap();
    assert_eq!(f.row_key(0), Key::one(KeyPart::str("0.5")), "non-integral numbers key as their text");
    assert_eq!(f.row_key(1), Key::one(KeyPart::Int(-2)));
}

#[test]
fn without_a_key_rows_are_their_index() {
    let t = t();
    assert!(!t.has_stable_key());
    assert_eq!(t.row_key(2), Key::one(KeyPart::Int(2)));
    assert!(t.validate_keys().is_ok());
}

#[test]
fn duplicate_keys_are_an_error_with_examples() {
    let t = Table::from_columns("t", vec![("k", Column::strs(&["a", "b", "a", "c", "b", "a"])), ("v", Column::Num(vec![0.0; 6]))]).unwrap();
    let e = t.clone().with_key(&["k"]).unwrap_err();
    assert_eq!(
        e,
        DataError::DuplicateKeys { key: vec!["k".into()], count: 3, examples: vec![("(\"a\",)".into(), vec![0, 2, 5]), ("(\"b\",)".into(), vec![1, 4])] }
    );
    assert_eq!(e.to_string(), "duplicate keys on (k): 3 row(s) repeat an earlier key — (\"a\",) at rows 0, 2, 5; (\"b\",) at rows 1, 4");
    // Multi-column keys are unique as tuples.
    let m = Table::from_columns("m", vec![("a", Column::strs(&["x", "x"])), ("b", Column::Num(vec![1.0, 2.0]))]).unwrap();
    assert!(m.clone().with_key(&["a", "b"]).is_ok());
    assert!(m.with_key(&["a"]).is_err());
}

#[test]
fn null_and_missing_keys() {
    let t = t();
    let e = t.clone().with_key(&["value"]).unwrap_err();
    assert_eq!(e, DataError::NullKey { column: "value".into(), row: 2 });
    let e = t.clone().with_key(&["nope"]).unwrap_err();
    match &e {
        DataError::UnknownColumn { name, available } => {
            assert_eq!(name, "nope");
            assert_eq!(available.len(), 5);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(e.to_string(), "no column \"nope\" (columns: \"region\", \"year\", \"value\", \"day\", \"flag\")");
}

#[test]
fn construction_checks_lengths_and_names() {
    let e = Table::from_columns("t", vec![("a", Column::Num(vec![1.0])), ("b", Column::Num(vec![1.0, 2.0]))]).unwrap_err();
    assert_eq!(e, DataError::LengthMismatch { column: "b".into(), expected: 1, found: 2 });
    assert!(Table::from_columns("t", vec![("a", Column::Num(vec![1.0])), ("a", Column::Num(vec![2.0]))]).is_err());
    let t = Table::new("empty").with_column("a", Column::Num(vec![1.0, 2.0])).unwrap();
    assert_eq!(t.len(), 2);
}

#[test]
fn schema_describes_the_table() {
    let s = t().with_key(&["region", "year"]).unwrap().schema();
    assert_eq!(s.rows, 4);
    assert_eq!(s.key, vec!["region", "year"]);
    assert_eq!(s.fields[2], Field { name: "value".into(), ty: ColumnType::Num, nulls: 1 });
    assert_eq!(s.field("day").unwrap().nulls, 1);
    assert_eq!(s.field("flag").unwrap().ty, ColumnType::Bool);
    let j = serde_json::to_string(&s).unwrap();
    assert!(j.contains(r#"{"name":"value","type":"num","nulls":1}"#), "{j}");
}

#[test]
fn values_and_rows() {
    let t = t();
    assert_eq!(t.row(0), vec![Value::str("N"), Value::Num(2020.0), Value::Num(1.5), Value::Date(0), Value::Bool(true)]);
    assert_eq!(t.get("value", 2), Value::Null);
    assert_eq!(t.get("nope", 0), Value::Null);
    assert_eq!(t.column("day").unwrap().f64_at(1), 1.0);
    let c = Column::from_values(ColumnType::Str, &[Value::Num(2.0), Value::Null, Value::Date(0)]);
    assert_eq!(c, Column::strs_opt(&[Some("2"), None, Some("1970-01-01")]));
    assert_eq!(Column::infer_type(&[Value::Null, Value::Num(1.0)], ColumnType::Str), ColumnType::Num);
    assert_eq!(Column::infer_type(&[Value::Bool(true), Value::Num(1.0)], ColumnType::Num), ColumnType::Str);
}

#[test]
fn drop_and_rename() {
    let t = t().with_key(&["region", "year"]).unwrap();
    let d = t.drop_columns(&["value", "flag"]).unwrap();
    assert_eq!(d.column_names(), vec!["region", "year", "day"]);
    assert_eq!(d.key, t.key);
    assert!(t.drop_columns(&["nope"]).is_err());
    let r = t.rename("year", "yr").unwrap();
    assert_eq!(r.key, vec!["region", "yr"]);
    assert!(t.rename("nope", "x").is_err());
}

#[test]
fn stable_hash_and_json_form() {
    let a = t().with_key(&["region", "year"]).unwrap();
    let b = Table::from_json(&a.to_json()).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.hash(), b.hash());
    // Pinned: the hash is part of caches and content addresses and must not drift by platform.
    let small = Table::from_columns("h", vec![("a", Column::Num(vec![1.0, f64::NAN])), ("b", Column::strs_opt(&[Some("x"), None]))]).unwrap();
    assert_eq!(small.hash(), PINNED_HASH, "{:#x}", small.hash());
    let neg_zero = Table::from_columns("h", vec![("a", Column::Num(vec![-0.0, f64::NAN])), ("b", Column::strs_opt(&[Some("x"), None]))]).unwrap();
    let zero = Table::from_columns("h", vec![("a", Column::Num(vec![0.0, f64::NAN])), ("b", Column::strs_opt(&[Some("x"), None]))]).unwrap();
    assert_eq!(neg_zero.hash(), zero.hash());
}

const PINNED_HASH: u64 = 0x8e2f_5223_a02a_ca1b;
