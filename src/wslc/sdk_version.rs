//! Host / NuGet version matching (no Windows-only deps).
//!
//! `wsl.exe --version` writes UTF-16LE when stdout is a pipe (no console).
//! Reading those bytes as UTF-8/Latin-1 leaves NUL gaps (`W\0S\0L`) or, once
//! NULs are shown as spaces, `W S L   Hr,g:   3 . 0 . 1 . 0` — `版本` is the
//! UTF-16LE code units `48 72 2C 67`. Either form hides `3.0.1.0` from a
//! naive `x.y.z` scan, so a new enough WSL looks missing.

/// Oldest WSL release that ships WSL Containers.
pub const MIN_WSL_VERSION: &str = "2.9.3";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemVer {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub revision: u32,
}

impl SemVer {
    pub fn parse(s: &str) -> Option<Self> {
        let mut parts = s.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next().unwrap_or("0").parse().ok()?;
        let revision = match parts.next() {
            Some(part) => part.parse().ok()?,
            None => 0,
        };
        Some(Self {
            major,
            minor,
            patch,
            revision,
        })
    }
}

/// `true` when `version` is `2.9.3` or newer. `3.0.1.0` qualifies; the fourth
/// component is the Windows revision and missing components count as zero.
pub fn meets_min_wsl_version(version: &str) -> bool {
    match (SemVer::parse(version), SemVer::parse(MIN_WSL_VERSION)) {
        (Some(version), Some(min)) => version >= min,
        _ => false,
    }
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

/// Read a WSL / `wslc` version from raw process pipes.
pub fn wsl_version_from_output(stdout: &[u8], stderr: &[u8]) -> Option<String> {
    let out = decode_command_output(stdout);
    let err = decode_command_output(stderr);
    let combined = match (out.trim().is_empty(), err.trim().is_empty()) {
        (true, _) => err,
        (_, true) => out,
        _ => format!("{out}\n{err}"),
    };
    extract_wsl_version(&combined)
}

/// Pull the first `x.y.z` (dropping a fourth component) from command output.
pub fn extract_version(text: &str) -> Option<String> {
    let full = extract_wsl_version(text)?;
    let semver = SemVer::parse(&full)?;
    Some(format!(
        "{}.{}.{}",
        semver.major, semver.minor, semver.patch
    ))
}

/// WSL product version from `wsl --version` / `wslc version` text.
///
/// Prefers a `WSL version` / `WSL 版本` line over kernel, WSLg, and Windows
/// lines. Also accepts NUL-separated UTF-16 misreads and the spaced form
/// those NULs become when displayed.
pub fn extract_wsl_version(text: &str) -> Option<String> {
    let stripped: String = text.chars().filter(|c| *c != '\0').collect();
    if let Some(version) = extract_wsl_version_raw(&stripped) {
        return Some(version);
    }
    if looks_like_spaced_utf16(&stripped) {
        let collapsed = collapse_ascii_gaps(&stripped);
        if collapsed != stripped {
            return extract_wsl_version_raw(&collapsed);
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

fn extract_wsl_version_raw(text: &str) -> Option<String> {
    let lines = non_empty_lines(text);

    for line in &lines {
        if is_explicit_wsl_version_line(line)
            && let Some(version) = first_version_token(line)
        {
            return Some(version);
        }
    }

    // UTF-16 read as Latin-1 turns `版本` into `Hr,g`, so the label is gone
    // but the line still starts with WSL. Skip kernel / WSLg / Windows lines.
    for line in &lines {
        if mentions_wsl_product(line)
            && !is_non_wsl_component_line(line)
            && let Some(version) = first_version_token(line)
        {
            return Some(version);
        }
    }

    bare_tool_version(&lines)
}

fn non_empty_lines(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect()
}

fn is_explicit_wsl_version_line(line: &str) -> bool {
    if !mentions_wsl_product(line) || is_non_wsl_component_line(line) {
        return false;
    }
    let lower = line.to_ascii_lowercase();
    lower.contains("version") || line.contains("版本")
}

fn mentions_wsl_product(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    if lower.contains("wslg") {
        return false;
    }
    lower.contains("wsl")
}

fn is_non_wsl_component_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("wslg")
        || lower.contains("kernel")
        || lower.contains("msrdc")
        || lower.contains("direct3d")
        || lower.contains("dxcore")
        || lower.contains("windows")
        || line.contains("内核")
}

/// `wslc version` often prints a bare `x.y.z` or `wslc x.y.z` with no banner.
fn bare_tool_version(lines: &[&str]) -> Option<String> {
    if lines.len() != 1 {
        return None;
    }
    let line = lines[0];
    if is_non_wsl_component_line(line) {
        return None;
    }
    let token = first_version_token(line)?;
    let lower = line.to_ascii_lowercase();
    if line.trim() == token || lower.contains("wslc") || mentions_wsl_product(line) {
        Some(token)
    } else {
        None
    }
}

fn first_version_token(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        let mut dots = 0;
        let mut prev_digit = true;
        i += 1;
        while i < bytes.len() {
            if bytes[i].is_ascii_digit() {
                prev_digit = true;
                i += 1;
            } else if bytes[i] == b'.' && prev_digit {
                dots += 1;
                prev_digit = false;
                i += 1;
            } else {
                break;
            }
        }
        if !prev_digit && i > start {
            i -= 1;
        }
        if dots >= 2 && prev_digit {
            return Some(truncate_components(&text[start..i], 4));
        }
    }
    None
}

fn truncate_components(token: &str, max_components: usize) -> String {
    let mut dots = 0;
    for (idx, ch) in token.char_indices() {
        if ch == '.' {
            dots += 1;
            if dots == max_components {
                return token[..idx].to_string();
            }
        }
    }
    token.to_string()
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

    const ENGLISH_VERSION: &str = "\
WSL version: 2.9.3.0
Kernel version: 6.6.87.1-1
WSLg version: 1.0.66
MSRDC version: 1.2.6074
Direct3D version: 1.611.1-81528511
DXCore version: 10.0.26100.1-240331-1435.ge-release
Windows version: 10.0.22621.2715
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
    fn utf16_le_chinese_wsl_version_meets_minimum() {
        let bytes = encode_utf16_le(CHINESE_VERSION);
        let version = wsl_version_from_output(&bytes, b"").expect("version");
        assert_eq!(version, "3.0.1.0");
        assert!(meets_min_wsl_version(&version));
        assert_eq!(
            decode_command_output(&bytes).lines().next(),
            Some("WSL 版本: 3.0.1.0")
        );
    }

    #[test]
    fn utf16_le_bom_and_stderr_only() {
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(encode_utf16_le(
            "WSL version: 3.0.1.0\r\nKernel version: 6.18.40.1-1\r\n",
        ));
        assert_eq!(
            wsl_version_from_output(b"", &bytes).as_deref(),
            Some("3.0.1.0")
        );
    }

    #[test]
    fn english_utf8_boundary_and_kernel_is_not_wsl() {
        assert_eq!(
            extract_wsl_version(ENGLISH_VERSION).as_deref(),
            Some("2.9.3.0")
        );
        assert!(meets_min_wsl_version("2.9.3.0"));
        assert!(meets_min_wsl_version("2.9.3"));
        assert!(!meets_min_wsl_version("2.9.2.9"));
        assert!(!meets_min_wsl_version("1.2.5"));
        assert_eq!(
            extract_wsl_version("Kernel version: 6.18.40.1-1\nWSL version: 3.0.1.0\n").as_deref(),
            Some("3.0.1.0")
        );
        assert_eq!(extract_wsl_version("内核版本: 6.18.40.1-1\n"), None);
    }

    #[test]
    fn nul_separated_utf16_misread_and_spaced_regression() {
        let bytes = encode_utf16_le(CHINESE_VERSION);
        let lossy = String::from_utf8_lossy(&bytes);
        assert!(
            lossy.contains('\0'),
            "UTF-16LE read as UTF-8 must keep NUL gaps"
        );
        assert_eq!(extract_wsl_version(&lossy).as_deref(), Some("3.0.1.0"));

        // Log viewers (and a Latin-1 decode) show those NULs as spaces:
        // `W S L   Hr,g:   3 . 0 . 1 . 0`
        let spaced = lossy.replace('\0', " ");
        assert!(
            spaced.contains("Hr,g"),
            "版本 misread as Latin-1 should surface as Hr,g, got {spaced:?}"
        );
        assert!(spaced.contains("3 . 0 . 1 . 0"), "got {spaced:?}");
        assert_eq!(extract_wsl_version(&spaced).as_deref(), Some("3.0.1.0"));
        assert!(meets_min_wsl_version(
            extract_wsl_version(&spaced).as_deref().unwrap()
        ));
    }

    #[test]
    fn nul_strip_fallback_when_utf16_length_is_odd() {
        let mut bytes = encode_utf16_le("WSL version: 3.0.1.0\n");
        bytes.push(b'A');
        assert_eq!(bytes.len() % 2, 1);
        assert!(super::decode_utf16(&bytes).is_none());
        let text = decode_command_output(&bytes);
        assert_eq!(extract_wsl_version(&text).as_deref(), Some("3.0.1.0"));
    }

    #[test]
    fn rejects_wslg_line_and_accepts_wslc_banner() {
        let text = "\
WSLg version: 1.0.66
Kernel version: 6.18.40.1-1
WSL version: 3.0.1.0
";
        assert_eq!(extract_wsl_version(text).as_deref(), Some("3.0.1.0"));
        assert_eq!(
            extract_wsl_version("wslc 3.0.1\n").as_deref(),
            Some("3.0.1")
        );
        assert_eq!(extract_version("3.0.1.0\n").as_deref(), Some("3.0.1"));
    }
}
