use datars_data::date::date_from_ymd;
use datars_data::{read_csv, Column, ColumnType, CsvOptions, DataError, Table};

fn csv(s: &str) -> Table {
    read_csv(s.as_bytes(), &CsvOptions::default()).unwrap()
}

fn strs(t: &Table, c: &str) -> Vec<Option<String>> {
    t.str(c).unwrap_or_else(|| panic!("{c} is {:?}", t.column(c).map(Column::ty))).iter().map(|s| s.as_deref().map(str::to_string)).collect()
}

fn s(v: &str) -> Option<String> {
    Some(v.to_string())
}

#[test]
fn quotes_escaped_quotes_and_embedded_delimiters() {
    let t = csv("name,quote\n\"Smith, J\",\"He said \"\"hi\"\"\"\nplain,\"\"\n");
    assert_eq!(t.column_names(), vec!["name", "quote"]);
    assert_eq!(strs(&t, "name"), vec![s("Smith, J"), s("plain")]);
    assert_eq!(strs(&t, "quote"), vec![s("He said \"hi\""), None]);
}

#[test]
fn newlines_inside_quotes_keep_line_numbers_right() {
    let t = csv("a,b\n\"line one\nline two\",2\nx,3\n");
    assert_eq!(strs(&t, "a"), vec![s("line one\nline two"), s("x")]);
    assert_eq!(t.num("b").unwrap(), &[2.0, 3.0]);
    // The bad quote is on line 4 (the quoted newline counts as a line).
    let e = read_csv(b"a,b\n\"one\ntwo\",2\n\"x\"y,3\n", &CsvOptions::default()).unwrap_err();
    assert_eq!(e, DataError::Parse { line: 4, column: 4, message: "unexpected text after a closing quote (quotes inside a quoted field are written \"\")".into() });
}

#[test]
fn crlf_cr_and_missing_final_newline() {
    let t = csv("a,b\r\n1,2\r\n3,4");
    assert_eq!(t.num("a").unwrap(), &[1.0, 3.0]);
    assert_eq!(t.num("b").unwrap(), &[2.0, 4.0]);
    let t = csv("a,b\r1,2\r3,4\r");
    assert_eq!(t.len(), 2);
    let t = csv("a\r\n\"x\r\ny\"\r\n");
    assert_eq!(strs(&t, "a"), vec![s("x\r\ny")]);
}

#[test]
fn byte_order_mark_is_skipped() {
    let t = read_csv(b"\xEF\xBB\xBFname,v\nx,1\n", &CsvOptions::default()).unwrap();
    assert_eq!(t.column_names(), vec!["name", "v"]);
}

#[test]
fn semicolons_and_decimal_commas() {
    let t = csv("kommun;andel;antal\nStockholm;12,5;1 234\nGöteborg;-3,25;2 000,5\nMalmö;0,5e1;7\n");
    assert_eq!(t.num("andel").unwrap(), &[12.5, -3.25, 5.0]);
    // "1 234" has no decimal comma, so it isn't a number: the column stays text.
    assert_eq!(t.column("antal").unwrap().ty(), ColumnType::Str);
    let t = csv("a;b\n1 234,5;1.234,5\n");
    assert_eq!(t.num("a").unwrap(), &[1234.5]);
    assert_eq!(t.num("b").unwrap(), &[1234.5]);
    // Commas with a comma delimiter are never decimal separators…
    let t = csv("a,b\n\"1,5\",2\n");
    assert_eq!(t.column("a").unwrap().ty(), ColumnType::Str);
    // …unless asked.
    let t = read_csv(b"a,b\n\"1,5\",2\n", &CsvOptions::default().decimal_comma(true)).unwrap();
    assert_eq!(t.num("a").unwrap(), &[1.5]);
    // And a semicolon file can opt out.
    let t = read_csv(b"a;b\n1,5;2\n", &CsvOptions::default().decimal_comma(false)).unwrap();
    assert_eq!(t.column("a").unwrap().ty(), ColumnType::Str);
}

#[test]
fn tabs_are_detected() {
    let t = csv("city\tpop\nUppsala, Sweden\t230000\n");
    assert_eq!(strs(&t, "city"), vec![s("Uppsala, Sweden")]);
    assert_eq!(t.num("pop").unwrap(), &[230000.0]);
    let t = read_csv(b"a|b\n1|2\n", &CsvOptions::default().delimiter(b'|')).unwrap();
    assert_eq!(t.num("b").unwrap(), &[2.0]);
}

#[test]
fn empty_cells_are_null() {
    let t = csv("a,b,c\n1,,2020-01-01\n,x,\n");
    let a = t.num("a").unwrap();
    assert_eq!(a[0], 1.0);
    assert!(a[1].is_nan());
    assert_eq!(strs(&t, "b"), vec![None, s("x")]);
    assert_eq!(t.date("c").unwrap(), &[date_from_ymd(2020, 1, 1), None]);
    // Short rows are padded with nulls; blank lines are skipped.
    let t = csv("a,b,c\n1,2\n\n3,4,5\n");
    assert_eq!(t.len(), 2);
    assert!(t.num("c").unwrap()[0].is_nan());
}

#[test]
fn type_inference() {
    let t = csv(
        "flag,n,d,m,s,code,year,mixed\n\
         true,-1.5,2024-02-29,2024-02,hello,0114,1999,1\n\
         FALSE,2e3,2024-03-01,2024-03,world,0180,2000,two\n\
         True,+.5,1970-01-01,1970-01,x,0001,2001,3\n",
    );
    assert_eq!(t.bool("flag").unwrap(), &[true, false, true]);
    assert_eq!(t.num("n").unwrap(), &[-1.5, 2000.0, 0.5]);
    assert_eq!(t.date("d").unwrap(), &[date_from_ymd(2024, 2, 29), date_from_ymd(2024, 3, 1), Some(0)]);
    assert_eq!(t.date("m").unwrap(), &[date_from_ymd(2024, 2, 1), date_from_ymd(2024, 3, 1), Some(0)]);
    assert_eq!(t.column("s").unwrap().ty(), ColumnType::Str);
    // Zero-padded codes stay text, so joins on them keep working.
    assert_eq!(strs(&t, "code"), vec![s("0114"), s("0180"), s("0001")]);
    // Plain years stay numbers unless asked.
    assert_eq!(t.num("year").unwrap(), &[1999.0, 2000.0, 2001.0]);
    assert_eq!(strs(&t, "mixed"), vec![s("1"), s("two"), s("3")]);
    let y = read_csv(b"year,v\n1999,1\n2000,2\n", &CsvOptions::default().years_as_dates(true)).unwrap();
    assert_eq!(y.date("year").unwrap(), &[date_from_ymd(1999, 1, 1), date_from_ymd(2000, 1, 1)]);
    assert_eq!(y.column("v").unwrap().ty(), ColumnType::Num, "only four-digit columns become dates");
}

#[test]
fn missing_value_markers_only_in_typed_columns() {
    let t = csv("iso,v,d\nSE,1,2020-01-01\nNA,NA,..\nNO,-,2020-01-03\n");
    // Namibia is "NA": a text column keeps it.
    assert_eq!(strs(&t, "iso"), vec![s("SE"), s("NA"), s("NO")]);
    let v = t.num("v").unwrap();
    assert_eq!(v[0], 1.0);
    assert!(v[1].is_nan() && v[2].is_nan());
    assert_eq!(t.date("d").unwrap()[1], None);
    // Booleans can't hold nulls, so a boolean column with gaps is text.
    let t = csv("b\ntrue\n\nfalse\n,\n");
    assert_eq!(t.column("b").unwrap().ty(), ColumnType::Str);
}

#[test]
fn type_overrides() {
    let opts = CsvOptions::default()
        .column_type("id", ColumnType::Num)
        .column_type("zip", ColumnType::Str)
        .column_type("year", ColumnType::Date)
        .column_type("ok", ColumnType::Bool);
    let t = read_csv(b"id,zip,year,ok\n0114,12345,2020,yes\n0115,54321,2021-06,0\n,,,\n", &opts).unwrap();
    assert_eq!(t.num("id").unwrap()[..2], [114.0, 115.0]);
    assert_eq!(strs(&t, "zip"), vec![s("12345"), s("54321"), None]);
    assert_eq!(t.date("year").unwrap(), &[date_from_ymd(2020, 1, 1), date_from_ymd(2021, 6, 1), None]);
    assert_eq!(t.bool("ok").unwrap(), &[true, false, false]);
    // A bad cell in an overridden column is reported where it is.
    let e = read_csv(b"a,v\nx,1\ny, abc\n", &CsvOptions::default().column_type("v", ColumnType::Num)).unwrap_err();
    assert_eq!(e, DataError::Parse { line: 3, column: 3, message: "column \"v\": \"abc\" is not a number".into() });
    assert_eq!(e.to_string(), "line 3, column 3: column \"v\": \"abc\" is not a number");
    // Overriding a column that doesn't exist is a mistake worth reporting.
    let e = read_csv(b"a\n1\n", &CsvOptions::default().column_type("b", ColumnType::Num)).unwrap_err();
    assert!(matches!(e, DataError::UnknownColumn { ref name, .. } if name == "b"));
}

#[test]
fn key_columns_are_validated() {
    let t = read_csv(b"id,v\na,1\nb,2\n", &CsvOptions::default().key(&["id"])).unwrap();
    assert_eq!(t.key, vec!["id"]);
    assert!(t.has_stable_key());
    let e = read_csv(b"id,v\na,1\nb,2\na,3\n", &CsvOptions::default().key(&["id"])).unwrap_err();
    assert!(matches!(e, DataError::DuplicateKeys { count: 1, .. }), "{e}");
    let e = read_csv(b"id,v\na,1\n", &CsvOptions::default().key(&["nope"])).unwrap_err();
    assert!(matches!(e, DataError::UnknownColumn { .. }));
}

#[test]
fn headers_are_cleaned_up() {
    let t = csv(" a ,a,,b\n1,2,3,4\n");
    assert_eq!(t.column_names(), vec!["a", "a_2", "column_3", "b"]);
    let t = read_csv(b"1,2\n3,4,5\n", &CsvOptions::default().no_header()).unwrap();
    assert_eq!(t.column_names(), vec!["column_1", "column_2", "column_3"]);
    assert_eq!(t.num("column_1").unwrap(), &[1.0, 3.0]);
    assert!(t.num("column_3").unwrap()[0].is_nan());
}

#[test]
fn spaces_around_unquoted_fields_are_trimmed() {
    let t = csv("a, b\n 1 , \"  kept  \"\n");
    assert_eq!(t.column_names(), vec!["a", "b"]);
    assert_eq!(t.num("a").unwrap(), &[1.0]);
    assert_eq!(strs(&t, "b"), vec![s("  kept  ")]);
}

#[test]
fn malformed_input_errors_point_at_the_problem() {
    let e = read_csv(b"a,b\n1,\"open\n2,3\n", &CsvOptions::default()).unwrap_err();
    assert_eq!(e, DataError::Parse { line: 2, column: 3, message: "unterminated quoted field (the quote opened here is never closed)".into() });
    let e = read_csv(b"a,b\n1,2,3\n", &CsvOptions::default()).unwrap_err();
    match e {
        DataError::Parse { line: 2, message, .. } => assert!(message.contains("3 fields, but the header has 2"), "{message}"),
        other => panic!("{other:?}"),
    }
    // A trailing delimiter (an empty extra field) is tolerated.
    assert_eq!(csv("a,b\n1,2,\n").len(), 1);
}

#[test]
fn empty_input_is_an_empty_table() {
    let t = csv("");
    assert_eq!((t.len(), t.width()), (0, 0));
    let t = csv("a,b\n");
    assert_eq!((t.len(), t.width()), (0, 2));
}

#[test]
fn table_name_and_determinism() {
    let src = b"x,y\n1,a\n2,b\n";
    let a = read_csv(src, &CsvOptions::named("pts")).unwrap();
    let b = read_csv(src, &CsvOptions::named("pts")).unwrap();
    assert_eq!(a.name, "pts");
    assert_eq!(a.hash(), b.hash());
}
