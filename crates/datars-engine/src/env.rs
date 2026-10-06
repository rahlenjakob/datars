//! What an expression can see while resolving: the current row (or tick, legend entry, group), the
//! signals, the scales in scope, the layout box, theme tokens, key metadata and formatting.

use crate::scales::Scope;
use datars_data::{Column, Table};
use datars_expr::{ColumnView, Env, Value};
use datars_theme::ResolvedTheme;
use std::collections::BTreeMap;
use std::sync::Arc;

/// One row of a table as expression columns (1-element slices).
pub struct Row {
    pub table: Arc<Table>,
    pub row: usize,
}

/// Values of synthetic datums (ticks, legend entries, counters).
pub type Fields = BTreeMap<String, Value>;

pub struct ResolveEnv<'a> {
    /// Vectorized evaluation over a whole table (instances, table ops).
    pub whole: Option<&'a Table>,
    pub row: Option<&'a Row>,
    pub fields: Option<&'a Fields>,
    /// Rows of the current group (for `group.*` aggregates).
    pub group: Option<(&'a Table, &'a [usize])>,
    pub signals: &'a BTreeMap<String, Value>,
    pub scopes: &'a Scope,
    pub box_w: f64,
    pub box_h: f64,
    pub theme: &'a ResolvedTheme,
    pub keys: &'a BTreeMap<String, datars_ir::KeyMeta>,
    pub locale: &'a str,
    pub tables: &'a BTreeMap<String, Arc<Table>>,
    /// Scratch for single-row slices (Str columns need owned storage).
    pub scratch: std::cell::RefCell<Vec<Box<[Option<Arc<str>>]>>>,
    pub proj: Option<&'a datars_geo::Projection>,
    pub geo: Option<&'a crate::geo::GeoStore>,
    /// For `measure(text, size, weight)`: fit decisions (label inside or outside a bar) need real
    /// glyph widths, not guesses.
    pub fonts: Option<&'a datars_text::FontDb>,
    /// For `hover()`: the interactive element this expression sits under, and the one under the
    /// pointer.
    pub hover: Option<Hover<'a>>,
}

/// What `hover()` needs. Asking records the element, so the engine re-resolves only when the
/// pointer moves onto or off an element some expression asked about.
pub struct Hover<'a> {
    pub at: &'a std::rc::Rc<datars_scene::KeyPath>,
    pub hovered: Option<&'a datars_scene::KeyPath>,
    pub asked: &'a std::cell::RefCell<Vec<datars_scene::KeyPath>>,
    /// The element last recorded (its expressions ask one after another).
    pub last: &'a std::cell::RefCell<Option<std::rc::Rc<datars_scene::KeyPath>>>,
}

fn num_of(v: &Value) -> f64 {
    match v {
        Value::Num(n) => *n,
        Value::Bool(b) => *b as i32 as f64,
        Value::Str(s) => s.parse().unwrap_or(f64::NAN),
        _ => f64::NAN,
    }
}

pub fn str_of(v: &Value) -> String {
    match v {
        Value::Str(s) => s.to_string(),
        Value::Num(n) => {
            if n.fract() == 0.0 && n.abs() < 1e15 {
                format!("{}", *n as i64)
            } else {
                format!("{n}")
            }
        }
        Value::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}

impl<'a> ResolveEnv<'a> {
    fn column_value(&self, name: &str) -> Option<Value> {
        if let Some(f) = self.fields {
            if let Some(v) = f.get(name) {
                return Some(v.clone());
            }
        }
        let r = self.row?;
        let col = r.table.column(name)?;
        Some(match col {
            Column::Num(v) => Value::Num(v.get(r.row).copied().unwrap_or(f64::NAN)),
            Column::Str(v) => v.get(r.row).cloned().flatten().map(Value::Str).unwrap_or(Value::Null),
            Column::Bool(v) => Value::Bool(v.get(r.row).copied().unwrap_or(false)),
            Column::Date(v) => v.get(r.row).copied().flatten().map(|d| Value::Num(d as f64)).unwrap_or(Value::Null),
        })
    }

    fn aggregate(&self, table: &Table, rows: Option<&[usize]>, op: &str, field: &str) -> Value {
        let Some(Column::Num(v)) = table.column(field) else {
            return if op == "count" { Value::Num(rows.map(|r| r.len()).unwrap_or(table.len()) as f64) } else { Value::Null };
        };
        let vals: Vec<f64> = match rows {
            Some(rs) => rs.iter().filter_map(|&i| v.get(i).copied()).filter(|x| !x.is_nan()).collect(),
            None => v.iter().copied().filter(|x| !x.is_nan()).collect(),
        };
        let r = match op {
            "sum" => vals.iter().sum::<f64>(),
            "max" => vals.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            "min" => vals.iter().copied().fold(f64::INFINITY, f64::min),
            "mean" => {
                if vals.is_empty() {
                    f64::NAN
                } else {
                    vals.iter().sum::<f64>() / vals.len() as f64
                }
            }
            "count" => vals.len() as f64,
            // In row order: where a series starts and ends (a sparkline's trend).
            "first" => vals.first().copied().unwrap_or(f64::NAN),
            "last" => vals.last().copied().unwrap_or(f64::NAN),
            _ => return Value::Null,
        };
        Value::Num(r)
    }
}

impl Env for ResolveEnv<'_> {
    fn column(&self, name: &str) -> Option<ColumnView<'_>> {
        if let Some(t) = self.whole {
            return match t.column(name)? {
                Column::Num(v) => Some(ColumnView::Num(v)),
                Column::Bool(v) => Some(ColumnView::Bool(v)),
                Column::Str(v) => Some(ColumnView::Str(v)),
                Column::Date(_) => None,
            };
        }
        if let Some(f) = self.fields {
            if f.contains_key(name) {
                return None; // served through signal() fallback of synthetic fields
            }
        }
        let r = self.row?;
        let col = r.table.column(name)?;
        let i = r.row;
        Some(match col {
            Column::Num(v) => ColumnView::Num(v.get(i..i + 1).unwrap_or(&[])),
            Column::Bool(v) => ColumnView::Bool(v.get(i..i + 1).unwrap_or(&[])),
            Column::Str(v) => ColumnView::Str(v.get(i..i + 1).unwrap_or(&[])),
            Column::Date(v) => {
                let s: Box<[Option<Arc<str>>]> = Box::new([None]);
                let _ = v;
                let mut sc = self.scratch.borrow_mut();
                sc.push(s);
                // Dates are exposed as numbers (days since epoch) through signal(); see `column_value`.
                return None;
            }
        })
    }

    fn signal(&self, name: &str) -> Option<Value> {
        // Bare names are signals first (a column never hides one); `d.x` reads the row.
        if let Some(v) = self.signals.get(name) {
            return Some(v.clone());
        }
        if let Some(v) = self.column_value(name) {
            return Some(v);
        }
        match name {
            "box.w" | "width" => return Some(Value::Num(self.box_w)),
            "box.h" | "height" => return Some(Value::Num(self.box_h)),
            _ => {}
        }
        if let Some(rest) = name.strip_prefix("d.") {
            return self.column_value(rest);
        }
        None
    }

    fn has_signal(&self, name: &str) -> bool {
        self.signals.contains_key(name)
    }

    fn call(&self, name: &str, args: &[Value]) -> Option<Value> {
        let a0 = args.first();
        if name.starts_with("geo.") {
            return crate::geo::call(name, args, self.proj, self.geo?);
        }
        // scale.<name>(v) and scale.<name>.<method>(…)
        if let Some(rest) = name.strip_prefix("scale.") {
            let (sname, method) = match rest.split_once('.') {
                Some((s, m)) => (s, Some(m)),
                None => (rest, None),
            };
            let sc = self.scopes.get(sname)?;
            return Some(match method {
                None => sc.map_value(a0.unwrap_or(&Value::Null)),
                Some("bandwidth") => Value::Num(sc.bandwidth()),
                Some("step") => Value::Num(sc.step()),
                Some("invert") => Value::Num(sc.invert(a0.map(num_of).unwrap_or(f64::NAN))),
                Some("ink") => Value::Str(Arc::from(sc.ink(a0.unwrap_or(&Value::Null)).to_string().as_str())),
                // `scale.x.label(v)`: the value as the axis would write it (dates in full).
                Some("label") => Value::Str(Arc::from(sc.label(a0.unwrap_or(&Value::Null), self.locale).as_str())),
                Some("min") => Value::Num(sc.range_min()),
                Some("max") => Value::Num(sc.range_max()),
                _ => return None,
            });
        }
        match name {
            // `hover()`: is the pointer over the element this node belongs to (the nearest one at
            // or above it with intents, or `pickable`)? False outside one, and on touch screens.
            "hover" => {
                let Some(h) = &self.hover else { return Some(Value::Bool(false)) };
                if !h.last.borrow().as_ref().is_some_and(|l| std::rc::Rc::ptr_eq(l, h.at)) {
                    *h.last.borrow_mut() = Some(h.at.clone());
                    h.asked.borrow_mut().push((**h.at).clone());
                }
                Some(Value::Bool(h.hovered.is_some_and(|p| p.0.starts_with(&h.at.0))))
            }
            "format" => {
                let v = a0.map(num_of).unwrap_or(f64::NAN);
                let spec = args.get(1).map(str_of).unwrap_or_else(|| ",.2~f".into());
                Some(Value::Str(Arc::from(datars_text::format::number(v, &spec, self.locale).as_str())))
            }
            // `measure(text, size?, weight?)`: the text's width on one line. `measure.word(…)`: the
            // width it can't wrap below — its widest unbreakable run (a label wrapped to any width
            // still overflows by this much).
            "measure" | "measure.word" => {
                let text = a0.map(str_of).unwrap_or_default();
                let size = args.get(1).map(num_of).filter(|x| x.is_finite()).unwrap_or_else(|| self.theme.number("size.label").unwrap_or(11.0));
                let weight = args.get(2).map(num_of).filter(|x| x.is_finite()).unwrap_or(400.0) as u16;
                let family = self.theme.font("font.body").map(|f| f.stack().join(", ")).filter(|f| !f.is_empty()).unwrap_or_else(|| "Inter".into());
                let word = name == "measure.word";
                let style = datars_scene::text::TextStyle { family: family.into(), weight, size, max_width: word.then_some(0.0), ..Default::default() };
                let fallback = || {
                    let longest = if word { text.split_whitespace().map(|w| w.chars().count()).max().unwrap_or(0) } else { text.chars().count() };
                    longest as f64 * size * 0.55
                };
                let w = self.fonts.map(|db| datars_text::measure(db, &text, &style).w).unwrap_or_else(fallback);
                Some(Value::Num(w))
            }
            "formatDate" => {
                let v = a0.map(num_of).unwrap_or(f64::NAN);
                let spec = args.get(1).map(str_of).unwrap_or_else(|| "%Y-%m-%d".into());
                Some(Value::Str(Arc::from(datars_text::format::date(v as i64, &spec, self.locale).as_str())))
            }
            "token" => {
                let n = a0.map(str_of)?;
                if let Some(x) = self.theme.number(&n) {
                    Some(Value::Num(x))
                } else if let Some(t) = self.theme.text(&n) {
                    Some(Value::Str(Arc::from(t)))
                } else {
                    Some(Value::Str(Arc::from(format!("${n}").as_str())))
                }
            }
            "key.name" => {
                let k = a0.map(str_of)?;
                Some(Value::Str(Arc::from(self.keys.get(&k).and_then(|m| m.name.clone()).unwrap_or(k).as_str())))
            }
            "key.color" => {
                let k = a0.map(str_of)?;
                self.keys.get(&k).and_then(|m| m.color.clone()).map(|c| Value::Str(Arc::from(c.as_str())))
            }
            "group.count" => Some(Value::Num(self.group.map(|g| g.1.len()).unwrap_or(0) as f64)),
            "group.sum" | "group.max" | "group.min" | "group.mean" | "group.first" | "group.last" => {
                let (t, rows) = self.group?;
                Some(self.aggregate(t, Some(rows), &name[6..], &a0.map(str_of)?))
            }
            "count" => {
                let t = self.tables.get(&a0.map(str_of)?)?;
                Some(Value::Num(t.len() as f64))
            }
            "sum" | "max" | "min" | "mean" => {
                let t = self.tables.get(&a0.map(str_of)?)?;
                Some(self.aggregate(t, None, name, &args.get(1).map(str_of)?))
            }
            // Namespaced forms: plain `min(…)`/`max(…)` are the expression language's numeric
            // built-ins, so table aggregates need a name of their own.
            "table.sum" | "table.max" | "table.min" | "table.mean" | "table.count" | "table.first" | "table.last" => {
                let t = self.tables.get(&a0.map(str_of)?)?;
                let op = &name[6..];
                if op == "count" && args.len() < 2 {
                    return Some(Value::Num(t.len() as f64));
                }
                Some(self.aggregate(t, None, op, &args.get(1).map(str_of)?))
            }
            _ => {
                // <signal>.has(k), <signal>.isEmpty(), <signal>.size()
                let (sig, method) = name.rsplit_once('.')?;
                let v = self.signals.get(sig)?;
                let set: Vec<String> = match v {
                    Value::Str(s) => s.split('\u{1f}').filter(|x| !x.is_empty()).map(String::from).collect(),
                    Value::Null => Vec::new(),
                    other => vec![str_of(other)],
                };
                match method {
                    "has" | "includes" => Some(Value::Bool(set.contains(&a0.map(str_of).unwrap_or_default()))),
                    "isEmpty" => Some(Value::Bool(set.is_empty())),
                    "size" => Some(Value::Num(set.len() as f64)),
                    _ => None,
                }
            }
        }
    }
}

/// Key sets travel as signals encoded in one string separated by U+001F (unit separator).
pub fn encode_keyset(keys: &[String]) -> Value {
    Value::Str(Arc::from(keys.join("\u{1f}").as_str()))
}

pub fn decode_keyset(v: &Value) -> Vec<String> {
    match v {
        Value::Str(s) => s.split('\u{1f}').filter(|x| !x.is_empty()).map(String::from).collect(),
        _ => Vec::new(),
    }
}
