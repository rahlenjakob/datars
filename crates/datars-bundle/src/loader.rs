//! The sans-IO loader: open a manifest, pick a variant, list the chunks to fetch, verify what the
//! host provides. Chunks already in the content store (cache, built-ins, app-shipped files) are used
//! without a request.

use crate::{chunk_hash, BundleError, Capabilities, Manifest, Variant};
use std::collections::BTreeMap;
use std::sync::Arc;

/// A content-addressed store (memory, disk cache, app bundle). Immutable entries: a hash always maps
/// to the same bytes, so caching is trivially correct and shared across bundles.
pub trait ContentStore {
    fn get(&self, hash: &str) -> Option<Arc<[u8]>>;
    fn put(&mut self, hash: &str, bytes: Arc<[u8]>);
}

#[derive(Default)]
pub struct MemStore {
    pub map: BTreeMap<String, Arc<[u8]>>,
}

impl ContentStore for MemStore {
    fn get(&self, hash: &str) -> Option<Arc<[u8]>> {
        self.map.get(hash).cloned()
    }
    fn put(&mut self, hash: &str, bytes: Arc<[u8]>) {
        self.map.insert(hash.to_string(), bytes);
    }
}

pub struct Loader {
    pub manifest: Manifest,
    pub variant: Variant,
    have: BTreeMap<String, Arc<[u8]>>,
    entry: Option<serde_json::Value>,
    /// Lazy chunks the runtime asked for (script subsets text needs now).
    wanted_lazy: std::collections::BTreeSet<String>,
}

impl Loader {
    /// Verify and select. `store` is consulted for everything, starting with the entry chunk.
    pub fn open(manifest_bytes: &[u8], caps: &Capabilities, store: &dyn ContentStore) -> Result<Loader, BundleError> {
        let manifest = Manifest::from_json(manifest_bytes)?;
        manifest.verify(&caps.publishers)?;
        let variant = manifest.select(caps)?.clone();
        let mut l = Loader { manifest, variant, have: BTreeMap::new(), entry: None, wanted_lazy: Default::default() };
        l.fill_from(store);
        Ok(l)
    }

    fn fill_from(&mut self, store: &dyn ContentStore) {
        // Loop: the entry chunk, once found, reveals more chunks to look up.
        loop {
            let mut progressed = false;
            for h in self.wanted(true) {
                if !self.have.contains_key(&h) {
                    if let Some(b) = store.get(&h) {
                        progressed |= self.accept(&h, b).is_ok();
                    }
                }
            }
            if !progressed {
                break;
            }
        }
    }

    /// Chunks this variant needs (entry first; the rest once the entry is known).
    fn wanted(&self, include_lazy: bool) -> Vec<String> {
        let mut out = vec![self.variant.entry.clone()];
        if let Some(e) = &self.entry {
            for h in e.get("chunks").and_then(|c| c.as_array()).into_iter().flatten().filter_map(|h| h.as_str()) {
                let lazy = self.manifest.chunk(h).map(|c| c.lazy).unwrap_or(false);
                if include_lazy || !lazy || self.wanted_lazy.contains(h) {
                    out.push(h.to_string());
                }
            }
        }
        out
    }

    /// What the host should fetch now (not already present): eager chunks, and lazy ones asked
    /// for with [`Loader::want`].
    pub fn requests(&self) -> Vec<String> {
        self.wanted(false).into_iter().filter(|h| !self.have.contains_key(h)).collect()
    }

    /// Fetch a lazily loaded chunk of this variant too (a script subset that text needs now).
    /// Returns false if the variant doesn't list it.
    pub fn want(&mut self, hash: &str) -> bool {
        let listed = self.entry.as_ref().and_then(|e| e.get("chunks")).and_then(|c| c.as_array()).is_some_and(|a| a.iter().any(|h| h.as_str() == Some(hash)));
        if listed {
            self.wanted_lazy.insert(hash.to_string());
        }
        listed
    }

    fn accept(&mut self, hash: &str, bytes: Arc<[u8]>) -> Result<(), BundleError> {
        let actual = chunk_hash(&bytes);
        if actual != hash {
            return Err(BundleError::Hash { expected: hash.to_string(), actual });
        }
        if hash == self.variant.entry {
            self.entry = serde_json::from_slice(&bytes).ok();
        }
        self.have.insert(hash.to_string(), bytes);
        Ok(())
    }

    /// Hand over fetched bytes (verified against the hash; stored in `store` for next time).
    pub fn provide(&mut self, hash: &str, bytes: Arc<[u8]>, store: &mut dyn ContentStore) -> Result<(), BundleError> {
        self.accept(hash, bytes.clone())?;
        store.put(hash, bytes);
        self.fill_from(store);
        Ok(())
    }

    /// The entry and every eager chunk are here (lazy chunks come later, on demand).
    pub fn is_ready(&self) -> bool {
        self.entry.is_some() && self.wanted(false).iter().filter(|h| !self.wanted_lazy.contains(*h)).all(|h| self.have.contains_key(h))
    }

    pub fn entry(&self) -> Option<&serde_json::Value> {
        self.entry.as_ref()
    }

    pub fn chunk(&self, hash: &str) -> Option<Arc<[u8]>> {
        self.have.get(hash).cloned()
    }

    /// The first chunk of `kind` (optionally with matching meta key/value) in this variant.
    pub fn find(&self, kind: &str, meta: Option<(&str, &str)>) -> Option<Arc<[u8]>> {
        let e = self.entry.as_ref()?;
        let hashes = e.get("chunks")?.as_array()?;
        hashes.iter().filter_map(|h| h.as_str()).find_map(|h| {
            let c = self.manifest.chunk(h)?;
            let ok = c.kind == kind && meta.is_none_or(|(k, v)| c.meta.get(k).and_then(|x| x.as_str()) == Some(v));
            if ok { self.have.get(h).cloned() } else { None }
        })
    }
}
