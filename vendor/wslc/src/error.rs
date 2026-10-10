use std::fmt;

/// Crate result type.
pub type Result<T> = std::result::Result<T, Error>;

/// Error type used by the safe WSLC wrapper.
#[derive(Debug)]
pub enum Error {
    /// The current target platform cannot run WSLC.
    UnsupportedPlatform(&'static str),
    /// `wslcsdk.dll` or one of its required exports could not be loaded.
    SdkNotFound(String),
    /// Required Windows/WSL components are not present or need updates.
    MissingComponents(crate::ComponentFlags),
    /// COM initialization failed on the current thread.
    ComInitialization { code: i32, message: String },
    /// A WSLC SDK call returned a failing HRESULT.
    HResult { code: i32, message: String },
    /// User input failed Rust-side validation.
    InvalidInput(String),
    /// A string passed to a C API contains an interior NUL byte.
    Nul(String),
    /// Standard I/O error.
    Io(std::io::Error),
    /// UTF-8 conversion error.
    Utf8(std::string::FromUtf8Error),
}

impl Error {
    /// Creates an HRESULT-backed error.
    pub fn from_hresult(code: i32, message: impl Into<String>) -> Self {
        Self::HResult {
            code,
            message: message.into(),
        }
    }

    /// Returns the original HRESULT when this error has one.
    pub fn hresult(&self) -> Option<i32> {
        match self {
            Self::ComInitialization { code, .. } | Self::HResult { code, .. } => Some(*code),
            _ => None,
        }
    }

    /// Returns a typed WSLC error kind for known WSLC-specific HRESULT values.
    pub fn wslc_kind(&self) -> Option<WslcErrorKind> {
        let code = self.hresult()?;
        Some(match code as u32 {
            wslc_sys::WSLC_E_IMAGE_NOT_FOUND => WslcErrorKind::ImageNotFound,
            wslc_sys::WSLC_E_CONTAINER_PREFIX_AMBIGUOUS => WslcErrorKind::ContainerPrefixAmbiguous,
            wslc_sys::WSLC_E_CONTAINER_NOT_FOUND => WslcErrorKind::ContainerNotFound,
            wslc_sys::WSLC_E_VOLUME_NOT_FOUND => WslcErrorKind::VolumeNotFound,
            wslc_sys::WSLC_E_CONTAINER_NOT_RUNNING => WslcErrorKind::ContainerNotRunning,
            wslc_sys::WSLC_E_CONTAINER_IS_RUNNING => WslcErrorKind::ContainerIsRunning,
            wslc_sys::WSLC_E_SESSION_RESERVED => WslcErrorKind::SessionReserved,
            wslc_sys::WSLC_E_INVALID_SESSION_NAME => WslcErrorKind::InvalidSessionName,
            wslc_sys::WSLC_E_NETWORK_NOT_FOUND => WslcErrorKind::NetworkNotFound,
            wslc_sys::WSLC_E_WU_SEARCH_FAILED => WslcErrorKind::WindowsUpdateSearchFailed,
            wslc_sys::WSLC_E_SDK_UPDATE_NEEDED => WslcErrorKind::SdkUpdateNeeded,
            wslc_sys::WSLC_E_CONTAINER_DISABLED => WslcErrorKind::ContainerDisabled,
            wslc_sys::WSLC_E_REGISTRY_BLOCKED_BY_POLICY => WslcErrorKind::RegistryBlockedByPolicy,
            wslc_sys::WSLC_E_VOLUME_NOT_AVAILABLE => WslcErrorKind::VolumeNotAvailable,
            wslc_sys::WSLC_E_SESSION_NOT_FOUND => WslcErrorKind::SessionNotFound,
            other if (other & 0xffff_0000) == 0x8004_0000 => WslcErrorKind::Unknown(code),
            _ => return None,
        })
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform(platform) => write!(f, "unsupported platform: {platform}"),
            Self::SdkNotFound(message) => write!(f, "WSLC SDK not found: {message}"),
            Self::MissingComponents(flags) => write!(f, "missing WSL components: {flags:?}"),
            Self::ComInitialization { code, message } => {
                write!(
                    f,
                    "COM initialization failed: HRESULT=0x{:08x}, {message}",
                    *code as u32
                )
            }
            Self::HResult { code, message } => {
                write!(
                    f,
                    "WSLC call failed: HRESULT=0x{:08x}, {message}",
                    *code as u32
                )
            }
            Self::InvalidInput(message) => write!(f, "invalid input: {message}"),
            Self::Nul(field) => write!(f, "string contains interior NUL: {field}"),
            Self::Io(error) => write!(f, "I/O error: {error}"),
            Self::Utf8(error) => write!(f, "UTF-8 conversion error: {error}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Utf8(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(value: std::string::FromUtf8Error) -> Self {
        Self::Utf8(value)
    }
}

/// Matchable WSLC-specific error classes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WslcErrorKind {
    /// Image name or ID was not found.
    ImageNotFound,
    /// Container prefix matched multiple containers.
    ContainerPrefixAmbiguous,
    /// Container was not found.
    ContainerNotFound,
    /// Volume was not found.
    VolumeNotFound,
    /// Container was not running.
    ContainerNotRunning,
    /// Container was already running.
    ContainerIsRunning,
    /// Session name is reserved.
    SessionReserved,
    /// Session name is invalid.
    InvalidSessionName,
    /// Network was not found.
    NetworkNotFound,
    /// Windows Update search failed.
    WindowsUpdateSearchFailed,
    /// SDK needs to be updated.
    SdkUpdateNeeded,
    /// Container support is disabled.
    ContainerDisabled,
    /// Registry access is blocked by policy.
    RegistryBlockedByPolicy,
    /// Volume is not available.
    VolumeNotAvailable,
    /// Session was not found.
    SessionNotFound,
    /// Unknown WSLC-specific HRESULT.
    Unknown(i32),
}
