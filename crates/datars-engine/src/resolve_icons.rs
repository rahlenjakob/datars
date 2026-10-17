//! Resolving icons and pictures: `icon` nodes, icon symbols, icon instances and markers, `image`
//! nodes (see `assets`). An icon node becomes a group of plain shapes — one per layer of its
//! drawing, in its inks — so every renderer, hit test, export and transition handles it as it
//! handles any shape; many of one icon become one instances node instead.

use crate::assets::{icon_request, image_request};
use crate::resolve::{Cx, Resolver};
use datars_icon::{Icon, Source};
use datars_ir::{Prop, TIcon};
use datars_math::{Affine, Rect, Vec2};
use datars_scene::{Geom, IconGlyph, IconInk, Key, Node, NodeKind, Paint, Stroke};
use datars_theme::Ink;
use std::sync::Arc;

/// An icon's box when nothing says (and the theme has no `size.icon`), px.
pub const DEFAULT_ICON_SIZE: f64 = 16.0;

impl Resolver<'_> {
    /// The icon a prop names, if it's here; otherwise asked for (and `None`: nothing drawn yet).
    pub(crate) fn icon_of(&self, p: &Prop, cx: &Cx) -> Option<(String, Arc<Icon>)> {
        if p.is_null() {
            return None;
        }
        // Path data inline: `{ d, viewBox, stroke }`.
        if let (None, serde_json::Value::Object(o)) = (p.as_expr(), &p.0) {
            if let Some(svg) = o.get("svg").and_then(|s| s.as_str()) {
                return self.inline_icon(svg);
            }
            let d = o.get("d").and_then(|d| d.as_str())?;
            let vb = o.get("viewBox").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_f64()).collect::<Vec<_>>()).filter(|v| v.len() == 4).map(|v| Rect::new(v[0], v[1], v[2], v[3]));
            let stroke = o.get("stroke").and_then(|s| s.as_f64());
            return match Icon::from_path(d, vb, stroke) {
                Ok(i) => Some((format!("path:{}", Source::Inline(d.to_string()).key()), Arc::new(i))),
                Err(e) => {
                    self.diag(format!("icon: {e}"));
                    None
                }
            };
        }
        let mut name = self.string(p, cx);
        // A theme icon token names the icon (`$icon.up` → `lucide:arrow-up`).
        if let Some(tok) = name.strip_prefix('$') {
            match self.theme.text(tok) {
                Some(v) => name = v.to_string(),
                None => {
                    self.diag(format!("icon `{name}`: the theme has no `{tok}` token (icon tokens are text: \"{tok}\": \"lucide:…\")"));
                    return None;
                }
            }
        }
        let src = match Source::parse(&name) {
            Some(s) => s,
            None => {
                if !name.is_empty() {
                    self.diag(format!("`{name}` isn't an icon name (a set icon like `heart` or `lucide:heart`, a `.svg` file or URL, or `<svg …>` markup)"));
                }
                return None;
            }
        };
        if let Source::Inline(svg) = &src {
            return self.inline_icon(svg);
        }
        let key = src.key();
        let assets = self.assets?;
        if let Some(i) = assets.icons.get(&key) {
            assets.used.borrow_mut().insert(format!("{}{key}", crate::assets::ICON_PREFIX));
            // What the reader skipped in the file (text, clipping): said in `datars check`.
            for n in &i.notes {
                self.diag(format!("icon `{key}`: {n}"));
            }
            return Some((key, i.clone()));
        }
        if let Some(why) = assets.failed.get(&format!("{}{key}", crate::assets::ICON_PREFIX)) {
            self.diag(format!("icon `{name}`: {why}"));
            return None;
        }
        if !assets.sealed {
            if let Some((n, u)) = icon_request(&src) {
                self.asks.borrow_mut().entry(n).or_insert(u);
            }
        } else {
            self.diag(format!("icon `{name}` isn't in this bundle (it wasn't drawn when it was published)"));
        }
        None
    }

    fn inline_icon(&self, svg: &str) -> Option<(String, Arc<Icon>)> {
        let key = Source::Inline(svg.to_string()).key();
        let cache = self.assets.map(|a| &a.inline);
        if let Some(i) = cache.and_then(|c| c.borrow().get(&key).cloned()) {
            return Some((key, i));
        }
        match datars_icon::parse_svg(svg) {
            Ok(i) => {
                for n in &i.notes {
                    self.diag(format!("inline icon: {n}"));
                }
                let i = Arc::new(i);
                if let Some(c) = cache {
                    c.borrow_mut().insert(key.clone(), i.clone());
                }
                Some((key, i))
            }
            Err(e) => {
                self.diag(format!("inline icon: {e}"));
                None
            }
        }
    }

    /// Inks for an icon's slots: as passed, else the categorical palette in order.
    pub(crate) fn slot_inks(&self, inks: &[Prop], n: usize, cx: &Cx) -> Vec<Ink> {
        (0..n).map(|i| inks.get(i).and_then(|p| self.ink(p, cx)).unwrap_or_else(|| Ink::palette("categorical", i as u32))).collect()
    }

    /// The shapes of an icon's layers in `target` (px), in `ink` and the slots' inks.
    pub(crate) fn icon_shapes(&self, icon: &Icon, target: Rect, ink: &Ink, slots: &[Ink]) -> Vec<Node> {
        let pick = |i: &IconInk| match i {
            IconInk::Current => ink.clone(),
            IconInk::Slot(k) => slots.get(*k as usize).cloned().unwrap_or_else(|| ink.clone()),
        };
        icon.placed(target)
            .into_iter()
            .enumerate()
            .map(|(k, (path, l, width))| {
                let fade = |i: Ink| if l.opacity < 1.0 { i.fade(l.opacity as f32) } else { i };
                let mut n = Node::new(Key::name(&format!("l{k}")), NodeKind::Shape { geom: Geom::path(path), fill: None, stroke: None, markers: None });
                if let NodeKind::Shape { fill, stroke, .. } = &mut n.kind {
                    *fill = l.fill.as_ref().map(|f| Paint::Solid(fade(pick(f))));
                    *stroke = l.stroke.as_ref().map(|s| Stroke { paint: Paint::Solid(fade(pick(&s.ink))), width, dash: None, cap: s.cap, join: s.join, non_scaling: false });
                }
                n
            })
            .collect()
    }

    /// An `icon` node: its layers as shapes in its box (the box placed by `anchor` around the
    /// origin), and the transform that puts it at `at`, turned.
    pub(crate) fn icon_node(&self, ti: &TIcon, cx: &Cx) -> Option<(NodeKind, Affine, Ink)> {
        let (_, icon) = self.icon_of(&ti.icon, cx)?;
        let size = if ti.icon_size.is_null() { self.theme.number("size.icon").unwrap_or(DEFAULT_ICON_SIZE) } else { self.num(&ti.icon_size, cx, DEFAULT_ICON_SIZE) };
        if !(size.is_finite() && size > 0.0) {
            return None;
        }
        // The box is the icon's own aspect, its longer side `size`.
        let vb = icon.view_box;
        let (w, h) = if vb.w >= vb.h { (size, size * vb.h / vb.w.max(1e-9)) } else { (size * vb.w / vb.h.max(1e-9), size) };
        let anchor = if ti.anchor.is_null() { "center".to_string() } else { self.string(&ti.anchor, cx) };
        let o = datars_icon::anchor_offset(&anchor, w, h);
        let ink = self.ink(&ti.ink, cx).unwrap_or_else(|| Ink::token("ink"));
        let slots = self.slot_inks(&ti.inks, icon.slot_count(), cx);
        let kids = self.icon_shapes(&icon, Rect::new(o.x, o.y, w, h), &ink, &slots);
        let at = Vec2::new(self.num(&ti.at[0], cx, 0.0), self.num(&ti.at[1], cx, 0.0));
        let turn = self.num(&ti.rotate, cx, 0.0);
        let place = Affine::translate(at.x, at.y).mul(Affine::rotate(turn.to_radians()));
        Some((NodeKind::Group { children: kids }, place, ink))
    }

    /// A `geom.symbol` whose symbol names an icon: the icon centred on (x, y), `size` its half box
    /// (as a symbol's radius), in the shape's fill (or its stroke's paint).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn icon_symbol(&self, symbol: &Prop, x: f64, y: f64, size: f64, fill: Option<&Ink>, stroke: Option<&Stroke>, cx: &Cx) -> Option<NodeKind> {
        let (_, icon) = self.icon_of(symbol, cx)?;
        let ink = fill.cloned().or_else(|| stroke.and_then(|s| if let Paint::Solid(i) = &s.paint { Some(i.clone()) } else { None })).unwrap_or_else(|| Ink::token("mark"));
        let slots = self.slot_inks(&[], icon.slot_count(), cx);
        let target = Rect::new(x - size, y - size, 2.0 * size, 2.0 * size);
        Some(NodeKind::Group { children: self.icon_shapes(&icon, target, &ink, &slots) })
    }

    /// An icon as instances and markers repeat it.
    pub(crate) fn icon_glyph(&self, p: &Prop, inks: &[Prop], cx: &Cx) -> Option<Arc<IconGlyph>> {
        let (key, icon) = self.icon_of(p, cx)?;
        let slots = self.slot_inks(inks, icon.slot_count(), cx);
        Some(Arc::new(icon.glyph(&key, slots)))
    }

    /// A shape's markers, with icon markers resolved (`{ type: "icon", icon, size }`).
    pub(crate) fn markers(&self, m: &serde_json::Value, cx: &Cx) -> Option<datars_scene::Markers> {
        let one = |v: Option<&serde_json::Value>| -> Option<datars_scene::Marker> {
            let v = v?;
            if v["type"] == "icon" {
                let glyph = self.icon_glyph(&Prop(v.get("icon").cloned().unwrap_or_default()), &[], cx)?;
                let size = v["size"].as_f64().unwrap_or(10.0);
                return Some(datars_scene::Marker::Icon { icon: glyph, size });
            }
            serde_json::from_value(v.clone()).ok()
        };
        Some(datars_scene::Markers { start: one(m.get("start")), end: one(m.get("end")) })
    }

    /// An `image` node: the picture, if it's here (else asked for), and its rect as `fit` says,
    /// with the clip `cover` and `none` cut it to.
    pub(crate) fn image_node(&self, asset: &Prop, rect: &[Prop; 4], fit: Option<&str>, cx: &Cx) -> Option<(NodeKind, Option<Rect>)> {
        let src = self.string(asset, cx);
        if src.is_empty() {
            return None;
        }
        let r = Rect::new(self.num(&rect[0], cx, 0.0), self.num(&rect[1], cx, 0.0), self.num(&rect[2], cx, 0.0), self.num(&rect[3], cx, 0.0));
        let (name, url) = image_request(&src);
        let assets = self.assets?;
        let Some(img) = assets.images.get(&src) else {
            match assets.failed.get(&name) {
                Some(why) => self.diag(format!("picture `{src}`: {why}")),
                None if assets.sealed => self.diag(format!("picture `{src}` isn't in this bundle")),
                None => {
                    self.asks.borrow_mut().entry(name).or_insert(url);
                }
            }
            return None;
        };
        assets.used.borrow_mut().insert(name);
        let (iw, ih) = (img.width as f64, img.height as f64);
        let fit = fit.unwrap_or("contain");
        let c = r.center();
        let (drawn, clip) = match fit {
            "fill" => (r, None),
            "cover" => {
                let s = (r.w / iw).max(r.h / ih);
                (Rect::new(c.x - iw * s / 2.0, c.y - ih * s / 2.0, iw * s, ih * s), Some(r))
            }
            "none" => (Rect::new(c.x - iw / 2.0, c.y - ih / 2.0, iw, ih), Some(r)),
            other => {
                if other != "contain" {
                    self.diag(format!("image `fit: {other}`: expected contain, cover, fill or none"));
                }
                let s = (r.w / iw).min(r.h / ih);
                (Rect::new(c.x - iw * s / 2.0, c.y - ih * s / 2.0, iw * s, ih * s), None)
            }
        };
        Some((NodeKind::Image { asset: Arc::from(src.as_str()), rect: drawn }, clip))
    }
}
