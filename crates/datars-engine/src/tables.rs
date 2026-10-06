//! Tables: sources (inline, CSV, host-provided) and derived tables (a pipeline of ops over another
//! table). Ops map onto datars-data transforms and datars-algo layouts. Tables whose ops read the
//! layout box or scales are computed per context (cached by box + scope fingerprint); the rest are
//! kept across resolves until the data changes ([`SharedTables`]).

use crate::resolve::{Cx, Resolver};
use crate::{Diag, Request};
use datars_data::transform::{self as tf, Agg, AggOp, BinSpec, JoinKind, TextPart, TimeBin, WindowOp};
use datars_data::{Column, CsvOptions, Table};
use datars_ir::{Derived, Doc, Prop, Source, SourceKind};
use datars_scene::{Key, KeyPart};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

/// Derived tables that are pure functions of their inputs — no signal, layout box, scale or host
/// state read by their ops, all the way down the chain — kept across resolves. A scene per story
/// step, layout's measuring passes, the re-resolve for a crowded card and every other source
/// arriving all reuse them: a dot-density table of 400,000 dots is computed once per data it reads,
/// not once per state, pass and download. The engine empties it when the document changes.
pub type SharedTables = Rc<RefCell<BTreeMap<String, SharedTable>>>;

/// A kept table and exactly what it was computed from.
pub struct SharedTable {
    table: Arc<Table>,
    deps: Vec<(String, Dep)>,
}

/// A loaded input, held (not just named) so a table is reused only while each of its inputs is
/// still the very value loaded — a source provided again is a new value.
#[derive(Clone)]
enum Dep {
    Table(Arc<Table>),
    Geo(Arc<crate::geo::GeoSource>),
    /// A declared source that hadn't arrived: current only while it still hasn't.
    Missing,
}

impl SharedTable {
    #[cfg(test)]
    pub(crate) fn table(&self) -> &Arc<Table> {
        &self.table
    }

    fn current(&self, r: &Resolver) -> bool {
        let store = r.tables.borrow();
        self.deps.iter().all(|(name, d)| match d {
            Dep::Table(t) => store.sources.get(name).is_some_and(|now| Arc::ptr_eq(now, t)),
            Dep::Geo(g) => r.geo.get(name).is_some_and(|now| Arc::ptr_eq(now, g)),
            Dep::Missing => !store.sources.contains_key(name) && !r.geo.contains_key(name),
        })
    }
}

/// The loaded inputs a derived table reads: its chain's root source, and every source, geo source
/// or derived table an op names (a join's `with`, a dot map's `geo`), transitively.
fn deps(r: &Resolver, name: &str, out: &mut Vec<(String, Dep)>, seen: &mut std::collections::BTreeSet<String>) {
    if !seen.insert(name.to_string()) {
        return;
    }
    if let Some(t) = r.tables.borrow().sources.get(name) {
        out.push((name.to_string(), Dep::Table(t.clone())));
    }
    if let Some(g) = r.geo.get(name) {
        out.push((name.to_string(), Dep::Geo(g.clone())));
    }
    if r.doc.data.contains_key(name) && !r.tables.borrow().sources.contains_key(name) && !r.geo.contains_key(name) {
        out.push((name.to_string(), Dep::Missing));
    }
    let Some(d) = r.tables.borrow().derived.get(name).cloned() else { return };
    deps(r, &d.from, out, seen);
    for op in &d.ops {
        let mut names = Vec::new();
        strings(op, &mut names);
        for n in names {
            let known = r.geo.contains_key(n) || r.doc.data.contains_key(n) || {
                let s = r.tables.borrow();
                s.sources.contains_key(n) || s.derived.contains_key(n)
            };
            if known {
                deps(r, n, out, seen);
            }
        }
    }
}

fn strings<'a>(v: &'a serde_json::Value, out: &mut Vec<&'a str>) {
    match v {
        serde_json::Value::String(s) => out.push(s),
        serde_json::Value::Array(a) => a.iter().for_each(|x| strings(x, out)),
        serde_json::Value::Object(o) => o.values().for_each(|x| strings(x, out)),
        _ => {}
    }
}

pub struct TableStore {
    /// Loaded sources (original types, dates kept as dates).
    sources: BTreeMap<String, Arc<Table>>,
    derived: BTreeMap<String, Derived>,
    /// Computed originals by cache key.
    originals: BTreeMap<String, Arc<Table>>,
    /// Evaluation views (dates as day numbers) by cache key, shared with every evaluation's
    /// environment (copied only when a table is added, not per expression).
    views: Rc<BTreeMap<String, Arc<Table>>>,
    /// Context-free originals shared with other resolves (see [`SharedTables`]).
    shared: SharedTables,
}

impl TableStore {
    pub fn new(sources: BTreeMap<String, Arc<Table>>, derived: BTreeMap<String, Derived>, shared: SharedTables) -> TableStore {
        let views = Rc::new(sources.iter().map(|(k, t)| (k.clone(), eval_view_arc(t))).collect());
        TableStore { sources, derived, originals: BTreeMap::new(), views, shared }
    }
    pub fn add_derived(&mut self, more: BTreeMap<String, Derived>) {
        for (k, v) in more {
            self.derived.entry(k).or_insert(v);
        }
    }
    /// Evaluation views computed so far (for aggregate functions like `sum("votes", "share")`).
    pub fn snapshot(&self) -> Rc<BTreeMap<String, Arc<Table>>> {
        self.views.clone()
    }
    pub fn snapshot_has(&self, name: &str) -> bool {
        self.views.contains_key(name)
    }
}

/// The evaluation view of a shared table: the same table (not a copy — sources can have millions
/// of rows, and a store is made for every resolve) unless it has dates to turn into numbers.
fn eval_view_arc(t: &Arc<Table>) -> Arc<Table> {
    if t.columns.iter().any(|(_, c)| matches!(c, Column::Date(_))) {
        Arc::new(eval_view(t))
    } else {
        t.clone()
    }
}

/// Dates become day numbers for expressions; everything else is shared.
fn eval_view(t: &Table) -> Table {
    if !t.columns.iter().any(|(_, c)| matches!(c, Column::Date(_))) {
        return t.clone();
    }
    let mut out = t.clone();
    for (_, c) in &mut out.columns {
        if let Column::Date(d) = c {
            *c = Column::Num(d.iter().map(|x| x.map(|v| v as f64).unwrap_or(f64::NAN)).collect());
        }
    }
    out
}

fn table_from_json_value(name: &str, v: &serde_json::Value, decl: &Source) -> Result<Table, String> {
    let bytes = serde_json::to_vec(v).map_err(|e| e.to_string())?;
    let mut t = datars_data::read_json(&bytes).map_err(|e| e.to_string())?;
    t.name = name.to_string();
    finish(t, decl)
}

fn finish(mut t: Table, decl: &Source) -> Result<Table, String> {
    coerce(&mut t, decl)?;
    if !decl.key.is_empty() {
        let keys: Vec<&str> = decl.key.iter().map(|s| s.as_str()).collect();
        t = t.with_key(&keys).map_err(|e| e.to_string())?;
        t.validate_keys().map_err(|e| e.to_string())?;
    }
    Ok(t)
}

/// The declared `types`, applied: web APIs send numbers and dates as strings
/// (`"40068807991924.84"`, `"2026-09-24"`), and a column the document says is a number must be one.
/// A column converts only if every cell does (`""`, `null`, `NA` are gaps): one that holds words
/// stays as it is, and a slot's check refuses it with the reason.
fn coerce(t: &mut Table, decl: &Source) -> Result<(), String> {
    fn gap(s: &str) -> bool {
        matches!(s, "" | "null" | "NULL" | "NA" | "N/A" | "NaN" | "-")
    }
    fn all<T>(c: &Column, parse: impl Fn(&str) -> Option<T>) -> Option<Vec<Option<T>>> {
        let Column::Str(v) = c else { return None };
        v.iter()
            .map(|s| match s.as_deref().map(str::trim) {
                None => Some(None),
                Some(s) if gap(s) => Some(None),
                Some(s) => parse(s).map(Some),
            })
            .collect()
    }
    for (col, ty) in &decl.types {
        let Some(c) = t.column_mut(col) else { continue };
        let next = match (ty.as_str(), &*c) {
            ("num", Column::Str(_)) => all(c, |s| datars_data::csv::parse_number(s, false)).map(|v| Column::Num(v.into_iter().map(|x| x.unwrap_or(f64::NAN)).collect())),
            ("date", Column::Str(_)) => all(c, datars_data::date::parse_date).map(Column::Date),
            ("bool", Column::Str(_)) => all(c, |s| match s {
                "true" | "TRUE" | "True" | "1" | "yes" => Some(true),
                "false" | "FALSE" | "False" | "0" | "no" => Some(false),
                _ => None,
            })
            .map(|v| Column::Bool(v.into_iter().map(|x| x.unwrap_or(false)).collect())),
            ("str", Column::Num(v)) => Some(Column::Str(v.iter().map(|x| (!x.is_nan()).then(|| std::sync::Arc::from(datars_data::num::fmt_num(*x).as_str()))).collect())),
            ("num" | "date" | "bool" | "str", _) => None,
            (other, _) => return Err(format!("column `{col}`: unknown type `{other}` (num, str, bool or date)")),
        };
        if let Some(n) = next {
            *c = n;
        }
    }
    Ok(())
}

/// The records under `path` (`data`, `results.items`) of a JSON response, as their own document.
fn rows_at(bytes: &[u8], path: &str) -> Result<Vec<u8>, String> {
    let v: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let mut at = &v;
    for part in path.split('.').filter(|p| !p.is_empty()) {
        at = match at {
            serde_json::Value::Object(o) => o.get(part).ok_or_else(|| format!("no `{part}` in the response (rows: \"{path}\")"))?,
            serde_json::Value::Array(a) => part.parse::<usize>().ok().and_then(|i| a.get(i)).ok_or_else(|| format!("no item `{part}` in the response (rows: \"{path}\")"))?,
            _ => return Err(format!("`{part}` of rows: \"{path}\" is inside a value, not an object")),
        };
    }
    serde_json::to_vec(at).map_err(|e| e.to_string())
}

pub fn parse_bytes(name: &str, bytes: &[u8], decl: &Source) -> Result<Table, String> {
    let looks_json = bytes.iter().find(|b| !b.is_ascii_whitespace()).is_some_and(|b| *b == b'[' || *b == b'{');
    let mut t = if looks_json {
        match &decl.rows {
            Some(path) => datars_data::read_json(&rows_at(bytes, path)?).map_err(|e| e.to_string())?,
            None => datars_data::read_json(bytes).map_err(|e| e.to_string())?,
        }
    } else {
        let mut o = CsvOptions::default();
        o.name = name.to_string();
        datars_data::read_csv(bytes, &o).map_err(|e| e.to_string())?
    };
    t.name = name.to_string();
    finish(t, decl)
}

/// A host's data for a slot must have the columns the document relies on — its key, its typed
/// columns, and its sample's columns — with numbers where the document declares numbers.
pub fn check_slot(name: &str, t: &Table, decl: &Source) -> Result<(), String> {
    let mut expected: Vec<String> = decl.key.clone();
    expected.extend(decl.types.keys().cloned());
    if let Some(sample) = decl.sample.as_ref().and_then(|v| table_from_json_value(name, v, decl).ok()) {
        expected.extend(sample.column_names().into_iter().map(String::from));
    }
    expected.sort();
    expected.dedup();
    let have = t.column_names();
    let missing: Vec<&String> = expected.iter().filter(|c| !have.contains(&c.as_str())).collect();
    if !missing.is_empty() {
        let list = |v: &[&String]| v.iter().map(|c| format!("`{c}`")).collect::<Vec<_>>().join(", ");
        return Err(format!("slot `{name}`: the data has no column {} (expected {})", list(&missing), list(&expected.iter().collect::<Vec<_>>())));
    }
    for (c, ty) in &decl.types {
        let ok = match (ty.as_str(), t.column(c)) {
            ("num", Some(col)) => matches!(col, datars_data::Column::Num(_)),
            ("date", Some(col)) => matches!(col, datars_data::Column::Date(_) | datars_data::Column::Num(_)),
            _ => true,
        };
        if !ok {
            return Err(format!("slot `{name}`: column `{c}` should hold {ty} values"));
        }
    }
    Ok(())
}

/// Load a document's sources: inline ones now, the rest as requests for the host.
pub fn load_sources(doc: &Doc) -> (BTreeMap<String, Arc<Table>>, crate::geo::GeoStore, Vec<Request>, Vec<Diag>) {
    let (mut out, mut req, mut diags) = (BTreeMap::new(), Vec::new(), Vec::new());
    let mut geo = crate::geo::GeoStore::new();
    for (name, s) in &doc.data {
        let r = match &s.from {
            SourceKind::Values(v) => table_from_json_value(name, v, s),
            SourceKind::Csv(text) => parse_bytes(name, text.as_bytes(), s),
            SourceKind::Geojson(v) | SourceKind::Topojson(v) => {
                match crate::geo::load(name, v.to_string().as_bytes(), s.id.as_deref()) {
                    Ok((g, t)) => {
                        geo.insert(name.clone(), Arc::new(g));
                        Ok(t)
                    }
                    Err(e) => Err(e),
                }
            }
            SourceKind::Atlas(a) => {
                req.push(Request::Atlas { name: name.clone(), atlas: a.clone() });
                continue;
            }
            SourceKind::Url(u) | SourceKind::Font(u) => {
                req.push(Request::Source { name: name.clone(), url: u.clone() });
                continue;
            }
            SourceKind::Slot(slot) => {
                req.push(Request::Slot { name: name.clone(), slot: slot.clone() });
                // Its sample stands in until the host provides the real rows.
                match &s.sample {
                    Some(v) => table_from_json_value(name, v, s),
                    None => continue,
                }
            }
            // Tile archives are read by range as views need them (`tiles`), never as tables.
            SourceKind::Tiles(_) => continue,
            SourceKind::Generate(g) => generate(name, g).and_then(|t| finish(t, s)),
        };
        match r {
            Ok(t) => {
                out.insert(name.clone(), Arc::new(t));
            }
            Err(e) => diags.push(Diag { message: format!("source `{name}`: {e}") }),
        }
    }
    (out, geo, req, diags)
}

/// Generated rows beyond this are refused: a mistyped count mustn't take the host's memory.
const MAX_GENERATED_ROWS: u64 = 50_000_000;
/// Rows a generated table evaluates at a time: its working columns stay small however many rows
/// there are, and only the kept columns grow.
const GEN_CHUNK: usize = 65_536;

/// A generated table's working columns for one chunk: `i` and every column computed so far.
struct GenEnv {
    cols: Vec<(String, datars_expr::OwnedColumn)>,
}

impl datars_expr::Env for GenEnv {
    fn column(&self, name: &str) -> Option<datars_expr::ColumnView<'_>> {
        self.cols.iter().rev().find(|(n, _)| n == name).map(|(_, c)| c.view())
    }
    fn signal(&self, _: &str) -> Option<datars_expr::Value> {
        None
    }
    fn call(&self, _: &str, _: &[datars_expr::Value]) -> Option<datars_expr::Value> {
        None
    }
}

/// Values of one generated column, typed by the first chunk (numbers, booleans or strings).
enum GenValues {
    Num(Vec<f64>),
    Bool(Vec<bool>),
    Str(Vec<Option<Arc<str>>>),
}

impl GenValues {
    fn owned(&self, from: usize) -> datars_expr::OwnedColumn {
        match self {
            GenValues::Num(v) => datars_expr::OwnedColumn::Num(v[from..].to_vec()),
            GenValues::Bool(v) => datars_expr::OwnedColumn::Bool(v[from..].to_vec()),
            GenValues::Str(v) => datars_expr::OwnedColumn::Str(v[from..].to_vec()),
        }
    }
}

/// Evaluate a `generate` source: its column expressions over `rows` rows, a chunk at a time.
fn generate(name: &str, g: &datars_ir::Generate) -> Result<Table, String> {
    use datars_expr::Value as V;
    if g.rows > MAX_GENERATED_ROWS {
        return Err(format!("{} rows to generate: at most {MAX_GENERATED_ROWS}", g.rows));
    }
    let mut compiled = Vec::with_capacity(g.columns.len());
    for c in &g.columns {
        let e = datars_expr::parse(&c.expr).map_err(|e| format!("column `{}`: {}", c.name, e.render(&c.expr)))?;
        let comp = datars_expr::compile(&e).map_err(|e| format!("column `{}`: {e}", c.name))?;
        if comp.uses_row() {
            return Err(format!("column `{}`: the row number of a generated table is `d.i`", c.name));
        }
        compiled.push((c.name.clone(), comp));
    }
    let names: Vec<&str> = std::iter::once("i").chain(g.columns.iter().map(|c| c.name.as_str())).collect();
    let keep: Vec<String> = if g.keep.is_empty() { g.columns.iter().map(|c| c.name.clone()).collect() } else { g.keep.clone() };
    if let Some(k) = keep.iter().find(|k| !names.contains(&k.as_str())) {
        return Err(format!("keeps `{k}`, which it doesn't generate"));
    }
    let n = g.rows as usize;
    let mut out: Vec<Option<GenValues>> = keep.iter().map(|_| None).collect();
    let mut types: Vec<Option<u8>> = vec![None; compiled.len()];
    for start in (0..n).step_by(GEN_CHUNK) {
        let len = GEN_CHUNK.min(n - start);
        let mut env = GenEnv { cols: vec![("i".into(), datars_expr::OwnedColumn::Num((start..start + len).map(|i| i as f64).collect()))] };
        for (k, (cname, c)) in compiled.iter().enumerate() {
            // Numbers take the VM's numeric path once the first chunk showed the column is numeric.
            let col = match types[k] {
                Some(0) => datars_expr::OwnedColumn::Num(c.eval_rows_num(len, &env)),
                _ => {
                    let vals = c.eval_rows(len, &env);
                    let ty = *types[k].get_or_insert(if vals.iter().all(|v| matches!(v, V::Num(_) | V::Null)) {
                        0
                    } else if vals.iter().all(|v| matches!(v, V::Bool(_))) {
                        1
                    } else {
                        2
                    });
                    match ty {
                        0 => datars_expr::OwnedColumn::Num(vals.iter().map(|v| if let V::Num(x) = v { *x } else { f64::NAN }).collect()),
                        1 => datars_expr::OwnedColumn::Bool(vals.iter().map(|v| v.truthy()).collect()),
                        _ => datars_expr::OwnedColumn::Str(vals.iter().map(|v| if matches!(v, V::Null) { None } else { Some(Arc::from(crate::env::str_of(v).as_str())) }).collect()),
                    }
                }
            };
            env.cols.push((cname.clone(), col));
        }
        for (slot, k) in out.iter_mut().zip(&keep) {
            let Some((_, col)) = env.cols.iter().rev().find(|(n, _)| n == k) else { continue };
            match (slot.get_or_insert_with(|| match col {
                datars_expr::OwnedColumn::Num(_) => GenValues::Num(Vec::with_capacity(n)),
                datars_expr::OwnedColumn::Bool(_) => GenValues::Bool(Vec::with_capacity(n)),
                datars_expr::OwnedColumn::Str(_) => GenValues::Str(Vec::with_capacity(n)),
            }), col) {
                (GenValues::Num(a), datars_expr::OwnedColumn::Num(b)) => a.extend_from_slice(b),
                (GenValues::Bool(a), datars_expr::OwnedColumn::Bool(b)) => a.extend_from_slice(b),
                (GenValues::Str(a), datars_expr::OwnedColumn::Str(b)) => a.extend_from_slice(b),
                (acc, other) => {
                    // A column whose type differs between chunks: keep it as text from here on.
                    let so_far = match acc.owned(0) {
                        datars_expr::OwnedColumn::Num(v) => v.iter().map(|x| Some(Arc::from(crate::env::str_of(&V::Num(*x)).as_str()))).collect(),
                        datars_expr::OwnedColumn::Bool(v) => v.iter().map(|x| Some(Arc::from(x.to_string().as_str()))).collect(),
                        datars_expr::OwnedColumn::Str(v) => v,
                    };
                    let mut all: Vec<Option<Arc<str>>> = so_far;
                    for r in 0..len {
                        let v = other.view().get(r);
                        all.push(if matches!(v, V::Null) { None } else { Some(Arc::from(crate::env::str_of(&v).as_str())) });
                    }
                    *acc = GenValues::Str(all);
                }
            }
        }
    }
    let columns: Vec<(String, Column)> = keep
        .iter()
        .zip(out)
        .map(|(k, v)| {
            let c = match v {
                Some(GenValues::Num(v)) => Column::Num(v),
                Some(GenValues::Bool(v)) => Column::Bool(v),
                Some(GenValues::Str(v)) => Column::Str(v),
                None => Column::Num(Vec::new()),
            };
            (k.clone(), c)
        })
        .collect();
    Table::from_columns(name, columns).map_err(|e| e.to_string())
}

/// A row's key: the table's key columns (a `__unit` column becomes a Unit part), or its index.
pub fn row_key(t: &Table, i: usize) -> Key {
    if t.key.is_empty() {
        return Key::one(i as i64);
    }
    let parts: Vec<KeyPart> = t
        .key
        .iter()
        .map(|k| {
            let v = t.get(k, i);
            if k == "__unit" {
                KeyPart::Unit { unit: v.as_f64().unwrap_or(0.0) as u32 }
            } else {
                v.to_key_part()
            }
        })
        .collect();
    Key::new(parts)
}

pub fn cell_string(t: &Table, col: &str, i: usize) -> String {
    match t.column(col) {
        None => String::new(),
        Some(_) => crate::env::str_of(&crate::scales::from_d(&t.get(col, i))),
    }
}

pub fn take_rows(t: &Table, rows: &[usize]) -> Table {
    t.take_rows(rows)
}

/// Does an op read anything but its rows — signals, the layout box, scales, projections, other
/// tables, any host state? Then its table is computed per context (box + scope) and never kept
/// across resolves. Each expression is asked what it reads: one that reads only row fields and
/// built-ins (`floor(d.votes / 100)`) keeps the op a pure function of its input table.
fn op_depends_on_context(r: &Resolver, op: &serde_json::Value) -> bool {
    let s = op.to_string();
    if s.contains("box.") || s.contains("scale.") {
        return true;
    }
    let mut exprs = Vec::new();
    collect_exprs(op, &mut exprs);
    exprs.iter().any(|src| r.compiled(src).is_none_or(|c| !c.signals().is_empty() || !c.calls().is_empty()))
}

/// Expression sources inside an op: `{"expr": "…"}` objects and `"=…"` strings, at any depth.
fn collect_exprs<'a>(v: &'a serde_json::Value, out: &mut Vec<&'a str>) {
    match v {
        serde_json::Value::String(s) => {
            if let Some(src) = s.strip_prefix('=') {
                out.push(src);
            }
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| collect_exprs(x, out)),
        serde_json::Value::Object(o) => {
            if let (1, Some(serde_json::Value::String(src))) = (o.len(), o.get("expr")) {
                out.push(src);
            } else {
                o.values().for_each(|x| collect_exprs(x, out));
            }
        }
        _ => {}
    }
}

/// The evaluation view of a table in this context.
pub(crate) fn get(r: &Resolver, name: &str, cx: &Cx) -> Option<Arc<Table>> {
    let key = cache_key(r, name, cx)?;
    if let Some(t) = r.tables.borrow().views.get(&key) {
        return Some(t.clone());
    }
    let orig = get_original(r, name, cx)?;
    let view = eval_view_arc(&orig);
    Rc::make_mut(&mut r.tables.borrow_mut().views).insert(key.clone(), view.clone());
    if key != name {
        let mut store = r.tables.borrow_mut();
        if !store.views.contains_key(name) {
            Rc::make_mut(&mut store.views).insert(name.to_string(), view.clone());
        }
    }
    Some(view)
}

fn cache_key(r: &Resolver, name: &str, cx: &Cx) -> Option<String> {
    if r.tables.borrow().sources.contains_key(name) {
        return Some(name.to_string());
    }
    r.tables.borrow().derived.get(name)?;
    let ctx = chain_depends(r, name, 0);
    Some(if ctx { format!("{name}|{}|{}|{:x}", cx.box_w, cx.box_h, cx.scope.fingerprint) } else { name.to_string() })
}

/// Does a derived table depend on its context, anywhere it reads from — its own ops, its `from`
/// chain, and every derived table an op names (a join's `with`: a map's regions joined to a table
/// filtered by the year signal follow the year)?
fn chain_depends(r: &Resolver, name: &str, depth: usize) -> bool {
    if depth > 16 {
        return false;
    }
    let d = r.tables.borrow().derived.get(name).cloned();
    match d {
        Some(d) => {
            d.ops.iter().any(|op| {
                op_depends_on_context(r, op) || {
                    let mut names = Vec::new();
                    strings(op, &mut names);
                    names.into_iter().filter(|n| *n != name && r.tables.borrow().derived.contains_key(*n)).any(|n| chain_depends(r, n, depth + 1))
                }
            }) || chain_depends(r, &d.from, depth + 1)
        }
        None => false,
    }
}

/// Layouts (box and scales) kept per table across resolves ([`chain_reads_state`]).
const MAX_LAYOUTS_KEPT: usize = 8;

/// Does a derived table read anything its layout key doesn't pin — a signal, the theme, fonts, a
/// projection, another table through `table.*`, `hover()` — anywhere in its chain? Reading the box
/// and the scales is fine: the key holds them.
fn chain_reads_state(r: &Resolver, name: &str, depth: usize) -> bool {
    if depth > 16 {
        return true;
    }
    let d = r.tables.borrow().derived.get(name).cloned();
    match d {
        Some(d) => {
            d.ops.iter().any(|op| {
                op_reads_state(r, op) || {
                    let mut names = Vec::new();
                    strings(op, &mut names);
                    names.into_iter().filter(|n| *n != name && r.tables.borrow().derived.contains_key(*n)).any(|n| chain_reads_state(r, n, depth + 1))
                }
            }) || chain_reads_state(r, &d.from, depth + 1)
        }
        None => false,
    }
}

fn op_reads_state(r: &Resolver, op: &serde_json::Value) -> bool {
    let mut exprs = Vec::new();
    collect_exprs(op, &mut exprs);
    exprs.iter().any(|src| r.compiled(src).is_none_or(|c| c.signals().iter().any(|s| s != "box.w" && s != "box.h") || c.calls().iter().any(|f| !f.starts_with("scale."))))
}

/// The original (typed) table.
/// The loaded source a table derives from (following `from` through derived tables).
pub(crate) fn root_source(r: &Resolver, name: &str) -> Option<Arc<Table>> {
    let store = r.tables.borrow();
    let mut n = name.to_string();
    for _ in 0..64 {
        if let Some(t) = store.sources.get(&n) {
            return Some(t.clone());
        }
        n = store.derived.get(&n)?.from.clone();
    }
    None
}

pub(crate) fn get_original(r: &Resolver, name: &str, cx: &Cx) -> Option<Arc<Table>> {
    if let Some(t) = r.tables.borrow().sources.get(name) {
        return Some(t.clone());
    }
    let key = cache_key(r, name, cx)?;
    if let Some(t) = r.tables.borrow().originals.get(&key) {
        return Some(t.clone());
    }
    // A context-free table (keyed by its bare name) another resolve already computed — or one that
    // reads its context only through the box and the scales, which its key pins (the scope's
    // fingerprint hashes the resolved scales): 1,000 lines through a plot's scales aren't rebuilt
    // when a hover or a state that doesn't move the plot resolves again.
    let pure = key == name;
    let layout = !pure && !chain_reads_state(r, name, 0);
    let shared = if pure || layout {
        let store = r.tables.borrow();
        let kept = store.shared.borrow();
        kept.get(&key).map(|s| (s.table.clone(), s.deps.clone()))
    } else {
        None
    };
    if let Some((t, d)) = shared {
        if (SharedTable { table: t.clone(), deps: d }).current(r) {
            r.tables.borrow_mut().originals.insert(key, t.clone());
            return Some(t);
        }
    }
    let d = r.tables.borrow().derived.get(name).cloned()?;
    let base = get_original(r, &d.from, cx)?;
    let mut t = (*base).clone();
    let mut failed = false;
    for op in &d.ops {
        match apply_op(r, &t, op, cx) {
            Ok(next) => t = next,
            Err(e) => {
                r.diag(format!("table `{name}`, op {}: {e}", op.get("op").and_then(|o| o.as_str()).unwrap_or("?")));
                failed = true;
                break;
            }
        }
    }
    t.name = name.to_string();
    let t = Arc::new(t);
    // A failed op says so on every resolve (its diagnostic is recomputed with the data).
    if (pure || layout) && !failed {
        let mut inputs = Vec::new();
        deps(r, name, &mut inputs, &mut Default::default());
        let store = r.tables.borrow();
        let mut kept = store.shared.borrow_mut();
        if layout {
            // A few layouts per table (small multiples, a resize): not one per window size ever seen.
            let prefix = format!("{name}|");
            let mine: Vec<String> = kept.keys().filter(|k| k.starts_with(&prefix)).cloned().collect();
            if mine.len() >= MAX_LAYOUTS_KEPT {
                mine.iter().for_each(|k| drop(kept.remove(k)));
            }
        }
        kept.insert(key.clone(), SharedTable { table: t.clone(), deps: inputs });
    }
    r.tables.borrow_mut().originals.insert(key, t.clone());
    Some(t)
}

fn s<'a>(op: &'a serde_json::Value, k: &str) -> Result<&'a str, String> {
    op.get(k).and_then(|v| v.as_str()).ok_or_else(|| format!("needs `{k}`"))
}

fn strs(op: &serde_json::Value, k: &str) -> Vec<String> {
    match op.get(k) {
        Some(serde_json::Value::Array(a)) => a.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
        Some(serde_json::Value::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    }
}

/// Evaluate an expression prop for every row of `t` (per-row context).
pub(crate) fn eval_rows(r: &Resolver, t: &Table, p: &Prop, cx: &Cx) -> Vec<datars_expr::Value> {
    let view = Arc::new(eval_view(t));
    if let Some(src) = p.as_expr() {
        if let Some(c) = r.compiled(src) {
            let snapshot = r.snapshot_for(&c.tables, cx);
            let mut env = r.env(cx, &snapshot);
            env.whole = Some(&view);
            env.row = None;
            return c.eval_rows(view.len(), &env);
        }
    }
    vec![r.eval(p, cx); view.len()]
}

/// The field an expression only reads (`d.id` → `id`), if that's all it does.
fn field_ref(src: &str) -> Option<&str> {
    let f = src.trim().strip_prefix("d.")?;
    (!f.is_empty() && f.chars().all(|c| c.is_alphanumeric() || c == '_')).then_some(f)
}

fn to_column(vals: &[datars_expr::Value]) -> Column {
    use datars_expr::Value as V;
    if vals.iter().all(|v| matches!(v, V::Num(_) | V::Null)) {
        Column::Num(vals.iter().map(|v| if let V::Num(n) = v { *n } else { f64::NAN }).collect())
    } else if vals.iter().all(|v| matches!(v, V::Bool(_))) {
        Column::Bool(vals.iter().map(|v| matches!(v, V::Bool(true))).collect())
    } else {
        Column::Str(vals.iter().map(|v| if matches!(v, V::Null) { None } else { Some(Arc::from(crate::env::str_of(v).as_str())) }).collect())
    }
}

fn apply_op(r: &Resolver, t: &Table, op: &serde_json::Value, cx: &Cx) -> Result<Table, String> {
    let e = |x: datars_data::DataError| x.to_string();
    let kind = s(op, "op")?;
    Ok(match kind {
        "filter" => {
            // Without an expression nothing passes: say so rather than show an empty chart.
            let p = Prop(op.get("expr").cloned().ok_or_else(|| format!("filter: needs `expr`, the expression the rows it keeps satisfy (the SDK's op.filter writes it){}", if op.get("where").is_some() { "; `where` isn't a filter field" } else { "" }))?);
            let mask: Vec<bool> = eval_rows(r, t, &p, cx).iter().map(truthy).collect();
            tf::filter(t, &mask).map_err(e)?
        }
        "derive" => {
            let p = Prop(op.get("expr").cloned().ok_or("derive: needs `expr`, the expression each row's new value is (the SDK's op.derive writes it)")?);
            // Over zero rows there are no values to infer a type from; a copied field keeps its
            // column's (so an emptied table's text key still joins as text).
            let copied = if t.is_empty() { p.as_expr().and_then(|src| field_ref(src)).and_then(|f| t.column(f)).map(|c| c.take(&[])) } else { None };
            tf::derive(t, s(op, "as")?, copied.unwrap_or_else(|| to_column(&eval_rows(r, t, &p, cx)))).map_err(e)?
        }
        "aggregate" => {
            let g = strs(op, "groupby");
            let gr: Vec<&str> = g.iter().map(|x| x.as_str()).collect();
            // A malformed list is an error, not an empty result (an agent writing the IR by hand
            // should hear about `ops: { … }` instead of seeing an empty chart).
            let list = match op.get("ops") {
                None => Vec::new(),
                Some(serde_json::Value::Array(a)) => a.clone(),
                Some(_) => return Err("aggregate: `ops` must be a list of { as, op, field } (the SDK's op.aggregate writes it)".into()),
            };
            for x in &list {
                if x.get("op").and_then(|o| o.as_str()).is_none() || x.get("as").and_then(|o| o.as_str()).is_none() {
                    return Err(format!("aggregate: each entry needs `op` and `as`: {x}"));
                }
            }
            let aggs: Vec<Agg> = Some(&list)
                .map(|a| {
                    a.iter()
                        .filter_map(|x| {
                            let f = x.get("op")?.as_str()?;
                            let op = match f {
                                "count" => AggOp::Count,
                                "sum" => AggOp::Sum,
                                "mean" | "average" => AggOp::Mean,
                                "median" => AggOp::Median,
                                "min" => AggOp::Min,
                                "max" => AggOp::Max,
                                "distinct" => AggOp::Distinct,
                                "first" => AggOp::First,
                                "last" => AggOp::Last,
                                q if q.starts_with('q') => AggOp::Quantile(q[1..].parse::<f64>().unwrap_or(50.0) / 100.0),
                                _ => return None,
                            };
                            Some(Agg { op, column: x.get("field").and_then(|f| f.as_str()).map(String::from), as_name: x.get("as")?.as_str()?.to_string() })
                        })
                        .collect()
                })
                .unwrap_or_default();
            if let Some(bad) = list.iter().filter_map(|x| x.get("op").and_then(|o| o.as_str())).find(|f| !matches!(*f, "count" | "sum" | "mean" | "average" | "median" | "min" | "max" | "distinct" | "first" | "last") && !f.starts_with('q')) {
                return Err(format!("aggregate: unknown op `{bad}` (count, sum, mean, median, min, max, distinct, first, last, q<percent>)"));
            }
            let mut out = tf::aggregate(t, &gr, &aggs).map_err(e)?;
            if !g.is_empty() {
                out = out.with_key(&gr).map_err(e)?;
            }
            out
        }
        "sort" => {
            let by: Vec<(String, bool)> = op
                .get("by")
                .and_then(|b| b.as_array())
                .map(|a| a.iter().filter_map(|x| Some((x.get(0)?.as_str()?.to_string(), x.get(1).and_then(|d| d.as_str()) == Some("desc")))).collect())
                .unwrap_or_default();
            let b: Vec<(&str, bool)> = by.iter().map(|(c, d)| (c.as_str(), *d)).collect();
            tf::sort(t, &b).map_err(e)?
        }
        "top" => tf::top_n(t, s(op, "by")?, op.get("n").and_then(|n| n.as_u64()).unwrap_or(10) as usize, op.get("other").and_then(|o| o.as_str())).map_err(e)?,
        "bin" => {
            let field = s(op, "field")?;
            let as_ = s(op, "as")?;
            if let Some(u) = op.get("unit").and_then(|u| u.as_str()) {
                let unit = match u {
                    "year" => TimeBin::Year,
                    "quarter" => TimeBin::Quarter,
                    "month" => TimeBin::Month,
                    "week" => TimeBin::Week,
                    "day" => TimeBin::Day,
                    "weekday" => TimeBin::Weekday,
                    _ => TimeBin::MonthOfYear,
                };
                tf::bin_time(t, field, unit, as_).map_err(e)?
            } else if let Some(p) = op.get("part").and_then(|p| p.as_str()) {
                let part = if let Some(sep) = p.strip_prefix("after:") {
                    TextPart::After(sep.into())
                } else if let Some(sep) = p.strip_prefix("before:") {
                    TextPart::Before(sep.into())
                } else if p == "first-letter" {
                    TextPart::FirstLetter
                } else {
                    TextPart::FirstWord
                };
                tf::bin_text(t, field, part, as_).map_err(e)?
            } else {
                let spec = if let Some(st) = op.get("step").and_then(|v| v.as_f64()) {
                    BinSpec::Step(st)
                } else {
                    BinSpec::Nice(op.get("count").and_then(|v| v.as_u64()).unwrap_or(10) as usize)
                };
                tf::bin_num(t, field, spec, as_).map_err(e)?
            }
        }
        "window" => {
            let f = s(op, "fn")?;
            let k = op.get("k").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
            let w = match f {
                "cumsum" => WindowOp::Cumsum,
                "rank" => WindowOp::Rank,
                "dense_rank" => WindowOp::DenseRank,
                "lag" => WindowOp::Lag(k),
                "lead" => WindowOp::Lead(k),
                "rolling_mean" => WindowOp::RollingMean(k.max(1)),
                "rolling_sum" => WindowOp::RollingSum(k.max(1)),
                "rolling_std" => WindowOp::RollingStd(k.max(1)),
                "rolling_min" => WindowOp::RollingMin(k.max(1)),
                "rolling_max" => WindowOp::RollingMax(k.max(1)),
                "ema" => WindowOp::Ema(k.max(1)),
                "cummax" => WindowOp::Cummax,
                "cummin" => WindowOp::Cummin,
                "first" => WindowOp::First,
                "last" => WindowOp::Last,
                "share_of_total" => WindowOp::ShareOfTotal,
                "pct_change" => WindowOp::PctChange,
                "share_of_first" => {
                    // value / first value (funnels): computed directly.
                    let v = t.num(s(op, "field")?).ok_or("numeric field needed")?.to_vec();
                    let first = v.first().copied().unwrap_or(1.0);
                    return tf::derive(t, s(op, "as")?, Column::Num(v.iter().map(|x| x / first).collect())).map_err(e);
                }
                other => return Err(format!("unknown window fn `{other}`")),
            };
            let part = strs(op, "partition");
            let pr: Vec<&str> = part.iter().map(|x| x.as_str()).collect();
            // `order: "-share"` sorts descending (a Pareto's running total from the largest).
            let order: Vec<(String, bool)> = op.get("order").and_then(|o| o.as_str()).map(|o| vec![o.strip_prefix('-').map_or((o.to_string(), false), |f| (f.to_string(), true))]).unwrap_or_default();
            let ob: Vec<(&str, bool)> = order.iter().map(|(c, d)| (c.as_str(), *d)).collect();
            // `min`: rolling windows (and the EMA) with fewer values are null, not partial.
            let opts = tf::WindowOpts { min_periods: op.get("min").and_then(|v| v.as_u64()).unwrap_or(1) as usize };
            tf::window_with(t, w, s(op, "field")?, &pr, &ob, s(op, "as")?, opts).map_err(e)?
        }
        "join" => {
            let other = get_original(r, s(op, "with")?, cx).ok_or("unknown table to join")?;
            let on = strs(op, "on");
            let o: Vec<&str> = on.iter().map(|x| x.as_str()).collect();
            tf::join(t, &other, &o, if op.get("kind").and_then(|k| k.as_str()) == Some("inner") { JoinKind::Inner } else { JoinKind::Left }).map_err(e)?
        }
        "pivot" => {
            let idx = strs(op, "index");
            let i: Vec<&str> = idx.iter().map(|x| x.as_str()).collect();
            tf::pivot(t, &i, s(op, "key")?, s(op, "value")?).map_err(e)?
        }
        "unpivot" => {
            let cols = strs(op, "columns");
            let c: Vec<&str> = cols.iter().map(|x| x.as_str()).collect();
            let as_ = strs(op, "as");
            let id: Vec<String> = t.column_names().into_iter().map(String::from).filter(|n| !cols.contains(n)).collect();
            let idr: Vec<&str> = id.iter().map(|x| x.as_str()).collect();
            tf::unpivot(t, &idr, &c, as_.first().map(|x| x.as_str()).unwrap_or("key"), as_.get(1).map(|x| x.as_str()).unwrap_or("value")).map_err(e)?
        }
        "union" => {
            let other = get_original(r, s(op, "with")?, cx).ok_or("unknown table to union")?;
            tf::union(&[t, &other]).map_err(e)?
        }
        // Rows written into the op itself (columns or records, keyed by `key`), in place of its
        // input: a recipe's own small lookup table — a tile map's grid layout — to join data onto.
        "values" => {
            let v = op.get("values").ok_or("values: needs `values`, columns ({ col: [...] }) or records ([{ … }])")?;
            let bytes = serde_json::to_vec(v).map_err(|x| x.to_string())?;
            let mut out = datars_data::read_json(&bytes).map_err(|x| format!("values: {x}"))?;
            out.name = t.name.clone();
            let key = strs(op, "key");
            if !key.is_empty() {
                let k: Vec<&str> = key.iter().map(|x| x.as_str()).collect();
                out = out.with_key(&k).map_err(e)?;
                out.validate_keys().map_err(e)?;
            }
            out
        }
        "sample" => tf::sample(t, op.get("n").and_then(|n| n.as_u64()).unwrap_or(100) as usize, op.get("seed").and_then(|s| s.as_u64()).unwrap_or(1)),
        "interpolate" => {
            let at = r.num(&Prop(op.get("at").cloned().unwrap_or_default()), cx, 0.0);
            let mut out = tf::interpolate_at(t, s(op, "key")?, s(op, "time")?, s(op, "value")?, at).map_err(e)?;
            if out.key.is_empty() {
                out = out.with_key(&[s(op, "key")?]).map_err(e)?;
            }
            out
        }
        _ => crate::layout_ops::apply(r, t, kind, op, cx)?,
    })
}

fn truthy(v: &datars_expr::Value) -> bool {
    use datars_expr::Value as V;
    match v {
        V::Null => false,
        V::Bool(b) => *b,
        V::Num(n) => *n != 0.0 && !n.is_nan(),
        V::Str(s) => !s.is_empty(),
    }
}

#[cfg(test)]
mod api_tests {
    use super::*;

    fn decl(json: &str) -> Source {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn an_api_response_gives_its_rows_with_the_declared_types() {
        // The shape of the U.S. Treasury's Fiscal Data API: records under `data`, numbers and
        // dates as strings, paging beside them.
        let body = br#"{"data":[{"record_date":"2026-09-24","tot_pub_debt_out_amt":"40068807991924.84"},
            {"record_date":"2026-09-23","tot_pub_debt_out_amt":"40073558531201.68"}],
            "meta":{"count":2},"links":{"next":null}}"#;
        let s = decl(r#"{"url": "https://api.example/debt", "rows": "data", "types": {"record_date": "date", "tot_pub_debt_out_amt": "num"}}"#);
        let t = parse_bytes("debt", body, &s).unwrap();
        assert_eq!(t.len(), 2);
        match t.column("tot_pub_debt_out_amt") {
            Some(Column::Num(v)) => assert_eq!(v[0], 40068807991924.84),
            other => panic!("{other:?}"),
        }
        match t.column("record_date") {
            Some(Column::Date(v)) => assert_eq!(v[0], datars_data::date::parse_date("2026-09-24")),
            other => panic!("{other:?}"),
        }
        // A path that isn't there says so; an unknown type too.
        let missing = parse_bytes("debt", body, &decl(r#"{"url": "x", "rows": "results.items"}"#)).unwrap_err();
        assert!(missing.contains("no `results`"), "{missing}");
        let bad = parse_bytes("debt", body, &decl(r#"{"url": "x", "rows": "data", "types": {"record_date": "time"}}"#)).unwrap_err();
        assert!(bad.contains("unknown type"), "{bad}");
    }
}
