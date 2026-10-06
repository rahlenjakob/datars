//! The evaluators.
//!
//! - [`eval_op`]: the scalar semantics of one instruction. The single source of truth — constant
//!   folding, the scalar interpreter and every generic vector path call it.
//! - [`eval_row`]: the scalar reference interpreter (one row, `Value` registers).
//! - [`eval_rows`] / [`eval_rows_num`]: the vector VM. Rows are processed in chunks of [`CHUNK`];
//!   each instruction runs over the whole chunk before the next one starts (one dispatch per
//!   instruction per chunk). Registers hold typed lanes (`Vec<f64>`, `Vec<bool>`, `Vec<Value>`), a
//!   runtime-constant scalar, or an alias; columns are borrowed, never copied. Fast kernels cover
//!   numeric arithmetic, comparisons, math built-ins and selects; anything else falls back to
//!   `eval_op` per row, so results are identical to the scalar interpreter by construction.
//!
//! Output NaNs are canonicalized (hardware differs in the NaN bits it produces), so results are
//! bit-identical across targets.

use crate::ast::UnOp;
use crate::builtins::{self, Conv, F1, F2};
use crate::compile::{Bin, Compiled, InputKind, Op, Opnd, When};
use crate::env::{ColumnView, Env};
use crate::value::{self, Cmp, Value, NULL};
use std::sync::Arc;

/// Rows per chunk: big enough to amortize dispatch, small enough that a chunk's registers stay in
/// cache.
pub(crate) const CHUNK: usize = 1024;

// ---------------------------------------------------------------------------------------------
// Scalar semantics
// ---------------------------------------------------------------------------------------------

/// Evaluates one instruction on scalar operands. `Host` and `Jump` need the interpreter and
/// return null here.
pub(crate) fn eval_op(op: &Op, get: &dyn Fn(Opnd) -> Value) -> Value {
    match op {
        Op::Un { op, a, .. } => un(*op, &get(*a)),
        Op::Bin { op, a, b, .. } => bin(*op, &get(*a), &get(*b)),
        Op::F1 { f, a, .. } => Value::Num(f.apply(get(*a).to_num())),
        Op::F2 { f, a, b, .. } => Value::Num(f.apply(get(*a).to_num(), get(*b).to_num())),
        Op::F3 { f, a, b, c, .. } => Value::Num(f.apply(get(*a).to_num(), get(*b).to_num(), get(*c).to_num())),
        Op::FN { f, args, .. } => Value::Num(args.iter().fold(f.identity(), |acc, o| f.apply(acc, get(*o).to_num()))),
        Op::Conv { f, a, .. } => f.apply(&get(*a)),
        Op::Method { m, recv, args, .. } => {
            let argv: Vec<Value> = args.iter().map(|o| get(*o)).collect();
            m.apply(&get(*recv), &argv)
        }
        Op::Index { obj, index, .. } => builtins::index_value(&get(*obj), &get(*index)),
        Op::InList { x, items, index_of, .. } => {
            let x = get(*x);
            let pos = items.iter().position(|o| value::equals(&x, &get(*o)));
            if *index_of {
                Value::Num(pos.map_or(-1.0, |p| p as f64))
            } else {
                Value::Bool(pos.is_some())
            }
        }
        Op::Pick { index, items, .. } => match builtins::pick_index(&get(*index), items.len()) {
            Some(i) => get(items[i]),
            None => Value::Null,
        },
        Op::Concat { parts, .. } => {
            let mut s = String::new();
            for p in parts {
                s.push_str(&get(*p).to_str());
            }
            Value::from(s)
        }
        Op::Select { test, when, then, other, .. } => {
            if when.holds(&get(*test)) {
                get(*then)
            } else {
                get(*other)
            }
        }
        Op::Host { .. } | Op::Jump { .. } => Value::Null,
    }
}

fn un(op: UnOp, a: &Value) -> Value {
    match op {
        UnOp::Neg => Value::Num(-a.to_num()),
        UnOp::Pos => Value::Num(a.to_num()),
        UnOp::Not => Value::Bool(!a.truthy()),
    }
}

fn cmp_of(op: Bin) -> Cmp {
    match op {
        Bin::Lt => Cmp::Lt,
        Bin::Le => Cmp::Le,
        Bin::Gt => Cmp::Gt,
        _ => Cmp::Ge,
    }
}

fn bin(op: Bin, a: &Value, b: &Value) -> Value {
    match op {
        Bin::Add => value::add(a, b),
        Bin::Sub => Value::Num(a.to_num() - b.to_num()),
        Bin::Mul => Value::Num(a.to_num() * b.to_num()),
        Bin::Div => Value::Num(a.to_num() / b.to_num()),
        Bin::Rem => Value::Num(value::js_rem(a.to_num(), b.to_num())),
        Bin::Pow => Value::Num(value::js_pow(a.to_num(), b.to_num())),
        Bin::Lt | Bin::Le | Bin::Gt | Bin::Ge => Value::Bool(value::order(cmp_of(op), a, b)),
        Bin::Eq => Value::Bool(value::equals(a, b)),
        Bin::Ne => Value::Bool(!value::equals(a, b)),
    }
}

#[inline(always)]
fn canon(x: f64) -> f64 {
    if x.is_nan() {
        f64::NAN
    } else {
        x
    }
}

fn canon_value(v: Value) -> Value {
    match v {
        Value::Num(x) if x.is_nan() => Value::Num(f64::NAN),
        v => v,
    }
}

// ---------------------------------------------------------------------------------------------
// Scalar interpreter
// ---------------------------------------------------------------------------------------------

fn input_at(env: &dyn Env, kind: InputKind, name: &str, row: usize) -> Value {
    if !reads_signal(env, kind, name) {
        if let Some(col) = env.column(name) {
            return col.get(row);
        }
    }
    match kind {
        InputKind::Field => Value::Null,
        _ => env.signal(name).unwrap_or(Value::Null),
    }
}

/// Signals are read as signals; bare names too when the env has such a signal (see
/// [`Env::has_signal`]).
fn reads_signal(env: &dyn Env, kind: InputKind, name: &str) -> bool {
    kind == InputKind::Signal || (kind == InputKind::Ident && env.has_signal(name))
}

/// Evaluates row `row` with scalar registers. Every value is "runtime-constant" here, so every
/// jump whose condition holds is taken and only live branches run.
pub(crate) fn eval_row(c: &Compiled, row: usize, env: &dyn Env) -> Value {
    let inputs: Vec<Value> = c.inputs.iter().map(|i| input_at(env, i.kind, &i.name, row)).collect();
    let mut regs: Vec<Value> = vec![Value::Null; c.nregs];
    let mut pc = 0;
    while pc < c.code.len() {
        let op = &c.code[pc];
        let v = {
            let get = |o: Opnd| -> Value {
                match o {
                    Opnd::Reg(r) => regs.get(r as usize).cloned().unwrap_or(Value::Null),
                    Opnd::Const(k) => c.consts.get(k as usize).cloned().unwrap_or(Value::Null),
                    Opnd::Input(i) => inputs.get(i as usize).cloned().unwrap_or(Value::Null),
                    Opnd::Row => Value::Num(row as f64),
                }
            };
            match op {
                Op::Jump { test, when, target } => {
                    pc = if when.holds(&get(*test)) { *target as usize } else { pc + 1 };
                    continue;
                }
                Op::Host { name, args, .. } => {
                    let argv: Vec<Value> = args.iter().map(|o| get(*o)).collect();
                    let name = c.hosts.get(*name as usize).map_or("", String::as_str);
                    env.call(name, &argv).unwrap_or(Value::Null)
                }
                _ => eval_op(op, &get),
            }
        };
        if let Some(r) = op.dst().and_then(|d| regs.get_mut(d as usize)) {
            *r = v;
        }
        pc += 1;
    }
    let result = match c.result {
        Opnd::Reg(r) => regs.get(r as usize).cloned().unwrap_or(Value::Null),
        Opnd::Const(k) => c.consts.get(k as usize).cloned().unwrap_or(Value::Null),
        Opnd::Input(i) => inputs.get(i as usize).cloned().unwrap_or(Value::Null),
        Opnd::Row => Value::Num(row as f64),
    };
    canon_value(result)
}

// ---------------------------------------------------------------------------------------------
// Vector VM
// ---------------------------------------------------------------------------------------------

/// An input resolved once per evaluation.
enum Bound<'a> {
    Scalar(Value),
    Num(&'a [f64]),
    Str(&'a [Option<Arc<str>>]),
    Bool(&'a [bool]),
    Val(&'a [Value]),
}

/// A padded copy of a column shorter than the row count (rows past the end read as null).
enum Owned {
    Num(Vec<f64>),
    Str(Vec<Option<Arc<str>>>),
    Val(Vec<Value>),
}

fn pad(col: ColumnView<'_>, n: usize) -> Owned {
    match col {
        ColumnView::Num(s) => {
            let mut v = s.to_vec();
            v.resize(n, f64::NAN);
            Owned::Num(v)
        }
        ColumnView::Str(s) => {
            let mut v = s.to_vec();
            v.resize(n, None);
            Owned::Str(v)
        }
        ColumnView::Bool(s) => {
            let mut v: Vec<Value> = s.iter().map(|&b| Value::Bool(b)).collect();
            v.resize(n, Value::Null);
            Owned::Val(v)
        }
    }
}

fn bind<'a>(c: &Compiled, env: &'a dyn Env, n: usize, arena: &'a mut Vec<Owned>) -> Vec<Bound<'a>> {
    let cols: Vec<Option<ColumnView<'a>>> = c
        .inputs
        .iter()
        .map(|i| if reads_signal(env, i.kind, &i.name) { None } else { env.column(&i.name) })
        .collect();
    let mut slots: Vec<Option<usize>> = vec![None; cols.len()];
    for (i, col) in cols.iter().enumerate() {
        if let Some(col) = col {
            if col.len() < n {
                arena.push(pad(*col, n));
                slots[i] = Some(arena.len() - 1);
            }
        }
    }
    let arena: &'a Vec<Owned> = arena;
    c.inputs
        .iter()
        .enumerate()
        .map(|(i, inp)| {
            if let Some(a) = slots[i] {
                return match &arena[a] {
                    Owned::Num(v) => Bound::Num(v),
                    Owned::Str(v) => Bound::Str(v),
                    Owned::Val(v) => Bound::Val(v),
                };
            }
            match cols[i] {
                Some(ColumnView::Num(s)) => Bound::Num(&s[..n]),
                Some(ColumnView::Str(s)) => Bound::Str(&s[..n]),
                Some(ColumnView::Bool(s)) => Bound::Bool(&s[..n]),
                None => Bound::Scalar(match inp.kind {
                    InputKind::Field => Value::Null,
                    _ => env.signal(&inp.name).unwrap_or(Value::Null),
                }),
            }
        })
        .collect()
}

/// A register's contents for the current chunk.
enum Reg {
    Unset,
    /// Runtime-constant (same for every row, hence for every chunk: computed once).
    Scalar(Value),
    /// The same as another operand (a select with a runtime-constant test).
    Alias(Opnd),
    Num(Vec<f64>),
    Bool(Vec<bool>),
    Val(Vec<Value>),
}

fn take_num(r: &mut Reg) -> Vec<f64> {
    match std::mem::replace(r, Reg::Unset) {
        Reg::Num(mut v) => {
            v.clear();
            v
        }
        _ => Vec::with_capacity(CHUNK),
    }
}

fn take_bool(r: &mut Reg) -> Vec<bool> {
    match std::mem::replace(r, Reg::Unset) {
        Reg::Bool(mut v) => {
            v.clear();
            v
        }
        _ => Vec::with_capacity(CHUNK),
    }
}

fn take_val(r: &mut Reg) -> Vec<Value> {
    match std::mem::replace(r, Reg::Unset) {
        Reg::Val(mut v) => {
            v.clear();
            v
        }
        _ => Vec::with_capacity(CHUNK),
    }
}

/// An operand's rows for the current chunk.
#[derive(Clone, Copy)]
enum View<'a> {
    Scalar(&'a Value),
    Num(&'a [f64]),
    Bool(&'a [bool]),
    Str(&'a [Option<Arc<str>>]),
    Val(&'a [Value]),
}

struct Ctx<'a> {
    bound: &'a [Bound<'a>],
    consts: &'a [Value],
    hosts: &'a [String],
    rows: &'a [f64],
    start: usize,
    len: usize,
}

fn view<'r>(ctx: &'r Ctx<'_>, regs: &'r [Reg], o: Opnd) -> View<'r> {
    let (s, e) = (ctx.start, ctx.start + ctx.len);
    match o {
        Opnd::Const(i) => View::Scalar(ctx.consts.get(i as usize).unwrap_or(&NULL)),
        Opnd::Row => View::Num(ctx.rows),
        Opnd::Input(i) => match ctx.bound.get(i as usize) {
            Some(Bound::Scalar(v)) => View::Scalar(v),
            Some(Bound::Num(c)) => View::Num(&c[s..e]),
            Some(Bound::Str(c)) => View::Str(&c[s..e]),
            Some(Bound::Bool(c)) => View::Bool(&c[s..e]),
            Some(Bound::Val(c)) => View::Val(&c[s..e]),
            None => View::Scalar(&NULL),
        },
        Opnd::Reg(r) => match regs.get(r as usize) {
            Some(Reg::Scalar(v)) => View::Scalar(v),
            Some(Reg::Alias(o)) => view(ctx, regs, *o),
            Some(Reg::Num(v)) => View::Num(v),
            Some(Reg::Bool(v)) => View::Bool(v),
            Some(Reg::Val(v)) => View::Val(v),
            Some(Reg::Unset) | None => View::Scalar(&NULL),
        },
    }
}

/// Row `i` of a view as a `Value` (null if out of range, which the kernels never produce).
#[inline]
fn get(v: View<'_>, i: usize) -> Value {
    match v {
        View::Scalar(x) => x.clone(),
        View::Num(s) => s.get(i).map_or(Value::Null, |x| Value::Num(*x)),
        View::Bool(s) => s.get(i).map_or(Value::Null, |b| Value::Bool(*b)),
        View::Str(s) => s.get(i).cloned().flatten().map_or(Value::Null, Value::Str),
        View::Val(s) => s.get(i).cloned().unwrap_or(Value::Null),
    }
}

/// Lanes whose values coerce to numbers without looking at strings (for `+` and ordering).
fn numeric_kind(v: &View<'_>) -> bool {
    matches!(v, View::Num(_) | View::Bool(_) | View::Scalar(Value::Num(_) | Value::Bool(_) | Value::Null))
}

/// Lanes where `==` is numeric (null ≡ NaN).
fn num_eq_kind(v: &View<'_>) -> bool {
    matches!(v, View::Num(_) | View::Scalar(Value::Num(_) | Value::Null))
}

/// Lanes that are numbers, full stop (a select of two of these is a numeric lane).
fn num_kind(v: &View<'_>) -> bool {
    matches!(v, View::Num(_) | View::Scalar(Value::Num(_)))
}

fn bool_kind(v: &View<'_>) -> bool {
    matches!(v, View::Bool(_) | View::Scalar(Value::Bool(_)))
}

/// A boolean operand: a scalar or a lane.
#[derive(Clone, Copy)]
enum BS<'a> {
    S(bool),
    V(&'a [bool]),
}

fn bools(v: View<'_>) -> Option<BS<'_>> {
    match v {
        View::Bool(s) => Some(BS::V(s)),
        View::Scalar(Value::Bool(b)) => Some(BS::S(*b)),
        _ => None,
    }
}

#[inline]
fn bool_at(v: View<'_>, i: usize) -> bool {
    match v {
        View::Bool(s) => s.get(i).copied().unwrap_or(false),
        View::Scalar(x) => x.truthy(),
        _ => false,
    }
}

/// A numeric operand: a scalar, a borrowed lane, or a converted copy.
enum Nums<'a> {
    S(f64),
    V(&'a [f64]),
    O(Vec<f64>),
}

#[derive(Clone, Copy)]
enum NS<'a> {
    S(f64),
    V(&'a [f64]),
}

impl Nums<'_> {
    fn ns(&self) -> NS<'_> {
        match self {
            Nums::S(x) => NS::S(*x),
            Nums::V(s) => NS::V(s),
            Nums::O(v) => NS::V(v),
        }
    }
}

impl NS<'_> {
    #[inline(always)]
    fn at(self, i: usize) -> f64 {
        match self {
            NS::S(x) => x,
            NS::V(s) => s.get(i).copied().unwrap_or(f64::NAN),
        }
    }
}

/// The arithmetic coercion over a lane (see `Value::to_num`).
fn nums(v: View<'_>) -> Nums<'_> {
    match v {
        View::Num(s) => Nums::V(s),
        View::Scalar(x) => Nums::S(x.to_num()),
        View::Bool(s) => Nums::O(s.iter().map(|&b| if b { 1.0 } else { 0.0 }).collect()),
        View::Str(s) => Nums::O(vec![f64::NAN; s.len()]),
        View::Val(s) => Nums::O(s.iter().map(Value::to_num).collect()),
    }
}

#[derive(Clone, Copy)]
enum SV<'a> {
    S(&'a str),
    V(&'a [Option<Arc<str>>]),
}

fn strs(v: View<'_>) -> Option<SV<'_>> {
    match v {
        View::Str(s) => Some(SV::V(s)),
        View::Scalar(Value::Str(s)) => Some(SV::S(s)),
        _ => None,
    }
}

#[inline]
fn str_at(s: SV<'_>, i: usize) -> Option<&str> {
    match s {
        SV::S(x) => Some(x),
        SV::V(v) => v.get(i).and_then(|o| o.as_deref()),
    }
}

#[inline(always)]
fn map1(a: NS<'_>, len: usize, out: &mut Vec<f64>, f: impl Fn(f64) -> f64) {
    match a {
        NS::V(x) => out.extend(x.iter().map(|&x| f(x))),
        NS::S(x) => out.resize(len, f(x)),
    }
}

#[inline(always)]
fn map2(a: NS<'_>, b: NS<'_>, len: usize, out: &mut Vec<f64>, f: impl Fn(f64, f64) -> f64) {
    match (a, b) {
        (NS::V(x), NS::V(y)) => out.extend(x.iter().zip(y).map(|(&x, &y)| f(x, y))),
        (NS::V(x), NS::S(y)) => out.extend(x.iter().map(|&x| f(x, y))),
        (NS::S(x), NS::V(y)) => out.extend(y.iter().map(|&y| f(x, y))),
        (NS::S(x), NS::S(y)) => out.resize(len, f(x, y)),
    }
}

#[inline(always)]
fn cmp2(a: NS<'_>, b: NS<'_>, len: usize, out: &mut Vec<bool>, f: impl Fn(f64, f64) -> bool) {
    match (a, b) {
        (NS::V(x), NS::V(y)) => out.extend(x.iter().zip(y).map(|(&x, &y)| f(x, y))),
        (NS::V(x), NS::S(y)) => out.extend(x.iter().map(|&x| f(x, y))),
        (NS::S(x), NS::V(y)) => out.extend(y.iter().map(|&y| f(x, y))),
        (NS::S(x), NS::S(y)) => out.resize(len, f(x, y)),
    }
}

/// Per-row truthiness (`when`) of a lane.
fn mask(v: View<'_>, when: When, len: usize, out: &mut Vec<bool>) {
    let truthy = |x: f64| !(x == 0.0 || x.is_nan());
    match (v, when) {
        (View::Bool(b), When::Truthy) => out.extend_from_slice(b),
        (View::Bool(b), When::Falsy) => out.extend(b.iter().map(|&b| !b)),
        (View::Bool(b), When::NotNull) => out.resize(out.len() + b.len(), true),
        (View::Num(s), When::Truthy) => out.extend(s.iter().map(|&x| truthy(x))),
        (View::Num(s), When::Falsy) => out.extend(s.iter().map(|&x| !truthy(x))),
        (View::Num(s), When::NotNull) => out.extend(s.iter().map(|&x| !x.is_nan())),
        (View::Str(s), When::Truthy) => out.extend(s.iter().map(|o| o.as_ref().is_some_and(|s| !s.is_empty()))),
        (View::Str(s), When::Falsy) => out.extend(s.iter().map(|o| !o.as_ref().is_some_and(|s| !s.is_empty()))),
        (View::Str(s), When::NotNull) => out.extend(s.iter().map(Option::is_some)),
        (View::Val(s), w) => out.extend(s.iter().map(|v| w.holds(v))),
        (View::Scalar(x), w) => out.resize(out.len() + len, w.holds(x)),
    }
}

fn f1_vec(f: F1, a: NS<'_>, len: usize, out: &mut Vec<f64>) {
    macro_rules! go {
        ($($v:ident),*) => { match f { $(F1::$v => map1(a, len, out, |x| F1::$v.apply(x)),)* } };
    }
    go!(Abs, Floor, Ceil, Trunc, Sqrt, Cbrt, Exp, Log, Log10, Log2, Sin, Cos, Tan, Asin, Acos, Atan, Sign, Round, Rand, Randn)
}

fn f2_vec(f: F2, a: NS<'_>, b: NS<'_>, len: usize, out: &mut Vec<f64>) {
    macro_rules! go {
        ($($v:ident),*) => { match f { $(F2::$v => map2(a, b, len, out, |x, y| F2::$v.apply(x, y)),)* } };
    }
    go!(Pow, Atan2, RoundTo, Rand, Randn)
}

/// Numeric binary operators, comparisons and equality over lanes. Returns `false` when the operand
/// kinds need the generic per-row path (strings mixed with other things, `Value` lanes).
fn bin_vec(op: Bin, a: View<'_>, b: View<'_>, len: usize, out: &mut Reg) -> bool {
    match op {
        Bin::Add if !(numeric_kind(&a) && numeric_kind(&b)) => false,
        Bin::Add | Bin::Sub | Bin::Mul | Bin::Div | Bin::Rem | Bin::Pow => {
            let (na, nb) = (nums(a), nums(b));
            let (x, y) = (na.ns(), nb.ns());
            let mut o = take_num(out);
            match op {
                Bin::Add => map2(x, y, len, &mut o, |x, y| x + y),
                Bin::Sub => map2(x, y, len, &mut o, |x, y| x - y),
                Bin::Mul => map2(x, y, len, &mut o, |x, y| x * y),
                Bin::Div => map2(x, y, len, &mut o, |x, y| x / y),
                Bin::Rem => map2(x, y, len, &mut o, value::js_rem),
                _ => map2(x, y, len, &mut o, value::js_pow),
            }
            *out = Reg::Num(o);
            true
        }
        Bin::Lt | Bin::Le | Bin::Gt | Bin::Ge => {
            let c = cmp_of(op);
            if numeric_kind(&a) && numeric_kind(&b) {
                let (na, nb) = (nums(a), nums(b));
                let (x, y) = (na.ns(), nb.ns());
                let mut o = take_bool(out);
                match c {
                    Cmp::Lt => cmp2(x, y, len, &mut o, |x, y| x < y),
                    Cmp::Le => cmp2(x, y, len, &mut o, |x, y| x <= y),
                    Cmp::Gt => cmp2(x, y, len, &mut o, |x, y| x > y),
                    Cmp::Ge => cmp2(x, y, len, &mut o, |x, y| x >= y),
                }
                *out = Reg::Bool(o);
                true
            } else if let (Some(sa), Some(sb)) = (strs(a), strs(b)) {
                let mut o = take_bool(out);
                o.extend((0..len).map(|i| match (str_at(sa, i), str_at(sb, i)) {
                    (Some(x), Some(y)) => value::cmp_str(c, x, y),
                    _ => false,
                }));
                *out = Reg::Bool(o);
                true
            } else {
                false
            }
        }
        Bin::Eq | Bin::Ne => {
            let ne = op == Bin::Ne;
            let mut o = take_bool(out);
            if num_eq_kind(&a) && num_eq_kind(&b) {
                let (na, nb) = (nums(a), nums(b));
                cmp2(na.ns(), nb.ns(), len, &mut o, |x, y| (x == y || (x.is_nan() && y.is_nan())) != ne);
            } else if bool_kind(&a) && bool_kind(&b) {
                o.extend((0..len).map(|i| (bool_at(a, i) == bool_at(b, i)) != ne));
            } else if let (Some(sa), Some(sb)) = (strs(a), strs(b)) {
                o.extend((0..len).map(|i| {
                    let eq = match (str_at(sa, i), str_at(sb, i)) {
                        (Some(x), Some(y)) => x == y,
                        (None, None) => true,
                        _ => false,
                    };
                    eq != ne
                }));
            } else {
                *out = Reg::Bool(o);
                return false;
            }
            *out = Reg::Bool(o);
            true
        }
    }
}

/// `[items].includes(x)` / `.indexOf(x)` when every item is runtime-constant and `x` is a string
/// or number lane (the categorical-filter case): compares in place, no `Value` per row. Same
/// semantics as `value::equals`: null-ish items match null-ish rows only.
fn in_list_vec(x: View<'_>, items: &[View<'_>], index_of: bool, len: usize, out: &mut Reg) -> bool {
    let mut consts = Vec::with_capacity(items.len());
    for it in items {
        match it {
            View::Scalar(v) => consts.push(*v),
            _ => return false,
        }
    }
    let pos_str = |row: Option<&str>| {
        consts.iter().position(|k| match (row, k) {
            (Some(r), Value::Str(s)) => r == &**s,
            (None, k) => k.is_null(),
            _ => false,
        })
    };
    let pos_num = |r: f64| {
        consts.iter().position(|k| match k {
            Value::Num(y) if !y.is_nan() => r == *y,
            k => r.is_nan() && k.is_null(),
        })
    };
    let finish = |pos: &mut dyn Iterator<Item = Option<usize>>, out: &mut Reg| {
        if index_of {
            let mut o = take_num(out);
            o.extend(pos.map(|p| p.map_or(-1.0, |p| p as f64)));
            *out = Reg::Num(o);
        } else {
            let mut o = take_bool(out);
            o.extend(pos.map(|p| p.is_some()));
            *out = Reg::Bool(o);
        }
    };
    match x {
        View::Str(s) => finish(&mut s.iter().take(len).map(|r| pos_str(r.as_deref())), out),
        View::Num(s) => finish(&mut s.iter().take(len).map(|&r| pos_num(r)), out),
        _ => return false,
    }
    true
}

/// Per-row fallback through the scalar semantics.
fn generic(op: &Op, ctx: &Ctx<'_>, regs: &[Reg], out: &mut Reg) {
    let mut o = take_val(out);
    for i in 0..ctx.len {
        o.push(eval_op(op, &|x| get(view(ctx, regs, x), i)));
    }
    *out = Reg::Val(o);
}

fn select(ctx: &Ctx<'_>, regs: &[Reg], sel: (Opnd, When, Opnd, Opnd), out: &mut Reg) {
    let (test, when, then, other) = sel;
    let tv = view(ctx, regs, test);
    if let View::Scalar(v) = tv {
        *out = Reg::Alias(if when.holds(v) { then } else { other });
        return;
    }
    let len = ctx.len;
    let mut m = Vec::with_capacity(len);
    mask(tv, when, len, &mut m);
    let (a, b) = (view(ctx, regs, then), view(ctx, regs, other));
    if num_kind(&a) && num_kind(&b) {
        let (na, nb) = (nums(a), nums(b));
        let mut o = take_num(out);
        match (na.ns(), nb.ns()) {
            (NS::V(x), NS::V(y)) => o.extend(m.iter().zip(x).zip(y).map(|((&m, &x), &y)| if m { x } else { y })),
            (NS::V(x), NS::S(y)) => o.extend(m.iter().zip(x).map(|(&m, &x)| if m { x } else { y })),
            (NS::S(x), NS::V(y)) => o.extend(m.iter().zip(y).map(|(&m, &y)| if m { x } else { y })),
            (NS::S(x), NS::S(y)) => o.extend(m.iter().map(|&m| if m { x } else { y })),
        }
        *out = Reg::Num(o);
    } else if let (Some(x), Some(y)) = (bools(a), bools(b)) {
        let mut o = take_bool(out);
        match (x, y) {
            (BS::V(x), BS::V(y)) => o.extend(m.iter().zip(x).zip(y).map(|((&m, &x), &y)| if m { x } else { y })),
            (BS::V(x), BS::S(y)) => o.extend(m.iter().zip(x).map(|(&m, &x)| if m { x } else { y })),
            (BS::S(x), BS::V(y)) => o.extend(m.iter().zip(y).map(|(&m, &y)| if m { x } else { y })),
            (BS::S(x), BS::S(y)) => o.extend(m.iter().map(|&m| if m { x } else { y })),
        }
        *out = Reg::Bool(o);
    } else {
        let mut o = take_val(out);
        o.extend(m.iter().enumerate().map(|(i, &m)| get(if m { a } else { b }, i)));
        *out = Reg::Val(o);
    }
}

fn host(ctx: &Ctx<'_>, regs: &[Reg], env: &dyn Env, name: u32, args: &[Opnd], out: &mut Reg) {
    let name = ctx.hosts.get(name as usize).map_or("", String::as_str);
    let views: Vec<View<'_>> = args.iter().map(|&a| view(ctx, regs, a)).collect();
    let mut argv: Vec<Value> = Vec::with_capacity(args.len());
    if views.iter().all(|v| matches!(v, View::Scalar(_))) {
        argv.extend(views.iter().map(|v| get(*v, 0)));
        *out = Reg::Scalar(env.call(name, &argv).unwrap_or(Value::Null));
        return;
    }
    let mut o = take_val(out);
    for i in 0..ctx.len {
        argv.clear();
        argv.extend(views.iter().map(|v| get(*v, i)));
        o.push(env.call(name, &argv).unwrap_or(Value::Null));
    }
    *out = Reg::Val(o);
}

fn exec(op: &Op, ctx: &Ctx<'_>, regs: &[Reg], env: &dyn Env, out: &mut Reg) {
    let len = ctx.len;
    let v = |o: Opnd| view(ctx, regs, o);
    match op {
        Op::Select { test, when, then, other, .. } => return select(ctx, regs, (*test, *when, *then, *other), out),
        Op::Host { name, args, .. } => return host(ctx, regs, env, *name, args, out),
        Op::Jump { .. } => return,
        _ => {}
    }
    let mut all_scalar = true;
    op.operands(&mut |o| all_scalar &= matches!(v(o), View::Scalar(_)));
    if all_scalar {
        let get = |o: Opnd| match v(o) {
            View::Scalar(x) => x.clone(),
            _ => Value::Null,
        };
        *out = Reg::Scalar(eval_op(op, &get));
        return;
    }
    let done = match op {
        Op::Un { op: UnOp::Not, a, .. } => {
            let mut o = take_bool(out);
            mask(v(*a), When::Falsy, len, &mut o);
            *out = Reg::Bool(o);
            true
        }
        Op::Un { op: u, a, .. } => {
            let na = nums(v(*a));
            let mut o = take_num(out);
            if *u == UnOp::Neg {
                map1(na.ns(), len, &mut o, |x| -x);
            } else {
                map1(na.ns(), len, &mut o, |x| x);
            }
            *out = Reg::Num(o);
            true
        }
        Op::Bin { op: b, a, b: bb, .. } => bin_vec(*b, v(*a), v(*bb), len, out),
        Op::F1 { f, a, .. } => {
            let na = nums(v(*a));
            let mut o = take_num(out);
            f1_vec(*f, na.ns(), len, &mut o);
            *out = Reg::Num(o);
            true
        }
        Op::F2 { f, a, b, .. } => {
            let (na, nb) = (nums(v(*a)), nums(v(*b)));
            let mut o = take_num(out);
            f2_vec(*f, na.ns(), nb.ns(), len, &mut o);
            *out = Reg::Num(o);
            true
        }
        Op::F3 { f, a, b, c, .. } => {
            let (na, nb, nc) = (nums(v(*a)), nums(v(*b)), nums(v(*c)));
            let (x, y, z) = (na.ns(), nb.ns(), nc.ns());
            let mut o = take_num(out);
            o.extend((0..len).map(|i| f.apply(x.at(i), y.at(i), z.at(i))));
            *out = Reg::Num(o);
            true
        }
        Op::FN { f, args, .. } => {
            let mut o = take_num(out);
            o.resize(len, f.identity());
            for &arg in args {
                match nums(v(arg)).ns() {
                    NS::V(y) => o.iter_mut().zip(y).for_each(|(acc, &y)| *acc = f.apply(*acc, y)),
                    NS::S(y) => o.iter_mut().for_each(|acc| *acc = f.apply(*acc, y)),
                }
            }
            *out = Reg::Num(o);
            true
        }
        Op::InList { x, items, index_of, .. } => {
            let views: Vec<View<'_>> = items.iter().map(|o| v(*o)).collect();
            in_list_vec(v(*x), &views, *index_of, len, out)
        }
        Op::Conv { f: Conv::Boolean, a, .. } => {
            let mut o = take_bool(out);
            mask(v(*a), When::Truthy, len, &mut o);
            *out = Reg::Bool(o);
            true
        }
        Op::Conv { f, a, .. } => match (f, v(*a)) {
            (Conv::Number, View::Num(s)) => {
                let mut o = take_num(out);
                o.extend_from_slice(s);
                *out = Reg::Num(o);
                true
            }
            (Conv::IsNaN, View::Num(s)) => {
                let mut o = take_bool(out);
                o.extend(s.iter().map(|x| x.is_nan()));
                *out = Reg::Bool(o);
                true
            }
            (Conv::IsFinite, View::Num(s)) => {
                let mut o = take_bool(out);
                o.extend(s.iter().map(|x| x.is_finite()));
                *out = Reg::Bool(o);
                true
            }
            _ => false,
        },
        _ => false,
    };
    if !done {
        generic(op, ctx, regs, out);
    }
}

fn run_chunk(c: &Compiled, ctx: &Ctx<'_>, regs: &mut [Reg], env: &dyn Env) {
    let mut pc = 0;
    while pc < c.code.len() {
        let op = &c.code[pc];
        if let Op::Jump { test, when, target } = op {
            if let View::Scalar(v) = view(ctx, regs, *test) {
                if when.holds(v) {
                    pc = *target as usize;
                    continue;
                }
            }
            pc += 1;
            continue;
        }
        if let Some(dst) = op.dst().map(|d| d as usize).filter(|&d| d < regs.len()) {
            // Scalars and aliases depend only on runtime constants: computed in the first chunk.
            if !matches!(regs[dst], Reg::Scalar(_) | Reg::Alias(_)) {
                let mut out = std::mem::replace(&mut regs[dst], Reg::Unset);
                exec(op, ctx, regs, env, &mut out);
                regs[dst] = out;
            }
        }
        pc += 1;
    }
}

/// Runs the program over rows `0..n`, handing each chunk's result to `sink`.
fn run(c: &Compiled, n: usize, env: &dyn Env, mut sink: impl FnMut(View<'_>, usize)) {
    let mut arena = Vec::new();
    let bound = bind(c, env, n, &mut arena);
    let mut regs: Vec<Reg> = (0..c.nregs).map(|_| Reg::Unset).collect();
    let mut rows: Vec<f64> = Vec::new();
    let mut start = 0;
    while start < n {
        let len = CHUNK.min(n - start);
        if c.uses_row {
            rows.clear();
            rows.extend((start..start + len).map(|i| i as f64));
        }
        let ctx = Ctx { bound: &bound, consts: &c.consts, hosts: &c.hosts, rows: &rows, start, len };
        run_chunk(c, &ctx, &mut regs, env);
        sink(view(&ctx, &regs, c.result), len);
        start += len;
    }
}

pub(crate) fn eval_rows(c: &Compiled, n: usize, env: &dyn Env) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::with_capacity(n);
    run(c, n, env, |v, len| match v {
        View::Scalar(x) => out.extend(std::iter::repeat_n(canon_value(x.clone()), len)),
        View::Num(s) => out.extend(s.iter().map(|&x| Value::Num(canon(x)))),
        View::Bool(s) => out.extend(s.iter().map(|&b| Value::Bool(b))),
        View::Str(s) => out.extend(s.iter().map(|o| o.clone().map_or(Value::Null, Value::Str))),
        View::Val(s) => out.extend(s.iter().map(|v| canon_value(v.clone()))),
    });
    out
}

pub(crate) fn eval_rows_num(c: &Compiled, n: usize, env: &dyn Env) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::with_capacity(n);
    run(c, n, env, |v, len| match v {
        View::Scalar(x) => out.resize(out.len() + len, canon(x.to_num())),
        View::Num(s) => out.extend(s.iter().map(|&x| canon(x))),
        View::Bool(s) => out.extend(s.iter().map(|&b| if b { 1.0 } else { 0.0 })),
        View::Str(s) => out.resize(out.len() + s.len(), f64::NAN),
        View::Val(s) => out.extend(s.iter().map(|v| canon(v.to_num()))),
    });
    out
}
