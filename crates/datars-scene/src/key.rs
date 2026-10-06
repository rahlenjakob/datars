//! Keys: identity from data (P3). A key is a small tuple; a key path locates a node in the tree.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;

/// An interned-ish shared string (cheap to clone, compares by content).
pub type Sym = Arc<str>;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum KeyPart {
    Int(i64),
    Str(Sym),
    /// The i-th unit of its parent datum (a seat of a party, a cell of a waffle).
    Unit { unit: u32 },
}

impl KeyPart {
    pub fn str(s: &str) -> KeyPart {
        KeyPart::Str(Arc::from(s))
    }
}

impl From<&str> for KeyPart {
    fn from(s: &str) -> KeyPart {
        KeyPart::Str(Arc::from(s))
    }
}
impl From<String> for KeyPart {
    fn from(s: String) -> KeyPart {
        KeyPart::Str(Arc::from(s.as_str()))
    }
}
impl From<i64> for KeyPart {
    fn from(v: i64) -> KeyPart {
        KeyPart::Int(v)
    }
}

impl fmt::Display for KeyPart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyPart::Int(v) => write!(f, "{v}"),
            KeyPart::Str(s) => write!(f, "{s:?}"),
            KeyPart::Unit { unit } => write!(f, "Unit({unit})"),
        }
    }
}

/// A node's key within its parent: a tuple of parts. `Key::default()` is the empty tuple.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Key(pub Arc<[KeyPart]>);

impl Key {
    pub fn new(parts: Vec<KeyPart>) -> Key {
        Key(parts.into())
    }
    pub fn one(p: impl Into<KeyPart>) -> Key {
        Key(Arc::from(vec![p.into()]))
    }
    pub fn name(s: &str) -> Key {
        Key::one(s)
    }
    pub fn parts(&self) -> &[KeyPart] {
        &self.0
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// This key with one more part (a unit child: `("S",)` → `("S", Unit(3))`).
    pub fn child(&self, p: KeyPart) -> Key {
        let mut v: Vec<KeyPart> = self.0.to_vec();
        v.push(p);
        Key(v.into())
    }
    /// The key without its last part, if it has more than one.
    pub fn parent(&self) -> Option<Key> {
        (self.0.len() > 1).then(|| Key(self.0[..self.0.len() - 1].to_vec().into()))
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(")?;
        for (i, p) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{p}")?;
        }
        if self.0.len() == 1 {
            write!(f, ",")?;
        }
        write!(f, ")")
    }
}

/// A path of keys from the root to a node.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct KeyPath(pub Vec<Key>);

impl KeyPath {
    pub fn push(&self, k: &Key) -> KeyPath {
        let mut v = self.0.clone();
        v.push(k.clone());
        KeyPath(v)
    }
    pub fn last(&self) -> Option<&Key> {
        self.0.last()
    }
}

impl fmt::Display for KeyPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, k) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, "/")?;
            }
            write!(f, "{k}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keys_order_and_display() {
        let a = Key::new(vec!["S".into(), KeyPart::Unit { unit: 3 }]);
        assert_eq!(a.to_string(), "(\"S\", Unit(3))");
        assert_eq!(a.parent().unwrap(), Key::one("S"));
        let j = serde_json::to_string(&a).unwrap();
        assert_eq!(j, r#"["S",{"unit":3}]"#);
        let back: Key = serde_json::from_str(&j).unwrap();
        assert_eq!(back, a);
        let mut ks = vec![Key::one("b"), Key::one(2), Key::one("a"), Key::one(1)];
        ks.sort();
        assert_eq!(ks, vec![Key::one(1), Key::one(2), Key::one("a"), Key::one("b")]);
    }
}
