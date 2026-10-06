//! `datars-geo-build` — build atlases and tiles from open data.
//!
//!   datars-geo-build atlas <in.geojson> --id ADM0_A3 --name NAME,ADMIN --tolerance 0.05 --out assets/atlas/countries.geojson
//!   datars-geo-build tiles <job.json> --data <inputs dir> --out region.pmtiles
//!   datars-geo-build camera <views.json> --data <inputs dir> --out story.pmtiles [--job story.job.json]
//!     views.json: {"size": [w, h], "lang": "sv", "chains": [[{"label": "…", "bbox": [w, s, e, n]},
//!                  {"label": "…", "center": [lon, lat], "zoom": 11.5}, …]]}  (tile zoom: world = 512·2^zoom px)
//!   datars-geo-build auto <views.json> --out story.pmtiles [--job story.job.json]
//!     the same views, from open data fetched into the cache (~/.cache/datars/geo, $DATARS_GEO_CACHE)
//!   datars-geo-build osm <overpass.json>... --out data/
use std::collections::BTreeMap;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut pos = Vec::new();
    let mut flags = BTreeMap::new();
    let mut i = 0;
    while i < args.len() {
        if let Some(f) = args[i].strip_prefix("--") {
            flags.insert(f.to_string(), args.get(i + 1).cloned().unwrap_or_default());
            i += 2;
        } else {
            pos.push(args[i].clone());
            i += 1;
        }
    }
    let r = run(&pos, &flags);
    if let Err(e) = r {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(pos: &[String], flags: &BTreeMap<String, String>) -> Result<(), String> {
    let out = flags.get("out").cloned().unwrap_or_else(|| "out".into());
    match pos.first().map(|s| s.as_str()) {
        Some("atlas") => {
            let input = std::fs::read(pos.get(1).ok_or("input GeoJSON required")?).map_err(|e| e.to_string())?;
            let names: Vec<&str> = flags.get("name").map(|s| s.split(',').collect()).unwrap_or_else(|| vec!["name", "NAME"]);
            let tol: f64 = flags.get("tolerance").and_then(|t| t.parse().ok()).unwrap_or(0.05);
            let dec: u32 = flags.get("decimals").and_then(|t| t.parse().ok()).unwrap_or(3);
            let s = datars_geo_build::atlas(&input, flags.get("id").map(|s| s.as_str()).unwrap_or("id"), &names, tol, dec)?;
            if let Some(d) = std::path::Path::new(&out).parent() {
                std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
            }
            std::fs::write(&out, &s).map_err(|e| e.to_string())?;
            eprintln!("{out}: {} KB", s.len() / 1024);
        }
        Some("tiles") => {
            let job: datars_geo_build::TileJob = serde_json::from_slice(&std::fs::read(pos.get(1).ok_or("job.json required")?).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            // `--data <dir>`: where the job's inputs live (they're build inputs, not in git).
            let data = std::path::PathBuf::from(flags.get("data").cloned().unwrap_or_else(|| ".".into()));
            build(&job, &data, &out)?;
        }
        Some("camera") => {
            // A camera-aware extract from views: the job (written next to the archive, or to
            // `--job`), then the archive.
            let spec: datars_geo_build::recipe::CameraSpec = serde_json::from_slice(&std::fs::read(pos.get(1).ok_or("views.json required")?).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            let data = std::path::PathBuf::from(flags.get("data").cloned().unwrap_or_else(|| ".".into()));
            let local = datars_geo_build::recipe::LocalData::scan(&data)?;
            let opts = datars_geo_build::recipe::RecipeOptions { lang: spec.lang.clone(), about: spec.about.clone() };
            let job = datars_geo_build::recipe::camera_job(&spec.extents()?, spec.size[0], spec.size[1], &local, &opts, &Default::default());
            let job_path = flags.get("job").cloned().unwrap_or_else(|| out.trim_end_matches(".pmtiles").to_string() + ".job.json");
            std::fs::write(&job_path, datars_geo_build::job_json(&job)).map_err(|e| e.to_string())?;
            for u in &job.unserved {
                eprintln!("  ! {} (z{}): no local {} there", u.label, u.zoom, u.missing);
            }
            build(&job, &data, &out)?;
        }
        Some("auto") => {
            // Flights between consecutive views, as `camera` samples them; the data from the cache.
            let spec: datars_geo_build::recipe::CameraSpec = serde_json::from_slice(&std::fs::read(pos.get(1).ok_or("views.json required")?).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            let [w, h] = spec.size;
            let mut views = Vec::new();
            for chain in spec.extents()? {
                for (i, v) in chain.iter().enumerate() {
                    if i > 0 {
                        views.extend(datars_geo_build::camera::flight(&chain[i - 1], v, w, h, datars_geo_build::recipe::FLIGHT_SAMPLES));
                    }
                    views.push(v.clone());
                }
            }
            let o = datars_geo_build::auto::AutoOptions { lang: spec.lang.clone(), about: spec.about.clone(), ..Default::default() };
            let job = datars_geo_build::auto::plan(&views, &[], &o);
            let cache = std::sync::Arc::new(datars_geo_build::fetch::Cache::from_env());
            eprintln!("cache {}", cache.root.display());
            let src = datars_geo_build::auto::Source::new(cache, spec.lang.clone(), true);
            let t0 = std::time::Instant::now();
            let built = datars_geo_build::auto::build(job, &src, &|i, n, need| eprintln!("  [{}/{n}] {}", i + 1, need.describe()))?;
            let job_path = flags.get("job").cloned().unwrap_or_else(|| out.trim_end_matches(".pmtiles").to_string() + ".job.json");
            std::fs::write(&job_path, datars_geo_build::job_json(&built.job)).map_err(|e| e.to_string())?;
            std::fs::write(&out, &built.bytes).map_err(|e| e.to_string())?;
            for (z, (n, b)) in &built.stats.by_zoom {
                eprintln!("  z{z:<2} {n:>5} tiles  {:>6} KB before compression", b / 1024);
            }
            eprintln!("{out}: {} tiles, {} features in, {} KB ({}; {:.1} s)", built.stats.tiles, built.stats.features, built.bytes.len() / 1024, built.job.data_version, t0.elapsed().as_secs_f64());
        }
        Some("osm") => {
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            let paths = datars_geo_build::osm::convert(&pos[1..], &out)?;
            eprintln!("{}\n{}\n{}", paths.water, paths.roads, paths.buildings);
        }
        _ => return Err("usage: datars-geo-build atlas|tiles|camera|osm … (see src/main.rs)".into()),
    }
    Ok(())
}

/// Run a tile job over a data directory and write the archive, reporting where its size goes.
fn build(job: &datars_geo_build::TileJob, data: &std::path::Path, out: &str) -> Result<(), String> {
    let (bytes, stats) = datars_geo_build::tiles(job, &|p, regions| {
        eprintln!("  reading {p}");
        datars_geo_build::load_file(data, p, regions)
    })?;
    std::fs::write(out, bytes).map_err(|e| e.to_string())?;
    for (z, (n, b)) in &stats.by_zoom {
        eprintln!("  z{z:<2} {n:>5} tiles  {:>6} KB before compression", b / 1024);
    }
    eprintln!("{out}: {} tiles, {} features in, {} KB", stats.tiles, stats.features, stats.bytes / 1024);
    Ok(())
}
