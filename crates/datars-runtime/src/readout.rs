//! What a native screen reader walks through (VoiceOver, TalkBack, desktop readers): the engine's
//! semantics tree as a flat reading order a swipe at a time moves along — the chart first (its
//! name, and the summary as its value), then headings for the parts, an axis or a legend as one
//! line, each data mark with its name and value apart, clickable marks as buttons. Hosts turn
//! these into their platform's elements; the web runtime builds its own (tables) from the same
//! tree. Engine-drawn sliders and selects aren't here: hosts offer them from `status().controls`.

use datars_engine::SemanticItem;

/// One stop for a screen reader.
#[derive(Clone, Debug, PartialEq)]
pub struct Readout {
    /// `chart` (the whole: first), `heading` (a part: a sub-chart's or a series' name), `text`
    /// (an axis, a legend, a note), `datum` (a data mark), `button` (something a tap acts on),
    /// `checkbox`, `switch`, `radio` (a control with a state: `checked`), `brush` (a range to
    /// select with the keyboard) and `pan` (a view to move and zoom): keys go to
    /// `Engine::key` by `path`.
    pub kind: &'static str,
    /// A checkbox's, switch's or radio's state.
    pub checked: Option<bool>,
    pub label: String,
    /// What the element says after its name: a datum's value ("10,240", "× 24" for repeated
    /// marks), the chart's summary.
    pub value: String,
    /// Where it is, in root coordinates (CSS px); none for the chart (the whole view).
    pub rect: Option<[f64; 4]>,
    /// A button's key path (`Engine::activate`).
    pub path: String,
}

impl Readout {
    pub fn to_json(&self) -> serde_json::Value {
        let mut v = serde_json::json!({ "kind": self.kind, "label": self.label, "value": self.value, "rect": self.rect, "path": self.path });
        if let Some(c) = self.checked {
            v["checked"] = serde_json::json!(c);
        }
        v
    }
}

struct Tree<'a> {
    item: &'a SemanticItem,
    kids: Vec<Tree<'a>>,
}

fn is_data(i: &SemanticItem) -> bool {
    i.role == "datum" || i.role == "region"
}

fn rect_of(i: &SemanticItem) -> Option<[f64; 4]> {
    let r = i.rect;
    (r.w.is_finite() && r.h.is_finite() && r.w > 0.0 && r.h > 0.0).then_some([r.x, r.y, r.w, r.h])
}

/// Labels as one line: all of a few, the ends of many.
fn line(labels: &[&str]) -> String {
    if labels.len() <= 12 {
        labels.join(", ")
    } else {
        format!("{} to {}", labels[0], labels[labels.len() - 1])
    }
}

fn descend<'a>(t: &Tree<'a>, role: &str, out: &mut Vec<&'a str>) {
    for k in &t.kids {
        if k.item.role == role {
            if !k.item.label.trim().is_empty() {
                out.push(k.item.label.as_str());
            }
        } else {
            descend(k, role, out);
        }
    }
}

/// "Housing: 10,240" → ("Housing", "10,240"); a label without a value stays whole.
fn split(label: &str) -> (String, String) {
    match label.rfind(": ") {
        Some(i) if i > 0 => (label[..i].to_string(), label[i + 2..].to_string()),
        _ => (label.to_string(), String::new()),
    }
}

/// What a screen reader walks through for an engine as it is now: [`readout_in`] over its
/// semantics, then the step's narration right after what the chart shows (unless the chart draws
/// it: then it's in the content already), and a story's Previous step and Next step after the
/// content (`Engine::activate` runs them by their paths, `event:prev` and `event:next`) — the
/// order the web element reads too.
pub fn of_engine(engine: &mut datars_engine::Engine) -> Vec<Readout> {
    let items = engine.semantic_items();
    let name = engine.accessible_name();
    let summary = engine.accessible_summary();
    let locale = engine.doc().locale.clone();
    let mut out = readout_in(&locale, &name, &summary, &items);
    if let Some(n) = engine.narration().filter(|_| !engine.draws_narration()) {
        let lines: Vec<Readout> = [(n.title.clone(), "heading"), (n.text.clone(), "text")]
            .into_iter()
            .filter(|(t, _)| !t.trim().is_empty())
            .map(|(t, kind)| Readout { kind, label: t.trim().to_string(), value: String::new(), rect: None, path: String::new(), checked: None })
            .collect();
        let at = out.len().min(1);
        out.splice(at..at, lines);
    }
    let (index, states) = (engine.state_index(), engine.state_names().len());
    if states > 1 {
        let phrase = |k: &str| datars_engine::phrases::phrase(&locale, k, &[]);
        if index > 0 {
            out.push(Readout { kind: "button", label: phrase("previous-step"), value: String::new(), rect: None, path: "event:prev".into(), checked: None });
        }
        if index + 1 < states {
            out.push(Readout { kind: "button", label: phrase("next-step"), value: String::new(), rect: None, path: "event:next".into(), checked: None });
        }
    }
    out
}

/// The reading order for a chart named `name` with `summary`, from its semantics `items`, its own
/// words in English (see [`readout_in`]).
pub fn readout(name: &str, summary: &str, items: &[SemanticItem]) -> Vec<Readout> {
    readout_in("en", name, summary, items)
}

/// [`readout`] with its own words ("Legend", "Note:", "… without data") in `locale`'s language.
pub fn readout_in(locale: &str, name: &str, summary: &str, items: &[SemanticItem]) -> Vec<Readout> {
    use datars_engine::phrases::{counted, phrase};
    let mut out = vec![Readout { kind: "chart", label: if name.is_empty() { phrase(locale, "chart", &[]) } else { name.into() }, value: summary.into(), rect: None, path: String::new(), checked: None }];
    // Dragged and picked controls are the host's native ones; clicked controls are buttons.
    let shown: Vec<&SemanticItem> = items.iter().filter(|i| i.role != "control" || i.actionable).collect();
    let mut top: Vec<Tree> = Vec::new();
    // Pre-order with depths → a tree (a stack of open ancestors, as indices into their parents).
    let mut path: Vec<usize> = Vec::new();
    for item in shown {
        let node = Tree { item, kids: Vec::new() };
        // Close the ancestors at this depth or deeper.
        let mut level = &mut top;
        let mut keep = 0;
        for &i in &path {
            if level[i].item.depth >= item.depth {
                break;
            }
            keep += 1;
            level = &mut level[i].kids;
        }
        path.truncate(keep);
        level.push(node);
        path.push(level.len() - 1);
    }
    fn walk(locale: &str, nodes: &[Tree], name: &str, group: &str, out: &mut Vec<Readout>) {
        use datars_engine::phrases::{counted, phrase};
        // Marks without data (a map's regions with no value): one line after the rest, not a
        // stop each — and no buttons.
        let missing: Vec<&SemanticItem> = nodes.iter().map(|n| n.item).filter(|i| is_data(i) && i.missing).collect();
        for n in nodes.iter().filter(|n| !(is_data(n.item) && n.item.missing)) {
            let (role, label) = (n.item.role.as_str(), n.item.label.trim());
            // What a click acts on — a filtering bar, a switch, a select's box — is a button.
            if n.item.actionable {
                // A control with a state is its native kind (a checkbox, a switch, one of a set).
                let kind = match n.item.kind.as_deref() {
                    Some("checkbox") => "checkbox",
                    Some("switch") => "switch",
                    Some("radio") => "radio",
                    Some("brush") => "brush",
                    Some("pan") => "pan",
                    _ => "button",
                };
                // A brushable area or a pannable view: keys by its path (`Engine::key`); what it
                // holds as its value ("2017 to 2019", "nothing selected").
                if kind == "brush" || kind == "pan" {
                    let (l, v) = split(label);
                    out.push(Readout { kind, label: l, value: v, rect: rect_of(n.item), path: n.item.path.clone(), checked: None });
                    continue;
                }
                if kind != "button" {
                    out.push(Readout { kind, label: label.into(), value: String::new(), rect: rect_of(n.item), path: n.item.path.clone(), checked: Some(n.item.checked.unwrap_or(false)) });
                    continue;
                }
                out.push(Readout { kind: "button", label: label.into(), value: String::new(), rect: rect_of(n.item), path: n.item.path.clone(), checked: None });
                continue;
            }
            if is_data(n.item) {
                // Marks that say the same in a row (a parliament's seats) are one, with a count.
                if let Some(last) = out.last_mut() {
                    if last.kind == "datum" && last.label == label && (last.value.is_empty() || last.value.starts_with('×')) {
                        let n_was: usize = last.value.trim_start_matches('×').trim().parse().unwrap_or(1);
                        last.value = format!("× {}", n_was + 1);
                        if let (Some(a), Some(b)) = (last.rect, rect_of(n.item)) {
                            let (x0, y0) = (a[0].min(b[0]), a[1].min(b[1]));
                            last.rect = Some([x0, y0, (a[0] + a[2]).max(b[0] + b[2]) - x0, (a[1] + a[3]).max(b[1] + b[3]) - y0]);
                        }
                        continue;
                    }
                }
                let (l, v) = split(label);
                out.push(Readout { kind: "datum", label: l, value: v, rect: rect_of(n.item), path: String::new(), checked: None });
                continue;
            }
            match role {
                // Said with their axis or legend.
                "tick" | "legend-item" => continue,
                "axis" | "legend" => {
                    let mut parts = Vec::new();
                    descend(n, if role == "axis" { "tick" } else { "legend-item" }, &mut parts);
                    let legend = phrase(locale, "legend", &[]);
                    let title = if label.is_empty() && role == "legend" { legend.as_str() } else { label };
                    let text = if parts.is_empty() { title.to_string() } else if title.is_empty() { line(&parts) } else { format!("{title}: {}", line(&parts)) };
                    if !text.is_empty() {
                        out.push(Readout { kind: "text", label: text, value: String::new(), rect: rect_of(n.item), path: String::new(), checked: None });
                    }
                    continue;
                }
                _ => {}
            }
            // The chart itself (named first) and repeats of its name or its part's say nothing new.
            let dup = label.is_empty() || label == name || (role == "title" && label == group);
            if !dup {
                let kind = match role {
                    "group" | "series" | "title" => "heading",
                    _ => "text",
                };
                let text = if role == "annotation" { phrase(locale, "note", &[("text", label)]) } else { label.to_string() };
                // A series right under its part's heading of the same name: once is enough.
                if !(kind == "heading" && out.last().is_some_and(|l| l.kind == "heading" && l.label == text)) {
                    out.push(Readout { kind, label: text, value: String::new(), rect: rect_of(n.item), path: String::new(), checked: None });
                }
            }
            let group = if matches!(role, "group" | "series") && !label.is_empty() { label } else { group };
            walk(locale, &n.kids, name, group, out);
        }
        if !missing.is_empty() {
            let key = if missing.iter().all(|i| i.role == "region") { "regions-missing" } else { "points-missing" };
            let n = missing.len();
            out.push(Readout { kind: "text", label: counted(locale, key, n, &n.to_string()), value: String::new(), rect: None, path: String::new(), checked: None });
        }
    }
    let _ = counted;
    walk(locale, &top, name, name, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(role: &str, label: &str, depth: usize) -> SemanticItem {
        SemanticItem { role: role.into(), label: label.into(), depth, rect: datars_math::Rect::new(0.0, 0.0, 10.0, 10.0), path: String::new(), ..Default::default() }
    }

    #[test]
    fn a_bar_chart_reads_as_its_name_its_axes_and_its_data() {
        let items = vec![
            item("group", "Per month", 0),
            item("title", "Per month", 1),
            item("axis", "y axis", 1),
            item("tick", "Housing", 2),
            item("tick", "Food", 2),
            item("series", "amount by category", 1),
            item("datum", "Housing: 10,240", 2),
            item("datum", "Food: 5,120", 2),
            item("axis", "x axis", 1),
            item("tick", "0", 2),
            item("tick", "10,000", 2),
        ];
        let r = readout("Per month", "2 data points.", &items);
        let said: Vec<(&str, &str, &str)> = r.iter().map(|x| (x.kind, x.label.as_str(), x.value.as_str())).collect();
        assert_eq!(said, vec![
            ("chart", "Per month", "2 data points."),
            ("text", "y axis: Housing, Food", ""),
            ("heading", "amount by category", ""),
            ("datum", "Housing", "10,240"),
            ("datum", "Food", "5,120"),
            ("text", "x axis: 0, 10,000", ""),
        ]);
    }

    #[test]
    fn regions_without_data_are_one_line_and_no_buttons() {
        let mut no = item("region", "Norway: no data", 0);
        no.missing = true;
        no.actionable = true;
        let mut fi = no.clone();
        fi.label = "Finland: no data".into();
        let r = readout("Nordics", "", &[item("region", "Sweden: 81", 0), no, fi]);
        let said: Vec<(&str, &str)> = r.iter().skip(1).map(|x| (x.kind, x.label.as_str())).collect();
        assert_eq!(said, vec![("datum", "Sweden"), ("text", "2 regions without data")]);
    }

    #[test]
    fn its_own_words_follow_the_documents_language() {
        let mut no = item("region", "Norge: inga data", 0);
        no.missing = true;
        let r = readout_in("sv", "", "", &[item("legend", "", 0), item("legend-item", "Norden", 1), no]);
        let said: Vec<&str> = r.iter().map(|x| x.label.as_str()).collect();
        assert_eq!(said, vec!["Diagram", "Teckenförklaring: Norden", "1 område utan data"]);
    }

    #[test]
    fn repeated_marks_are_one_with_a_count_and_clicks_are_buttons() {
        let mut click = item("datum", "North: 774", 0);
        click.actionable = true;
        click.path = "(\"bars\",\"North\")".into();
        let items = vec![item("datum", "V", 0), item("datum", "V", 0), item("datum", "V", 0), item("datum", "S", 0), click, item("control", "Rate: 3%", 0)];
        let r = readout("Seats", "", &items);
        let said: Vec<(&str, &str, &str)> = r.iter().skip(1).map(|x| (x.kind, x.label.as_str(), x.value.as_str())).collect();
        assert_eq!(said, vec![("datum", "V", "× 3"), ("datum", "S", ""), ("button", "North: 774", "")], "a dragged control is the host's own slider");
        assert_eq!(r[3].path, "(\"bars\",\"North\")");
    }
}
