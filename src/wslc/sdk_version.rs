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

impl std::fmt::Display for SemVer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// First-launch gate. Older hosts cannot run the WSL Containers SDK this app targets.
pub const MIN_WSL_VERSION: SemVer = SemVer {
    major: 2,
    minor: 9,
    patch: 3,
};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum WslVersionVerdict {
    Ok(String),
    Missing,
    TooOld { found: String },
}

/// Classify `wsl --version` output.
///
/// Prefers a line labeled `WSL` (English or `WSL 版本`) and ignores `WSLg`.
/// A single bare version token is accepted. Multiple unlabeled versions are
/// treated as unreadable so a kernel version cannot pass the check by accident.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn judge_wsl_version(output: Option<&str>) -> WslVersionVerdict {
    let Some(text) = output.map(str::trim).filter(|s| !s.is_empty()) else {
        return WslVersionVerdict::Missing;
    };
    let version = labeled_wsl_version(text).or_else(|| sole_version(text));
    match version {
        Some(v) => match SemVer::parse(&v) {
            Some(sv) if sv >= MIN_WSL_VERSION => WslVersionVerdict::Ok(v),
            Some(_) => WslVersionVerdict::TooOld { found: v },
            None => WslVersionVerdict::Missing,
        },
        None => WslVersionVerdict::Missing,
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
fn labeled_wsl_version(text: &str) -> Option<String> {
    let line = text.lines().find(|line| {
        let head = line
            .split(':')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        head.starts_with("wsl") && !head.starts_with("wslg")
    })?;
    extract_version(line)
}

#[cfg_attr(not(windows), allow(dead_code))]
fn sole_version(text: &str) -> Option<String> {
    let mut found = None;
    for token in text.split(|c: char| !(c.is_ascii_digit() || c == '.')) {
        if token.is_empty() {
            continue;
        }
        let Some(version) = extract_version(token) else {
            continue;
        };
        if found.is_some() {
            return None;
        }
        found = Some(version);
    }
    found
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

    #[test]
    fn judges_wsl_version_output() {
        assert_eq!(judge_wsl_version(None), WslVersionVerdict::Missing);
        assert_eq!(judge_wsl_version(Some("  ")), WslVersionVerdict::Missing);
        assert_eq!(
            judge_wsl_version(Some("2.9.3")),
            WslVersionVerdict::Ok("2.9.3".into())
        );
        assert_eq!(
            judge_wsl_version(Some("2.10.0")),
            WslVersionVerdict::Ok("2.10.0".into())
        );
        assert_eq!(
            judge_wsl_version(Some("2.9.2")),
            WslVersionVerdict::TooOld {
                found: "2.9.2".into()
            }
        );

        let english = "\
WSL version: 2.4.13.0
Kernel version: 5.15.167.4-1
WSLg version: 1.0.65
";
        assert_eq!(
            judge_wsl_version(Some(english)),
            WslVersionVerdict::TooOld {
                found: "2.4.13".into()
            }
        );

        let chinese = "WSL 版本: 2.9.3.0\n内核版本: 5.15.167.4-1\n";
        assert_eq!(
            judge_wsl_version(Some(chinese)),
            WslVersionVerdict::Ok("2.9.3".into())
        );

        let unlabeled = "Kernel version: 5.15.167.4-1\nWSLg version: 1.0.65\n";
        assert_eq!(
            judge_wsl_version(Some(unlabeled)),
            WslVersionVerdict::Missing
        );
        assert_eq!(format!("{}", MIN_WSL_VERSION), "2.9.3");
    }
}
