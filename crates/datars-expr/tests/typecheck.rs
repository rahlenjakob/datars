//! Typechecking against a schema: result types, and errors with spans and helpful messages.

use datars_expr::{parse, typecheck, EmptyEnv, Expr, MapEnv, Type, TypeEnv, Value};

fn schema() -> MapEnv {
    MapEnv::new()
        .num("value", vec![])
        .num("share", vec![])
        .strs("name", &[])
        .strs("party", &[])
        .bools("flag", vec![])
        .signal("k", 1.0)
        .signal("label", "x")
        .signal("on", true)
        .signal("palette.accent", "#f00")
        .function("scale.y", |_| None)
        .function("selected.has", |_| None)
        .function("format", |_| None)
}

fn ty(src: &str) -> Type {
    let e = parse(src).unwrap_or_else(|e| panic!("{}", e.render(src)));
    typecheck(&e, &schema()).unwrap_or_else(|err| panic!("{}", err.render(src)))
}

/// The error for `src`, whose span must cover `at` (a substring of `src`).
#[track_caller]
fn fails(src: &str, at: &str, needle: &str) {
    let e = parse(src).unwrap_or_else(|e| panic!("{}", e.render(src)));
    let err = match typecheck(&e, &schema()) {
        Ok(t) => panic!("{src} typechecked as {t}"),
        Err(err) => err,
    };
    assert_eq!(&src[err.span.start..err.span.end], at, "{src}: span of {err}");
    assert!(err.message.contains(needle), "{src}: {:?} lacks {needle:?}", err.message);
}

#[test]
fn result_types() {
    let cases = [
        ("d.value * 2 + 1", Type::Num),
        ("d.name", Type::Str),
        ("d.flag", Type::Bool),
        ("d.name + 1", Type::Str),
        ("`${d.value}`", Type::Str),
        ("d.value > 3", Type::Bool),
        ("d.name < \"m\"", Type::Bool),
        ("d.value == null", Type::Bool),
        ("d.share > 30 ? palette.accent : \"#ccc\"", Type::Str),
        ("d.flag ? 1 : \"x\"", Type::Any),
        ("d.flag ? 1 : null", Type::Any),
        ("d.name || \"none\"", Type::Str),
        ("d.value ?? 0", Type::Num),
        ("!d.name", Type::Bool),
        ("-d.value", Type::Num),
        ("max(d.value, 0)", Type::Num),
        ("round(d.value, 1)", Type::Num),
        ("String(d.value)", Type::Str),
        ("Number(d.name)", Type::Num),
        ("isNaN(d.value)", Type::Bool),
        ("d.name.toUpperCase()", Type::Str),
        ("d.name.length", Type::Num),
        ("d.name.startsWith(\"A\")", Type::Bool),
        ("d.name.indexOf(\"a\")", Type::Num),
        ("(d.value).toString()", Type::Str),
        ("[\"S\", \"M\"].includes(d.party)", Type::Bool),
        ("[1, 2].indexOf(d.value)", Type::Num),
        ("[1, 2][0]", Type::Num),
        ("[1, \"a\"][0]", Type::Any),
        ("d.name[0]", Type::Str),
        ("scale.y(d.value)", Type::Any),
        ("value * k", Type::Num),
        ("label + \"!\"", Type::Str),
        ("d => d.value * k", Type::Num),
        ("(d, i) => i", Type::Num),
        ("null", Type::Any),
    ];
    for (src, want) in cases {
        assert_eq!(ty(src), want, "{src}");
    }
}

#[test]
fn errors_carry_spans_and_messages() {
    fails("d.valeu * 2", "d.valeu", "unknown column `valeu`; did you mean `value`?");
    fails("d.name * 2", "d.name", "needs a number, but `d.name` is a string (convert with Number(…))");
    fails("1 + d.flag", "1 + d.flag", "can't add a number and a boolean");
    fails("d.value > \"3\"", "d.value > \"3\"", "compares a number with a string");
    fails("d.value == \"3\"", "d.value == \"3\"", "always false");
    fails("d.value !== \"3\"", "d.value !== \"3\"", "always true");
    fails("d.flag < true", "d.flag < true", "ordering needs two numbers or two strings");
    fails("-d.name", "d.name", "unary `-` needs a number");
    fails("abs(d.name)", "d.name", "argument 1 of abs()");
    fails("d.value.toUpperCase()", "d.value", "`.toUpperCase` needs a string");
    fails("d.name.slice(\"a\")", "\"a\"", ".slice() needs a number");
    fails("d.name.startsWith(1)", "1", ".startsWith() needs a string");
    fails("scale.z(d.value)", "scale.z(d.value)", "unknown function `scale.z`");
    fails("kk + 1", "kk", "unknown name `kk` (not a column or a signal); did you mean `k`?");
    fails("d => kk", "kk", "unknown signal `kk`; did you mean `k`?");
    fails("d.flag[0]", "d.flag[0]", "can't index a boolean");
    fails("d.value > 1 ? d.nme : 0", "d.nme", "did you mean `name`?");
    fails("`${d.bogus}`", "d.bogus", "unknown column");
    fails("d.value / d.name", "d.name", "`/` needs a number");
}

#[test]
fn hand_built_trees() {
    // Trees that didn't come from the parser: errors still say what's wrong (span unknown).
    let e = Expr::call("round", vec![]);
    let err = typecheck(&e, &schema()).unwrap_err();
    assert!(err.span.is_unknown());
    assert!(err.message.contains("round() takes 1 to 2 arguments, got 0"), "{}", err.message);
    let e = Expr::Array { items: vec![], span: Default::default() };
    assert!(typecheck(&e, &schema()).unwrap_err().message.contains("array literal"));
    let e = Expr::method(Expr::field("name"), "reverse", vec![]);
    assert!(typecheck(&e, &schema()).unwrap_err().message.contains("unknown method"));
    // `Display` of the error is useful on its own.
    let err = typecheck(&parse("d.x").unwrap(), &EmptyEnv).unwrap_err();
    assert_eq!(err.to_string(), "type error: unknown column `x` (at 0..3)");
}

/// A host's own `TypeEnv`: signatures for host functions.
struct Host;

impl TypeEnv for Host {
    fn column_type(&self, name: &str) -> Option<Type> {
        (name == "v").then_some(Type::Num)
    }
    fn signal_type(&self, _: &str) -> Option<Type> {
        None
    }
    fn call_type(&self, name: &str, args: &[Type]) -> Option<Type> {
        match (name, args) {
            ("scale.y", [Type::Num | Type::Any]) => Some(Type::Num),
            ("format", [_, Type::Str]) => Some(Type::Str),
            _ => None,
        }
    }
}

#[test]
fn host_signatures() {
    assert_eq!(typecheck(&parse("scale.y(d.v) + 1").unwrap(), &Host), Ok(Type::Num));
    assert_eq!(typecheck(&parse("format(d.v, '.1f')").unwrap(), &Host), Ok(Type::Str));
    let err = typecheck(&parse("format(d.v, 2)").unwrap(), &Host).unwrap_err();
    assert!(err.message.contains("unknown function `format`"));
    // Evaluation stays total even for programs that don't typecheck.
    let c = datars_expr::compile(&parse("d.name * 2").unwrap()).unwrap();
    assert!(c.eval_scalar(&schema()).identical(&Value::Num(f64::NAN)));
}
