//! Lowering an [`Expr`] to register bytecode.
//!
//! The bytecode is in SSA form: every instruction writes a fresh register, so an instruction never
//! reads the register it writes and the vector evaluator can reuse each register's buffer across
//! row chunks. Operands are registers, constants, inputs (columns / signals, resolved once per
//! evaluation) or the row index.
//!
//! Constant folding happens during emission: an operation whose operands are all constants is
//! evaluated right away with the same scalar semantics the evaluators use, and a conditional with
//! a constant test emits only the branch taken. Host calls are never folded (they need the env).
//!
//! Conditionals (`?:`, `&&`, `||`, `??`) compile to `Jump`s around the branches followed by a
//! `Select`. A jump is taken only when its test is *runtime-constant* (same for every row: a signal,
//! a constant); then only the live branch is evaluated. When the test varies per row, both branches
//! are evaluated over the chunk and `Select` picks per row — safe because everything is pure and
//! total.

use crate::ast::{BinOp, Expr, Span, UnOp, MAX_DEPTH};
use crate::builtins::{self, Builtin, Conv, Method, F1, F2, F3, FN};
use crate::env::Env;
use crate::error::CompileError;
use crate::parser::arity_message;
use crate::value::Value;
use crate::vm;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Opnd {
    Reg(u32),
    Const(u32),
    Input(u32),
    Row,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum When {
    Truthy,
    Falsy,
    NotNull,
}

impl When {
    #[inline]
    pub(crate) fn holds(self, v: &Value) -> bool {
        match self {
            When::Truthy => v.truthy(),
            When::Falsy => !v.truthy(),
            When::NotNull => !v.is_null(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Bin {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

#[derive(Clone, Debug)]
pub(crate) enum Op {
    Un { dst: u32, op: UnOp, a: Opnd },
    Bin { dst: u32, op: Bin, a: Opnd, b: Opnd },
    F1 { dst: u32, f: F1, a: Opnd },
    F2 { dst: u32, f: F2, a: Opnd, b: Opnd },
    F3 { dst: u32, f: F3, a: Opnd, b: Opnd, c: Opnd },
    FN { dst: u32, f: FN, args: Vec<Opnd> },
    Conv { dst: u32, f: Conv, a: Opnd },
    Method { dst: u32, m: Method, recv: Opnd, args: Vec<Opnd> },
    Index { dst: u32, obj: Opnd, index: Opnd },
    /// `[items].includes(x)` / `.indexOf(x)` on an array literal.
    InList { dst: u32, x: Opnd, items: Vec<Opnd>, index_of: bool },
    /// `[items][index]` on an array literal.
    Pick { dst: u32, index: Opnd, items: Vec<Opnd> },
    Concat { dst: u32, parts: Vec<Opnd> },
    Host { dst: u32, name: u32, args: Vec<Opnd> },
    /// Jump to `target` if `test` is runtime-constant and `when` holds for it.
    Jump { test: Opnd, when: When, target: u32 },
    /// `when(test) ? then : other`, per row.
    Select { dst: u32, test: Opnd, when: When, then: Opnd, other: Opnd },
}

impl Op {
    pub(crate) fn dst(&self) -> Option<u32> {
        match self {
            Op::Un { dst, .. }
            | Op::Bin { dst, .. }
            | Op::F1 { dst, .. }
            | Op::F2 { dst, .. }
            | Op::F3 { dst, .. }
            | Op::FN { dst, .. }
            | Op::Conv { dst, .. }
            | Op::Method { dst, .. }
            | Op::Index { dst, .. }
            | Op::InList { dst, .. }
            | Op::Pick { dst, .. }
            | Op::Concat { dst, .. }
            | Op::Host { dst, .. }
            | Op::Select { dst, .. } => Some(*dst),
            Op::Jump { .. } => None,
        }
    }

    pub(crate) fn operands(&self, f: &mut dyn FnMut(Opnd)) {
        match self {
            Op::Un { a, .. } | Op::F1 { a, .. } | Op::Conv { a, .. } => f(*a),
            Op::Bin { a, b, .. } | Op::F2 { a, b, .. } => {
                f(*a);
                f(*b);
            }
            Op::F3 { a, b, c, .. } => {
                f(*a);
                f(*b);
                f(*c);
            }
            Op::FN { args, .. } | Op::Concat { parts: args, .. } | Op::Host { args, .. } => args.iter().for_each(|o| f(*o)),
            Op::Method { recv, args, .. } => {
                f(*recv);
                args.iter().for_each(|o| f(*o));
            }
            Op::Index { obj, index, .. } => {
                f(*obj);
                f(*index);
            }
            Op::InList { x, items, .. } => {
                f(*x);
                items.iter().for_each(|o| f(*o));
            }
            Op::Pick { index, items, .. } => {
                f(*index);
                items.iter().for_each(|o| f(*o));
            }
            Op::Jump { test, .. } => f(*test),
            Op::Select { test, then, other, .. } => {
                f(*test);
                f(*then);
                f(*other);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum InputKind {
    /// `d.x`: a column (null if missing).
    Field,
    /// A bare name outside a lambda: the column if the env has one, else the signal.
    Ident,
    /// A free name inside a lambda: a signal.
    Signal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Input {
    pub kind: InputKind,
    pub name: String,
}

/// A compiled expression: register bytecode plus its constant pool and inputs. Cheap to clone,
/// `Send + Sync`, evaluable any number of times against any [`Env`].
#[derive(Clone, Debug)]
pub struct Compiled {
    pub(crate) code: Vec<Op>,
    pub(crate) consts: Vec<Value>,
    pub(crate) inputs: Vec<Input>,
    pub(crate) hosts: Vec<String>,
    pub(crate) nregs: usize,
    pub(crate) result: Opnd,
    pub(crate) uses_row: bool,
}

impl Compiled {
    /// Evaluates for rows `0..n`, vectorized: each instruction runs over a chunk of rows at a time.
    pub fn eval_rows(&self, n: usize, env: &dyn Env) -> Vec<Value> {
        vm::eval_rows(self, n, env)
    }

    /// Like [`Compiled::eval_rows`] but numeric: non-numbers become NaN (booleans 0/1). The fast
    /// path for numeric channels; no `Value` is built for numeric results.
    pub fn eval_rows_num(&self, n: usize, env: &dyn Env) -> Vec<f64> {
        vm::eval_rows_num(self, n, env)
    }

    /// Evaluates once, as for row 0 (per-frame expressions normally read no columns; if they do,
    /// they see the first row, or null for an empty column).
    pub fn eval_scalar(&self, env: &dyn Env) -> Value {
        vm::eval_row(self, 0, env)
    }

    /// Evaluates one row with the scalar interpreter (the reference the vector path must match).
    pub fn eval_row(&self, row: usize, env: &dyn Env) -> Value {
        vm::eval_row(self, row, env)
    }

    /// Columns the expression may read (dependency tracking), sorted. Bare names outside a lambda
    /// appear in both `fields()` and `signals()`: the env decides at evaluation time.
    pub fn fields(&self) -> Vec<String> {
        self.names(|k| matches!(k, InputKind::Field | InputKind::Ident))
    }

    /// Signals the expression may read, sorted.
    pub fn signals(&self) -> Vec<String> {
        self.names(|k| matches!(k, InputKind::Signal | InputKind::Ident))
    }

    /// Host functions the expression may call, sorted.
    pub fn calls(&self) -> Vec<String> {
        let mut v = self.hosts.clone();
        v.sort();
        v
    }

    /// Whether the result depends on the row index.
    pub fn uses_row(&self) -> bool {
        self.uses_row
    }

    /// Whether the value is the same for every row of every table, given that `signal` names the
    /// env's signals: no row index, no host call, no column (`d.x`), and every name a signal (a
    /// bare name the env doesn't have as a signal may read a column).
    pub fn row_free(&self, signal: &dyn Fn(&str) -> bool) -> bool {
        !self.uses_row && self.hosts.is_empty() && self.inputs.iter().all(|i| i.kind != InputKind::Field && signal(&i.name))
    }

    /// The value, if constant folding reduced the whole expression to a constant.
    pub fn constant(&self) -> Option<&Value> {
        match self.result {
            Opnd::Const(i) => self.consts.get(i as usize),
            _ => None,
        }
    }

    /// Number of instructions (after folding).
    pub fn len(&self) -> usize {
        self.code.len()
    }

    pub fn is_empty(&self) -> bool {
        self.code.is_empty()
    }

    fn names(&self, keep: impl Fn(InputKind) -> bool) -> Vec<String> {
        let mut v: Vec<String> = self.inputs.iter().filter(|i| keep(i.kind)).map(|i| i.name.clone()).collect();
        v.sort();
        v.dedup();
        v
    }
}

/// Disassembly, for devtools and tests.
impl fmt::Display for Compiled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let opnd = |o: &Opnd| match *o {
            Opnd::Reg(r) => format!("r{r}"),
            Opnd::Const(c) => format!("{:?}", self.consts.get(c as usize).unwrap_or(&Value::Null)),
            Opnd::Input(i) => self.inputs.get(i as usize).map_or("?".into(), |i| format!("{:?}({})", i.kind, i.name)),
            Opnd::Row => "row".into(),
        };
        for (pc, op) in self.code.iter().enumerate() {
            let mut ops = Vec::new();
            op.operands(&mut |o| ops.push(opnd(&o)));
            let name = mnemonic(op);
            match op {
                Op::Jump { when, target, .. } => writeln!(f, "{pc:3}  jump if {when:?} {} -> {target}", ops.join(", "))?,
                Op::Host { name: n, .. } => writeln!(
                    f,
                    "{pc:3}  r{} = host {}({})",
                    op.dst().unwrap_or(0),
                    self.hosts.get(*n as usize).map_or("?", |s| s.as_str()),
                    ops.join(", ")
                )?,
                _ => writeln!(f, "{pc:3}  r{} = {name} {}", op.dst().unwrap_or(0), ops.join(", "))?,
            }
        }
        writeln!(f, "     result {}", opnd(&self.result))
    }
}

fn mnemonic(op: &Op) -> String {
    let lower = |d: &dyn fmt::Debug| format!("{d:?}").to_lowercase();
    match op {
        Op::Un { op, .. } => lower(op),
        Op::Bin { op, .. } => lower(op),
        Op::F1 { f, .. } => lower(f),
        Op::F2 { f, .. } => lower(f),
        Op::F3 { f, .. } => lower(f),
        Op::FN { f, .. } => lower(f),
        Op::Conv { f, .. } => lower(f),
        Op::Method { m, .. } => format!(".{}", lower(m)),
        Op::Index { .. } => "index".into(),
        Op::InList { index_of: true, .. } => "index_of".into(),
        Op::InList { .. } => "in_list".into(),
        Op::Pick { .. } => "pick".into(),
        Op::Concat { .. } => "concat".into(),
        Op::Host { .. } => "host".into(),
        Op::Jump { .. } => "jump".into(),
        Op::Select { when, .. } => format!("select_if_{}", lower(when)),
    }
}

/// Compiles an expression. Fails only for trees the parser wouldn't produce (wrong arity, unknown
/// method, an array literal in value position, excessive nesting).
pub fn compile(e: &Expr) -> Result<Compiled, CompileError> {
    if e.depth() > MAX_DEPTH {
        return cerr(e.span(), format!("expression is nested too deeply (more than {MAX_DEPTH} levels)"));
    }
    let mut em = Emitter::default();
    let v = em.emit(e)?;
    let result = em.opnd(v);
    Ok(Compiled {
        code: em.code,
        consts: em.consts,
        inputs: em.inputs,
        hosts: em.hosts,
        nregs: em.nregs as usize,
        result,
        uses_row: em.uses_row,
    })
}

/// An emitted value: known at compile time, or an operand.
enum V {
    K(Value),
    O(Opnd),
}

#[derive(Default)]
struct Emitter {
    code: Vec<Op>,
    consts: Vec<Value>,
    inputs: Vec<Input>,
    hosts: Vec<String>,
    nregs: u32,
    uses_row: bool,
}

fn cerr<T>(span: Span, msg: impl Into<String>) -> Result<T, CompileError> {
    Err(CompileError::new(span, msg))
}

impl Emitter {
    fn konst(&mut self, v: Value) -> Opnd {
        let i = match self.consts.iter().position(|c| c.identical(&v)) {
            Some(i) => i,
            None => {
                self.consts.push(v);
                self.consts.len() - 1
            }
        };
        Opnd::Const(i as u32)
    }

    fn opnd(&mut self, v: V) -> Opnd {
        match v {
            V::K(x) => self.konst(x),
            V::O(o) => o,
        }
    }

    fn reg(&mut self) -> u32 {
        self.nregs += 1;
        self.nregs - 1
    }

    fn input(&mut self, kind: InputKind, name: &str) -> V {
        let i = match self.inputs.iter().position(|i| i.kind == kind && i.name == name) {
            Some(i) => i,
            None => {
                self.inputs.push(Input { kind, name: name.to_string() });
                self.inputs.len() - 1
            }
        };
        V::O(Opnd::Input(i as u32))
    }

    /// Folds when every operand is a constant, else emits `make(dst, operands)`.
    fn op(&mut self, vals: Vec<V>, make: impl FnOnce(u32, Vec<Opnd>) -> Op) -> V {
        if vals.iter().all(|v| matches!(v, V::K(_))) {
            let ks: Vec<Value> = vals.into_iter().map(|v| if let V::K(k) = v { k } else { Value::Null }).collect();
            let op = make(0, (0..ks.len() as u32).map(Opnd::Const).collect());
            let get = |o: Opnd| match o {
                Opnd::Const(i) => ks.get(i as usize).cloned().unwrap_or(Value::Null),
                _ => Value::Null,
            };
            return V::K(vm::eval_op(&op, &get));
        }
        let opnds: Vec<Opnd> = vals.into_iter().map(|v| self.opnd(v)).collect();
        let dst = self.reg();
        self.code.push(make(dst, opnds));
        V::O(Opnd::Reg(dst))
    }

    fn emit_all(&mut self, es: &[Expr]) -> Result<Vec<V>, CompileError> {
        es.iter().map(|e| self.emit(e)).collect()
    }

    fn emit(&mut self, e: &Expr) -> Result<V, CompileError> {
        Ok(match e {
            Expr::Null { .. } => V::K(Value::Null),
            Expr::Bool { value, .. } => V::K(Value::Bool(*value)),
            Expr::Num { value, .. } => V::K(Value::Num(*value)),
            Expr::Str { value, .. } => V::K(Value::str(value)),
            Expr::Array { span, .. } => {
                return cerr(*span, "an array literal can only be used with .includes(), .indexOf(), .length or [index]")
            }
            Expr::Field { name, .. } => self.input(InputKind::Field, name),
            Expr::Ident { name, .. } => self.input(InputKind::Ident, name),
            Expr::Signal { name, .. } => self.input(InputKind::Signal, name),
            Expr::Row { .. } => {
                self.uses_row = true;
                V::O(Opnd::Row)
            }
            Expr::Unary { op, arg, .. } => {
                let a = self.emit(arg)?;
                let op = *op;
                self.op(vec![a], |dst, o| Op::Un { dst, op, a: o[0] })
            }
            Expr::Binary { op, lhs, rhs, .. } if op.is_logical() => self.logical(*op, lhs, rhs)?,
            Expr::Binary { op, lhs, rhs, .. } => {
                let a = self.emit(lhs)?;
                let b = self.emit(rhs)?;
                let op = match op {
                    BinOp::Add => Bin::Add,
                    BinOp::Sub => Bin::Sub,
                    BinOp::Mul => Bin::Mul,
                    BinOp::Div => Bin::Div,
                    BinOp::Rem => Bin::Rem,
                    BinOp::Pow => Bin::Pow,
                    BinOp::Lt => Bin::Lt,
                    BinOp::Le => Bin::Le,
                    BinOp::Gt => Bin::Gt,
                    BinOp::Ge => Bin::Ge,
                    BinOp::Eq | BinOp::StrictEq => Bin::Eq,
                    BinOp::Ne | BinOp::StrictNe => Bin::Ne,
                    BinOp::And | BinOp::Or | BinOp::Coalesce => Bin::Eq, // handled above
                };
                self.op(vec![a, b], |dst, o| Op::Bin { dst, op, a: o[0], b: o[1] })
            }
            Expr::Cond { test, then, otherwise, .. } => self.cond(test, then, otherwise)?,
            Expr::Call { name, args, span } => self.call(name, args, *span)?,
            Expr::Method { recv, name, args, span } => self.method(recv, name, args, *span)?,
            Expr::Index { obj, index, .. } => {
                if let Expr::Array { items, .. } = &**obj {
                    let mut vals = vec![self.emit(index)?];
                    vals.extend(self.emit_all(items)?);
                    self.op(vals, |dst, o| Op::Pick { dst, index: o[0], items: o[1..].to_vec() })
                } else {
                    let o = self.emit(obj)?;
                    let i = self.emit(index)?;
                    self.op(vec![o, i], |dst, o| Op::Index { dst, obj: o[0], index: o[1] })
                }
            }
            Expr::Concat { parts, .. } => {
                // Merge adjacent constant parts: concatenation only sees each part's String(…).
                let mut vals: Vec<V> = Vec::new();
                for p in parts {
                    let v = self.emit(p)?;
                    if let (V::K(k), Some(V::K(prev))) = (&v, vals.last_mut()) {
                        let mut s = String::from(&*prev.to_str());
                        s.push_str(&k.to_str());
                        *prev = Value::from(s);
                        continue;
                    }
                    vals.push(v);
                }
                match vals.len() {
                    0 => V::K(Value::str("")),
                    _ => self.op(vals, |dst, o| Op::Concat { dst, parts: o }),
                }
            }
        })
    }

    fn cond(&mut self, test: &Expr, then: &Expr, otherwise: &Expr) -> Result<V, CompileError> {
        let t = match self.emit(test)? {
            V::K(k) => return if k.truthy() { self.emit(then) } else { self.emit(otherwise) },
            V::O(o) => o,
        };
        let j1 = self.jump(t, When::Falsy);
        let a = self.emit(then)?;
        let a = self.opnd(a);
        let j2 = self.jump(t, When::Truthy);
        self.patch(j1);
        let b = self.emit(otherwise)?;
        let b = self.opnd(b);
        self.patch(j2);
        let dst = self.reg();
        self.code.push(Op::Select { dst, test: t, when: When::Truthy, then: a, other: b });
        Ok(V::O(Opnd::Reg(dst)))
    }

    /// `&&`, `||`, `??` return one of their operands (JS semantics), and skip the right operand when
    /// the left decides.
    fn logical(&mut self, op: BinOp, lhs: &Expr, rhs: &Expr) -> Result<V, CompileError> {
        let a = match self.emit(lhs)? {
            V::K(k) => {
                let take_rhs = match op {
                    BinOp::And => k.truthy(),
                    BinOp::Or => !k.truthy(),
                    _ => k.is_null(),
                };
                return if take_rhs { self.emit(rhs) } else { Ok(V::K(k)) };
            }
            V::O(o) => o,
        };
        let skip = match op {
            BinOp::And => When::Falsy,
            BinOp::Or => When::Truthy,
            _ => When::NotNull,
        };
        let j = self.jump(a, skip);
        let b = self.emit(rhs)?;
        let b = self.opnd(b);
        self.patch(j);
        let dst = self.reg();
        let sel = match op {
            BinOp::And => Op::Select { dst, test: a, when: When::Truthy, then: b, other: a },
            BinOp::Or => Op::Select { dst, test: a, when: When::Truthy, then: a, other: b },
            _ => Op::Select { dst, test: a, when: When::NotNull, then: a, other: b },
        };
        self.code.push(sel);
        Ok(V::O(Opnd::Reg(dst)))
    }

    fn jump(&mut self, test: Opnd, when: When) -> usize {
        self.code.push(Op::Jump { test, when, target: 0 });
        self.code.len() - 1
    }

    fn patch(&mut self, at: usize) {
        let here = self.code.len() as u32;
        if let Some(Op::Jump { target, .. }) = self.code.get_mut(at) {
            *target = here;
        }
    }

    fn call(&mut self, name: &str, args: &[Expr], span: Span) -> Result<V, CompileError> {
        let Some((lo, hi)) = builtins::arity(name) else {
            let vals = self.emit_all(args)?;
            let opnds: Vec<Opnd> = vals.into_iter().map(|v| self.opnd(v)).collect();
            let idx = match self.hosts.iter().position(|h| h == name) {
                Some(i) => i,
                None => {
                    self.hosts.push(name.to_string());
                    self.hosts.len() - 1
                }
            };
            let dst = self.reg();
            self.code.push(Op::Host { dst, name: idx as u32, args: opnds });
            return Ok(V::O(Opnd::Reg(dst)));
        };
        if args.len() < lo || args.len() > hi {
            return cerr(span, arity_message(&format!("{name}()"), lo, hi, args.len()));
        }
        let Some(b) = builtins::resolve(name, args.len()) else {
            return cerr(span, format!("unknown built-in `{name}`"));
        };
        let vals = self.emit_all(args)?;
        Ok(match b {
            Builtin::F1(f) => self.op(vals, |dst, o| Op::F1 { dst, f, a: o[0] }),
            Builtin::F2(f) => self.op(vals, |dst, o| Op::F2 { dst, f, a: o[0], b: o[1] }),
            Builtin::F3(f) => self.op(vals, |dst, o| Op::F3 { dst, f, a: o[0], b: o[1], c: o[2] }),
            Builtin::FN(f) => self.op(vals, |dst, o| Op::FN { dst, f, args: o }),
            Builtin::Conv(f) => self.op(vals, |dst, o| Op::Conv { dst, f, a: o[0] }),
        })
    }

    fn method(&mut self, recv: &Expr, name: &str, args: &[Expr], span: Span) -> Result<V, CompileError> {
        let Some(m) = Method::from_name(name) else {
            return cerr(span, format!("unknown method `.{name}`; values support {}", Method::list()));
        };
        let (lo, hi) = m.arity();
        if args.len() < lo || args.len() > hi {
            return cerr(span, arity_message(&format!(".{name}()"), lo, hi, args.len()));
        }
        if let Expr::Array { items, .. } = recv {
            return match m {
                Method::Length => Ok(V::K(Value::Num(items.len() as f64))),
                Method::Includes | Method::IndexOf => {
                    let mut vals = vec![self.emit(&args[0])?];
                    vals.extend(self.emit_all(items)?);
                    let index_of = m == Method::IndexOf;
                    Ok(self.op(vals, |dst, o| Op::InList { dst, x: o[0], items: o[1..].to_vec(), index_of }))
                }
                _ => cerr(span, format!("arrays support only .includes(), .indexOf() and .length, not .{name}()")),
            };
        }
        let mut vals = vec![self.emit(recv)?];
        vals.extend(self.emit_all(args)?);
        Ok(self.op(vals, |dst, o| Op::Method { dst, m, recv: o[0], args: o[1..].to_vec() }))
    }
}
