use crate::wslc::{PortMapping, VolumeMapping};

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

pub(crate) fn parse_bind_mounts(raw: &str) -> Result<Vec<VolumeMapping>, String> {
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
            out.push(VolumeMapping {
                host,
                container,
                read_only,
            });
        }
        Ok(out)
    }
