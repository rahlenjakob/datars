use datars_bundle::*;
use std::collections::BTreeMap;
use std::sync::Arc;

fn sample(sign: bool) -> Bundle {
    let mut b = Builder::new();
    let poster = b.chunk("poster", b"<svg/>".to_vec(), BTreeMap::new());
    let text = b.chunk("a11y", b"Vote share by party".to_vec(), BTreeMap::new());
    let mut meta = BTreeMap::new();
    meta.insert("state".to_string(), serde_json::json!("bars"));
    let scene = b.chunk("scene", b"{\"scene\":1}".to_vec(), meta);
    let font = b.chunk("font-subset", vec![7u8; 100], BTreeMap::new());
    b.mark(&font, true, true, None);
    b.variant(Tier::T0, Requires::default(), &[poster.clone(), text.clone()], serde_json::json!({}));
    b.variant(Tier::T1, Requires { runtime: ">=1.0".into(), modules: vec!["core".into()], ..Default::default() }, &[poster.clone(), text.clone(), scene, font], serde_json::json!({"states": ["bars"]}));
    b.variant(Tier::T3, Requires { runtime: ">=1.3".into(), modules: vec!["core".into(), "sandbox".into()], ..Default::default() }, &[poster, text], serde_json::json!({}));
    let mut out = b.finish("votes", "r1");
    if sign {
        out.manifest.sign(&[42u8; 32]);
    }
    out
}

fn caps(v: &str, modules: &[&str], script: bool, publishers: Vec<String>) -> Capabilities {
    Capabilities { runtime: Version::parse(v).unwrap(), modules: modules.iter().map(|s| s.to_string()).collect(), allow_script: script, publishers, packages: Default::default() }
}

#[test]
fn variant_selection_follows_capabilities_and_policy() {
    let b = sample(false);
    assert_eq!(b.manifest.variants[0].tier, Tier::T3, "best first");
    assert_eq!(b.manifest.select(&caps("1.4", &["core", "sandbox"], true, vec![])).unwrap().tier, Tier::T3);
    assert_eq!(b.manifest.select(&caps("1.4", &["core", "sandbox"], false, vec![])).unwrap().tier, Tier::T1, "host forbids script");
    assert_eq!(b.manifest.select(&caps("1.2", &["core"], true, vec![])).unwrap().tier, Tier::T1, "old runtime");
    assert_eq!(b.manifest.select(&caps("0.9", &[], true, vec![])).unwrap().tier, Tier::T0, "always a static fallback");
}

#[test]
fn loader_fetches_only_what_is_missing_and_verifies() {
    let b = sample(false);
    let mut store = MemStore::default();
    let c = caps("1.2", &["core"], false, vec![]);
    let mut l = Loader::open(&b.manifest.to_json(), &c, &store).unwrap();
    assert_eq!(l.requests(), vec![l.variant.entry.clone()], "entry first");
    let entry = l.variant.entry.clone();
    l.provide(&entry, Arc::from(b.chunks[&entry].clone()), &mut store).unwrap();
    let wanted = l.requests();
    assert_eq!(wanted.len(), 3, "poster, text, scene — the font subset is lazy");
    assert!(l.provide(&wanted[0], Arc::from(b"tampered".to_vec()), &mut store).is_err());
    for h in wanted {
        l.provide(&h, Arc::from(b.chunks[&h].clone()), &mut store).unwrap();
    }
    assert!(l.is_ready());
    assert!(l.find("scene", Some(("state", "bars"))).is_some());
    // A second open with a warm store needs nothing.
    let l2 = Loader::open(&b.manifest.to_json(), &c, &store).unwrap();
    assert!(l2.is_ready(), "warm cache: zero requests");
}

#[test]
fn signatures_and_publisher_pinning() {
    let b = sample(true);
    let me = public_key_for(&[42u8; 32]);
    assert!(b.manifest.verify(&[me.clone()]).is_ok());
    assert!(b.manifest.verify(&["ed25519:00".into()]).is_err(), "not an allowed publisher");
    let mut t = b.manifest.clone();
    t.title = Some("tampered".into());
    assert!(t.verify(&[me.clone()]).is_err(), "any change breaks the signature");
    assert!(sample(false).manifest.verify(&[me]).is_err(), "unsigned but pinned");
}

#[test]
fn single_file_round_trip() {
    let b = sample(true);
    let file = to_single_file(&b);
    assert_eq!(&file[..8], MAGIC);
    let back = from_single_file(&file).unwrap();
    assert_eq!(back.manifest, b.manifest);
    assert_eq!(back.chunks, b.chunks);
    let mut bad = file.clone();
    let n = bad.len();
    bad[n - 1] ^= 1;
    assert!(from_single_file(&bad).is_err(), "corruption is detected");
}

#[test]
fn versions() {
    let v = Version::parse("1.4.2").unwrap();
    assert!(satisfies(&v, ">=1.2"));
    assert!(satisfies(&v, ">=1.2, <2"));
    assert!(!satisfies(&v, "<1.4"));
    assert!(satisfies(&v, "=1.4.2"));
}
