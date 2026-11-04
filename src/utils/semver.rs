//! [Shared] Semantic versions: agent mode resolves npm ranges against them,
//! `wasmrun update` compares published releases with them.

use std::cmp::Ordering;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemVer {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub pre: Option<String>,
}

impl SemVer {
    pub fn parse(s: &str) -> std::result::Result<Self, String> {
        let s = s.trim().trim_start_matches('v');
        let (core, pre) = match s.split_once('-') {
            Some((c, p)) => (c, Some(p.split('+').next().unwrap_or(p).to_string())),
            None => (s.split('+').next().unwrap_or(s), None),
        };
        let mut parts = core.split('.');
        let mut next_num = |what: &str| {
            parts
                .next()
                .ok_or(format!("missing {what}"))?
                .parse::<u64>()
                .map_err(|_| format!("invalid {what}"))
        };
        let major = next_num("major")?;
        let minor = next_num("minor")?;
        let patch = next_num("patch")?;
        if parts.next().is_some() {
            return Err("too many version components".into());
        }
        Ok(Self {
            major,
            minor,
            patch,
            pre,
        })
    }

    pub fn exact(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
            pre: None,
        }
    }

    pub fn core(&self) -> (u64, u64, u64) {
        (self.major, self.minor, self.patch)
    }
}

impl fmt::Display for SemVer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        match &self.pre {
            Some(pre) => write!(f, "-{pre}"),
            None => Ok(()),
        }
    }
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> Ordering {
        self.core()
            .cmp(&other.core())
            .then_with(|| match (&self.pre, &other.pre) {
                (None, None) => Ordering::Equal,
                // A prerelease sorts below its release.
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (Some(a), Some(b)) => cmp_prerelease(a, b),
            })
    }
}

/// Semver prerelease ordering: numeric identifiers compare numerically and
/// sort below alphanumeric ones. String comparison gets `alpha.10` vs
/// `alpha.2` backwards.
fn cmp_prerelease(a: &str, b: &str) -> Ordering {
    let mut ai = a.split('.');
    let mut bi = b.split('.');
    loop {
        match (ai.next(), bi.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => {
                let ord = match (x.parse::<u64>(), y.parse::<u64>()) {
                    (Ok(nx), Ok(ny)) => nx.cmp(&ny),
                    (Ok(_), Err(_)) => Ordering::Less,
                    (Err(_), Ok(_)) => Ordering::Greater,
                    (Err(_), Err(_)) => x.cmp(y),
                };
                if ord != Ordering::Equal {
                    return ord;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_round_trips() {
        for version in ["1.2.3", "0.0.1", "2.0.0-rc.1"] {
            assert_eq!(SemVer::parse(version).unwrap().to_string(), version);
        }
        assert_eq!(SemVer::parse("v1.2.3+build").unwrap().to_string(), "1.2.3");
    }
}
