//! Recipe expansion through the sandbox (docs/07-extensibility.md). Recipes are ordinary packages —
//! the standard library included — and their expansions are cached by (recipe, params, context).

use crate::resolve::{Expand, Expansion};
use datars_sandbox::{Host, Options, Sandbox};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// The SDK runtime every recipe imports, and the standard library (built from packages/sdk and
/// packages/std by `pnpm -C public build`; committed so cargo builds don't need Node).
pub const SDK_JS: &str = include_str!("../js/sdk.js");
pub const STD_JS: &str = include_str!("../js/std.js");

/// Services for recipes: text measurement, formatting, theme tokens.
pub struct EngineHost {
    /// The engine's fonts (swapped when a document provides a font).
    pub fonts: RefCell<Rc<datars_text::FontDb>>,
    pub theme: RefCell<datars_theme::ResolvedTheme>,
    pub locale: RefCell<String>,
}

impl Host for EngineHost {
    fn call(&self, name: &str, args: &str) -> Result<String, String> {
        let v: serde_json::Value = serde_json::from_str(args).map_err(|e| e.to_string())?;
        match name {
            "measure" => {
                let text = v.get("text").and_then(|t| t.as_str()).unwrap_or("");
                let size = v.get("size").and_then(|s| s.as_f64()).unwrap_or(11.0);
                let weight = v.get("weight").and_then(|s| s.as_u64()).unwrap_or(400) as u16;
                // Recipes measure in the theme's body font unless they name one.
                let body = self.theme.borrow().font("font.body").map(|f| f.stack().join(", ")).filter(|f| !f.is_empty()).unwrap_or_else(|| "Inter".into());
                let family = v.get("family").and_then(|s| s.as_str()).map(String::from).unwrap_or(body);
                let style = datars_scene::text::TextStyle { family: family.as_str().into(), weight, size, ..Default::default() };
                let r = datars_text::measure(&self.fonts.borrow(), text, &style);
                Ok(serde_json::json!({ "w": r.w, "h": r.h }).to_string())
            }
            "format" => {
                let x = v.get(0).and_then(|x| x.as_f64()).unwrap_or(f64::NAN);
                let spec = v.get(1).and_then(|x| x.as_str()).unwrap_or(",.2~f");
                Ok(serde_json::Value::String(datars_text::format::number(x, spec, &self.locale.borrow())).to_string())
            }
            "token" => {
                let n = v.as_str().unwrap_or("");
                let t = self.theme.borrow();
                let out = if let Some(x) = t.number(n) {
                    serde_json::json!(x)
                } else if let Some(s) = t.text(n) {
                    serde_json::json!(s)
                } else {
                    serde_json::json!(format!("${n}"))
                };
                Ok(out.to_string())
            }
            _ => Err(format!("unknown host function `{name}`")),
        }
    }
}

pub struct SandboxExpander {
    sandbox: RefCell<Sandbox>,
    cache: RefCell<BTreeMap<String, Expansion>>,
    pub host: Rc<EngineHost>,
}

impl SandboxExpander {
    pub fn new(host: Rc<EngineHost>) -> Result<SandboxExpander, String> {
        let mut sb = Sandbox::new(Options::default()).map_err(|e| e.0)?;
        sb.add_module("@datars/sdk", SDK_JS);
        sb.add_module("@datars/std", STD_JS);
        Ok(SandboxExpander { sandbox: RefCell::new(sb), cache: RefCell::new(BTreeMap::new()), host })
    }

    /// Register a package module (user recipes).
    pub fn add_module(&self, name: &str, source: &str) {
        self.sandbox.borrow_mut().add_module(name, source);
        self.cache.borrow_mut().clear();
    }

    pub fn clear_cache(&self) {
        self.cache.borrow_mut().clear();
    }

    /// A recipe's self-description (params, docs, tokens, examples) — for docs, inspectors, agents.
    pub fn describe(&self, recipe: &str) -> Result<serde_json::Value, String> {
        let (module, export) = split_recipe(recipe);
        let out = self.sandbox.borrow().call(&module, &export, Some("describe"), "[]", self.host.clone()).map_err(|e| e.0)?;
        serde_json::from_str(&out).map_err(|e| e.to_string())
    }

    /// Every recipe exported by a module (`@datars/std`).
    pub fn list(&self, module: &str) -> Result<Vec<String>, String> {
        let out = self
            .sandbox
            .borrow()
            .eval_json("0")
            .map(|_| String::new())
            .map_err(|e| e.0)?;
        let _ = out;
        let src = format!("import * as m from '{module}'; export default function () {{ return Object.keys(m).filter(k => m[k] && m[k].__expand); }}");
        let mut sb = self.sandbox.borrow_mut();
        sb.add_module("__list", &src);
        let r = sb.call("__list", "default", None, "[]", self.host.clone()).map_err(|e| e.0)?;
        let names: Vec<String> = serde_json::from_str(&r).map_err(|e| e.to_string())?;
        Ok(names.into_iter().map(|n| format!("{module}/{n}")).collect())
    }
}

/// `"@datars/std/bar"` → (`"@datars/std"`, `"bar"`); `"mypkg/lollipop"` → (`"mypkg"`, `"lollipop"`).
pub fn split_recipe(id: &str) -> (String, String) {
    let parts: Vec<&str> = id.split('/').collect();
    let n_mod = if id.starts_with('@') { 2 } else { 1 };
    if parts.len() <= n_mod {
        return (id.to_string(), "default".to_string());
    }
    (parts[..n_mod].join("/"), parts[n_mod..].join("/"))
}

impl Expand for SandboxExpander {
    fn expand(&self, recipe: &str, params: &serde_json::Value, cx: &serde_json::Value) -> Result<Expansion, String> {
        let key = format!("{recipe}\u{1f}{params}\u{1f}{cx}");
        if let Some(e) = self.cache.borrow().get(&key) {
            return Ok(e.clone());
        }
        let (module, export) = split_recipe(recipe);
        let args = serde_json::json!([params, cx]).to_string();
        let out = self.sandbox.borrow().call(&module, &export, Some("__expand"), &args, self.host.clone()).map_err(|e| e.0)?;
        let v: serde_json::Value = serde_json::from_str(&out).map_err(|e| e.to_string())?;
        let tv = v.get("template").cloned().unwrap_or(v.clone());
        let template: datars_ir::Template = serde_json::from_value(tv.clone()).map_err(|e| format!("expansion is not a valid template: {e}"))?;
        let mut notes = datars_ir::migrate::ignored_in::<datars_ir::Template>(&tv).into_iter().map(|n| format!("ignored field `{n}` (not part of the IR)")).collect::<Vec<_>>();
        // Settings the recipe doesn't have (typos, or a recipe that changed): said, not ignored.
        if let Some(u) = v.get("unknown").and_then(|u| u.as_array()) {
            notes.extend(u.iter().filter_map(|x| x.as_str()).map(|k| format!("has no setting {k}")));
        }
        let tables: BTreeMap<String, datars_ir::Derived> = match v.get("tables") {
            Some(t) if !t.is_null() => serde_json::from_value(t.clone()).map_err(|e| format!("expansion tables: {e}"))?,
            _ => BTreeMap::new(),
        };
        let e = Expansion { template, tables, notes };
        self.cache.borrow_mut().insert(key, e.clone());
        Ok(e)
    }
}

/// Expand every `use` node in a template recursively (static pre-expansion for T2 bundles).
pub fn expand_all(ex: &SandboxExpander, t: &datars_ir::Template, cx: &serde_json::Value, tables: &mut BTreeMap<String, datars_ir::Derived>, depth: usize) -> Result<datars_ir::Template, String> {
    use datars_ir::TKind;
    if depth > 24 {
        return Err("recipe expansion too deep".into());
    }
    let mut out = t.clone();
    match &t.kind {
        TKind::Use { recipe, params } => {
            let e = ex.expand(recipe, params, cx)?;
            tables.extend(e.tables);
            let mut inner = expand_all(ex, &e.template, cx, tables, depth + 1)?;
            if !t.key.is_null() {
                inner.key = t.key.clone();
            }
            if !t.when.is_null() {
                inner.when = t.when.clone();
            }
            if t.size.is_some() {
                inner.size = t.size.clone();
            }
            if t.id.is_some() && inner.id.is_none() {
                inner.id = t.id.clone();
            }
            return Ok(inner);
        }
        TKind::Group { children } => {
            out.kind = TKind::Group { children: children.iter().map(|c| expand_all(ex, c, cx, tables, depth)).collect::<Result<_, _>>()? };
        }
        TKind::View { camera, children } => {
            out.kind = TKind::View { camera: camera.clone(), children: children.iter().map(|c| expand_all(ex, c, cx, tables, depth)).collect::<Result<_, _>>()? };
        }
        TKind::Repeat { from, template } => {
            out.kind = TKind::Repeat { from: from.clone(), template: Box::new(expand_all(ex, template, cx, tables, depth)?) };
        }
        _ => {}
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn recipe_ids_split_into_module_and_export() {
        assert_eq!(super::split_recipe("@datars/std/bar"), ("@datars/std".into(), "bar".into()));
        assert_eq!(super::split_recipe("mypkg/lollipop"), ("mypkg".into(), "lollipop".into()));
        assert_eq!(super::split_recipe("@datars/std/map/region"), ("@datars/std".into(), "map/region".into()));
    }
}
