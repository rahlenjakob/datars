//! Starting points for a new document (`datars new --template …`, an editor's "new chart"): each
//! a complete, working TypeScript document to edit — one per kind of piece (a chart, a stepped
//! story, an explorable, a map).

use std::path::Path;

/// Starting points for `datars new`: each a complete, working document to edit.
pub const TEMPLATES: &[(&str, &str, &str)] = &[
    ("chart", "a bar chart with labels", TEMPLATE_CHART),
    ("story", "a stepped story: bars that morph into a ranking and a pie", TEMPLATE_STORY),
    ("explorable", "a chart driven by an engine-drawn slider", TEMPLATE_EXPLORABLE),
    ("map", "a world choropleth over the built-in countries atlas", TEMPLATE_MAP),
];

const TEMPLATE_CHART: &str = r#"// A bar chart. Edit the data, then `datars dev doc.ts` for a live preview.
import { doc, data, group } from "@datars/sdk";
import { plot, bar } from "@datars/std";

export default doc({
  id: "__NAME__",
  title: "Sales by region",
  size: [720, 420],
  data: { sales: data.values({ region: ["North", "South", "East", "West"], sales: [120, 98, 143, 87] }, { key: "region" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [plot({ data: "sales", x: "region", y: "sales", title: "Sales by region", children: [bar({ labels: true })] })],
  }),
});
"#;

const TEMPLATE_STORY: &str = r#"// A story in steps: each party keeps its key, so its bar becomes its slice.
import { doc, data, e, group, motion, signal, story, step } from "@datars/sdk";
import { plot, bar, pie } from "@datars/std";

export default doc({
  id: "__NAME__",
  title: "Where the votes went",
  size: [720, 440],
  data: { votes: data.values({ party: ["A", "B", "C", "D"], share: [34.2, 27.9, 21.5, 16.4] }, { key: "party" }) },
  signals: { shape: signal.str("bars") },
  // Pair data marks by their own key across recipes (a bar and a slice are the same party).
  motion: motion({ select: { role: "datum" }, matcher: "by-key" }),
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ data: "votes", x: "party", y: "share", color: "party", title: "Vote share (%)", children: [bar({ labels: true })] }, { key: "chart", when: e('shape == "bars"') }),
      pie({ data: "votes", value: "share", category: "party" }, { key: "chart", when: e('shape == "pie"') }),
    ],
  }),
  program: story({
    steps: [
      step("bars", { set: { shape: "bars" }, title: "Four parties" }),
      step("pie", { set: { shape: "pie" }, title: "Shares of the whole" }),
    ],
  }),
});
"#;

const TEMPLATE_EXPLORABLE: &str = r#"// An explorable: the slider is drawn by the engine (web, apps, video alike); the bars read its
// signal in their expressions — no host code.
import { doc, data, group, interactive, signal } from "@datars/sdk";
import { plot, bar, slider } from "@datars/std";

export default doc({
  id: "__NAME__",
  title: "What a raise buys",
  size: [720, 440],
  data: { shares: data.values({ item: ["Rent", "Food", "Travel", "Savings"], share: [35, 20, 15, 30] }, { key: "item" }) },
  tables: { amounts: { from: "shares", ops: [{ op: "derive", as: "amount", expr: { expr: "d.share * income / 100" } }] } },
  signals: { income: signal.num(30000) },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 8, padding: [16, 20, 12, 12] },
    children: [
      slider({ signal: "income", min: 10000, max: 80000, step: 1000, label: "Monthly income", format: ",.0f" }, { size: { h: 44 } }),
      plot({ data: "amounts", x: "amount", y: "item", xType: "linear", yType: "band", xDomain: [0, 30000], title: "Per month", children: [bar({ labels: true, format: ",.0f" })] }),
    ],
  }),
  program: interactive(),
});
"#;

const TEMPLATE_MAP: &str = r#"// A world choropleth: regions from the built-in countries atlas (ISO 3166 alpha-3 ids), coloured
// by the joined values. The publish compiler ships the atlas as a compact topology, cached by hash.
import { doc, data, group } from "@datars/sdk";
import { map } from "@datars/std";

export default doc({
  id: "__NAME__",
  title: "Renewable share of energy",
  size: [760, 480],
  data: {
    world: data.atlas("countries"),
    values: data.values({ id: ["SWE", "NOR", "FIN", "DNK", "ISL"], share: [66, 75, 48, 44, 85] }, { key: "id" }),
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [map({ source: "world", data: "values", key: "id", value: "share", colorType: "sequential" })],
  }),
});
"#;

/// Write `<dir>/doc.ts` from a template. Refuses to overwrite.
pub fn new_doc(dir: &Path, template: &str) -> Result<String, String> {
    let (_, _, src) = TEMPLATES.iter().find(|(n, _, _)| *n == template).ok_or_else(|| format!("unknown template `{template}` (have: {})", TEMPLATES.iter().map(|t| t.0).collect::<Vec<_>>().join(", ")))?;
    let file = dir.join("doc.ts");
    if file.exists() {
        return Err(format!("{} exists", file.display()));
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "chart".into());
    std::fs::write(&file, src.replace("__NAME__", &name)).map_err(|e| e.to_string())?;
    Ok(file.display().to_string())
}

/// A template's source with its document id filled in (`None`: no such template).
pub fn template_source(template: &str, id: &str) -> Option<String> {
    TEMPLATES.iter().find(|(n, _, _)| *n == template).map(|(_, _, src)| src.replace("__NAME__", id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_are_written_once() {
        let dir = std::env::temp_dir().join(format!("datars-new-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (name, _, _) in TEMPLATES {
            let d = dir.join(name);
            let f = new_doc(&d, name).unwrap();
            assert!(std::fs::read_to_string(&f).unwrap().contains(&format!("id: \"{name}\"")));
            assert!(new_doc(&d, name).is_err(), "never overwrites");
        }
        assert!(new_doc(&dir.join("x"), "nope").is_err());
        assert!(template_source("story", "my-story").unwrap().contains("id: \"my-story\""));
        assert!(template_source("nope", "x").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
