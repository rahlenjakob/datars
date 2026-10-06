//! Compiler: constant folding, dead-branch elimination, dependency tracking, compile errors for
//! hand-built trees, disassembly, and WGSL output.

use datars_expr::{compile, parse, to_wgsl, to_wgsl_with, Compiled, Expr, MapEnv, Value};

fn c(src: &str) -> Compiled {
    compile(&parse(src).unwrap_or_else(|e| panic!("{}", e.render(src)))).unwrap()
}

#[test]
fn constant_folding() {
    let cases = [
        ("1 + 2 * 3", Value::Num(7.0)),
        ("\"a\" + 1", Value::str("a1")),
        ("max(1, 2, 3) - min(4, 5)", Value::Num(-1.0)),
        ("round(1.23456, 3)", Value::Num(1.235)),
        ("`x=${1 + 1}`", Value::str("x=2")),
        ("\"abc\".toUpperCase().length", Value::Num(3.0)),
        ("[1, 2, 3].includes(2)", Value::Bool(true)),
        ("[\"a\", \"b\"][1]", Value::str("b")),
        ("[1, 2].length", Value::Num(2.0)),
        ("true ? 1 : 2", Value::Num(1.0)),
        ("null ?? \"dflt\"", Value::str("dflt")),
        ("0 || 5", Value::Num(5.0)),
        ("1 && 0", Value::Num(0.0)),
        ("!\"\"", Value::Bool(true)),
        ("String(1 / 3)", Value::str("0.3333333333333333")),
        ("isNaN(0 / 0)", Value::Bool(true)),
    ];
    for (src, want) in cases {
        let compiled = c(src);
        assert!(compiled.is_empty(), "{src} left instructions:\n{compiled}");
        assert_eq!(compiled.constant(), Some(&want), "{src}");
        assert!(compiled.fields().is_empty() && compiled.signals().is_empty());
    }
}

#[test]
fn partial_folding() {
    // Constant subtrees fold; the rest compiles to one instruction per remaining operation.
    assert_eq!(c("d.x * (2 + 3)").len(), 1);
    assert_eq!(c("d.x * 2 + 1").len(), 2);
    assert_eq!(c("d.x * (60 * 60 * 24)").len(), 1);
    assert_eq!(c("`${\"a\"}${\"b\"}${d.x}${\"c\"}${1 + 1}`").len(), 1);
    // A constant test keeps only the live branch (no jumps, no select).
    let k = c("1 > 2 ? d.a : d.b * 2");
    assert_eq!(k.len(), 1);
    assert_eq!(k.fields(), vec!["b"]);
    assert_eq!(c("false && d.a").constant(), Some(&Value::Bool(false)));
    assert_eq!(c("true && d.a").fields(), vec!["a"]);
    assert_eq!(c("\"x\" ?? d.a").constant(), Some(&Value::str("x")));
    // A variable test: two jumps and a select around the branches.
    let v = c("d.a > 30 ? d.b : d.c");
    assert_eq!(v.len(), 4, "{v}");
    assert!(v.constant().is_none());
}

#[test]
fn dependency_tracking() {
    let e = c("d.share > 30 ? palette.accent : `${d.name} ${label}`");
    assert_eq!(e.fields(), vec!["label", "name", "palette.accent", "share"]);
    assert_eq!(e.signals(), vec!["label", "palette.accent"]);
    assert!(e.calls().is_empty());
    // In a lambda, free names are signals only.
    let e = c("d => d.share > 30 ? palette.accent : `${d.name} ${label}`");
    assert_eq!(e.fields(), vec!["name", "share"]);
    assert_eq!(e.signals(), vec!["label", "palette.accent"]);
    // Host calls, deduplicated and sorted.
    let e = c("d => selected.isEmpty() || selected.has(d.party) ? scale.y(d.v) : scale.y(0)");
    assert_eq!(e.calls(), vec!["scale.y", "selected.has", "selected.isEmpty"]);
    assert_eq!(e.fields(), vec!["party", "v"]);
    assert!(!e.uses_row());
    assert!(c("(d, i) => i % 2").uses_row());
    // Row-free: signals and constants only (a bare name the env lacks as a signal reads a column).
    let zoom = |n: &str| n == "tile.zoom";
    assert!(c("tile.zoom >= 12 ? 2.6 : 1.3").row_free(&zoom));
    assert!(!c("tile.zoom >= 12 ? d.w : 1.3").row_free(&zoom));
    assert!(!c("kind == 'major' ? 2 : 1").row_free(&zoom));
    assert!(!c("(d, i) => tile.zoom + i").row_free(&zoom));
    assert!(!c("scale.y(tile.zoom)").row_free(&zoom));
    // Built-ins aren't host calls.
    assert!(c("abs(d.x) + Math.max(d.y, 1) + String(d.z).length").calls().is_empty());
}

#[test]
fn compile_errors_for_hand_built_trees() {
    let bad = [
        (Expr::Array { items: vec![], span: Default::default() }, "array literal"),
        (Expr::call("clamp", vec![Expr::num(1.0)]), "clamp() takes 3 arguments"),
        (Expr::method(Expr::str("a"), "reverse", vec![]), "unknown method"),
        (Expr::method(Expr::str("a"), "slice", vec![]), ".slice() takes"),
        (
            Expr::method(Expr::Array { items: vec![], span: Default::default() }, "slice", vec![Expr::num(0.0)]),
            "arrays support only",
        ),
    ];
    for (e, needle) in bad {
        let err = compile(&e).unwrap_err();
        assert!(err.message.contains(needle), "{:?} lacks {needle:?}", err.message);
    }
    // Deep hand-built trees are an error, not a stack overflow.
    let mut e = Expr::num(1.0);
    for _ in 0..2000 {
        e = Expr::unary(datars_expr::UnOp::Neg, e);
    }
    assert!(compile(&e).unwrap_err().message.contains("deeply"));
}

/// Trees at the depth limit go through every recursive pass on a default test-thread stack (in
/// debug builds too); deeper ones are rejected up front, including long left-associative chains
/// that the parser builds with loops.
#[test]
fn depth_limit() {
    use datars_expr::{typecheck, BinOp, UnOp, MAX_DEPTH};
    let env = MapEnv::new().num("x", vec![1.0, 2.0]).signal("k", 1.0);
    let shapes: [&dyn Fn(Expr, usize) -> Expr; 6] = [
        &|e, _| Expr::binary(BinOp::Add, e, Expr::field("x")),
        &|e, _| Expr::cond(Expr::binary(BinOp::Gt, Expr::field("x"), Expr::num(1.0)), e, Expr::num(0.0)),
        &|e, _| Expr::unary(UnOp::Neg, e),
        &|e, _| Expr::call("max", vec![e, Expr::signal("k")]),
        &|e, _| Expr::binary(BinOp::Coalesce, e, Expr::num(2.0)),
        &|e, i| if i % 2 == 0 { Expr::method(e, "toString", vec![]) } else { Expr::Concat { parts: vec![e], span: Default::default() } },
    ];
    for (s, shape) in shapes.iter().enumerate() {
        let mut e = Expr::field("x");
        while e.depth() < MAX_DEPTH {
            let d = e.depth();
            e = shape(e, d);
        }
        assert_eq!(e.depth(), MAX_DEPTH, "shape {s}");
        let compiled = compile(&e).unwrap_or_else(|err| panic!("shape {s}: {err}"));
        assert_eq!(compiled.eval_rows(2, &env).len(), 2);
        let _ = typecheck(&e, &env);
        let _ = to_wgsl(&e);
        assert!(!e.to_string().is_empty());
        let deeper = shape(e.clone(), MAX_DEPTH);
        assert!(compile(&deeper).unwrap_err().message.contains("deeply"), "shape {s}");
        assert!(typecheck(&deeper, &env).unwrap_err().message.contains("deeply"), "shape {s}");
        assert_eq!(to_wgsl(&deeper), None);
    }
    // Long chains parse up to the limit and fail cleanly beyond it.
    let chain = |n: usize| vec!["d.x"; n].join(" + ");
    assert!(parse(&chain(MAX_DEPTH - 1)).is_ok());
    assert!(parse(&chain(MAX_DEPTH + 10)).unwrap_err().message.contains("deeply"));
    let methods = format!("\"a\"{}", ".trim()".repeat(MAX_DEPTH + 10));
    assert!(parse(&methods).unwrap_err().message.contains("deeply"));
}

#[test]
fn disassembly() {
    let text = c("d.a > 30 ? d.b : d.c * 2").to_string();
    assert!(text.contains("gt"), "{text}");
    assert!(text.contains("jump if Falsy"), "{text}");
    assert!(text.contains("select_if_truthy"), "{text}");
    assert!(text.contains("Field(b)"), "{text}");
    let text = c("scale.y(d.v)").to_string();
    assert!(text.contains("host scale.y(Field(v))"), "{text}");
}

#[test]
fn compiled_is_reusable_across_envs() {
    let compiled = c("d.x * k");
    let e1 = MapEnv::new().num("x", vec![1.0, 2.0]).signal("k", 10.0);
    let e2 = MapEnv::new().num("x", vec![5.0]).signal("k", -1.0);
    assert_eq!(compiled.eval_rows_num(2, &e1), vec![10.0, 20.0]);
    assert_eq!(compiled.eval_rows_num(1, &e2), vec![-5.0]);
    fn send_sync<T: Send + Sync + Clone>(_: &T) {}
    send_sync(&compiled);
}

#[test]
fn wgsl_numeric_subset() {
    let w = |src: &str| to_wgsl(&parse(src).unwrap());
    assert_eq!(w("d.value * 2 + 1").as_deref(), Some("((inst.value * 2.0) + 1.0)"));
    assert_eq!(w("d => d.x * k").as_deref(), Some("(inst.x * u.k)"));
    assert_eq!(w("d.a > 30 ? d.b : d.c").as_deref(), Some("select(inst.c, inst.b, (inst.a > 30.0))"));
    assert_eq!(w("d.a ? 1 : 0").as_deref(), Some("select(0.0, 1.0, (inst.a != 0.0))"));
    assert_eq!(w("d.a > 1 && d.b < 2").as_deref(), Some("select(0.0, 1.0, ((inst.a > 1.0) && (inst.b < 2.0)))"));
    assert_eq!(w("sqrt(abs(d.x)) ** 2").as_deref(), Some("pow(sqrt(abs(inst.x)), 2.0)"));
    assert_eq!(w("clamp(lerp(0, 10, d.t), 1, 9)").as_deref(), Some("clamp(mix(0.0, 10.0, inst.t), 1.0, 9.0)"));
    assert_eq!(w("max(d.a, d.b, 0)").as_deref(), Some("max(max(inst.a, inst.b), 0.0)"));
    assert_eq!(w("round(d.x)").as_deref(), Some("floor(inst.x + 0.5)"));
    assert_eq!(w("round(d.x, 2)").as_deref(), Some("(floor(inst.x * 100.0 + 0.5) / 100.0)"));
    assert_eq!(w("-d.x % 3").as_deref(), Some("((-inst.x) % 3.0)"));
    assert_eq!(w("d.x - -1.5").as_deref(), Some("(inst.x - (-1.5))"));
    assert_eq!(w("atan2(d.y, d.x) * 180 / PI").as_deref(), Some("((atan2(inst.y, inst.x) * 180.0) / 3.141592653589793)"));
    assert_eq!(w("hypot(d.x, d.y)").as_deref(), Some("length(vec2<f32>(inst.x, inst.y))"));
    assert_eq!(w("log10(d.x)").as_deref(), Some("(log(inst.x) * 0.4342944819032518)"));
    assert_eq!(w("1e-7").as_deref(), Some("1e-7"));
    // Constant subtrees fold with the CPU semantics.
    assert_eq!(w("d.x * round(PI, 2)").as_deref(), Some("(inst.x * 3.14)"));
    assert_eq!(w("d.x * (60 * 60)").as_deref(), Some("(inst.x * 3600.0)"));
    assert_eq!(w("1 < 2").as_deref(), Some("select(0.0, 1.0, true)"));
    assert_eq!(w("\"abc\".length * d.x").as_deref(), Some("(3.0 * inst.x)"));
    // Outside the subset.
    for src in [
        "d.name + \"x\"",
        "null",
        "d.x ?? 0",
        "scale.y(d.x)",
        "(d, i) => i",
        "d.x > 1 ? 1 : true",
        "1 / 0 + d.x",
        "round(d.x, d.y)",
        "String(d.x)",
        "d.s.length",
        "d[\"has space\"]",
        "d.fn",
        "value * 2",
        "d.x || 1",
    ] {
        assert_eq!(w(src), None, "{src}");
    }
    // Bare identifiers resolve with a TypeEnv (and must be numeric).
    let env = MapEnv::new().num("value", vec![]).strs("name", &[]).signal("k", 1.0);
    assert_eq!(to_wgsl_with(&parse("value * k").unwrap(), &env).as_deref(), Some("(inst.value * u.k)"));
    assert_eq!(to_wgsl_with(&parse("name").unwrap(), &env), None);
    assert_eq!(to_wgsl_with(&parse("d.missing").unwrap(), &env), None);
}
