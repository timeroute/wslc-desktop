//! Shared helpers for mapping SDK / Win32 errors into UI-friendly conditions.

/// Win32 `ERROR_ALREADY_EXISTS` (183) as HRESULT `0x800700B7`.
pub fn is_already_exists_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("0x800700b7")
        || lower.contains("already exists")
        || lower.contains("cannot create a file when that file already exists")
}

#[cfg(test)]
mod tests {
    use super::is_already_exists_error;

    #[test]
    fn detects_hresult_already_exists() {
        assert!(is_already_exists_error(
            "WSLC call failed: HRESULT=0x800700b7, Cannot create a file when that file already exists."
        ));
        assert!(is_already_exists_error("volume already exists: data"));
        assert!(!is_already_exists_error(
            "WSLC call failed: HRESULT=0x80070005, Access denied"
        ));
    }
}
