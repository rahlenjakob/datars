//! The words hosts and the summary say around a chart's own text, in the document's language
//! (`doc.locale`): "8 data points, from 3.5 to 27.6", "Step 2 of 4", "215 regions without data".
//! A table of phrases per language, `{n}`-style holes filled by the caller; a language missing a
//! phrase (or missing altogether) says it in English. Add a language by adding a table.

/// (key, phrase) for one language. Plural pairs are `….one` and `….other`.
type Table = &'static [(&'static str, &'static str)];

const EN: Table = &[
    ("points.one", "{n} data point"),
    ("points.other", "{n} data points"),
    ("series", " in {n} series"),
    ("range", ", from {lo} to {hi}"),
    ("missing", "; {n} without data"),
    ("regions-missing.one", "{n} region without data"),
    ("regions-missing.other", "{n} regions without data"),
    ("points-missing.one", "{n} data point without data"),
    ("points-missing.other", "{n} data points without data"),
    ("step", "Step {i} of {n}"),
    ("note", "Note: {text}"),
    ("legend", "Legend"),
    ("data", "Data"),
    ("controls", "Controls"),
    ("chart", "Chart"),
    ("more", "{n} more"),
];

const SV: Table = &[
    ("points.one", "{n} datapunkt"),
    ("points.other", "{n} datapunkter"),
    ("series", " i {n} serier"),
    ("range", ", från {lo} till {hi}"),
    ("missing", "; {n} utan data"),
    ("regions-missing.one", "{n} område utan data"),
    ("regions-missing.other", "{n} områden utan data"),
    ("points-missing.one", "{n} datapunkt utan data"),
    ("points-missing.other", "{n} datapunkter utan data"),
    ("step", "Steg {i} av {n}"),
    ("note", "Anteckning: {text}"),
    ("legend", "Teckenförklaring"),
    ("data", "Data"),
    ("controls", "Reglage"),
    ("chart", "Diagram"),
    ("more", "{n} till"),
];

const LANGUAGES: &[(&str, Table)] = &[("en", EN), ("sv", SV)];

/// The table for a locale (`sv`, `sv-SE`, `sv_FI` → Swedish), else English.
fn table(locale: &str) -> Table {
    let lang = locale.split(['-', '_']).next().unwrap_or("").to_ascii_lowercase();
    LANGUAGES.iter().find(|(l, _)| *l == lang).map(|(_, t)| *t).unwrap_or(EN)
}

/// The phrase `key` in `locale`'s language, its `{name}` holes filled from `args`.
pub fn phrase(locale: &str, key: &str, args: &[(&str, &str)]) -> String {
    let raw = table(locale).iter().chain(EN.iter()).find(|(k, _)| *k == key).map(|(_, p)| *p).unwrap_or(key);
    let mut out = raw.to_string();
    for (name, value) in args {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

/// A counted phrase: `key.one` for one, `key.other` otherwise, with `{n}` as written (`n_text`).
pub fn counted(locale: &str, key: &str, n: usize, n_text: &str) -> String {
    phrase(locale, &format!("{key}.{}", if n == 1 { "one" } else { "other" }), &[("n", n_text)])
}

/// Every phrase of a locale's language (English where it has none), for hosts that say them
/// themselves (the web mirror's captions, a step announcement): `{key: phrase}`.
pub fn all(locale: &str) -> std::collections::BTreeMap<String, String> {
    EN.iter().map(|(k, _)| (k.to_string(), phrase(locale, k, &[]))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phrases_follow_the_language_and_fall_back_to_english() {
        assert_eq!(counted("sv-SE", "points", 3, "3"), "3 datapunkter");
        assert_eq!(counted("sv", "points", 1, "1"), "1 datapunkt");
        assert_eq!(phrase("sv", "step", &[("i", "2"), ("n", "4")]), "Steg 2 av 4");
        assert_eq!(phrase("fi", "step", &[("i", "2"), ("n", "4")]), "Step 2 of 4", "a language without a table says it in English");
        assert_eq!(counted("en", "regions-missing", 215, "215"), "215 regions without data");
        // Every language has every phrase English has (a missing one would silently be English).
        for (lang, t) in LANGUAGES {
            for (k, _) in EN {
                assert!(t.iter().any(|(k2, _)| k2 == k), "{lang} lacks {k}");
            }
        }
    }
}
