//! Signed publishing: `datars keygen` makes a key, `publish --sign` signs the manifest with it, and
//! the runtimes' check (`Manifest::verify`, what `<datars-view publishers>` runs) accepts it for that
//! key only — not for another key, not after a byte of it changed.

use std::process::Command;

fn datars(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_datars")).args(args).output().expect("the CLI runs")
}

#[test]
fn a_published_chart_signed_with_a_new_key_verifies_for_that_key_only() {
    let dir = std::env::temp_dir().join(format!("datars-signing-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (key, other) = (dir.join("a.key"), dir.join("b.key"));
    let k = datars(&["keygen", "--out", key.to_str().unwrap(), "--json"]);
    assert!(k.status.success(), "{}", String::from_utf8_lossy(&k.stderr));
    let publisher = serde_json::from_slice::<serde_json::Value>(&k.stdout).unwrap()["publisher"].as_str().unwrap().to_string();
    assert!(publisher.starts_with("ed25519:"));
    assert!(!datars(&["keygen", "--out", key.to_str().unwrap()]).status.success(), "an existing key is never overwritten");
    let other_pub = serde_json::from_slice::<serde_json::Value>(&datars(&["keygen", "--out", other.to_str().unwrap(), "--json"]).stdout).unwrap()["publisher"].as_str().unwrap().to_string();

    let doc = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/votes/doc.json");
    let site = dir.join("site");
    let p = datars(&["publish", doc, "--alias", "votes", "--to", site.to_str().unwrap(), "--sign", key.to_str().unwrap(), "--json"]);
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    assert_eq!(serde_json::from_slice::<serde_json::Value>(&p.stdout).unwrap()["publisher"], publisher.as_str());

    let bytes = std::fs::read(site.join("c/votes")).unwrap();
    let m = datars_bundle::Manifest::from_json(&bytes).unwrap();
    assert!(m.verify(&[publisher.clone()]).is_ok(), "signed by the key the page names");
    assert!(m.verify(&[other_pub]).is_err(), "not a key the page names");
    // One field changed in transit: the signature no longer matches.
    let mut tampered = m.clone();
    tampered.revision = format!("{}-x", tampered.revision);
    assert!(tampered.verify(&[publisher]).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
