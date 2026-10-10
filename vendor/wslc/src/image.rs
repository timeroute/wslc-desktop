use crate::{raw, registry, strings, Error, Result, Session};

/// Options for pulling an image.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImagePullOptions {
    /// Image URI.
    pub uri: String,
    /// Optional registry auth.
    pub registry_auth: Option<String>,
}

impl ImagePullOptions {
    /// Creates pull options.
    pub fn new(uri: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            registry_auth: None,
        }
    }

    /// Sets a registry auth token.
    pub fn registry_auth(mut self, registry_auth: impl Into<String>) -> Self {
        self.registry_auth = Some(registry_auth.into());
        self
    }

    /// Validates the options.
    pub fn validate(&self) -> Result<()> {
        if self.uri.trim().is_empty() {
            return Err(Error::InvalidInput("image uri cannot be empty".to_owned()));
        }
        if self.uri.contains('\0') {
            return Err(Error::Nul("image uri".to_owned()));
        }
        if self
            .registry_auth
            .as_ref()
            .is_some_and(|value| value.contains('\0'))
        {
            return Err(Error::Nul("registry_auth".to_owned()));
        }
        Ok(())
    }
}

/// Pull operation builder.
pub struct ImagePullOperation<'a> {
    pub(crate) session: &'a Session,
    pub(crate) options: ImagePullOptions,
    pub(crate) progress: Option<Box<dyn FnMut(ImageProgress) + Send>>,
}

impl<'a> ImagePullOperation<'a> {
    /// Registers a progress callback.
    pub fn on_progress<F>(mut self, f: F) -> Self
    where
        F: FnMut(ImageProgress) + Send + 'static,
    {
        self.progress = Some(Box::new(f));
        self
    }

    /// Runs the pull operation.
    pub fn run(mut self) -> Result<()> {
        self.options.validate()?;
        let sdk = raw::sdk()?;
        let resolved_uri = registry::resolve_image_reference(&self.options.uri)?;
        let uri = strings::cstring(&resolved_uri, "image uri")?;
        let registry_auth;
        let registry_auth_ptr = if let Some(auth) = &self.options.registry_auth {
            registry_auth = strings::cstring(auth, "registry_auth")?;
            registry_auth.as_ptr()
        } else {
            std::ptr::null()
        };

        if let Some(mut callback) = self.progress.take() {
            raw::map_result(sdk.pull_session_image_with_progress(
                self.session.raw(),
                uri.as_ptr(),
                registry_auth_ptr,
                &mut |progress| {
                    callback(ImageProgress {
                        id: progress.id,
                        status: progress.status.into(),
                        current_bytes: progress.current_bytes,
                        total_bytes: progress.total_bytes,
                    });
                },
            ))
        } else {
            raw::map_result(sdk.pull_session_image(
                self.session.raw(),
                uri.as_ptr(),
                registry_auth_ptr,
            ))
        }
    }
}

/// Image progress event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageProgress {
    /// Layer ID or digest.
    pub id: String,
    /// Progress status.
    pub status: ImageProgressStatus,
    /// Current bytes.
    pub current_bytes: u64,
    /// Total bytes.
    pub total_bytes: u64,
}

/// Image progress status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageProgressStatus {
    /// Unknown.
    Unknown,
    /// Pulling.
    Pulling,
    /// Waiting.
    Waiting,
    /// Downloading.
    Downloading,
    /// Verifying.
    Verifying,
    /// Extracting.
    Extracting,
    /// Complete.
    Complete,
}

impl From<wslc_sys::WslcImageProgressStatus> for ImageProgressStatus {
    fn from(value: wslc_sys::WslcImageProgressStatus) -> Self {
        match value {
            wslc_sys::WslcImageProgressStatus::WSLC_IMAGE_PROGRESS_STATUS_PULLING => Self::Pulling,
            wslc_sys::WslcImageProgressStatus::WSLC_IMAGE_PROGRESS_STATUS_WAITING => Self::Waiting,
            wslc_sys::WslcImageProgressStatus::WSLC_IMAGE_PROGRESS_STATUS_DOWNLOADING => {
                Self::Downloading
            }
            wslc_sys::WslcImageProgressStatus::WSLC_IMAGE_PROGRESS_STATUS_VERIFYING => {
                Self::Verifying
            }
            wslc_sys::WslcImageProgressStatus::WSLC_IMAGE_PROGRESS_STATUS_EXTRACTING => {
                Self::Extracting
            }
            wslc_sys::WslcImageProgressStatus::WSLC_IMAGE_PROGRESS_STATUS_COMPLETE => {
                Self::Complete
            }
            _ => Self::Unknown,
        }
    }
}

/// Image metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageInfo {
    /// Image name.
    pub name: String,
    /// SHA-256 digest bytes.
    pub sha256: [u8; 32],
    /// Size in bytes.
    pub size_bytes: i64,
    /// Creation time as Unix timestamp.
    pub created_unix_time: u64,
}

impl TryFrom<wslc_sys::WslcImageInfo> for ImageInfo {
    type Error = crate::Error;

    fn try_from(value: wslc_sys::WslcImageInfo) -> Result<Self> {
        let name = raw::image_name_to_string(&value.name)?;
        Ok(Self {
            name,
            sha256: value.sha256,
            size_bytes: value.sizeBytes,
            created_unix_time: value.createdUnixTime,
        })
    }
}
