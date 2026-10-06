//! An engine-drawn select offered to a native picker: the host reads the choices from the status
//! and sets the signal to what the reader picked, as JSON.

use datars_ffi::*;
use std::ffi::CStr;

fn status(v: *mut DatarsView) -> serde_json::Value {
    let p = datars_view_status(v);
    let s = unsafe { CStr::from_ptr(p as *const std::ffi::c_char) }.to_string_lossy().into_owned();
    datars_string_free(p);
    serde_json::from_str(&s).unwrap()
}

#[test]
fn a_native_picker_reads_the_choices_and_sets_the_signal() {
    let doc = serde_json::json!({ "datars": 1, "size": { "width": 300, "height": 200 },
        "keys": { "SE": { "name": "Sweden" } },
        "signals": { "country": { "type": "str", "default": "SE" } },
        "scene": { "kind": "group", "key": "root", "children": [
            { "kind": "use", "key": "country", "recipe": "@datars/std/select", "params": { "signal": "country", "options": ["SE", "NO"], "label": "Country" } }] } })
    .to_string();
    let v = datars_view_new(1);
    datars_string_free(datars_view_load_doc(v, doc.as_ptr(), doc.len()));
    datars_view_resize(v, 300.0, 200.0, 1.0);
    datars_view_frame(v, 0.0);
    let s = status(v);
    let sel = s["controls"].as_array().unwrap().iter().find(|c| c["kind"] == "select").cloned().unwrap_or_else(|| panic!("{s}"));
    assert_eq!(sel["signal"], "country");
    assert_eq!(sel["current"], "SE");
    assert_eq!(sel["options"], serde_json::json!([{ "value": "SE", "label": "Sweden" }, { "value": "NO", "label": "NO" }]));
    assert!(sel["rect"][2].as_f64().unwrap() > 100.0, "{sel}");
    let (name, json) = ("country", "\"NO\"");
    assert_eq!(datars_view_set_signal_json(v, name.as_ptr(), name.len(), json.as_ptr(), json.len()), 1);
    datars_view_frame(v, 5.0);
    let s = status(v);
    assert_eq!(s["controls"].as_array().unwrap().iter().find(|c| c["kind"] == "select").unwrap()["current"], "NO");
    let bad = "not json";
    assert_eq!(datars_view_set_signal_json(v, name.as_ptr(), name.len(), bad.as_ptr(), bad.len()), 0);
    datars_view_free(v);
}

#[test]
fn the_host_shows_the_cursor_the_engine_asks_for() {
    let doc = serde_json::json!({ "datars": 1, "size": { "width": 300, "height": 200 },
        "signals": { "country": { "type": "str", "default": "SE" } },
        "scene": { "kind": "group", "key": "root", "children": [
            { "kind": "use", "key": "country", "recipe": "@datars/std/select", "params": { "signal": "country", "options": ["SE", "NO"], "label": "Country" } }] } })
    .to_string();
    let v = datars_view_new(1);
    datars_string_free(datars_view_load_doc(v, doc.as_ptr(), doc.len()));
    datars_view_resize(v, 300.0, 200.0, 1.0);
    datars_view_frame(v, 0.0);
    let rect = status(v)["controls"][0]["rect"].clone();
    let (x, y) = (rect[0].as_f64().unwrap() + 20.0, rect[1].as_f64().unwrap() + 10.0);
    let label = datars_view_pointer(v, 0, x, y);
    if !label.is_null() {
        datars_string_free(label);
    }
    assert_eq!(datars_view_cursor(v), 1, "a pointing hand over the select");
    let label = datars_view_pointer(v, 0, 290.0, 190.0);
    if !label.is_null() {
        datars_string_free(label);
    }
    assert_eq!(datars_view_cursor(v), 0, "an arrow over nothing");
    datars_view_free(v);
}
