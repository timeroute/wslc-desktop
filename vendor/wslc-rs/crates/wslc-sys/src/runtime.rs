//! Runtime loading and low-level WSLC SDK call boundary.

#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::ffi::{c_char, c_void, CStr, OsStr};
use std::ptr::NonNull;
use std::sync::{Arc, Condvar, Mutex, OnceLock};

use crate::*;

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryW(lpLibFileName: *const u16) -> *mut c_void;
    fn GetProcAddress(hModule: *mut c_void, lpProcName: *const c_char) -> *mut c_void;
    fn FreeLibrary(hLibModule: *mut c_void) -> i32;
    fn WaitForSingleObject(hHandle: *mut c_void, dwMilliseconds: u32) -> u32;
}

#[cfg(windows)]
#[link(name = "ole32")]
extern "system" {
    fn CoInitializeEx(pvReserved: *mut c_void, dwCoInit: u32) -> HRESULT;
    fn CoUninitialize();
    fn CoTaskMemFree(pv: *mut c_void);
}

/// Result type for runtime loading and SDK calls.
pub type Result<T> = std::result::Result<T, Error>;

/// Error type returned by runtime loading and low-level SDK calls.
#[derive(Debug)]
pub enum Error {
    /// The current platform cannot host WSLC.
    UnsupportedPlatform(&'static str),
    /// `wslcsdk.dll` or one of its exported functions could not be loaded.
    SdkNotFound(String),
    /// COM initialization failed.
    ComInitialization {
        /// The HRESULT returned by COM.
        code: HRESULT,
        /// Human-readable context.
        message: String,
    },
    /// A WSLC SDK call returned a failing HRESULT.
    HResult {
        /// The HRESULT returned by WSLC.
        code: HRESULT,
        /// SDK-provided or synthesized message.
        message: String,
    },
    /// The caller passed data that cannot be represented for the SDK call.
    InvalidInput(String),
    /// A SDK-provided UTF-8 string was invalid.
    Utf8(std::string::FromUtf8Error),
}

/// Progress message emitted while transferring a container image.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageProgress {
    /// Layer ID or digest.
    pub id: String,
    /// Raw SDK progress status.
    pub status: WslcImageProgressStatus,
    /// Downloaded or processed bytes.
    pub current_bytes: u64,
    /// Expected total bytes.
    pub total_bytes: u64,
}

/// Captured process output from SDK callbacks.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CapturedOutput {
    /// Process exit status.
    pub status: i32,
    /// Captured stdout.
    pub stdout: Vec<u8>,
    /// Captured stderr.
    pub stderr: Vec<u8>,
}

#[derive(Default)]
struct CaptureState {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    status: Option<i32>,
}

/// Registration object that owns process callback state.
#[derive(Clone)]
pub struct CaptureRegistration {
    state: Arc<(Mutex<CaptureState>, Condvar)>,
}

impl CaptureRegistration {
    /// Creates a new process callback registration.
    pub fn new() -> Self {
        Self {
            state: Arc::new((Mutex::new(CaptureState::default()), Condvar::new())),
        }
    }

    fn context(&self) -> PVOID {
        Arc::as_ptr(&self.state).cast_mut().cast()
    }

    /// Waits until the process exit callback arrives and returns captured output.
    pub fn wait_output(&self) -> CapturedOutput {
        let (lock, cvar) = &*self.state;
        let mut state = lock.lock().expect("capture lock poisoned");
        while state.status.is_none() {
            state = cvar.wait(state).expect("capture lock poisoned");
        }
        CapturedOutput {
            status: state.status.unwrap_or_default(),
            stdout: state.stdout.clone(),
            stderr: state.stderr.clone(),
        }
    }
}

impl Default for CaptureRegistration {
    fn default() -> Self {
        Self::new()
    }
}

struct Library {
    #[cfg_attr(not(windows), allow(dead_code))]
    handle: NonNull<c_void>,
}

impl Library {
    #[cfg(windows)]
    fn load(name: &OsStr) -> Result<Self> {
        let wide = wide_os(name);
        let handle = unsafe { LoadLibraryW(wide.as_ptr()) };
        let handle = NonNull::new(handle).ok_or_else(|| {
            Error::SdkNotFound(
                "could not load wslcsdk.dll; install Microsoft.WSL.Containers or put the SDK native directory on PATH"
                    .to_owned(),
            )
        })?;
        Ok(Self { handle })
    }

    #[cfg(not(windows))]
    fn load(_name: &OsStr) -> Result<Self> {
        Err(Error::UnsupportedPlatform(
            "WSLC requires a Windows host target",
        ))
    }

    #[cfg(windows)]
    unsafe fn get<T: Copy>(&self, symbol: &'static [u8]) -> Result<T> {
        let ptr = unsafe { GetProcAddress(self.handle.as_ptr(), symbol.as_ptr().cast()) };
        if ptr.is_null() {
            let name = std::str::from_utf8(symbol)
                .unwrap_or("<invalid symbol>")
                .trim_end_matches('\0');
            return Err(Error::SdkNotFound(format!(
                "wslcsdk.dll does not export {name}"
            )));
        }

        Ok(unsafe { std::mem::transmute_copy::<*mut c_void, T>(&ptr) })
    }

    #[cfg(not(windows))]
    unsafe fn get<T: Copy>(&self, _symbol: &'static [u8]) -> Result<T> {
        let _ = self;
        Err(Error::UnsupportedPlatform(
            "WSLC requires a Windows host target",
        ))
    }
}

unsafe impl Send for Library {}
unsafe impl Sync for Library {}

impl Drop for Library {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            let _ = FreeLibrary(self.handle.as_ptr());
        }
    }
}

/// Loaded `wslcsdk.dll` function table with checked call helpers.
#[allow(non_snake_case, dead_code)]
pub struct Sdk {
    _library: Library,
    WslcInitSessionSettings: WslcInitSessionSettingsFn,
    WslcCreateSession: WslcCreateSessionFn,
    WslcSetSessionSettingsCpuCount: WslcSetSessionSettingsCpuCountFn,
    WslcSetSessionSettingsMemory: WslcSetSessionSettingsMemoryFn,
    WslcSetSessionSettingsTimeout: WslcSetSessionSettingsTimeoutFn,
    WslcSetSessionSettingsVhd: WslcSetSessionSettingsVhdFn,
    WslcSetSessionSettingsFeatureFlags: WslcSetSessionSettingsFeatureFlagsFn,
    WslcTerminateSession: WslcTerminateSessionFn,
    WslcReleaseSession: WslcReleaseSessionFn,
    WslcInitContainerSettings: WslcInitContainerSettingsFn,
    WslcCreateContainer: WslcCreateContainerFn,
    WslcOpenContainer: WslcOpenContainerFn,
    WslcStartContainer: WslcStartContainerFn,
    WslcSetContainerSettingsName: WslcSetContainerSettingsNameFn,
    WslcSetContainerSettingsInitProcess: WslcSetContainerSettingsInitProcessFn,
    WslcSetContainerSettingsNetworkingMode: WslcSetContainerSettingsNetworkingModeFn,
    WslcSetContainerSettingsHostName: WslcSetContainerSettingsHostNameFn,
    WslcSetContainerSettingsDomainName: WslcSetContainerSettingsDomainNameFn,
    WslcSetContainerSettingsFlags: WslcSetContainerSettingsFlagsFn,
    WslcSetContainerSettingsPortMappings: WslcSetContainerSettingsPortMappingsFn,
    WslcSetContainerSettingsVolumes: WslcSetContainerSettingsVolumesFn,
    WslcSetContainerSettingsNamedVolumes: WslcSetContainerSettingsNamedVolumesFn,
    WslcCreateContainerProcess: WslcCreateContainerProcessFn,
    WslcReleaseContainer: WslcReleaseContainerFn,
    WslcGetContainerID: WslcGetContainerIDFn,
    WslcGetContainerInitProcess: WslcGetContainerInitProcessFn,
    WslcInspectContainer: WslcInspectContainerFn,
    WslcGetContainerState: WslcGetContainerStateFn,
    WslcStopContainer: WslcStopContainerFn,
    WslcDeleteContainer: WslcDeleteContainerFn,
    WslcInitProcessSettings: WslcInitProcessSettingsFn,
    WslcSetProcessSettingsWorkingDirectory: WslcSetProcessSettingsWorkingDirectoryFn,
    WslcSetProcessSettingsCmdLine: WslcSetProcessSettingsCmdLineFn,
    WslcSetProcessSettingsEnvVariables: WslcSetProcessSettingsEnvVariablesFn,
    WslcSetProcessSettingsCallbacks: WslcSetProcessSettingsCallbacksFn,
    WslcGetProcessPid: WslcGetProcessPidFn,
    WslcGetProcessExitEvent: WslcGetProcessExitEventFn,
    WslcGetProcessState: WslcGetProcessStateFn,
    WslcGetProcessExitCode: WslcGetProcessExitCodeFn,
    WslcSignalProcess: WslcSignalProcessFn,
    WslcReleaseProcess: WslcReleaseProcessFn,
    WslcPullSessionImage: WslcPullSessionImageFn,
    WslcDeleteSessionImage: WslcDeleteSessionImageFn,
    WslcTagSessionImage: WslcTagSessionImageFn,
    WslcListSessionImages: WslcListSessionImagesFn,
    WslcCreateSessionVhdVolume: WslcCreateSessionVhdVolumeFn,
    WslcDeleteSessionVhdVolume: WslcDeleteSessionVhdVolumeFn,
    WslcGetMissingComponents: WslcGetMissingComponentsFn,
    WslcGetVersion: WslcGetVersionFn,
    WslcInstallWithDependencies: WslcInstallWithDependenciesFn,
}

impl Sdk {
    fn load() -> Result<Self> {
        let dll_path = std::env::var_os("WSLC_SDK_PATH")
            .unwrap_or_else(|| std::ffi::OsString::from("wslcsdk.dll"));
        let library = Library::load(OsStr::new(&dll_path))?;
        macro_rules! sym {
            ($field:ident) => {
                unsafe { library.get(concat!(stringify!($field), "\0").as_bytes())? }
            };
        }

        Ok(Self {
            WslcInitSessionSettings: sym!(WslcInitSessionSettings),
            WslcCreateSession: sym!(WslcCreateSession),
            WslcSetSessionSettingsCpuCount: sym!(WslcSetSessionSettingsCpuCount),
            WslcSetSessionSettingsMemory: sym!(WslcSetSessionSettingsMemory),
            WslcSetSessionSettingsTimeout: sym!(WslcSetSessionSettingsTimeout),
            WslcSetSessionSettingsVhd: sym!(WslcSetSessionSettingsVhd),
            WslcSetSessionSettingsFeatureFlags: sym!(WslcSetSessionSettingsFeatureFlags),
            WslcTerminateSession: sym!(WslcTerminateSession),
            WslcReleaseSession: sym!(WslcReleaseSession),
            WslcInitContainerSettings: sym!(WslcInitContainerSettings),
            WslcCreateContainer: sym!(WslcCreateContainer),
            WslcOpenContainer: sym!(WslcOpenContainer),
            WslcStartContainer: sym!(WslcStartContainer),
            WslcSetContainerSettingsName: sym!(WslcSetContainerSettingsName),
            WslcSetContainerSettingsInitProcess: sym!(WslcSetContainerSettingsInitProcess),
            WslcSetContainerSettingsNetworkingMode: sym!(WslcSetContainerSettingsNetworkingMode),
            WslcSetContainerSettingsHostName: sym!(WslcSetContainerSettingsHostName),
            WslcSetContainerSettingsDomainName: sym!(WslcSetContainerSettingsDomainName),
            WslcSetContainerSettingsFlags: sym!(WslcSetContainerSettingsFlags),
            WslcSetContainerSettingsPortMappings: sym!(WslcSetContainerSettingsPortMappings),
            WslcSetContainerSettingsVolumes: sym!(WslcSetContainerSettingsVolumes),
            WslcSetContainerSettingsNamedVolumes: sym!(WslcSetContainerSettingsNamedVolumes),
            WslcCreateContainerProcess: sym!(WslcCreateContainerProcess),
            WslcReleaseContainer: sym!(WslcReleaseContainer),
            WslcGetContainerID: sym!(WslcGetContainerID),
            WslcGetContainerInitProcess: sym!(WslcGetContainerInitProcess),
            WslcInspectContainer: sym!(WslcInspectContainer),
            WslcGetContainerState: sym!(WslcGetContainerState),
            WslcStopContainer: sym!(WslcStopContainer),
            WslcDeleteContainer: sym!(WslcDeleteContainer),
            WslcInitProcessSettings: sym!(WslcInitProcessSettings),
            WslcSetProcessSettingsWorkingDirectory: sym!(WslcSetProcessSettingsWorkingDirectory),
            WslcSetProcessSettingsCmdLine: sym!(WslcSetProcessSettingsCmdLine),
            WslcSetProcessSettingsEnvVariables: sym!(WslcSetProcessSettingsEnvVariables),
            WslcSetProcessSettingsCallbacks: sym!(WslcSetProcessSettingsCallbacks),
            WslcGetProcessPid: sym!(WslcGetProcessPid),
            WslcGetProcessExitEvent: sym!(WslcGetProcessExitEvent),
            WslcGetProcessState: sym!(WslcGetProcessState),
            WslcGetProcessExitCode: sym!(WslcGetProcessExitCode),
            WslcSignalProcess: sym!(WslcSignalProcess),
            WslcReleaseProcess: sym!(WslcReleaseProcess),
            WslcPullSessionImage: sym!(WslcPullSessionImage),
            WslcDeleteSessionImage: sym!(WslcDeleteSessionImage),
            WslcTagSessionImage: sym!(WslcTagSessionImage),
            WslcListSessionImages: sym!(WslcListSessionImages),
            WslcCreateSessionVhdVolume: sym!(WslcCreateSessionVhdVolume),
            WslcDeleteSessionVhdVolume: sym!(WslcDeleteSessionVhdVolume),
            WslcGetMissingComponents: sym!(WslcGetMissingComponents),
            WslcGetVersion: sym!(WslcGetVersion),
            WslcInstallWithDependencies: sym!(WslcInstallWithDependencies),
            _library: library,
        })
    }

    /// Returns missing WSLC component flags.
    pub fn missing_components(&self) -> Result<WslcComponentFlags> {
        let mut flags = 0;
        let hr = unsafe { (self.WslcGetMissingComponents)(&mut flags) };
        check_hr(hr, std::ptr::null_mut())?;
        Ok(flags)
    }

    /// Returns the installed WSLC runtime version.
    pub fn version(&self) -> Result<WslcVersion> {
        let mut version = WslcVersion::default();
        let hr = unsafe { (self.WslcGetVersion)(&mut version) };
        check_hr(hr, std::ptr::null_mut())?;
        Ok(version)
    }

    /// Installs WSLC dependencies and reports raw progress values.
    pub fn install_with_dependencies<F>(&self, progress: &mut F) -> Result<()>
    where
        F: FnMut(WslcComponentFlags, u32, u32),
    {
        let context = progress as *mut F as PVOID;
        let hr =
            unsafe { (self.WslcInstallWithDependencies)(Some(install_trampoline::<F>), context) };
        check_hr(hr, std::ptr::null_mut())
    }

    /// Initializes session settings.
    pub fn init_session_settings(
        &self,
        name: PCWSTR,
        storage_path: PCWSTR,
        settings: &mut WslcSessionSettings,
    ) -> Result<()> {
        let hr = unsafe { (self.WslcInitSessionSettings)(name, storage_path, settings) };
        check_hr(hr, std::ptr::null_mut())
    }

    /// Sets session CPU count.
    pub fn set_session_cpu_count(
        &self,
        settings: &mut WslcSessionSettings,
        cpu_count: u32,
    ) -> Result<()> {
        let hr = unsafe { (self.WslcSetSessionSettingsCpuCount)(settings, cpu_count) };
        check_hr(hr, std::ptr::null_mut())
    }

    /// Sets session memory in MB.
    pub fn set_session_memory(
        &self,
        settings: &mut WslcSessionSettings,
        memory_mb: u32,
    ) -> Result<()> {
        let hr = unsafe { (self.WslcSetSessionSettingsMemory)(settings, memory_mb) };
        check_hr(hr, std::ptr::null_mut())
    }

    /// Sets session creation timeout in milliseconds.
    pub fn set_session_timeout(
        &self,
        settings: &mut WslcSessionSettings,
        timeout_ms: u32,
    ) -> Result<()> {
        let hr = unsafe { (self.WslcSetSessionSettingsTimeout)(settings, timeout_ms) };
        check_hr(hr, std::ptr::null_mut())
    }

    /// Sets session feature flags.
    pub fn set_session_feature_flags(
        &self,
        settings: &mut WslcSessionSettings,
        flags: WslcSessionFeatureFlags,
    ) -> Result<()> {
        let hr = unsafe { (self.WslcSetSessionSettingsFeatureFlags)(settings, flags) };
        check_hr(hr, std::ptr::null_mut())
    }

    /// Sets session VHD requirements.
    pub fn set_session_vhd(
        &self,
        settings: &mut WslcSessionSettings,
        vhd: &WslcVhdRequirements,
    ) -> Result<()> {
        let hr = unsafe { (self.WslcSetSessionSettingsVhd)(settings, vhd) };
        check_hr(hr, std::ptr::null_mut())
    }

    /// Creates a session handle.
    pub fn create_session(&self, settings: &mut WslcSessionSettings) -> Result<WslcSession> {
        let mut session = std::ptr::null_mut();
        let mut error_message = std::ptr::null_mut();
        let hr = unsafe { (self.WslcCreateSession)(settings, &mut session, &mut error_message) };
        check_hr(hr, error_message)?;
        Ok(session)
    }

    /// Terminates a session handle.
    pub fn terminate_session(&self, session: WslcSession) -> Result<()> {
        let hr = unsafe { (self.WslcTerminateSession)(session) };
        check_hr(hr, std::ptr::null_mut())
    }

    /// Releases a session handle.
    pub fn release_session(&self, session: WslcSession) -> Result<()> {
        let hr = unsafe { (self.WslcReleaseSession)(session) };
        check_hr(hr, std::ptr::null_mut())
    }

    /// Lists images in a session.
    pub fn list_session_images(&self, session: WslcSession) -> Result<Vec<WslcImageInfo>> {
        let mut raw_images = std::ptr::null_mut();
        let mut count = 0;
        let hr = unsafe { (self.WslcListSessionImages)(session, &mut raw_images, &mut count) };
        check_hr(hr, std::ptr::null_mut())?;
        if raw_images.is_null() || count == 0 {
            return Ok(Vec::new());
        }

        let images = unsafe { std::slice::from_raw_parts(raw_images, count as usize) }.to_vec();
        free_cotaskmem(raw_images.cast());
        Ok(images)
    }

    /// Deletes an image from a session.
    pub fn delete_session_image(&self, session: WslcSession, name_or_id: PCSTR) -> Result<()> {
        let mut error_message = std::ptr::null_mut();
        let hr = unsafe { (self.WslcDeleteSessionImage)(session, name_or_id, &mut error_message) };
        check_hr(hr, error_message)
    }

    /// Tags an image in a session.
    pub fn tag_session_image(
        &self,
        session: WslcSession,
        options: &WslcTagImageOptions,
    ) -> Result<()> {
        let mut error_message = std::ptr::null_mut();
        let hr = unsafe { (self.WslcTagSessionImage)(session, options, &mut error_message) };
        check_hr(hr, error_message)
    }

    /// Creates a VHD-backed session volume.
    pub fn create_session_vhd_volume(
        &self,
        session: WslcSession,
        options: &WslcVhdRequirements,
    ) -> Result<()> {
        let mut error_message = std::ptr::null_mut();
        let hr = unsafe { (self.WslcCreateSessionVhdVolume)(session, options, &mut error_message) };
        check_hr(hr, error_message)
    }

    /// Deletes a VHD-backed session volume.
    pub fn delete_session_vhd_volume(&self, session: WslcSession, name: PCSTR) -> Result<()> {
        let mut error_message = std::ptr::null_mut();
        let hr = unsafe { (self.WslcDeleteSessionVhdVolume)(session, name, &mut error_message) };
        check_hr(hr, error_message)
    }

    /// Pulls a session image without progress callbacks.
    pub fn pull_session_image(
        &self,
        session: WslcSession,
        uri: PCSTR,
        registry_auth: PCSTR,
    ) -> Result<()> {
        let options = WslcPullImageOptions {
            uri,
            progressCallback: None,
            progressCallbackContext: std::ptr::null_mut(),
            registryAuth: registry_auth,
        };
        let mut error_message = std::ptr::null_mut();
        let hr = unsafe { (self.WslcPullSessionImage)(session, &options, &mut error_message) };
        check_hr(hr, error_message)
    }

    /// Pulls a session image and reports progress callbacks.
    pub fn pull_session_image_with_progress<F>(
        &self,
        session: WslcSession,
        uri: PCSTR,
        registry_auth: PCSTR,
        progress: &mut F,
    ) -> Result<()>
    where
        F: FnMut(ImageProgress),
    {
        let context = progress as *mut F as PVOID;
        let options = WslcPullImageOptions {
            uri,
            progressCallback: Some(progress_trampoline::<F>),
            progressCallbackContext: context,
            registryAuth: registry_auth,
        };
        let mut error_message = std::ptr::null_mut();
        let hr = unsafe { (self.WslcPullSessionImage)(session, &options, &mut error_message) };
        check_hr(hr, error_message)
    }

    /// Initializes container settings.
    pub fn init_container_settings(
        &self,
        image: PCSTR,
        settings: &mut WslcContainerSettings,
    ) -> Result<()> {
        let hr = unsafe { (self.WslcInitContainerSettings)(image, settings) };
        check_hr(hr, std::ptr::null_mut())
    }

    /// Sets the container name.
    pub fn set_container_name(
        &self,
        settings: &mut WslcContainerSettings,
        name: PCSTR,
    ) -> Result<()> {
        let hr = unsafe { (self.WslcSetContainerSettingsName)(settings, name) };
        check_hr(hr, std::ptr::null_mut())
    }

    /// Sets the container host name.
    pub fn set_container_host_name(
        &self,
        settings: &mut WslcContainerSettings,
        host_name: PCSTR,
    ) -> Result<()> {
        let hr = unsafe { (self.WslcSetContainerSettingsHostName)(settings, host_name) };
        check_hr(hr, std::ptr::null_mut())
    }

    /// Sets the container domain name.
    pub fn set_container_domain_name(
        &self,
        settings: &mut WslcContainerSettings,
        domain_name: PCSTR,
    ) -> Result<()> {
        let hr = unsafe { (self.WslcSetContainerSettingsDomainName)(settings, domain_name) };
        check_hr(hr, std::ptr::null_mut())
    }

    /// Sets the container networking mode.
    pub fn set_container_networking_mode(
        &self,
        settings: &mut WslcContainerSettings,
        mode: WslcContainerNetworkingMode,
    ) -> Result<()> {
        let hr = unsafe { (self.WslcSetContainerSettingsNetworkingMode)(settings, mode) };
        check_hr(hr, std::ptr::null_mut())
    }

    /// Sets container flags.
    pub fn set_container_flags(
        &self,
        settings: &mut WslcContainerSettings,
        flags: WslcContainerFlags,
    ) -> Result<()> {
        let hr = unsafe { (self.WslcSetContainerSettingsFlags)(settings, flags) };
        check_hr(hr, std::ptr::null_mut())
    }

    /// Sets container init process settings.
    pub fn set_container_init_process(
        &self,
        settings: &mut WslcContainerSettings,
        process: &mut WslcProcessSettings,
    ) -> Result<()> {
        let hr = unsafe { (self.WslcSetContainerSettingsInitProcess)(settings, process) };
        check_hr(hr, std::ptr::null_mut())
    }

    /// Sets container port mappings.
    pub fn set_container_port_mappings(
        &self,
        settings: &mut WslcContainerSettings,
        ports: &[WslcContainerPortMapping],
    ) -> Result<()> {
        let hr = unsafe {
            (self.WslcSetContainerSettingsPortMappings)(
                settings,
                ports.as_ptr(),
                ports.len() as u32,
            )
        };
        check_hr(hr, std::ptr::null_mut())
    }

    /// Sets container bind mount volumes.
    pub fn set_container_volumes(
        &self,
        settings: &mut WslcContainerSettings,
        volumes: &[WslcContainerVolume],
    ) -> Result<()> {
        let hr = unsafe {
            (self.WslcSetContainerSettingsVolumes)(settings, volumes.as_ptr(), volumes.len() as u32)
        };
        check_hr(hr, std::ptr::null_mut())
    }

    /// Sets container named (session VHD) volume mounts.
    pub fn set_container_named_volumes(
        &self,
        settings: &mut WslcContainerSettings,
        volumes: &[WslcContainerNamedVolume],
    ) -> Result<()> {
        let hr = unsafe {
            (self.WslcSetContainerSettingsNamedVolumes)(
                settings,
                volumes.as_ptr(),
                volumes.len() as u32,
            )
        };
        check_hr(hr, std::ptr::null_mut())
    }

    /// Creates a container handle.
    pub fn create_container(
        &self,
        session: WslcSession,
        settings: &WslcContainerSettings,
    ) -> Result<WslcContainer> {
        let mut container = std::ptr::null_mut();
        let mut error_message = std::ptr::null_mut();
        let hr = unsafe {
            (self.WslcCreateContainer)(session, settings, &mut container, &mut error_message)
        };
        check_hr(hr, error_message)?;
        Ok(container)
    }

    /// Opens an existing container by name, full ID, or unambiguous ID prefix.
    pub fn open_container(&self, session: WslcSession, name_or_id: PCSTR) -> Result<WslcContainer> {
        let mut container = std::ptr::null_mut();
        let mut error_message = std::ptr::null_mut();
        let hr = unsafe {
            (self.WslcOpenContainer)(session, name_or_id, &mut container, &mut error_message)
        };
        check_hr(hr, error_message)?;
        Ok(container)
    }

    /// Releases a container handle.
    pub fn release_container(&self, container: WslcContainer) -> Result<()> {
        let hr = unsafe { (self.WslcReleaseContainer)(container) };
        check_hr(hr, std::ptr::null_mut())
    }

    /// Returns the container ID string.
    pub fn container_id(&self, container: WslcContainer) -> Result<String> {
        let mut id = [0i8; WSLC_CONTAINER_ID_BUFFER_SIZE];
        let hr = unsafe { (self.WslcGetContainerID)(container, id.as_mut_ptr()) };
        check_hr(hr, std::ptr::null_mut())?;
        c_ptr_to_string(id.as_ptr())
    }

    /// Starts a container.
    pub fn start_container(
        &self,
        container: WslcContainer,
        flags: WslcContainerStartFlags,
    ) -> Result<()> {
        let mut error_message = std::ptr::null_mut();
        let hr = unsafe { (self.WslcStartContainer)(container, flags, &mut error_message) };
        check_hr(hr, error_message)
    }

    /// Returns raw container inspect JSON.
    pub fn inspect_container(&self, container: WslcContainer) -> Result<String> {
        let mut inspect = std::ptr::null_mut();
        let hr = unsafe { (self.WslcInspectContainer)(container, &mut inspect) };
        check_hr(hr, std::ptr::null_mut())?;
        let value = c_ptr_to_string(inspect)?;
        free_cotaskmem(inspect.cast());
        Ok(value)
    }

    /// Returns the container state.
    pub fn container_state(&self, container: WslcContainer) -> Result<WslcContainerState> {
        let mut state = WslcContainerState::WSLC_CONTAINER_STATE_INVALID;
        let hr = unsafe { (self.WslcGetContainerState)(container, &mut state) };
        check_hr(hr, std::ptr::null_mut())?;
        Ok(state)
    }

    /// Creates a process in a container.
    pub fn create_container_process(
        &self,
        container: WslcContainer,
        settings: &mut WslcProcessSettings,
    ) -> Result<WslcProcess> {
        let mut process = std::ptr::null_mut();
        let mut error_message = std::ptr::null_mut();
        let hr = unsafe {
            (self.WslcCreateContainerProcess)(container, settings, &mut process, &mut error_message)
        };
        check_hr(hr, error_message)?;
        Ok(process)
    }

    /// Stops a container.
    pub fn stop_container(
        &self,
        container: WslcContainer,
        signal: WslcSignal,
        timeout_seconds: u32,
    ) -> Result<()> {
        let mut error_message = std::ptr::null_mut();
        let hr = unsafe {
            (self.WslcStopContainer)(container, signal, timeout_seconds, &mut error_message)
        };
        check_hr(hr, error_message)
    }

    /// Deletes a container.
    pub fn delete_container(
        &self,
        container: WslcContainer,
        flags: WslcDeleteContainerFlags,
    ) -> Result<()> {
        let mut error_message = std::ptr::null_mut();
        let hr = unsafe { (self.WslcDeleteContainer)(container, flags, &mut error_message) };
        check_hr(hr, error_message)
    }

    /// Initializes process settings.
    pub fn init_process_settings(&self, settings: &mut WslcProcessSettings) -> Result<()> {
        let hr = unsafe { (self.WslcInitProcessSettings)(settings) };
        check_hr(hr, std::ptr::null_mut())
    }

    /// Sets process command line.
    pub fn set_process_cmdline(
        &self,
        settings: &mut WslcProcessSettings,
        argv: &[PCSTR],
    ) -> Result<()> {
        let hr = unsafe {
            (self.WslcSetProcessSettingsCmdLine)(settings, argv.as_ptr().cast(), argv.len())
        };
        check_hr(hr, std::ptr::null_mut())
    }

    /// Sets process working directory.
    pub fn set_process_working_directory(
        &self,
        settings: &mut WslcProcessSettings,
        working_dir: PCSTR,
    ) -> Result<()> {
        let hr = unsafe { (self.WslcSetProcessSettingsWorkingDirectory)(settings, working_dir) };
        check_hr(hr, std::ptr::null_mut())
    }

    /// Sets process environment variables.
    pub fn set_process_env_variables(
        &self,
        settings: &mut WslcProcessSettings,
        env: &[PCSTR],
    ) -> Result<()> {
        let hr = unsafe {
            (self.WslcSetProcessSettingsEnvVariables)(settings, env.as_ptr().cast(), env.len())
        };
        check_hr(hr, std::ptr::null_mut())
    }

    /// Sets process output callbacks using the provided capture registration.
    pub fn set_process_capture_callbacks(
        &self,
        settings: &mut WslcProcessSettings,
        capture: &CaptureRegistration,
    ) -> Result<()> {
        let callbacks = WslcProcessCallbacks {
            onStdOut: Some(stdio_trampoline),
            onStdErr: Some(stdio_trampoline),
            onExit: Some(exit_trampoline),
        };
        let hr = unsafe {
            (self.WslcSetProcessSettingsCallbacks)(settings, &callbacks, capture.context())
        };
        check_hr(hr, std::ptr::null_mut())
    }

    /// Returns process PID.
    pub fn process_pid(&self, process: WslcProcess) -> Result<u32> {
        let mut pid = 0;
        let hr = unsafe { (self.WslcGetProcessPid)(process, &mut pid) };
        check_hr(hr, std::ptr::null_mut())?;
        Ok(pid)
    }

    /// Returns process state.
    pub fn process_state(&self, process: WslcProcess) -> Result<WslcProcessState> {
        let mut state = WslcProcessState::WSLC_PROCESS_STATE_UNKNOWN;
        let hr = unsafe { (self.WslcGetProcessState)(process, &mut state) };
        check_hr(hr, std::ptr::null_mut())?;
        Ok(state)
    }

    /// Sends a signal to a process.
    pub fn signal_process(&self, process: WslcProcess, signal: WslcSignal) -> Result<()> {
        let hr = unsafe { (self.WslcSignalProcess)(process, signal) };
        check_hr(hr, std::ptr::null_mut())
    }

    /// Returns process exit code.
    pub fn process_exit_code(&self, process: WslcProcess) -> Result<i32> {
        let mut exit_code = 0;
        let hr = unsafe { (self.WslcGetProcessExitCode)(process, &mut exit_code) };
        check_hr(hr, std::ptr::null_mut())?;
        Ok(exit_code)
    }

    /// Returns process exit event handle.
    pub fn process_exit_event(&self, process: WslcProcess) -> Result<HANDLE> {
        let mut event = std::ptr::null_mut();
        let hr = unsafe { (self.WslcGetProcessExitEvent)(process, &mut event) };
        check_hr(hr, std::ptr::null_mut())?;
        Ok(event)
    }

    /// Releases a process handle.
    pub fn release_process(&self, process: WslcProcess) -> Result<()> {
        let hr = unsafe { (self.WslcReleaseProcess)(process) };
        check_hr(hr, std::ptr::null_mut())
    }
}

/// Returns the process-global loaded SDK function table.
///
/// Successful loads are cached. Failures are **not** — Retry after fixing /
/// redownloading the DLL must be able to call `Sdk::load` again.
pub fn sdk() -> Result<&'static Sdk> {
    static SDK: OnceLock<Sdk> = OnceLock::new();
    if let Some(sdk) = SDK.get() {
        return Ok(sdk);
    }
    static LOAD: Mutex<()> = Mutex::new(());
    let _guard = LOAD.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(sdk) = SDK.get() {
        return Ok(sdk);
    }
    let loaded = Sdk::load()?;
    Ok(SDK.get_or_init(|| loaded))
}

/// Initializes COM on the current thread.
pub fn co_initialize_ex(reserved: *mut c_void, coinit: u32) -> HRESULT {
    #[cfg(windows)]
    unsafe {
        CoInitializeEx(reserved, coinit)
    }

    #[cfg(not(windows))]
    {
        let _ = (reserved, coinit);
        0x8000_4001_u32 as i32
    }
}

/// Uninitializes COM on the current thread.
pub fn co_uninitialize() {
    #[cfg(windows)]
    unsafe {
        CoUninitialize();
    }
}

/// Waits for a Windows kernel object.
pub fn wait_for_single_object(handle: HANDLE, timeout_ms: u32) -> u32 {
    #[cfg(windows)]
    unsafe {
        WaitForSingleObject(handle, timeout_ms)
    }

    #[cfg(not(windows))]
    {
        let _ = (handle, timeout_ms);
        0xffff_ffff
    }
}

/// Converts a SDK-owned C string pointer into an owned Rust string.
pub fn c_ptr_to_string(ptr: *const c_char) -> Result<String> {
    if ptr.is_null() {
        return Ok(String::new());
    }

    let bytes = unsafe { CStr::from_ptr(ptr) }.to_bytes().to_vec();
    String::from_utf8(bytes).map_err(Error::Utf8)
}

/// Converts an image info name buffer into an owned Rust string.
pub fn image_name_to_string(name: &[c_char; WSLC_IMAGE_NAME_LENGTH]) -> Result<String> {
    c_ptr_to_string(name.as_ptr())
}

fn check_hr(hr: HRESULT, error_message: PWSTR) -> Result<()> {
    if succeeded(hr) {
        free_cotaskmem(error_message.cast());
        return Ok(());
    }

    let message = utf16_ptr_to_string(error_message);
    free_cotaskmem(error_message.cast());
    Err(Error::HResult { code: hr, message })
}

fn succeeded(hr: HRESULT) -> bool {
    hr >= 0
}

fn free_cotaskmem(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }

    #[cfg(windows)]
    unsafe {
        CoTaskMemFree(ptr);
    }

    #[cfg(not(windows))]
    {
        let _ = ptr;
    }
}

fn utf16_ptr_to_string(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }

    let mut len = 0usize;
    while unsafe { *ptr.add(len) } != 0 {
        len += 1;
    }

    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    String::from_utf16_lossy(slice)
}

#[cfg(windows)]
fn wide_os(value: &OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    value.encode_wide().chain(std::iter::once(0)).collect()
}

unsafe extern "system" fn install_trampoline<F>(
    component: WslcComponentFlags,
    progress_steps: u32,
    total_steps: u32,
    context: PVOID,
) where
    F: FnMut(WslcComponentFlags, u32, u32),
{
    if context.is_null() {
        return;
    }

    let callback = unsafe { &mut *(context as *mut F) };
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        callback(component, progress_steps, total_steps);
    }));
}

unsafe extern "system" fn progress_trampoline<F>(
    progress: *const WslcImageProgressMessage,
    context: PVOID,
) -> HRESULT
where
    F: FnMut(ImageProgress),
{
    if progress.is_null() || context.is_null() {
        return S_OK;
    }

    let progress = unsafe { &*progress };
    let event = ImageProgress {
        id: c_ptr_to_string(progress.id).unwrap_or_default(),
        status: progress.status,
        current_bytes: progress.detail.currentBytes,
        total_bytes: progress.detail.totalBytes,
    };
    let callback = unsafe { &mut *(context as *mut F) };
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(event)));
    S_OK
}

unsafe extern "system" fn stdio_trampoline(
    io_handle: WslcProcessIOHandle,
    data: *const u8,
    data_bytes: u32,
    context: PVOID,
) {
    if context.is_null() || data.is_null() {
        return;
    }
    let state = unsafe { &*(context as *const (Mutex<CaptureState>, Condvar)) };
    let bytes = unsafe { std::slice::from_raw_parts(data, data_bytes as usize) };
    if let Ok(mut capture) = state.0.lock() {
        match io_handle {
            WslcProcessIOHandle::WSLC_PROCESS_IO_HANDLE_STDOUT => {
                capture.stdout.extend_from_slice(bytes)
            }
            WslcProcessIOHandle::WSLC_PROCESS_IO_HANDLE_STDERR => {
                capture.stderr.extend_from_slice(bytes)
            }
            _ => {}
        }
    }
}

unsafe extern "system" fn exit_trampoline(exit_code: i32, context: PVOID) {
    if context.is_null() {
        return;
    }
    let state = unsafe { &*(context as *const (Mutex<CaptureState>, Condvar)) };
    if let Ok(mut capture) = state.0.lock() {
        capture.status = Some(exit_code);
        state.1.notify_all();
    }
}
