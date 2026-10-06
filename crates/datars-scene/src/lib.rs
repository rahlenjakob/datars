//! `datars-scene` — the resolved scene: a keyed tree of primitives (docs/04-primitives.md).
//!
//! A scene is what the engine resolves for one signal state, and what a frame evaluates to at a
//! given time. It knows shapes, text, instances, groups and views — never chart types (P4).

pub mod geom;
pub mod hash;
pub mod key;
pub mod node;
pub mod paint;
pub mod snapshot;
pub mod text;

pub use geom::{Curve, Geom, SymbolKind};
pub use key::{Key, KeyPart, KeyPath, Sym};
pub use node::{FLOAT_Z, 
    Anchor, Blend, Camera, Clip, Common, Instances, Marker, Markers, Node, NodeKind, Proto, Role, Scene, Semantics,
};
pub use paint::{Cap, Join, Paint, Stop, Stroke};
pub use text::{GlyphPos, NumberText, TextNode, TextRun};

pub use datars_color::Color;
pub use datars_math::{Affine, PathData, Rect, Vec2};
