//! Parser and canonical printer: round trips, lambda forms, precedence, templates, host calls,
//! methods, JSON AST, and syntax errors with spans.

use datars_expr::{parse, BinOp, Expr, UnOp};

fn p(src: &str) -> Expr {
    parse(src).unwrap_or_else(|e| panic!("{}", e.render(src)))
}

fn perr(src: &str) -> datars_expr::ParseError {
    match parse(src) {
        Ok(e) => panic!("expected a syntax error for {src:?}, parsed {e}"),
        Err(e) => e,
    }
}

/// Every expression here must print to canonical source that re-parses to an equal tree, print
/// the same way again (a fixed point), and survive the JSON AST.
const ROUND_TRIPS: &[&str] = &[
    "1",
    "0",
    "-0",
    "-3",
    "-(3)",
    "-(-3)",
    "- -x",
    "-(-x)",
    "+x",
    "+(+x)",
    "!x",
    "!!x",
    "!-3",
    "+-3",
    ".5",
    "1e3",
    "1.5e-7",
    "1e21",
    "2.5e300",
    "0x1f",
    "1_000_000",
    "NaN",
    "Infinity",
    "-Infinity",
    "PI",
    "Math.E",
    "true",
    "false",
    "null",
    "undefined",
    "'single'",
    "\"double\"",
    "\"esc \\\" \\\\ \\n \\t \\u{1F600} \\x41 \\u00e9\"",
    "'it\\'s'",
    "\"\\u0001\"",
    "[1, 2, 3]",
    "[1, 'a', true, null][d.i]",
    "[]",
    "[1, 2,]",
    "d.value",
    "d.value * 2 + 1",
    "d[\"GDP per capita\"]",
    "d['with.dot'] + 1",
    "d.length",
    "d.true",
    "a + b * c",
    "(a + b) * c",
    "a - (b - c)",
    "a - b - c",
    "a / b / c",
    "a / (b / c)",
    "a % b",
    "2 ** 3 ** 2",
    "(2 ** 3) ** 2",
    "(-2) ** 2",
    "-(2 ** 2)",
    "2 ** -1",
    "a < b == c < x",
    "a <= b && c >= x || !e",
    "a && (b || c)",
    "a || b && c",
    "(a || b) && c",
    "a ?? b",
    "a ?? b ?? c",
    "(a ?? b) || c",
    "a ?? (b && c)",
    "(a || b) ?? c",
    "a === b",
    "a !== b",
    "a != b",
    "a ? b : c",
    "a ? b : c ? x : e",
    "(a ? b : c) ? x : e",
    "a ? b ? c : x : e",
    "a || b ? c : x",
    "(a ? b : c) + 1",
    "d.share > 30 ? palette.accent : \"#ccc\"",
    "d => d.share > 30 ? palette.accent : \"#ccc\"",
    "(d, i) => i % 2 == 0 ? d.a : d.b",
    "(d, i) => i",
    "(row) => row.x",
    "abs(-d.x)",
    "max(d.a, d.b, 0)",
    "min()",
    "round(d.value, 2)",
    "Math.round(d.v)",
    "Math.max(1, 2)",
    "clamp(d.x, 0, 1)",
    "lerp(0, 100, t)",
    "String(d.x).length",
    "Number(d.s) + 1",
    "isNaN(d.x) || Number.isFinite(d.y)",
    "format(d.value, \".1%\")",
    "scale.y(d.value)",
    "selected.has(d.party)",
    "selected.isEmpty() || selected.has(d.party) ? 1 : 0.3",
    "cx.token(\"accent\")",
    "a.b.c(1, 2)",
    "palette.accent",
    "x.y.z",
    "d.name.toUpperCase()",
    "d.name.toLowerCase().trim()",
    "d.name.slice(0, 3)",
    "d.name.slice(-2)",
    "d.name.startsWith(\"A\") && d.name.endsWith(\"z\")",
    "d.name.includes(\"x\")",
    "d.name.indexOf(\"x\")",
    "d.name.length",
    "d.name.length > 3",
    "\"abc\".length",
    "(name).toUpperCase()",
    "(1).toString()",
    "(-1).toString()",
    "(a + b).toString()",
    "[1, 2, 3].includes(d.x)",
    "['a', 'b'].indexOf(d.s)",
    "[1, 2].length",
    "d.s[0]",
    "(\"abc\")[1]",
    "`plain`",
    "``",
    "`${d.name}: ${d.value}`",
    "`a${1}b${d.x}c`",
    "`${`nested ${d.x}`}!`",
    "`tick \\` and \\${not} and $ alone`",
    "`${a ? b : c}`",
    "`line\\nbreak`",
    "d => `${d.name}`.length",
    "d => d.x + k",
    "() => k * 2",
    "(d, i) => d.x * i + offset",
    "d => { return d.x * 2; }",
];

#[test]
fn round_trips() {
    for src in ROUND_TRIPS {
        let e = p(src);
        let printed = e.to_string();
        let again = parse(&printed).unwrap_or_else(|err| panic!("{src:?} printed as {printed:?}: {}", err.render(&printed)));
        assert_eq!(again, e, "{src:?} printed as {printed:?} re-parses differently");
        assert_eq!(again.to_string(), printed, "printing isn't a fixed point for {src:?}");
        let json = e.to_json();
        let back = Expr::from_json(&json).unwrap_or_else(|err| panic!("{src:?}: {err} in {json}"));
        assert_eq!(back, e, "JSON round trip of {src:?}");
        assert_eq!(back.to_json(), json);
    }
}

#[test]
fn canonical_forms() {
    let cases = [
        ("Math.max( 1,2 )", "max(1, 2)"),
        ("Math.PI", "3.141592653589793"),
        ("(((a)))", "a"),
        ("a+b*c", "a + b * c"),
        ("1e3", "1000"),
        ("1E-9", "1e-9"),
        ("'x'", "\"x\""),
        ("[1,2,]", "[1, 2]"),
        ("d['x']", "d.x"),
        ("(d) => d.x", "d.x"),
        ("undefined", "null"),
        ("a === b", "a === b"),
        ("Number.isNaN(x)", "isNaN(x)"),
        ("d => { return d.x }", "d.x"),
        ("datum => datum.v + k", "d => d.v + k"),
        ("(d, i) => d.x + i", "(d, i) => d.x + i"),
    ];
    for (src, want) in cases {
        assert_eq!(p(src).to_string(), want, "{src}");
    }
}

#[test]
fn lambda_forms() {
    // All wrappers name the datum; `d.x` is column x.
    for src in ["d => d.x", "(d) => d.x", "(d, i) => d.x", "row => row.x", "(r, j) => r.x"] {
        assert_eq!(p(src), Expr::field("x"), "{src}");
    }
    // Without a wrapper, `d` is the datum and bare names are column-or-signal.
    assert_eq!(p("d.x"), Expr::field("x"));
    assert_eq!(p("x"), Expr::ident("x"));
    // Inside a lambda, free names are signals (JS scoping), even `d` if it isn't the parameter.
    assert_eq!(p("row => x"), Expr::signal("x"));
    assert_eq!(p("row => d.x"), Expr::signal("d.x"));
    assert_eq!(p("() => d.x"), Expr::signal("d.x"));
    // The second parameter is the row index.
    assert_eq!(p("(d, i) => i"), Expr::row());
    assert_eq!(p("(d, idx) => idx * 2"), Expr::binary(BinOp::Mul, Expr::row(), Expr::num(2.0)));
    // `i` without a second parameter is just a name.
    assert_eq!(p("d => i"), Expr::signal("i"));
    assert_eq!(p("i"), Expr::ident("i"));
    // Parenthesized expressions aren't lambdas.
    assert_eq!(p("(a) + 1"), Expr::binary(BinOp::Add, Expr::ident("a"), Expr::num(1.0)));
    // Printing a tree with signals or the row index produces a wrapper, with parameter names that
    // don't collide with free names.
    assert_eq!(Expr::binary(BinOp::Add, Expr::field("x"), Expr::signal("d")).to_string(), "d2 => d2.x + d");
    let e = Expr::binary(BinOp::Add, Expr::row(), Expr::signal("i"));
    assert_eq!(e.to_string(), "(d, i2) => i2 + i");
    assert_eq!(parse(&e.to_string()).unwrap(), e);
}

#[test]
fn lambda_errors() {
    assert!(perr("(d, d) => d.x").message.contains("different names"));
    assert!(perr("true => 1").message.contains("parameter"));
    assert!(perr("d => { d.x }").message.contains("return"));
}

#[test]
fn precedence_matches_js() {
    let same = [
        ("a + b * c", "a + (b * c)"),
        ("a * b + c", "(a * b) + c"),
        ("a - b - c", "(a - b) - c"),
        ("2 ** 3 ** 2", "2 ** (3 ** 2)"),
        ("2 * 3 ** 2", "2 * (3 ** 2)"),
        ("-a * b", "(-a) * b"),
        ("!a && b", "(!a) && b"),
        ("a < b + 1", "a < (b + 1)"),
        ("a == b < c", "a == (b < c)"),
        ("a && b || c && x", "(a && b) || (c && x)"),
        ("a || b ? c : x", "(a || b) ? c : x"),
        ("a ? b : c ? x : e", "a ? b : (c ? x : e)"),
        ("a ? b ? c : x : e", "a ? (b ? c : x) : e"),
        ("a ?? b ?? c", "(a ?? b) ?? c"),
        ("a + b ?? c", "(a + b) ?? c"),
        ("a % b * c", "(a % b) * c"),
        ("-x.y", "-(x.y)"),
        ("-d.x.length", "-(d.x.length)"),
        ("!a == b", "(!a) == b"),
        ("a ? b : c + 1", "a ? b : (c + 1)"),
    ];
    for (a, b) in same {
        assert_eq!(p(a), p(b), "{a} vs {b}");
    }
    assert_eq!(
        p("1 + 2 * 3"),
        Expr::binary(BinOp::Add, Expr::num(1.0), Expr::binary(BinOp::Mul, Expr::num(2.0), Expr::num(3.0)))
    );
}

#[test]
fn js_syntax_rules() {
    // A bare unary before `**` is ambiguous in JS.
    let e = perr("-x ** 2");
    assert_eq!((e.span.start, e.span.end), (0, 5));
    perr("-2 ** 2");
    perr("!a ** 2");
    // `??` can't mix with `||`/`&&` without parentheses.
    for src in ["a ?? b || c", "a || b ?? c", "a && b ?? c", "a ?? b && c"] {
        assert!(perr(src).message.contains("??"), "{src}");
    }
    // Negative literals vs negation.
    assert_eq!(p("-3"), Expr::num(-3.0));
    assert_eq!(p("-(3)"), Expr::unary(UnOp::Neg, Expr::num(3.0)));
    assert_eq!(p("-Infinity"), Expr::num(f64::NEG_INFINITY));
    assert_eq!(p("- 3"), Expr::num(-3.0));
}

#[test]
fn numbers() {
    let cases = [
        ("1e3", 1000.0),
        (".5", 0.5),
        ("5.", 5.0),
        ("1.e2", 100.0),
        ("0x1F", 31.0),
        ("0b101", 5.0),
        ("0o17", 15.0),
        ("1_000", 1000.0),
        ("2.5E-3", 0.0025),
        ("0.1", 0.1),
    ];
    for (src, v) in cases {
        assert_eq!(p(src), Expr::num(v), "{src}");
    }
    perr("1e");
    perr("1x");
    perr("0x");
    perr("1.toString()");
    assert_eq!(p("1..toString()"), Expr::method(Expr::num(1.0), "toString", vec![]));
}

#[test]
fn strings_and_escapes() {
    assert_eq!(p(r#""a\nb""#), Expr::str("a\nb"));
    assert_eq!(p(r#"'\u00e9\u{1F600}\x41'"#), Expr::str("é😀A"));
    assert_eq!(p(r#""\uD83D\uDE00""#), Expr::str("😀"));
    assert_eq!(p(r#""\uD83D""#), Expr::str("\u{FFFD}"));
    assert_eq!(p(r#"'it\'s'"#), Expr::str("it's"));
    assert_eq!(p("\"line\\\ncontinued\""), Expr::str("linecontinued"));
    let e = perr("\"unterminated");
    assert_eq!(e.span.start, 0);
    perr("'a\nb'");
    perr("\"\\u{110000}\"");
}

#[test]
fn template_literals() {
    let e = p("`${d.name}: ${d.value}`");
    let Expr::Concat { parts, .. } = &e else { panic!("{e:?}") };
    assert_eq!(parts, &vec![Expr::field("name"), Expr::str(": "), Expr::field("value")]);
    assert_eq!(p("`plain`"), Expr::Concat { parts: vec![Expr::str("plain")], span: Default::default() });
    assert_eq!(p("``"), Expr::Concat { parts: vec![], span: Default::default() });
    // Substitutions are full expressions, including nested templates and braces in strings.
    let e = p("`${a ? `x${b}` : \"}\"}!`");
    assert_eq!(e.to_string(), "`${a ? `x${b}` : \"}\"}!`");
    // Escapes, and `$` without `{`.
    assert_eq!(p(r"`\`$5 \${x}`"), Expr::Concat { parts: vec![Expr::str("`$5 ${x}")], span: Default::default() });
    // Postfix on a template.
    assert!(matches!(p("`abc`.length"), Expr::Method { .. }));
    perr("`unterminated");
    perr("`${a`");
    perr("`${a b}`");
}

#[test]
fn host_calls_with_dotted_names() {
    assert_eq!(p("scale.y(d.value)"), Expr::call("scale.y", vec![Expr::field("value")]));
    assert_eq!(p("selected.has(d.party)"), Expr::call("selected.has", vec![Expr::field("party")]));
    assert_eq!(p("selected.isEmpty()"), Expr::call("selected.isEmpty", vec![]));
    assert_eq!(p("a.b.c(1)"), Expr::call("a.b.c", vec![Expr::num(1.0)]));
    assert_eq!(p("format(d.v, '.1f')"), Expr::call("format", vec![Expr::field("v"), Expr::str(".1f")]));
    // Built-ins resolve by name, `Math.` or not.
    assert_eq!(p("Math.abs(x)"), Expr::call("abs", vec![Expr::ident("x")]));
    assert_eq!(p("abs(x)"), Expr::call("abs", vec![Expr::ident("x")]));
    // Methods on a call result.
    assert_eq!(
        p("format(d.v).toUpperCase()"),
        Expr::method(Expr::call("format", vec![Expr::field("v")]), "toUpperCase", vec![])
    );
    // Host calls inside lambdas are still host calls.
    assert_eq!(p("d => scale.y(d.value)"), Expr::call("scale.y", vec![Expr::field("value")]));
    // Dotted non-calls are dotted names.
    assert_eq!(p("palette.accent"), Expr::ident("palette.accent"));
    assert_eq!(p("d => palette.accent"), Expr::signal("palette.accent"));
}

#[test]
fn string_methods_parse() {
    let e = p("d.name.slice(0, 3).toUpperCase()");
    assert_eq!(
        e,
        Expr::method(Expr::method(Expr::field("name"), "slice", vec![Expr::num(0.0), Expr::num(3.0)]), "toUpperCase", vec![])
    );
    assert_eq!(p("d.name.length"), Expr::method(Expr::field("name"), "length", vec![]));
    // On a parenthesized name the method applies to the value (not a host call).
    assert_eq!(p("(name).trim()"), Expr::method(Expr::ident("name"), "trim", vec![]));
    // Errors: unknown methods, wrong arity, `length()` and uncalled methods.
    assert!(perr("d.name.reverse()").message.contains("unknown"));
    assert!(perr("d.name.slice()").message.contains("takes"));
    assert!(perr("d.name.length()").message.contains("property"));
    assert!(perr("d.name.trim").message.contains("call it"));
    assert!(perr("d.a.b").message.contains("unknown"));
}

#[test]
fn syntax_errors_have_spans() {
    let cases: &[(&str, usize, &str)] = &[
        ("a +", 3, "end of input"),
        ("a = 1", 2, "=="),
        ("a & b", 2, "&&"),
        ("a | b", 2, "||"),
        ("x++", 1, "pure"),
        ("a ?. b", 2, "?."),
        ("new Foo()", 0, "new"),
        ("typeof x", 0, "typeof"),
        ("Math.random()", 0, "deterministic"),
        ("Math.foo(1)", 0, "Math.foo"),
        ("d", 0, "datum"),
        ("d.x(1)", 0, "not a function"),
        ("d[k]", 2, "string literal"),
        ("(a)(b)", 3, "named functions"),
        ("round(1, 2, 3)", 0, "takes 1 to 2 arguments, got 3"),
        ("clamp(1)", 0, "takes 3 arguments"),
        ("[1,,2]", 3, "missing"),
        ("f(...x)", 2, "spread"),
        ("a b", 2, "unexpected `b`"),
        ("@", 0, "unexpected character"),
        ("/* open", 0, "comment"),
        ("1 +\n  #", 6, "unexpected character"),
    ];
    for (src, at, needle) in cases {
        let e = perr(src);
        assert_eq!(e.span.start, *at, "{src}: {e}");
        assert!(e.message.contains(needle), "{src}: {:?} lacks {needle:?}", e.message);
    }
    // `render` points at the error.
    let e = perr("d.value * * 2");
    assert_eq!(e.render("d.value * * 2"), "syntax error: unexpected `*`, expected an expression\n  d.value * * 2\n            ^");
}

#[test]
fn comments_and_whitespace() {
    assert_eq!(p("d.x /* the value */ * 2 // doubled\n"), p("d.x * 2"));
    assert_eq!(p("\n\td =>\n  d.x\n"), p("d.x"));
}

#[test]
fn node_spans_point_into_source() {
    let src = "d.value * 2 + scale.y(d.v)";
    let e = p(src);
    let Expr::Binary { lhs, rhs, span, .. } = &e else { panic!() };
    assert_eq!((span.start, span.end), (0, src.len()));
    assert_eq!(&src[lhs.span().start..lhs.span().end], "d.value * 2");
    assert_eq!(&src[rhs.span().start..rhs.span().end], "scale.y(d.v)");
    // Spans don't affect equality and aren't serialized.
    assert_eq!(p("d.value*2+scale.y(d.v)"), e);
    assert!(!e.to_json().contains("span"));
}

#[test]
fn deep_nesting_is_an_error_not_a_crash() {
    let src = format!("{}1{}", "(".repeat(10_000), ")".repeat(10_000));
    assert!(perr(&src).message.contains("deeply"));
    let src = format!("{}1", "-".repeat(10_001).replace("--", "- -"));
    assert!(parse(&src).is_err());
}

#[test]
fn json_ast_format() {
    let e = p("d.x > 1 ? \"hi\" : null");
    assert_eq!(
        e.to_json(),
        r#"{"kind":"cond","test":{"kind":"binary","op":">","lhs":{"kind":"field","name":"x"},"rhs":{"kind":"num","value":1.0}},"then":{"kind":"str","value":"hi"},"else":{"kind":"null"}}"#
    );
    // Non-finite numbers serialize as strings; integers deserialize; spans are accepted on input.
    assert_eq!(p("NaN").to_json(), r#"{"kind":"num","value":"NaN"}"#);
    let back = Expr::from_json(r#"{"kind":"num","value":-Infinity}"#);
    assert!(back.is_err());
    let back = Expr::from_json(r#"{"kind":"num","value":"-Infinity","span":{"start":3,"end":7}}"#).unwrap();
    assert_eq!(back, Expr::num(f64::NEG_INFINITY));
    assert_eq!(back.span().start, 3);
    assert_eq!(Expr::from_json(r#"{"kind":"num","value":2}"#).unwrap(), Expr::num(2.0));
    assert!(Expr::from_json(r#"{"kind":"nope"}"#).is_err());
}
