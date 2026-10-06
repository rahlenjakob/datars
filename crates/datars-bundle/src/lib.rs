//! `datars-bundle` — how charts travel (docs/12-delivery.md, P16).
//!
//! A bundle is a signed **manifest** plus **chunks** addressed by BLAKE3 hash. The manifest lists
//! **variants** (T0 static … T3 programmable), each with the runtime version and modules it needs
//! and an **entry** chunk that lists the chunks it uses. An installed runtime opens the manifest,
//! verifies it, picks the best variant it can run under the host's policy, and asks the host for the
//! chunks it doesn't have cached — the loader is sans-IO (P10).

mod container;
mod loader;
mod semver;

pub use container::{from_single_file, to_single_file, MAGIC};
pub use loader::{ContentStore, Loader, MemStore};
pub use semver::{satisfies, Version};

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const FORMAT: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Tier {
    T0,
    T1,
    T2,
    T3,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Requires {
    /// A version requirement: `">=1.2"`, `">=1.2, <2"`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub runtime: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modules: Vec<String>,
    /// Built-in packages the variant expands with on the device, by content hash
    /// (`"@datars/std": "b3:…"`). A runtime whose package differs would draw a different chart than
    /// the author saw (P1), so it takes the next variant down (pre-expanded, the same output).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub packages: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Variant {
    pub tier: Tier,
    #[serde(default)]
    pub requires: Requires,
    /// Hash of the entry chunk (JSON with at least `"chunks": [hash…]`).
    pub entry: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChunkRef {
    pub hash: String,
    pub kind: String,
    pub bytes: u64,
    /// Fetched on demand, not before first render.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub lazy: bool,
    /// Likely shared with other bundles (atlases, fonts, std bytecode).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub shared: bool,
    /// Satisfied by a runtime built-in with this name when hashes match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub builtin: Option<String>,
    /// Free-form metadata (`state`, `from`, `to`, `size`, `locale`, …).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub meta: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DataSlot {
    pub slot: String,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub schema: serde_json::Value,
    /// `"host"` or `{"endpoint": url, "every": "15m"}`.
    pub from: serde_json::Value,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub format: u32,
    pub id: String,
    #[serde(default)]
    pub revision: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The document's authored size (px, width and height): the aspect ratio an embedding page
    /// reserves before the chart arrives, so the page doesn't jump.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    pub variants: Vec<Variant>,
    pub chunks: Vec<ChunkRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data: Vec<DataSlot>,
    /// Tokens a host may override.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub themeable: Vec<String>,
    /// Network endpoints the bundle may use, and per-bundle budgets.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub policy: serde_json::Value,
    /// The fonts the bundle's font chunks were cut from, with their licences — OFL and Apache
    /// fonts may be embedded, and their notices travel in each subset's name table too.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fonts: Vec<FontCredit>,
}

/// One font a bundle ships (as subsets): which face, where it came from, under what licence.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FontCredit {
    pub family: String,
    pub weight: u16,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    /// SPDX-style id read from the font's own notices: `OFL-1.1`, `Apache-2.0`, `UFL-1.0`, or
    /// `unknown` (no recognised licence in its name table).
    pub licence: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub licence_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copyright: Option<String>,
    /// Where the build step got it: `google:Newsreader:600`, a URL, a project file,
    /// `datars:fonts/Inter-Regular.ttf`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub from: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BundleError {
    Format(String),
    Signature(String),
    Hash { expected: String, actual: String },
    NoVariant(String),
    Missing(String),
}

impl std::fmt::Display for BundleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BundleError::Format(m) => write!(f, "bundle format: {m}"),
            BundleError::Signature(m) => write!(f, "signature: {m}"),
            BundleError::Hash { expected, actual } => write!(f, "chunk hash mismatch: expected {expected}, got {actual}"),
            BundleError::NoVariant(m) => write!(f, "no runnable variant: {m}"),
            BundleError::Missing(h) => write!(f, "missing chunk {h}"),
        }
    }
}

/// `"b3:<hex>"` — the content address of `bytes`.
pub fn chunk_hash(bytes: &[u8]) -> String {
    format!("b3:{}", blake3::hash(bytes).to_hex())
}

impl Manifest {
    pub fn from_json(s: &[u8]) -> Result<Manifest, BundleError> {
        let m: Manifest = serde_json::from_slice(s).map_err(|e| BundleError::Format(e.to_string()))?;
        if m.format > FORMAT {
            return Err(BundleError::Format(format!("bundle format {} is newer than this runtime reads ({FORMAT})", m.format)));
        }
        if !m.variants.iter().any(|v| v.tier == Tier::T0) {
            return Err(BundleError::Format("every bundle needs a T0 (static, accessible) variant".into()));
        }
        Ok(m)
    }
    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
    /// The bytes that are signed: the manifest without its signature.
    pub fn signing_bytes(&self) -> Vec<u8> {
        let mut m = self.clone();
        m.signature = None;
        m.to_json()
    }
    pub fn chunk(&self, hash: &str) -> Option<&ChunkRef> {
        self.chunks.iter().find(|c| c.hash == hash)
    }

    /// Sign with an ed25519 secret key (32 bytes); sets `publisher` and `signature`.
    pub fn sign(&mut self, secret: &[u8; 32]) {
        use ed25519_dalek::{Signer, SigningKey};
        let key = SigningKey::from_bytes(secret);
        self.publisher = Some(public_key_id(&key.verifying_key().to_bytes()));
        let sig = key.sign(&self.signing_bytes());
        self.signature = Some(hex(&sig.to_bytes()));
    }

    /// Verify the signature. With a non-empty `allowed` list, the publisher must be in it.
    pub fn verify(&self, allowed: &[String]) -> Result<(), BundleError> {
        use ed25519_dalek::{Signature, Verifier, VerifyingKey};
        let (Some(publisher), Some(sig)) = (&self.publisher, &self.signature) else {
            return if allowed.is_empty() { Ok(()) } else { Err(BundleError::Signature("unsigned bundle, but the host requires a publisher".into())) };
        };
        if !allowed.is_empty() && !allowed.contains(publisher) {
            return Err(BundleError::Signature(format!("publisher {publisher} is not allowed by the host")));
        }
        let pk = unhex(publisher.strip_prefix("ed25519:").unwrap_or(publisher)).ok_or_else(|| BundleError::Signature("bad publisher key".into()))?;
        let pk: [u8; 32] = pk.try_into().map_err(|_| BundleError::Signature("bad publisher key length".into()))?;
        let vk = VerifyingKey::from_bytes(&pk).map_err(|e| BundleError::Signature(e.to_string()))?;
        let sb = unhex(sig).ok_or_else(|| BundleError::Signature("bad signature encoding".into()))?;
        let sb: [u8; 64] = sb.try_into().map_err(|_| BundleError::Signature("bad signature length".into()))?;
        vk.verify(&self.signing_bytes(), &Signature::from_bytes(&sb)).map_err(|_| BundleError::Signature("signature does not match".into()))
    }
}

/// `"ed25519:<hex>"` for a public key.
pub fn public_key_id(pk: &[u8; 32]) -> String {
    format!("ed25519:{}", hex(pk))
}

/// The public key id for a secret key.
pub fn public_key_for(secret: &[u8; 32]) -> String {
    public_key_id(&ed25519_dalek::SigningKey::from_bytes(secret).verifying_key().to_bytes())
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
}

/// A bundle in memory: manifest + chunk bytes.
#[derive(Clone, Debug, Default)]
pub struct Bundle {
    pub manifest: Manifest,
    pub chunks: BTreeMap<String, Vec<u8>>,
}

/// Builds bundles: add chunks (deduplicated by hash), add variants, finish.
#[derive(Default)]
pub struct Builder {
    chunks: BTreeMap<String, (ChunkRef, Vec<u8>)>,
    variants: Vec<Variant>,
}

impl Builder {
    pub fn new() -> Builder {
        Builder::default()
    }
    /// Add a chunk; returns its hash. Identical bytes are stored once.
    pub fn chunk(&mut self, kind: &str, bytes: Vec<u8>, meta: BTreeMap<String, serde_json::Value>) -> String {
        let hash = chunk_hash(&bytes);
        self.chunks.entry(hash.clone()).or_insert_with(|| (ChunkRef { hash: hash.clone(), kind: kind.into(), bytes: bytes.len() as u64, meta, ..Default::default() }, bytes));
        hash
    }
    /// The bytes of a chunk added earlier.
    pub fn bytes(&self, hash: &str) -> Option<&[u8]> {
        self.chunks.get(hash).map(|(_, b)| b.as_slice())
    }
    pub fn mark(&mut self, hash: &str, lazy: bool, shared: bool, builtin: Option<&str>) {
        if let Some((c, _)) = self.chunks.get_mut(hash) {
            c.lazy = lazy;
            c.shared = shared;
            c.builtin = builtin.map(String::from);
        }
    }
    /// Add a variant whose entry chunk lists `chunks` plus arbitrary entry metadata.
    pub fn variant(&mut self, tier: Tier, requires: Requires, chunks: &[String], mut entry: serde_json::Value) -> String {
        if !entry.is_object() {
            entry = serde_json::json!({});
        }
        entry["tier"] = serde_json::to_value(tier).unwrap_or_default();
        entry["chunks"] = serde_json::to_value(chunks).unwrap_or_default();
        let hash = self.chunk("entry", serde_json::to_vec(&entry).unwrap_or_default(), BTreeMap::new());
        self.variants.push(Variant { tier, requires, entry: hash.clone() });
        hash
    }
    pub fn finish(mut self, id: &str, revision: &str) -> Bundle {
        // Best first: higher tiers first; the loader falls back down the list.
        self.variants.sort_by(|a, b| b.tier.cmp(&a.tier));
        let manifest = Manifest {
            format: FORMAT,
            id: id.into(),
            revision: revision.into(),
            variants: self.variants,
            chunks: self.chunks.values().map(|(c, _)| c.clone()).collect(),
            ..Default::default()
        };
        Bundle { manifest, chunks: self.chunks.into_iter().map(|(h, (_, b))| (h, b)).collect() }
    }
}

/// What the installed runtime can do, and what the host allows.
#[derive(Clone, Debug)]
pub struct Capabilities {
    pub runtime: Version,
    pub modules: Vec<String>,
    pub allow_script: bool,
    /// Accepted publisher keys (empty = any, including unsigned).
    pub publishers: Vec<String>,
    /// Built-in packages by content hash (see [`Requires::packages`]).
    pub packages: BTreeMap<String, String>,
}

impl Manifest {
    /// The best variant `caps` can run (manifest order is best-first).
    pub fn select(&self, caps: &Capabilities) -> Result<&Variant, BundleError> {
        let mut reasons = Vec::new();
        for v in &self.variants {
            if v.tier == Tier::T3 && !caps.allow_script {
                reasons.push(format!("{:?}: script not allowed", v.tier));
                continue;
            }
            if !v.requires.runtime.is_empty() && !satisfies(&caps.runtime, &v.requires.runtime) {
                reasons.push(format!("{:?}: needs runtime {}", v.tier, v.requires.runtime));
                continue;
            }
            if let Some(m) = v.requires.modules.iter().find(|m| !caps.modules.contains(m)) {
                reasons.push(format!("{:?}: needs module {m}", v.tier));
                continue;
            }
            if let Some((p, _)) = v.requires.packages.iter().find(|(p, h)| caps.packages.get(*p) != Some(*h)) {
                reasons.push(format!("{:?}: built with a different {p}", v.tier));
                continue;
            }
            return Ok(v);
        }
        Err(BundleError::NoVariant(reasons.join("; ")))
    }
}
