//! P1 through the C ABI: every example state renders to the goldens' pixels. Runs on the host with
//! `cargo test`, and on devices (Android via adb, see scripts/test-android.sh) with `DATARS_ROOT`
//! pointing at a copy of `examples/` and `tests/golden/`.

use datars_ffi::*;
use std::path::PathBuf;

fn root() -> PathBuf {
    std::env::var("DATARS_ROOT").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

#[test]
fn examples_render_to_the_golden_pixels() {
    let root = root();
    let mut checked = 0;
    for name in ["votes", "riksdag", "business", "flows", "inflation", "shapes", "warming", "dashboard"] {
        let doc = std::fs::read(root.join(format!("examples/{name}/doc.json"))).expect("example doc");
        let golden: serde_json::Value = serde_json::from_slice(&std::fs::read(root.join(format!("tests/golden/{name}/golden.json"))).expect("golden")).unwrap();
        let size: serde_json::Value = serde_json::from_slice::<serde_json::Value>(&doc).unwrap()["size"].clone();
        let v = datars_view_new(1);
        datars_string_free(datars_view_load_doc(v, doc.as_ptr(), doc.len()));
        datars_view_resize(v, size["width"].as_f64().unwrap(), size["height"].as_f64().unwrap(), 1.0);
        for (i, st) in golden["states"].as_array().unwrap().iter().enumerate() {
            datars_view_goto(v, i as u32);
            datars_view_frame(v, i as f64 * 100.0 + 50.0); // long after any transition
            assert_eq!(format!("{:016x}", datars_view_pixel_hash(v)), st["pixels"].as_str().unwrap(), "{name} / {}", st["name"]);
            checked += 1;
        }
        datars_view_free(v);
    }
    assert!(checked >= 18, "{checked} states");
    eprintln!("{checked} states match the goldens on {}", std::env::consts::ARCH);
}
