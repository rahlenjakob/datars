//! Where a scene's views look at their tile sources: the extents an archive for the document must
//! hold (`Engine::tile_views`). The same walk over the same placeholders as the frame pass
//! ([`super::fill`]), computing each view's extent instead of looking tiles up — so an archive cut
//! from these (an automatic basemap, built by `datars-build`) holds what the engine will ask for,
//! at the zooms it will ask for it.

use super::view::{self, Extent};
use super::Binding;
use crate::bounds::transform_rect;
use datars_math::{Affine, Rect};
use datars_scene::{Clip, KeyPath, Node, NodeKind, Scene};
use std::collections::BTreeMap;
use std::rc::Rc;

/// One placeholder as a frame shows it.
pub(crate) struct Seen {
    pub source: String,
    /// The placeholder's key path (to tell explorable views apart).
    pub path: String,
    pub extent: Extent,
}

/// Every filled-in-per-frame `tiles` placeholder of `scene` that shows, with its extent.
pub(crate) fn extents(bindings: &BTreeMap<String, Rc<Binding>>, scene: &Scene) -> Vec<Seen> {
    let mut out = Vec::new();
    if !bindings.is_empty() {
        walk(bindings, &scene.root, &KeyPath::default(), Affine::IDENTITY, Rect::new(0.0, 0.0, scene.width, scene.height), &mut out);
    }
    out
}

fn walk(bindings: &BTreeMap<String, Rc<Binding>>, n: &Node, path: &KeyPath, parent: Affine, clip: Rect, out: &mut Vec<Seen>) {
    if !n.common.visible || n.common.opacity <= 0.0 {
        return;
    }
    let xf = parent.mul(n.common.transform);
    let here = path.push(&n.key);
    let clip = match &n.common.clip {
        Some(Clip::Rect { rect }) => transform_rect(*rect, &xf).intersect(&clip).unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0)),
        _ => clip,
    };
    match &n.kind {
        NodeKind::Group { children } if children.is_empty() => {
            let key = here.to_string();
            if let Some(b) = bindings.get(&key) {
                if let Some(extent) = view::extent(&b.proj, &xf, clip, b.spec.tile_size.unwrap_or(512.0)) {
                    out.push(Seen { source: b.source.clone(), path: key, extent });
                }
            }
        }
        NodeKind::Group { children } => {
            for c in children {
                walk(bindings, c, &here, xf, clip, out);
            }
        }
        NodeKind::View { viewport, camera, clip: clips, children } => {
            let clip = if *clips { transform_rect(*viewport, &xf).intersect(&clip).unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0)) } else { clip };
            let cam = camera.map(|c| c.transform(*viewport)).unwrap_or(Affine::translate(viewport.x, viewport.y));
            for c in children {
                walk(bindings, c, &here, xf.mul(cam), clip, out);
            }
        }
        _ => {}
    }
}
