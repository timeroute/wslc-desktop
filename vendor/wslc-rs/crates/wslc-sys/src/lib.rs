//! Raw Rust ABI definitions for Microsoft WSL Container SDK.
//!
//! This crate mirrors the public `wslcsdk.h` surface closely, but it does not
//! link to or load `wslcsdk.dll`. Safe wrappers can use these definitions with
//! either static linking or runtime loading.

#![deny(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

pub mod runtime;

use core::ffi::{c_char, c_void};

/// HRESULT-compatible return value.
pub type HRESULT = i32;
/// Windows BOOL-compatible value.
pub type BOOL = i32;
/// Windows HANDLE-compatible value.
pub type HANDLE = *mut c_void;
/// UTF-16 string pointer.
pub type PCWSTR = *const u16;
/// UTF-16 mutable string pointer.
pub type PWSTR = *mut u16;
/// ANSI/UTF-8 string pointer used by the WSLC SDK.
pub type PCSTR = *const c_char;
/// ANSI/UTF-8 mutable string pointer used by the WSLC SDK.
pub type PSTR = *mut c_char;
/// Generic SDK callback context pointer.
pub type PVOID = *mut c_void;

/// `S_OK`.
pub const S_OK: HRESULT = 0;

/// Base offset used by WSLC-specific HRESULT values.
pub const WSLC_E_BASE: u32 = 0x0600;
/// Image not found.
pub const WSLC_E_IMAGE_NOT_FOUND: u32 = 0x8004_0601;
/// Container prefix ambiguous.
pub const WSLC_E_CONTAINER_PREFIX_AMBIGUOUS: u32 = 0x8004_0602;
/// Container not found.
pub const WSLC_E_CONTAINER_NOT_FOUND: u32 = 0x8004_0603;
/// Volume not found.
pub const WSLC_E_VOLUME_NOT_FOUND: u32 = 0x8004_0604;
/// Container not running.
pub const WSLC_E_CONTAINER_NOT_RUNNING: u32 = 0x8004_0605;
/// Container is already running.
pub const WSLC_E_CONTAINER_IS_RUNNING: u32 = 0x8004_0606;
/// Session name is reserved.
pub const WSLC_E_SESSION_RESERVED: u32 = 0x8004_0607;
/// Session name is invalid.
pub const WSLC_E_INVALID_SESSION_NAME: u32 = 0x8004_0608;
/// Network not found.
pub const WSLC_E_NETWORK_NOT_FOUND: u32 = 0x8004_0609;
/// Windows Update search failed.
pub const WSLC_E_WU_SEARCH_FAILED: u32 = 0x8004_060A;
/// SDK update is needed.
pub const WSLC_E_SDK_UPDATE_NEEDED: u32 = 0x8004_060B;
/// Container support is disabled.
pub const WSLC_E_CONTAINER_DISABLED: u32 = 0x8004_060C;
/// Registry is blocked by policy.
pub const WSLC_E_REGISTRY_BLOCKED_BY_POLICY: u32 = 0x8004_060D;
/// Volume is not available.
pub const WSLC_E_VOLUME_NOT_AVAILABLE: u32 = 0x8004_060E;
/// Session not found.
pub const WSLC_E_SESSION_NOT_FOUND: u32 = 0x8004_060F;

/// `WslcSessionSettings` size from `wslcsdk.h`.
pub const WSLC_SESSION_OPTIONS_SIZE: usize = 72;
/// `WslcSessionSettings` alignment from `wslcsdk.h`.
pub const WSLC_SESSION_OPTIONS_ALIGNMENT: usize = 8;
/// `WslcContainerSettings` size from `wslcsdk.h`.
pub const WSLC_CONTAINER_OPTIONS_SIZE: usize = 104;
/// `WslcContainerSettings` alignment from `wslcsdk.h`.
pub const WSLC_CONTAINER_OPTIONS_ALIGNMENT: usize = 8;
/// `WslcProcessSettings` size from `wslcsdk.h`.
pub const WSLC_CONTAINER_PROCESS_OPTIONS_SIZE: usize = 72;
/// `WslcProcessSettings` alignment from `wslcsdk.h`.
pub const WSLC_CONTAINER_PROCESS_OPTIONS_ALIGNMENT: usize = 8;
/// Container ID buffer size: 64 hex chars plus NUL terminator.
pub const WSLC_CONTAINER_ID_BUFFER_SIZE: usize = 65;
/// Image name buffer size.
pub const WSLC_IMAGE_NAME_LENGTH: usize = 256;

/// Opaque session settings blob.
#[repr(C, align(8))]
#[derive(Clone, Copy)]
pub struct WslcSessionSettings {
    /// Opaque SDK-owned settings bytes.
    pub _opaque: [u8; WSLC_SESSION_OPTIONS_SIZE],
}

impl Default for WslcSessionSettings {
    fn default() -> Self {
        Self {
            _opaque: [0; WSLC_SESSION_OPTIONS_SIZE],
        }
    }
}

/// Opaque container settings blob.
#[repr(C, align(8))]
#[derive(Clone, Copy)]
pub struct WslcContainerSettings {
    /// Opaque SDK-owned settings bytes.
    pub _opaque: [u8; WSLC_CONTAINER_OPTIONS_SIZE],
}

impl Default for WslcContainerSettings {
    fn default() -> Self {
        Self {
            _opaque: [0; WSLC_CONTAINER_OPTIONS_SIZE],
        }
    }
}

/// Opaque process settings blob.
#[repr(C, align(8))]
#[derive(Clone, Copy)]
pub struct WslcProcessSettings {
    /// Opaque SDK-owned settings bytes.
    pub _opaque: [u8; WSLC_CONTAINER_PROCESS_OPTIONS_SIZE],
}

impl Default for WslcProcessSettings {
    fn default() -> Self {
        Self {
            _opaque: [0; WSLC_CONTAINER_PROCESS_OPTIONS_SIZE],
        }
    }
}

/// Raw WSLC session handle.
pub type WslcSession = *mut c_void;
/// Raw WSLC container handle.
pub type WslcContainer = *mut c_void;
/// Raw WSLC process handle.
pub type WslcProcess = *mut c_void;
/// Raw WSLC crash dump subscription handle.
pub type WslcCrashDumpSubscription = *mut c_void;

/// Container networking mode.
#[repr(i32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WslcContainerNetworkingMode {
    /// No networking / isolated.
    WSLC_CONTAINER_NETWORKING_MODE_NONE = 0,
    /// Bridged networking.
    WSLC_CONTAINER_NETWORKING_MODE_BRIDGED = 1,
}

/// VHD volume type.
#[repr(i32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WslcVhdType {
    /// Expanding VHDX.
    WSLC_VHD_TYPE_DYNAMIC = 0,
    /// Fixed-allocation VHDX.
    WSLC_VHD_TYPE_FIXED = 1,
}

/// VHD requirement flags.
pub type WslcVhdRequirementsFlags = u32;
/// No VHD requirement flags.
pub const WSLC_VHD_REQ_FLAG_NONE: WslcVhdRequirementsFlags = 0;
/// Honor owner UID/GID.
pub const WSLC_VHD_REQ_FLAG_OWNER: WslcVhdRequirementsFlags = 0x0000_0001;

/// VHD requirements.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct WslcVhdRequirements {
    /// Volume name.
    pub name: PCSTR,
    /// Desired size.
    pub sizeBytes: u64,
    /// Volume type.
    pub type_: WslcVhdType,
    /// Flags controlling optional fields.
    pub flags: WslcVhdRequirementsFlags,
    /// Linux UID.
    pub uid: u32,
    /// Linux GID.
    pub gid: u32,
}

/// Session feature flags.
pub type WslcSessionFeatureFlags = u32;
/// No session feature flags.
pub const WSLC_SESSION_FEATURE_FLAG_NONE: WslcSessionFeatureFlags = 0;
/// Enable GPU support.
pub const WSLC_SESSION_FEATURE_FLAG_ENABLE_GPU: WslcSessionFeatureFlags = 0x0000_0004;

/// Session termination reason.
#[repr(i32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WslcSessionTerminationReason {
    /// Unknown reason.
    WSLC_SESSION_TERMINATION_REASON_UNKNOWN = 0,
    /// Session was shut down.
    WSLC_SESSION_TERMINATION_REASON_SHUTDOWN = 1,
    /// Session crashed.
    WSLC_SESSION_TERMINATION_REASON_CRASHED = 2,
}

/// Session crash dump information.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct WslcSessionCrashDumpInfo {
    /// Dump file path.
    pub dumpPath: PCWSTR,
    /// Process name.
    pub processName: PCSTR,
    /// Process ID.
    pub pid: u32,
    /// Signal value.
    pub signal: u32,
    /// Timestamp.
    pub timestamp: u64,
}

/// Session crash dump callback.
pub type WslcSessionCrashDumpCallback =
    Option<unsafe extern "system" fn(info: *const WslcSessionCrashDumpInfo, context: PVOID)>;

/// Port protocol.
#[repr(i32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WslcPortProtocol {
    /// TCP protocol.
    WSLC_PORT_PROTOCOL_TCP = 0,
    /// UDP protocol.
    WSLC_PORT_PROTOCOL_UDP = 1,
}

/// Large enough storage for IPv4/IPv6 socket addresses.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct sockaddr_storage {
    /// Platform socket storage bytes.
    pub storage: [u8; 128],
}

/// Container port mapping.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct WslcContainerPortMapping {
    /// Port on Windows host.
    pub windowsPort: u16,
    /// Port inside the container.
    pub containerPort: u16,
    /// Transport protocol.
    pub protocol: WslcPortProtocol,
    /// Optional Windows bind address.
    pub windowsAddress: *mut sockaddr_storage,
}

/// Container bind mount volume.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct WslcContainerVolume {
    /// Windows host path.
    pub windowsPath: PCWSTR,
    /// Absolute path in the container.
    pub containerPath: PCSTR,
    /// Non-zero for read-only.
    pub readOnly: BOOL,
}

/// Named VHD volume mount.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct WslcContainerNamedVolume {
    /// Session volume name.
    pub name: PCSTR,
    /// Absolute path in the container.
    pub containerPath: PCSTR,
    /// Non-zero for read-only.
    pub readOnly: BOOL,
}

/// Container flags.
pub type WslcContainerFlags = u32;
/// No container flags.
pub const WSLC_CONTAINER_FLAG_NONE: WslcContainerFlags = 0;
/// Auto-remove the container.
pub const WSLC_CONTAINER_FLAG_AUTO_REMOVE: WslcContainerFlags = 0x0000_0001;
/// Enable GPU for the container.
pub const WSLC_CONTAINER_FLAG_ENABLE_GPU: WslcContainerFlags = 0x0000_0002;
/// Run the container as privileged.
pub const WSLC_CONTAINER_FLAG_PRIVILEGED: WslcContainerFlags = 0x0000_0004;

/// Container start flags.
pub type WslcContainerStartFlags = u32;
/// No container start flags.
pub const WSLC_CONTAINER_START_FLAG_NONE: WslcContainerStartFlags = 0;
/// Attach to the init process.
pub const WSLC_CONTAINER_START_FLAG_ATTACH: WslcContainerStartFlags = 0x0000_0001;

/// Container logs flags from the WSLC shared IDL (`WSLCLogsFlags`).
///
/// The public C SDK header does not yet expose a logs entry point; these flags
/// mirror the service/CLI surface so safe wrappers can share the same bit values.
pub type WslcLogsFlags = u32;
/// No logs flags.
pub const WSLC_LOGS_FLAG_NONE: WslcLogsFlags = 0;
/// Follow log output.
pub const WSLC_LOGS_FLAG_FOLLOW: WslcLogsFlags = 0x0000_0001;
/// Include timestamps in log output.
pub const WSLC_LOGS_FLAG_TIMESTAMPS: WslcLogsFlags = 0x0000_0002;
/// Include extra log details.
pub const WSLC_LOGS_FLAG_DETAILS: WslcLogsFlags = 0x0000_0004;
/// Valid combination mask for logs flags.
pub const WSLC_LOGS_FLAGS_VALID: WslcLogsFlags =
    WSLC_LOGS_FLAG_FOLLOW | WSLC_LOGS_FLAG_TIMESTAMPS | WSLC_LOGS_FLAG_DETAILS;

/// Container state.
#[repr(i32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WslcContainerState {
    /// Invalid state.
    WSLC_CONTAINER_STATE_INVALID = 0,
    /// Created.
    WSLC_CONTAINER_STATE_CREATED = 1,
    /// Running.
    WSLC_CONTAINER_STATE_RUNNING = 2,
    /// Exited.
    WSLC_CONTAINER_STATE_EXITED = 3,
    /// Deleted.
    WSLC_CONTAINER_STATE_DELETED = 4,
}

/// Linux signal values supported by WSLC.
#[repr(i32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WslcSignal {
    /// No signal.
    WSLC_SIGNAL_NONE = 0,
    /// SIGHUP.
    WSLC_SIGNAL_SIGHUP = 1,
    /// SIGINT.
    WSLC_SIGNAL_SIGINT = 2,
    /// SIGQUIT.
    WSLC_SIGNAL_SIGQUIT = 3,
    /// SIGKILL.
    WSLC_SIGNAL_SIGKILL = 9,
    /// SIGTERM.
    WSLC_SIGNAL_SIGTERM = 15,
}

/// Delete container flags.
pub type WslcDeleteContainerFlags = u32;
/// No delete flags.
pub const WSLC_DELETE_CONTAINER_FLAG_NONE: WslcDeleteContainerFlags = 0;
/// Force deletion.
pub const WSLC_DELETE_CONTAINER_FLAG_FORCE: WslcDeleteContainerFlags = 0x01;

/// Process IO handle.
#[repr(i32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WslcProcessIOHandle {
    /// Standard input.
    WSLC_PROCESS_IO_HANDLE_STDIN = 0,
    /// Standard output.
    WSLC_PROCESS_IO_HANDLE_STDOUT = 1,
    /// Standard error.
    WSLC_PROCESS_IO_HANDLE_STDERR = 2,
}

/// STDIO callback.
pub type WslcStdIOCallback = Option<
    unsafe extern "system" fn(
        ioHandle: WslcProcessIOHandle,
        data: *const u8,
        dataBytes: u32,
        context: PVOID,
    ),
>;

/// Process exit callback.
pub type WslcProcessExitCallback = Option<unsafe extern "system" fn(exitCode: i32, context: PVOID)>;

/// Process callback set.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct WslcProcessCallbacks {
    /// Standard output callback.
    pub onStdOut: WslcStdIOCallback,
    /// Standard error callback.
    pub onStdErr: WslcStdIOCallback,
    /// Process exit callback.
    pub onExit: WslcProcessExitCallback,
}

/// Process state.
#[repr(i32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WslcProcessState {
    /// Unknown.
    WSLC_PROCESS_STATE_UNKNOWN = 0,
    /// Running.
    WSLC_PROCESS_STATE_RUNNING = 1,
    /// Exited.
    WSLC_PROCESS_STATE_EXITED = 2,
    /// Signalled.
    WSLC_PROCESS_STATE_SIGNALLED = 3,
}

/// Image progress detail.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WslcImageProgressDetail {
    /// Downloaded bytes.
    pub currentBytes: u64,
    /// Expected bytes.
    pub totalBytes: u64,
}

/// Image progress status.
#[repr(i32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WslcImageProgressStatus {
    /// Unknown.
    WSLC_IMAGE_PROGRESS_STATUS_UNKNOWN = 0,
    /// Pulling.
    WSLC_IMAGE_PROGRESS_STATUS_PULLING = 1,
    /// Waiting.
    WSLC_IMAGE_PROGRESS_STATUS_WAITING = 2,
    /// Downloading.
    WSLC_IMAGE_PROGRESS_STATUS_DOWNLOADING = 3,
    /// Verifying.
    WSLC_IMAGE_PROGRESS_STATUS_VERIFYING = 4,
    /// Extracting.
    WSLC_IMAGE_PROGRESS_STATUS_EXTRACTING = 5,
    /// Complete.
    WSLC_IMAGE_PROGRESS_STATUS_COMPLETE = 6,
}

/// Image progress message.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct WslcImageProgressMessage {
    /// Layer ID or digest.
    pub id: PCSTR,
    /// Status.
    pub status: WslcImageProgressStatus,
    /// Detail.
    pub detail: WslcImageProgressDetail,
}

/// Container image progress callback.
pub type WslcContainerImageProgressCallback = Option<
    unsafe extern "system" fn(progress: *const WslcImageProgressMessage, context: PVOID) -> HRESULT,
>;

/// Pull image options.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct WslcPullImageOptions {
    /// Image URI.
    pub uri: PCSTR,
    /// Progress callback.
    pub progressCallback: WslcContainerImageProgressCallback,
    /// Progress callback context.
    pub progressCallbackContext: PVOID,
    /// Optional registry auth.
    pub registryAuth: PCSTR,
}

/// Import image options.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct WslcImportImageOptions {
    /// Progress callback.
    pub progressCallback: WslcContainerImageProgressCallback,
    /// Progress callback context.
    pub progressCallbackContext: PVOID,
}

/// Load image options.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct WslcLoadImageOptions {
    /// Progress callback.
    pub progressCallback: WslcContainerImageProgressCallback,
    /// Progress callback context.
    pub progressCallbackContext: PVOID,
}

/// Image information.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct WslcImageInfo {
    /// Image name buffer.
    pub name: [c_char; WSLC_IMAGE_NAME_LENGTH],
    /// SHA-256 digest bytes.
    pub sha256: [u8; 32],
    /// Image size in bytes.
    pub sizeBytes: i64,
    /// Creation time as Unix timestamp.
    pub createdUnixTime: u64,
}

/// Tag image options.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct WslcTagImageOptions {
    /// Source image.
    pub image: PCSTR,
    /// Target repository.
    pub repo: PCSTR,
    /// Target tag.
    pub tag: PCSTR,
}

/// Push image options.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct WslcPushImageOptions {
    /// Image name.
    pub image: PCSTR,
    /// Registry auth token.
    pub registryAuth: PCSTR,
    /// Progress callback.
    pub progressCallback: WslcContainerImageProgressCallback,
    /// Progress callback context.
    pub progressCallbackContext: PVOID,
}

/// Component flags.
pub type WslcComponentFlags = u32;
/// No missing components.
pub const WSLC_COMPONENT_FLAG_NONE: WslcComponentFlags = 0;
/// Virtual Machine Platform optional feature is missing.
pub const WSLC_COMPONENT_FLAG_VIRTUAL_MACHINE_PLATFORM: WslcComponentFlags = 1;
/// WSL runtime package is missing.
pub const WSLC_COMPONENT_FLAG_WSL_PACKAGE: WslcComponentFlags = 2;
/// WSLC SDK needs an update.
pub const WSLC_COMPONENT_FLAG_SDK_NEEDS_UPDATE: WslcComponentFlags = 4;

/// WSLC version.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WslcVersion {
    /// Major version.
    pub major: u32,
    /// Minor version.
    pub minor: u32,
    /// Revision version.
    pub revision: u32,
}

/// Install progress callback.
pub type WslcInstallCallback = Option<
    unsafe extern "system" fn(
        component: WslcComponentFlags,
        progressSteps: u32,
        totalSteps: u32,
        context: PVOID,
    ),
>;

/// `WslcInitSessionSettings`.
pub type WslcInitSessionSettingsFn = unsafe extern "system" fn(
    name: PCWSTR,
    storagePath: PCWSTR,
    sessionSettings: *mut WslcSessionSettings,
) -> HRESULT;
/// `WslcCreateSession`.
pub type WslcCreateSessionFn = unsafe extern "system" fn(
    sessionSettings: *mut WslcSessionSettings,
    session: *mut WslcSession,
    errorMessage: *mut PWSTR,
) -> HRESULT;
/// `WslcSetSessionSettingsCpuCount`.
pub type WslcSetSessionSettingsCpuCountFn =
    unsafe extern "system" fn(sessionSettings: *mut WslcSessionSettings, cpuCount: u32) -> HRESULT;
/// `WslcSetSessionSettingsMemory`.
pub type WslcSetSessionSettingsMemoryFn =
    unsafe extern "system" fn(sessionSettings: *mut WslcSessionSettings, memoryMB: u32) -> HRESULT;
/// `WslcSetSessionSettingsTimeout`.
pub type WslcSetSessionSettingsTimeoutFn =
    unsafe extern "system" fn(sessionSettings: *mut WslcSessionSettings, timeoutMS: u32) -> HRESULT;
/// `WslcSetSessionSettingsVhd`.
pub type WslcSetSessionSettingsVhdFn = unsafe extern "system" fn(
    sessionSettings: *mut WslcSessionSettings,
    vhdRequirements: *const WslcVhdRequirements,
) -> HRESULT;
/// `WslcSetSessionSettingsFeatureFlags`.
pub type WslcSetSessionSettingsFeatureFlagsFn = unsafe extern "system" fn(
    sessionSettings: *mut WslcSessionSettings,
    flags: WslcSessionFeatureFlags,
) -> HRESULT;
/// `WslcGetSessionTerminationEvent`.
pub type WslcGetSessionTerminationEventFn =
    unsafe extern "system" fn(session: WslcSession, terminationEvent: *mut HANDLE) -> HRESULT;
/// `WslcGetSessionTerminationReason`.
pub type WslcGetSessionTerminationReasonFn = unsafe extern "system" fn(
    session: WslcSession,
    reason: *mut WslcSessionTerminationReason,
) -> HRESULT;
/// `WslcTerminateSession`.
pub type WslcTerminateSessionFn = unsafe extern "system" fn(session: WslcSession) -> HRESULT;
/// `WslcReleaseSession`.
pub type WslcReleaseSessionFn = unsafe extern "system" fn(session: WslcSession) -> HRESULT;
/// `WslcRegisterSessionCrashDumpCallback`.
pub type WslcRegisterSessionCrashDumpCallbackFn = unsafe extern "system" fn(
    session: WslcSession,
    crashDumpCallback: WslcSessionCrashDumpCallback,
    crashDumpContext: PVOID,
    subscription: *mut WslcCrashDumpSubscription,
    errorMessage: *mut PWSTR,
) -> HRESULT;
/// `WslcReleaseCrashDumpSubscription`.
pub type WslcReleaseCrashDumpSubscriptionFn =
    unsafe extern "system" fn(subscription: WslcCrashDumpSubscription) -> HRESULT;

/// `WslcInitContainerSettings`.
pub type WslcInitContainerSettingsFn = unsafe extern "system" fn(
    imageName: PCSTR,
    containerSettings: *mut WslcContainerSettings,
) -> HRESULT;
/// `WslcCreateContainer`.
pub type WslcCreateContainerFn = unsafe extern "system" fn(
    session: WslcSession,
    containerSettings: *const WslcContainerSettings,
    container: *mut WslcContainer,
    errorMessage: *mut PWSTR,
) -> HRESULT;
/// `WslcOpenContainer`.
pub type WslcOpenContainerFn = unsafe extern "system" fn(
    session: WslcSession,
    nameOrId: PCSTR,
    container: *mut WslcContainer,
    errorMessage: *mut PWSTR,
) -> HRESULT;
/// `WslcStartContainer`.
pub type WslcStartContainerFn = unsafe extern "system" fn(
    container: WslcContainer,
    flags: WslcContainerStartFlags,
    errorMessage: *mut PWSTR,
) -> HRESULT;
/// `WslcSetContainerSettingsName`.
pub type WslcSetContainerSettingsNameFn = unsafe extern "system" fn(
    containerSettings: *mut WslcContainerSettings,
    name: PCSTR,
) -> HRESULT;
/// `WslcSetContainerSettingsInitProcess`.
pub type WslcSetContainerSettingsInitProcessFn = unsafe extern "system" fn(
    containerSettings: *mut WslcContainerSettings,
    initProcess: *mut WslcProcessSettings,
) -> HRESULT;
/// `WslcSetContainerSettingsNetworkingMode`.
pub type WslcSetContainerSettingsNetworkingModeFn = unsafe extern "system" fn(
    containerSettings: *mut WslcContainerSettings,
    networkingMode: WslcContainerNetworkingMode,
) -> HRESULT;
/// `WslcSetContainerSettingsHostName`.
pub type WslcSetContainerSettingsHostNameFn = unsafe extern "system" fn(
    containerSettings: *mut WslcContainerSettings,
    hostName: PCSTR,
) -> HRESULT;
/// `WslcSetContainerSettingsDomainName`.
pub type WslcSetContainerSettingsDomainNameFn = unsafe extern "system" fn(
    containerSettings: *mut WslcContainerSettings,
    domainName: PCSTR,
) -> HRESULT;
/// `WslcSetContainerSettingsFlags`.
pub type WslcSetContainerSettingsFlagsFn = unsafe extern "system" fn(
    containerSettings: *mut WslcContainerSettings,
    flags: WslcContainerFlags,
) -> HRESULT;
/// `WslcSetContainerSettingsPortMappings`.
pub type WslcSetContainerSettingsPortMappingsFn = unsafe extern "system" fn(
    containerSettings: *mut WslcContainerSettings,
    portMappings: *const WslcContainerPortMapping,
    portMappingCount: u32,
) -> HRESULT;
/// `WslcSetContainerSettingsVolumes`.
pub type WslcSetContainerSettingsVolumesFn = unsafe extern "system" fn(
    containerSettings: *mut WslcContainerSettings,
    volumes: *const WslcContainerVolume,
    volumeCount: u32,
) -> HRESULT;
/// `WslcSetContainerSettingsNamedVolumes`.
pub type WslcSetContainerSettingsNamedVolumesFn = unsafe extern "system" fn(
    containerSettings: *mut WslcContainerSettings,
    namedVolumes: *const WslcContainerNamedVolume,
    namedVolumeCount: u32,
) -> HRESULT;
/// `WslcCreateContainerProcess`.
pub type WslcCreateContainerProcessFn = unsafe extern "system" fn(
    container: WslcContainer,
    newProcessSettings: *mut WslcProcessSettings,
    newProcess: *mut WslcProcess,
    errorMessage: *mut PWSTR,
) -> HRESULT;
/// `WslcReleaseContainer`.
pub type WslcReleaseContainerFn = unsafe extern "system" fn(container: WslcContainer) -> HRESULT;
/// `WslcGetContainerID`.
pub type WslcGetContainerIDFn =
    unsafe extern "system" fn(container: WslcContainer, containerID: *mut c_char) -> HRESULT;
/// `WslcGetContainerInitProcess`.
pub type WslcGetContainerInitProcessFn =
    unsafe extern "system" fn(container: WslcContainer, initProcess: *mut WslcProcess) -> HRESULT;
/// `WslcInspectContainer`.
pub type WslcInspectContainerFn =
    unsafe extern "system" fn(container: WslcContainer, inspectData: *mut PSTR) -> HRESULT;
/// `WslcGetContainerState`.
pub type WslcGetContainerStateFn =
    unsafe extern "system" fn(container: WslcContainer, state: *mut WslcContainerState) -> HRESULT;
/// `WslcStopContainer`.
pub type WslcStopContainerFn = unsafe extern "system" fn(
    container: WslcContainer,
    signal: WslcSignal,
    timeoutSeconds: u32,
    errorMessage: *mut PWSTR,
) -> HRESULT;
/// `WslcDeleteContainer`.
pub type WslcDeleteContainerFn = unsafe extern "system" fn(
    container: WslcContainer,
    flags: WslcDeleteContainerFlags,
    errorMessage: *mut PWSTR,
) -> HRESULT;

/// `WslcInitProcessSettings`.
pub type WslcInitProcessSettingsFn =
    unsafe extern "system" fn(processSettings: *mut WslcProcessSettings) -> HRESULT;
/// `WslcSetProcessSettingsWorkingDirectory`.
pub type WslcSetProcessSettingsWorkingDirectoryFn = unsafe extern "system" fn(
    processSettings: *mut WslcProcessSettings,
    workingDirectory: PCSTR,
) -> HRESULT;
/// `WslcSetProcessSettingsCmdLine`.
pub type WslcSetProcessSettingsCmdLineFn = unsafe extern "system" fn(
    processSettings: *mut WslcProcessSettings,
    argv: *const PCSTR,
    argc: usize,
) -> HRESULT;
/// `WslcSetProcessSettingsEnvVariables`.
pub type WslcSetProcessSettingsEnvVariablesFn = unsafe extern "system" fn(
    processSettings: *mut WslcProcessSettings,
    key_value: *const PCSTR,
    argc: usize,
) -> HRESULT;
/// `WslcSetProcessSettingsCallbacks`.
pub type WslcSetProcessSettingsCallbacksFn = unsafe extern "system" fn(
    processSettings: *mut WslcProcessSettings,
    callbacks: *const WslcProcessCallbacks,
    context: PVOID,
) -> HRESULT;
/// `WslcGetProcessPid`.
pub type WslcGetProcessPidFn =
    unsafe extern "system" fn(process: WslcProcess, pid: *mut u32) -> HRESULT;
/// `WslcGetProcessExitEvent`.
pub type WslcGetProcessExitEventFn =
    unsafe extern "system" fn(process: WslcProcess, exitEvent: *mut HANDLE) -> HRESULT;
/// `WslcGetProcessState`.
pub type WslcGetProcessStateFn =
    unsafe extern "system" fn(process: WslcProcess, state: *mut WslcProcessState) -> HRESULT;
/// `WslcGetProcessExitCode`.
pub type WslcGetProcessExitCodeFn =
    unsafe extern "system" fn(process: WslcProcess, exitCode: *mut i32) -> HRESULT;
/// `WslcSignalProcess`.
pub type WslcSignalProcessFn =
    unsafe extern "system" fn(process: WslcProcess, signal: WslcSignal) -> HRESULT;
/// `WslcGetProcessIOHandle`.
pub type WslcGetProcessIOHandleFn = unsafe extern "system" fn(
    process: WslcProcess,
    ioHandle: WslcProcessIOHandle,
    handle: *mut HANDLE,
) -> HRESULT;
/// `WslcReleaseProcess`.
pub type WslcReleaseProcessFn = unsafe extern "system" fn(process: WslcProcess) -> HRESULT;

/// `WslcPullSessionImage`.
pub type WslcPullSessionImageFn = unsafe extern "system" fn(
    session: WslcSession,
    options: *const WslcPullImageOptions,
    errorMessage: *mut PWSTR,
) -> HRESULT;
/// `WslcImportSessionImage`.
pub type WslcImportSessionImageFn = unsafe extern "system" fn(
    session: WslcSession,
    imageName: PCSTR,
    imageContent: HANDLE,
    imageContentBytes: u64,
    options: *const WslcImportImageOptions,
    errorMessage: *mut PWSTR,
) -> HRESULT;
/// `WslcImportSessionImageFromFile`.
pub type WslcImportSessionImageFromFileFn = unsafe extern "system" fn(
    session: WslcSession,
    imageName: PCSTR,
    path: PCWSTR,
    options: *const WslcImportImageOptions,
    errorMessage: *mut PWSTR,
) -> HRESULT;
/// `WslcLoadSessionImage`.
pub type WslcLoadSessionImageFn = unsafe extern "system" fn(
    session: WslcSession,
    imageContent: HANDLE,
    imageContentBytes: u64,
    options: *const WslcLoadImageOptions,
) -> HRESULT;
/// `WslcLoadSessionImageFromFile`.
pub type WslcLoadSessionImageFromFileFn = unsafe extern "system" fn(
    session: WslcSession,
    path: PCWSTR,
    options: *const WslcLoadImageOptions,
) -> HRESULT;
/// `WslcDeleteSessionImage`.
pub type WslcDeleteSessionImageFn = unsafe extern "system" fn(
    session: WslcSession,
    nameOrID: PCSTR,
    errorMessage: *mut PWSTR,
) -> HRESULT;
/// `WslcTagSessionImage`.
pub type WslcTagSessionImageFn = unsafe extern "system" fn(
    session: WslcSession,
    options: *const WslcTagImageOptions,
    errorMessage: *mut PWSTR,
) -> HRESULT;
/// `WslcPushSessionImage`.
pub type WslcPushSessionImageFn = unsafe extern "system" fn(
    session: WslcSession,
    options: *const WslcPushImageOptions,
    errorMessage: *mut PWSTR,
) -> HRESULT;
/// `WslcSessionAuthenticate`.
pub type WslcSessionAuthenticateFn = unsafe extern "system" fn(
    session: WslcSession,
    serverAddress: PCSTR,
    username: PCSTR,
    password: PCSTR,
    identityToken: *mut PSTR,
    errorMessage: *mut PWSTR,
) -> HRESULT;
/// `WslcListSessionImages`.
pub type WslcListSessionImagesFn = unsafe extern "system" fn(
    session: WslcSession,
    images: *mut *mut WslcImageInfo,
    count: *mut u32,
) -> HRESULT;

/// `WslcCreateSessionVhdVolume`.
pub type WslcCreateSessionVhdVolumeFn = unsafe extern "system" fn(
    session: WslcSession,
    options: *const WslcVhdRequirements,
    errorMessage: *mut PWSTR,
) -> HRESULT;
/// `WslcDeleteSessionVhdVolume`.
pub type WslcDeleteSessionVhdVolumeFn = unsafe extern "system" fn(
    session: WslcSession,
    name: PCSTR,
    errorMessage: *mut PWSTR,
) -> HRESULT;

/// `WslcGetMissingComponents`.
pub type WslcGetMissingComponentsFn =
    unsafe extern "system" fn(missingComponents: *mut WslcComponentFlags) -> HRESULT;
/// `WslcGetVersion`.
pub type WslcGetVersionFn = unsafe extern "system" fn(version: *mut WslcVersion) -> HRESULT;
/// `WslcInstallWithDependencies`.
pub type WslcInstallWithDependenciesFn =
    unsafe extern "system" fn(progressCallback: WslcInstallCallback, context: PVOID) -> HRESULT;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaque_settings_layout_matches_header() {
        assert_eq!(
            core::mem::size_of::<WslcSessionSettings>(),
            WSLC_SESSION_OPTIONS_SIZE
        );
        assert_eq!(
            core::mem::align_of::<WslcSessionSettings>(),
            WSLC_SESSION_OPTIONS_ALIGNMENT
        );
        assert_eq!(
            core::mem::size_of::<WslcContainerSettings>(),
            WSLC_CONTAINER_OPTIONS_SIZE
        );
        assert_eq!(
            core::mem::align_of::<WslcContainerSettings>(),
            WSLC_CONTAINER_OPTIONS_ALIGNMENT
        );
        assert_eq!(
            core::mem::size_of::<WslcProcessSettings>(),
            WSLC_CONTAINER_PROCESS_OPTIONS_SIZE
        );
        assert_eq!(
            core::mem::align_of::<WslcProcessSettings>(),
            WSLC_CONTAINER_PROCESS_OPTIONS_ALIGNMENT
        );
    }

    #[test]
    fn image_info_layout_is_stable() {
        assert_eq!(core::mem::size_of::<WslcImageInfo>(), 304);
        assert_eq!(core::mem::align_of::<WslcImageInfo>(), 8);
    }

    #[test]
    fn privileged_flag_matches_the_public_sdk_abi() {
        assert_eq!(WSLC_CONTAINER_FLAG_PRIVILEGED, 0x0000_0004);
    }

    #[test]
    fn logs_flags_match_shared_idl_bits() {
        assert_eq!(WSLC_LOGS_FLAG_NONE, 0);
        assert_eq!(WSLC_LOGS_FLAG_FOLLOW, 1);
        assert_eq!(WSLC_LOGS_FLAG_TIMESTAMPS, 2);
        assert_eq!(WSLC_LOGS_FLAG_DETAILS, 4);
        assert_eq!(
            WSLC_LOGS_FLAGS_VALID,
            WSLC_LOGS_FLAG_FOLLOW | WSLC_LOGS_FLAG_TIMESTAMPS | WSLC_LOGS_FLAG_DETAILS
        );
    }
}
