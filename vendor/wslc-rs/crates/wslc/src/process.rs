use crate::{raw, strings, Error, Result};
use std::ffi::CString;
use std::rc::Rc;

/// Output capture mode for a process.
#[derive(Default)]
pub enum OutputMode {
    /// Discard process output.
    #[default]
    Null,
    /// Capture stdout and stderr.
    Capture,
    /// Stream stdout and stderr to callbacks.
    Streaming {
        /// Stdout callback.
        stdout: Option<OutputCallback>,
        /// Stderr callback.
        stderr: Option<OutputCallback>,
    },
}

type OutputCallback = Box<dyn FnMut(&[u8]) + Send>;

impl std::fmt::Debug for OutputMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Null => f.write_str("Null"),
            Self::Capture => f.write_str("Capture"),
            Self::Streaming { .. } => f.write_str("Streaming"),
        }
    }
}

/// Options for a container process.
#[derive(Debug)]
pub struct ProcessOptions {
    /// Command and arguments.
    pub cmdline: Vec<String>,
    /// Working directory inside the container.
    pub working_dir: Option<String>,
    /// Environment variables.
    pub env: Vec<(String, String)>,
    /// Output mode.
    pub output_mode: OutputMode,
}

impl ProcessOptions {
    /// Creates process options from command arguments.
    pub fn new<I, S>(args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            cmdline: args.into_iter().map(Into::into).collect(),
            working_dir: None,
            env: Vec::new(),
            output_mode: OutputMode::Null,
        }
    }

    /// Sets the working directory.
    pub fn working_dir(mut self, dir: impl Into<String>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    /// Adds an environment variable.
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// Captures stdout and stderr.
    pub fn capture_stdout(mut self) -> Self {
        self.output_mode = OutputMode::Capture;
        self
    }

    /// Captures stdout and stderr.
    pub fn capture_stderr(mut self) -> Self {
        self.output_mode = OutputMode::Capture;
        self
    }

    /// Inherits output behavior from the SDK/runtime.
    pub fn inherit_output(mut self) -> Self {
        self.output_mode = OutputMode::Null;
        self
    }

    /// Enables streaming mode without callbacks yet.
    pub fn streaming(mut self) -> Self {
        self.output_mode = OutputMode::Streaming {
            stdout: None,
            stderr: None,
        };
        self
    }

    pub(crate) fn captures_output(&self) -> bool {
        matches!(
            self.output_mode,
            OutputMode::Capture | OutputMode::Streaming { .. }
        )
    }

    /// Validates process options.
    pub fn validate(&self) -> Result<()> {
        if self.cmdline.is_empty() {
            return Err(Error::InvalidInput(
                "process command cannot be empty".to_owned(),
            ));
        }
        if self.cmdline.iter().any(|arg| arg.is_empty()) {
            return Err(Error::InvalidInput(
                "process command arguments cannot be empty".to_owned(),
            ));
        }
        if self.cmdline.iter().any(|arg| arg.contains('\0')) {
            return Err(Error::Nul("process arg".to_owned()));
        }
        if let Some(dir) = &self.working_dir {
            if dir.is_empty() {
                return Err(Error::InvalidInput(
                    "working directory cannot be empty".to_owned(),
                ));
            }
            if dir.contains('\0') {
                return Err(Error::Nul("working_dir".to_owned()));
            }
        }
        for (key, value) in &self.env {
            if key.is_empty() || key.contains('=') {
                return Err(Error::InvalidInput(
                    "env variable key cannot be empty or contain '='".to_owned(),
                ));
            }
            if key.contains('\0') || value.contains('\0') {
                return Err(Error::Nul("env".to_owned()));
            }
        }
        Ok(())
    }

    pub(crate) fn to_raw(
        &self,
        sdk: &raw::Sdk,
        capture: Option<&CaptureRegistration>,
    ) -> Result<RawProcessSettings> {
        self.validate()?;
        let mut settings = wslc_sys::WslcProcessSettings::default();
        raw::map_result(sdk.init_process_settings(&mut settings))?;

        let argv = RetainedCStringArray::new(strings::cstrings(
            self.cmdline.iter().map(String::as_str),
            "process arg",
        )?);
        raw::map_result(sdk.set_process_cmdline(&mut settings, argv.pointers()))?;

        let working_dir = self
            .working_dir
            .as_deref()
            .map(|dir| strings::cstring(dir, "working_dir"))
            .transpose()?;
        if let Some(working_dir) = &working_dir {
            raw::map_result(
                sdk.set_process_working_directory(&mut settings, working_dir.as_ptr()),
            )?;
        }

        let env_values: Vec<String> = self
            .env
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect();
        let env = RetainedCStringArray::new(strings::cstrings(
            env_values.iter().map(String::as_str),
            "env",
        )?);
        if !env.pointers().is_empty() {
            raw::map_result(sdk.set_process_env_variables(&mut settings, env.pointers()))?;
        }

        if let Some(capture) = capture {
            raw::map_result(sdk.set_process_capture_callbacks(&mut settings, capture))?;
        }

        Ok(RawProcessSettings {
            settings,
            _argv: argv,
            _working_dir: working_dir,
            _env: env,
        })
    }
}

struct RetainedCStringArray {
    _values: Vec<CString>,
    pointers: Vec<wslc_sys::PCSTR>,
}

impl RetainedCStringArray {
    fn new(values: Vec<CString>) -> Self {
        let pointers = values.iter().map(|value| value.as_ptr()).collect();
        Self {
            _values: values,
            pointers,
        }
    }

    fn pointers(&self) -> &[wslc_sys::PCSTR] {
        &self.pointers
    }
}

pub(crate) struct RawProcessSettings {
    pub settings: wslc_sys::WslcProcessSettings,
    _argv: RetainedCStringArray,
    _working_dir: Option<CString>,
    _env: RetainedCStringArray,
}

/// Process output.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Output {
    /// Exit status.
    pub status: i32,
    /// Captured stdout.
    pub stdout: Vec<u8>,
    /// Captured stderr.
    pub stderr: Vec<u8>,
}

/// Process state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessState {
    /// Unknown.
    Unknown,
    /// Running.
    Running,
    /// Exited.
    Exited,
    /// Signalled.
    Signalled,
}

impl From<wslc_sys::WslcProcessState> for ProcessState {
    fn from(value: wslc_sys::WslcProcessState) -> Self {
        match value {
            wslc_sys::WslcProcessState::WSLC_PROCESS_STATE_RUNNING => Self::Running,
            wslc_sys::WslcProcessState::WSLC_PROCESS_STATE_EXITED => Self::Exited,
            wslc_sys::WslcProcessState::WSLC_PROCESS_STATE_SIGNALLED => Self::Signalled,
            _ => Self::Unknown,
        }
    }
}

pub(crate) struct ProcessInner {
    pub raw: wslc_sys::WslcProcess,
    pub capture: Option<Rc<CaptureRegistration>>,
}

/// A process running inside a WSLC container.
pub struct Process {
    pub(crate) inner: Rc<ProcessInner>,
}

impl std::fmt::Debug for Process {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Process").finish_non_exhaustive()
    }
}

impl Process {
    /// Returns the process ID.
    pub fn pid(&self) -> Result<u32> {
        let sdk = raw::sdk()?;
        raw::map_result(sdk.process_pid(self.inner.raw))
    }

    /// Returns the process state.
    pub fn state(&self) -> Result<ProcessState> {
        let sdk = raw::sdk()?;
        let state = raw::map_result(sdk.process_state(self.inner.raw))?;
        Ok(state.into())
    }

    /// Sends a signal to the process.
    pub fn signal(&self, signal: crate::Signal) -> Result<()> {
        let sdk = raw::sdk()?;
        raw::map_result(sdk.signal_process(self.inner.raw, signal.as_raw_signal()))
    }

    /// Returns the exit code.
    pub fn exit_code(&self) -> Result<Option<i32>> {
        let sdk = raw::sdk()?;
        let exit_code = raw::map_result(sdk.process_exit_code(self.inner.raw))?;
        Ok(Some(exit_code))
    }

    /// Waits for the process to finish.
    pub fn wait(&self) -> Result<i32> {
        let sdk = raw::sdk()?;
        let event = raw::map_result(sdk.process_exit_event(self.inner.raw))?;
        const INFINITE: u32 = 0xffff_ffff;
        const WAIT_OBJECT_0: u32 = 0;
        let wait = raw::wait_for_single_object(event, INFINITE);
        if wait != WAIT_OBJECT_0 {
            return Err(Error::InvalidInput(format!(
                "waiting for process exit failed with WAIT result 0x{wait:08x}"
            )));
        }
        self.exit_code().map(|code| code.unwrap_or_default())
    }

    /// Waits for the process and returns captured output.
    pub fn wait_with_output(&self) -> Result<Output> {
        self.wait()?;
        let captured = self
            .inner
            .capture
            .as_ref()
            .map(|c| c.wait_output())
            .unwrap_or_default();
        Ok(Output {
            status: captured.status,
            stdout: captured.stdout,
            stderr: captured.stderr,
        })
    }
}

impl Drop for ProcessInner {
    fn drop(&mut self) {
        if let Ok(sdk) = raw::sdk() {
            let _com = crate::com::try_initialize_mta().ok().flatten();
            let _ = sdk.release_process(self.raw);
        }
    }
}

pub(crate) type CaptureRegistration = raw::CaptureRegistration;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_options_only_capture_when_requested() {
        assert!(!ProcessOptions::new(["/bin/sleep", "30"]).captures_output());
        assert!(ProcessOptions::new(["/bin/echo", "hello"])
            .capture_stdout()
            .captures_output());
        assert!(ProcessOptions::new(["/bin/echo", "hello"])
            .streaming()
            .captures_output());
        assert!(!ProcessOptions::new(["/bin/echo", "hello"])
            .capture_stdout()
            .inherit_output()
            .captures_output());
    }

    #[test]
    fn retained_cstring_array_keeps_values_and_pointer_storage_alive() {
        let array = RetainedCStringArray::new(vec![
            CString::new("first").unwrap(),
            CString::new("second=value").unwrap(),
        ]);

        assert_eq!(array._values.len(), 2);
        assert_eq!(array.pointers().len(), 2);
        assert_eq!(array.pointers()[0], array._values[0].as_ptr());
        assert_eq!(array.pointers()[1], array._values[1].as_ptr());
    }
}
