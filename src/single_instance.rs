//! Per-user single-instance guard so two app processes cannot share one session.
//!
//! A second launch finds the existing main window, shows it (even when hidden
//! to the tray), and exits silently — no "already running" MessageBox.

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

/// Bring the already-running instance's main window to the foreground.
///
/// Used when a second process hits the single-instance mutex: show the window
/// even if it was minimized or hidden to the tray, then exit without a dialog.
pub fn activate_existing_instance() {
    #[cfg(windows)]
    {
        activate_windows();
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
fn activate_windows() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, FindWindowW, IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE,
        SW_SHOW,
    };

    // Must match `window.set_window_title("WSLc Desktop")` in `main`.
    let title = wide("WSLc Desktop");
    unsafe {
        let hwnd = FindWindowW(std::ptr::null(), title.as_ptr());
        if hwnd.is_null() {
            return;
        }
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        } else {
            // Covers normal windows and SW_HIDE (tray minimize).
            ShowWindow(hwnd, SW_SHOW);
        }
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
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
