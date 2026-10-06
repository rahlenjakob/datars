//! Minimal version requirements: `>=1.2`, `<2`, `=1.4.1`, comma/space-separated conjunctions.

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u32, pub u32, pub u32);

impl Version {
    pub fn parse(s: &str) -> Option<Version> {
        let mut it = s.trim().split('.').map(|p| p.trim().parse::<u32>());
        let a = it.next()?.ok()?;
        let b = it.next().transpose().ok()?.unwrap_or(0);
        let c = it.next().transpose().ok()?.unwrap_or(0);
        Some(Version(a, b, c))
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

pub fn satisfies(v: &Version, req: &str) -> bool {
    req.split([',', ' ']).filter(|s| !s.trim().is_empty()).all(|c| {
        let c = c.trim();
        let (op, rest) = if let Some(r) = c.strip_prefix(">=") {
            (">=", r)
        } else if let Some(r) = c.strip_prefix("<=") {
            ("<=", r)
        } else if let Some(r) = c.strip_prefix('>') {
            (">", r)
        } else if let Some(r) = c.strip_prefix('<') {
            ("<", r)
        } else if let Some(r) = c.strip_prefix('=') {
            ("=", r)
        } else {
            (">=", c)
        };
        let Some(w) = Version::parse(rest) else { return false };
        match op {
            ">=" => *v >= w,
            "<=" => *v <= w,
            ">" => *v > w,
            "<" => *v < w,
            _ => *v == w,
        }
    })
}
