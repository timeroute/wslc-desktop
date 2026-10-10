//! Host / NuGet version matching (no Windows-only deps).

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemVer {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl SemVer {
    pub fn parse(s: &str) -> Option<Self> {
        let mut parts = s.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next().unwrap_or("0").parse().ok()?;
        Some(Self {
            major,
            minor,
            patch,
        })
    }
}

/// Pull the first `x.y.z` / `x.y.z.w` from command output.
pub fn extract_version(text: &str) -> Option<String> {
    for token in text.split(|c: char| !(c.is_ascii_digit() || c == '.')) {
        if token.is_empty() {
            continue;
        }
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() >= 3
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
        {
            return Some(format!("{}.{}.{}", parts[0], parts[1], parts[2]));
        }
    }
    None
}

/// Prefer exact host match, else newest package with the same major as the
/// host, else the newest published package (SDK must not be older than WSL).
pub fn pick_nuget_version(host: &str, available: &[String]) -> Option<String> {
    let mut parsed: Vec<(SemVer, &String)> = available
        .iter()
        .filter_map(|v| SemVer::parse(v).map(|sv| (sv, v)))
        .collect();
    if parsed.is_empty() {
        return available.last().cloned();
    }
    parsed.sort_by(|a, b| a.0.cmp(&b.0));

    if let Some(h) = SemVer::parse(host) {
        if let Some((_, v)) = parsed.iter().find(|(sv, _)| *sv == h) {
            return Some((*v).clone());
        }
        if let Some((_, v)) = parsed.iter().rev().find(|(sv, _)| sv.major == h.major) {
            return Some((*v).clone());
        }
    }
    parsed.last().map(|(_, v)| (*v).clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_version_tokens() {
        assert_eq!(
            extract_version("WSL version: 2.9.3.0\n"),
            Some("2.9.3".into())
        );
        assert_eq!(extract_version("wslc 3.0.1\n"), Some("3.0.1".into()));
    }

    #[test]
    fn picks_exact_same_major_or_latest() {
        let available = vec!["2.9.3".into(), "2.9.9".into(), "3.0.1".into()];
        assert_eq!(
            pick_nuget_version("2.9.9", &available).as_deref(),
            Some("2.9.9")
        );
        assert_eq!(
            pick_nuget_version("2.9.5", &available).as_deref(),
            Some("2.9.9")
        );
        assert_eq!(
            pick_nuget_version("3.0.0", &available).as_deref(),
            Some("3.0.1")
        );
        assert_eq!(
            pick_nuget_version("4.0.0", &available).as_deref(),
            Some("3.0.1")
        );
        assert_eq!(
            pick_nuget_version("unknown", &available).as_deref(),
            Some("3.0.1")
        );
    }
}
