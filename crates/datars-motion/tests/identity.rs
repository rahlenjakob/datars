//! Keys are identity across recipes: data marks the path matcher leaves unpaired pair by their own
//! key — when that key identifies one data mark — so a bar becomes its slice with no motion rule.

mod common;
use common::*;
use datars_motion::{plan, MotionRules};
use datars_scene::{Node, Role, Semantics};

fn datum(n: Node) -> Node {
    n.semantics(Semantics::new(Role::Datum, "d"))
}

#[test]
fn a_bar_pairs_with_its_slice_in_another_recipe() {
    let a = scene(vec![group("plot", vec![group("marks", vec![datum(rect("S", 0.0, 0.0, 10.0, 50.0)), datum(rect("M", 20.0, 0.0, 10.0, 30.0))])])]);
    let b = scene(vec![group("pie", vec![datum(circle("S", 50.0, 50.0, 20.0)), datum(circle("M", 90.0, 50.0, 10.0))])]);
    let p = plan(&a, &b, &MotionRules::default(), &cx());
    let pairs = &p.correspondence().pairs;
    assert!(pairs.iter().any(|(x, y)| x.0.last() == Some(&key("S")) && y.0.last() == Some(&key("S"))), "{pairs:?}");
    assert_eq!(pairs.len(), 2);
}

#[test]
fn structural_names_and_non_data_marks_do_not_pair() {
    // Every datum's shape is called "bar" under its datum group: not an identity.
    let a = scene(vec![group("bridge", vec![group("2023", vec![datum(rect("bar", 0.0, 0.0, 10.0, 50.0))]), group("Price", vec![datum(rect("bar", 20.0, 0.0, 10.0, 30.0))])])]);
    let b = scene(vec![group("funnel", vec![group("Visits", vec![datum(rect("bar", 0.0, 0.0, 80.0, 10.0))]), group("Trials", vec![datum(rect("bar", 0.0, 20.0, 40.0, 10.0))])])]);
    assert!(plan(&a, &b, &MotionRules::default(), &cx()).correspondence().pairs.is_empty(), "different data cross-fades");
    // Same key, but not data (a tick label and a legend swatch): no pairing across paths.
    let a = scene(vec![group("axis", vec![rect("10", 0.0, 0.0, 1.0, 5.0)])]);
    let b = scene(vec![group("legend", vec![rect("10", 50.0, 50.0, 5.0, 5.0)])]);
    assert!(plan(&a, &b, &MotionRules::default(), &cx()).correspondence().pairs.is_empty());
}
