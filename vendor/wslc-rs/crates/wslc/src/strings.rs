use std::ffi::{CString, OsStr};
use std::path::Path;

use crate::{Error, Result};

pub(crate) fn cstring(value: &str, field: &str) -> Result<CString> {
    CString::new(value).map_err(|_| Error::Nul(field.to_owned()))
}

pub(crate) fn cstrings<'a>(
    values: impl IntoIterator<Item = &'a str>,
    field: &str,
) -> Result<Vec<CString>> {
    values
        .into_iter()
        .map(|value| cstring(value, field))
        .collect()
}

#[cfg(windows)]
pub(crate) fn wide_os(value: &OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    value.encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(not(windows))]
pub(crate) fn wide_os(value: &OsStr) -> Vec<u16> {
    value
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect()
}

pub(crate) fn wide_path(value: &Path) -> Vec<u16> {
    wide_os(value.as_os_str())
}

pub(crate) fn wide_str(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cstring_rejects_interior_nul() {
        let err = cstring("a\0b", "field").unwrap_err();
        assert!(matches!(err, Error::Nul(field) if field == "field"));
    }
}
