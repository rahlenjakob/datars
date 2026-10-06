//! `datars-view <doc.json | bundle.datars | http://…/c/alias>` — a desktop window for a document, a
//! bundle, or a published chart by URL (the same bundle web pages and apps play).
//! ←/→ step the program, Esc leaves a chapter, D toggles dark mode; drag and wheel for brushes and
//! explorable maps. `--screenshot out.png [--state N] [--dpr 2]` renders offscreen through the same
//! GPU path instead of opening a window.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let path = args.iter().enumerate().find(|(i, a)| !a.starts_with("--") && (*i == 0 || !args[i - 1].starts_with("--"))).map(|(_, a)| a.clone()).unwrap_or_else(|| {
        eprintln!("usage: datars-view <doc.json | bundle.datars | http://host/c/alias> [--screenshot out.png --state N --dpr 2]");
        std::process::exit(2)
    });
    // A published chart by URL, or a document / bundle on disk.
    let src = if path.starts_with("http://") || path.starts_with("https://") {
        datars_host_native::Source::Url(path.clone())
    } else {
        let bytes = std::fs::read(&path).unwrap_or_else(|e| {
            eprintln!("{path}: {e}");
            std::process::exit(1)
        });
        if path.ends_with(".datars") { datars_host_native::Source::Bundle(bytes) } else { datars_host_native::Source::Doc(String::from_utf8_lossy(&bytes).into_owned()) }
    };
    let result = match flag("--screenshot") {
        Some(out) => {
            let state = flag("--state").and_then(|s| s.parse().ok()).unwrap_or(0);
            let dpr = flag("--dpr").and_then(|s| s.parse().ok()).unwrap_or(2.0);
            datars_host_native::screenshot(src, &path, state, dpr, std::path::Path::new(&out)).map(|_| println!("{out}"))
        }
        None => datars_host_native::run(src, &path),
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
