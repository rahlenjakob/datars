use datars_theme::*;
use std::collections::BTreeMap;

fn resolved(set: &ThemeSet, name: &str, mode: Mode) -> ResolvedTheme {
    let chain = set.chain(name).unwrap();
    let (t, diags) = resolve(&chain, mode, &[]);
    assert!(diags.is_empty(), "{diags:?}");
    t
}

#[test]
fn neutral_resolves_in_every_mode_and_passes_its_checks() {
    let set = ThemeSet::with_builtins();
    for mode in [Mode::Light, Mode::Dark, Mode::HighContrast] {
        let t = resolved(&set, "datars/neutral", mode);
        assert!(t.color("grid").is_some());
        assert_eq!(t.palette("categorical").unwrap().colors.len(), 10);
        let errors: Vec<_> = validate(&t).into_iter().filter(|f| f.severity == Severity::Error).collect();
        assert!(errors.is_empty(), "{mode:?}: {errors:?}");
        // Warnings are allowed (e.g. colour-vision pairs) but every contrast check must pass.
        let contrast: Vec<_> = validate(&t).into_iter().filter(|f| f.check.starts_with("contrast")).collect();
        assert!(contrast.is_empty(), "{mode:?}: {contrast:?}");
    }
}

#[test]
fn derived_tokens_follow_the_mode() {
    let set = ThemeSet::with_builtins();
    let light = resolved(&set, "datars/neutral", Mode::Light);
    let dark = resolved(&set, "datars/neutral", Mode::Dark);
    // grid = mix($ink, $paper, 0.9): light grid is light, dark grid is dark — no dark-mode value needed.
    assert!(light.color("grid").unwrap().luminance() > 0.7);
    assert!(dark.color("grid").unwrap().luminance() < 0.1);
}

#[test]
fn custom_theme_extends_locks_and_overrides() {
    let mut set = ThemeSet::with_builtins();
    set.add(
        Theme::from_json(
            r##"{
        "name": "acme", "extends": "datars/neutral",
        "tokens": { "accent": "#b3261e", "brand": "#b3261e",
                    "categorical": { "generate": "categorical", "from": "$brand", "n": 6 } },
        "locked": ["accent"]
    }"##,
        )
        .unwrap(),
    );
    let chain = set.chain("acme").unwrap();
    assert_eq!(chain.len(), 2);
    let mut doc = BTreeMap::new();
    doc.insert("accent".to_string(), TokenValue::parse(&serde_json::json!("#00ff00")).unwrap());
    doc.insert("paper".to_string(), TokenValue::parse(&serde_json::json!("#fbf8f1")).unwrap());
    let (t, diags) = resolve(&chain, Mode::Light, &[&doc]);
    assert_eq!(t.color("accent").unwrap().to_hex(), "#b3261e", "locked accent wins over the document");
    assert_eq!(t.color("paper").unwrap().to_hex(), "#fbf8f1", "unlocked tokens can be overridden");
    assert_eq!(diags.len(), 1);
    assert_eq!(t.palette("categorical").unwrap().colors.len(), 6);
    // accent-ink = on($accent) re-derives for the new accent.
    assert_eq!(t.color("accent-ink").unwrap().to_hex(), "#ffffff");
}

#[test]
fn cycles_are_diagnosed_not_hung() {
    let t = Theme::from_json(r#"{ "name": "loop", "tokens": { "a": "$b", "b": "$a" } }"#).unwrap();
    let (r, diags) = resolve(&[&t], Mode::Light, &[]);
    assert!(r.color("a").is_none());
    assert!(diags.iter().any(|d| d.message.contains("cycle")));
}

#[test]
fn inks_resolve_late_and_themes_interpolate() {
    let set = ThemeSet::with_builtins();
    let light = resolved(&set, "datars/neutral", Mode::Light);
    let dark = resolved(&set, "datars/neutral", Mode::Dark);
    let ink = Ink::parse("$ink@0.5").unwrap();
    assert_eq!(ink.to_string(), "$ink@0.5");
    assert_eq!(ink.resolve(&light).a, 0.5);
    let p = Ink::parse("$categorical[12]").unwrap();
    assert_eq!(p.resolve(&light), light.palette("categorical").unwrap().get(2), "categorical cycles");
    let mid = ResolvedTheme::lerp(&light, &dark, 0.5);
    let l = mid.color("paper").unwrap().to_oklab().l;
    assert!(l > 0.3 && l < 0.8, "halfway between white and near-black paper: {l}");
    assert_eq!(Ink::parse("#e8112d").unwrap().to_string(), "#e8112d");
    let r = Ink::parse("$sequential~0.5").unwrap();
    assert_eq!(r.to_string(), "$sequential~0.5");
    let seq = light.palette("sequential").unwrap();
    assert_eq!(r.resolve(&light), seq.at(0.5));
}

#[test]
fn theme_json_round_trips() {
    let n = Theme::neutral();
    let again = Theme::from_json(&n.to_json()).unwrap();
    assert_eq!(n, again);
}

#[test]
fn a_brand_colour_survives_every_mode_and_shadowing_is_reported() {
    // A brand sets its accent (base only) and a warm paper (base only).
    let mut set = ThemeSet::with_builtins();
    set.add(Theme::from_json(r##"{ "name": "acme", "extends": "datars/neutral", "tokens": { "accent": "#b3261e", "paper": "#fffaf2" } }"##).unwrap());
    let chain = set.chain("acme").unwrap();
    let (light, d_light) = resolve(&chain, Mode::Light, &[]);
    assert!(d_light.is_empty(), "{d_light:?}");
    assert_eq!(light.color("accent").unwrap().to_hex(), "#b3261e");
    let (dark, d_dark) = resolve(&chain, Mode::Dark, &[]);
    assert_eq!(dark.color("accent").unwrap().to_hex(), "#b3261e", "the child's accent wins in dark mode too");
    // neutral varies both in dark mode: the child is told, token by token.
    let told: Vec<&str> = d_dark.iter().map(|d| d.token.as_str()).collect();
    assert_eq!(told, ["accent", "paper"], "{d_dark:?}");
    // With its own dark value, nothing to report and the dark value applies.
    set.add(Theme::from_json(r##"{ "name": "acme2", "extends": "datars/neutral", "tokens": { "accent": "#b3261e" }, "modes": { "dark": { "accent": "#f28b82" } } }"##).unwrap());
    let (dark2, d2) = resolve(&set.chain("acme2").unwrap(), Mode::Dark, &[]);
    assert!(d2.is_empty(), "{d2:?}");
    assert_eq!(dark2.color("accent").unwrap().to_hex(), "#f28b82");
}

#[test]
fn a_dark_first_theme_is_dark_in_light_mode_with_its_parents_dark_values() {
    // A magazine-style theme: dark paper and ink in its base, nothing about roads or ramps.
    let mut set = ThemeSet::with_builtins();
    set.add(Theme::from_json(r##"{ "name": "night", "extends": "datars/neutral", "scheme": "dark", "tokens": { "paper": "#0a0a0a", "ink": "#f0f0f0", "map.land": "#1d1d1d" } }"##).unwrap());
    let chain = set.chain("night").unwrap();
    let neutral = set.chain("datars/neutral").unwrap();
    let (n_dark, _) = resolve(&neutral, Mode::Dark, &[]);
    for mode in [Mode::Light, Mode::Dark] {
        let (r, diags) = resolve(&chain, mode, &[]);
        assert!(diags.is_empty(), "{mode:?}: {diags:?}");
        assert_eq!(r.color("paper").unwrap().to_hex(), "#0a0a0a");
        assert_eq!(r.color("map.land").unwrap().to_hex(), "#1d1d1d");
        // Unset tokens are the parent's dark ones in both modes, derived ones re-derive from its paper.
        assert_eq!(r.color("map.road"), n_dark.color("map.road"), "{mode:?}");
        assert_eq!(r.palette("sequential"), n_dark.palette("sequential"), "{mode:?}");
        assert!(r.color("grid").unwrap().to_hex() != n_dark.color("grid").unwrap().to_hex());
    }
    let t = Theme::from_json(&set.themes["night"].to_json()).unwrap();
    assert_eq!(t.scheme, Scheme::Dark, "scheme round-trips");
}

#[test]
fn on_inks_pick_the_readable_one_of_ink_and_paper_late() {
    let set = ThemeSet::with_builtins();
    let light = resolved(&set, "datars/neutral", Mode::Light);
    let dark = resolved(&set, "datars/neutral", Mode::Dark);
    let on_navy = Ink::parse("on(#1d4e89)").unwrap();
    let on_pale = Ink::parse("on(#fdc0d9)").unwrap();
    // On a dark fill: the lighter of ink/paper — paper in light mode, ink in dark mode.
    assert_eq!(on_navy.resolve(&light), light.color("paper").unwrap());
    assert_eq!(on_navy.resolve(&dark), dark.color("ink").unwrap());
    assert_eq!(on_pale.resolve(&light), light.color("ink").unwrap());
    // Tokens as the background, alpha, and the JSON form round-trips.
    let on_mark = Ink::parse("on($mark)").unwrap().fade(0.5);
    assert_eq!(on_mark.to_string(), "on($mark)@0.5");
    assert_eq!(Ink::parse(&on_mark.to_string()), Some(on_mark.clone()));
    assert!((on_mark.resolve(&light).a - 0.5).abs() < 1e-6);
    assert!(Ink::parse("on(nonsense").is_none());
}

/// Font tokens name where their face comes from — a file, a URL or a Google Fonts family — and
/// accept a family as a string or a stack. Without a family or a Google name they're an error.
#[test]
fn font_tokens_carry_their_source() {
    let parse = |v: serde_json::Value| match TokenValue::parse(&v) {
        Ok(TokenValue::Font(f)) => f,
        other => panic!("{v}: {other:?}"),
    };
    let file = parse(serde_json::json!({ "family": "Cambon", "weight": 700, "src": "fonts/Cambon-Bold.otf" }));
    assert_eq!(file.stack(), vec!["Cambon"]);
    assert_eq!(file.source_url().as_deref(), Some("fonts/Cambon-Bold.otf"));

    let google = parse(serde_json::json!({ "google": "Source Serif 4", "weight": 600, "italic": true }));
    assert_eq!(google.stack(), vec!["Source Serif 4"]);
    assert_eq!(google.source_url().as_deref(), Some("google:Source Serif 4:600italic"));
    assert_eq!(parse_google_url("google:Source Serif 4:600italic"), Some(("Source Serif 4".into(), 600, true)));
    assert_eq!(parse_google_url("google:Inter"), Some(("Inter".into(), 400, false)));
    assert_eq!(parse_google_url("fonts/x.ttf"), None);

    // A Google family leads the stack; listed fallbacks follow once.
    let stacked = parse(serde_json::json!({ "google": "Newsreader", "family": ["newsreader", "Georgia"] }));
    assert_eq!(stacked.stack(), vec!["Newsreader", "Georgia"]);
    assert_eq!(stacked.weight, 400);

    // Families only: no source (the build step can't ship it; `datars check` says so).
    let plain = parse(serde_json::json!({ "family": ["Inter"], "weight": 400 }));
    assert_eq!(plain.source_url(), None);

    assert!(TokenValue::parse(&serde_json::json!({ "family": [] })).is_err());
    // Round trip keeps the source.
    assert_eq!(parse(TokenValue::Font(google.clone()).to_json()), google);
}

/// The built-in theme's fonts are ordinary sourced tokens: Inter from the datars distribution.
#[test]
fn neutral_fonts_come_from_the_datars_distribution() {
    let t = resolved(&ThemeSet::with_builtins(), "datars/neutral", Mode::Light);
    for (token, file) in [("font.body", "Inter-Regular"), ("font.strong", "Inter-SemiBold"), ("font.title", "Inter-Bold"), ("font.number", "Inter-Regular")] {
        let f = t.font(token).unwrap();
        assert_eq!(f.source_url(), Some(format!("datars:fonts/{file}.ttf")), "{token}");
    }
}
