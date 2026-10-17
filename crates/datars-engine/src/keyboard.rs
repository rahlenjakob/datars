//! The keyboard's way into what a pointer drags, and what an interaction changed, said.
//!
//! - A brushable area: Enter starts a selection at the start of its scale (or edits the one there
//!   is), the arrow keys extend it a step at a time (Shift and the arrows move it), Enter keeps
//!   it, Escape puts back what was there before (and, with no selection under way, clears the
//!   one there is). Steps are nice numbers on a continuous scale (a year, 0.5, a month on a time
//!   scale) and one band on a band scale.
//! - A pannable view: the arrow keys move it a tenth of its size, `+` and `-` zoom about its
//!   middle, `0` shows the whole again.
//! - What a click, a key or an activation changed is put into words for the reader's live region
//!   ([`Engine::take_announcement`]): the clicked mark's new label, the parts whose label
//!   changed (a total, a status line), and the data shown when that changed ("Now 3 data
//!   points, from 16 to 107.") — in the document's language.
//!
//! Hosts find these areas among the semantic items (role `control`, kind `brush` or `pan`) and
//! pass keys by the item's path ([`Engine::key`]); keys are recorded and replay like the rest.

use crate::phrases::{counted, phrase};
use crate::resolve::{BoundAction, BrushMap};
use crate::{env, pick, session, Engine, SemanticItem, Value};
use datars_scene::{KeyPath, NodeKind};

/// A selection being made with the keyboard: where (the brush's path), what was there before it
/// started (Escape puts it back), the semantics then (to say what changed), and its ends — data
/// units on a continuous scale, band indices on a band scale.
#[derive(Clone, Debug)]
pub(crate) struct KeyBrush {
    path: KeyPath,
    saved: Vec<(String, Value)>,
    before: (Vec<SemanticItem>, usize),
    lo: f64,
    hi: f64,
}

/// A nice step for `span`: 1, 2 or 5 × a power of ten, about a tenth of it; on a time scale (days)
/// a day, a week, a month, a quarter or years.
fn nice_step(span: f64, time: bool) -> f64 {
    if !(span.is_finite() && span > 0.0) {
        return 1.0;
    }
    if time {
        let want = span / 12.0;
        return [1.0, 7.0, 30.4375, 91.3125, 365.25, 730.5, 1826.25, 3652.5, 18262.5].into_iter().find(|s| *s >= want).unwrap_or(36525.0);
    }
    let raw = span / 10.0;
    let mag = datars_math::m::pow(10.0, datars_math::m::log10(raw).floor());
    let f = raw / mag;
    mag * if f < 1.5 { 1.0 } else if f < 3.5 { 2.0 } else if f < 7.5 { 5.0 } else { 10.0 }
}

/// A value on a continuous brush as a reader should hear it: a date in full, a number to the
/// step's precision.
fn say_value(v: f64, step: f64, time: bool, locale: &str) -> String {
    if time {
        return datars_text::format::date(v.round() as i64, "%-d %b %Y", locale);
    }
    let decimals = if step >= 1.0 { 0 } else { ((-datars_math::m::log10(step)).ceil() as usize).min(6) };
    let group = if v.abs() >= 10_000.0 { "," } else { "" };
    datars_text::format::number(v, &format!("{group}.{decimals}f"), locale)
}

/// Labels as one line: all of a few, the first five and a count of the rest.
fn list(labels: &[String], locale: &str) -> String {
    if labels.len() <= 6 {
        return labels.join(", ");
    }
    format!("{}, {}", labels[..5].join(", "), phrase(locale, "more", &[("n", &(labels.len() - 5).to_string())]))
}

fn is_data(i: &SemanticItem) -> bool {
    (i.role == "datum" || i.role == "region") && !i.missing
}

impl Engine {
    /// A key on the keyboard-operable area at `path` (a semantic item of kind `brush` or `pan`):
    /// `Enter`, `Escape`, `ArrowLeft` … (`Shift+ArrowLeft` …), `+`, `-`, `0`, as the web names
    /// them. Returns whether the area used it (hosts let the rest do what they do: Tab moves on).
    /// What it did is said by [`Engine::take_announcement`].
    pub fn key(&mut self, path: &str, key: &str) -> bool {
        self.rec(session::Input::Key { path: path.to_string(), key: key.to_string() });
        let Some(scene) = self.current_scene() else { return false };
        let Some((kp, act)) = self.drag_areas(&scene).into_iter().map(|(p, a, ..)| (p, a)).find(|(p, _)| format!("{p}") == path) else { return false };
        let (key, shift) = match key.strip_prefix("Shift+") {
            Some(k) => (k, true),
            None => (key, false),
        };
        match act {
            BoundAction::Brush { signal, axis, map } => self.key_brush(kp, &signal, axis, &map, key, shift),
            BoundAction::Explore { prefix, fit, zoom } => {
                let Some((node, _)) = pick::node_at(&scene, &kp) else { return false };
                let NodeKind::View { viewport, camera: Some(cam), .. } = &node.kind else { return false };
                let (vw, vh, cam) = (viewport.w, viewport.h, *cam);
                let loc = self.doc.locale.clone();
                let factor = cam.zoom / fit.zoom;
                let (step_x, step_y) = (vw * 0.1 / cam.zoom, vh * 0.1 / cam.zoom);
                let (kv, said) = match key {
                    "ArrowLeft" => (vec![(format!("{prefix}.x"), Value::Num(cam.x - step_x)), (format!("{prefix}.y"), Value::Num(cam.y))], phrase(&loc, "pan-left", &[])),
                    "ArrowRight" => (vec![(format!("{prefix}.x"), Value::Num(cam.x + step_x)), (format!("{prefix}.y"), Value::Num(cam.y))], phrase(&loc, "pan-right", &[])),
                    "ArrowUp" => (vec![(format!("{prefix}.x"), Value::Num(cam.x)), (format!("{prefix}.y"), Value::Num(cam.y - step_y))], phrase(&loc, "pan-up", &[])),
                    "ArrowDown" => (vec![(format!("{prefix}.x"), Value::Num(cam.x)), (format!("{prefix}.y"), Value::Num(cam.y + step_y))], phrase(&loc, "pan-down", &[])),
                    "+" | "=" | "-" | "_" => {
                        let f = if matches!(key, "+" | "=") { factor * 1.25 } else { factor / 1.25 };
                        let f = f.clamp(zoom.0, zoom.1);
                        let z = datars_text::format::number(f, ".1f", &loc);
                        (vec![(format!("{prefix}.zoom"), Value::Num(f)), (format!("{prefix}.x"), Value::Num(cam.x)), (format!("{prefix}.y"), Value::Num(cam.y))], phrase(&loc, "zoom", &[("z", &z)]))
                    }
                    "0" | "Home" => (vec![(format!("{prefix}.zoom"), Value::Num(1.0)), (format!("{prefix}.x"), Value::Num(fit.x)), (format!("{prefix}.y"), Value::Num(fit.y))], phrase(&loc, "pan-reset", &[])),
                    _ => return false,
                };
                self.set_signals(kv);
                self.announcement = Some(format!("{said}."));
                true
            }
            _ => false,
        }
    }

    /// What the last interaction (a click, an activation, a key) changed, in words — once: hosts
    /// put it in their live region (politely) and it's gone.
    pub fn take_announcement(&mut self) -> Option<String> {
        self.announcement.take()
    }

    fn key_brush(&mut self, path: KeyPath, signal: &str, axis: usize, map: &BrushMap, key: &str, shift: bool) -> bool {
        let loc = self.doc.locale.clone();
        let editing = self.key_brush.as_ref().is_some_and(|b| b.path == path);
        let names = [format!("{signal}.active"), format!("{signal}.lo"), format!("{signal}.hi"), signal.to_string()];
        match key {
            "Enter" | " " if editing => {
                let Some(b) = self.key_brush.take() else { return false };
                let range = self.brush_range(signal, map).unwrap_or_else(|| phrase(&loc, "brush-none", &[]));
                let mut said = vec![phrase(&loc, "brush-done", &[("range", &range)])];
                let after = self.semantic_items();
                said.extend(self.change_words((&b.before.0, b.before.1), &after, None));
                self.announcement = Some(said.into_iter().map(|s| format!("{s}.")).collect::<Vec<_>>().join(" "));
                true
            }
            "Enter" | " " => {
                let now = self.all_signals();
                let saved: Vec<(String, Value)> = names.iter().map(|n| (n.clone(), now.get(n).cloned().unwrap_or(Value::Null))).collect();
                let active = matches!(now.get(&names[0]), Some(Value::Bool(true)));
                let (lo, hi) = match map {
                    BrushMap::Continuous { v, time, .. } => {
                        let (min, max) = (v.iter().copied().fold(f64::INFINITY, f64::min), v.iter().copied().fold(f64::NEG_INFINITY, f64::max));
                        let step = nice_step(max - min, *time);
                        let num = |n: &str| match now.get(n) {
                            Some(Value::Num(x)) if x.is_finite() => Some(*x),
                            _ => None,
                        };
                        match (active, num(&names[1]), num(&names[2])) {
                            (true, Some(a), Some(b)) => (a, b),
                            // From the start of the scale to the first nice value past it.
                            _ => (min, (((min / step).floor() + 1.0) * step).min(max)),
                        }
                    }
                    BrushMap::Bands { centers } => {
                        let chosen = now.get(signal).map(env::decode_keyset).unwrap_or_default();
                        let idx: Vec<usize> = centers.iter().enumerate().filter(|(_, (_, k))| chosen.contains(k)).map(|(i, _)| i).collect();
                        match (idx.first(), idx.last()) {
                            (Some(a), Some(b)) if active => (*a as f64, *b as f64),
                            _ => (0.0, 0.0),
                        }
                    }
                };
                let before = (self.semantic_items(), self.data_count());
                self.key_brush = Some(KeyBrush { path, saved, before, lo, hi });
                self.apply_key_brush(signal, map);
                let range = self.brush_range(signal, map).unwrap_or_default();
                self.announcement = Some(format!("{}.", phrase(&loc, "brush-start", &[("range", &range)])));
                true
            }
            "Escape" if editing => {
                let Some(b) = self.key_brush.take() else { return false };
                self.set_signals_now(b.saved);
                self.announcement = Some(format!("{}.", phrase(&loc, "brush-cancel", &[])));
                true
            }
            "Escape" | "Delete" | "Backspace" => {
                if !matches!(self.all_signals().get(&names[0]), Some(Value::Bool(true))) {
                    return false;
                }
                let before = (self.semantic_items(), self.data_count());
                self.set_signals(vec![(names[0].clone(), Value::Bool(false)), (names[1].clone(), Value::Num(f64::NAN)), (names[2].clone(), Value::Num(f64::NAN)), (signal.to_string(), env::encode_keyset(&[]))]);
                let after = self.semantic_items();
                let mut said = vec![phrase(&loc, "brush-clear", &[])];
                said.extend(self.change_words((&before.0, before.1), &after, None));
                self.announcement = Some(said.into_iter().map(|s| format!("{s}.")).collect::<Vec<_>>().join(" "));
                true
            }
            "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown" if editing => {
                let Some(mut b) = self.key_brush.take() else { return false };
                match map {
                    BrushMap::Continuous { v, time, .. } => {
                        let (min, max) = (v.iter().copied().fold(f64::INFINITY, f64::min), v.iter().copied().fold(f64::NEG_INFINITY, f64::max));
                        let step = nice_step(max - min, *time);
                        // Up and right are more; on a y axis too (values grow upward).
                        let d: f64 = if matches!(key, "ArrowRight" | "ArrowUp") { step } else { -step };
                        if shift {
                            let d = d.clamp(min - b.lo, max - b.hi);
                            b.lo += d;
                            b.hi += d;
                        } else {
                            // On to the next nice value (a step from one, from wherever it was).
                            let next = ((b.hi / step).round() * step + d).clamp(b.lo + (step / 2.0).min(max - b.lo), max);
                            b.hi = if next <= b.lo { (b.lo + step).min(max) } else { next };
                        }
                    }
                    BrushMap::Bands { centers } => {
                        // The next band along: right, or down a vertical list.
                        let d: f64 = if matches!(key, "ArrowRight" | "ArrowDown") { 1.0 } else { -1.0 };
                        let last = centers.len().saturating_sub(1) as f64;
                        if shift {
                            let d = d.clamp(-b.lo, last - b.hi);
                            b.lo += d;
                            b.hi += d;
                        } else {
                            b.hi = (b.hi + d).clamp(b.lo, last);
                        }
                    }
                }
                self.key_brush = Some(b);
                self.apply_key_brush(signal, map);
                let _ = axis;
                self.announcement = Some(format!("{}.", self.brush_range(signal, map).unwrap_or_default()));
                true
            }
            _ => false,
        }
    }

    /// The keyboard selection's ends into the brush's signals, as a drag would set them.
    fn apply_key_brush(&mut self, signal: &str, map: &BrushMap) {
        let Some(b) = &self.key_brush else { return };
        let mut kv = vec![(format!("{signal}.active"), Value::Bool(true))];
        match map {
            BrushMap::Continuous { .. } => {
                kv.push((format!("{signal}.lo"), Value::Num(b.lo.min(b.hi))));
                kv.push((format!("{signal}.hi"), Value::Num(b.lo.max(b.hi))));
            }
            BrushMap::Bands { centers } => {
                let (a, z) = (b.lo.min(b.hi) as usize, b.lo.max(b.hi) as usize);
                let keys: Vec<String> = centers.iter().enumerate().filter(|(i, _)| *i >= a && *i <= z).map(|(_, (_, k))| k.clone()).collect();
                kv.push((signal.to_string(), env::encode_keyset(&keys)));
            }
        }
        self.set_signals_now(kv);
    }

    /// What a brush selects now, in words ("2017 to 2019", "Apples, Pears, Plums"),
    /// `None` when nothing is.
    pub(crate) fn brush_range(&self, signal: &str, map: &BrushMap) -> Option<String> {
        let now = self.all_signals();
        if !matches!(now.get(&format!("{signal}.active")), Some(Value::Bool(true))) {
            return None;
        }
        let loc = &self.doc.locale;
        match map {
            BrushMap::Continuous { v, time, .. } => {
                let (Some(Value::Num(lo)), Some(Value::Num(hi))) = (now.get(&format!("{signal}.lo")), now.get(&format!("{signal}.hi"))) else { return None };
                if !(lo.is_finite() && hi.is_finite()) {
                    return None;
                }
                let (min, max) = (v.iter().copied().fold(f64::INFINITY, f64::min), v.iter().copied().fold(f64::NEG_INFINITY, f64::max));
                let step = nice_step(max - min, *time);
                Some(phrase(loc, "brush-range", &[("lo", &say_value(*lo, step, *time, loc)), ("hi", &say_value(*hi, step, *time, loc))]))
            }
            BrushMap::Bands { centers } => {
                let chosen = now.get(signal).map(env::decode_keyset).unwrap_or_default();
                // In the order they're drawn, by their names where the keys have them.
                let keys: Vec<String> = centers.iter().filter(|(_, k)| chosen.contains(k)).map(|(_, k)| self.key_name(k)).collect();
                if keys.is_empty() {
                    return None;
                }
                Some(list(&keys, loc))
            }
        }
    }

    /// A key's display name from the document's `keys` (`S` → "Social Democrats"), else itself.
    fn key_name(&self, k: &str) -> String {
        self.doc.keys.get(k).and_then(|m| m.name.clone()).unwrap_or_else(|| k.to_string())
    }

    /// The keyboard-operable areas (a brush, a pannable view) as semantic items — role `control`,
    /// kind `brush` or `pan`, actionable, labelled by the part they're in and what they hold now
    /// ("Seats by party: nothing selected") — each placed right after the item it's inside.
    pub(crate) fn drag_items(&self, resolved: &crate::Resolved, scene: &datars_scene::Scene, items: &mut Vec<SemanticItem>) {
        let loc = self.doc.locale.clone();
        for (path, act, _, area) in Engine::drag_areas_in(resolved, scene) {
            let (kind, state) = match &act {
                BoundAction::Brush { signal, map, .. } => ("brush", self.brush_range(signal, map).unwrap_or_else(|| phrase(&loc, "brush-none", &[]))),
                BoundAction::Explore { .. } => ("pan", String::new()),
                _ => continue,
            };
            let p = format!("{path}");
            let Some((_, xf)) = pick::node_at(scene, &path) else { continue };
            // Inside the innermost item whose path holds it (or that is it).
            let parent = items.iter().enumerate().filter(|(_, i)| p.starts_with(&i.path) && (i.path.is_empty() || p == i.path || p[i.path.len()..].starts_with('/'))).max_by_key(|(_, i)| i.path.len()).map(|(k, _)| k);
            let what = parent.map(|k| items[k].label.trim().to_string()).filter(|l| !l.is_empty()).unwrap_or_else(|| phrase(&loc, kind, &[]));
            let label = if state.is_empty() { format!("{what}, {}", phrase(&loc, kind, &[])) } else { format!("{what}, {}: {state}", phrase(&loc, kind, &[])) };
            let item = SemanticItem { role: "control".into(), label, depth: parent.map(|k| items[k].depth + 1).unwrap_or(0), rect: crate::bounds::transform_rect(area, &xf), path: p, actionable: true, value: None, kind: Some(kind.into()), checked: None, missing: false };
            let at = parent.map(|k| k + 1).unwrap_or(items.len());
            items.insert(at, item);
        }
    }

    /// Run an interaction (a click's or an activation's action) and say what it changed — unless
    /// it moved the story on (the step's narration says that).
    pub(crate) fn reported(&mut self, what: Option<String>, run: impl FnOnce(&mut Engine)) {
        let before = (self.semantic_items(), self.data_count());
        let state = self.state_index();
        run(self);
        if self.state_index() != state {
            return;
        }
        let after = self.semantic_items();
        let words = self.change_words((&before.0, before.1), &after, what.as_deref());
        self.announcement = (!words.is_empty()).then(|| words.into_iter().map(|s| format!("{s}.")).collect::<Vec<_>>().join(" "));
    }

    /// What changed between two readings of the semantics, in words: the item at `what` (the one
    /// clicked) if its label changed — not a checkbox's or a switch's, whose state the reader's own
    /// words say — then the parts (groups, headings) whose label changed, then the data shown.
    /// (`before`: the items then, and how many data marks were drawn — past the items listed.)
    fn change_words(&mut self, before: (&[SemanticItem], usize), after: &[SemanticItem], what: Option<&str>) -> Vec<String> {
        let (before, drawn_before) = before;
        let mut out: Vec<String> = Vec::new();
        let was = |p: &str| before.iter().find(|i| i.path == p && !p.is_empty());
        if let Some(p) = what {
            if let (Some(b), Some(a)) = (was(p), after.iter().find(|i| i.path == p)) {
                if a.label != b.label && a.kind.is_none() && !a.label.trim().is_empty() {
                    out.push(a.label.trim().to_string());
                }
            }
        }
        for a in after.iter().filter(|i| matches!(i.role.as_str(), "group" | "heading" | "title") && Some(i.path.as_str()) != what) {
            if out.len() >= 3 {
                break;
            }
            if let Some(b) = was(&a.path) {
                if b.label != a.label && !a.label.trim().is_empty() {
                    out.push(a.label.trim().to_string());
                }
            }
        }
        // Every mark drawn (a line's points past the ones listed), and what the items say.
        let missing = |items: &[SemanticItem]| items.iter().filter(|i| (i.role == "datum" || i.role == "region") && i.missing).count();
        let n0 = drawn_before.saturating_sub(missing(before)).max(before.iter().filter(|i| is_data(i)).count());
        let n1 = self.data_count().saturating_sub(missing(after)).max(after.iter().filter(|i| is_data(i)).count());
        if n0 != n1 {
            let loc = self.doc.locale.clone();
            let shown = self.data_sentence(after).unwrap_or_else(|| counted(&loc, "points", 0, &datars_text::format::number(0.0, ",.0f", &loc)));
            out.push(phrase(&loc, "now", &[("what", &shown)]));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_are_nice() {
        assert_eq!(nice_step(8.0, false), 1.0);
        assert_eq!(nice_step(100.0, false), 10.0);
        assert_eq!(nice_step(2.5, false), 0.2);
        assert_eq!(nice_step(3650.0, true), 365.25);
        assert_eq!(say_value(2018.0, 1.0, false, "en"), "2018");
        assert_eq!(say_value(0.5, 0.1, false, "sv"), "0,5");
    }
}
