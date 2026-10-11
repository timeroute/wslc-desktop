//! Resolve the installed WSL package version before `wslcsdk.dll` is available.
//!
//! Order: machine registry (same source WSL uses) → `wsl --version` → `wslc version`.
//! After the SDK loads, callers should use `wslc::Service::version()` instead.

#![cfg(windows)]

use std::process::Command;

use anyhow::{anyhow, Context as _, Result};

use super::sdk_version::{
    decode_command_output, extract_version, judge_wsl_version, WslVersionVerdict,
};

/// How the host WSL package version was obtained.
#[derive(Debug, Clone)]
pub enum WslProbe {
    /// `HKLM\...\Lxss\Msi\Version`
    Registry { version: String },
    /// Raw `wsl --version` text (may be empty if the command failed).
    Cli { raw: String },
}

impl WslProbe {
    pub fn verdict(&self) -> WslVersionVerdict {
        match self {
            Self::Registry { version } => judge_wsl_version(Some(version)),
            Self::Cli { raw } => {
                let text = raw.trim();
                judge_wsl_version(if text.is_empty() { None } else { Some(text) })
            }
        }
    }
}

/// Probe for the first-launch WSL gate (registry, else CLI).
pub fn probe_wsl() -> WslProbe {
    if let Some(version) = registry_version() {
        return WslProbe::Registry { version };
    }
    WslProbe::Cli {
        raw: cli_wsl_version_output().unwrap_or_default(),
    }
}

/// Best-effort host version string for NuGet package selection.
pub fn host_version() -> Result<String> {
    match probe_wsl() {
        WslProbe::Registry { version } => Ok(version),
        WslProbe::Cli { raw } => match judge_wsl_version(Some(raw.trim()).filter(|s| !s.is_empty()))
        {
            WslVersionVerdict::Ok(v) | WslVersionVerdict::TooOld { found: v } => Ok(v),
            WslVersionVerdict::Missing => {
                if let Some(v) = extract_version(&raw) {
                    return Ok(v);
                }
                wslc_cli_version().ok_or_else(|| {
                    anyhow!(
                        "could not read WSL package version from registry or CLI (is WSL installed?)"
                    )
                })
            }
        },
    }
}

fn registry_version() -> Option<String> {
    for subkey in [
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Lxss\Msi",
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Lxss\MSI",
    ] {
        if let Some(raw) = read_hk_lm_string(subkey, "Version")
            && let Some(v) = extract_version(&raw)
        {
            return Some(v);
        }
    }
    None
}

fn cli_wsl_version_output() -> Option<String> {
    for args in [
        ["wsl", "--version"].as_slice(),
        ["wsl.exe", "--version"].as_slice(),
    ] {
        if let Ok(out) = run_hidden(args)
            && !out.trim().is_empty()
        {
            return Some(out);
        }
    }
    None
}

fn wslc_cli_version() -> Option<String> {
    for args in [
        ["wslc", "version"].as_slice(),
        ["wslc.exe", "version"].as_slice(),
    ] {
        if let Ok(out) = run_hidden(args)
            && let Some(v) = extract_version(&out)
        {
            return Some(v);
        }
    }
    None
}

fn read_hk_lm_string(subkey: &str, value_name: &str) -> Option<String> {
    use windows_sys::Win32::Foundation::{ERROR_MORE_DATA, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_LOCAL_MACHINE, KEY_READ, REG_SZ,
    };

    let subkey_w = wide(subkey);
    let value_w = wide(value_name);
    unsafe {
        let mut hkey = std::ptr::null_mut();
        if RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            subkey_w.as_ptr(),
            0,
            KEY_READ,
            &mut hkey,
        ) != ERROR_SUCCESS
        {
            return None;
        }

        let mut kind = 0u32;
        let mut byte_len = 0u32;
        let query = RegQueryValueExW(
            hkey,
            value_w.as_ptr(),
            std::ptr::null(),
            &mut kind,
            std::ptr::null_mut(),
            &mut byte_len,
        );
        if (query != ERROR_SUCCESS && query != ERROR_MORE_DATA) || kind != REG_SZ || byte_len < 2 {
            let _ = RegCloseKey(hkey);
            return None;
        }

        let mut buf = vec![0u16; (byte_len as usize).div_ceil(2)];
        let mut buf_bytes = (buf.len() * 2) as u32;
        let status = RegQueryValueExW(
            hkey,
            value_w.as_ptr(),
            std::ptr::null(),
            &mut kind,
            buf.as_mut_ptr().cast::<u8>(),
            &mut buf_bytes,
        );
        let _ = RegCloseKey(hkey);
        if status != ERROR_SUCCESS || kind != REG_SZ {
            return None;
        }
        let units = buf_bytes as usize / 2;
        let text = String::from_utf16_lossy(&buf[..units]);
        let trimmed = text.trim_end_matches('\0').trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn run_hidden(args: &[&str]) -> Result<String> {
    let (program, rest) = args
        .split_first()
        .ok_or_else(|| anyhow!("empty command"))?;
    let mut cmd = Command::new(program);
    cmd.args(rest);
    {
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let output = cmd
        .output()
        .with_context(|| format!("spawn `{program}`"))?;
    // `wsl.exe` writes UTF-16LE when stdout is a pipe, not a console.
    let mut text = decode_command_output(&output.stdout);
    if text.trim().is_empty() {
        text = decode_command_output(&output.stderr);
    }
    if !output.status.success() && text.trim().is_empty() {
        return Err(anyhow!("`{program}` exited {}", output.status));
    }
    Ok(text)
}
