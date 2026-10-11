//! Read the installed WSL package version from the machine registry.
//!
//! Same source WSL itself uses: `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Lxss\Msi\Version`.
//! No CLI. After `wslcsdk.dll` loads, use `wslc::Service::version()` instead.

#![cfg(windows)]

use anyhow::{anyhow, Result};

use super::sdk_version::SemVer;

/// Installed WSL package version (`x.y.z`), if the MSI registry value is present.
pub fn package_version() -> Option<String> {
    for subkey in [
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Lxss\Msi",
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Lxss\MSI",
    ] {
        if let Some(raw) = read_hk_lm_string(subkey, "Version")
            && let Some(v) = SemVer::parse(&raw)
        {
            return Some(v.to_string());
        }
    }
    None
}

/// Host version for NuGet package selection.
pub fn host_version() -> Result<String> {
    package_version().ok_or_else(|| {
        anyhow!(
            "WSL package version not found in registry (Lxss\\Msi\\Version); \
             install or update WSL with `wsl --update`"
        )
    })
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
