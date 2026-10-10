//! Backend for WSL Containers via the official `wslc` Rust SDK (`wslcsdk.dll`).
//!
//! There is no CLI path. When the SDK / runtime is unavailable (or the host is
//! not Windows) the client uses an in-memory demo dataset so the UI stays
//! previewable.

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};

#[cfg(windows)]
mod live;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // `Live` is constructed on Windows SDK builds.
pub enum BackendKind {
    Live,
    Demo,
}

#[derive(Debug, Clone)]
pub struct EngineStatus {
    pub backend: BackendKind,
    /// Engine label shown in the status bar (`wslcsdk` or `demo`).
    pub binary: String,
    pub version: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub ports: String,
    pub created: String,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeInfo {
    pub name: String,
    pub driver: String,
    pub mountpoint: String,
    pub created: String,
    pub size: String,
}

#[derive(Debug, Clone)]
pub enum PullEvent {
    Status(String),
    Progress(f32),
    Log(String),
    Finished(Result<(), String>),
}

#[derive(Clone, Debug, Default)]
struct DemoState {
    containers: Vec<ContainerInfo>,
    images: Vec<ImageInfo>,
    volumes: Vec<VolumeInfo>,
}

fn demo_state() -> &'static Mutex<DemoState> {
    static STATE: OnceLock<Mutex<DemoState>> = OnceLock::new();
    STATE.get_or_init(|| {
        Mutex::new(DemoState {
            containers: vec![
                ContainerInfo {
                    id: "a1b2c3d4e5f6".into(),
                    name: "web".into(),
                    image: "nginx:alpine".into(),
                    status: "Up 2 hours".into(),
                    ports: "0.0.0.0:8080->80/tcp".into(),
                    created: "2 hours ago".into(),
                },
                ContainerInfo {
                    id: "f6e5d4c3b2a1".into(),
                    name: "redis-dev".into(),
                    image: "redis:7".into(),
                    status: "Exited (0) 10 minutes ago".into(),
                    ports: "".into(),
                    created: "1 day ago".into(),
                },
                ContainerInfo {
                    id: "112233445566".into(),
                    name: "hello".into(),
                    image: "hello-world".into(),
                    status: "Exited (0) 3 days ago".into(),
                    ports: "".into(),
                    created: "3 days ago".into(),
                },
            ],
            images: vec![
                ImageInfo {
                    repository: "nginx".into(),
                    tag: "alpine".into(),
                    id: "sha256:9beeda1".into(),
                    size: "42.2 MB".into(),
                    created: "2 weeks ago".into(),
                },
                ImageInfo {
                    repository: "redis".into(),
                    tag: "7".into(),
                    id: "sha256:7c1bb1".into(),
                    size: "117 MB".into(),
                    created: "1 month ago".into(),
                },
                ImageInfo {
                    repository: "hello-world".into(),
                    tag: "latest".into(),
                    id: "sha256:d2c94e".into(),
                    size: "13.3 kB".into(),
                    created: "3 months ago".into(),
                },
                ImageInfo {
                    repository: "ubuntu".into(),
                    tag: "24.04".into(),
                    id: "sha256:c6ad44".into(),
                    size: "78.1 MB".into(),
                    created: "5 weeks ago".into(),
                },
            ],
            volumes: vec![
                VolumeInfo {
                    name: "app-data".into(),
                    driver: "vhd".into(),
                    mountpoint: "session:app-data".into(),
                    created: "3 days ago".into(),
                    size: "128 MB".into(),
                },
                VolumeInfo {
                    name: "postgres-data".into(),
                    driver: "vhd".into(),
                    mountpoint: "session:postgres-data".into(),
                    created: "1 week ago".into(),
                    size: "512 MB".into(),
                },
                VolumeInfo {
                    name: "cache".into(),
                    driver: "vhd".into(),
                    mountpoint: "session:cache".into(),
                    created: "2 weeks ago".into(),
                    size: "64 MB".into(),
                },
            ],
        })
    })
}

fn now_label() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() % 10_000)
        .unwrap_or(0);
    format!("demo-{secs}")
}

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
    Demo {
        /// Last detect error detail (empty when SDK was simply missing).
        detail: String,
    },
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

impl WslcClient {
    fn from_backend(backend: Backend) -> Self {
        Self {
            inner: Arc::new(ClientInner { backend }),
        }
    }

    pub fn detect() -> Self {
        #[cfg(windows)]
        {
            match live::spawn_worker() {
                Ok((tx, version)) => Self::from_backend(Backend::Live { tx, version }),
                Err(detail) => Self::from_backend(Backend::Demo {
                    detail: classify_demo_detail(&detail),
                }),
            }
        }
        #[cfg(not(windows))]
        {
            Self::from_backend(Backend::Demo {
                detail: String::new(),
            })
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
                EngineStatus {
                    backend: BackendKind::Demo,
                    binary: "wslcsdk".into(),
                    version: version.clone(),
                    message: "SDK session became unavailable; using demo data".into(),
                }
            }
            Backend::Demo { detail } if detail.is_empty() => EngineStatus {
                backend: BackendKind::Demo,
                binary: "demo".into(),
                version: "demo-0.1".into(),
                message: "WSL Containers SDK not found — showing demo data".into(),
            },
            Backend::Demo { detail } => EngineStatus {
                backend: BackendKind::Demo,
                binary: "demo".into(),
                version: "demo-0.1".into(),
                message: format!("SDK unavailable ({detail}); using demo data"),
            },
        }
    }

    #[allow(dead_code)]
    pub fn is_demo(&self) -> bool {
        matches!(self.inner.backend, Backend::Demo { .. })
    }

    pub fn list_containers(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        match &self.inner.backend {
            Backend::Demo { .. } => {
                let rows = demo_state().lock().expect("demo lock").containers.clone();
                if all {
                    Ok(rows)
                } else {
                    Ok(rows.into_iter().filter(|c| c.is_running()).collect())
                }
            }
            #[cfg(windows)]
            Backend::Live { tx, .. } => live::call(tx, |reply| live::Cmd::ListContainers {
                all,
                reply,
            }),
        }
    }

    pub fn list_images(&self) -> Result<Vec<ImageInfo>> {
        match &self.inner.backend {
            Backend::Demo { .. } => Ok(demo_state().lock().expect("demo lock").images.clone()),
            #[cfg(windows)]
            Backend::Live { tx, .. } => live::call(tx, |reply| live::Cmd::ListImages { reply }),
        }
    }

    pub fn start_container(&self, name_or_id: &str) -> Result<()> {
        match &self.inner.backend {
            Backend::Demo { .. } => {
                let mut state = demo_state().lock().expect("demo lock");
                let c = state
                    .containers
                    .iter_mut()
                    .find(|c| c.name == name_or_id || c.id == name_or_id)
                    .ok_or_else(|| anyhow!("container not found: {name_or_id}"))?;
                c.status = "Up Just now".into();
                Ok(())
            }
            #[cfg(windows)]
            Backend::Live { tx, .. } => live::call(tx, |reply| live::Cmd::StartContainer {
                key: name_or_id.to_string(),
                reply,
            }),
        }
    }

    pub fn stop_container(&self, name_or_id: &str) -> Result<()> {
        match &self.inner.backend {
            Backend::Demo { .. } => {
                let mut state = demo_state().lock().expect("demo lock");
                let c = state
                    .containers
                    .iter_mut()
                    .find(|c| c.name == name_or_id || c.id == name_or_id)
                    .ok_or_else(|| anyhow!("container not found: {name_or_id}"))?;
                c.status = "Exited (0) Just now".into();
                c.ports.clear();
                Ok(())
            }
            #[cfg(windows)]
            Backend::Live { tx, .. } => live::call(tx, |reply| live::Cmd::StopContainer {
                key: name_or_id.to_string(),
                reply,
            }),
        }
    }

    pub fn remove_container(&self, name_or_id: &str) -> Result<()> {
        match &self.inner.backend {
            Backend::Demo { .. } => {
                let mut state = demo_state().lock().expect("demo lock");
                let before = state.containers.len();
                state
                    .containers
                    .retain(|c| c.name != name_or_id && c.id != name_or_id);
                if state.containers.len() == before {
                    return Err(anyhow!("container not found: {name_or_id}"));
                }
                Ok(())
            }
            #[cfg(windows)]
            Backend::Live { tx, .. } => live::call(tx, |reply| live::Cmd::RemoveContainer {
                key: name_or_id.to_string(),
                reply,
            }),
        }
    }

    pub fn remove_image(&self, reference: &str) -> Result<()> {
        match &self.inner.backend {
            Backend::Demo { .. } => {
                let mut state = demo_state().lock().expect("demo lock");
                let before = state.images.len();
                state
                    .images
                    .retain(|img| img.reference() != reference && img.id != reference);
                if state.images.len() == before {
                    return Err(anyhow!("image not found: {reference}"));
                }
                Ok(())
            }
            #[cfg(windows)]
            Backend::Live { tx, .. } => live::call(tx, |reply| live::Cmd::RemoveImage {
                reference: reference.to_string(),
                reply,
            }),
        }
    }

    pub fn run_detached(&self, image: &str, name: Option<&str>) -> Result<String> {
        let image = image.trim();
        if image.is_empty() {
            return Err(anyhow!("image reference is empty"));
        }

        match &self.inner.backend {
            Backend::Demo { .. } => {
                let mut state = demo_state().lock().expect("demo lock");
                let name = name
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(now_label);
                let id = format!("{:x}", name.len() * 97 + image.len() * 13);
                state.containers.insert(
                    0,
                    ContainerInfo {
                        id: id.clone(),
                        name: name.clone(),
                        image: image.to_string(),
                        status: "Up Just now".into(),
                        ports: "".into(),
                        created: "Just now".into(),
                    },
                );
                if !state.images.iter().any(|i| i.reference() == image) {
                    let (repo, tag) = split_ref(image);
                    state.images.insert(
                        0,
                        ImageInfo {
                            repository: repo,
                            tag,
                            id: format!("sha256:{id}"),
                            size: "—".into(),
                            created: "Just now".into(),
                        },
                    );
                }
                Ok(name)
            }
            #[cfg(windows)]
            Backend::Live { tx, .. } => live::call(tx, |reply| live::Cmd::RunDetached {
                image: image.to_string(),
                name: name.map(str::to_string),
                reply,
            }),
        }
    }

    pub fn list_volumes(&self) -> Result<Vec<VolumeInfo>> {
        match &self.inner.backend {
            Backend::Demo { .. } => Ok(demo_state().lock().expect("demo lock").volumes.clone()),
            #[cfg(windows)]
            Backend::Live { tx, .. } => live::call(tx, |reply| live::Cmd::ListVolumes { reply }),
        }
    }

    pub fn create_volume(&self, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(anyhow!("volume name is empty"));
        }

        match &self.inner.backend {
            Backend::Demo { .. } => {
                let mut state = demo_state().lock().expect("demo lock");
                if state.volumes.iter().any(|v| v.name == name) {
                    return Err(anyhow!("volume already exists: {name}"));
                }
                state.volumes.insert(
                    0,
                    VolumeInfo {
                        name: name.to_string(),
                        driver: "vhd".into(),
                        mountpoint: format!("session:{name}"),
                        created: "Just now".into(),
                        size: "0 B".into(),
                    },
                );
                Ok(())
            }
            #[cfg(windows)]
            Backend::Live { tx, .. } => live::call(tx, |reply| live::Cmd::CreateVolume {
                name: name.to_string(),
                reply,
            }),
        }
    }

    pub fn remove_volume(&self, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(anyhow!("volume name is empty"));
        }

        match &self.inner.backend {
            Backend::Demo { .. } => {
                let mut state = demo_state().lock().expect("demo lock");
                let before = state.volumes.len();
                state.volumes.retain(|v| v.name != name);
                if state.volumes.len() == before {
                    return Err(anyhow!("volume not found: {name}"));
                }
                Ok(())
            }
            #[cfg(windows)]
            Backend::Live { tx, .. } => live::call(tx, |reply| live::Cmd::RemoveVolume {
                name: name.to_string(),
                reply,
            }),
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
            Backend::Demo { .. } => {
                thread::spawn(move || {
                    if reference.is_empty() {
                        let _ =
                            tx.send(PullEvent::Finished(Err("image reference is empty".into())));
                        return;
                    }
                    let _ = tx.send(PullEvent::Status(format!("Pulling {reference}…")));
                    let _ = tx.send(PullEvent::Progress(0.0));
                    demo_pull(&reference, &tx);
                });
            }
        }
    }
}

/// Empty detail → show the generic “SDK missing” i18n string.
#[allow(dead_code)] // only referenced from the Windows `detect` path
fn classify_demo_detail(detail: &str) -> String {
    let lower = detail.to_ascii_lowercase();
    if lower.contains("sdk not found")
        || lower.contains("unsupported platform")
        || lower.contains("missing wsl components")
        || lower.contains("wslcsdk.dll")
    {
        String::new()
    } else {
        detail.to_string()
    }
}

fn demo_pull(reference: &str, tx: &Sender<PullEvent>) {
    let (repo, tag) = split_ref(reference);
    let layers = [
        ("Resolving manifest", 8.0),
        ("Pulling fs layer sha256:aaa111", 22.0),
        ("Downloading [=========>         ]  4.2MB/9.1MB", 45.0),
        ("Downloading [=================> ]  8.4MB/9.1MB", 68.0),
        ("Download complete", 78.0),
        ("Extracting layers", 90.0),
        ("Pull complete", 97.0),
    ];

    let _ = tx.send(PullEvent::Log(format!(
        "{repo}:{tag}: Pulling from registry (demo)"
    )));
    for (line, progress) in layers {
        thread::sleep(Duration::from_millis(220));
        let _ = tx.send(PullEvent::Log(line.into()));
        let _ = tx.send(PullEvent::Progress(progress));
        let _ = tx.send(PullEvent::Status(line.into()));
    }

    {
        let mut state = demo_state().lock().expect("demo lock");
        if !state.images.iter().any(|i| {
            i.reference() == format!("{repo}:{tag}") || (i.repository == repo && i.tag == tag)
        }) {
            state.images.insert(
                0,
                ImageInfo {
                    repository: repo.clone(),
                    tag: tag.clone(),
                    id: format!("sha256:{:x}", repo.len() * 31 + tag.len() * 17),
                    size: "—".into(),
                    created: "Just now".into(),
                },
            );
        }
    }

    let _ = tx.send(PullEvent::Log(format!(
        "Status: Downloaded newer image for {repo}:{tag}"
    )));
    let _ = tx.send(PullEvent::Progress(100.0));
    let _ = tx.send(PullEvent::Status("Pull complete".into()));
    let _ = tx.send(PullEvent::Finished(Ok(())));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_roundtrip_start_stop() {
        let client = WslcClient::from_backend(Backend::Demo {
            detail: String::new(),
        });
        let before = client.list_containers(true).unwrap();
        assert!(!before.is_empty());
        let name = before[1].name.clone();
        client.start_container(&name).unwrap();
        let running = client
            .list_containers(true)
            .unwrap()
            .into_iter()
            .find(|c| c.name == name)
            .unwrap();
        assert!(running.is_running());
        client.stop_container(&name).unwrap();
    }

    #[test]
    fn demo_volume_create_remove() {
        let client = WslcClient::from_backend(Backend::Demo {
            detail: String::new(),
        });
        let name = format!("vol-{}", now_label());
        client.create_volume(&name).unwrap();
        assert!(
            client
                .list_volumes()
                .unwrap()
                .iter()
                .any(|v| v.name == name)
        );
        client.remove_volume(&name).unwrap();
        assert!(
            !client
                .list_volumes()
                .unwrap()
                .iter()
                .any(|v| v.name == name)
        );
    }

    #[test]
    fn formats_refs() {
        assert_eq!(split_ref("nginx:alpine"), ("nginx".into(), "alpine".into()));
        assert_eq!(
            split_ref("docker.io/library/alpine"),
            ("docker.io/library/alpine".into(), "latest".into())
        );
    }
}
