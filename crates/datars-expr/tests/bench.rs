//! Throughput benchmarks (ignored by default; timing belongs in release builds):
//!
//! ```sh
//! cargo test --release -p datars-expr --test bench -- --ignored --nocapture
//! ```
//!
//! Targets (release, 1,000,000 rows): `d.value * 2 + 1` < 20 ms, `d.a > 30 ? d.b : d.c` < 50 ms.

use datars_expr::{compile, parse, MapEnv, Value};
use datars_math::Rng;
use std::time::{Duration, Instant};

const N: usize = 1_000_000;

fn env() -> MapEnv {
    let mut rng = Rng::new(1);
    let mut col = |lo: f64, hi: f64| (0..N).map(|_| rng.range(lo, hi)).collect::<Vec<f64>>();
    let (value, a, b, c) = (col(-100.0, 100.0), col(0.0, 60.0), col(0.0, 1.0), col(1.0, 2.0));
    let names = ["Alice", "Bob", "Carol", "Dave"];
    let parties = ["S", "M", "C", "V", "L"];
    let name: Vec<Option<&str>> = (0..N).map(|i| Some(names[i % names.len()])).collect();
    let party: Vec<Option<&str>> = (0..N).map(|i| Some(parties[i % parties.len()])).collect();
    MapEnv::new()
        .num("value", value)
        .num("a", a)
        .num("b", b)
        .num("c", c)
        .strs("name", &name)
        .strs("party", &party)
        .signal("k", 2.0)
        .function("scale.y", |args| Some(Value::Num(300.0 - args.first()?.as_num()? * 1.5)))
        .function("selected.has", |args| Some(Value::Bool(args.first()?.as_str() == Some("S"))))
}

/// Best of `runs` wall-clock timings.
fn best(runs: usize, mut f: impl FnMut()) -> Duration {
    (0..runs)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed()
        })
        .min()
        .unwrap_or_default()
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

fn bench(env: &MapEnv, src: &str, rows: usize) -> (f64, f64) {
    let c = compile(&parse(src).unwrap()).unwrap();
    let values = best(7, || {
        let v = c.eval_rows(rows, env);
        assert_eq!(v.len(), rows);
    });
    let nums = best(7, || {
        let v = c.eval_rows_num(rows, env);
        assert_eq!(v.len(), rows);
    });
    println!(
        "{src:<52} {rows:>9} rows   eval_rows {:>7.2} ms   eval_rows_num {:>7.2} ms",
        ms(values),
        ms(nums)
    );
    (ms(values), ms(nums))
}

#[test]
#[ignore]
fn throughput() {
    let env = env();
    let (arith, arith_num) = bench(&env, "d.value * 2 + 1", N);
    let (cond, cond_num) = bench(&env, "d.a > 30 ? d.b : d.c", N);
    bench(&env, "clamp(d.a / 60, 0, 1) * 255", N);
    bench(&env, "sqrt(d.a) + sin(d.value) * k", N);
    bench(&env, "(d, i) => i % 2 == 0 ? d.a : -d.a", N);
    bench(&env, "d.a > 30 && d.b < 0.5 ? 1 : 0.3", N);
    bench(&env, "scale.y(d.value)", N);
    bench(&env, "selected.has(d.party) ? 1 : 0.3", N);
    bench(&env, "[\"S\", \"C\"].includes(d.party) ? 1 : 0", N);
    bench(&env, "`${d.name}: ${round(d.value, 1)}`", N / 10);

    // The scalar interpreter, for scale.
    let c = compile(&parse("d.value * 2 + 1").unwrap()).unwrap();
    let scalar = best(3, || {
        let mut acc = 0.0;
        for i in 0..N {
            acc += c.eval_row(i, &env).to_num();
        }
        assert!(acc.is_finite());
    });
    println!("{:<52} {N:>9} rows   scalar per-row {:>7.2} ms", "d.value * 2 + 1 (eval_row loop)", ms(scalar));

    if !cfg!(debug_assertions) {
        assert!(arith < 20.0 && arith_num < 20.0, "d.value * 2 + 1: {arith:.2} ms / {arith_num:.2} ms");
        assert!(cond < 50.0 && cond_num < 50.0, "d.a > 30 ? d.b : d.c: {cond:.2} ms / {cond_num:.2} ms");
    }
}

/// A quick always-on sanity check that the vector path handles a million rows correctly.
#[test]
fn million_rows_quick() {
    let x: Vec<f64> = (0..N).map(|i| i as f64).collect();
    let env = MapEnv::new().num("x", x);
    let c = compile(&parse("d.x * 2 + 1").unwrap()).unwrap();
    let v = c.eval_rows_num(N, &env);
    assert_eq!(v.len(), N);
    assert_eq!(v[0], 1.0);
    assert_eq!(v[N - 1], (N - 1) as f64 * 2.0 + 1.0);
}
