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

fn role_of(role: &str, actionable: bool) -> Role {
    if actionable {
        return Role::Button;
    }
    match role {
        "title" | "annotation" | "tick" => Role::Label,
        "datum" | "region" => Role::Image,
        "legend-item" => Role::ListItem,
        "legend" => Role::List,
        _ => Role::Group,
    }
}

/// The whole tree for the current frame, in physical pixels (`dpr`), and what each node targets.
pub fn tree(engine: &mut Engine, title: &str, dpr: f64) -> (TreeUpdate, Vec<(NodeId, Target)>) {
    let vp = engine.viewport();
    let items = engine.semantic_items();
    let controls = engine.controls();
    let mut nodes: Vec<(NodeId, Node)> = Vec::new();
    let mut targets = Vec::new();
    let mut root = Node::new(Role::Window);
    root.set_label(title);
    root.set_bounds(Rect { x0: 0.0, y0: 0.0, x1: vp.width * dpr, y1: vp.height * dpr });
    // Children by depth: a stack of (depth, index into `nodes`) for the open ancestors.
    let mut root_children = Vec::new();
    let mut stack: Vec<(usize, usize)> = Vec::new();
    let mut controls_left = controls.iter();
    for (i, it) in items.iter().enumerate() {
        let key = if it.path.is_empty() { format!("{}#{i}", it.label) } else { it.path.clone() };
        let id = id_for(&key);
        let mut n;
        if it.role == "control" {
            n = Node::new(Role::Slider);
            if let Some(c) = controls_left.next() {
                n.set_numeric_value(c.value);
                n.set_min_numeric_value(c.min);
                n.set_max_numeric_value(c.max);
                if c.step > 0.0 {
                    n.set_numeric_value_step(c.step);
                }
                for a in [Action::Increment, Action::Decrement, Action::SetValue] {
                    n.add_action(a);
                }
                targets.push((id, Target::Control { signal: c.signal.clone(), min: c.min, max: c.max, step: c.step, value: c.value }));
            }
        } else {
            n = Node::new(role_of(&it.role, it.actionable));
            if it.actionable {
                n.add_action(Action::Click);
                targets.push((id, Target::Mark(it.path.clone())));
            }
        }
        n.set_label(it.label.as_str());
        if !it.rect.is_empty() {
            n.set_bounds(rect(it.rect, dpr));
        }
        while stack.last().is_some_and(|&(d, _)| d >= it.depth) {
            stack.pop();
        }
        match stack.last() {
            Some(&(_, parent)) => nodes[parent].1.push_child(id),
            None => root_children.push(id),
        }
        stack.push((it.depth, nodes.len()));
        nodes.push((id, n));
    }
    root.set_children(root_children);
    nodes.insert(0, (ROOT, root));
    (TreeUpdate { nodes, tree: Some(TreeInfo::new(ROOT)), tree_id: TreeId::ROOT, focus: ROOT }, targets)
}

/// Carry out a screen reader's request. Returns whether the chart changed.
pub fn act(engine: &mut Engine, targets: &[(NodeId, Target)], req: &ActionRequest) -> bool {
    let Some((_, t)) = targets.iter().find(|(id, _)| *id == req.target_node) else { return false };
    match (t, req.action) {
        (Target::Mark(path), Action::Click) => engine.activate(path),
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
        let (t, targets) = tree(&mut e, "Budget", 2.0);
        let slider = t.nodes.iter().find(|(_, n)| n.role() == Role::Slider).expect("a slider");
        assert_eq!(slider.1.numeric_value(), Some(32000.0));
        assert_eq!(slider.1.max_numeric_value(), Some(80000.0));
        assert!(slider.1.label().unwrap().starts_with("Monthly income"));
        let b = slider.1.bounds().unwrap();
        assert!(b.x1 - b.x0 > 500.0, "framed by the whole control, in physical px: {b:?}");
        assert!(t.nodes.iter().any(|(_, n)| n.role() == Role::Image && n.label().is_some_and(|l| l.starts_with("Housing"))), "data marks are labelled");
        let req = ActionRequest { action: Action::Increment, target_tree: TreeId::ROOT, target_node: slider.0, data: None };
        assert!(act(&mut e, &targets, &req));
        assert_eq!(e.signal("income"), Some(datars_engine::SignalValue::Num(33000.0)));

        // dashboard: bars that filter on click are buttons, and clicking one filters.
        let mut e = load("dashboard");
        let (t, targets) = tree(&mut e, "Dashboard", 1.0);
        let south = t.nodes.iter().find(|(_, n)| n.role() == Role::Button && n.label().is_some_and(|l| l.starts_with("South"))).expect("a button");
        assert!(act(&mut e, &targets, &ActionRequest { action: Action::Click, target_tree: TreeId::ROOT, target_node: south.0, data: None }));
        assert_eq!(e.signal("selected"), Some(datars_engine::SignalValue::Str("South".into())));
        // Every non-root node hangs off the tree exactly once.
        let children: usize = t.nodes.iter().map(|(_, n)| n.children().len()).sum();
        assert_eq!(children, t.nodes.len() - 1);
    }
}
