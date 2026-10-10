//! Backend for WSL Containers via the official `wslc` Rust SDK (`wslcsdk.dll`).
//!
//! There is no CLI path and no demo mode. The app must obtain a live SDK session
//! (download/cache `wslcsdk.dll`, then open a Session) before any operations.

use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;

use anyhow::{anyhow, Result};

#[cfg(windows)]
mod live;
#[cfg(windows)]
mod sdk_fetch;
mod sdk_version;
#[cfg(windows)]
mod syspath;
mod types;

pub use types::*;

pub(super) fn split_ref(reference: &str) -> (String, String) {
    if let Some((repo, tag)) = reference.rsplit_once(':') {
        if !tag.contains('/') {
            return (repo.to_string(), tag.to_string());
        }
    }
    (reference.to_string(), "latest".into())
}

enum Backend {
    #[cfg(windows)]
    Live {
        tx: std::sync::mpsc::Sender<live::Cmd>,
        version: String,
    },
    /// Placeholder so non-Windows builds type-check; never constructed.
    #[cfg(not(windows))]
    #[allow(dead_code)]
    Unsupported,
}

struct ClientInner {
    backend: Backend,
}

impl Drop for ClientInner {
    fn drop(&mut self) {
        #[cfg(windows)]
        if let Backend::Live { tx, .. } = &self.backend {
            let _ = tx.send(live::Cmd::Shutdown);
        }
    }
}

#[derive(Clone)]
pub struct WslcClient {
    inner: Arc<ClientInner>,
}

#[cfg_attr(not(windows), allow(unused_variables))]
impl WslcClient {
    #[cfg(windows)]
    fn from_live(tx: std::sync::mpsc::Sender<live::Cmd>, version: String) -> Self {
        Self {
            inner: Arc::new(ClientInner {
                backend: Backend::Live { tx, version },
            }),
        }
    }

    /// Spawn a background install/connect attempt and stream events on `tx`.
    ///
    /// When `force_redownload` is true, the cached NuGet DLL is discarded first
    /// so Retry reinstalls the SDK.
    pub fn start_setup(force_redownload: bool, tx: Sender<SdkSetupEvent>) {
        thread::Builder::new()
            .name("wslc-sdk-setup".into())
            .spawn(move || Self::run_setup(force_redownload, tx))
            .expect("spawn SDK setup thread");
    }

    fn run_setup(force_redownload: bool, tx: Sender<SdkSetupEvent>) {
        #[cfg(not(windows))]
        {
            let _ = force_redownload;
            let _ = tx.send(SdkSetupEvent::Status(
                "WSLc Desktop requires Windows".into(),
            ));
            let _ = tx.send(SdkSetupEvent::Log(
                "This build is not Windows; the WSL Containers SDK cannot load.".into(),
            ));
            let _ = tx.send(SdkSetupEvent::Progress(0.0));
            let _ = tx.send(SdkSetupEvent::Finished(Err(
                "WSLc Desktop only runs on Windows with WSL Containers".into(),
            )));
        }

        #[cfg(windows)]
        {
            Self::run_setup_windows(force_redownload, tx);
        }
    }

    #[cfg(windows)]
    fn run_setup_windows(force_redownload: bool, tx: Sender<SdkSetupEvent>) {
        let _ = tx.send(SdkSetupEvent::Status(
            "Preparing WSL Containers SDK…".into(),
        ));
        let _ = tx.send(SdkSetupEvent::Progress(2.0));
        let _ = tx.send(SdkSetupEvent::Log(
            "Resolving host wslc/wsl version and matching NuGet package…".into(),
        ));

        let tx_fetch = tx.clone();
        let fetch_result = sdk_fetch::ensure_sdk_dir(force_redownload, move |ev| {
            let mapped = match ev {
                sdk_fetch::FetchEvent::Status(s) => SdkSetupEvent::Status(s),
                sdk_fetch::FetchEvent::Log(s) => SdkSetupEvent::Log(s),
                sdk_fetch::FetchEvent::Progress(p) => {
                    // Reserve 0–80% for download/cache; 80–100% for session boot.
                    SdkSetupEvent::Progress((p * 0.8).clamp(0.0, 80.0))
                }
            };
            let _ = tx_fetch.send(mapped);
        });

        let mut preferred = Vec::new();
        match fetch_result {
            Ok(dir) => {
                let _ = tx.send(SdkSetupEvent::Log(format!(
                    "SDK directory ready: {}",
                    dir.display()
                )));
                preferred.push(dir);
            }
            Err(err) => {
                let _ = tx.send(SdkSetupEvent::Log(format!("SDK install failed: {err:#}")));
                let _ = tx.send(SdkSetupEvent::Progress(0.0));
                let _ = tx.send(SdkSetupEvent::Finished(Err(format!(
                    "SDK install failed: {err:#}"
                ))));
                return;
            }
        }

        let _ = tx.send(SdkSetupEvent::Status("Loading wslcsdk.dll…".into()));
        let _ = tx.send(SdkSetupEvent::Progress(82.0));
        let _ = tx.send(SdkSetupEvent::Log(
            "Prepending SDK path and starting live session…".into(),
        ));
        syspath::prepare_sdk_search_path(&preferred);

        let _ = tx.send(SdkSetupEvent::Progress(88.0));
        match live::spawn_worker() {
            Ok((cmd_tx, version)) => {
                let _ = tx.send(SdkSetupEvent::Log(format!(
                    "WSL Containers SDK session ready (version {version})"
                )));
                let _ = tx.send(SdkSetupEvent::Progress(100.0));
                let _ = tx.send(SdkSetupEvent::Status("Connected".into()));
                let _ = tx.send(SdkSetupEvent::Finished(Ok(Self::from_live(
                    cmd_tx, version,
                ))));
            }
            Err(detail) => {
                let _ = tx.send(SdkSetupEvent::Log(format!(
                    "Live session failed: {detail}"
                )));
                let _ = tx.send(SdkSetupEvent::Progress(0.0));
                let _ = tx.send(SdkSetupEvent::Finished(Err(detail)));
            }
        }
    }

    pub fn status(&self) -> EngineStatus {
        match &self.inner.backend {
            #[cfg(windows)]
            Backend::Live { tx, version } => {
                let (reply_tx, reply_rx) = std::sync::mpsc::channel();
                if tx.send(live::Cmd::Status(reply_tx)).is_ok() {
                    if let Ok(status) = reply_rx.recv() {
                        return status;
                    }
                }
                EngineStatus::unavailable(format!(
                    "SDK session became unavailable (was {version})"
                ))
            }
            #[cfg(not(windows))]
            Backend::Unsupported => EngineStatus::unavailable(
                "WSLc Desktop only runs on Windows with WSL Containers",
            ),
        }
    }

    pub fn list_containers(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        match &self.inner.backend {
            #[cfg(windows)]
            Backend::Live { tx, .. } => live::call(tx, |reply| live::Cmd::ListContainers {
                all,
                reply,
            }),
            #[cfg(not(windows))]
            Backend::Unsupported => Err(anyhow!("WSL Containers SDK unavailable")),
        }
    }

    pub fn list_images(&self) -> Result<Vec<ImageInfo>> {
        match &self.inner.backend {
            #[cfg(windows)]
            Backend::Live { tx, .. } => live::call(tx, |reply| live::Cmd::ListImages { reply }),
            #[cfg(not(windows))]
            Backend::Unsupported => Err(anyhow!("WSL Containers SDK unavailable")),
        }
    }

    pub fn start_container(&self, name_or_id: &str) -> Result<()> {
        match &self.inner.backend {
            #[cfg(windows)]
            Backend::Live { tx, .. } => live::call(tx, |reply| live::Cmd::StartContainer {
                key: name_or_id.to_string(),
                reply,
            }),
            #[cfg(not(windows))]
            Backend::Unsupported => Err(anyhow!("WSL Containers SDK unavailable")),
        }
    }

    pub fn stop_container(&self, name_or_id: &str) -> Result<()> {
        match &self.inner.backend {
            #[cfg(windows)]
            Backend::Live { tx, .. } => live::call(tx, |reply| live::Cmd::StopContainer {
                key: name_or_id.to_string(),
                reply,
            }),
            #[cfg(not(windows))]
            Backend::Unsupported => Err(anyhow!("WSL Containers SDK unavailable")),
        }
    }

    pub fn remove_container(&self, name_or_id: &str) -> Result<()> {
        match &self.inner.backend {
            #[cfg(windows)]
            Backend::Live { tx, .. } => live::call(tx, |reply| live::Cmd::RemoveContainer {
                key: name_or_id.to_string(),
                reply,
            }),
            #[cfg(not(windows))]
            Backend::Unsupported => Err(anyhow!("WSL Containers SDK unavailable")),
        }
    }

    pub fn remove_image(&self, reference: &str) -> Result<()> {
        match &self.inner.backend {
            #[cfg(windows)]
            Backend::Live { tx, .. } => live::call(tx, |reply| live::Cmd::RemoveImage {
                reference: reference.to_string(),
                reply,
            }),
            #[cfg(not(windows))]
            Backend::Unsupported => Err(anyhow!("WSL Containers SDK unavailable")),
        }
    }

    pub fn create_container(&self, request: CreateContainerRequest) -> Result<String> {
        if request.image.trim().is_empty() {
            return Err(anyhow!("image reference is empty"));
        }

        match &self.inner.backend {
            #[cfg(windows)]
            Backend::Live { tx, .. } => {
                live::call(tx, |reply| live::Cmd::CreateContainer { request, reply })
            }
            #[cfg(not(windows))]
            Backend::Unsupported => Err(anyhow!("WSL Containers SDK unavailable")),
        }
    }

    pub fn container_logs(&self, name_or_id: &str) -> Result<String> {
        match &self.inner.backend {
            #[cfg(windows)]
            Backend::Live { tx, .. } => live::call(tx, |reply| live::Cmd::ContainerLogs {
                key: name_or_id.to_string(),
                reply,
            }),
            #[cfg(not(windows))]
            Backend::Unsupported => Err(anyhow!("WSL Containers SDK unavailable")),
        }
    }

    pub fn list_volumes(&self) -> Result<Vec<VolumeInfo>> {
        match &self.inner.backend {
            #[cfg(windows)]
            Backend::Live { tx, .. } => live::call(tx, |reply| live::Cmd::ListVolumes { reply }),
            #[cfg(not(windows))]
            Backend::Unsupported => Err(anyhow!("WSL Containers SDK unavailable")),
        }
    }

    pub fn create_volume(&self, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(anyhow!("volume name is empty"));
        }

        match &self.inner.backend {
            #[cfg(windows)]
            Backend::Live { tx, .. } => live::call(tx, |reply| live::Cmd::CreateVolume {
                name: name.to_string(),
                reply,
            }),
            #[cfg(not(windows))]
            Backend::Unsupported => Err(anyhow!("WSL Containers SDK unavailable")),
        }
    }

    pub fn remove_volume(&self, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(anyhow!("volume name is empty"));
        }

        match &self.inner.backend {
            #[cfg(windows)]
            Backend::Live { tx, .. } => live::call(tx, |reply| live::Cmd::RemoveVolume {
                name: name.to_string(),
                reply,
            }),
            #[cfg(not(windows))]
            Backend::Unsupported => Err(anyhow!("WSL Containers SDK unavailable")),
        }
    }

    /// Spawn a background pull and stream status / progress / logs on `tx`.
    pub fn start_pull(&self, reference: &str, tx: Sender<PullEvent>) {
        let reference = reference.trim().to_string();
        match &self.inner.backend {
            #[cfg(windows)]
            Backend::Live { tx: cmd_tx, .. } => {
                let _ = cmd_tx.send(live::Cmd::StartPull {
                    reference,
                    events: tx,
                });
            }
            #[cfg(not(windows))]
            Backend::Unsupported => {
                let _ = tx.send(PullEvent::Finished(Err(
                    "WSL Containers SDK unavailable".into(),
                )));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_refs() {
        assert_eq!(split_ref("nginx:alpine"), ("nginx".into(), "alpine".into()));
        assert_eq!(
            split_ref("docker.io/library/alpine"),
            ("docker.io/library/alpine".into(), "latest".into())
        );
    }

    #[test]
    fn unavailable_status_helper() {
        let s = EngineStatus::unavailable("no sdk");
        assert_eq!(s.backend, BackendKind::Unavailable);
        assert_eq!(s.message, "no sdk");
    }
}
