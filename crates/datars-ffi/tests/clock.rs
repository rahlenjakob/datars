//! Hosts render on demand: after idling, an input must start its transition at the host's time,
//! not at the last frame's (which would count the transition as long started and jump to its end).

use datars_ffi::*;

#[test]
fn a_transition_after_idling_plays_from_the_input() {
    let doc = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/votes/doc.json")).unwrap();
    let run = |sync: bool| {
        let v = datars_view_new(1);
        datars_string_free(datars_view_load_doc(v, doc.as_ptr(), doc.len()));
        datars_view_resize(v, 320.0, 240.0, 1.0);
        datars_view_frame(v, 0.0);
        datars_view_frame(v, 5.0); // settled; the host stops rendering
        if sync {
            datars_view_set_clock(v, 60.0);
        }
        assert_eq!(datars_view_event(v, b"next".as_ptr(), 4), 1);
        let moving = datars_view_frame(v, 60.1);
        assert_eq!(datars_view_animating(v), moving, "the flag is the last frame's");
        datars_view_free(v);
        moving == 1
    };
    assert!(run(true), "with the clock told, the transition plays");
    assert!(!run(false), "without it, it is already over (the bug set_clock exists for)");
}
