use crate::{com, raw, Error, Result};

/// Flags describing missing WSLC prerequisites.
#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub struct ComponentFlags(u32);

impl ComponentFlags {
    /// No missing components.
    pub const NONE: Self = Self(wslc_sys::WSLC_COMPONENT_FLAG_NONE);
    /// Virtual Machine Platform optional feature is missing.
    pub const VIRTUAL_MACHINE_PLATFORM: Self =
        Self(wslc_sys::WSLC_COMPONENT_FLAG_VIRTUAL_MACHINE_PLATFORM);
    /// WSL runtime package is missing.
    pub const WSL_PACKAGE: Self = Self(wslc_sys::WSLC_COMPONENT_FLAG_WSL_PACKAGE);
    /// WSLC SDK needs an update.
    pub const SDK_NEEDS_UPDATE: Self = Self(wslc_sys::WSLC_COMPONENT_FLAG_SDK_NEEDS_UPDATE);

    /// Creates flags from raw SDK bits.
    pub const fn from_bits_retain(bits: u32) -> Self {
        Self(bits)
    }

    /// Returns the raw SDK bits.
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Returns true when no bits are set.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns true when all bits from `other` are present.
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl std::fmt::Debug for ComponentFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts = Vec::new();
        if self.contains(Self::VIRTUAL_MACHINE_PLATFORM) {
            parts.push("VIRTUAL_MACHINE_PLATFORM");
        }
        if self.contains(Self::WSL_PACKAGE) {
            parts.push("WSL_PACKAGE");
        }
        if self.contains(Self::SDK_NEEDS_UPDATE) {
            parts.push("SDK_NEEDS_UPDATE");
        }
        if parts.is_empty() {
            parts.push("NONE");
        }
        write!(f, "ComponentFlags({})", parts.join(" | "))
    }
}

/// WSLC runtime version.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Version {
    /// Major version.
    pub major: u32,
    /// Minor version.
    pub minor: u32,
    /// Revision version.
    pub revision: u32,
}

impl From<wslc_sys::WslcVersion> for Version {
    fn from(value: wslc_sys::WslcVersion) -> Self {
        Self {
            major: value.major,
            minor: value.minor,
            revision: value.revision,
        }
    }
}

/// Installation progress event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstallProgress {
    /// Component being installed.
    pub component: ComponentFlags,
    /// Completed progress steps.
    pub progress_steps: u32,
    /// Total progress steps.
    pub total_steps: u32,
}

/// Service-level WSLC API.
pub struct Service;

impl Service {
    /// Returns missing WSLC components.
    pub fn missing_components() -> Result<ComponentFlags> {
        let _com = com::try_initialize_mta()?;
        let sdk = raw::sdk()?;
        let flags = raw::map_result(sdk.missing_components())?;
        Ok(ComponentFlags::from_bits_retain(flags))
    }

    /// Returns the WSLC runtime version.
    pub fn version() -> Result<Version> {
        let _com = com::try_initialize_mta()?;
        let sdk = raw::sdk()?;
        let version = raw::map_result(sdk.version())?;
        Ok(version.into())
    }

    /// Ensures WSLC is available and the runtime can report its version.
    pub fn ensure_available() -> Result<()> {
        let missing = Self::missing_components()?;
        if !missing.is_empty() {
            return Err(Error::MissingComponents(missing));
        }
        Self::version()?;
        Ok(())
    }

    /// Installs missing WSLC dependencies, reporting progress through a callback.
    pub fn install_with_dependencies<F>(progress: F) -> Result<()>
    where
        F: FnMut(InstallProgress) + Send + 'static,
    {
        let _com = com::try_initialize_mta()?;
        let sdk = raw::sdk()?;
        let mut callback = Box::new(progress);
        raw::map_result(sdk.install_with_dependencies(
            &mut |component, progress_steps, total_steps| {
                callback(InstallProgress {
                    component: ComponentFlags::from_bits_retain(component),
                    progress_steps,
                    total_steps,
                });
            },
        ))
    }
}
