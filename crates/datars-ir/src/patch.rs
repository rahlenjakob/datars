//! Typed edits on documents — how editors, agents and hot reload change a document without
//! rewriting it. Paths are JSON-pointer-like; a segment `#id` jumps to the first descendant object
//! whose `"id"` equals `id` (stable addressing that survives reordering).

use crate::Doc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum PatchOp {
    Set { path: String, value: Value },
    Remove { path: String },
    /// Insert into an array at `index` (or append when absent).
    Insert { path: String, #[serde(default)] index: Option<usize>, value: Value },
}

#[derive(Clone, Debug, PartialEq)]
pub struct PatchError(pub String);

impl std::fmt::Display for PatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Apply ops in order; the result must still be a valid document.
pub fn apply_patch(doc: &Doc, ops: &[PatchOp]) -> Result<Doc, PatchError> {
    let mut v = serde_json::to_value(doc).map_err(|e| PatchError(e.to_string()))?;
    for op in ops {
        match op {
            PatchOp::Set { path, value } => {
                let (parent, last) = split(path)?;
                let p = lookup_mut(&mut v, &parent)?;
                match p {
                    Value::Object(o) => {
                        o.insert(last, value.clone());
                    }
                    Value::Array(a) => {
                        let i: usize = last.parse().map_err(|_| PatchError(format!("bad index `{last}`")))?;
                        *a.get_mut(i).ok_or_else(|| PatchError(format!("index {i} out of range")))? = value.clone();
                    }
                    _ => return Err(PatchError(format!("cannot set into a scalar at `{path}`"))),
                }
            }
            PatchOp::Remove { path } => {
                let (parent, last) = split(path)?;
                match lookup_mut(&mut v, &parent)? {
                    Value::Object(o) => {
                        o.remove(&last).ok_or_else(|| PatchError(format!("no `{last}` at `{path}`")))?;
                    }
                    Value::Array(a) => {
                        let i: usize = last.parse().map_err(|_| PatchError(format!("bad index `{last}`")))?;
                        if i >= a.len() {
                            return Err(PatchError(format!("index {i} out of range")));
                        }
                        a.remove(i);
                    }
                    _ => return Err(PatchError(format!("cannot remove from a scalar at `{path}`"))),
                }
            }
            PatchOp::Insert { path, index, value } => {
                let segs = segments(path);
                match lookup_mut(&mut v, &segs)? {
                    Value::Array(a) => {
                        let i = index.unwrap_or(a.len()).min(a.len());
                        a.insert(i, value.clone());
                    }
                    _ => return Err(PatchError(format!("`{path}` is not an array"))),
                }
            }
        }
    }
    serde_json::from_value(v).map_err(|e| PatchError(format!("patched document is invalid: {e}")))
}

fn segments(path: &str) -> Vec<String> {
    path.split('/').filter(|s| !s.is_empty()).map(|s| s.replace("~1", "/").replace("~0", "~")).collect()
}

fn split(path: &str) -> Result<(Vec<String>, String), PatchError> {
    let mut s = segments(path);
    let last = s.pop().ok_or_else(|| PatchError("empty path".into()))?;
    Ok((s, last))
}

fn find_id<'a>(v: &'a mut Value, id: &str) -> Option<&'a mut Value> {
    let hit = matches!(v, Value::Object(o) if o.get("id").and_then(|x| x.as_str()) == Some(id));
    if hit {
        return Some(v);
    }
    match v {
        Value::Object(o) => o.values_mut().find_map(|c| find_id(c, id)),
        Value::Array(a) => a.iter_mut().find_map(|c| find_id(c, id)),
        _ => None,
    }
}

fn lookup_mut<'a>(v: &'a mut Value, segs: &[String]) -> Result<&'a mut Value, PatchError> {
    let mut cur = v;
    for s in segs {
        cur = if let Some(id) = s.strip_prefix('#') {
            find_id(cur, id).ok_or_else(|| PatchError(format!("no node with id `{id}`")))?
        } else {
            match cur {
                Value::Object(o) => o.get_mut(s).ok_or_else(|| PatchError(format!("no `{s}`")))?,
                Value::Array(a) => {
                    let i: usize = s.parse().map_err(|_| PatchError(format!("bad index `{s}`")))?;
                    a.get_mut(i).ok_or_else(|| PatchError(format!("index {i} out of range")))?
                }
                _ => return Err(PatchError(format!("`{s}` goes through a scalar"))),
            }
        };
    }
    Ok(cur)
}
