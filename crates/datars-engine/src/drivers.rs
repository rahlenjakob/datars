//! Program drivers (docs/08-programs.md): what moves a program besides explicit events.
//!
//! - **autoplay** — each state holds for its `hold` seconds (default 2.5), then `next`; a story with
//!   a loop edge wraps. Hosts can pause (`set_playing(false)`: reader interaction, reduced motion,
//!   off-screen).
//! - **scroll scrub** — the host maps scroll progress to a program position (`seek(2.4)` = 40 % of
//!   the way from state 2 to state 3) and the engine shows that exact frame of the planned
//!   transition. No clock is involved, so scrubbing back and forth is exact.
//! - **live sources** — sources with `live.every` are re-requested on schedule; the new bytes arrive
//!   through `provide` and the change animates like any other.
//!
//! Frames report `wake_at`: the next host time at which something will happen without input, so a
//! host can sleep (a timer, not a spinning frame loop) between holds and refreshes.

use datars_ir::Program;

/// Does the program declare `driver`? Drivers are strings (`"autoplay"`) or one-key objects
/// (`{"scroll": "scrub"}`).
pub fn has(program: Option<&Program>, driver: &str) -> bool {
    program.is_some_and(|p| {
        p.drivers.iter().any(|d| match d {
            serde_json::Value::String(s) => s == driver,
            serde_json::Value::Object(o) => o.contains_key(driver),
            _ => false,
        })
    })
}

/// Split a program position into (state, fraction toward the next state), clamped to the states.
pub fn split(pos: f64, states: usize) -> (usize, f64) {
    if states == 0 || !pos.is_finite() {
        return (0, 0.0);
    }
    let max = (states - 1) as f64;
    let p = pos.clamp(0.0, max);
    let i = p.floor() as usize;
    if i + 1 >= states {
        (states - 1, 0.0)
    } else {
        (i, p - i as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_split_into_state_and_fraction() {
        assert_eq!(split(0.0, 4), (0, 0.0));
        assert_eq!(split(2.25, 4), (2, 0.25));
        assert_eq!(split(3.0, 4), (3, 0.0));
        assert_eq!(split(9.0, 4), (3, 0.0));
        assert_eq!(split(-1.0, 4), (0, 0.0));
        assert_eq!(split(f64::NAN, 4), (0, 0.0));
        assert_eq!(split(0.5, 0), (0, 0.0));
    }

    #[test]
    fn drivers_are_strings_or_objects() {
        let p: Program = serde_json::from_value(serde_json::json!({ "states": [], "drivers": ["steps", { "scroll": "scrub" }] })).unwrap();
        assert!(has(Some(&p), "steps"));
        assert!(has(Some(&p), "scroll"));
        assert!(!has(Some(&p), "autoplay"));
        assert!(!has(None, "autoplay"));
    }
}
