use crate::wslc::{PortMapping, VolumeInfo, VolumeMapping};

pub(crate) fn parse_port_mappings(raw: &str) -> Result<Vec<PortMapping>, String> {
    let mut out = Vec::new();
    for part in raw.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let Some((host_s, container_s)) = part.split_once(':') else {
            return Err(format!("expected host:container, got `{part}`"));
        };
        let host = host_s
            .trim()
            .parse::<u16>()
            .map_err(|_| format!("invalid host port in `{part}`"))?;
        let container = container_s
            .trim()
            .parse::<u16>()
            .map_err(|_| format!("invalid container port in `{part}`"))?;
        out.push(PortMapping { host, container });
    }
    Ok(out)
}

/// Bind mounts: `C:\data:/data[:ro]`
pub(crate) fn parse_bind_mounts(raw: &str) -> Result<Vec<VolumeMapping>, String> {
    parse_volume_specs(raw, false)
}

/// Named session volumes: `data:/data[:ro]`
pub(crate) fn parse_named_volumes(raw: &str) -> Result<Vec<VolumeMapping>, String> {
    parse_volume_specs(raw, true)
}

fn parse_volume_specs(raw: &str, named: bool) -> Result<Vec<VolumeMapping>, String> {
    let mut out = Vec::new();
    for part in raw.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let lower = part.to_ascii_lowercase();
        let read_only = lower.ends_with(":ro");
        let core = if read_only {
            part[..part.len() - 3].trim_end()
        } else {
            part
        };
        let Some(idx) = core.find(":/") else {
            return Err(format!("expected host:/container path, got `{part}`"));
        };
        let host = core[..idx].trim().to_string();
        let container = core[idx + 1..].trim().to_string();
        if host.is_empty() || container.is_empty() {
            return Err(format!("empty host or mount path in `{part}`"));
        }
        if named {
            if host.contains('\\') || host.contains('/') || looks_like_windows_drive(&host) {
                return Err(format!(
                    "named volume expected a simple name, got `{host}`"
                ));
            }
        }
        out.push(VolumeMapping {
            host,
            container,
            read_only,
            named,
        });
    }
    Ok(out)
}

fn looks_like_windows_drive(host: &str) -> bool {
    let bytes = host.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// Session VHD volume names referenced by a container's mounts label.
///
/// Only names that exist in `known` are returned, so bind mounts and inspect
/// host paths are never treated as deletable named volumes.
pub(crate) fn named_volumes_attached(volumes_label: &str, known: &[VolumeInfo]) -> Vec<String> {
    if volumes_label.trim().is_empty() || known.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for part in volumes_label.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let part = part.strip_prefix("vol:").unwrap_or(part);
        let lower = part.to_ascii_lowercase();
        let core = if lower.ends_with(":ro") {
            part[..part.len() - 3].trim_end()
        } else {
            part
        };
        let host = if let Some(idx) = core.find(":/") {
            core[..idx].trim()
        } else {
            core.trim()
        };
        if host.is_empty()
            || host.contains('\\')
            || host.contains('/')
            || looks_like_windows_drive(host)
        {
            continue;
        }
        if known.iter().any(|v| v.name == host) && !out.iter().any(|n| n == host) {
            out.push(host.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_named_and_bind() {
        let named = parse_named_volumes("data:/data, cache:/var/cache:ro").unwrap();
        assert_eq!(named.len(), 2);
        assert!(named[0].named);
        assert!(!named[0].read_only);
        assert!(named[1].read_only);

        let bind = parse_bind_mounts(r"C:\data:/data:ro").unwrap();
        assert_eq!(bind.len(), 1);
        assert!(!bind[0].named);
        assert!(bind[0].read_only);
    }

    #[test]
    fn extracts_only_known_named_volumes() {
        let known = vec![
            VolumeInfo {
                name: "data".into(),
                driver: "vhd".into(),
                mountpoint: String::new(),
                created: String::new(),
                size: "1 GiB".into(),
            },
            VolumeInfo {
                name: "db".into(),
                driver: "vhd".into(),
                mountpoint: String::new(),
                created: String::new(),
                size: "2 GiB".into(),
            },
        ];
        let names = named_volumes_attached(
            r"vol:data:/data, C:\logs:/var/log, vol:db:/var/lib/mysql:ro, other:/tmp",
            &known,
        );
        assert_eq!(names, vec!["data".to_string(), "db".to_string()]);
    }
}
