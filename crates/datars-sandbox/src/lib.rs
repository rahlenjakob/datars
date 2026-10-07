//! `datars-sandbox` — runs recipes and kernels (docs/07-extensibility.md) identically on every
//! platform: the same QuickJS natively and compiled into the web runtime's WebAssembly.
//!
//! The sandbox has no ambient authority: no `Date`, no `Math.random`, no IO, no timers. `Math`'s
//! transcendental functions are replaced with `datars_math::m` so `Math.sin` gives the same bits
//! everywhere (P1). Every call runs under a time budget (interrupt ticks) and a memory limit.
//! Recipes talk to the engine only through `host(name, argsJson)` → JSON, served by [`Host`].
//!
//! Known limit: the `**` operator uses QuickJS's C `pow`; the SDK compiles `**` to `Math.pow`, and
//! the linter flags raw `**` in recipe code.

use rquickjs::loader::{Loader, Resolver};
use rquickjs::{CatchResultExt, Context, Ctx, Function, Module, Object, Runtime, Value};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

/// Services the engine offers recipes (text measurement, tokens, scale info, …). JSON in, JSON out.
pub trait Host {
    fn call(&self, name: &str, args_json: &str) -> Result<String, String>;
}

/// A host with no services.
pub struct NoHost;
impl Host for NoHost {
    fn call(&self, name: &str, _: &str) -> Result<String, String> {
        Err(format!("host function `{name}` is not available"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SandboxError(pub String);

impl std::fmt::Display for SandboxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn err(e: impl std::fmt::Display) -> SandboxError {
    SandboxError(e.to_string())
}

#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub memory_limit: usize,
    /// Interrupt ticks per call (QuickJS checks roughly every 10k operations): ~1 s of work.
    pub budget: u64,
}

impl Default for Options {
    fn default() -> Self {
        Options { memory_limit: 256 << 20, budget: 50_000 }
    }
}

type Sources = Rc<RefCell<BTreeMap<String, String>>>;

/// QuickJS keeps an evaluated module for the runtime's life, by the name its resolver gave it. A
/// package whose source changes (an editor's edit, a reloaded document) bumps this epoch, which is
/// part of every package module's resolved name (`mypkg#3`): the next import evaluates the new
/// source, and so do the modules importing it. The built-in `@datars/` modules never change and
/// keep their names, so the standard library is evaluated once.
type Epoch = Rc<Cell<u32>>;

/// A resolved module name without its epoch.
fn source_key(resolved: &str) -> &str {
    resolved.split_once('#').map_or(resolved, |(k, _)| k)
}

struct MapResolver(Sources, Epoch);
impl Resolver for MapResolver {
    fn resolve<'js>(&mut self, _ctx: &Ctx<'js>, base: &str, name: &str) -> rquickjs::Result<String> {
        let base = source_key(base);
        let name = if let Some(rel) = name.strip_prefix("./") {
            // relative to the importing module's package
            match base.rsplit_once('/') {
                Some((dir, _)) => format!("{dir}/{rel}"),
                None => rel.to_string(),
            }
        } else {
            name.to_string()
        };
        let key = name.trim_end_matches(".js").to_string();
        if self.0.borrow().contains_key(&key) {
            let epoch = self.1.get();
            Ok(if epoch == 0 || key.starts_with("@datars/") { key } else { format!("{key}#{epoch}") })
        } else {
            Err(rquickjs::Error::new_resolving(base, name))
        }
    }
}

struct MapLoader(Sources);
impl Loader for MapLoader {
    fn load<'js>(&mut self, ctx: &Ctx<'js>, name: &str) -> rquickjs::Result<Module<'js, rquickjs::module::Declared>> {
        let src = self.0.borrow().get(source_key(name)).cloned().ok_or_else(|| rquickjs::Error::new_loading(name))?;
        Module::declare(ctx.clone(), name, src)
    }
}

thread_local! {
    static HOST: RefCell<Option<Rc<dyn Host>>> = const { RefCell::new(None) };
}

// Field order is drop order: context before runtime.
pub struct Sandbox {
    ctx: Context,
    rt: Runtime,
    sources: Sources,
    epoch: Epoch,
    ticks: Rc<Cell<u64>>,
    budget: u64,
}

impl Sandbox {
    pub fn new(opts: Options) -> Result<Sandbox, SandboxError> {
        let rt = Runtime::new().map_err(err)?;
        rt.set_memory_limit(opts.memory_limit);
        let ticks = Rc::new(Cell::new(0u64));
        {
            let t = ticks.clone();
            let budget = opts.budget;
            rt.set_interrupt_handler(Some(Box::new(move || {
                t.set(t.get() + 1);
                t.get() > budget
            })));
        }
        let sources: Sources = Rc::new(RefCell::new(BTreeMap::new()));
        let epoch: Epoch = Rc::new(Cell::new(0));
        rt.set_loader(MapResolver(sources.clone(), epoch.clone()), MapLoader(sources.clone()));
        let ctx = Context::full(&rt).map_err(err)?;
        ctx.with(|c| -> Result<(), SandboxError> {
            install_math(&c).map_err(err)?;
            let host = Function::new(c.clone(), |name: String, args: String| -> rquickjs::Result<String> {
                let h = HOST.with(|h| h.borrow().clone());
                match h {
                    Some(h) => h.call(&name, &args).map_err(|e| rquickjs::Error::new_from_js_message("host", "result", e)),
                    None => Err(rquickjs::Error::new_from_js_message("host", "result", "no host installed")),
                }
            })
            .map_err(err)?;
            c.globals().set("__host", host).map_err(err)?;
            c.eval::<(), _>(
                "globalThis.Date = undefined; Math.random = undefined; globalThis.setTimeout = undefined; \
                 globalThis.host = (name, args) => JSON.parse(__host(name, JSON.stringify(args === undefined ? null : args))); \
                 Object.freeze(Math);",
            )
            .catch(&c)
            .map_err(|e| SandboxError(format!("sandbox setup: {e}")))
        })?;
        Ok(Sandbox { ctx, rt, sources, epoch, ticks, budget: opts.budget })
    }

    /// Register an ES module by name (e.g. `"@datars/sdk"`, `"@datars/std/bar"`). Imports resolve
    /// against registered names; `./x` resolves relative to the importing module's directory.
    /// Add (or replace) a module's source. Returns whether anything changed: a replaced source is
    /// what later imports and calls see (see [`Epoch`]).
    pub fn add_module(&mut self, name: &str, source: &str) -> bool {
        let key = name.trim_end_matches(".js").to_string();
        let mut sources = self.sources.borrow_mut();
        match sources.get(&key) {
            Some(old) if old == source => false,
            old => {
                if old.is_some() {
                    self.epoch.set(self.epoch.get() + 1);
                }
                sources.insert(key, source.to_string());
                true
            }
        }
    }

    pub fn has_module(&self, name: &str) -> bool {
        self.sources.borrow().contains_key(name)
    }

    /// Call `export(args)` of `module` with JSON args; returns the JSON result. Exports may be
    /// functions or objects with a method named by `method` (e.g. a recipe's `expand`).
    pub fn call(&self, module: &str, export: &str, method: Option<&str>, args_json: &str, host: Rc<dyn Host>) -> Result<String, SandboxError> {
        self.ticks.set(0);
        let prev = HOST.with(|h| h.replace(Some(host)));
        let out = self.ctx.with(|c| -> Result<String, SandboxError> {
            let wrapper = format!(
                "import * as m from '{m}'; \
                 export function __call(args) {{ const e = m[{e:?}]; \
                   const f = {meth}; \
                   const r = f(...JSON.parse(args)); return JSON.stringify(r === undefined ? null : r); }}",
                m = module.trim_end_matches(".js"),
                e = export,
                meth = match method {
                    Some(mm) => format!("(...a) => e[{mm:?}](...a)"),
                    None => "e".to_string(),
                }
            );
            let name = format!("__call:{module}:{export}:{}", method.unwrap_or(""));
            let decl = Module::declare(c.clone(), name, wrapper).catch(&c).map_err(|e| self.explain(e.to_string()))?;
            let (m, p) = decl.eval().catch(&c).map_err(|e| self.explain(e.to_string()))?;
            p.finish::<()>().catch(&c).map_err(|e| self.explain(e.to_string()))?;
            let f: Function = m.get("__call").catch(&c).map_err(|e| self.explain(e.to_string()))?;
            let v: Value = f.call((args_json,)).catch(&c).map_err(|e| self.explain(e.to_string()))?;
            v.as_string().and_then(|s| s.to_string().ok()).ok_or_else(|| SandboxError("no result".into()))
        });
        HOST.with(|h| *h.borrow_mut() = prev);
        self.rt.run_gc();
        out
    }

    /// Evaluate a script expression (tests, the REPL in the dev tools). Returns JSON.
    pub fn eval_json(&self, src: &str) -> Result<String, SandboxError> {
        self.ticks.set(0);
        self.ctx.with(|c| {
            let v: Value = c.eval(format!("JSON.stringify(({src}))")).catch(&c).map_err(|e| self.explain(e.to_string()))?;
            v.as_string().and_then(|s| s.to_string().ok()).ok_or_else(|| SandboxError("no result".into()))
        })
    }

    fn explain(&self, msg: String) -> SandboxError {
        if self.ticks.get() > self.budget {
            SandboxError("took too long (over its time budget) — is there an endless loop?".into())
        } else {
            SandboxError(msg)
        }
    }
}

/// Replace Math's transcendental functions with the engine's deterministic libm.
fn install_math(c: &Ctx<'_>) -> rquickjs::Result<()> {
    use datars_math::m;
    let math: Object = c.globals().get("Math")?;
    macro_rules! f1 {
        ($name:literal, $f:expr) => {
            math.set($name, Function::new(c.clone(), |x: f64| -> f64 { $f(x) })?)?;
        };
    }
    macro_rules! f2 {
        ($name:literal, $f:expr) => {
            math.set($name, Function::new(c.clone(), |a: f64, b: f64| -> f64 { $f(a, b) })?)?;
        };
    }
    f1!("sin", m::sin);
    f1!("cos", m::cos);
    f1!("tan", m::tan);
    f1!("asin", m::asin);
    f1!("acos", m::acos);
    f1!("atan", m::atan);
    f1!("exp", m::exp);
    f1!("log", m::ln);
    f1!("log10", m::log10);
    f1!("log2", m::log2);
    f1!("cbrt", m::cbrt);
    f1!("sinh", m::sinh);
    f1!("cosh", m::cosh);
    f1!("tanh", m::tanh);
    f1!("expm1", |x: f64| libm_expm1(x));
    f1!("log1p", |x: f64| libm_log1p(x));
    f2!("atan2", m::atan2);
    f2!("pow", m::pow);
    math.set("hypot", Function::new(c.clone(), |a: f64, b: f64| -> f64 { m::hypot(a, b) })?)?;
    Ok(())
}

fn libm_expm1(x: f64) -> f64 {
    datars_math::m::exp(x) - 1.0
}
fn libm_log1p(x: f64) -> f64 {
    datars_math::m::ln(1.0 + x)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Measure;
    impl Host for Measure {
        fn call(&self, name: &str, args: &str) -> Result<String, String> {
            match name {
                "measure" => Ok(format!("{{\"w\": {}}}", args.len() * 7)),
                _ => Err(format!("no {name}")),
            }
        }
    }

    #[test]
    fn modules_import_each_other_and_call_the_host() {
        let mut s = Sandbox::new(Options::default()).unwrap();
        s.add_module("@x/lib", "export const twice = (v) => v * 2;");
        s.add_module(
            "@x/recipe",
            "import { twice } from '@x/lib'; export default { expand(p) { return { w: twice(p.n), m: host('measure', 'abc').w }; } };",
        );
        let out = s.call("@x/recipe", "default", Some("expand"), "[{\"n\": 21}]", Rc::new(Measure)).unwrap();
        assert_eq!(out, r#"{"w":42,"m":35}"#);
    }

    #[test]
    fn a_replaced_module_is_what_the_next_call_runs() {
        let mut s = Sandbox::new(Options::default()).unwrap();
        s.add_module("@x/lib", "export const k = 1;");
        s.add_module("@x/recipe", "import { k } from '@x/lib'; export default { expand() { return k; } };");
        let call = |s: &Sandbox| s.call("@x/recipe", "default", Some("expand"), "[]", Rc::new(Measure)).unwrap();
        assert_eq!(call(&s), "1");
        // An edit to the recipe itself, and then to a module it imports: both seen at once.
        assert!(s.add_module("@x/recipe", "import { k } from '@x/lib'; export default { expand() { return k * 10; } };"));
        assert_eq!(call(&s), "10");
        assert!(s.add_module("@x/lib", "export const k = 2;"));
        assert_eq!(call(&s), "20");
        // The same source again changes nothing.
        assert!(!s.add_module("@x/lib", "export const k = 2;"));
        assert_eq!(call(&s), "20");
    }

    #[test]
    fn no_ambient_authority_and_deterministic_math() {
        let s = Sandbox::new(Options::default()).unwrap();
        assert_eq!(s.eval_json("typeof Date").unwrap(), "\"undefined\"");
        assert_eq!(s.eval_json("typeof Math.random").unwrap(), "\"undefined\"");
        let v: f64 = s.eval_json("Math.sin(1)").unwrap().parse().unwrap();
        assert_eq!(v.to_bits(), datars_math::m::sin(1.0).to_bits(), "Math.sin is the engine's libm");
    }

    #[test]
    fn runaway_code_hits_the_budget() {
        let mut s = Sandbox::new(Options { budget: 200, ..Default::default() }).unwrap();
        s.add_module("@x/loop", "export default function () { for (;;) {} }");
        let e = s.call("@x/loop", "default", None, "[]", Rc::new(NoHost)).unwrap_err();
        assert!(e.0.contains("time budget"), "{e}");
        // The sandbox is still usable afterwards.
        s.add_module("@x/ok", "export default function () { return 1; }");
        assert_eq!(s.call("@x/ok", "default", None, "[]", Rc::new(NoHost)).unwrap(), "1");
    }
}
