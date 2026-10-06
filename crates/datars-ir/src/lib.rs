//! `datars-ir` — the document format (docs/03-architecture.md, P9).
//!
//! A document describes *what* to show: data sources, derived tables, signals, a scene **template**,
//! motion rules, a program (statechart), a theme. The engine resolves the template against tables,
//! signals, the viewport and the theme into concrete scenes. Every SDK (TypeScript, Rust, Python),
//! visual editors and coding agents produce this format; it is plain JSON with a version.
//!
//! Property values in templates are either literals or **expressions**: `{"expr": "d.share * 2"}`,
//! or the shorthand string `"=d.share * 2"`. Colours are inks (`"#e8112d"`, `"$accent"`).

pub mod migrate;
mod doc;
mod patch;
mod template;

pub use doc::*;
pub use patch::{apply_patch, PatchError, PatchOp};
pub use template::*;

/// The IR as a JSON Schema (draft 2020-12): what a `doc.json` may contain — for editors,
/// validators and agents writing documents without the TypeScript SDK.
#[cfg(feature = "schema")]
pub fn json_schema() -> serde_json::Value {
    let mut s = serde_json::to_value(schemars::schema_for!(Doc)).unwrap_or_default();
    s["$id"] = "https://datars.dev/schema/ir-1.json".into();
    s["title"] = "datars document (IR 1)".into();
    s
}

/// The current document format version. Bumped on breaking changes, with a migration.
pub const FORMAT_VERSION: u32 = 1;
