//! SemVer helpers for the WSL package gate and NuGet SDK matching.

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemVer {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl SemVer {
    /// Parse `x.y`, `x.y.z`, or `x.y.z.w` (components after patch ignored).
    pub fn parse(s: &str) -> Option<Self> {
        let mut parts = s.trim().split('.');
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

/// Classify a registry package version string (`3.0.1` / `3.0.1.0`).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn judge_package_version(version: Option<&str>) -> WslVersionVerdict {
    let Some(raw) = version.map(str::trim).filter(|s| !s.is_empty()) else {
        return WslVersionVerdict::Missing;
    };
    let Some(sv) = SemVer::parse(raw) else {
        return WslVersionVerdict::Missing;
    };
    let normalized = sv.to_string();
    if sv >= MIN_WSL_VERSION {
        WslVersionVerdict::Ok(normalized)
    } else {
        WslVersionVerdict::TooOld {
            found: normalized,
        }
    }
}

/// Prefer exact host match, else newest package with the same major as the
/// host, else the newest published package.
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

/// Whether a cached NuGet SDK can serve this host without re-downloading.
///
/// Re-download only when the cached package major is behind the host, or when
/// the user forces Retry.
pub fn cache_compatible(host: &str, nuget_version: &str) -> bool {
    let Some(host_ver) = SemVer::parse(host) else {
        return true;
    };
    let Some(nuget_ver) = SemVer::parse(nuget_version) else {
        return false;
    };
    nuget_ver.major >= host_ver.major
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_registry_style_versions() {
        assert_eq!(SemVer::parse("2.9.3"), Some(SemVer { major: 2, minor: 9, patch: 3 }));
        assert_eq!(SemVer::parse("3.0.1.0"), Some(SemVer { major: 3, minor: 0, patch: 1 }));
        assert_eq!(SemVer::parse(" 3.0.1 "), Some(SemVer { major: 3, minor: 0, patch: 1 }));
        assert!(SemVer::parse("not-a-version").is_none());
        assert!(SemVer::parse("").is_none());
    }

    #[test]
    fn judges_package_version() {
        assert_eq!(judge_package_version(None), WslVersionVerdict::Missing);
        assert_eq!(judge_package_version(Some("  ")), WslVersionVerdict::Missing);
        assert_eq!(
            judge_package_version(Some("2.9.3")),
            WslVersionVerdict::Ok("2.9.3".into())
        );
        assert_eq!(
            judge_package_version(Some("3.0.1.0")),
            WslVersionVerdict::Ok("3.0.1".into())
        );
        assert_eq!(
            judge_package_version(Some("2.9.2")),
            WslVersionVerdict::TooOld {
                found: "2.9.2".into()
            }
        );
        assert!(SemVer::parse("3.0.1.0").unwrap() >= MIN_WSL_VERSION);
        assert_eq!(format!("{MIN_WSL_VERSION}"), "2.9.3");
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
            pick_nuget_version("3.0.1.0", &available).as_deref(),
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
    fn cache_compatible_by_major() {
        assert!(cache_compatible("unknown", "3.0.1"));
        assert!(cache_compatible("2.9.3", "2.9.9"));
        assert!(cache_compatible("2.9.5", "3.0.1"));
        assert!(cache_compatible("3.0.1", "3.0.1"));
        assert!(!cache_compatible("3.0.1", "2.9.9"));
        assert!(!cache_compatible("3.0.0", "not-a-version"));
    }
}
