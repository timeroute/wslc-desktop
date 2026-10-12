//! Public data types for the WSLc client API.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // `Live` is constructed on Windows SDK builds.
pub enum BackendKind {
    Live,
    Unavailable,
}

#[derive(Debug, Clone)]
pub struct EngineStatus {
    pub backend: BackendKind,
    /// Engine label shown in the status bar (`wslcsdk` when live).
    pub binary: String,
    pub version: String,
    pub message: String,
    /// Session CPU count requested at start.
    pub cpu_count: u32,
    /// Session memory (MiB) requested at start.
    pub memory_mb: u32,
}

impl EngineStatus {
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            backend: BackendKind::Unavailable,
            binary: "—".into(),
            version: String::new(),
            message: message.into(),
            cpu_count: 0,
            memory_mb: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub ports: String,
    pub volumes: String,
    pub auto_remove: bool,
    pub created: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortMapping {
    pub host: u16,
    pub container: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeMapping {
    /// Host path (bind) or session volume name (named).
    pub host: String,
    pub container: String,
    pub read_only: bool,
    /// When true, `host` is a session VHD volume name.
    pub named: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateContainerRequest {
    pub image: String,
    pub name: Option<String>,
    pub ports: Vec<PortMapping>,
    pub volumes: Vec<VolumeMapping>,
    pub auto_remove: bool,
    /// Extra init command line (quoted tokens OK). Empty → image ENTRYPOINT/CMD.
    pub command: Option<String>,
}

impl ContainerInfo {
    pub fn is_running(&self) -> bool {
        let s = self.status.to_ascii_lowercase();
        s.contains("up") || s.contains("running")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageInfo {
    pub repository: String,
    pub tag: String,
    pub id: String,
    pub size: String,
    pub created: String,
}

impl ImageInfo {
    pub fn reference(&self) -> String {
        if self.tag.is_empty() || self.tag == "<none>" {
            self.repository.clone()
        } else {
            format!("{}:{}", self.repository, self.tag)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VolumeInfo {
    pub name: String,
    pub driver: String,
    pub mountpoint: String,
    pub created: String,
    pub size: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // Variants are produced by the Windows live worker.
pub enum PullEvent {
    Status(String),
    Progress(f32),
    Log(String),
    Finished(Result<(), String>),
}

/// First-launch steps shown in the setup dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupStepId {
    WslVersion,
    SdkInstall,
}

/// Why a setup step failed. The UI localizes the known variants.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum SetupFault {
    /// Registry `Lxss\Msi\Version` missing or unreadable.
    WslMissing,
    /// Package version is below 2.9.3.
    WslTooOld { found: String },
    /// This build is not running on Windows.
    NotWindows,
    /// Free-form failure (SDK download, extract, or session boot).
    Message(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum SetupStepState {
    Pending,
    Running,
    Passed { detail: String },
    Failed(SetupFault),
}

impl SetupStepState {
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }
}

/// Progress / log stream while installing or connecting the SDK.
pub enum SdkSetupEvent {
    Status(String),
    Progress(f32),
    Log(String),
    Step {
        id: SetupStepId,
        state: SetupStepState,
    },
    Finished(Result<super::WslcClient, String>),
}

