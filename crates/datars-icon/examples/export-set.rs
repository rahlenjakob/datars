//! The default set as the web runtime serves it: one compact file per icon
//! (`<out>/lucide/<name>.json`, what `datars:icons/lucide/<name>.json` resolves to beside the
//! runtime) and the set's licence. The runtime reads no SVG, so the tools read each icon once here.
//!
//! `cargo run --release -p datars-icon --features bundled --example export-set -- packages/web/dist/icons`

use std::path::PathBuf;

fn main() -> Result<(), String> {
    let out = PathBuf::from(std::env::args().nth(1).ok_or("usage: export-set <out dir>")?);
    let set = datars_icon::set::bundled(datars_icon::DEFAULT_SET).ok_or("the default set isn't compiled in")?;
    let dir = out.join(&set.name);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let (mut n, mut bytes) = (0usize, 0usize);
    for name in set.names() {
        let json = set.compact(name).ok_or("missing")?.map_err(|e| format!("{name}: {e}"))?;
        bytes += json.len();
        n += 1;
        std::fs::write(dir.join(format!("{name}.json")), json).map_err(|e| e.to_string())?;
    }
    let licence = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/icons/lucide/LICENSE");
    std::fs::copy(licence, dir.join("LICENSE")).map_err(|e| e.to_string())?;
    println!("{}: {n} icons, {} KB in all ({} B each on average), {} {} ({})", dir.display(), bytes / 1024, bytes / n.max(1), set.name, set.version, set.licence);
    Ok(())
}
