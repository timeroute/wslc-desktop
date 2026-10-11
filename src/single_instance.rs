//! Per-user single-instance guard so two app processes cannot share one session.

pub struct InstanceLock {
    #[cfg(windows)]
    handle: windows_sys::Win32::Foundation::HANDLE,
}

#[derive(Debug)]
pub enum AcquireError {
    /// Another process already holds the instance mutex.
    #[cfg_attr(not(windows), allow(dead_code))]
    AlreadyRunning,
    /// `CreateMutexW` failed; caller may continue without a lock.
    #[cfg_attr(not(windows), allow(dead_code))]
    Failed(u32),
}

/// Try to become the only WSLc Desktop instance for this Windows user session.
///
/// Non-Windows builds always succeed (the app cannot load the SDK there anyway).
pub fn try_acquire() -> Result<InstanceLock, AcquireError> {
    #[cfg(windows)]
    {
        acquire_windows()
    }
    #[cfg(not(windows))]
    {
        Ok(InstanceLock {})
    }
}

/// Tell the user another instance is already open (Windows MessageBox).
pub fn notify_already_running() {
    #[cfg(windows)]
    {
        notify_windows();
    }
}

#[cfg(windows)]
fn acquire_windows() -> Result<InstanceLock, AcquireError> {
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, SetLastError, ERROR_ALREADY_EXISTS,
    };
    use windows_sys::Win32::System::Threading::CreateMutexW;

    // `Local\` scopes the mutex to the current Windows session / user.
    let name = wide("Local\\WSLcDesktop_SingleInstance");
    unsafe {
        SetLastError(0);
        let handle = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        if handle.is_null() {
            return Err(AcquireError::Failed(GetLastError()));
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(handle);
            return Err(AcquireError::AlreadyRunning);
        }
        Ok(InstanceLock { handle })
    }
}

#[cfg(windows)]
fn notify_windows() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_OK};

    let (title, body) = if crate::i18n::Locale::from_system() == crate::i18n::Locale::Zh {
        (
            "WSLc Desktop",
            "WSLc Desktop 已在运行，不能同时打开多个实例。",
        )
    } else {
        (
            "WSLc Desktop",
            "WSLc Desktop is already running. Only one instance is allowed.",
        )
    };
    let title = wide(title);
    let body = wide(body);
    unsafe {
        MessageBoxW(std::ptr::null_mut(), body.as_ptr(), title.as_ptr(), MB_OK | MB_ICONINFORMATION);
    }
}

#[cfg(windows)]
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
impl Drop for InstanceLock {
    fn drop(&mut self) {
        use windows_sys::Win32::Foundation::CloseHandle;
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}
