//! Script subsets for text that arrives at runtime (host data slots, live URL sources): the
//! Unicode ranges Google Fonts splits families into, so a chart whose data isn't known at build
//! time ships each face's scripts as lazily loaded chunks instead of whole fonts.

/// (name, CSS `unicode-range`) — Google Fonts' subset definitions for the scripts charts meet
/// most. Characters outside all of them fall into per-4096-code-point blocks.
pub const SCRIPTS: &[(&str, &str)] = &[
    ("latin", "U+0000-00FF,U+0131,U+0152-0153,U+02BB-02BC,U+02C6,U+02DA,U+02DC,U+0304,U+0308,U+0329,U+2000-206F,U+20AC,U+2122,U+2191,U+2193,U+2212,U+2215,U+FEFF,U+FFFD"),
    ("latin-ext", "U+0100-02BA,U+02BD-02C5,U+02C7-02CC,U+02CE-02D7,U+02DD-02FF,U+0304,U+0308,U+0329,U+1D00-1DBF,U+1E00-1E9F,U+1EF2-1EFF,U+2020,U+20A0-20AB,U+20AD-20C0,U+2113,U+2C60-2C7F,U+A720-A7FF"),
    ("vietnamese", "U+0102-0103,U+0110-0111,U+0128-0129,U+0168-0169,U+01A0-01A1,U+01AF-01B0,U+0300-0301,U+0303-0304,U+0308-0309,U+0323,U+0329,U+1EA0-1EF9,U+20AB"),
    ("greek", "U+0370-0377,U+037A-037F,U+0384-038A,U+038C,U+038E-03A1,U+03A3-03FF"),
    ("greek-ext", "U+1F00-1FFF"),
    ("cyrillic", "U+0301,U+0400-045F,U+0490-0491,U+04B0-04B1,U+2116"),
    ("cyrillic-ext", "U+0460-052F,U+1C80-1C8A,U+20B4,U+2DE0-2DFF,U+A640-A69F,U+FE2E-FE2F"),
    ("hebrew", "U+0307-0308,U+0590-05FF,U+200C-2010,U+20AA,U+25CC,U+FB1D-FB4F"),
    ("arabic", "U+0600-06FF,U+0750-077F,U+0870-088E,U+0890-0891,U+0897-08E1,U+08E3-08FF,U+200C-200E,U+2010-2011,U+204F,U+2E41,U+FB50-FDFF,U+FE70-FE74,U+FE76-FEFC"),
    ("symbols", "U+2100-214F,U+2190-21FF,U+2200-22FF,U+2300-23FF,U+25A0-25FF,U+2600-26FF,U+2700-27BF"),
];

/// Parse a `unicode-range` list into inclusive ranges.
pub fn ranges(list: &str) -> Vec<(u32, u32)> {
    list.split(',')
        .filter_map(|r| {
            let r = r.trim().trim_start_matches("U+").trim_start_matches("u+");
            let (a, b) = r.split_once('-').unwrap_or((r, r));
            Some((u32::from_str_radix(a, 16).ok()?, u32::from_str_radix(b, 16).ok()?))
        })
        .collect()
}

/// A `unicode-range` list naming exactly `chars` (runs of consecutive code points merged), so a
/// runtime fetches a part only for characters it really has.
pub fn exact_ranges(chars: &[char]) -> String {
    let mut cps: Vec<u32> = chars.iter().map(|&c| c as u32).collect();
    cps.sort_unstable();
    cps.dedup();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < cps.len() {
        let mut j = i;
        while j + 1 < cps.len() && cps[j + 1] == cps[j] + 1 {
            j += 1;
        }
        out.push(if i == j { format!("U+{:04X}", cps[i]) } else { format!("U+{:04X}-{:04X}", cps[i], cps[j]) });
        i = j + 1;
    }
    out.join(",")
}

/// Group a font's characters into script subsets: every named script it covers, then
/// 4096-code-point blocks for the rest. Returns (name, unicode-range of exactly its characters,
/// characters), in order.
pub fn split(chars: &[char]) -> Vec<(String, String, Vec<char>)> {
    let named: Vec<(&str, Vec<(u32, u32)>)> = SCRIPTS.iter().map(|(n, r)| (*n, ranges(r))).collect();
    let mut out: Vec<(String, String, Vec<char>)> = named.iter().map(|(n, _)| (n.to_string(), String::new(), Vec::new())).collect();
    let mut rest: std::collections::BTreeMap<u32, Vec<char>> = std::collections::BTreeMap::new();
    for &c in chars {
        let cp = c as u32;
        let mut hit = false;
        // The first script that lists it (the lists overlap on shared punctuation and marks).
        for (i, (_, rs)) in named.iter().enumerate() {
            if rs.iter().any(|&(a, b)| (a..=b).contains(&cp)) {
                out[i].2.push(c);
                hit = true;
                break;
            }
        }
        if !hit {
            rest.entry(cp >> 12).or_default().push(c);
        }
    }
    out.retain(|(_, _, cs)| !cs.is_empty());
    for (block, cs) in rest {
        out.push((format!("u{:04x}", block << 12), String::new(), cs));
    }
    for part in &mut out {
        part.1 = exact_ranges(&part.2);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn characters_group_by_script_then_block() {
        let groups = split(&['A', 'é', 'Ł', 'ж', 'א', '中']);
        let names: Vec<&str> = groups.iter().map(|g| g.0.as_str()).collect();
        assert_eq!(names, vec!["latin", "latin-ext", "cyrillic", "hebrew", "u4000"]);
        assert_eq!(groups[0].2, vec!['A', 'é']);
        assert_eq!(groups[4].1, "U+4E2D");
        assert_eq!(exact_ranges(&['a', 'b', 'c', 'x']), "U+0061-0063,U+0078");
        assert_eq!(ranges("U+0000-00FF,U+0131"), vec![(0, 0xFF), (0x131, 0x131)]);
    }
}
