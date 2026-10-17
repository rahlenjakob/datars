//! Accessibility on the desktop: the engine's semantics as an AccessKit tree (VoiceOver on macOS,
//! Narrator/NVDA on Windows, Orca on Linux) — the chart's content, not a picture. Data marks are
//! labelled images, interactive marks buttons (activation runs their click intent), engine-drawn
//! sliders adjustable sliders (increment, decrement, set value → the signal).

use accesskit::{Action, ActionData, ActionRequest, Node, NodeId, Rect, Role, TreeId, TreeInfo, TreeUpdate};
use datars_engine::Engine;

const ROOT: NodeId = NodeId(0);

/// What an AccessKit node stands for, to answer its actions.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// An interactive mark: its key path (for `Engine::activate`).
    Mark(String),
    /// An engine-drawn control: its signal and range.
    Control { signal: String, min: f64, max: f64, step: f64, value: f64 },
    /// A brushable area or a pannable view (`brush` / `pan`): its key path (for `Engine::key`).
    Drag { path: String, kind: &'static str },
}

/// A stable id for a key path (FNV-1a), so a screen reader keeps its place across updates.
fn id_for(s: &str) -> NodeId {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    NodeId(h.max(1))
}

fn rect(r: datars_math::Rect, dpr: f64) -> Rect {
    Rect { x0: r.x * dpr, y0: r.y * dpr, x1: (r.x + r.w) * dpr, y1: (r.y + r.h) * dpr }
}

/// The whole tree for the current frame, in physical pixels (`dpr`), and what each node targets:
/// the window, the chart in it as a figure — named, its summary as its description — and the
/// runtime's reading order under it ([`datars_runtime::readout`]): engine-drawn sliders as
/// adjustable sliders, headings, lines of text (an axis, a legend, a note), data marks with their
/// values, buttons for what a click acts on (a switch, a filtering bar, a select's box).
///
/// `said` is what the reader's last interaction changed (`Engine::take_announcement`): a polite
/// live region under the figure says it.
pub fn tree(engine: &mut Engine, title: &str, dpr: f64, said: &str) -> (TreeUpdate, Vec<(NodeId, Target)>) {
    let vp = engine.viewport();
    let items = engine.semantic_items();
    let name = engine.accessible_name();
    let summary = engine.accessible_summary();
    let locale = engine.doc().locale.clone();
    let readout = datars_runtime::readout::readout_in(&locale, &name, &summary, &items);
    let mut nodes: Vec<(NodeId, Node)> = Vec::new();
    let mut targets = Vec::new();
    let whole = Rect { x0: 0.0, y0: 0.0, x1: vp.width * dpr, y1: vp.height * dpr };
    let mut root = Node::new(Role::Window);
    root.set_label(title);
    root.set_bounds(whole);
    let figure_id = id_for("figure");
    let mut figure = Node::new(Role::Figure);
    if let Some(chart) = readout.first() {
        figure.set_label(chart.label.as_str());
        if !chart.value.is_empty() {
            figure.set_description(chart.value.as_str());
        }
    }
    figure.set_bounds(whole);
    let mut kids = Vec::new();
    // Engine-drawn sliders first (selects are their drawn box, a button below).
    for c in engine.controls().into_iter().filter(|c| c.kind == "slider") {
        let id = id_for(&format!("slider:{}", c.signal));
        let mut n = Node::new(Role::Slider);
        n.set_label(c.label.as_str());
        n.set_numeric_value(c.value);
        n.set_min_numeric_value(c.min);
        n.set_max_numeric_value(c.max);
        if c.step > 0.0 {
            n.set_numeric_value_step(c.step);
        }
        // What the drawn control says ("32,000"), not the raw number.
        if let Some((_, said)) = c.label.rsplit_once(": ") {
            n.set_value(said);
        }
        for a in [Action::Increment, Action::Decrement, Action::SetValue] {
            n.add_action(a);
        }
        let [x, y, w, h] = c.rect;
        if w > 0.0 && h > 0.0 {
            n.set_bounds(rect(datars_math::Rect::new(x, y, w, h), dpr));
        }
        targets.push((id, Target::Control { signal: c.signal.clone(), min: c.min, max: c.max, step: c.step, value: c.value }));
        kids.push(id);
        nodes.push((id, n));
    }
    for (i, r) in readout.iter().enumerate().skip(1) {
        let id = id_for(&if r.path.is_empty() { format!("{}:{}#{i}", r.kind, r.label) } else { r.path.clone() });
        let mut n = Node::new(match r.kind {
            "button" => Role::Button,
            "checkbox" => Role::CheckBox,
            "switch" => Role::Switch,
            "radio" => Role::RadioButton,
            "heading" => Role::Heading,
            "brush" | "pan" => Role::Group,
            _ => Role::Label,
        });
        // What a pointer drags, for a screen reader's actions: Click starts (and keeps) a
        // selection or shows all of a view, Increment and Decrement extend it or zoom, the scroll
        // actions move a view; how, said as its description.
        if matches!(r.kind, "brush" | "pan") {
            let kind: &'static str = if r.kind == "brush" { "brush" } else { "pan" };
            n.set_role_description(datars_engine::phrases::phrase(&locale, kind, &[]));
            n.set_description(datars_engine::phrases::phrase(&locale, &format!("{kind}-hint"), &[]));
            for a in [Action::Click, Action::Increment, Action::Decrement] {
                n.add_action(a);
            }
            if kind == "pan" {
                for a in [Action::ScrollLeft, Action::ScrollRight, Action::ScrollUp, Action::ScrollDown] {
                    n.add_action(a);
                }
            }
            targets.push((id, Target::Drag { path: r.path.clone(), kind }));
        }
        // A checkbox, switch or radio says its state the platform's way.
        if let Some(c) = r.checked {
            n.set_toggled(accesskit::Toggled::from(c));
        }
        // A data mark says its name and its value ("Housing, 10,240").
        if r.value.is_empty() {
            n.set_label(r.label.as_str());
        } else {
            n.set_label(format!("{}, {}", r.label, r.value));
        }
        if matches!(r.kind, "button" | "checkbox" | "switch" | "radio") {
            n.add_action(Action::Click);
            targets.push((id, Target::Mark(r.path.clone())));
        }
        if let Some([x, y, w, h]) = r.rect {
            n.set_bounds(rect(datars_math::Rect::new(x, y, w, h), dpr));
        }
        kids.push(id);
        nodes.push((id, n));
    }
    // What the last interaction changed, said politely (not cutting the reader off).
    let live = id_for("live");
    let mut l = Node::new(Role::Label);
    l.set_live(accesskit::Live::Polite);
    l.set_value(said);
    kids.push(live);
    nodes.push((live, l));
    figure.set_children(kids);
    root.set_children(vec![figure_id]);
    nodes.insert(0, (figure_id, figure));
    nodes.insert(0, (ROOT, root));
    (TreeUpdate { nodes, tree: Some(TreeInfo::new(ROOT)), tree_id: TreeId::ROOT, focus: ROOT }, targets)
}

/// Carry out a screen reader's request. Returns whether the chart changed.
pub fn act(engine: &mut Engine, targets: &[(NodeId, Target)], req: &ActionRequest) -> bool {
    let Some((_, t)) = targets.iter().find(|(id, _)| *id == req.target_node) else { return false };
    match (t, req.action) {
        (Target::Mark(path), Action::Click) => engine.activate(path),
        (Target::Drag { path, kind }, a) => {
            let brush = *kind == "brush";
            match a {
                Action::Click => engine.key(path, if brush { "Enter" } else { "0" }),
                Action::Increment if brush => engine.key(path, "ArrowRight") || engine.key(path, "Enter"),
                Action::Increment => engine.key(path, "+"),
                Action::Decrement if brush => engine.key(path, "ArrowLeft") || engine.key(path, "Enter"),
                Action::Decrement => engine.key(path, "-"),
                Action::ScrollLeft => engine.key(path, "ArrowLeft"),
                Action::ScrollRight => engine.key(path, "ArrowRight"),
                Action::ScrollUp => engine.key(path, "ArrowUp"),
                Action::ScrollDown => engine.key(path, "ArrowDown"),
                _ => false,
            }
        }
        (Target::Control { signal, min, max, step, value }, a) => {
            let step = if *step > 0.0 { *step } else { (max - min) / 20.0 };
            let v = match (a, &req.data) {
                (Action::Increment, _) => value + step,
                (Action::Decrement, _) => value - step,
                (Action::SetValue, Some(ActionData::NumericValue(v))) => *v,
                _ => return false,
            };
            engine.set_signal(signal, datars_engine::SignalValue::Num(v.clamp(*min, *max)));
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(name: &str) -> Engine {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples").join(name);
        let json = std::fs::read_to_string(dir.join("doc.json")).unwrap();
        let mut e = datars_headless::load_at(&json, Some(&dir)).unwrap();
        e.frame(0.0);
        e
    }

    #[test]
    fn sliders_are_adjustable_and_marks_are_buttons() {
        // budget: an engine-drawn slider over bars.
        let mut e = load("budget");
        let (t, targets) = tree(&mut e, "Budget", 2.0, "");
        let slider = t.nodes.iter().find(|(_, n)| n.role() == Role::Slider).expect("a slider");
        assert_eq!(slider.1.numeric_value(), Some(32000.0));
        assert_eq!(slider.1.max_numeric_value(), Some(80000.0));
        assert!(slider.1.label().unwrap().starts_with("Monthly income"));
        let b = slider.1.bounds().unwrap();
        assert!(b.x1 - b.x0 > 500.0, "framed by the whole control, in physical px: {b:?}");
        assert!(t.nodes.iter().any(|(_, n)| n.role() == Role::Label && n.label() == Some("Housing, 10,240")), "data marks say their name and value");
        assert_eq!(slider.1.value(), Some("32,000"), "the slider says its value as drawn");
        let figure = t.nodes.iter().find(|(_, n)| n.role() == Role::Figure).expect("the chart");
        assert_eq!(figure.1.label(), Some("Where a monthly income goes"));
        assert!(figure.1.description().is_some_and(|d| d.contains("6 data points")), "{:?}", figure.1.description());
        assert!(t.nodes.iter().any(|(_, n)| n.role() == Role::Label && n.label().is_some_and(|l| l.starts_with("y axis: Housing, Food"))), "an axis is one line");
        let req = ActionRequest { action: Action::Increment, target_tree: TreeId::ROOT, target_node: slider.0, data: None };
        assert!(act(&mut e, &targets, &req));
        assert_eq!(e.signal("income"), Some(datars_engine::SignalValue::Num(33000.0)));

        // dashboard: bars that filter on click are buttons, and clicking one filters.
        let mut e = load("dashboard");
        let (t, targets) = tree(&mut e, "Dashboard", 1.0, "");
        let south = t.nodes.iter().find(|(_, n)| n.role() == Role::Button && n.label().is_some_and(|l| l.starts_with("South"))).expect("a button");
        assert!(act(&mut e, &targets, &ActionRequest { action: Action::Click, target_tree: TreeId::ROOT, target_node: south.0, data: None }));
        assert_eq!(e.signal("selected"), Some(datars_engine::SignalValue::Str("South".into())));

        // A switch is a button that flips it (every control used to become a slider, taking the
        // next slider's range — a switch got none, and no click).
        let doc = r#"{"datars":1,"size":{"width":300,"height":60},"signals":{"on":{"type":"bool","default":false}},
            "scene":{"kind":"use","recipe":"@datars/std/toggle","params":{"signal":"on","label":"Show averages"}}}"#;
        let mut e = datars_headless::load_at(doc, None).unwrap();
        e.frame(0.0);
        let (t, targets) = tree(&mut e, "Toggle", 1.0, "");
        assert!(!t.nodes.iter().any(|(_, n)| n.role() == Role::Slider), "no slider without one");
        let switch = t.nodes.iter().find(|(_, n)| n.role() == Role::Switch && n.label() == Some("Show averages")).expect("the switch is a switch");
        assert_eq!(switch.1.toggled(), Some(accesskit::Toggled::False), "off, said by the platform");
        assert!(act(&mut e, &targets, &ActionRequest { action: Action::Click, target_tree: TreeId::ROOT, target_node: switch.0, data: None }));
        assert_eq!(e.signal("on"), Some(datars_engine::SignalValue::Bool(true)));
        e.frame(1.0);
        let (t, _) = tree(&mut e, "Toggle", 1.0, "");
        assert!(t.nodes.iter().any(|(_, n)| n.role() == Role::Switch && n.toggled() == Some(accesskit::Toggled::True)), "and on after the click");
        // Every non-root node hangs off the tree exactly once.
        let children: usize = t.nodes.iter().map(|(_, n)| n.children().len()).sum();
        assert_eq!(children, t.nodes.len() - 1);
    }

    #[test]
    fn a_brush_is_operable_and_says_what_it_selected() {
        // prices: the overview plot is brushable.
        let mut e = load("prices");
        let (t, targets) = tree(&mut e, "Prices", 1.0, "");
        let brush = t.nodes.iter().find(|(_, n)| n.role() == Role::Group && n.role_description() == Some("range selection")).expect("the brush");
        assert!(brush.1.label().is_some_and(|l| l.contains("nothing selected")), "{:?}", brush.1.label());
        for action in [Action::Click, Action::Increment, Action::Click] {
            assert!(act(&mut e, &targets, &ActionRequest { action, target_tree: TreeId::ROOT, target_node: brush.0, data: None }));
        }
        let said = e.take_announcement().unwrap_or_default();
        assert!(said.starts_with("Selected 1 to 100."), "{said}");
        e.frame(1.0);
        let (t, _) = tree(&mut e, "Prices", 1.0, &said);
        let live = t.nodes.iter().find(|(_, n)| n.live() == Some(accesskit::Live::Polite)).expect("a live region");
        assert_eq!(live.1.value(), Some(said.as_str()));
    }
}
