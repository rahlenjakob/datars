//! Locales for number and date formatting: separators, currency affixes, month and weekday names.
//!
//! Values follow d3-format / d3-time-format locale definitions (and CLDR where d3 has none), with
//! one deliberate change: currency affixes use a no-break space so "1 234 kr" never wraps apart.

/// Number and date conventions of one locale. Weekday arrays start on Sunday.
#[derive(Debug, PartialEq, Eq)]
pub struct Locale {
    /// BCP 47 tag (`"en"`, `"sv"`, `"en-GB"`, …).
    pub tag: &'static str,
    pub decimal: &'static str,
    /// Thousands separator (a no-break space where the convention is a space).
    pub group: &'static str,
    /// Digit group sizes from the right, repeating the last (d3's `grouping`).
    pub grouping: &'static [usize],
    pub currency_prefix: &'static str,
    pub currency_suffix: &'static str,
    pub percent: &'static str,
    /// Minus sign (U+2212, as d3-format uses by default).
    pub minus: &'static str,
    pub months: [&'static str; 12],
    pub months_short: [&'static str; 12],
    pub days: [&'static str; 7],
    pub days_short: [&'static str; 7],
    /// A day and month on a time axis: `18 May` in most of Europe and in British English, `May 18`
    /// in American English.
    pub day_month: &'static str,
}

const MINUS: &str = "\u{2212}";
const NBSP: &str = "\u{a0}";

pub static EN: Locale = EN_BASE;

pub static EN_GB: Locale = Locale { tag: "en-GB", currency_prefix: "£", ..EN_BASE };

/// American English: month before day (`May 18`).
pub static EN_US: Locale = Locale { tag: "en-US", day_month: "%b %-d", ..EN_BASE };

// Shared by the English variants (a `const` so struct-update syntax can copy it).
const EN_BASE: Locale = Locale {
    tag: "en",
    decimal: ".",
    group: ",",
    grouping: &[3],
    currency_prefix: "$",
    currency_suffix: "",
    percent: "%",
    minus: MINUS,
    months: ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"],
    months_short: ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"],
    days: ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"],
    days_short: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
    day_month: "%-d %b",
};

pub static SV: Locale = Locale {
    tag: "sv",
    decimal: ",",
    group: NBSP,
    grouping: &[3],
    currency_prefix: "",
    currency_suffix: "\u{a0}kr",
    percent: "%",
    minus: MINUS,
    months: ["januari", "februari", "mars", "april", "maj", "juni", "juli", "augusti", "september", "oktober", "november", "december"],
    months_short: ["jan", "feb", "mar", "apr", "maj", "jun", "jul", "aug", "sep", "okt", "nov", "dec"],
    days: ["söndag", "måndag", "tisdag", "onsdag", "torsdag", "fredag", "lördag"],
    days_short: ["sön", "mån", "tis", "ons", "tor", "fre", "lör"],
    day_month: "%-d %b",
};

pub static DE: Locale = Locale {
    tag: "de",
    decimal: ",",
    group: ".",
    grouping: &[3],
    currency_prefix: "",
    currency_suffix: "\u{a0}€",
    percent: "%",
    minus: MINUS,
    months: ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"],
    months_short: ["Jan", "Feb", "Mrz", "Apr", "Mai", "Jun", "Jul", "Aug", "Sep", "Okt", "Nov", "Dez"],
    days: ["Sonntag", "Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag"],
    days_short: ["So", "Mo", "Di", "Mi", "Do", "Fr", "Sa"],
    day_month: "%-d %b",
};

pub static FR: Locale = Locale {
    tag: "fr",
    decimal: ",",
    group: NBSP,
    grouping: &[3],
    currency_prefix: "",
    currency_suffix: "\u{a0}€",
    percent: "\u{202f}%",
    minus: MINUS,
    months: ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"],
    months_short: ["janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc."],
    days: ["dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi"],
    days_short: ["dim.", "lun.", "mar.", "mer.", "jeu.", "ven.", "sam."],
    day_month: "%-d %b",
};

pub static ES: Locale = Locale {
    tag: "es",
    decimal: ",",
    group: ".",
    grouping: &[3],
    currency_prefix: "",
    currency_suffix: "\u{a0}€",
    percent: "%",
    minus: MINUS,
    months: ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"],
    months_short: ["ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic"],
    days: ["domingo", "lunes", "martes", "miércoles", "jueves", "viernes", "sábado"],
    days_short: ["dom", "lun", "mar", "mié", "jue", "vie", "sáb"],
    day_month: "%-d %b",
};

pub static IT: Locale = Locale {
    tag: "it",
    decimal: ",",
    group: ".",
    grouping: &[3],
    currency_prefix: "",
    currency_suffix: "\u{a0}€",
    percent: "%",
    minus: MINUS,
    months: ["gennaio", "febbraio", "marzo", "aprile", "maggio", "giugno", "luglio", "agosto", "settembre", "ottobre", "novembre", "dicembre"],
    months_short: ["gen", "feb", "mar", "apr", "mag", "giu", "lug", "ago", "set", "ott", "nov", "dic"],
    days: ["domenica", "lunedì", "martedì", "mercoledì", "giovedì", "venerdì", "sabato"],
    days_short: ["dom", "lun", "mar", "mer", "gio", "ven", "sab"],
    day_month: "%-d %b",
};

pub static NL: Locale = Locale {
    tag: "nl",
    decimal: ",",
    group: ".",
    grouping: &[3],
    currency_prefix: "€\u{a0}",
    currency_suffix: "",
    percent: "%",
    minus: MINUS,
    months: ["januari", "februari", "maart", "april", "mei", "juni", "juli", "augustus", "september", "oktober", "november", "december"],
    months_short: ["jan", "feb", "mrt", "apr", "mei", "jun", "jul", "aug", "sep", "okt", "nov", "dec"],
    days: ["zondag", "maandag", "dinsdag", "woensdag", "donderdag", "vrijdag", "zaterdag"],
    days_short: ["zo", "ma", "di", "wo", "do", "vr", "za"],
    day_month: "%-d %b",
};

pub static NB: Locale = Locale {
    tag: "nb",
    decimal: ",",
    group: NBSP,
    grouping: &[3],
    currency_prefix: "",
    currency_suffix: "\u{a0}kr",
    percent: "\u{a0}%",
    minus: MINUS,
    months: ["januar", "februar", "mars", "april", "mai", "juni", "juli", "august", "september", "oktober", "november", "desember"],
    months_short: ["jan", "feb", "mar", "apr", "mai", "jun", "jul", "aug", "sep", "okt", "nov", "des"],
    days: ["søndag", "mandag", "tirsdag", "onsdag", "torsdag", "fredag", "lørdag"],
    days_short: ["søn", "man", "tir", "ons", "tor", "fre", "lør"],
    day_month: "%-d %b",
};

pub static DA: Locale = Locale {
    tag: "da",
    decimal: ",",
    group: ".",
    grouping: &[3],
    currency_prefix: "",
    currency_suffix: "\u{a0}kr.",
    percent: "\u{a0}%",
    minus: MINUS,
    months: ["januar", "februar", "marts", "april", "maj", "juni", "juli", "august", "september", "oktober", "november", "december"],
    months_short: ["jan", "feb", "mar", "apr", "maj", "jun", "jul", "aug", "sep", "okt", "nov", "dec"],
    days: ["søndag", "mandag", "tirsdag", "onsdag", "torsdag", "fredag", "lørdag"],
    days_short: ["søn", "man", "tir", "ons", "tor", "fre", "lør"],
    day_month: "%-d %b",
};

pub static FI: Locale = Locale {
    tag: "fi",
    decimal: ",",
    group: NBSP,
    grouping: &[3],
    currency_prefix: "",
    currency_suffix: "\u{a0}€",
    percent: "\u{a0}%",
    minus: MINUS,
    months: [
        "tammikuu", "helmikuu", "maaliskuu", "huhtikuu", "toukokuu", "kesäkuu", "heinäkuu", "elokuu", "syyskuu", "lokakuu", "marraskuu",
        "joulukuu",
    ],
    months_short: ["tammi", "helmi", "maalis", "huhti", "touko", "kesä", "heinä", "elo", "syys", "loka", "marras", "joulu"],
    days: ["sunnuntai", "maanantai", "tiistai", "keskiviikko", "torstai", "perjantai", "lauantai"],
    days_short: ["su", "ma", "ti", "ke", "to", "pe", "la"],
    day_month: "%-d %b",
};

/// Every built-in locale.
pub static ALL: &[&Locale] = &[&EN, &EN_GB, &EN_US, &SV, &DE, &FR, &ES, &IT, &NL, &NB, &DA, &FI];

/// The locale for a BCP 47 tag: exact match (case-insensitive, `_` accepted for `-`), then the
/// primary language (`"sv-SE"` → `sv`; `"no"`/`"nn"` → `nb`), then English.
pub fn get(tag: &str) -> &'static Locale {
    let norm: String = tag.trim().chars().map(|c| if c == '_' { '-' } else { c.to_ascii_lowercase() }).collect();
    if let Some(l) = ALL.iter().find(|l| l.tag.eq_ignore_ascii_case(&norm)) {
        return l;
    }
    let lang = norm.split('-').next().unwrap_or("");
    let lang = match lang {
        "no" | "nn" => "nb",
        other => other,
    };
    ALL.iter().find(|l| l.tag == lang).copied().unwrap_or(&EN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_and_fallback() {
        assert_eq!(get("sv").tag, "sv");
        assert_eq!(get("sv-SE").tag, "sv");
        assert_eq!(get("SV_se").tag, "sv");
        assert_eq!(get("en-GB").tag, "en-GB");
        assert_eq!(get("en-us").tag, "en-US");
        assert_eq!((get("en-US").day_month, get("en").day_month, get("sv").day_month), ("%b %-d", "%-d %b", "%-d %b"), "American English puts the month first");
        assert_eq!(get("no").tag, "nb");
        assert_eq!(get("nb-NO").tag, "nb");
        assert_eq!(get("xx").tag, "en");
        assert_eq!(get("").tag, "en");
    }

    #[test]
    fn required_locales_exist_with_separators() {
        for tag in ["en", "sv", "de", "fr", "es", "nb", "da", "fi"] {
            let l = get(tag);
            assert_eq!(l.tag, tag);
            assert!(!l.decimal.is_empty() && !l.group.is_empty());
            assert_ne!(l.decimal, l.group);
            assert!(l.months.iter().chain(&l.months_short).chain(&l.days).chain(&l.days_short).all(|s| !s.is_empty()));
        }
        assert_eq!(get("sv").decimal, ",");
        assert_eq!(get("sv").group, "\u{a0}");
        assert_eq!(get("en-GB").months[0], "January");
    }
}
