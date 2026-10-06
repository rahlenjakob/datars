//! Keeping documents readable across IR changes, and telling authors what the engine ignored.
//!
//! - [`migrate`] rewrites older shapes into the current IR before parsing (documents and bundles
//!   written against earlier releases keep working) and says what it changed.
//! - [`ignored_fields`] lists fields of a document the IR doesn't know — a typo like `"fil"` would
//!   otherwise vanish silently, which is the worst possible failure for an author or an agent.

use serde_json::Value;

/// Rewrite older IR shapes in place. Returns one note per change.
///
/// Pre-1.0 changes handled:
/// - `instances`: the radius was `size` (clashing with the node's layout `size`) → `r`; the
///   per-instance key was `key` (clashing with the node key) → `instance_key`; per-instance
///   opacity was `opacity` (clashing with the node's opacity) → `instance_opacity` when it's an
///   expression.
/// - `view`: an explicit `clip: true` (now the default, and `clip` is the node's clip) is dropped.
pub fn migrate(doc: &mut Value) -> Vec<String> {
    let mut notes = Vec::new();
    walk(doc, "", &mut notes);
    notes
}

fn is_expr(v: &Value) -> bool {
    match v {
        Value::String(s) => s.starts_with('='),
        Value::Object(o) => o.contains_key("expr"),
        _ => false,
    }
}

fn walk(v: &mut Value, path: &str, notes: &mut Vec<String>) {
    match v {
        Value::Object(o) => {
            match o.get("kind").and_then(|k| k.as_str()) {
                Some("instances") => {
                    if o.get("size").is_some_and(|s| !s.is_object()) && !o.contains_key("r") {
                        let s = o.remove("size").unwrap_or_default();
                        o.insert("r".into(), s);
                        notes.push(format!("{path}: instances `size` → `r`"));
                    }
                    if o.get("key").is_some_and(is_expr) && !o.contains_key("instance_key") {
                        let k = o.remove("key").unwrap_or_default();
                        o.insert("instance_key".into(), k);
                        notes.push(format!("{path}: instances `key` expression → `instance_key`"));
                    }
                    if o.get("opacity").is_some_and(is_expr) && !o.contains_key("instance_opacity") {
                        let k = o.remove("opacity").unwrap_or_default();
                        o.insert("instance_opacity".into(), k);
                        notes.push(format!("{path}: instances `opacity` expression → `instance_opacity`"));
                    }
                }
                Some("view")
                    if o.get("clip") == Some(&Value::Bool(true)) => {
                        o.remove("clip");
                        notes.push(format!("{path}: view `clip: true` is the default; dropped"));
                    }
                _ => {}
            }
            for (k, c) in o.iter_mut() {
                walk(c, &format!("{path}.{k}"), notes);
            }
        }
        Value::Array(a) => {
            for (i, c) in a.iter_mut().enumerate() {
                walk(c, &format!("{path}[{i}]"), notes);
            }
        }
        _ => {}
    }
}

/// Fields present in `input` that don't survive a parse → serialize round trip through the IR,
/// as JSON paths. Fields at default-like values (`false`, `0`, `""`, `null`, `[]`, `{}`) are
/// skipped: known fields at their defaults aren't serialized either.
pub fn ignored_fields(input: &Value) -> Vec<String> {
    ignored_in::<crate::Doc>(input)
}

/// [`ignored_fields`] for any IR type (a template from a recipe expansion, a table op list).
pub fn ignored_in<T: serde::de::DeserializeOwned + serde::Serialize>(input: &Value) -> Vec<String> {
    let Ok(parsed) = serde_json::from_value::<T>(input.clone()) else { return Vec::new() };
    let Ok(back) = serde_json::to_value(&parsed) else { return Vec::new() };
    let mut out = Vec::new();
    diff(input, &back, &mut String::new(), &mut out);
    out
}

fn default_like(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::Bool(b) => !b,
        Value::Number(n) => n.as_f64() == Some(0.0),
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
    }
}

/// Walk `a` against its round trip `b`; `path` is one buffer grown and cut back as the walk goes
/// (a document's inline data can be tens of thousands of values: no string per value).
fn diff(a: &Value, b: &Value, path: &mut String, out: &mut Vec<String>) {
    use std::fmt::Write;
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for (k, v) in x {
                let len = path.len();
                if len > 0 {
                    path.push('.');
                }
                path.push_str(k);
                match y.get(k) {
                    Some(w) => diff(v, w, path, out),
                    None if !default_like(v) => out.push(path.clone()),
                    None => {}
                }
                path.truncate(len);
            }
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            for (i, (v, w)) in x.iter().zip(y).enumerate() {
                // Scalars can't hide an ignored field.
                if !(v.is_object() || v.is_array()) {
                    continue;
                }
                let len = path.len();
                let _ = write!(path, "[{i}]");
                diff(v, w, path, out);
                path.truncate(len);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_instances_and_views_migrate() {
        let mut v = serde_json::json!({ "datars": 1, "scene": { "kind": "view", "clip": true, "children": [
            { "kind": "instances", "from": "t", "x": "=d.x", "y": "=d.y", "size": 3, "key": "=d.id", "opacity": "=d.o" }
        ] } });
        let notes = migrate(&mut v);
        assert_eq!(notes.len(), 4, "{notes:?}");
        let inst = &v["scene"]["children"][0];
        assert_eq!(inst["r"], 3);
        assert_eq!(inst["instance_key"], "=d.id");
        assert_eq!(inst["instance_opacity"], "=d.o");
        assert!(v["scene"].get("clip").is_none());
        // Current documents are untouched.
        let mut again = v.clone();
        assert!(migrate(&mut again).is_empty());
        assert_eq!(again, v);
    }

    #[test]
    fn typos_are_reported_defaults_are_not() {
        let v = serde_json::json!({ "datars": 1, "size": { "width": 10, "height": 10 },
            "scene": { "kind": "shape", "geom": { "type": "rect", "x": 0, "y": 0, "w": 1, "h": 1 }, "fil": "$accent", "pickable": false } });
        assert_eq!(ignored_fields(&v), vec!["scene.fil".to_string()]);
    }

    #[test]
    fn recipe_output_is_checked_as_a_template() {
        let t = serde_json::json!({ "kind": "group", "children": [{ "kind": "text", "text": "x", "at": [0, 0], "styel": { "size": 11 } }] });
        assert_eq!(ignored_in::<crate::Template>(&t), vec!["children[0].styel".to_string()]);
    }
}
