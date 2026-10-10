use std::ffi::OsString;
use std::process::Command;

use crate::{Error, Result};

/// Options for reading container logs.
///
/// These mirror the WSLC CLI / service `Logs` surface (`follow`, `timestamps`,
/// `details`, `since`, `until`, `tail`). The public C SDK does not yet export a
/// logs entry point, so the safe wrapper retrieves logs through `wslc.exe`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LogsOptions {
    /// Keep streaming new log lines until the stream closes.
    pub follow: bool,
    /// Prefix each line with a timestamp.
    pub timestamps: bool,
    /// Include extra log details when supported by the runtime.
    pub details: bool,
    /// Only return logs at or after this Unix timestamp (seconds).
    pub since: Option<i64>,
    /// Only return logs at or before this Unix timestamp (seconds).
    pub until: Option<i64>,
    /// Return only the last `n` lines. `None` means no tail limit.
    pub tail: Option<u64>,
}

impl LogsOptions {
    /// Creates default logs options (no follow, no timestamps, full history).
    pub fn new() -> Self {
        Self::default()
    }

    /// Enables follow mode.
    pub fn follow(mut self, enable: bool) -> Self {
        self.follow = enable;
        self
    }

    /// Enables timestamps.
    pub fn timestamps(mut self, enable: bool) -> Self {
        self.timestamps = enable;
        self
    }

    /// Enables detailed log output.
    pub fn details(mut self, enable: bool) -> Self {
        self.details = enable;
        self
    }

    /// Sets the lower bound timestamp (Unix seconds). Use `0` for "from the beginning".
    pub fn since(mut self, since: i64) -> Self {
        self.since = Some(since);
        self
    }

    /// Sets the upper bound timestamp (Unix seconds).
    pub fn until(mut self, until: i64) -> Self {
        self.until = Some(until);
        self
    }

    /// Limits output to the last `n` lines. `n` must be greater than zero.
    pub fn tail(mut self, n: u64) -> Self {
        self.tail = Some(n);
        self
    }

    /// Validates options before invoking the logs backend.
    pub fn validate(&self) -> Result<()> {
        if let Some(0) = self.tail {
            return Err(Error::InvalidInput(
                "logs tail must be greater than zero".to_owned(),
            ));
        }
        if let (Some(since), Some(until)) = (self.since, self.until) {
            if since > until {
                return Err(Error::InvalidInput(
                    "logs since cannot be greater than until".to_owned(),
                ));
            }
        }
        Ok(())
    }

    /// Builds the bitflags representation matching `WSLCLogsFlags`.
    pub fn as_flags(&self) -> u32 {
        let mut flags = wslc_sys::WSLC_LOGS_FLAG_NONE;
        if self.follow {
            flags |= wslc_sys::WSLC_LOGS_FLAG_FOLLOW;
        }
        if self.timestamps {
            flags |= wslc_sys::WSLC_LOGS_FLAG_TIMESTAMPS;
        }
        if self.details {
            flags |= wslc_sys::WSLC_LOGS_FLAG_DETAILS;
        }
        flags
    }

    /// Builds `wslc` CLI arguments for this options set.
    ///
    /// `--session` is global and must appear before the `container logs` subcommand.
    pub fn to_cli_args(&self, session_name: &str, container_id: &str) -> Result<Vec<OsString>> {
        self.validate()?;
        if session_name.trim().is_empty() {
            return Err(Error::InvalidInput(
                "session name cannot be empty".to_owned(),
            ));
        }
        if session_name.contains('\0') {
            return Err(Error::Nul("session name".to_owned()));
        }
        if container_id.trim().is_empty() {
            return Err(Error::InvalidInput(
                "container id cannot be empty".to_owned(),
            ));
        }
        if container_id.contains('\0') {
            return Err(Error::Nul("container id".to_owned()));
        }

        let mut args = vec![
            OsString::from("--session"),
            OsString::from(session_name),
            OsString::from("container"),
            OsString::from("logs"),
        ];
        if self.follow {
            args.push(OsString::from("--follow"));
        }
        if self.timestamps {
            args.push(OsString::from("--timestamps"));
        }
        if self.details {
            args.push(OsString::from("--details"));
        }
        if let Some(since) = self.since {
            args.push(OsString::from("--since"));
            args.push(OsString::from(since.to_string()));
        }
        if let Some(until) = self.until {
            args.push(OsString::from("--until"));
            args.push(OsString::from(until.to_string()));
        }
        if let Some(tail) = self.tail {
            args.push(OsString::from("--tail"));
            args.push(OsString::from(tail.to_string()));
        }
        args.push(OsString::from(container_id));
        Ok(args)
    }
}

/// Captured container log output.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContainerLogs {
    /// Log bytes written to stdout by `wslc container logs`.
    pub stdout: Vec<u8>,
    /// Log bytes written to stderr by `wslc container logs`.
    pub stderr: Vec<u8>,
}

impl ContainerLogs {
    /// Interprets stdout as UTF-8 lossily.
    pub fn stdout_lossy(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.stdout)
    }

    /// Interprets stderr as UTF-8 lossily.
    pub fn stderr_lossy(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.stderr)
    }
}

pub(crate) fn fetch_container_logs(
    session_name: &str,
    container_id: &str,
    options: &LogsOptions,
) -> Result<ContainerLogs> {
    let args = options.to_cli_args(session_name, container_id)?;
    let program = std::env::var_os("WSLC_CLI_PATH").unwrap_or_else(|| OsString::from("wslc"));

    let output = Command::new(&program).args(&args).output().map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            Error::CommandNotFound(format!(
                "could not find `{}`; install the WSL container CLI or set WSLC_CLI_PATH",
                PathDisplay(&program)
            ))
        } else {
            Error::Io(err)
        }
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let message = if stderr.is_empty() {
            format!(
                "`{}` exited with {}",
                PathDisplay(&program),
                status_code(&output.status)
            )
        } else {
            format!(
                "`{}` exited with {}: {stderr}",
                PathDisplay(&program),
                status_code(&output.status)
            )
        };
        return Err(Error::CommandFailed {
            status: output.status.code(),
            message,
        });
    }

    Ok(ContainerLogs {
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

fn status_code(status: &std::process::ExitStatus) -> String {
    status
        .code()
        .map(|code| code.to_string())
        .unwrap_or_else(|| "signal".to_owned())
}

struct PathDisplay<'a>(&'a OsString);

impl std::fmt::Display for PathDisplay<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.to_string_lossy())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_args_put_session_before_subcommand() {
        let args = LogsOptions::new()
            .timestamps(true)
            .tail(2)
            .to_cli_args("demo-session", "abc123")
            .expect("args");
        let rendered: Vec<_> = args
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            rendered,
            vec![
                "--session",
                "demo-session",
                "container",
                "logs",
                "--timestamps",
                "--tail",
                "2",
                "abc123",
            ]
        );
    }

    #[test]
    fn flags_match_shared_idl_bits() {
        let options = LogsOptions::new()
            .follow(true)
            .timestamps(true)
            .details(true);
        assert_eq!(
            options.as_flags(),
            wslc_sys::WSLC_LOGS_FLAG_FOLLOW
                | wslc_sys::WSLC_LOGS_FLAG_TIMESTAMPS
                | wslc_sys::WSLC_LOGS_FLAG_DETAILS
        );
        assert_eq!(options.as_flags() & !wslc_sys::WSLC_LOGS_FLAGS_VALID, 0);
    }

    #[test]
    fn rejects_zero_tail() {
        let err = LogsOptions::new().tail(0).validate().unwrap_err();
        assert!(matches!(err, Error::InvalidInput(message) if message.contains("tail")));
    }
}
