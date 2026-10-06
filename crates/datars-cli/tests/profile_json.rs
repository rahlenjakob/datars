//! `datars profile --json` is one JSON document (scripts read it: bench-engine.mjs): every state's
//! build times and every transition's frames, nothing else on stdout.

use std::process::Command;

#[test]
fn profile_json_is_one_document_with_states_and_transitions() {
    let doc = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/votes/doc.json");
    let out = Command::new(env!("CARGO_BIN_EXE_datars")).args(["profile", doc, "--json"]).output().expect("the CLI runs");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let j: serde_json::Value = serde_json::from_slice(&out.stdout).expect("stdout is JSON and only JSON");
    let states = j["states"].as_array().expect("states");
    assert_eq!(states.len(), 4);
    assert!(states.iter().all(|s| s["name"].is_string() && s["cold_ms"].as_f64().is_some_and(|v| v >= 0.0) && s["ops"].as_u64().is_some()));
    let transitions = j["transitions"].as_array().expect("transitions");
    assert!(!transitions.is_empty());
    assert!(transitions.iter().all(|t| t["frames"].as_array().is_some_and(|f| !f.is_empty()) && t["unprepared_input_ms"].is_number()));
}
