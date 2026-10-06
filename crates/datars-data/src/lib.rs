//! `datars-data` — typed, keyed, columnar tables; readers; transforms; scales (docs/06, docs/04).
//!
//! - [`Table`]: named, typed columns ([`Column`]: numbers, strings, booleans, calendar dates) plus
//!   the key columns that give every row its identity (P3). Keys are validated; tables without a
//!   key fall back to row indices and say so ([`Table::has_stable_key`]).
//! - Readers turn bytes into tables (sans-IO, P10): [`read_csv`] (RFC 4180, delimiter detection,
//!   type inference with overrides) and [`read_json`] (records or columns).
//! - [`transform`]: pure, deterministic table → table functions (filter, aggregate, sort, bins,
//!   windows, joins, reshaping, seeded sampling, interpolation at a data time).
//! - [`scale`]: continuous, discrete, calendar-aware time and colour scales with nice ticks and
//!   interpolation for animated rescales. Colour scales return [`datars_theme::Ink`]s, so themes
//!   stay late-bound.
//!
//! Determinism: no hash maps, no platform maths (`datars_math::m`), fixed-order (pairwise) sums,
//! stable sorts with a total order (nulls last).

pub mod csv;
pub mod date;
pub mod json;
pub mod num;
pub mod scale;
pub mod table;
pub mod transform;
pub mod value;

pub use csv::{read_csv, CsvOptions, Encoding};
pub use json::read_json;
pub use scale::Scale;
pub use table::{Column, ColumnType, Field, Schema, Table};
pub use value::Value;

use std::fmt;

/// Everything that can go wrong reading or transforming data. Messages name the place (line and
/// column for parse errors, column names and rows otherwise) so a person can fix the input.
#[derive(Clone, Debug, PartialEq)]
pub enum DataError {
    /// Malformed input. `line` and `column` are 1-based (column counts characters); 0 = unknown.
    Parse { line: usize, column: usize, message: String },
    /// A column that doesn't exist; `available` lists what does.
    UnknownColumn { name: String, available: Vec<String> },
    /// Rows that share a key. `examples` holds up to three duplicated keys with their (0-based)
    /// row indices; `count` is the number of rows whose key was already taken.
    DuplicateKeys { key: Vec<String>, count: usize, examples: Vec<(String, Vec<usize>)> },
    /// A key column is null at a row (keys must be present).
    NullKey { column: String, row: usize },
    /// A column whose length differs from the table's.
    LengthMismatch { column: String, expected: usize, found: usize },
    /// A column of the wrong type for an operation.
    TypeMismatch { column: String, expected: String, found: String },
    /// Anything else (bad parameters, incompatible tables).
    Invalid(String),
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataError::Parse { line, column, message } => match (line, column) {
                (0, _) => write!(f, "{message}"),
                (l, 0) => write!(f, "line {l}: {message}"),
                (l, c) => write!(f, "line {l}, column {c}: {message}"),
            },
            DataError::UnknownColumn { name, available } => {
                let list: Vec<String> = available.iter().take(12).map(|c| format!("\"{c}\"")).collect();
                let more = if available.len() > 12 { ", …" } else { "" };
                if available.is_empty() {
                    write!(f, "no column \"{name}\" (the table has no columns)")
                } else {
                    write!(f, "no column \"{name}\" (columns: {}{more})", list.join(", "))
                }
            }
            DataError::DuplicateKeys { key, count, examples } => {
                write!(f, "duplicate keys on ({}): {count} row(s) repeat an earlier key", key.join(", "))?;
                for (i, (k, rows)) in examples.iter().enumerate() {
                    let rs: Vec<String> = rows.iter().map(|r| r.to_string()).collect();
                    write!(f, "{} {k} at rows {}", if i == 0 { " —" } else { ";" }, rs.join(", "))?;
                }
                Ok(())
            }
            DataError::NullKey { column, row } => write!(f, "key column \"{column}\" is empty at row {row} (keys must be present)"),
            DataError::LengthMismatch { column, expected, found } => {
                write!(f, "column \"{column}\" has {found} values, the table has {expected} rows")
            }
            DataError::TypeMismatch { column, expected, found } => write!(f, "column \"{column}\" is {found}, expected {expected}"),
            DataError::Invalid(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for DataError {}

impl From<serde_json::Error> for DataError {
    /// A JSON error with its position; serde_json's own " at line L column C" suffix is dropped
    /// since `Display` puts the position first.
    fn from(e: serde_json::Error) -> DataError {
        let text = e.to_string();
        let message = match text.rfind(" at line ") {
            Some(i) if e.line() > 0 => text[..i].to_string(),
            _ => text,
        };
        DataError::Parse { line: e.line(), column: e.column(), message }
    }
}
