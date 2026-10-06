//! Property test: the vector VM matches the scalar interpreter bit for bit, on random expressions
//! over random columns (seeded `datars_math::Rng`), across chunk boundaries and short columns.
//! Also: printing and re-parsing a random tree gives an equal tree.

use datars_expr::{compile, parse, BinOp, Expr, MapEnv, UnOp, Value};
use datars_math::Rng;

const N: usize = 2600; // crosses two chunk boundaries

fn pick<'a, T>(rng: &mut Rng, items: &'a [T]) -> &'a T {
    &items[rng.below(items.len() as u64) as usize]
}

fn random_num(rng: &mut Rng) -> f64 {
    match rng.below(10) {
        0 => f64::NAN,
        1 => 0.0,
        2 => -0.0,
        3 => rng.below(5) as f64,
        4 => f64::INFINITY,
        _ => (rng.range(-100.0, 100.0) * 4.0).round() / 4.0,
    }
}

fn env(rng: &mut Rng) -> MapEnv {
    let nums = |rng: &mut Rng, len: usize| (0..len).map(|_| random_num(rng)).collect::<Vec<f64>>();
    let words = ["", "a", "abc", "Alice", "bob", "S", "M", "  pad ", "wörld", "10", "9"];
    let strs: Vec<Option<&str>> =
        (0..N).map(|_| if rng.below(8) == 0 { None } else { Some(*pick(rng, &words)) }).collect();
    let bools: Vec<bool> = (0..N).map(|_| rng.below(2) == 0).collect();
    MapEnv::new()
        .num("a", nums(rng, N))
        .num("b", nums(rng, N))
        .num("short", nums(rng, N - 700)) // shorter than N: padded with null
        .strs("s", &strs)
        .strs("t", &strs[..N / 2])
        .bools("f", bools.clone())
        .bools("g", bools[..N - 1000].to_vec())
        .signal("k", 3.0)
        .signal("z", 0.0)
        .signal("w", "abc")
        .signal("on", true)
        .signal("off", false)
        .signal("nul", Value::Null)
        .function("h.f", |args| match args.first()? {
            Value::Num(x) if x.is_finite() => Some(Value::Num(x * 2.0)),
            Value::Str(s) => Some(Value::from(format!("<{s}>"))),
            Value::Bool(b) => Some(Value::Bool(!b)),
            _ => None,
        })
        .function("h.zero", |_| Some(Value::Num(7.0)))
}

struct Gen<'a> {
    rng: &'a mut Rng,
}

impl Gen<'_> {
    fn leaf(&mut self) -> Expr {
        match self.rng.below(12) {
            0 | 1 => Expr::field(*pick(self.rng, &["a", "b", "short", "s", "t", "f", "g", "missing"])),
            2 => Expr::ident(*pick(self.rng, &["k", "z", "w", "on", "off", "nul", "a", "s", "nosuch"])),
            3 => Expr::row(),
            4 | 5 => Expr::num(random_num(self.rng)),
            6 => Expr::str(*pick(self.rng, &["", "x", "abc", "S", "10"])),
            7 => Expr::bool(self.rng.below(2) == 0),
            8 => Expr::null(),
            _ => Expr::field(*pick(self.rng, &["a", "b", "s", "f"])),
        }
    }

    fn expr(&mut self, depth: u32) -> Expr {
        if depth == 0 || self.rng.below(5) == 0 {
            return self.leaf();
        }
        let d = depth - 1;
        match self.rng.below(16) {
            0 => {
                let op = *pick(self.rng, &[UnOp::Neg, UnOp::Pos, UnOp::Not]);
                Expr::unary(op, self.expr(d))
            }
            1..=5 => {
                use BinOp::*;
                let op = *pick(
                    self.rng,
                    &[Add, Sub, Mul, Div, Rem, Pow, Lt, Le, Gt, Ge, Eq, Ne, StrictEq, StrictNe, And, Or, Coalesce],
                );
                Expr::binary(op, self.expr(d), self.expr(d))
            }
            6 | 7 => Expr::cond(self.expr(d), self.expr(d), self.expr(d)),
            8 | 9 => {
                let (name, n) = *pick(
                    self.rng,
                    &[
                        ("abs", 1),
                        ("round", 1),
                        ("round", 2),
                        ("sqrt", 1),
                        ("log", 1),
                        ("sin", 1),
                        ("sign", 1),
                        ("pow", 2),
                        ("atan2", 2),
                        ("clamp", 3),
                        ("lerp", 3),
                        ("min", 0),
                        ("min", 3),
                        ("max", 2),
                        ("hypot", 2),
                        ("String", 1),
                        ("Number", 1),
                        ("Boolean", 1),
                        ("isNaN", 1),
                        ("isFinite", 1),
                    ],
                );
                Expr::call(name, (0..n).map(|_| self.expr(d)).collect())
            }
            10 => {
                let (name, n) = *pick(
                    self.rng,
                    &[
                        ("toUpperCase", 0),
                        ("trim", 0),
                        ("length", 0),
                        ("slice", 1),
                        ("slice", 2),
                        ("includes", 1),
                        ("indexOf", 1),
                        ("startsWith", 1),
                        ("toString", 0),
                    ],
                );
                Expr::method(self.expr(d), name, (0..n).map(|_| self.expr(d)).collect())
            }
            11 => Expr::Concat { parts: (0..self.rng.below(4)).map(|_| self.expr(d)).collect(), span: Default::default() },
            12 => {
                let items: Vec<Expr> = (0..self.rng.below(4)).map(|_| self.expr(d)).collect();
                let arr = Expr::Array { items, span: Default::default() };
                match self.rng.below(3) {
                    0 => Expr::method(arr, "includes", vec![self.expr(d)]),
                    1 => Expr::method(arr, "indexOf", vec![self.expr(d)]),
                    _ => Expr::Index { obj: Box::new(arr), index: Box::new(self.expr(d)), span: Default::default() },
                }
            }
            13 => Expr::Index { obj: Box::new(self.expr(d)), index: Box::new(self.expr(d)), span: Default::default() },
            14 => {
                if self.rng.below(4) == 0 {
                    Expr::call("h.zero", vec![])
                } else {
                    Expr::call("h.f", vec![self.expr(d)])
                }
            }
            _ => self.leaf(),
        }
    }
}

#[test]
fn vector_matches_scalar_on_random_expressions() {
    let seeds: Vec<u64> = match std::env::var("DATARS_EXPR_SEEDS") {
        Ok(n) => (0..n.parse().unwrap_or(1)).collect(),
        Err(_) => vec![0x5eed_da7a, 1],
    };
    let (mut checked, mut vectorized) = (0, 0);
    for seed in seeds {
        let mut rng = Rng::new(seed);
        let env = env(&mut rng);
        for case in 0..300 {
            let e = Gen { rng: &mut rng }.expr(4);
            let c = compile(&e).unwrap_or_else(|err| panic!("seed {seed} case {case}: {err} for {e}"));
            let n = [N, 1, 0, 1023, 1024, 1025][case % 6];
            let rows = c.eval_rows(n, &env);
            let nums = c.eval_rows_num(n, &env);
            assert_eq!(rows.len(), n);
            assert_eq!(nums.len(), n);
            for i in 0..n {
                let want = c.eval_row(i, &env);
                assert!(
                    rows[i].identical(&want),
                    "seed {seed} case {case} row {i}: `{e}` vector {:?} vs scalar {want:?}\n{c}",
                    rows[i]
                );
                let want_num = if want.to_num().is_nan() { f64::NAN } else { want.to_num() };
                assert_eq!(nums[i].to_bits(), want_num.to_bits(), "seed {seed} case {case} row {i}: `{e}` numeric");
            }
            checked += n;
            vectorized += usize::from(!c.is_empty());
        }
    }
    assert!(checked > 500_000);
    assert!(vectorized > 300, "most random programs should need instructions ({vectorized})");
}

/// Folding a constant subtree gives the same value as evaluating it: random trees without inputs
/// compile to a single constant.
#[test]
fn folding_matches_evaluation() {
    let mut rng = Rng::new(7);
    let env = MapEnv::new();
    for case in 0..500 {
        let e = Gen { rng: &mut rng }.expr(4);
        let mut pure = true;
        e.visit(&mut |x| {
            if matches!(x, Expr::Field { .. } | Expr::Ident { .. } | Expr::Signal { .. } | Expr::Row { .. })
                || matches!(x, Expr::Call { name, .. } if name.contains('.'))
            {
                pure = false;
            }
        });
        let c = compile(&e).unwrap();
        if pure {
            assert!(c.constant().is_some(), "case {case}: `{e}` wasn't folded\n{c}");
            assert!(c.is_empty(), "case {case}: `{e}`");
        }
        let _ = c.eval_rows(3, &env);
    }
}

/// Canonical printing round-trips random trees built from what the parser can produce.
#[test]
fn random_trees_round_trip_through_source() {
    let mut rng = Rng::new(42);
    let mut tested = 0;
    for _ in 0..2000 {
        let e = Gen { rng: &mut rng }.expr(5);
        // Bare identifiers and the row index can't share a tree the parser produced (a lambda
        // makes free names signals), and neither can `Ident` roots that collide with `d`.
        // Template parts are merged text: adjacent or empty string parts aren't parser output.
        let (mut ident, mut row, mut odd_template) = (false, false, false);
        e.visit(&mut |x| match x {
            Expr::Ident { .. } => ident = true,
            Expr::Row { .. } => row = true,
            Expr::Concat { parts, .. } => {
                let text = |p: &Expr| matches!(p, Expr::Str { .. });
                odd_template |= parts.iter().any(|p| matches!(p, Expr::Str { value, .. } if value.is_empty()))
                    || parts.windows(2).any(|w| text(&w[0]) && text(&w[1]));
            }
            _ => {}
        });
        if (ident && row) || odd_template {
            continue;
        }
        let src = e.to_string();
        let back = parse(&src).unwrap_or_else(|err| panic!("`{src}` from {e:?}: {}", err.render(&src)));
        assert_eq!(back, e, "`{src}`");
        assert_eq!(Value::from(back.to_string()), Value::from(src));
        let json = e.to_json();
        assert_eq!(datars_expr::Expr::from_json(&json).unwrap(), e);
        tested += 1;
    }
    assert!(tested > 1000);
}
