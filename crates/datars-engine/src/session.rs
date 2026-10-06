//! Session recording and replay (docs/13-testing.md, docs/14-devtools-and-agents.md): every input
//! a host gives the engine — frames with their clock, pointer, wheel, events, signals, provided
//! data, resizes, scrubs — is logged, and replaying the log on a fresh engine reproduces the same
//! frames bit for bit (P1). A reader's session becomes a bug report; an interaction becomes a test.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One host input, in order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "in", rename_all = "kebab-case")]
pub enum Input {
    Frame { t: f64 },
    /// The host's clock before an input (`Engine::set_clock`): where the next transition starts.
    Clock { t: f64 },
    Pointer { kind: String, x: f64, y: f64 },
    Wheel { x: f64, y: f64, delta: f64 },
    Event { name: String },
    Goto { index: usize },
    Signal { name: String, value: serde_json::Value },
    /// Bytes as base64.
    Provide { source: String, bytes: String },
    /// A tile archive's bytes at `offset` (a `Request::Range` answered), base64.
    Range { source: String, offset: u64, bytes: String },
    Resize { width: f64, height: f64, dpr: f64 },
    Seek { pos: f64 },
    Playing { on: bool },
    ReducedMotion { on: bool },
    Activate { path: String },
    Mode { mode: String },
    Tokens { tokens: BTreeMap<String, serde_json::Value> },
    /// The host's share of the per-frame work budgets (`Engine::set_work_scale`).
    Work { scale: f64 },
}

/// A recorded session: the document as loaded, and the inputs that followed.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub datars_session: u32,
    pub doc: serde_json::Value,
    pub inputs: Vec<Input>,
}

impl Session {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
    pub fn from_json(s: &str) -> Result<Session, String> {
        serde_json::from_str(s).map_err(|e| e.to_string())
    }
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub fn unbase64(s: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let mut acc = 0u32;
    let mut bits = 0;
    for c in s.bytes().filter(|c| !c.is_ascii_whitespace()) {
        if c == b'=' {
            break;
        }
        let v = B64.iter().position(|b| *b == c).ok_or_else(|| format!("not base64: {:?}", c as char))? as u32;
        acc = acc << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits & 0xff) as u8);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips() {
        for s in [&b""[..], b"a", b"ab", b"abc", b"abcd", &[0, 255, 7, 128, 3]] {
            assert_eq!(unbase64(&base64(s)).unwrap(), s);
        }
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
    }
}
