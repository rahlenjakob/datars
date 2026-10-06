//! Evaluation semantics: values, operators, totality, built-ins, string methods, host calls,
//! name resolution. Every expression is checked through both the vector VM and the scalar
//! interpreter.

use datars_expr::{compile, parse, EmptyEnv, Env, MapEnv, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

fn env() -> MapEnv {
    MapEnv::new()
        .num("value", vec![1.0, 2.5, f64::NAN, -4.0])
        .num("a", vec![10.0, 40.0, 30.0, 50.0])
        .num("b", vec![1.0, 2.0, 3.0, 4.0])
        .num("c", vec![-1.0, -2.0, -3.0, -4.0])
        .strs("name", &[Some("Alice"), Some("bob"), None, Some("  Eve ")])
        .strs("party", &[Some("S"), Some("M"), Some("C"), Some("S")])
        .bools("flag", vec![true, false, true, false])
        .signal("k", 3.0)
        .signal("label", "hi")
        .signal("on", true)
        .signal("nothing", Value::Null)
        .signal("value", 99.0)
        .function("scale.y", |args| Some(Value::Num(100.0 - args.first()?.as_num()? * 10.0)))
        .function("selected.has", |args| Some(Value::Bool(matches!(args.first()?.as_str(), Some("S")))))
        .function("selected.isEmpty", |_| Some(Value::Bool(false)))
        .function("format", |args| Some(Value::from(format!("<{}>", args.first()?))))
        .function("never", |_| None)
}

/// Evaluates over all rows with the vector VM, checking every row against the scalar interpreter.
fn rows(src: &str, env: &dyn Env, n: usize) -> Vec<Value> {
    let e = parse(src).unwrap_or_else(|err| panic!("{}", err.render(src)));
    let c = compile(&e).unwrap_or_else(|err| panic!("{src}: {err}"));
    let v = c.eval_rows(n, env);
    assert_eq!(v.len(), n);
    for (i, x) in v.iter().enumerate() {
        let s = c.eval_row(i, env);
        assert!(x.identical(&s), "{src} row {i}: vector {x:?} vs scalar {s:?}");
    }
    v
}

fn scalar(src: &str) -> Value {
    let r = rows(src, &env(), 1);
    let c = compile(&parse(src).unwrap()).unwrap();
    assert!(c.eval_scalar(&env()).identical(&r[0]));
    r[0].clone()
}

fn num(x: f64) -> Value {
    Value::Num(x)
}
fn s(x: &str) -> Value {
    Value::str(x)
}
fn b(x: bool) -> Value {
    Value::Bool(x)
}

#[track_caller]
fn same(got: Vec<Value>, want: Vec<Value>) {
    assert_eq!(got.len(), want.len());
    for (g, w) in got.iter().zip(&want) {
        assert!(g.identical(w), "got {got:?}, want {want:?}");
    }
}

#[track_caller]
fn check(src: &str, want: Value) {
    let got = scalar(src);
    assert!(got.identical(&want), "{src}: got {got:?}, want {want:?}");
}

#[test]
fn arithmetic() {
    check("1 + 2 * 3", num(7.0));
    check("(1 + 2) * 3", num(9.0));
    check("7 % 3", num(1.0));
    check("-7 % 3", num(-1.0));
    check("7.5 % 2", num(1.5));
    check("2 ** 10", num(1024.0));
    check("2 ** 3 ** 2", num(512.0));
    check("(-8) ** (1 / 3)", num(f64::NAN));
    check("-(3)", num(-3.0));
    check("+true", num(1.0));
    check("0.1 + 0.2", num(0.30000000000000004));
    check("10 / 4", num(2.5));
}

#[test]
fn division_by_zero_is_ieee() {
    check("1 / 0", num(f64::INFINITY));
    check("-1 / 0", num(f64::NEG_INFINITY));
    check("0 / 0", num(f64::NAN));
    check("5 % 0", num(f64::NAN));
    check("log(0)", num(f64::NEG_INFINITY));
    check("sqrt(-1)", num(f64::NAN));
}

#[test]
fn null_and_missing_are_total() {
    check("null + 1", num(f64::NAN));
    check("null * 2", num(f64::NAN));
    check("-null", num(f64::NAN));
    check("d.missing", Value::Null);
    check("d.missing * 2", num(f64::NAN));
    check("d.missing == null", b(true));
    check("nosuchsignal", Value::Null);
    check("nosuchsignal ?? 5", num(5.0));
    check("never(1)", Value::Null);
    check("unknown.host(1, 2)", Value::Null);
    check("\"a\" - 1", num(f64::NAN));
    check("\"3\" * 2", num(f64::NAN));
    check("null < 1", b(false));
    check("null >= null", b(false));
    check("abs(null)", num(f64::NAN));
    check("d.missing.toUpperCase()", Value::Null);
    check("[1, 2][5]", Value::Null);
    check("\"abc\"[-1]", Value::Null);
    check("\"abc\"[0.5]", Value::Null);
    // With zero rows nothing is evaluated.
    assert!(compile(&parse("d.value / 0").unwrap()).unwrap().eval_rows(0, &env()).is_empty());
}

#[test]
fn string_concatenation() {
    check("\"a\" + 1", s("a1"));
    check("1 + \"a\"", s("1a"));
    check("1 + 2 + \"a\"", s("3a"));
    check("\"a\" + 1 + 2", s("a12"));
    check("\"x\" + null", s("xnull"));
    check("\"x\" + true", s("xtrue"));
    check("\"x\" + 0.1", s("x0.1"));
    check("\"x\" + 1e21", s("x1e+21"));
    check("\"x\" + -0", s("x0"));
    check("\"x\" + 1 / 0", s("xInfinity"));
    check("`${1}${2}`", s("12"));
    check("`v=${d.value}`", s("v=1"));
    check("`${null}|${true}|${\"s\"}`", s("null|true|s"));
}

#[test]
fn comparisons() {
    check("1 < 2", b(true));
    check("2 <= 2", b(true));
    check("\"a\" < \"b\"", b(true));
    check("\"B\" < \"a\"", b(true));
    check("\"10\" < \"9\"", b(true));
    check("1 < \"2\"", b(false));
    check("\"2\" > 1", b(false));
    check("true > false", b(true));
    check("NaN < 1", b(false));
    check("NaN >= NaN", b(false));
}

#[test]
fn equality_is_strict_ish() {
    check("1 == 1", b(true));
    check("1 === 1", b(true));
    check("1 == \"1\"", b(false));
    check("1 != \"1\"", b(true));
    check("0 == false", b(false));
    check("\"\" == false", b(false));
    check("null == null", b(true));
    check("null == undefined", b(true));
    check("null == 0", b(false));
    check("null == \"\"", b(false));
    check("NaN == null", b(true));
    check("NaN == NaN", b(true));
    check("0 == -0", b(true));
    check("\"a\" === \"a\"", b(true));
    check("true !== false", b(true));
}

#[test]
fn truthiness_and_logic() {
    for (src, want) in [
        ("!0", true),
        ("!NaN", true),
        ("!\"\"", true),
        ("!null", true),
        ("!\"0\"", false),
        ("!\"false\"", false),
        ("!-1", false),
    ] {
        check(src, b(want));
    }
    // `&&`, `||`, `??` return an operand, like JS.
    check("0 || \"x\"", s("x"));
    check("2 || \"x\"", num(2.0));
    check("0 && \"x\"", num(0.0));
    check("2 && \"x\"", s("x"));
    check("null ?? 1", num(1.0));
    check("0 ?? 1", num(0.0));
    check("\"\" ?? 1", s(""));
    check("NaN ?? 1", num(1.0));
    check("false ?? 1", b(false));
    check("true ? 1 : 2", num(1.0));
    check("\"\" ? 1 : 2", num(2.0));
}

#[test]
fn builtins() {
    check("abs(-2)", num(2.0));
    check("min(3, 1, 2)", num(1.0));
    check("max(3, 1, 2)", num(3.0));
    check("min()", num(f64::INFINITY));
    check("max()", num(f64::NEG_INFINITY));
    check("max(1, NaN)", num(f64::NAN));
    check("max(1, null)", num(f64::NAN));
    check("round(2.5)", num(3.0));
    check("round(-2.5)", num(-2.0));
    check("round(1.2345, 2)", num(1.23));
    check("round(1234.5, -2)", num(1200.0));
    check("floor(-1.5)", num(-2.0));
    check("ceil(1.2)", num(2.0));
    check("trunc(-1.7)", num(-1.0));
    check("sqrt(16)", num(4.0));
    check("cbrt(27)", num(3.0));
    check("pow(2, 8)", num(256.0));
    check("exp(0)", num(1.0));
    check("log(E)", num(1.0));
    check("log10(1000)", num(3.0));
    check("log2(8)", num(3.0));
    check("sin(0)", num(0.0));
    check("cos(0)", num(1.0));
    check("atan2(1, 1) * 4", num(std::f64::consts::PI));
    check("clamp(5, 0, 1)", num(1.0));
    check("clamp(-5, 0, 1)", num(0.0));
    check("clamp(NaN, 0, 1)", num(f64::NAN));
    check("lerp(0, 10, 0.25)", num(2.5));
    check("sign(-3)", num(-1.0));
    check("sign(0)", num(0.0));
    check("hypot(3, 4)", num(5.0));
    check("hypot()", num(0.0));
    check("isNaN(NaN)", b(true));
    check("isNaN(\"abc\")", b(true));
    check("isNaN(\"12\")", b(false));
    check("isFinite(1 / 0)", b(false));
    check("isFinite(1)", b(true));
    check("String(1.5)", s("1.5"));
    check("String(null)", s("null"));
    check("String(1e-7)", s("1e-7"));
    check("Number(\" 42 \")", num(42.0));
    check("Number(\"4x\")", num(f64::NAN));
    check("Number(\"\")", num(f64::NAN));
    check("Number(true)", num(1.0));
    check("Boolean(\"\")", b(false));
    check("PI", num(std::f64::consts::PI));
    check("Math.PI * 2", num(std::f64::consts::TAU));
    check("Math.max(1, 5)", num(5.0));
    // Deterministic transcendental math: pinned bits.
    assert_eq!(scalar("sin(1)").as_num().map(f64::to_bits), Some(datars_math::m::sin(1.0).to_bits()));
    assert_eq!(scalar("2 ** 0.5").as_num().map(f64::to_bits), Some(datars_math::m::pow(2.0, 0.5).to_bits()));
}

#[test]
fn seeded_randomness_is_a_pure_function() {
    // Same key and stream → same number (pinned: every target must agree); streams are independent.
    check("rand(0) == rand(0, 0)", b(true));
    check("rand(-0) == rand(0)", b(true));
    check("rand(7, 1) == rand(7, 2)", b(false));
    check("rand(NaN)", num(f64::NAN));
    check("randn(1, NaN)", num(f64::NAN));
    let pinned = scalar("rand(42)").as_num().unwrap();
    assert_eq!(pinned.to_bits(), scalar("rand(42, 0)").as_num().unwrap().to_bits());
    // Per row: the vector VM agrees with the scalar interpreter (checked by `rows`), and a few
    // thousand draws look uniform / normal.
    let n = 4000;
    let env = MapEnv::new().num("i", (0..n).map(|i| i as f64).collect());
    let u: Vec<f64> = rows("rand(d.i, 3)", &env, n).iter().map(|v| v.as_num().unwrap()).collect();
    assert!(u.iter().all(|x| (0.0..1.0).contains(x)));
    let mean = u.iter().sum::<f64>() / n as f64;
    assert!((mean - 0.5).abs() < 0.02, "{mean}");
    let deciles = (0..10).map(|k| u.iter().filter(|&&x| (x * 10.0).floor() as usize == k).count()).collect::<Vec<_>>();
    assert!(deciles.iter().all(|&c| (320..480).contains(&c)), "{deciles:?}");
    let g: Vec<f64> = rows("randn(d.i, 5)", &env, n).iter().map(|v| v.as_num().unwrap()).collect();
    let (m, var) = (g.iter().sum::<f64>() / n as f64, g.iter().map(|x| x * x).sum::<f64>() / n as f64);
    assert!(m.abs() < 0.06 && (var - 1.0).abs() < 0.08, "{m} {var}");
    assert!(g.iter().all(|x| x.is_finite()));
}

#[test]
fn string_methods() {
    let e = env();
    assert_eq!(rows("d.name.toUpperCase()", &e, 4), vec![s("ALICE"), s("BOB"), Value::Null, s("  EVE ")]);
    assert_eq!(rows("d.name.toLowerCase()", &e, 4)[0], s("alice"));
    assert_eq!(rows("d.name.trim()", &e, 4)[3], s("Eve"));
    assert_eq!(rows("d.name.length", &e, 4), vec![num(5.0), num(3.0), Value::Null, num(6.0)]);
    assert_eq!(rows("d.name.slice(0, 3)", &e, 4)[0], s("Ali"));
    assert_eq!(rows("d.name.slice(-2)", &e, 4)[1], s("ob"));
    assert_eq!(rows("d.name.startsWith(\"A\")", &e, 4)[..2], [b(true), b(false)]);
    assert_eq!(rows("d.name.endsWith(\"b\")", &e, 4)[..2], [b(false), b(true)]);
    assert_eq!(rows("d.name.includes(\"li\")", &e, 4)[0], b(true));
    assert_eq!(rows("d.name.indexOf(\"i\")", &e, 4)[..2], [num(2.0), num(-1.0)]);
    check("\"wörld\".indexOf(\"l\")", num(3.0));
    check("\"abc\".slice(1, -1)", s("b"));
    check("\"abc\".slice(5)", s(""));
    check("\"abc\"[1]", s("b"));
    check("(12.5).toString()", s("12.5"));
    check("(12.5).toString().length", num(4.0));
    check("label.length", Value::Null); // a dotted name, not a method: no such signal
    check("(label).length", num(2.0));
    check("`${label}!`.toUpperCase()", s("HI!"));
}

#[test]
fn array_literals() {
    let e = env();
    assert_eq!(rows("[\"S\", \"C\"].includes(d.party)", &e, 4), vec![b(true), b(false), b(true), b(true)]);
    assert_eq!(rows("[\"S\", \"M\", \"C\"].indexOf(d.party)", &e, 4), vec![num(0.0), num(1.0), num(2.0), num(0.0)]);
    assert_eq!(rows("[d.a, d.b].includes(2)", &e, 4), vec![b(false), b(true), b(false), b(false)]);
    assert_eq!(rows("[\"x\", \"y\", \"z\"][d.b - 1]", &e, 4), vec![s("x"), s("y"), s("z"), Value::Null]);
    check("[1, 2, 3].length", num(3.0));
    check("[null].includes(NaN)", b(true));
    check("[1, 2].indexOf(\"1\")", num(-1.0));
}

#[test]
fn columns_and_rows() {
    let e = env();
    same(rows("d.value * 2 + 1", &e, 4), vec![num(3.0), num(6.0), num(f64::NAN), num(-7.0)]);
    assert_eq!(rows("d.a > 30 ? d.b : d.c", &e, 4), vec![num(-1.0), num(2.0), num(-3.0), num(4.0)]);
    assert_eq!(rows("(d, i) => i * 10 + d.b", &e, 4), vec![num(1.0), num(12.0), num(23.0), num(34.0)]);
    assert_eq!(rows("d.flag ? \"yes\" : \"no\"", &e, 4)[..2], [s("yes"), s("no")]);
    assert_eq!(rows("!d.flag", &e, 4)[..2], [b(false), b(true)]);
    // Rows past the end of a column read as null (per the column's null: NaN for numbers).
    let short = MapEnv::new().num("x", vec![1.0]).strs("s", &[Some("a")]).bools("f", vec![true]);
    same(rows("d.x", &short, 3), vec![num(1.0), num(f64::NAN), num(f64::NAN)]);
    assert_eq!(rows("d.s", &short, 3), vec![s("a"), Value::Null, Value::Null]);
    assert_eq!(rows("d.f", &short, 3), vec![b(true), Value::Null, Value::Null]);
    // Longer columns are cut to n.
    assert_eq!(rows("d.b", &e, 2), vec![num(1.0), num(2.0)]);
}

#[test]
fn name_resolution() {
    let e = env();
    // Bare names: column first, then signal.
    assert_eq!(rows("value", &e, 2), vec![num(1.0), num(2.5)]);
    assert_eq!(rows("k * 2", &e, 2), vec![num(6.0), num(6.0)]);
    // Inside a lambda, free names are signals even if a column has that name.
    assert_eq!(rows("d => value", &e, 2), vec![num(99.0), num(99.0)]);
    assert_eq!(rows("d => d.value", &e, 2), vec![num(1.0), num(2.5)]);
    // `d.x` never reads a signal.
    assert_eq!(rows("d.k", &e, 1), vec![Value::Null]);
}

#[test]
fn host_calls() {
    let e = env();
    assert_eq!(rows("scale.y(d.b)", &e, 3), vec![num(90.0), num(80.0), num(70.0)]);
    assert_eq!(rows("selected.has(d.party) ? 1 : 0.3", &e, 4), vec![num(1.0), num(0.3), num(0.3), num(1.0)]);
    assert_eq!(rows("selected.isEmpty() || selected.has(d.party) ? 1 : 0.3", &e, 2), vec![num(1.0), num(0.3)]);
    assert_eq!(rows("format(d.b)", &e, 2), vec![s("<1>"), s("<2>")]);
    assert_eq!(rows("scale.y(\"x\")", &e, 2), vec![Value::Null, Value::Null]);
}

/// Host calls with arguments that are the same on every row are made once per evaluation.
#[test]
fn constant_host_calls_are_hoisted() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c2 = calls.clone();
    let e = MapEnv::new().num("x", vec![1.0; 5000]).signal("k", 2.0).function("f", move |a| {
        c2.fetch_add(1, Ordering::Relaxed);
        Some(Value::Num(a.first().map_or(0.0, Value::to_num) + 1.0))
    });
    let c = compile(&parse("d.x + f(k)").unwrap()).unwrap();
    let out = c.eval_rows(5000, &e);
    assert_eq!(out[4999], num(4.0));
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    calls.store(0, Ordering::Relaxed);
    let c = compile(&parse("f(d.x)").unwrap()).unwrap();
    c.eval_rows(5000, &e);
    assert_eq!(calls.load(Ordering::Relaxed), 5000);
}

/// A runtime-constant condition evaluates only the live branch.
#[test]
fn constant_conditions_skip_dead_branches() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c2 = calls.clone();
    let e = MapEnv::new().num("x", vec![1.0; 3000]).signal("on", false).function("f", move |_| {
        c2.fetch_add(1, Ordering::Relaxed);
        Some(Value::Num(1.0))
    });
    for src in ["on ? f(d.x) : 0", "on && f(d.x)", "!on || f(d.x)", "(on ? 1 : null) ?? 2"] {
        let c = compile(&parse(src).unwrap()).unwrap();
        let v = c.eval_rows(3000, &e);
        assert_eq!(v.len(), 3000, "{src}");
    }
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    let v = compile(&parse("on ? f(d.x) : d.x * 2").unwrap()).unwrap().eval_rows(3, &e);
    assert_eq!(v, vec![num(2.0), num(2.0), num(2.0)]);
}

#[test]
fn eval_rows_num_fast_path() {
    let e = env();
    let c = compile(&parse("d.value * 2 + 1").unwrap()).unwrap();
    let v = c.eval_rows_num(4, &e);
    assert_eq!(v[0], 3.0);
    assert!(v[2].is_nan());
    let c = compile(&parse("d.flag").unwrap()).unwrap();
    assert_eq!(c.eval_rows_num(2, &e), vec![1.0, 0.0]);
    let c = compile(&parse("d.name").unwrap()).unwrap();
    assert!(c.eval_rows_num(2, &e).iter().all(|x| x.is_nan()));
    let c = compile(&parse("k").unwrap()).unwrap();
    assert_eq!(c.eval_rows_num(3, &e), vec![3.0; 3]);
}

#[test]
fn eval_scalar_for_per_frame_expressions() {
    let c = compile(&parse("k * 2 + (on ? 1 : 0)").unwrap()).unwrap();
    assert_eq!(c.eval_scalar(&env()), num(7.0));
    let c = compile(&parse("1 + 2").unwrap()).unwrap();
    assert_eq!(c.eval_scalar(&EmptyEnv), num(3.0));
    // Columns in a scalar context read row 0 (null if empty).
    let c = compile(&parse("d.value").unwrap()).unwrap();
    assert_eq!(c.eval_scalar(&env()), num(1.0));
    assert!(c.eval_scalar(&MapEnv::new().num("value", vec![])).identical(&num(f64::NAN)));
}

#[test]
fn nan_output_is_canonical() {
    // Hardware produces different NaN bit patterns; outputs are canonicalized.
    let e = MapEnv::new().num("x", vec![f64::from_bits(0xfff8_0000_0000_0001), 0.0]);
    let c = compile(&parse("d.x + 0").unwrap()).unwrap();
    for v in c.eval_rows_num(2, &e) {
        if v.is_nan() {
            assert_eq!(v.to_bits(), f64::NAN.to_bits());
        }
    }
    let c = compile(&parse("d.x / d.x").unwrap()).unwrap();
    for v in c.eval_rows(2, &e) {
        assert_eq!(v.as_num().map(f64::to_bits), Some(f64::NAN.to_bits()));
    }
}
