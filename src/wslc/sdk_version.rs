//! Host / NuGet version matching (no Windows-only deps).
//!
//! `wsl.exe --version` writes UTF-16LE when stdout is a pipe (no console).
//! Reading those bytes as UTF-8 leaves NUL gaps (`W\0S\0L`) or, once NULs are
//! shown as spaces, `W S L   Hr,g:   3 . 0 . 1 . 0`. `版本` is the UTF-16LE
//! code units `48 72 2C 67`. Either form hides `3.0.1.0` from a naive scan.

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

/// Decode a captured stdout/stderr buffer.
///
/// Order: UTF-16 BOM, then a UTF-16LE NUL pattern, then strip NULs that sit
/// between ASCII bytes, then lossy UTF-8.
pub fn decode_command_output(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    if let Some(text) = decode_utf16(bytes)
        && plausible_text(&text)
    {
        return text;
    }
    if let Some(text) = strip_nuls_between_ascii(bytes) {
        return text;
    }
    String::from_utf8_lossy(bytes).into_owned()
}

/// Classify `wsl --version` output.
///
/// Prefers a line labeled `WSL` (English or `WSL 版本`) and ignores `WSLg`.
/// A single bare version token is accepted. Multiple unlabeled versions are
/// treated as unreadable so a kernel version cannot pass the check by accident.
/// NUL-separated and spaced UTF-16 misreads are repaired before that scan.
/// `3.0.1.0` satisfies `>= 2.9.3` (the fourth component is ignored).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn judge_wsl_version(output: Option<&str>) -> WslVersionVerdict {
    let Some(text) = output.map(str::trim).filter(|s| !s.is_empty()) else {
        return WslVersionVerdict::Missing;
    };
    let normalized = normalize_command_text(text);
    let version = labeled_wsl_version(&normalized).or_else(|| sole_version(&normalized));
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

/// Pull the first `x.y.z` / `x.y.z.w` from command output (fourth component dropped).
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

/// Whether a cached NuGet SDK can serve this host without re-downloading.
///
/// Host probe strings can drift (`wslc` vs `wsl --version`, or `unknown` on a
/// failed probe). Do not require an exact stamp match — only re-download when
/// the cached package is clearly too old (major behind the host), or when the
/// user forces Retry.
pub fn cache_compatible(host: &str, nuget_version: &str) -> bool {
    let Some(host_ver) = SemVer::parse(host) else {
        return true;
    };
    let Some(nuget_ver) = SemVer::parse(nuget_version) else {
        return false;
    };
    nuget_ver.major >= host_ver.major
}

fn normalize_command_text(text: &str) -> String {
    let stripped: String = text.chars().filter(|c| *c != '\0').collect();
    if looks_like_spaced_utf16(&stripped) {
        let collapsed = collapse_ascii_gaps(&stripped);
        if collapsed != stripped {
            return collapsed;
        }
    }
    stripped
}

fn decode_utf16(bytes: &[u8]) -> Option<String> {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return Some(utf16_units_to_string(&bytes[2..], true));
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return Some(utf16_units_to_string(&bytes[2..], false));
    }
    if looks_like_utf16_le(bytes) {
        return Some(utf16_units_to_string(bytes, true));
    }
    None
}

fn utf16_units_to_string(bytes: &[u8], little_endian: bool) -> String {
    let units: Vec<u16> = bytes
        .chunks(2)
        .map(|chunk| {
            if chunk.len() == 2 {
                if little_endian {
                    u16::from_le_bytes([chunk[0], chunk[1]])
                } else {
                    u16::from_be_bytes([chunk[0], chunk[1]])
                }
            } else if little_endian {
                chunk[0] as u16
            } else {
                (chunk[0] as u16) << 8
            }
        })
        .collect();
    String::from_utf16_lossy(&units)
        .trim_start_matches('\u{feff}')
        .to_string()
}

/// ASCII-heavy UTF-16LE: the high byte of most units is NUL.
fn looks_like_utf16_le(bytes: &[u8]) -> bool {
    if bytes.len() < 8 || !bytes.len().is_multiple_of(2) {
        return false;
    }
    let mut odd_zeros = 0usize;
    let mut even_zeros = 0usize;
    for pair in bytes.as_chunks::<2>().0 {
        if pair[0] == 0 {
            even_zeros += 1;
        }
        if pair[1] == 0 {
            odd_zeros += 1;
        }
    }
    let pairs = bytes.len() / 2;
    odd_zeros * 2 > pairs && odd_zeros > even_zeros.saturating_mul(4)
}

fn plausible_text(text: &str) -> bool {
    let total = text.chars().count();
    if total == 0 {
        return false;
    }
    let printable = text
        .chars()
        .filter(|c| !c.is_control() || matches!(*c, '\n' | '\r' | '\t'))
        .count();
    printable * 4 >= total * 3
}

/// Fallback when the buffer is not even-length UTF-16 but still has
/// `ASCII, NUL` pairs (a truncated or mixed capture).
fn strip_nuls_between_ascii(bytes: &[u8]) -> Option<String> {
    let mut pairs = 0usize;
    let mut i = 0usize;
    while i + 1 < bytes.len() {
        if bytes[i] != 0 && bytes[i].is_ascii() && bytes[i + 1] == 0 {
            pairs += 1;
            i += 2;
        } else {
            i += 1;
        }
    }
    if pairs < 3 {
        return None;
    }
    let stripped: Vec<u8> = bytes.iter().copied().filter(|b| *b != 0).collect();
    let text = String::from_utf8_lossy(&stripped).into_owned();
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

fn looks_like_spaced_utf16(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let mut hits = 0usize;
    for window in chars.windows(3) {
        if window[1] == ' ' && is_gap_glue(window[0]) && is_gap_glue(window[2]) {
            hits += 1;
        }
    }
    hits >= 3
}

fn is_gap_glue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '.'
}

/// Join ASCII that was split by the NUL bytes of UTF-16LE (`3 . 0 . 1` → `3.0.1`).
/// Newlines stay put so a trailing digit cannot merge into the next line.
fn collapse_ascii_gaps(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\n' || c == '\r' {
            out.push(c);
            continue;
        }
        if c.is_whitespace() {
            let prev = match out.chars().last() {
                Some('\n' | '\r') | None => None,
                other => other,
            };
            let next = chars.peek().copied();
            if let (Some(prev), Some(next)) = (prev, next)
                && next != '\n'
                && next != '\r'
                && is_gap_glue(prev)
                && is_gap_glue(next)
            {
                continue;
            }
            if !matches!(out.chars().last(), Some(' ' | '\n' | '\r') | None) {
                out.push(' ');
            }
            continue;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_utf16_le(text: &str) -> Vec<u8> {
        text.encode_utf16()
            .flat_map(|unit| unit.to_le_bytes())
            .collect()
    }

    const CHINESE_VERSION: &str = "\
WSL 版本: 3.0.1.0
内核版本: 6.18.40.1-1
WSLg 版本: 1.0.66
MSRDC 版本: 1.2.6074
Direct3D 版本: 1.611.1-81528511
DXCore 版本: 10.0.26100.1-240331-1435.ge-release
Windows 版本: 10.0.26100.3476
";

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
    fn cache_compatible_ignores_host_string_drift() {
        assert!(cache_compatible("unknown", "3.0.1"));
        assert!(cache_compatible("2.9.3", "2.9.9"));
        assert!(cache_compatible("2.9.5", "3.0.1"));
        assert!(cache_compatible("3.0.1", "3.0.1"));
        assert!(!cache_compatible("3.0.1", "2.9.9"));
        assert!(!cache_compatible("3.0.0", "not-a-version"));
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

    #[test]
    fn utf16_le_chinese_3_0_1_0_meets_minimum() {
        let bytes = encode_utf16_le(CHINESE_VERSION);
        let text = decode_command_output(&bytes);
        assert_eq!(text.lines().next(), Some("WSL 版本: 3.0.1.0"));
        assert_eq!(
            judge_wsl_version(Some(&text)),
            WslVersionVerdict::Ok("3.0.1".into())
        );
        assert!(SemVer::parse("3.0.1.0").unwrap() >= MIN_WSL_VERSION);

        let mut bom = vec![0xFF, 0xFE];
        bom.extend(encode_utf16_le(
            "WSL version: 3.0.1.0\r\nKernel version: 6.18.40.1-1\r\n",
        ));
        let from_stderr = decode_command_output(&bom);
        assert_eq!(
            judge_wsl_version(Some(&from_stderr)),
            WslVersionVerdict::Ok("3.0.1".into())
        );
    }

    #[test]
    fn nul_and_spaced_misread_still_accepts_3_0_1_0() {
        let bytes = encode_utf16_le(CHINESE_VERSION);
        let lossy = String::from_utf8_lossy(&bytes);
        assert!(lossy.contains('\0'));
        assert_eq!(
            judge_wsl_version(Some(&lossy)),
            WslVersionVerdict::Ok("3.0.1".into())
        );

        let spaced = lossy.replace('\0', " ");
        assert!(
            spaced.contains("Hr,g"),
            "版本 misread as Latin-1 should surface as Hr,g, got {spaced:?}"
        );
        assert!(spaced.contains("3 . 0 . 1 . 0"), "got {spaced:?}");
        assert_eq!(
            judge_wsl_version(Some(&spaced)),
            WslVersionVerdict::Ok("3.0.1".into())
        );
    }

    #[test]
    fn nul_strip_fallback_when_utf16_length_is_odd() {
        let mut bytes = encode_utf16_le("WSL version: 3.0.1.0\n");
        bytes.push(b'A');
        assert_eq!(bytes.len() % 2, 1);
        assert!(super::decode_utf16(&bytes).is_none());
        let text = decode_command_output(&bytes);
        assert_eq!(
            judge_wsl_version(Some(&text)),
            WslVersionVerdict::Ok("3.0.1".into())
        );
    }
}
